"""Pack the generated working gesture, its held loop and a reversed-motion exit.

python tools/build-working-atlas.py --install
Preserves the installed idle/thinking pages and all existing manifest metadata.
"""
import argparse
import importlib.util
import json
import shutil
import subprocess
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw
from video_runtime import ffmpeg

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "artifacts/design/preprocessed/frames/workbuddy-working-cutout-v4"
OUTPUT = ROOT / "artifacts/design/preprocessed/frames/runtime-working-v2"
BUILTIN = ROOT / "apps/pet/assets/builtin"
CELL = (192, 208)
COLUMNS = 16
spec = importlib.util.spec_from_file_location("thinking_atlas_helpers", ROOT / "tools/build-thinking-atlas.py")
helpers = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helpers)
mix, solid, close_loop = helpers.mix, helpers.solid, helpers.close_loop


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--install", action="store_true")
    args = parser.parse_args()
    qa = json.loads((SOURCE / "qa.json").read_text(encoding="utf-8"))
    assert qa["frames"] == 124 and qa["greenResiduePixels"] == 0
    assert qa["lowerBodyColorsCorrected"] and qa["alphaByteIdenticalToV3"]
    manifest = json.loads((BUILTIN / "musume.json").read_text(encoding="utf-8"))
    base = Image.open(BUILTIN / "spritesheet.png").convert("RGBA")
    canonical = base.crop((0, 0, *CELL))
    paths = sorted((SOURCE / "frames").glob("[0-9][0-9][0-9][0-9].png"))
    assert len(paths) == 124
    first = Image.open(paths[0]).convert("RGBA")
    bounds = solid(first)
    scale = 184 / (bounds[3] - bounds[1])
    size = (round(first.width * scale), round(first.height * scale))
    scaled = first.resize(size, Image.Resampling.LANCZOS)
    bounds = solid(scaled)
    feet = scaled.getchannel("A").crop((0, bounds[3] - 6, size[0], bounds[3])).point(lambda a: 255 if a >= 128 else 0).getbbox()
    offset = (round(96 - (feet[0] + feet[2]) / 2), 200 - bounds[3])
    frames = []
    for path in paths:
        cell = Image.new("RGBA", CELL)
        cell.alpha_composite(Image.open(path).convert("RGBA").resize(size, Image.Resampling.LANCZOS), offset)
        frames.append(cell)
    entry = [frame.copy() for frame in frames[:36]]
    for i in range(8):
        t = i / 7
        entry[i] = mix(canonical, entry[i], t * t * (3 - 2 * t))
    loop = [frame.copy() for frame in frames[36:]]
    close_loop(loop, 16)
    # A real lowering motion from the same raising gesture, not a dissolved pose.
    exit_frames = []
    for value in np.linspace(35, 0, 30):
        low, high = int(np.floor(value)), int(np.ceil(value))
        exit_frames.append(mix(entry[low], entry[high], value - low))
    packed = entry + loop + exit_frames
    page = Image.new("RGBA", (CELL[0] * COLUMNS, CELL[1] * ((len(packed) + 15) // 16)))
    for i, frame in enumerate(packed):
        b = frame.getchannel("A").getbbox()
        assert b and b[0] > 0 and b[1] > 0 and b[2] < CELL[0] and b[3] < CELL[1]
        assert 198 <= solid(frame)[3] <= 201, (i, solid(frame))
        helpers.paste(page, frame, i)
    segments = {"working-enter": (0, 36, 48, False), "working": (36, 88, 24, True), "working-exit": (124, 30, 60, False)}
    for form in manifest["forms"]:
        for key, (start, count, fps, looping) in segments.items():
            form["clips"][key] = {"image": "working.png", "row": start // 16, "column": start % 16, "frames": count, "fps": fps, "looping": looping}
    assert entry[0].tobytes() == exit_frames[-1].tobytes() == canonical.tobytes()
    assert loop[0].tobytes() == loop[-1].tobytes()
    OUTPUT.mkdir(parents=True, exist_ok=True)
    page.save(OUTPUT / "working.png", optimize=True)
    (OUTPUT / "musume.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    pages = {"working.png": page}
    for name in {form["image"] for form in manifest["forms"]} | {clip["image"] for form in manifest["forms"] for clip in form["clips"].values() if "image" in clip}:
        if name != "working.png":
            shutil.copyfile(BUILTIN / name, OUTPUT / name)
            pages[name] = Image.open(BUILTIN / name).convert("RGBA")
    total = sum(image.width * image.height for image in pages.values())
    assert total <= 32 * 1024 * 1024
    report = {"source": str(SOURCE), "frames": len(packed), "segments": segments,
              "constantTransform": {"scale": scale, "size": size, "offset": offset},
              "commonNeutralPixelIdentical": True, "loopEndpointsPixelIdentical": True,
              "exitMethod": "Reverse and resample the actual generated raising gesture",
              "totalDecodedPixels": total, "pageSizes": {name: list(image.size) for name, image in pages.items()},
              "existingPagesPreserved": True, "sourceAspectRatioPreserved": True}
    (OUTPUT / "validation.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    samples = [("idle / common neutral", canonical), ("entry middle", entry[18]), ("working loop", loop[0]), ("working mid-loop", loop[44]), ("reverse exit", exit_frames[15]), ("idle / exit end", exit_frames[-1])]
    sheet = Image.new("RGB", (960, 432), "#eeeeee")
    draw = ImageDraw.Draw(sheet)
    for i, (label, frame) in enumerate(samples):
        x, y = i % 3 * 320, i // 3 * 216
        draw.text((x + 4, y + 6), label, fill="black")
        for j, color in enumerate(("white", "#252a36")):
            panel = Image.new("RGBA", (156, 174), color)
            panel.alpha_composite(frame.resize((144, 156), Image.Resampling.LANCZOS), (6, 6))
            sheet.paste(panel.convert("RGB"), (x + j * 160, y + 27))
    sheet.save(OUTPUT / "contact-sheet.png")
    # Viewing copy of the exact production cells, including both state changes.
    encoder = subprocess.Popen([ffmpeg(), "-v", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", "384x416", "-r", "24", "-i", "pipe:0", "-an", "-c:v", "libx264", "-crf", "18", "-pix_fmt", "yuv420p", "-movflags", "+faststart", str(OUTPUT / "working-transitions-preview.mp4")], stdin=subprocess.PIPE)
    try:
        for key, duration in (("idle", 1), ("working-enter", .75), ("working", 3.667), ("working-exit", .5), ("thinking-enter", 1.25), ("thinking", 2), ("thinking-exit", .5), ("working-enter", .75), ("working", 3.667), ("working-exit", .5), ("idle", 1)):
            form = manifest["forms"][0]
            clip = form["clips"][key]
            image = pages[clip.get("image", form["image"])]
            for tick in range(round(duration * 24)):
                index = min(clip["frames"] - 1, int(tick * clip["fps"] / 24))
                slot = clip["row"] * 16 + clip.get("column", 0) + index
                frame = image.crop((slot % 16 * 192, slot // 16 * 208, (slot % 16 + 1) * 192, (slot // 16 + 1) * 208))
                white = Image.new("RGBA", (384, 416), "white")
                white.alpha_composite(frame.resize((384, 416), Image.Resampling.LANCZOS))
                encoder.stdin.write(white.convert("RGB").tobytes())
    finally:
        encoder.stdin.close()
        assert encoder.wait() == 0
    if args.install:
        for name in ("working.png", "musume.json"):
            shutil.copyfile(OUTPUT / name, BUILTIN / name)
    print(json.dumps(report), flush=True)


if __name__ == "__main__":
    main()
