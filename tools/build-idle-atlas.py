"""Pack the approved idle video and existing seven stills into the builtin atlas.

Run from the project root: python tools/build-idle-atlas.py --install
Ordinary app builds only use the installed PNG and manifest, not these sources.
"""

import argparse
import hashlib
import json
import shutil
from pathlib import Path

from PIL import Image, ImageDraw


ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "artifacts/design/preprocessed/frames"
SOURCE = ART / "workbuddy-idle-cutout-v2"
STILLS = ART / "state-cutouts-v2/spritesheet.png"
OUTPUT = ART / "runtime-idle-v1"
BUILTIN = ROOT / "apps/pet/assets/builtin"
STATES = ("thinking", "working", "waiting-permission", "notification", "done", "error", "sleeping")
CELL = (192, 208)
COLUMNS = 16
BASELINE = 200


def bounds(image):
    return image.getchannel("A").point(lambda a: 255 if a >= 128 else 0).getbbox()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--install", action="store_true")
    args = parser.parse_args()
    if args.install:
        installed = json.loads((BUILTIN / "musume.json").read_text(encoding="utf-8"))
        if any(clip.get("image") for form in installed["forms"] for clip in form["clips"].values()):
            raise SystemExit("A multipage animation pack is installed; use tools/build-thinking-atlas.py --install instead.")
    OUTPUT.mkdir(parents=True, exist_ok=True)
    validation = json.loads((SOURCE / "validation.json").read_text(encoding="utf-8"))
    qa = json.loads((SOURCE / "qa.json").read_text(encoding="utf-8"))
    paths = sorted((SOURCE / "frames").glob("[0-9][0-9][0-9][0-9].png"))
    assert not validation["refinementSampleOnly"] and qa["status"] == "passed"
    assert len(paths) == validation["frameCount"] == 243
    assert validation["frameRate"] == 24
    with Image.open(paths[0]) as first:
        first = first.convert("RGBA")
    first_bounds = bounds(first)
    scale = 184 / (first_bounds[3] - first_bounds[1])
    resized_size = (round(first.width * scale), round(first.height * scale))
    resized_first = first.resize(resized_size, Image.Resampling.LANCZOS)
    first_solid = bounds(resized_first)
    feet = resized_first.getchannel("A").crop((0, first_solid[3] - 6, resized_size[0], first_solid[3])).point(lambda a: 255 if a >= 128 else 0).getbbox()
    offset = (round(CELL[0] / 2 - (feet[0] + feet[2]) / 2), BASELINE - first_solid[3])
    rows = (len(paths) + len(STATES) + COLUMNS - 1) // COLUMNS
    atlas = Image.new("RGBA", (CELL[0] * COLUMNS, CELL[1] * rows))
    assert max(atlas.size) <= 4096
    records = []
    samples = []
    for index, path in enumerate(paths):
        with Image.open(path) as original:
            frame = Image.new("RGBA", CELL)
            frame.alpha_composite(original.convert("RGBA").resize(resized_size, Image.Resampling.LANCZOS), offset)
        full = frame.getchannel("A").getbbox()
        solid = bounds(frame)
        assert full and full[0] > 0 and full[1] > 0 and full[2] < CELL[0] and full[3] < CELL[1], index
        assert abs(solid[3] - BASELINE) <= 1, (index, solid)
        atlas.paste(frame, (index % COLUMNS * CELL[0], index // COLUMNS * CELL[1]))
        records.append({"frame": index, "solidBounds": solid, "alphaBounds": full})
        if index in (0, 48, 87, 120, 181, 242):
            samples.append((f"idle {index / 24:.3f}s", frame.copy()))
    with Image.open(STILLS) as stills:
        assert stills.size == (384, 3328)
        for row, state in enumerate(STATES, start=1):
            slot = len(paths) + row - 1
            frame = stills.crop((0, row * 416, 384, (row + 1) * 416)).convert("RGBA").resize(CELL, Image.Resampling.LANCZOS)
            atlas.paste(frame, (slot % COLUMNS * CELL[0], slot // COLUMNS * CELL[1]))
            # Retain the same still artwork; only its atlas sampling resolution changes.
            packed = atlas.crop((slot % COLUMNS * CELL[0], slot // COLUMNS * CELL[1], (slot % COLUMNS + 1) * CELL[0], (slot // COLUMNS + 1) * CELL[1]))
            assert packed.tobytes() == frame.tobytes()
            samples.append((state, frame))
    manifest = json.loads((BUILTIN / "musume.json").read_text(encoding="utf-8"))
    for form in manifest["forms"]:
        form.update(cellWidth=CELL[0], cellHeight=CELL[1], columns=COLUMNS)
        form["clips"]["idle"] = {"row": 0, "column": 0, "frames": len(paths), "fps": 24, "looping": True}
        for index, state in enumerate(STATES, start=len(paths)):
            form["clips"][state].update(row=index // COLUMNS, column=index % COLUMNS, frames=1)
    atlas.save(OUTPUT / "spritesheet.png", optimize=True)
    (OUTPUT / "musume.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    contact = Image.new("RGB", (320 * 4, 216 * ((len(samples) + 3) // 4)), "#ecebf2")
    draw = ImageDraw.Draw(contact)
    for index, (name, frame) in enumerate(samples):
        x, y = index % 4 * 320, index // 4 * 216
        draw.text((x + 8, y + 8), name, fill="#333344")
        for column, color in enumerate(("#fafafa", "#252a36")):
            panel = Image.new("RGBA", (152, 168), color)
            panel.alpha_composite(frame.resize((144, 156), Image.Resampling.LANCZOS), (4, 6))
            contact.paste(panel.convert("RGB"), (x + 6 + column * 156, y + 30))
    contact.save(OUTPUT / "contact-sheet.png")
    report = {
        "source": str(SOURCE), "sourceValidationSha256": hashlib.sha256((SOURCE / "validation.json").read_bytes()).hexdigest(),
        "idleFrames": len(paths), "idleFps": 24, "idleDurationSeconds": len(paths) / 24,
        "atlasSize": list(atlas.size), "cellSize": list(CELL), "columns": COLUMNS,
        "staticSourceSha256": hashlib.sha256(STILLS.read_bytes()).hexdigest(), "otherStates": list(STATES),
        "idleTransform": {"scale": scale, "resizedSize": resized_size, "offset": offset, "constantAcrossFrames": True},
        "loopSeamRepaired": False, "frames": records,
        "atlasSha256": hashlib.sha256((OUTPUT / "spritesheet.png").read_bytes()).hexdigest(),
    }
    (OUTPUT / "validation.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    if args.install:
        for name in ("spritesheet.png", "musume.json"):
            shutil.copyfile(OUTPUT / name, BUILTIN / name)
    print(json.dumps({key: report[key] for key in ("idleFrames", "idleFps", "atlasSize", "cellSize", "atlasSha256")}, ensure_ascii=False))


if __name__ == "__main__":
    main()
