"""Matte the WorkBuddy idle sample against its changing green/black background.

Deterministic video post-processing: original opaque RGB and geometry stay intact;
only the narrow soft edge is unmixed from locally sampled background colors.
"""

import argparse
import hashlib
import json
import subprocess
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw
from scipy import ndimage
from video_runtime import ffmpeg, video_info


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "artifacts/design/preprocessed/source-videos/workbuddy-idle-v1/idle-original.mp4"
OUTPUT = ROOT / "artifacts/design/preprocessed/frames/workbuddy-idle-cutout-v1"
CANVAS = (1920, 1080)
# Processing window only. Coordinates remain the same within the original canvas.
ROI = (640, 128, 1280, 1008)
ROI_SIZE = (ROI[2] - ROI[0], ROI[3] - ROI[1])
CANVAS_FILTER = (
    f"scale={CANVAS[0]}:{CANVAS[1]}:force_original_aspect_ratio=decrease,"
    f"pad={CANVAS[0]}:{CANVAS[1]}:(ow-iw)/2:(oh-ih)/2:color=0x00ff00"
)


def matte_frame(rgb, reference_background=None, reference_opaque=None):
    color = rgb.astype(np.float32)
    maximum = color.max(axis=2)
    excess = color[:, :, 1] - np.maximum(color[:, :, 0], color[:, :, 2])
    # A fixed tiny green threshold would punch holes in compressed pale skin
    # and eye highlights. Require stronger dominance for bright foregrounds.
    green_threshold = np.maximum(12, np.minimum(45, maximum * 0.2))
    green_screen = (excess > green_threshold) & (color[:, :, 1] > 30)
    very_dark = maximum < 24
    possible_background = green_screen | very_dark
    holdout = np.zeros(possible_background.shape, dtype=bool)
    if reference_opaque is not None:
        # A conservative interior holdout stops black lashes/pupils connecting
        # to exterior black via a thin outline during a blink. It never copies
        # the reference colors or fixes the moving outer silhouette.
        holdout = reference_opaque & (excess < 100)
        possible_background &= ~holdout
    seeds = np.zeros(possible_background.shape, dtype=bool)
    seeds[0] = possible_background[0]
    seeds[-1] = possible_background[-1]
    seeds[:, 0] = possible_background[:, 0]
    seeds[:, -1] = possible_background[:, -1]
    connected_background = ndimage.binary_propagation(seeds, mask=possible_background)
    removed_holes = 0
    if reference_background is not None:
        # Black background also occupies enclosed gaps between moving strands.
        # Use the clean green first frame only to classify these dark islands;
        # do not transfer its silhouette or replace any current-frame artwork.
        dark_labels, dark_count = ndimage.label(very_dark & ~connected_background & ~holdout)
        if dark_count:
            sizes = np.bincount(dark_labels.ravel(), minlength=dark_count + 1)
            overlap = np.bincount(dark_labels[reference_background].ravel(), minlength=dark_count + 1)
            hole_labels = np.flatnonzero((sizes >= 4) & (overlap / np.maximum(sizes, 1) >= 0.05))
            hole_labels = hole_labels[hole_labels != 0]
            if len(hole_labels):
                holes = np.isin(dark_labels, hole_labels)
                connected_background |= holes
                removed_holes = int(holes.sum())
    # Green is absent from this character's palette. Enclosed deep dark pixels
    # stay in the foreground, preserving pupils, lashes and internal line work.
    support = ~(connected_background | green_screen) | holdout
    labels, count = ndimage.label(support)
    if not count:
        raise ValueError("Frame has no foreground")
    counts = np.bincount(labels.ravel())
    counts[0] = 0
    main = labels == counts.argmax()
    distance = ndimage.distance_transform_edt(~main)
    support &= distance <= 4
    sure_foreground = ndimage.binary_erosion(support, iterations=2)
    if not sure_foreground.any():
        raise ValueError("Frame has no solid foreground")
    extended = ndimage.binary_dilation(support, iterations=4)
    sure_background = ~ndimage.binary_dilation(support, iterations=5)
    foreground_index = ndimage.distance_transform_edt(
        ~sure_foreground, return_distances=False, return_indices=True
    )
    background_index = ndimage.distance_transform_edt(
        ~sure_background, return_distances=False, return_indices=True
    )
    edge = extended & ~sure_foreground
    foreground = color[foreground_index[0][edge], foreground_index[1][edge]]
    background = color[background_index[0][edge], background_index[1][edge]]
    direction = foreground - background
    denominator = np.maximum(np.sum(direction * direction, axis=1), 1)
    coverage = np.clip(np.sum((color[edge] - background) * direction, axis=1) / denominator, 0, 1)
    alpha = np.zeros(support.shape, dtype=np.uint8)
    alpha[sure_foreground] = 255
    alpha[edge] = np.rint(coverage * 255).astype(np.uint8)
    alpha[alpha <= 3] = 0
    rgba = np.zeros((*rgb.shape[:2], 4), dtype=np.uint8)
    rgba[sure_foreground, :3] = rgb[sure_foreground]
    rgba[edge, :3] = np.rint(foreground).astype(np.uint8)
    rgba[:, :, 3] = alpha
    rgba[alpha == 0] = 0
    assert np.array_equal(rgba[sure_foreground, :3], rgb[sure_foreground])
    if np.any(alpha[0]) or np.any(alpha[-1]) or np.any(alpha[:, 0]) or np.any(alpha[:, -1]):
        raise ValueError("Foreground reaches processing ROI edge")
    image = Image.fromarray(rgba)
    bounds = image.getchannel("A").getbbox()
    solid = image.getchannel("A").point(lambda a: 255 if a >= 128 else 0).getbbox()
    return image, {
        "alphaBoundsInRoi": list(bounds),
        "solidBoundsInRoi": list(solid),
        "opaqueRgbPreserved": True,
        "opaquePixels": int(sure_foreground.sum()),
        "softEdgePixels": int(np.count_nonzero((alpha > 0) & (alpha < 255))),
        "referenceClassifiedDarkBackgroundPixels": removed_holes,
        "greenResiduePixels": int(np.count_nonzero((alpha > 32) & (rgba[:, :, 1].astype(int) - np.maximum(rgba[:, :, 0], rgba[:, :, 2]).astype(int) > 45))),
    }


def decode_frame(source, time):
    raw = subprocess.check_output([
        ffmpeg(), "-v", "error", "-ss", str(time), "-i", str(source),
        "-vf", f"{CANVAS_FILTER},crop={ROI_SIZE[0]}:{ROI_SIZE[1]}:{ROI[0]}:{ROI[1]}",
        "-frames:v", "1", "-an", "-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1",
    ])
    return np.frombuffer(raw, np.uint8).reshape((ROI_SIZE[1], ROI_SIZE[0], 3))


def checker(size, cell=24):
    y, x = np.indices((size[1], size[0]))
    colors = np.where(((x // cell + y // cell) % 2)[:, :, None], 220, 250)
    return Image.fromarray(np.repeat(colors, 3, axis=2).astype(np.uint8)).convert("RGBA")


def contact_sheet(items, destination):
    width, height = 360, 440
    sheet = Image.new("RGB", (width * 3, height * len(items)), "#eeeeee")
    draw = ImageDraw.Draw(sheet)
    for row, (label, image) in enumerate(items):
        # An inspection crop only; does not alter saved frame coordinates.
        preview = image.crop((112, 56, 528, 824)).resize((216, 400), Image.Resampling.LANCZOS)
        for column, name in enumerate(("white", "dark", "checker")):
            panel = checker((width, height - 28)) if name == "checker" else Image.new("RGBA", (width, height - 28), "#ffffff" if name == "white" else "#252a36")
            panel.alpha_composite(preview, (72, 6))
            sheet.paste(panel.convert("RGB"), (column * width, row * height + 28))
            draw.text((column * width + 10, row * height + 8), f"{label} / {name}", fill="black")
    sheet.save(destination)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=SOURCE)
    parser.add_argument("--output", type=Path, default=OUTPUT)
    parser.add_argument("--samples-only", action="store_true")
    parser.add_argument("--skip-animation", action="store_true", help="Validate PNGs before spending time encoding the lossless animation")
    args = parser.parse_args()
    output = args.output
    output.mkdir(parents=True, exist_ok=True)
    info = video_info(args.source)
    rate = info["fps"]
    first_matte, _ = matte_frame(decode_frame(args.source, 0))
    reference_background = ndimage.binary_dilation(np.asarray(first_matte)[:, :, 3] <= 16, iterations=3)
    reference_opaque = ndimage.binary_erosion(np.asarray(first_matte)[:, :, 3] >= 254, iterations=12)
    if args.samples_only:
        items, records = [], []
        probe = info["probe"]
        duration = float(probe.get("format", {}).get("duration", probe.get("duration", 0)))
        end = max(0, duration - 1 / rate)
        for time in (0, min(0.5, end), min(2, end), min(5, end), max(0, end - 0.5), end):
            image, record = matte_frame(decode_frame(args.source, time), reference_background, reference_opaque)
            image.save(output / f"sample-{time:05.2f}.png")
            items.append((f"{time:.2f}s", image))
            records.append({"time": time, **record})
        contact_sheet(items, output / "sample-contact-sheet.png")
        (output / "sample-validation.json").write_text(json.dumps(records, indent=2) + "\n", encoding="utf-8")
        print(json.dumps({"samples": len(items), "output": str(output)}, ensure_ascii=False), flush=True)
        return

    frames_directory = output / "rgba-roi"
    frames_directory.mkdir(exist_ok=True)
    process = subprocess.Popen([
        ffmpeg(), "-v", "error", "-i", str(args.source),
        "-vf", f"{CANVAS_FILTER},crop={ROI_SIZE[0]}:{ROI_SIZE[1]}:{ROI[0]}:{ROI[1]}",
        "-an", "-fps_mode", "passthrough", "-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1",
    ], stdout=subprocess.PIPE)
    records, frame_index = [], 0
    byte_count = ROI_SIZE[0] * ROI_SIZE[1] * 3
    try:
        while True:
            raw = process.stdout.read(byte_count)
            if not raw:
                break
            if len(raw) != byte_count:
                raise ValueError("Incomplete decoded frame")
            rgb = np.frombuffer(raw, np.uint8).reshape((ROI_SIZE[1], ROI_SIZE[0], 3))
            image, record = matte_frame(rgb, reference_background, reference_opaque)
            image.save(frames_directory / f"{frame_index:04d}.png")
            records.append({"frame": frame_index, "time": frame_index / rate, **record})
            frame_index += 1
            if frame_index % 24 == 0:
                print(f"Matted {frame_index} frames", flush=True)
    finally:
        process.stdout.close()
        code = process.wait()
    if code or not records:
        raise ValueError(f"Video decode failed: exit={code}, frames={len(records)}")
    # One shared crop for every frame: no per-frame resize, centering or alignment.
    union = [min(r["alphaBoundsInRoi"][i] for r in records) if i < 2 else max(r["alphaBoundsInRoi"][i] for r in records) for i in range(4)]
    center_x, center_y = (union[0] + union[2]) / 2, (union[1] + union[3]) / 2
    crop_width = int(np.ceil((union[2] - union[0] + 48) / 16)) * 16
    crop_height = int(np.ceil((union[3] - union[1] + 48) / 16)) * 16
    left, top = round(center_x - crop_width / 2), round(center_y - crop_height / 2)
    crop = (left, top, left + crop_width, top + crop_height)
    final_directory = output / "frames"
    final_directory.mkdir(exist_ok=True)
    images, items = [], []
    sample_indexes = {0, round(2 * rate), round(4 * rate), round(6 * rate), round(8 * rate), len(records) - 1}
    for index in range(len(records)):
        with Image.open(frames_directory / f"{index:04d}.png") as roi_image:
            cropped = roi_image.crop(crop)
            cropped.save(final_directory / f"{index:04d}.png")
            images.append(cropped)
            if index in sample_indexes:
                items.append((f"{index / rate:.3f}s", roi_image.copy()))
    durations = [round((i + 1) * 1000 / rate) - round(i * 1000 / rate) for i in range(len(images))]
    if not args.skip_animation:
        images[0].save(output / "idle-transparent.webp", save_all=True, append_images=images[1:], duration=durations, loop=0, lossless=True, method=1, exact=True, allow_mixed=False)
    contact_sheet(items, output / "contact-sheet.png")
    report = {
        "source": str(args.source), "sourceSha256": hashlib.sha256(args.source.read_bytes()).hexdigest(),
        "frameCount": len(records), "frameRate": rate, "durationMs": sum(durations),
        "sourceCanvas": list(CANVAS), "processingRoi": list(ROI),
        "nativeSourceSize": info["size"],
        "canvasNormalization": "Uniform scale to fit with centered green padding; no aspect-ratio stretching.",
        "frameSize": [crop_width, crop_height], "cropInSourceCanvas": [left + ROI[0], top + ROI[1], left + ROI[0] + crop_width, top + ROI[1] + crop_height],
        "frameDurationsMs": durations,
        "method": "Border-connected green/near-black background; clean first-frame prior classifies enclosed dark hair gaps and supplies a conservative 12px-eroded opaque interior holdout; no transferred outer silhouette or colors; narrow edge color/alpha solved from nearest solid foreground and local background.",
        "geometry": "One constant crop; no frame-by-frame motion correction or redraw.",
        "opaqueRgbPreserved": all(r["opaqueRgbPreserved"] for r in records),
        "greenResiduePixels": sum(r["greenResiduePixels"] for r in records),
        "loopSeamRepaired": False, "runtimeAssetsReplaced": False,
        "frames": records,
    }
    (output / "validation.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({k: report[k] for k in ("frameCount", "frameSize", "durationMs", "opaqueRgbPreserved", "greenResiduePixels")}, ensure_ascii=False), flush=True)


if __name__ == "__main__":
    main()
