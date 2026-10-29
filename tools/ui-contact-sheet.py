"""Make review sheets from this application's captured PNGs; never capture the desktop."""
import json
import sys
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

directory = Path(sys.argv[1]).resolve()
visuals = json.loads((directory / "visual-results.json").read_text(encoding="utf-8"))
for pattern in ("*manual-*.png", "animated-*.png", "thinking-*.png", "working-*.png"):
    visuals.extend({"name": path.stem, "actual": str(path)} for path in sorted(directory.glob(pattern)))
font = ImageFont.truetype("C:/Windows/Fonts/arial.ttf", 15)

for group, columns, cell_width, cell_height in [("pets", 8, 184, 250), ("settings", 2, 872, 796)]:
    items = [item for item in visuals if item["name"].startswith("settings-") == (group == "settings")]
    rows = (len(items) + columns - 1) // columns
    sheet = Image.new("RGB", (columns * cell_width, rows * cell_height), "#eae7ef")
    draw = ImageDraw.Draw(sheet)
    for index, item in enumerate(items):
        path = Path(item.get("actual", directory / f"{item['name']}.png")).resolve()
        if not path.is_relative_to(directory):
            raise ValueError("Review input must belong to this test run")
        image = Image.open(path).convert("RGBA")
        x = (index % columns) * cell_width + 10
        y = (index // columns) * cell_height + 8
        draw.rectangle((x - 2, y - 2, x + cell_width - 16, y + cell_height - 12), fill="white")
        sheet.paste(image, (x, y + 22), image)
        draw.text((x, y), item["name"], fill="#44334f", font=font)
    output = directory / f"review-{group}.png"
    sheet.save(output)
    print(output)
