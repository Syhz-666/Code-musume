"""Pack the error reaction, its held pose and the return to neutral."""

import argparse
import json
import runpy
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
BUILTIN = ROOT / "apps/pet/assets/builtin"
ATLAS = "error.png"
helpers = runpy.run_path(str(Path(__file__).with_name("build-attention-atlas.py")))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--range", type=int, nargs=2, required=True, metavar=("START", "HOLD"))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    start, hold = args.range
    if not 0 <= start <= hold - 4:
        parser.error("Expected at least five frames for the reaction")

    with Image.open(BUILTIN / "spritesheet.png") as page:
        neutral = page.convert("RGBA").crop((0, 0, *helpers["CELL"]))
    frames, transform = helpers["load_frames"](args.source, hold, width_reference=neutral)
    entry = frames[start:hold + 1]
    for frame in entry:
        # Leave the moving arm free while pinning the trousers and feet.
        helpers["lock_neutral_stance"](frame, neutral, top=156)
    helpers["start_at_neutral"](entry, neutral, 5)
    clips = (("error-enter", entry, 24, False), ("error", [entry[-1]], 1, False),
             ("error-exit", entry[::-1], 60, False))

    atlas, segments, count = helpers["pack_clips"](ATLAS, clips)
    manifest = json.loads((BUILTIN / "musume.json").read_text(encoding="utf-8"))
    for form in manifest["forms"]:
        form["clips"].update(segments)
    args.output.mkdir(parents=True, exist_ok=True)
    atlas.save(args.output / ATLAS, optimize=True)
    (args.output / "musume.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
                                           encoding="utf-8", newline="\n")
    helpers["write_preview"](args.output / "error-preview.mp4", neutral, clips)
    report = {"source": str(args.source), "range": args.range, "transform": transform,
              "segments": segments, "frames": count, "atlasSize": atlas.size}
    (args.output / "validation.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report))


if __name__ == "__main__":
    main()
