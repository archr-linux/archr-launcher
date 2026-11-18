#!/usr/bin/env python3
"""Rebuild the UI-only CJK fallback with fonttools 4.61.1 (Python 3.11+).

Download the pinned upstream font separately; this script never uses the network.
Run: python -I tools/subset-cjk.py /path/to/NotoSansCJKsc-Regular.otf
"""
import hashlib
from pathlib import Path
import sys
import tomllib

from fontTools import subset
from fontTools.ttLib import TTFont

ROOT = Path(__file__).resolve().parent.parent
SOURCE_SHA256 = "2c76254f6fc379fddfce0a7e84fb5385bb135d3e399294f6eeb6680d0365b74b"


def main():
    source = Path(sys.argv[1])
    if hashlib.sha256(source.read_bytes()).hexdigest() != SOURCE_SHA256:
        raise SystemExit("Expected the unmodified Noto Sans CJK SC Regular Sans2.004 font")
    catalog = tomllib.loads((ROOT / "locales/zh_CN.toml").read_text())
    strings = [s for value in catalog.values()
               for s in (value.values() if isinstance(value, dict) else [value])]
    # Include uppercase headings and the autonym visible in every locale.
    text = "".join(strings) + "".join(strings).upper() + "简体中文"
    # Reserve glyphs for the image/save-sync translations awaiting a merge.
    # Keep this input in-tree so rebuilding never depends on temporary files.
    text += (ROOT / "fonts/cjk/pending-glyphs.txt").read_text()
    unicodes = {ord(c) for c in text if ord(c) > 127}
    font = TTFont(source, recalcTimestamp=False)
    missing = unicodes - font.getBestCmap().keys()
    if missing:
        raise SystemExit(f"Upstream font lacks {sorted(missing)}")
    options = subset.Options()
    options.layout_features = []
    options.name_IDs = [0, 1, 2, 3, 4, 5, 6, 13, 14]
    options.notdef_outline = True
    subsetter = subset.Subsetter(options=options)
    subsetter.populate(unicodes=unicodes)
    subsetter.subset(font)
    # A derivative name, retaining the upstream copyright and OFL metadata.
    names = {1: "Arch R UI SC", 2: "Regular", 3: "ArchR-UI-SC-Regular-2.004",
             4: "Arch R UI SC Regular", 6: "ArchR-UI-SC-Regular"}
    license_text = (ROOT / "fonts/cjk/OFL.txt").read_text()
    for entry in font["name"].names:
        if entry.nameID == 13:
            # Full license travels inside the embedded font, even if the
            # downstream package only installs the launcher executable.
            entry.string = license_text.encode(entry.getEncoding())
        elif entry.nameID in names:
            entry.string = names[entry.nameID].encode(entry.getEncoding())
    cff = font["CFF "].cff
    cff.fontNames = ["ArchR-UI-SC-Regular"]
    cff.topDictIndex[0].FamilyName = names[1]
    cff.topDictIndex[0].FullName = names[4]
    output = ROOT / "fonts/cjk/ArchR-UI-SC.otf"
    font.save(output)
    print(f"{len(unicodes)} codepoints, {output.stat().st_size} bytes: {output}")


if __name__ == "__main__":
    main()
