"""Recover lower-body green spill from the original matte, not clipped v3 RGB.

python tools/working_color.py --samples-only
python tools/working_color.py

Deterministic video color correction: geometry, alpha, upper body and timing stay
unchanged. The approved neutral reference supplies separate garment/skin colors.
"""
import argparse
import json
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "artifacts/design/preprocessed/frames"
RAW = ART / "workbuddy-working-cutout-v1"
BASE = ART / "workbuddy-working-cutout-v3"
OUT = ART / "workbuddy-working-cutout-v4"
REFERENCE = ROOT / "artifacts/design/preprocessed/source-videos/workbuddy-working-v3/neutral-reference-1920x1080.png"
REGIONS = (("trousers", 510, 686), ("cuffs", 686, 715), ("skin", 715, 744))


def recover_green(rgb, palette):
    """Solve C = u*palette + spill*[0,255,0], then undo coverage loss.

    R/B estimate foreground shading independently of green contamination.
    Unlike channel clipping, recovery restores the lost brightness and warm
    garment/skin chroma. Only a positive green residual activates correction.
    """
    rgb = np.asarray(rgb, dtype=np.float32)
    palette = np.asarray(palette, dtype=np.float32)
    shade_coverage = ((rgb[..., 0] * palette[0] + rgb[..., 2] * palette[2]) /
                      (palette[0] ** 2 + palette[2] ** 2))
    green_residual = rgb[..., 1] - shade_coverage * palette[1]
    spill = np.clip(green_residual / 255, 0, .85)
    shade = shade_coverage / np.maximum(1 - spill, .15)
    recovered = np.clip(shade[..., None] * palette, 0, 255)
    t = np.clip((green_residual - 2) / 8, 0, 1)
    weight = t * t * (3 - 2 * t)
    return recovered, weight


def reference_palettes():
    previous = json.loads((RAW / "validation.json").read_text(encoding="utf-8"))
    with Image.open(REFERENCE) as image:
        rgb = np.asarray(image.convert("RGB").crop(previous["cropInSourceCanvas"])).astype(float)
    y, x = np.indices(rgb.shape[:2])
    r, g, b = np.moveaxis(rgb, -1, 0)
    values = {}
    for name, top, bottom in (("trousers", 515, 676), ("cuffs", 692, 708), ("skin", 716, 732)):
        selected = (y >= top) & (y < bottom) & (x > 95) & (x < 290)
        selected &= (r > 180) & (r >= g) & (r >= b) & (g > 150) & (b > 140)
        values[name] = np.median(rgb[selected], axis=0).tolist()
    return values


def correct_lower_body(raw_image, baseline_image, palettes):
    raw = np.asarray(raw_image.convert("RGBA"))
    baseline = np.asarray(baseline_image.convert("RGBA"))
    if raw.shape != baseline.shape or raw.shape[:2] != (768, 384):
        raise ValueError("This calibration requires the fixed 384x768 working crop")
    result = baseline.copy()
    y, x = np.indices(raw.shape[:2])
    counts = {}
    for name, top, bottom in REGIONS:
        recovered, weight = recover_green(raw[:, :, :3], palettes[name])
        region = (y >= top) & (y < bottom) & (x >= 96) & (x < 290) & (baseline[:, :, 3] > 0)
        weight *= region
        affected = weight > 0
        # Start from pre-clamp RGB, so a gray v3 stain cannot become our color
        # estimate. The existing matte remains exactly as already approved.
        color = raw[:, :, :3] * (1 - weight[:, :, None]) + recovered * weight[:, :, None]
        result[affected, :3] = np.rint(color[affected]).astype(np.uint8)
        counts[name] = int(affected.sum())
    assert np.array_equal(result[:, :, 3], baseline[:, :, 3])
    assert np.array_equal(result[:510], baseline[:510])
    return Image.fromarray(result), counts


def on_background(image, color):
    background = Image.new("RGBA", image.size, color)
    background.alpha_composite(image)
    return background.convert("RGB")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--samples-only", action="store_true")
    args = parser.parse_args()
    palettes = reference_palettes()
    previous = json.loads((BASE / "validation.json").read_text(encoding="utf-8"))
    paths = sorted((BASE / "frames").glob("[0-9][0-9][0-9][0-9].png"))
    assert len(paths) == 124
    samples = {0, 18, 36, 60, 96, 123}
    output = OUT / "samples" if args.samples_only else OUT
    (output / "frames").mkdir(parents=True, exist_ok=True)
    sheet = Image.new("RGB", (960, 285 * len(samples)), "#eeeeee")
    draw = ImageDraw.Draw(sheet)
    records = []
    previews = []
    green_residue = 0
    for index, path in enumerate(paths):
        if args.samples_only and index not in samples:
            continue
        with Image.open(RAW / "frames" / path.name) as raw, Image.open(path) as baseline:
            corrected, counts = correct_lower_body(raw, baseline, palettes)
            pixels = np.asarray(corrected).astype(int)
            residue = ((pixels[:, :, 3] > 32) &
                       (pixels[:, :, 1] - np.maximum(pixels[:, :, 0], pixels[:, :, 2]) > 16))
            green_residue += int(residue.sum())
            assert not residue.any(), (index, int(residue.sum()))
            corrected.save(output / "frames" / path.name)
            records.append({"frame": index, "changedByRegion": counts, "greenResiduePixels": int(residue.sum())})
            if index in samples:
                row = sorted(samples).index(index)
                for col, (label, image) in enumerate((("before / white", baseline), ("after / white", corrected),
                                                       ("before / dark", baseline), ("after / dark", corrected))):
                    bg = "white" if col < 2 else "#252a36"
                    crop = on_background(image, bg).crop((88, 490, 298, 744))
                    sheet.paste(crop, (col * 240 + 15, row * 285 + 26))
                    draw.text((col * 240 + 6, row * 285 + 7), f"f{index} / {label}", fill="black")
            if not args.samples_only:
                previews.append(corrected.resize((192, 384), Image.Resampling.LANCZOS))
    sheet.save(output / "leg-comparison.png")
    if not args.samples_only:
        fps = previous["frameRate"]
        durations = [round((i + 1) * 1000 / fps) - round(i * 1000 / fps) for i in range(len(previews))]
        previews[0].save(output / "working-transparent-preview.webp", save_all=True, append_images=previews[1:],
                         duration=durations, loop=0, lossless=True, exact=True, method=1)
        report = {**previous, "refinedFrom": str(BASE), "originalUnclippedColors": str(RAW),
                  "refinement": "Palette-guided green-background unmixing from pre-clamp RGB; separate trousers/cuffs/skin, fixed coordinates; baseline alpha and upper body byte-identical.",
                  "paletteReference": str(REFERENCE), "palettesRgb": palettes, "frames": records,
                  "alphaByteIdenticalToV3": True, "upperBodyByteIdenticalToV3": True,
                  "greenResiduePixels": green_residue, "lowerBodyColorsCorrected": True}
        (output / "validation.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        (output / "qa.json").write_text(json.dumps({"frames":124, "frameSize":[384,768], "fps":fps,
            "alphaByteIdenticalToV3":True, "upperBodyByteIdenticalToV3":True,
            "lowerBodyColorsCorrected":True, "greenResiduePixels":green_residue}, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"frames":len(records), "palettes":palettes, "output":str(output)}), flush=True)


if __name__ == "__main__":
    main()
