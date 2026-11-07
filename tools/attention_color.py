"""Recover color from homewear footage after the green matte."""

import argparse
import json
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw
from scipy import ndimage

from working_color import recover_green

# Absolute coordinates on the normalized 1920x1080 source, shared by both takes.
MATERIALS = ((692, 868, 697, 858), (868, 897, 874, 890), (897, 926, 898, 914))


def reference_palettes(path):
    with Image.open(path) as image:
        if image.size != (1920, 1080):
            raise ValueError("Expected the normalized reference used by both videos")
        rgb = np.asarray(image.convert("RGB"))
    palettes = []
    for _, _, top, bottom in MATERIALS:
        samples = rgb[top:bottom, 867:1061]
        red, green, blue = np.moveaxis(samples, -1, 0)
        clean = (red > 180) & (red >= green) & (red >= blue) & (green > 150) & (blue > 140)
        if not clean.any():
            raise ValueError("Reference lacks a reliable lower-body palette")
        palettes.append(np.median(samples[clean], axis=0))
    return palettes


def correct_frame(image, crop, palettes, raised_hand=False):
    original = np.asarray(image.convert("RGBA"))
    rgb = original[:, :, :3].astype(np.float32)
    visible = original[:, :, 3] > 0
    corrected = rgb.copy()
    y, x = np.ogrid[crop[1]:crop[3], crop[0]:crop[2]]
    changed_lower = np.zeros(visible.shape, dtype=bool)
    for (top, bottom, _, _), palette in zip(MATERIALS, palettes, strict=True):
        region = visible & (y >= top) & (y < bottom) & (x >= 851) & (x < 1070)
        recovered, weight = recover_green(rgb[region], palette)
        corrected[region] = rgb[region] + weight[:, None] * (recovered - rgb[region])
        changed_lower[region] = weight > 0

    if raised_hand:
        red, green, blue = np.moveaxis(rgb, -1, 0)
        hand = visible & (x >= 1100) & (y >= 380) & (y < 700) & (red > 120) & (green > blue + 14)
        recovered, weight = recover_green(rgb[hand], palettes[2])
        corrected[hand] = rgb[hand] + weight[:, None] * (recovered - rgb[hand])

    red, green, blue = np.moveaxis(corrected, -1, 0)
    excess = green - np.maximum(red, blue)
    depth = ndimage.distance_transform_edt(original[:, :, 3] > 32)
    # Pale-blue hair usually has B well above G. Select teal only on its rim.
    cyan_rim = (depth <= 6) & (green > 160) & (green > red + 24) & (green >= blue - 8)
    contaminated = visible & ((excess > 12) | cyan_rim)
    if contaminated.any():
        clean = (original[:, :, 3] >= 250) & (excess <= 5) & ~cyan_rim
        if not clean.any():
            raise ValueError("No reliable foreground colors")
        nearest = ndimage.distance_transform_edt(~clean, return_distances=False, return_indices=True)
        source = corrected[nearest[0][contaminated], nearest[1][contaminated]]
        recovered, _ = recover_green(corrected[contaminated], source)
        is_rim = depth[contaminated] <= 6
        still_green = recovered[:, 1] - np.maximum(recovered[:, 0], recovered[:, 2]) > 12
        corrected[contaminated] = np.where((is_rim | still_green)[:, None], source, recovered)

    result = original.copy()
    result[:, :, :3] = np.rint(corrected).astype(np.uint8)
    red, green, blue = np.moveaxis(result[:, :, :3].astype(np.int16), -1, 0)
    residue = visible & (green - np.maximum(red, blue) > 16)
    if residue.any():
        raise ValueError(f"{residue.sum()} green pixels remain after spill recovery")
    assert np.array_equal(original[:, :, 3], result[:, :, 3])
    return Image.fromarray(result), {
        "lowerBodyCorrectedPixels": int(changed_lower.sum()),
        "additionalSpillCorrectedPixels": int(contaminated.sum()),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--reference", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--raised-hand", action="store_true", help="Recover skin color on the screen-right wave")
    args = parser.parse_args()
    if args.source.resolve() == args.output.resolve():
        raise ValueError("Preserve the original matte in a separate directory")
    report = json.loads((args.source / "validation.json").read_text(encoding="utf-8"))
    crop = report["cropInSourceCanvas"]
    palettes = reference_palettes(args.reference)
    (args.output / "frames").mkdir(parents=True, exist_ok=True)
    samples = (0, 20, 30, 36, 60, 72, 120, 157)
    sheet = Image.new("RGB", (832, 265 * (len(samples) // 2)), "#ececec")
    draw = ImageDraw.Draw(sheet)
    records = []
    for index in range(report["frameCount"]):
        name = f"{index:04d}.png"
        with Image.open(args.source / "frames" / name) as before:
            after, counts = correct_frame(before, crop, palettes, args.raised_hand)
            after.save(args.output / "frames" / name)
            records.append({"frame": index, **counts})
            if index in samples:
                panel = samples.index(index)
                for column, (pose, label) in enumerate(((before, "initial matte"), (after, "color restored"))):
                    thumb = pose.convert("RGBA")
                    thumb.thumbnail((196, 238), Image.Resampling.LANCZOS)
                    x, y = (panel % 2) * 416 + column * 208, (panel // 2) * 265
                    sheet.paste("#252a36", (x, y + 22, x + 208, y + 260))
                    sheet.paste(thumb.convert("RGB"), (x + (208 - thumb.width) // 2, y + 22), thumb.getchannel("A"))
                    draw.text((x + 6, y + 5), f"{index / report['frameRate']:.2f}s / {label}", fill="black")
    sheet.save(args.output / "color-comparison.png")
    report.update({
        "refinedFrom": str(args.source), "colorReference": str(args.reference),
        "referencePalettes": [p.tolist() for p in palettes], "greenResiduePixels": 0,
        "alphaByteIdenticalToInitialMatte": True, "opaqueRgbPreserved": False,
        "colorCorrectionFrames": records, "raisedHandCorrection": args.raised_hand,
    })
    (args.output / "validation.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"frames": len(records), "output": str(args.output), "palettes": report["referencePalettes"]}))


if __name__ == "__main__":
    main()
