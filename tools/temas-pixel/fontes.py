#!/usr/bin/env python3
"""Pixel fonts for the Retro and Praia themes, from the official Google Fonts
repository (SIL OFL 1.1):

- Pixelify Sans (no Reserved Font Name): static Regular and SemiBold cut from
  the variable font, with three characters the upstream font lacks: Cyrillic
  О (the Latin O glyph), Cyrillic П (drawn from the font's own Н) and the
  non-breaking hyphen (the hyphen glyph).
- Press Start 2P (Reserved Font Name "Press Start 2P"): copied unmodified.

Usage: python3 fontes.py <out-dir>   (needs fontTools; downloads with curl)
"""
import subprocess
import sys
import tempfile
from pathlib import Path

from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.pens.recordingPen import RecordingPen
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

BASE = "https://github.com/google/fonts/raw/main/ofl"


def fetch(path, into):
    target = into / Path(path).name
    subprocess.run(["curl", "-fsSL", "-o", str(target), f"{BASE}/{path}"], check=True)
    return target


def add_pe(font):
    """Cyrillic П from the font's Н: same stems, the bar moved to the top."""
    glyphs = font.getGlyphSet()
    pen = RecordingPen()
    glyphs["uni041D"].draw(pen)
    points = [args[0] for op, args in pen.value if args]
    xs = sorted({x for x, _ in points})
    ys = sorted({y for _, y in points})
    (left, left_in, right_in, right), (bottom, bar_low, bar_high, top) = xs, ys
    outline = TTGlyphPen(None)
    outline.moveTo((left, bottom))
    for point in [(left, top), (right, top), (right, bottom), (right_in, bottom),
                  (right_in, top - (bar_high - bar_low)), (left_in, top - (bar_high - bar_low)),
                  (left_in, bottom)]:
        outline.lineTo(point)
    outline.closePath()
    font["glyf"]["uni041F"] = outline.glyph()
    font["hmtx"]["uni041F"] = font["hmtx"]["uni041D"]
    if "uni041F" not in font.getGlyphOrder():
        font.setGlyphOrder(font.getGlyphOrder() + ["uni041F"])


def add_triangle(font):
    """▶ as a stepped triangle: columns one stem wide, one stem shorter
    at each end than the column before."""
    glyphs = font.getGlyphSet()
    pen = RecordingPen()
    glyphs["uni041D"].draw(pen)
    points = [args[0] for op, args in pen.value if args]
    xs = sorted({x for x, _ in points})
    bottom = min(y for _, y in points)
    left, unit = xs[0], xs[1] - xs[0]
    steps = 3
    top = bottom + 2 * steps * unit
    path = [(left, bottom), (left, top)]
    for i in range(steps):
        x = left + (i + 1) * unit
        path += [(x, top - i * unit), (x, top - (i + 1) * unit)]
    for i in reversed(range(steps)):
        x = left + (i + 1) * unit
        path += [(x, bottom + (i + 1) * unit), (x, bottom + i * unit)]
    outline = TTGlyphPen(None)
    outline.moveTo(path[0])
    for point in dict.fromkeys(path[1:]):
        outline.lineTo(point)
    outline.closePath()
    font["glyf"]["uni25B6"] = outline.glyph()
    font["hmtx"]["uni25B6"] = (2 * left + steps * unit, left)
    if "uni25B6" not in font.getGlyphOrder():
        font.setGlyphOrder(font.getGlyphOrder() + ["uni25B6"])


def pixelify(variable, weight, style, out):
    font = instantiateVariableFont(TTFont(variable), {"wght": weight}, updateFontNames=False)
    add_pe(font)
    add_triangle(font)
    for table in font["cmap"].tables:
        if table.isUnicode():
            table.cmap[0x041E] = "O"
            table.cmap[0x041F] = "uni041F"
            table.cmap[0x2011] = "hyphen"
            table.cmap[0x25B6] = "uni25B6"
    names = font["name"]
    for record in list(names.names):
        if record.nameID in (2, 4, 6, 16, 17):
            names.removeNames(nameID=record.nameID)
    names.setName(style, 2, 3, 1, 0x409)
    names.setName(f"Pixelify Sans {style}", 4, 3, 1, 0x409)
    names.setName(f"PixelifySans-{style.replace(' ', '')}", 6, 3, 1, 0x409)
    font["OS/2"].usWeightClass = weight
    font.save(out / f"PixelifySans-{style.replace(' ', '')}.ttf")


def main(out):
    out.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        variable = fetch("pixelifysans/PixelifySans%5Bwght%5D.ttf", tmp)
        pixelify(variable, 400, "Regular", out)
        pixelify(variable, 600, "SemiBold", out)
        fetch("pixelifysans/OFL.txt", tmp).rename(out / "PixelifySans-OFL.txt")
        fetch("pressstart2p/PressStart2P-Regular.ttf", tmp).rename(out / "PressStart2P-Regular.ttf")
        fetch("pressstart2p/OFL.txt", tmp).rename(out / "PressStart2P-OFL.txt")


if __name__ == "__main__":
    main(Path(sys.argv[1]))
