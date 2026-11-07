"""Pack the waiting loop and optional one-shot reminder onto one atlas page."""

import argparse
import json
import math
import runpy
import subprocess
from pathlib import Path

from PIL import Image
from video_runtime import ffmpeg

ROOT = Path(__file__).resolve().parents[1]
BUILTIN = ROOT / "apps/pet/assets/builtin"
CELL = (192, 208)
COLUMNS = 16
ATLAS = "attention.png"
helpers = runpy.run_path(str(Path(__file__).with_name("build-thinking-atlas.py")))
mix, solid, close_loop = (helpers[name] for name in ("mix", "solid", "close_loop"))


def load_frames(source, last_frame, width_reference=None):
    report = json.loads((source / "validation.json").read_text(encoding="utf-8"))
    fps = report["frameRate"]
    if fps != 24 or last_frame >= report["frameCount"] or report["greenResiduePixels"] != 0:
        raise ValueError("Expected a complete, color-corrected 24 fps source take")

    with Image.open(source / "frames/0000.png") as image:
        first = image.convert("RGBA")
    bounds = solid(first)
    scale = 184 / (bounds[3] - bounds[1])
    size = (round(first.width * scale), round(first.height * scale))
    scaled = first.resize(size, Image.Resampling.LANCZOS)
    bounds = solid(scaled)
    feet = solid(scaled.crop((0, bounds[3] - 6, size[0], bounds[3])))
    offset = (round(96 - (feet[0] + feet[2]) / 2), 200 - bounds[3])
    width_scale = 1.0
    if width_reference is not None:
        size, offset, width_scale = helpers["calibrate_neutral_width"](
            first, scaled, offset[1], width_reference
        )

    frames = []
    for index in range(last_frame + 1):
        cell = Image.new("RGBA", CELL)
        with Image.open(source / "frames" / f"{index:04d}.png") as image:
            if image.size != first.size:
                raise ValueError(f"Frame {index} uses a different crop")
            cell.alpha_composite(image.convert("RGBA").resize(size, Image.Resampling.LANCZOS), offset)
        frames.append(cell)
    return frames, {"scale": scale, "size": size, "offset": offset, "widthScale": width_scale}


def start_at_neutral(entry, canonical, blend_frames):
    for index in range(blend_frames):
        t = index / (blend_frames - 1)
        entry[index] = mix(canonical, entry[index], t * t * (3 - 2 * t))


def resample(frames, count):
    result = []
    for index in range(count):
        position = (len(frames) - 1) * index / (count - 1)
        lower = math.floor(position)
        result.append(mix(frames[lower], frames[math.ceil(position)], position - lower))
    return result


def assemble_waiting(source, canonical):
    ready_frame, last_frame = 36, 120
    frames, transform = load_frames(source, last_frame)
    entry = frames[: ready_frame + 1]
    start_at_neutral(entry, canonical, 6)
    loop = frames[ready_frame: last_frame + 1]
    close_loop(loop, 12)
    # Retain the actual arm movement when lowering the hand.
    exit_frames = resample(entry[::-1], 24)

    assert entry[0].tobytes() == exit_frames[-1].tobytes() == canonical.tobytes()
    assert entry[-1].tobytes() == loop[0].tobytes() == loop[-1].tobytes() == exit_frames[0].tobytes()
    clips = (
        ("waiting-permission-enter", entry, 44, False),
        ("waiting-permission", loop, 24, True),
        ("waiting-permission-exit", exit_frames, 48, False),
    )
    return clips, transform


def assemble_notification(source, canonical, motion_range):
    start, raised, cut_from, cut_to, rest = motion_range
    if not (0 <= start <= raised - 2 and raised < cut_from < cut_to <= rest - 2):
        raise ValueError("Expected ordered reminder markers for the two-wave cut")

    frames, transform = load_frames(source, rest, width_reference=canonical)
    # Cut between matching outward poses. Blending these or adjacent moving frames ghosts the fingers.
    motion = frames[start:cut_from] + frames[cut_to:rest + 1]
    peak = raised - start
    # Keep every frame of the edited motion at the source's 24 fps cadence.
    entry = motion[:peak + 1]
    start_at_neutral(entry, canonical, 3)
    lowering = motion[peak:]
    lowering.reverse()
    start_at_neutral(lowering, canonical, 3)
    lowering.reverse()

    assert entry[0].tobytes() == lowering[-1].tobytes() == canonical.tobytes()
    assert entry[-1].tobytes() == lowering[0].tobytes()
    return (("notification-enter", entry, 24, False), ("notification", lowering, 24, False)), transform


def write_preview(path, canonical, clips):
    # Sample the exact atlas poses at 24 fps, with the same action on two backgrounds.
    rate = 24
    sequence = [canonical] * 12
    for _, frames, fps, _ in clips:
        for tick in range(max(1, round(len(frames) * rate / fps))):
            sequence.append(frames[min(len(frames) - 1, int(tick * fps / rate))])
    sequence.extend([canonical] * 12)
    encoder = subprocess.Popen([
        ffmpeg(), "-v", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgb24",
        "-s", "768x416", "-r", str(rate), "-i", "pipe:0", "-an", "-c:v", "libx264",
        "-crf", "18", "-pix_fmt", "yuv420p", "-movflags", "+faststart", str(path),
    ], stdin=subprocess.PIPE)
    with encoder:
        with encoder.stdin as stream:
            for frame in sequence:
                pose = frame.resize((384, 416), Image.Resampling.LANCZOS)
                canvas = Image.new("RGBA", (768, 416), "#252a36")
                canvas.paste("white", (0, 0, 384, 416))
                canvas.alpha_composite(pose, (0, 0))
                canvas.alpha_composite(pose, (384, 0))
                stream.write(canvas.convert("RGB").tobytes())
    if encoder.returncode:
        raise ValueError(f"Unable to encode {path.name}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--waiting-source", type=Path, required=True)
    parser.add_argument("--notification-source", type=Path)
    parser.add_argument("--notification-range", type=int, nargs=5,
                        metavar=("START", "RAISED", "CUT_FROM", "CUT_TO", "REST"))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--preview", action="store_true")
    parser.add_argument("--install", action="store_true")
    args = parser.parse_args()
    if bool(args.notification_source) != bool(args.notification_range):
        parser.error("--notification-source and --notification-range must be provided together")
    manifest = json.loads((BUILTIN / "musume.json").read_text(encoding="utf-8"))
    if not args.notification_source and any("notification-enter" in form["clips"] for form in manifest["forms"]):
        parser.error("Include the installed notification animation when rebuilding the attention page")
    with Image.open(BUILTIN / "spritesheet.png") as base:
        canonical = base.convert("RGBA").crop((0, 0, *CELL))

    clips, transform = assemble_waiting(args.waiting_source, canonical)
    notification = ()
    if args.notification_source:
        notification, notification_transform = assemble_notification(
            args.notification_source, canonical, args.notification_range
        )
        clips += notification
    frame_count = sum(len(frames) for _, frames, _, _ in clips)
    atlas = Image.new("RGBA", (CELL[0] * COLUMNS, CELL[1] * math.ceil(frame_count / COLUMNS)))
    segments = {}
    slot = 0
    for key, frames, fps, looping in clips:
        segments[key] = {"image": ATLAS, "row": slot // COLUMNS, "column": slot % COLUMNS,
                         "frames": len(frames), "fps": fps, "looping": looping}
        for frame in frames:
            bounds = frame.getbbox()
            if not bounds or not (0 < bounds[0] < bounds[2] < CELL[0] and 0 < bounds[1] < bounds[3] < CELL[1]):
                raise ValueError(f"Frame {slot} is empty or reaches the atlas cell edge")
            if not 199 <= solid(frame)[3] <= 202:
                raise ValueError(f"Frame {slot} moves off the shared foot baseline")
            helpers["paste"](atlas, frame, slot)
            slot += 1

    for form in manifest["forms"]:
        form["clips"].update(segments)
    pages = {form["image"] for form in manifest["forms"]}
    pages.update(clip["image"] for form in manifest["forms"] for clip in form["clips"].values() if "image" in clip)
    decoded_pixels = atlas.width * atlas.height
    for page in pages - {ATLAS}:
        with Image.open(BUILTIN / page) as image:
            decoded_pixels += image.width * image.height
    args.output.mkdir(parents=True, exist_ok=True)
    atlas.save(args.output / ATLAS, optimize=True)
    manifest_text = json.dumps(manifest, ensure_ascii=False, indent=2) + "\n"
    (args.output / "musume.json").write_text(manifest_text, encoding="utf-8", newline="\n")
    report = {"source": str(args.waiting_source), "frames": frame_count, "segments": segments,
              "constantTransform": transform,
              "totalDecodedPixels": decoded_pixels, "commonNeutralPixelIdentical": True,
              "heldBoundaryPixelIdentical": True}
    if notification:
        report["notification"] = {"source": str(args.notification_source), "range": args.notification_range,
                                  "constantTransform": notification_transform, "playsOnce": True}
    (args.output / "validation.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    if args.preview:
        write_preview(args.output / "waiting-preview.mp4", canonical, clips[:3])
        if notification:
            write_preview(args.output / "notification-preview.mp4", canonical, notification)
    if args.install:
        # Install the validated page before updating its manifest references.
        (BUILTIN / ATLAS).write_bytes((args.output / ATLAS).read_bytes())
        (BUILTIN / "musume.json").write_text(manifest_text, encoding="utf-8", newline="\n")
    print(json.dumps(report))


if __name__ == "__main__":
    main()
