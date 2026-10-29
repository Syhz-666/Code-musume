"""Subset the pinned, OFL-licensed Noto Sans SC source for desktop screenshots.

Install fonttools[woff] and pass the downloaded TTF path. Normal builds do not
need Python: the output WOFF2 is committed and used only in e2e builds.
"""
import argparse
import hashlib
import json
from pathlib import Path

from fontTools import subset
from fontTools.ttLib import TTFont

ROOT = Path(__file__).resolve().parent.parent
SOURCE_URL = (
    "https://raw.githubusercontent.com/google/fonts/"
    "b68b528a75da7d9b73f3928899c07a23083a6633/ofl/notosanssc/NotoSansSC%5Bwght%5D.ttf"
)


def main():
    parser = argparse.ArgumentParser(__doc__)
    parser.add_argument("source", type=Path)
    args = parser.parse_args()
    source_hash = hashlib.sha256(args.source.read_bytes()).hexdigest()
    if source_hash != "a3041811a78c361b1de50f953c805e0244951c21c5bd412f7232ef0d899af0da":
        raise SystemExit("Source font does not match the pinned upstream SHA-256")
    files = []
    for directory in ["apps/pet/src", "packages/protocol/src", "crates", "tests/desktop/specs"]:
        files.extend(p for p in (ROOT / directory).rglob("*") if p.suffix in {".ts", ".tsx", ".rs", ".mjs"})
    files.extend(ROOT / p for p in ["apps/pet/index.html", "apps/pet/settings.html", "apps/pet/assets/builtin/musume.json"])
    characters = set(range(32, 127))
    for path in files:
        characters.update(map(ord, path.read_text(encoding="utf-8")))
    font = TTFont(args.source, recalcTimestamp=False)
    options = subset.Options()
    options.flavor = "woff2"
    options.recalc_timestamp = False
    subsetter = subset.Subsetter(options=options)
    subsetter.populate(unicodes=characters)
    subsetter.subset(font)
    # Give the modified font its own name; preserve original license metadata.
    for name in font["name"].names:
        if name.nameID in {1, 2, 3, 4, 6, 16, 17}:
            value = "Regular" if name.nameID in {2, 17} else "Code Musume Test"
            if name.nameID == 6:
                value = "CodeMusumeTest-Regular"
            name.string = value.encode(name.getEncoding())
    directory = ROOT / "tests/desktop/fonts"
    font.flavor = "woff2"
    target = directory / "code-musume-test.woff2"
    font.save(target)
    (directory / "source.json").write_text(json.dumps({
        "url": SOURCE_URL,
        "sourceSha256": source_hash,
        "outputSha256": hashlib.sha256(target.read_bytes()).hexdigest(),
        "characters": "".join(map(chr, sorted(font.getBestCmap()))),
    }, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"Wrote {target.relative_to(ROOT)} ({target.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
