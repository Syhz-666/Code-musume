"""Install approved thinking entry/loop/exit artwork and repair idle's loop boundary.

python tools/build-thinking-atlas.py --install
Sources and QA stay in artifacts/design; the app embeds only the installed pages.
"""
import argparse
import hashlib
import json
import shutil
from pathlib import Path
import numpy as np
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "artifacts/design/preprocessed/frames"
SOURCE = ART / "workbuddy-thinking-cutout-v3"
BASE = ART / "runtime-idle-v1"
OUTPUT = ART / "runtime-thinking-v1"
BUILTIN = ROOT / "apps/pet/assets/builtin"
CELL = (192, 208)
COLUMNS = 16


def solid(image):
    return image.getchannel("A").point(lambda value: 255 if value >= 128 else 0).getbbox()


def mix(left, right, weight):
    """Interpolate coverage and premultiplied color, avoiding dark alpha fringes."""
    if weight <= 0:
        return left.copy()
    if weight >= 1:
        return right.copy()
    a, b = np.asarray(left, dtype=np.float32), np.asarray(right, dtype=np.float32)
    alpha = a[:, :, 3:4] * (1 - weight) + b[:, :, 3:4] * weight
    color = a[:, :, :3] * a[:, :, 3:4] * (1 - weight) + b[:, :, :3] * b[:, :, 3:4] * weight
    result = np.concatenate((color / np.maximum(alpha, 1e-6), alpha), axis=2)
    result = np.rint(np.clip(result, 0, 255)).astype(np.uint8)
    result[result[:, :, 3] == 0] = 0
    return Image.fromarray(result)


def close_loop(frames, count):
    for index in range(len(frames) - count, len(frames)):
        t = (index - (len(frames) - count)) / (count - 1)
        frames[index] = mix(frames[index], frames[0], t * t * (3 - 2 * t))


def paste(atlas, frame, slot):
    atlas.paste(frame, (slot % COLUMNS * CELL[0], slot // COLUMNS * CELL[1]))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--install", action="store_true")
    args = parser.parse_args()
    if args.install and any(
        clip.get("image") == "working.png"
        for form in json.loads((BUILTIN / "musume.json").read_text(encoding="utf-8"))["forms"]
        for clip in form["clips"].values()
    ):
        raise SystemExit("A working animation pack is installed; use tools/build-working-atlas.py --install to preserve it.")
    OUTPUT.mkdir(parents=True, exist_ok=True)
    qa = json.loads((SOURCE / "qa.json").read_text(encoding="utf-8"))
    assert qa["frames"] == 243 and qa["greenResiduePixels"] == 0
    base = Image.open(BASE / "spritesheet.png").convert("RGBA")
    manifest = json.loads((BASE / "musume.json").read_text(encoding="utf-8"))
    idle = [base.crop((index % 16 * 192, index // 16 * 208, (index % 16 + 1) * 192, (index // 16 + 1) * 208)) for index in range(243)]
    canonical = idle[0].copy()
    close_loop(idle, 24)
    for index, frame in enumerate(idle):
        paste(base, frame, index)
    paths = sorted((SOURCE / "frames").glob("[0-9][0-9][0-9][0-9].png"))
    assert len(paths) == 243
    first = Image.open(paths[0]).convert("RGBA")
    bounds = solid(first)
    scale = 184 / (bounds[3] - bounds[1])
    size = (round(first.width * scale), round(first.height * scale))
    scaled = first.resize(size, Image.Resampling.LANCZOS)
    bounds = solid(scaled)
    feet = scaled.getchannel("A").crop((0, bounds[3] - 6, size[0], bounds[3])).point(lambda value: 255 if value >= 128 else 0).getbbox()
    offset = (round(96 - (feet[0] + feet[2]) / 2), 200 - bounds[3])
    source_frames = []
    for path in paths:
        frame = Image.new("RGBA", CELL)
        frame.alpha_composite(Image.open(path).convert("RGBA").resize(size, Image.Resampling.LANCZOS), offset)
        source_frames.append(frame)
    entry = [frame.copy() for frame in source_frames[:60]]
    for index in range(8):
        t = index / 7
        entry[index] = mix(canonical, entry[index], t * t * (3 - 2 * t))
    loop = [frame.copy() for frame in source_frames[60:180]]
    close_loop(loop, 16)
    # Resample the generated lowering gesture to a responsive half-second exit.
    exit_frames = []
    for value in np.linspace(180, 216, 30):
        low, high = int(np.floor(value)), int(np.ceil(value))
        exit_frames.append(mix(source_frames[low], source_frames[high], value - low))
    for index in range(22, 30):
        t = (index - 22) / 7
        exit_frames[index] = mix(exit_frames[index], canonical, t * t * (3 - 2 * t))
    packed = entry + loop + exit_frames
    page = Image.new("RGBA", (CELL[0] * COLUMNS, CELL[1] * ((len(packed) + 15) // 16)))
    for index, frame in enumerate(packed):
        bounds = frame.getchannel("A").getbbox()
        assert bounds and bounds[0] > 0 and bounds[1] > 0 and bounds[2] < CELL[0] and bounds[3] < CELL[1]
        assert 198 <= solid(frame)[3] <= 201, (index, solid(frame))
        paste(page, frame, index)
    specs = {"thinking-enter": (0, 60, 48, False), "thinking": (60, 120, 24, True), "thinking-exit": (180, 30, 60, False)}
    for form in manifest["forms"]:
        for key, (start, count, fps, looping) in specs.items():
            form["clips"][key] = {"image": "thinking.png", "row": start // 16, "column": start % 16, "frames": count, "fps": fps, "looping": looping}
    base.save(OUTPUT / "spritesheet.png", optimize=True)
    page.save(OUTPUT / "thinking.png", optimize=True)
    (OUTPUT / "musume.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    assert idle[0].tobytes() == idle[-1].tobytes() == entry[0].tobytes() == exit_frames[-1].tobytes()
    assert loop[0].tobytes() == loop[-1].tobytes()
    samples = [("idle neutral", canonical), ("enter last", entry[-1]), ("loop first / last", loop[0]), ("thinking blink", packed[144]), ("exit middle", exit_frames[15]), ("exit neutral", exit_frames[-1])]
    contact = Image.new("RGB", (320 * 3, 216 * 2), "#eeeeee")
    draw = ImageDraw.Draw(contact)
    for index, (label, frame) in enumerate(samples):
        x, y = index % 3 * 320, index // 3 * 216
        draw.text((x + 5, y + 6), label, fill="black")
        for column, color in enumerate(("white", "#252a36")):
            panel = Image.new("RGBA", (156, 174), color)
            panel.alpha_composite(frame.resize((144, 156), Image.Resampling.LANCZOS), (6, 6))
            contact.paste(panel.convert("RGB"), (x + column * 160, y + 27))
    contact.save(OUTPUT / "contact-sheet.png")
    report = {"source": str(SOURCE), "constantTransform": {"scale": scale, "size": size, "offset": offset}, "thinkingFrames": len(packed), "segments": specs, "idleSeamFrames": 24, "thinkingLoopSeamFrames": 16, "commonNeutralPixelIdentical": True, "thinkingLoopEndpointsPixelIdentical": True, "pages": {name: {"sha256": hashlib.sha256((OUTPUT / name).read_bytes()).hexdigest(), "bytes": (OUTPUT / name).stat().st_size} for name in ("spritesheet.png", "thinking.png")}}
    (OUTPUT / "validation.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    if args.install:
        for name in ("spritesheet.png", "thinking.png", "musume.json"):
            shutil.copyfile(OUTPUT / name, BUILTIN / name)
    print(json.dumps(report))


if __name__ == "__main__":
    main()
