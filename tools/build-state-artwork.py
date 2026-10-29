"""Extract original green-screen keyframes, align feet, and pack the runtime atlas.

Requires Pillow, NumPy and SciPy. Run from any directory:
    python tools/build-state-artwork.py
    python tools/build-state-artwork.py --install
"""

import argparse
import hashlib
import json
import shutil
from pathlib import Path

from PIL import Image, ImageDraw

from chroma_key import extract_green


ROOT = Path(__file__).resolve().parents[1]
STATES = (
    "idle", "thinking", "working", "waiting-permission",
    "notification", "done", "error", "sleeping",
)
CELL = (384, 416)
BODY_HEIGHT = 368
BASELINE = 400


def solid_bounds(image):
    return image.getchannel("A").point(lambda value: 255 if value >= 128 else 0).getbbox()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--install", action="store_true", help="Update the bundled runtime atlas")
    parser.add_argument(
        "--source", type=Path,
        default=ROOT / "artifacts/design/preprocessed/keyframes/state-design-v1",
    )
    parser.add_argument(
        "--output", type=Path,
        default=ROOT / "artifacts/design/preprocessed/frames/state-cutouts-v2",
    )
    args = parser.parse_args()
    if args.install:
        manifest = json.loads((ROOT / "apps/pet/assets/builtin/musume.json").read_text(encoding="utf-8"))
        if any(form["columns"] != 1 or form["clips"]["idle"]["frames"] != 1 for form in manifest["forms"]):
            raise SystemExit("当前内置包已包含动画。先不带 --install 生成静态输入，再运行 python tools/build-idle-atlas.py --install，避免图集与清单不匹配。")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    (output / "raw").mkdir(exist_ok=True)
    atlas = Image.new("RGBA", (CELL[0], CELL[1] * len(STATES)))
    # Side-by-side light/dark checks at the app's actual 144 x 156 display size.
    contact = Image.new("RGB", (320 * 4, 216 * 2), "#ecebf2")
    draw = ImageDraw.Draw(contact)
    records = []
    for row, state in enumerate(STATES):
        path = args.source / f"{row + 1:02d}-{state}.png"
        with Image.open(path) as original:
            image, extraction = extract_green(original)
        image.save(output / "raw" / path.name, optimize=True)
        alpha = image.getchannel("A")
        assert alpha.getextrema() == (0, 255), f"{state}: missing transparency or solid subject"
        assert all(alpha.getpixel(p) == 0 for p in (
            (0, 0), (image.width - 1, 0), (0, image.height - 1),
            (image.width - 1, image.height - 1),
        )), f"{state}: opaque background corner"
        green_pixels = sum(
            1 for r, g, b, a in image.getdata()
            if a > 32 and g > 140 and g - max(r, b) > 45
        )
        assert green_pixels == 0, f"{state}: green-screen residue ({green_pixels} pixels)"
        bounds = solid_bounds(image)
        assert bounds is not None, f"{state}: empty subject"
        scale = BODY_HEIGHT / (bounds[3] - bounds[1])
        # Resize existing alpha artwork, then use its feet as the shared origin.
        resized = image.resize(
            (round(image.width * scale), round(image.height * scale)),
            Image.Resampling.LANCZOS,
        )
        resized_bounds = solid_bounds(resized)
        foot_band = resized.getchannel("A").crop(
            (0, resized_bounds[3] - 14, resized.width, resized_bounds[3])
        ).point(lambda value: 255 if value >= 128 else 0).getbbox()
        foot_x = (foot_band[0] + foot_band[2]) / 2
        offset = (round(CELL[0] / 2 - foot_x), BASELINE - resized_bounds[3])
        frame = Image.new("RGBA", CELL)
        frame.alpha_composite(resized, offset)
        frame_bounds = solid_bounds(frame)
        full_bounds = frame.getchannel("A").getbbox()
        assert all((full_bounds[0] > 0, full_bounds[1] > 0,
                    full_bounds[2] < CELL[0], full_bounds[3] < CELL[1])), state
        assert frame_bounds[3] == BASELINE, f"{state}: inconsistent foot baseline"
        frame.save(output / f"{row + 1:02d}-{state}.png", optimize=True)
        atlas.alpha_composite(frame, (0, row * CELL[1]))
        tile_x, tile_y = row % 4 * 320, row // 4 * 216
        draw.text((tile_x + 10, tile_y + 8), state, fill="#333344")
        preview = frame.resize((144, 156), Image.Resampling.LANCZOS)
        for column, background in enumerate(("#fafafa", "#252a36")):
            panel = Image.new("RGBA", (152, 168), background)
            panel.alpha_composite(preview, (4, 6))
            contact.paste(panel.convert("RGB"), (tile_x + 6 + column * 156, tile_y + 30))
        records.append({
            "state": state,
            "source": str(path.relative_to(ROOT)) if path.is_relative_to(ROOT) else str(path),
            "sourceSha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "sourceSize": list(image.size),
            "extraction": extraction,
            "sourceSolidBounds": list(bounds),
            "scale": scale,
            "frameSolidBounds": list(frame_bounds),
            "frameFullAlphaBounds": list(full_bounds),
            "footBaseline": BASELINE,
            "greenResiduePixels": green_pixels,
        })
    atlas.save(output / "spritesheet.png", optimize=True)
    contact.save(output / "contact-sheet.png", optimize=True)
    report = {
        "status": "passed",
        "method": "deterministic chroma key from original green-screen images; resize and foot alignment",
        "atlasSize": list(atlas.size),
        "cellSize": list(CELL),
        "columns": 1,
        "framesPerState": 1,
        "animated": False,
        "states": records,
        "atlasSha256": hashlib.sha256((output / "spritesheet.png").read_bytes()).hexdigest(),
    }
    (output / "validation.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    if args.install:
        destination = ROOT / "apps/pet/assets/builtin/spritesheet.png"
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(output / "spritesheet.png", destination)
    print(json.dumps({key: report[key] for key in (
        "status", "atlasSize", "cellSize", "framesPerState", "animated", "atlasSha256"
    )}, ensure_ascii=False))


if __name__ == "__main__":
    main()
