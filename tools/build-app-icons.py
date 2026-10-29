"""Package the user-approved upright avatar as transparent Windows icons.

python tools/build-app-icons.py --install
Uses the checked-in PNG master; no image model or ignored art directory required.
"""
import argparse
import hashlib
import json
import shutil
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "apps/pet/assets/branding/avatar.png"
DESTINATION = ROOT / "apps/pet/src-tauri/icons"
OUTPUT = ROOT / "artifacts/design/icon-candidates/tray-avatar-v2-upright/windows-pack"
BACKUP = ROOT / "artifacts/design/backups/icons-before-avatar-2026-10-29"
PNG_SIZES = {"icon.png": 512, "32x32.png": 32, "64x64.png": 64,
             "128x128.png": 128, "128x128@2x.png": 256, "tray-icon.png": 32}
ICO_SIZES = [16, 20, 24, 32, 40, 48, 64, 128, 256]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--install", action="store_true")
    args = parser.parse_args()
    master = Image.open(SOURCE).convert("RGBA")
    assert master.size == (512, 512)
    assert master.getextrema()[3] == (0, 255)
    OUTPUT.mkdir(parents=True, exist_ok=True)
    for name, size in PNG_SIZES.items():
        image = master.resize((size, size), Image.Resampling.LANCZOS)
        image.save(OUTPUT / name)
        assert image.getpixel((0, 0))[3] == 0
    master.save(OUTPUT / "icon.ico", format="ICO", sizes=[(s, s) for s in ICO_SIZES])
    with Image.open(OUTPUT / "icon.ico") as icon:
        assert icon.ico.sizes() == {(s, s) for s in ICO_SIZES}
        for size in ICO_SIZES:
            frame = icon.ico.getimage((size, size)).convert("RGBA")
            expected = master.resize((size, size), Image.Resampling.LANCZOS)
            assert frame.tobytes() == expected.tobytes(), size
    files = [*PNG_SIZES, "icon.ico"]
    report = {"source": str(SOURCE.relative_to(ROOT)), "sourceSha256": hashlib.sha256(SOURCE.read_bytes()).hexdigest(),
              "icoSizes": ICO_SIZES, "traySize": 32, "icoFramesMatchMasterResizes": True,
              "alphaPreserved": True, "installed": args.install,
              "files": {name: hashlib.sha256((OUTPUT / name).read_bytes()).hexdigest() for name in files}}
    if args.install:
        BACKUP.mkdir(parents=True, exist_ok=True)
        for name in files:
            old = DESTINATION / name
            if old.is_file() and not (BACKUP / name).exists():
                shutil.copyfile(old, BACKUP / name)
            shutil.copyfile(OUTPUT / name, old)
    (OUTPUT / "validation.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps({"installed": args.install, "icoSizes": ICO_SIZES, "traySize": 32, "output": str(OUTPUT)}))


if __name__ == "__main__":
    main()
