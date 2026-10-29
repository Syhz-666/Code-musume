"""Measure a transparent sequence's loop boundary without changing any artwork."""
import argparse
import json
from pathlib import Path
import numpy as np
from PIL import Image, ImageDraw


def visible_difference(left, right):
    a = np.asarray(left, dtype=np.float32) / 255
    b = np.asarray(right, dtype=np.float32) / 255
    support = np.maximum(a[:, :, 3], b[:, :, 3]) > 0.02
    premul_a = np.concatenate((a[:, :, :3] * a[:, :, 3:4], a[:, :, 3:4]), axis=2)
    premul_b = np.concatenate((b[:, :, :3] * b[:, :, 3:4], b[:, :, 3:4]), axis=2)
    return float(np.abs(premul_a - premul_b)[support].mean() * 255)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    paths = sorted((args.input / "frames").glob("[0-9][0-9][0-9][0-9].png"))
    assert len(paths) > 1
    # Match the installed idle builder's fixed transform, then its 144px display.
    # Never stretch a tall source crop to the rectangular atlas cell.
    with Image.open(paths[0]) as first:
        first = first.convert("RGBA")
    bounds = first.getchannel("A").point(lambda a: 255 if a >= 128 else 0).getbbox()
    scale = 184 / (bounds[3] - bounds[1])
    size = (round(first.width * scale), round(first.height * scale))
    scaled = first.resize(size, Image.Resampling.LANCZOS)
    solid = scaled.getchannel("A").point(lambda a: 255 if a >= 128 else 0).getbbox()
    feet = scaled.getchannel("A").crop((0, solid[3] - 6, size[0], solid[3])).point(lambda a: 255 if a >= 128 else 0).getbbox()
    offset = (round(96 - (feet[0] + feet[2]) / 2), 200 - solid[3])
    frames = []
    for path in paths:
        with Image.open(path) as image:
            cell = Image.new("RGBA", (192, 208))
            cell.alpha_composite(image.convert("RGBA").resize(size, Image.Resampling.LANCZOS), offset)
            frames.append(cell.resize((144, 156), Image.Resampling.LANCZOS))
    adjacent = [visible_difference(a, b) for a, b in zip(frames[:-1], frames[1:])]
    seam = visible_difference(frames[-1], frames[0])
    report = {
        "source": str(args.input), "frameCount": len(frames), "inspectionSize": [144, 156],
        "constantTransform": {"scale": scale, "size": size, "offset": offset, "cell": [192, 208]},
        "metric": "Mean absolute premultiplied RGBA difference on visible support, range 0..255",
        "adjacentMedian": float(np.median(adjacent)), "adjacentP95": float(np.percentile(adjacent, 95)),
        "adjacentMaximum": max(adjacent), "lastToFirst": seam,
        "firstLastPixelIdentical": frames[0].tobytes() == frames[-1].tobytes(),
        "seamRepairApplied": False, "runtimeAssetsChanged": False,
        "note": "Diagnostic only. Low pixel error alone does not certify continuous motion, correct pose, or smooth transitions between different states.",
    }
    args.output.mkdir(parents=True, exist_ok=True)
    (args.output / "seam-inspection.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    sheet = Image.new("RGB", (720, 380), "#eeeeee")
    draw = ImageDraw.Draw(sheet)
    for index, (label, frame) in enumerate((("last - 1", frames[-2]), ("last", frames[-1]), ("first", frames[0]), ("first + 1", frames[1]))):
        draw.text((index * 180 + 6, 8), label, fill="black")
        for row, color in enumerate(("white", "#252a36")):
            panel = Image.new("RGBA", (180, 172), color)
            panel.alpha_composite(frame, (18, 8))
            sheet.paste(panel.convert("RGB"), (index * 180, 30 + row * 174))
    sheet.save(args.output / "loop-boundary.png")
    print(json.dumps(report))


if __name__ == "__main__":
    main()
