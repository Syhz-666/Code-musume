"""Prepare existing approved art as video references; never redraw the character."""
import json
import shutil
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "artifacts/design/preprocessed/source-videos/workbuddy-thinking-v1"
OUTPUT.mkdir(parents=True, exist_ok=True)
shutil.copyfile(ROOT / "artifacts/design/preprocessed/source-videos/workbuddy-idle-v1/idle-reference-1920x1080.png", OUTPUT / "neutral-reference-1920x1080.png")
source = ROOT / "artifacts/design/preprocessed/frames/state-cutouts-v2/raw/02-thinking.png"
image = Image.open(source).convert("RGBA")
bounds = image.getchannel("A").point(lambda a: 255 if a >= 128 else 0).getbbox()
scale = 702 / (bounds[3] - bounds[1])
scaled = image.resize((round(image.width * scale), round(image.height * scale)), Image.Resampling.LANCZOS)
solid = scaled.getchannel("A").point(lambda a: 255 if a >= 128 else 0).getbbox()
feet = scaled.getchannel("A").crop((0, solid[3] - 20, scaled.width, solid[3])).point(lambda a: 255 if a >= 128 else 0).getbbox()
offset = (round(960 - (feet[0] + feet[2]) / 2), 918 - solid[3])
canvas = Image.new("RGBA", (1920, 1080), "#00ff00")
canvas.alpha_composite(scaled, offset)
canvas.convert("RGB").save(OUTPUT / "thinking-pose-1920x1080.png", optimize=True)
(OUTPUT / "reference-validation.json").write_text(json.dumps({"source": str(source), "scale": scale, "offset": offset, "canvas": [1920, 1080], "feetY": 918, "generativeRedraw": False}, indent=2) + "\n", encoding="utf-8")
print(OUTPUT)
