"""Pack the entry, closed-eye loop and wake gesture from one sleeping take."""

import argparse
import json
import math
import runpy
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
BUILTIN = ROOT / "apps/pet/assets/builtin"
ATLAS = "sleeping.png"
helpers = runpy.run_path(str(Path(__file__).with_name("build-attention-atlas.py")))
CELL = helpers["CELL"]
COLUMNS = helpers["COLUMNS"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--range", type=int, nargs=3, required=True,
                        metavar=("START", "CLOSED", "LOOP_END"))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    start, closed, loop_end = args.range
    if not (0 <= start <= closed - 4 and loop_end >= closed + 24):
        parser.error("Expected an entry and at least one second of closed-eye footage")

    with Image.open(BUILTIN / "spritesheet.png") as page:
        neutral = page.convert("RGBA").crop((0, 0, *CELL))
    frames, transform = helpers["load_frames"](args.source, loop_end, width_reference=neutral)
    for frame in frames[start:loop_end + 1]:
        helpers["lock_neutral_stance"](frame, neutral)
    entry = frames[start:closed + 1]
    loop = frames[closed:loop_end + 1]
    helpers["start_at_neutral"](entry, neutral, 5)
    helpers["close_loop"](loop, 12)
    wake = entry[::-1]
    assert entry[0].tobytes() == wake[-1].tobytes() == neutral.tobytes()
    assert entry[-1].tobytes() == loop[0].tobytes() == loop[-1].tobytes() == wake[0].tobytes()
    clips = (("sleeping-enter", entry, 24, False), ("sleeping", loop, 24, True),
             ("sleeping-exit", wake, 48, False))

    count = sum(len(clip[1]) for clip in clips)
    atlas = Image.new("RGBA", (CELL[0] * COLUMNS, CELL[1] * math.ceil(count / COLUMNS)))
    segments = {}
    slot = 0
    for key, sequence, fps, looping in clips:
        segments[key] = {"image": ATLAS, "row": slot // COLUMNS, "column": slot % COLUMNS,
                         "frames": len(sequence), "fps": fps, "looping": looping}
        for frame in sequence:
            bounds = frame.getbbox()
            if not bounds or not (0 < bounds[0] < bounds[2] < CELL[0] and 0 < bounds[1] < bounds[3] < CELL[1]):
                raise ValueError(f"Frame {slot} is empty or touches the cell edge")
            if not 199 <= helpers["solid"](frame)[3] <= 202:
                raise ValueError(f"Frame {slot} drifts from the foot baseline")
            helpers["helpers"]["paste"](atlas, frame, slot)
            slot += 1

    manifest = json.loads((BUILTIN / "musume.json").read_text(encoding="utf-8"))
    for form in manifest["forms"]:
        form["clips"].update(segments)
    args.output.mkdir(parents=True, exist_ok=True)
    atlas.save(args.output / ATLAS, optimize=True)
    (args.output / "musume.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
                                           encoding="utf-8", newline="\n")
    helpers["write_preview"](args.output / "sleeping-preview.mp4", neutral,
                             (clips[0], clips[1], clips[1], clips[2]))
    report = {"source": str(args.source), "range": args.range, "transform": transform,
              "segments": segments, "frames": count, "atlasSize": atlas.size,
              "commonNeutralPixelIdentical": True, "loopEndpointsPixelIdentical": True}
    (args.output / "validation.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report))


if __name__ == "__main__":
    main()
