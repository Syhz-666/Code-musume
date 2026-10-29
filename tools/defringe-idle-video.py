"""Remove dark-matte contamination from the narrow edge of idle RGBA frames.

This refines existing video frames, not a generative redraw. Keep canvas,
timing, interior pixels and pose fixed. Preserve v1 for before/after review.
"""

import argparse
import json
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw
from scipy import ndimage


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "artifacts/design/preprocessed/frames/workbuddy-idle-cutout-v1"
OUTPUT = ROOT / "artifacts/design/preprocessed/frames/workbuddy-idle-cutout-v2"


def defringe(image, holdout):
    original = np.asarray(image.convert("RGBA"))
    rgb = original[:, :, :3].astype(np.float32)
    alpha = original[:, :, 3].astype(np.float32) / 255
    maximum = rgb.max(axis=2)
    minimum = rgb.min(axis=2)
    y, x = np.indices(alpha.shape)
    height, width = alpha.shape
    face = (x >= width * 0.26) & (x <= width * 0.74) & (y >= height * 0.2) & (y <= height * 0.42)
    # The approved pale-hair/pink-pajama character has near-black face lines,
    # but no near-black costume/hair fills. Blink frames also contain enclosed
    # dark backdrop islands by the sleeves that v1's holdout accidentally kept.
    background_islands = (maximum < 50) & (alpha > 0) & ~face
    active_holdout = (holdout & ~ndimage.binary_dilation(background_islands, iterations=8)) | face
    support = (alpha > 32 / 255) & ~background_islands
    depth = ndimage.distance_transform_edt(support)
    # Sample reliable bright foreground inward of the contaminated rim. The
    # character's pale hair/pink clothes provide a local color estimate, while
    # all deep opaque areas, including facial lines, are explicitly protected.
    clean = (alpha >= 250 / 255) & (depth >= 5) & (maximum >= 170)
    nearest = ndimage.distance_transform_edt(~clean, return_distances=False, return_indices=True)
    foreground = rgb[nearest[0], nearest[1]]
    coverage = np.clip(np.sum(rgb * foreground, axis=2) / np.maximum(np.sum(foreground * foreground, axis=2), 1), 0, 1)
    chroma = (maximum - minimum) / np.maximum(maximum, 1)
    edge_weight = np.clip((7 - depth) / 3, 0, 1)
    dark_weight = np.clip((0.98 - coverage) / 0.35, 0, 1)
    neutral_weight = np.clip((0.55 - chroma) / 0.2, 0, 1)
    weight = edge_weight * dark_weight * neutral_weight
    weight[(alpha == 0) | active_holdout | (depth > 7)] = 0
    # Blend in premultiplied space: a gray pixel on black becomes a brighter
    # partially covered strand, rather than an opaque gray speck on white.
    new_alpha = alpha * (1 - weight + weight * coverage)
    premultiplied = rgb * alpha[:, :, None] * (1 - weight[:, :, None])
    premultiplied += foreground * (alpha * coverage * weight)[:, :, None]
    result = original.copy()
    changed = weight > 0
    result[changed, :3] = np.rint(np.clip(premultiplied[changed] / np.maximum(new_alpha[changed, None], 1 / 255), 0, 255)).astype(np.uint8)
    result[changed, 3] = np.rint(new_alpha[changed] * 255).astype(np.uint8)
    result[background_islands] = 0
    result[result[:, :, 3] <= 3] = 0
    assert np.array_equal(result[active_holdout], original[active_holdout]), "Protected interior changed"
    assert np.all(result[:, :, 3] <= original[:, :, 3]), "Matte expanded"
    return Image.fromarray(result), {
        "changedEdgePixels": int(np.count_nonzero(np.any(result != original, axis=2))),
        "protectedInteriorPreserved": True,
        "removedEnclosedDarkBackgroundPixels": int(background_islands.sum()),
        "alphaExpanded": False,
    }


def composite(image, color):
    background = Image.new("RGBA", image.size, color)
    background.alpha_composite(image)
    return background.convert("RGB")


def comparison(items, destination):
    # 2x edge detail on white; enough resolution to see residual gray pixels.
    sheet = Image.new("RGB", (1600, 1140), "#e7e7e7")
    draw = ImageDraw.Draw(sheet)
    for column, (label, before, after) in enumerate(items):
        for row, image in enumerate((before, after)):
            white = composite(image, "white")
            crop = white.crop((25, 35, 175, 295)).resize((300, 520), Image.Resampling.NEAREST)
            sheet.paste(crop, (column * 400, row * 570 + 30))
            draw.text((column * 400 + 8, row * 570 + 8), f"{label} / {'before' if row == 0 else 'after'}", fill="black")
    sheet.save(destination)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=SOURCE)
    parser.add_argument("--output", type=Path, default=OUTPUT)
    parser.add_argument("--samples-only", action="store_true")
    args = parser.parse_args()
    source, output = args.source, args.output
    output.mkdir(parents=True, exist_ok=True)
    (output / "frames").mkdir(exist_ok=True)
    with Image.open(source / "frames/0000.png") as first:
        holdout = ndimage.binary_erosion(np.asarray(first)[:, :, 3] >= 254, iterations=12)
    previous = json.loads((source / "validation.json").read_text(encoding="utf-8"))
    indexes = (0, 48, 87, 120, 181, 240) if args.samples_only else range(previous["frameCount"])
    items, records = [], []
    for index in indexes:
        with Image.open(source / f"frames/{index:04d}.png") as image:
            corrected, record = defringe(image, holdout)
            corrected.save(output / f"frames/{index:04d}.png")
            records.append({"frame": index, **record})
            if index in (0, 48, 120, 240):
                items.append((f"{index / 24:.2f}s", image.copy(), corrected.copy()))
            if not args.samples_only and (index + 1) % 24 == 0:
                print(f"Refined {index + 1} frames", flush=True)
    comparison(items, output / "edge-comparison.png")
    report = {
        **previous,
        "refinedFrom": str(source),
        "refinement": "Face holdout; remove near-black non-face backdrop islands specific to the pale-hair/pink-pajama palette; local inward foreground color estimate; black-matte coverage correction within a 7px boundary band; premultiplied blending; reliable interior protected.",
        "changedEdgePixels": sum(r["changedEdgePixels"] for r in records),
        "protectedInteriorPreserved": all(r["protectedInteriorPreserved"] for r in records),
        "opaqueRgbPreserved": False,
        "opaqueRgbNote": "Refinement intentionally changes contaminated boundary RGB, including some pixels the initial matte classified as opaque. The protected deep interior remains byte-identical to v1.",
        "previousMattingValidation": str(source / "validation.json"),
        "frames": records,
        "refinementFrames": records,
        "refinementSampleOnly": args.samples_only,
    }
    (output / "validation.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"frames": len(records), "changedEdgePixels": report["changedEdgePixels"]}, ensure_ascii=False), flush=True)


if __name__ == "__main__":
    main()
