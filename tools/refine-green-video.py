"""Remove green spill from the approved pale-hair/pink-pajama video palette.

Keeps the existing RGBA crop and timing; no redrawing, warping or per-frame
alignment. Interior spill loses only its excess green; soft boundaries are
unmixed against green using the nearest reliable original foreground color.
"""
import argparse
import json
import subprocess
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw
from scipy import ndimage
from video_runtime import ffmpeg


def refine(image):
    original = np.asarray(image.convert("RGBA"))
    result = original.copy()
    rgb = original[:, :, :3].astype(np.float32)
    alpha = original[:, :, 3]
    excess = rgb[:, :, 1] - np.maximum(rgb[:, :, 0], rgb[:, :, 2])
    affected = (alpha > 0) & (excess > 16)
    clean = (alpha >= 250) & (excess <= 8)
    if not clean.any():
        raise ValueError("No reliable foreground for spill correction")
    depth = ndimage.distance_transform_edt(alpha > 32)
    interior = affected & (alpha >= 250) & (depth > 5)
    # The approved costume and hair have no green fills. Preserve opaque
    # coverage and R/B in the interior, instead of creating translucent pants.
    result[interior, 1] = np.maximum(original[interior, 0], original[interior, 2])
    edge = affected & ~interior
    nearest = ndimage.distance_transform_edt(~clean, return_distances=False, return_indices=True)
    foreground = rgb[nearest[0][edge], nearest[1][edge]]
    background = np.array([0, 255, 0], dtype=np.float32)
    direction = foreground - background
    coverage = np.sum((rgb[edge] - background) * direction, axis=1) / np.maximum(np.sum(direction * direction, axis=1), 1)
    result[edge, :3] = np.rint(foreground).astype(np.uint8)
    result[edge, 3] = np.minimum(alpha[edge], np.rint(np.clip(coverage, 0, 1) * 255).astype(np.uint8))
    # This approved costume has warm-white trousers and bare feet below 66%
    # of the shared crop, with no blue/green costume fills in that area. Chroma
    # subsampling also causes cyan spill where G only exceeds R, not B.
    y = np.indices(alpha.shape)[0]
    lower_spill = (alpha > 0) & (y >= alpha.shape[0] * .66) & (rgb[:, :, 1] > rgb[:, :, 0] + 4)
    result[lower_spill, 1] = np.minimum(result[lower_spill, 1], result[lower_spill, 0])
    result[lower_spill, 2] = np.minimum(result[lower_spill, 2], result[lower_spill, 0])
    affected |= lower_spill
    result[result[:, :, 3] <= 3] = 0
    assert np.array_equal(result[~affected], original[~affected])
    assert np.array_equal(result[interior, 3], original[interior, 3])
    assert np.all(result[:, :, 3] <= original[:, :, 3])
    return Image.fromarray(result), {
        "changedPixels": int(affected.sum()), "interiorSpillPixels": int(interior.sum()),
        "edgeSpillPixels": int(edge.sum()), "unaffectedRgbaPreserved": True,
        "lowerBodyPaletteSpillPixels": int(lower_spill.sum()),
        "interiorAlphaPreserved": True, "alphaExpanded": False,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--name", default="working")
    args = parser.parse_args()
    if args.source.resolve() == args.output.resolve():
        raise ValueError("Keep the original matte; output must be a new directory")
    previous = json.loads((args.source / "validation.json").read_text(encoding="utf-8"))
    paths = sorted((args.source / "frames").glob("[0-9][0-9][0-9][0-9].png"))
    assert len(paths) == previous["frameCount"]
    width, height = previous["frameSize"]
    fps = previous["frameRate"]
    args.output.mkdir(parents=True, exist_ok=True)
    (args.output / "frames").mkdir(exist_ok=True)
    previews, records = [], []
    face_minimum = 255
    encoder = subprocess.Popen([
        ffmpeg(), "-v", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgb24",
        "-s", f"{width}x{height}", "-r", str(fps), "-i", "pipe:0", "-an",
        "-c:v", "libx264", "-crf", "18", "-preset", "fast", "-pix_fmt", "yuv420p",
        "-movflags", "+faststart", str(args.output / f"{args.name}-white-preview.mp4"),
    ], stdin=subprocess.PIPE)
    samples = []
    try:
        for index, path in enumerate(paths):
            assert path.name == f"{index:04d}.png"
            with Image.open(path) as image:
                corrected, record = refine(image)
            corrected.save(args.output / "frames" / path.name)
            pixels = np.asarray(corrected)
            alpha = pixels[:, :, 3]
            assert not any((alpha[0].any(), alpha[-1].any(), alpha[:, 0].any(), alpha[:, -1].any()))
            color = pixels[:, :, :3].astype(np.int16)
            assert not ((alpha > 32) & (color[:, :, 1] - np.maximum(color[:, :, 0], color[:, :, 2]) > 16)).any()
            face = alpha[round(height * .24):round(height * .32), round(width * .42):round(width * .58)]
            face_minimum = min(face_minimum, int(face.min()))
            assert face.min() >= 250, f"Review a possible central-face hole in frame {index}"
            white = Image.new("RGBA", corrected.size, "white")
            white.alpha_composite(corrected)
            encoder.stdin.write(white.convert("RGB").tobytes())
            previews.append(corrected.resize((width // 2, height // 2), Image.Resampling.LANCZOS))
            records.append({"frame": index, **record})
            if index in {0, round(fps), round(1.5 * fps), round(2.5 * fps), round(4 * fps), len(paths) - 1}:
                samples.append((index, corrected.copy()))
            if index == round(2.5 * fps):
                white.convert("RGB").save(args.output / f"{args.name}-poster.png")
            if (index + 1) % 24 == 0:
                print(f"Refined {index + 1} frames", flush=True)
    finally:
        encoder.stdin.close()
        code = encoder.wait()
    assert code == 0
    durations = [round((i + 1) * 1000 / fps) - round(i * 1000 / fps) for i in range(len(paths))]
    previews[0].save(args.output / f"{args.name}-transparent-preview.webp", save_all=True, append_images=previews[1:], duration=durations, loop=0, lossless=True, method=1, exact=True, allow_mixed=False)
    sheet = Image.new("RGB", (720, 380 * len(samples)), "#eeeeee")
    draw = ImageDraw.Draw(sheet)
    for row, (index, frame) in enumerate(samples):
        for column, color in enumerate(("white", "#252a36", "checker")):
            panel = Image.new("RGBA", (240, 350), color if color != "checker" else "#eeeeee")
            if color == "checker":
                d = ImageDraw.Draw(panel)
                for y in range(0, 350, 16):
                    for x in range(0, 240, 16):
                        if (x // 16 + y // 16) % 2:
                            d.rectangle((x, y, x + 15, y + 15), fill="#cccccc")
            size = (round(frame.width * 336 / frame.height), 336)
            panel.alpha_composite(frame.resize(size, Image.Resampling.LANCZOS), ((240 - size[0]) // 2, 7))
            sheet.paste(panel.convert("RGB"), (column * 240, row * 380 + 26))
            draw.text((column * 240 + 6, row * 380 + 7), f"{index / fps:.2f}s / {color}", fill="black")
    sheet.save(args.output / "contact-sheet.png")
    report = {**previous, "refinedFrom": str(args.source), "frames": records, "greenResiduePixels": 0,
              "refinement": "Correct green excess >16 and cyan spill only in the approved warm-white-trouser/bare-foot region below 66% of the crop. Interior coverage preserved; soft edges unmixed against green; all unaffected RGBA exact. No geometry change.",
              "opaqueRgbPreserved": False, "faceInspectionRegionAlphaMinimum": face_minimum,
              "transparentBordersEveryFrame": True, "previewDurationMs": sum(durations),
              "runtimeAssetsReplaced": False, "seamRepairApplied": False}
    (args.output / "validation.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    (args.output / "qa.json").write_text(json.dumps({"frames": len(paths), "greenResiduePixels": 0, "faceAlphaMinimum": face_minimum, "frameSize": [width, height], "fps": fps, "status": "matting-checks-only"}, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"frames": len(paths), "faceAlphaMinimum": face_minimum, "output": str(args.output)}), flush=True)


if __name__ == "__main__":
    main()
