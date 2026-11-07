"""Pack a one-shot completion nod into the existing built-in atlas pages."""

import argparse
import json
import math
import runpy
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
BUILTIN = ROOT / "apps/pet/assets/builtin"
helpers = runpy.run_path(str(Path(__file__).with_name("build-attention-atlas.py")))
CELL = helpers["CELL"]
COLUMNS = helpers["COLUMNS"]
BASE = "spritesheet.png"
THINKING = "thinking.png"
FPS = 24


def get_cell(page, slot):
    x, y = slot % COLUMNS * CELL[0], slot // COLUMNS * CELL[1]
    return page.crop((x, y, x + CELL[0], y + CELL[1]))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--range", type=int, nargs=3, required=True,
                        metavar=("START", "SMILE", "REST"))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    start, smile, rest = args.range
    if not (0 <= start <= smile - 3 and smile <= rest - 3):
        parser.error("Expected increasing frame numbers with room to blend to and from neutral")

    manifest = json.loads((BUILTIN / "musume.json").read_text(encoding="utf-8"))
    pages = {}
    for name in (BASE, THINKING):
        with Image.open(BUILTIN / name) as image:
            pages[name] = image.convert("RGBA")
    neutral = get_cell(pages[BASE], 0)
    source, transform = helpers["load_frames"](args.source, rest, width_reference=neutral)
    # Follow the original nod, held smile and recovery as one continuous motion.
    entry = source[start:smile + 1]
    returning = source[smile:rest + 1]
    # This gesture is head-only. Keep the original hands and stance.
    for frame in source[start:rest + 1]:
        helpers["lock_neutral_stance"](frame, neutral)
    helpers["start_at_neutral"](entry, neutral, 4)
    returning.reverse()
    helpers["start_at_neutral"](returning, neutral, 4)
    returning.reverse()
    assert entry[0].tobytes() == returning[-1].tobytes() == neutral.tobytes()
    assert entry[-1].tobytes() == returning[0].tobytes()

    # Keep static poses and other animations in their existing slots.
    placements = {"error": (BASE, 243, 1), "sleeping": (BASE, 244, 1),
                  "done-enter": (BASE, 245, len(entry)), "done": (THINKING, 210, len(returning))}
    for form in manifest["forms"]:
        for key, clip in form["clips"].items():
            page = clip.get("image", form["image"])
            slot = clip["row"] * COLUMNS + clip["column"]
            if key not in placements and any(
                page == name and slot < first + count and first < slot + clip["frames"]
                for name, first, count in placements.values()
            ):
                raise ValueError(f"Completion pack would overwrite {form['id']} / {key}")

    clips = manifest["forms"][0]["clips"]
    paste = helpers["helpers"]["paste"]
    static = {}
    for key in ("error", "sleeping"):
        clip = clips[key]
        assert clip["frames"] == 1 and clip.get("image", BASE) == BASE
        static[key] = [get_cell(pages[BASE], clip["row"] * COLUMNS + clip["column"])]
    for key, frames in {**static, "done-enter": entry, "done": returning}.items():
        name, slot, count = placements[key]
        height = math.ceil((slot + count) / COLUMNS) * CELL[1]
        if pages[name].height < height:
            expanded = Image.new("RGBA", (pages[name].width, height))
            expanded.paste(pages[name])
            pages[name] = expanded
        for index, frame in enumerate(frames):
            bounds = frame.getbbox()
            if not bounds or not (0 < bounds[0] < bounds[2] < CELL[0] and 0 < bounds[1] < bounds[3] < CELL[1]):
                raise ValueError(f"{key} frame {index} is empty or touches the cell edge")
            if not 199 <= helpers["solid"](frame)[3] <= 202:
                raise ValueError(f"{key} frame {index} drifts from the foot baseline")
            paste(pages[name], frame, slot + index)
        for form in manifest["forms"]:
            spec = {"row": slot // COLUMNS, "column": slot % COLUMNS, "frames": count,
                    "fps": FPS if count > 1 else 1, "looping": key in static and form["clips"][key]["looping"]}
            if name != form["image"]:
                spec["image"] = name
            form["clips"][key] = spec

    args.output.mkdir(parents=True, exist_ok=True)
    for name, page in pages.items():
        page.save(args.output / name, optimize=True)
    (args.output / "musume.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
                                           encoding="utf-8", newline="\n")
    helpers["write_preview"](args.output / "done-preview.mp4", neutral,
                             (("done-enter", entry, FPS, False), ("done", returning, FPS, False)))
    print(json.dumps({"source": str(args.source), "range": args.range, "transform": transform,
                      "placements": placements, "fps": FPS, "commonNeutralPixelIdentical": True}))


if __name__ == "__main__":
    main()
