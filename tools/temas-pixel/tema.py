#!/usr/bin/env python3
"""Builds the Praia or Retro theme folder (a theme store repository root).

Usage: python3 tema.py <praia|retro> <out-dir> <fonts-dir> <cc-by-4.0.txt>
  fonts-dir: the output of fontes.py; cc-by-4.0.txt: the CC BY 4.0 legal code.
Needs Pillow and ffmpeg (sounds). The art and sounds are original, drawn and
synthesized here; the fonts keep their own SIL OFL 1.1.
"""
import shutil
import sys
from pathlib import Path

import pixelart as P
import sons

B, S = P.BEACH, P.SWEETIE
CONSOLES = list(P.CONSOLES)


def save(img, out, name):
    path = out / name
    path.parent.mkdir(parents=True, exist_ok=True)
    img.save(path, optimize=True)


# ------------------------------------------------------------------ Praia

def praia_art(out):
    save(P.caustics(320, 240), out, "images/background.png")
    header = P.canvas(160, 29)
    P.paste(header, P.wood(160, 22, seed=8), 0, 0)
    P.rect(header, 0, 21, 160, 1, B["outline"])
    P.paste(header, P.thatch(162, 7, seed=9), -1, 20)
    save(header, out, "frames/header.png")
    footer = P.canvas(160, 32)
    P.paste(footer, P.foam(160, 10, seed=4), 0, 0)
    P.paste(footer, P.wood(160, 24, seed=12), 0, 8)
    P.rect(footer, 0, 8, 160, 1, B["outline"])
    save(footer, out, "frames/footer.png")
    save(P.ragged(64, 64, seed=41, inset=False), out, "frames/slab.png")
    slab = P.ragged(64, 64, seed=53, inset=False)
    P.well(slab, 6, 6, 54, 52, 56, base="parchment")
    save(slab, out, "frames/panel.png")
    art = P.canvas(40, 40)
    P.paste(art, P.drop_shadow(P.canvas(36, 36, B["outline"]), 90), 3, 3)
    P.paste(art, P.wood(36, 36, seed=44, plank=6), 0, 0)
    for r in [(0, 0, 36, 1), (0, 35, 36, 1), (0, 0, 1, 36), (35, 0, 1, 36)]:
        P.rect(art, *r, B["outline"])
    P.rect(art, 4, 4, 28, 28, B["night"])
    save(art, out, "frames/art.png")
    save(P.button(40, 15, seed=2), out, "frames/row.png")
    save(P.button(40, 15, selected=True), out, "frames/selected.png")
    save(P.button(14, 14, seed=1), out, "frames/badge.png")
    tile = P.ragged(48, 48, seed=50, inset=False)
    save(tile, out, "frames/tile.png")
    big = P.ragged(48, 48, seed=51, inset=False)
    save(P.outline(P.outline(P.padded(big, 2), B["foam"]), B["sea"]), out, "frames/tile_selected.png")
    for name in ["crab", "shell", "starfish", "palm", "ball", "bucket"]:
        save(P.DECOR[name](), out, f"decor/{name}.png")
    save(P.foam(188, 8, seed=6), out, "decor/foam-wide.png")
    save(P.foam(116, 8, seed=200), out, "decor/foam-narrow.png")
    plate = P.ragged(108, 46, seed=46, inset=False)
    P.well(plate, 5, 5, 98, 34, 47, base="parchment")
    save(plate, out, "decor/plate.png")
    beach = P.canvas(320, 116)
    P.paste(beach, P.foam(320, 10, seed=31), 0, 0)
    P.paste(beach, P.ripples(P.sand(320, 104, seed=33, grain=0.6), seed=34), 0, 6)
    P.paste(beach, P.foam(320, 10, seed=35).transpose(P.Image.Transpose.FLIP_TOP_BOTTOM), 0, 106)
    save(beach, out, "decor/beach.png")


PRAIA_TOML = '''# Praia: pixel-art beach. Sand and wood over the sea, dark ink on smooth
# parchment, light titles with a dark outline. Keys: docs/temas.md of the
# archr-launcher.
name = { en = "Beach", pt_BR = "Praia", es = "Playa", ru = "Пляж" }
description = { en = "Pixel-art beach: sand panels, wooden bars and a turquoise sea.", pt_BR = "Praia em pixel-art: painéis de areia, barras de madeira e mar turquesa.", es = "Playa en pixel art: paneles de arena, barras de madera y mar turquesa.", ru = "Пиксельный пляж: песчаные панели, деревянные полосы и бирюзовое море." }
author = "ArchR"
version = "1.0.0"
license = "CC BY 4.0 (art, sounds); OFL 1.1 (fonts)"
min_launcher = "0.1.0"

background = "#F4C28A"
surface = "#FFE9C4"
surface_elevated = "#FFF6E2"
border = "#CF8448"
badge = "#E8A666"
text = "#22110A"
text_secondary = "#22110A"
muted = "#6B3B20"
dim = "#8A512C"
accent = "#45D6DC"
accent_text = "#083A5E"
accent_text_secondary = "#0B4F78"
success = "#3F9D3A"
warning = "#E59B2F"
scrim = "#083A5E99"

font = "fonts/PixelifySans-SemiBold.ttf"
font_semibold = "fonts/PixelifySans-SemiBold.ttf"
font_mono = "fonts/PixelifySans-SemiBold.ttf"
font_mono_medium = "fonts/PixelifySans-SemiBold.ttf"
font_scale = 1.6
font_smoothing = false
font_min = 14
text_outline = { color = "#3F2214", width = 2, ink = "#FFFFFF" }

corners = 0
border_width = 2
shadow = "hard"
art_scaling = "pixel"
easing = "steps"
header_height = 0.1
footer_height = 0.1

# Details grid: 16 px margins, columns 16-256 and 272-624, panels 60-392.
details_shot = { x = 0.025, y = 0.125, w = 0.375, h = 0.3 }
details_info = { x = 0.046875, y = 0.491667, w = 0.33125, h = 0.3 }
details_title = { x = 0.446875, y = 0.154167, w = 0.509375, h = 0.095833 }
details_subtitle = { x = 0.446875, y = 0.258333, w = 0.509375, h = 0.05 }
details_text = { x = 0.446875, y = 0.325, w = 0.509375, h = 0.354167 }
details_actions = { x = 0.446875, y = 0.704167, w = 0.509375, h = 0.075 }

background_image = "images/background.png"

[frames.header]
image = "frames/header.png"
slice = [0, 29, 0, 0]
scale = 2
pad = [0, 0, 0, 10]
tile = true

[frames.footer]
image = "frames/footer.png"
slice = [0, 32, 0, 0]
scale = 2
pad = [0, 16, 0, 0]
tile = true

[frames.list]
image = "frames/slab.png"
slice = 12
scale = 2
pad = [14, 12, 14, 6]
tile = true

[frames.panel]
image = "frames/panel.png"
slice = 12
scale = 2
pad = 14
tile = true

[frames.art]
image = "frames/art.png"
slice = 6
scale = 2
pad = 8

[frames.row]
image = "frames/row.png"
slice = 5
scale = 2
tile = true

[frames.selected]
image = "frames/selected.png"
slice = 5
scale = 2
tile = true

[frames.button]
image = "frames/row.png"
slice = 5
scale = 2

[frames.button_selected]
image = "frames/selected.png"
slice = 5
scale = 2
tile = true

[frames.badge]
image = "frames/badge.png"
slice = 5
scale = 2
pad = 4

[frames.tile]
image = "frames/tile.png"
slice = 12
scale = 2
tile = true

[frames.tile_selected]
image = "frames/tile_selected.png"
slice = 14
scale = 2
pad = 4
tile = true

[view.lista]
name = { en = "List", pt_BR = "Lista", es = "Lista", ru = "Список" }

[view.lista.systems]
title = "screen"
divider = false
summary = false
list = { x = 0.046875, y = 0.15, w = 0.53125, h = 0.675, row = 0.0625, selected = 0.0625, gap = 0.0125, font = 0.03125, selected_font = 0.03125, radius = 0, cursor = "▶" }
panel = { x = 0.625, y = 0.125, w = 0.35, h = 0.7 }
art = { x = 0.646875, y = 0.15, w = 0.30625, h = 0.266667 }
info = { x = 0.6625, y = 0.441667, w = 0.29, h = 0.35, title = 0.039, meta = 0.03125, valign = "top" }
decor = [
  { image = "decor/foam-wide.png", x = 0.01875, y = 0.825, scale = 2, layer = "back" },
  { image = "decor/foam-narrow.png", x = 0.61875, y = 0.825, scale = 2, layer = "back" },
  { image = "decor/crab.png", x = 0.025, y = 0.795, scale = 2 },
  { image = "decor/starfish.png", x = 0.9375, y = 0.79, scale = 2 },
]

[view.lista.games]
divider = false
list = { x = 0.04375, y = 0.15, w = 0.55, h = 0.675, row = 0.0625, selected = 0.0625, gap = 0.0125, font = 0.03125, selected_font = 0.03125, radius = 0, cursor = "▶" }
panel = { x = 0.6375, y = 0.141667, w = 0.3375, h = 0.458333 }
art = { x = 0.6375, y = 0.141667, w = 0.3375, h = 0.458333, fit = "contain" }
meta = { x = 0.66, y = 0.683333, w = 0.3, h = 0.058333, size = 0.026 }
decor = [
  { image = "decor/foam-wide.png", x = 0.0125, y = 0.825, scale = 2, layer = "back" },
  { image = "decor/foam-narrow.png", x = 0.63125, y = 0.825, scale = 2, layer = "back" },
  { image = "decor/plate.png", x = 0.634375, y = 0.641667, scale = 2, layer = "back" },
  { image = "decor/shell.png", x = 0.025, y = 0.8, scale = 2 },
]

[view.vitrine]
name = { en = "Showcase", pt_BR = "Vitrine", es = "Escaparate", ru = "Витрина" }

[view.vitrine.systems]
title = "screen"
list = { y = 0.233333, h = 0.383333, row = 0.266667, selected = 0.35, gap = 0.033333, radius = 0 }
info = { y = 0.666667, h = 0.15, title = 0.05, meta = 0.03125 }
dots = { y = 0.85 }
decor = [
  { image = "decor/beach.png", x = 0.0, y = 0.183333, scale = 2, layer = "back" },
  { image = "decor/palm.png", x = 0.0125, y = 0.5, scale = 2 },
  { image = "decor/ball.png", x = 0.86875, y = 0.6, scale = 2 },
  { image = "decor/bucket.png", x = 0.925, y = 0.575, scale = 2 },
]

[view.vitrine.games]
list = { font = 0.03125, selected_font = 0.04, cursor = "▶" }
'''


# ------------------------------------------------------------------ Retro

def retro_art(out):
    save(P.night(320, 240), out, "images/background.png")
    save(P.window(48, 40, 12), out, "frames/window.png")
    plain = P.window(48, 40, 0)
    save(plain, out, "frames/plain.png")
    badge = P.canvas(10, 10, S["white"])
    P.rect(badge, 1, 1, 8, 8, S["navy"])
    save(badge, out, "frames/badge.png")
    selected = P.window(48, 40, 0, body="navy", bar="yellow")
    save(selected, out, "frames/tile_selected.png")


RETRO_TOML = '''# Retro: 8-bit windows. Sweetie 16 palette (CC0, GrafxKid), Press Start 2P,
# square corners, hard shadows, motion in steps and a blinking cursor.
# Keys: docs/temas.md of the archr-launcher.
name = "Retro"
description = { en = "8-bit windows in the Sweetie 16 palette, with a blinking cursor and chiptune sounds.", pt_BR = "Janelas 8 bits na paleta Sweetie 16, com cursor piscando e sons chiptune.", es = "Ventanas de 8 bits con la paleta Sweetie 16, cursor parpadeante y sonidos chiptune.", ru = "8-битные окна в палитре Sweetie 16, мигающий курсор и чиптюн-звуки." }
author = "ArchR"
version = "1.0.0"
license = "CC BY 4.0 (art, sounds); OFL 1.1 (fonts)"
min_launcher = "0.1.0"

background = "#1A1C2C"
surface = "#29366F"
surface_elevated = "#3B5DC9"
border = "#F4F4F4"
badge = "#29366F"
text = "#F4F4F4"
text_secondary = "#F4F4F4"
muted = "#94B0C2"
dim = "#566C86"
accent = "#FFCD75"
accent_text = "#1A1C2C"
accent_text_secondary = "#333C57"
success = "#38B764"
warning = "#EF7D57"
scrim = "#1A1C2CCC"

font = "fonts/PressStart2P-Regular.ttf"
font_semibold = "fonts/PressStart2P-Regular.ttf"
font_mono = "fonts/PressStart2P-Regular.ttf"
font_mono_medium = "fonts/PressStart2P-Regular.ttf"
font_grid = 8
font_smoothing = false
font_min = 12

corners = 0
border_width = 2
shadow = "hard"
art_scaling = "pixel"
easing = "steps"

# Details: left column 18-216 (picture, facts), right column 240-627.
details_shot = { x = 0.03125, y = 0.141667, w = 0.290625, h = 0.283333 }
details_info = { x = 0.04375, y = 0.533333, w = 0.275, h = 0.316667 }
details_title = { x = 0.390625, y = 0.2, w = 0.570313, h = 0.066667 }
details_subtitle = { x = 0.390625, y = 0.275, w = 0.570313, h = 0.05 }
details_text = { x = 0.390625, y = 0.333333, w = 0.570313, h = 0.4 }
details_actions = { x = 0.390625, y = 0.766667, w = 0.570313, h = 0.075 }

background_image = "images/background.png"

[frames.list]
image = "frames/window.png"
slice = [4, 16, 18, 8]
scale = 2
pad = [10, 34, 12, 12]

[frames.panel]
image = "frames/window.png"
slice = [4, 16, 18, 8]
scale = 2
pad = [10, 34, 12, 12]

[frames.art]
image = "frames/plain.png"
slice = [4, 4, 8, 8]
scale = 2
pad = [6, 6, 8, 8]

[frames.badge]
image = "frames/badge.png"
slice = 2
scale = 2
pad = 2

[frames.tile]
image = "frames/plain.png"
slice = [4, 4, 8, 8]
scale = 2

[frames.tile_selected]
image = "frames/tile_selected.png"
slice = [4, 4, 8, 8]
scale = 2

[view.lista]
name = { en = "List", pt_BR = "Lista", es = "Lista", ru = "Список" }

[view.lista.systems]
title = "screen"
divider = false
summary = false
list = { x = 0.03125, y = 0.2, w = 0.6, h = 0.65, row = 0.066667, selected = 0.066667, gap = 0.0, font = 0.033333, selected_font = 0.033333, radius = 0, cursor = "▶" }
panel = { x = 0.6875, y = 0.2, w = 0.284375, h = 0.65 }
art = { x = 0.696875, y = 0.216667, w = 0.265625, h = 0.233333 }
info = { x = 0.70625, y = 0.466667, w = 0.25, h = 0.366667, title = 0.033333, meta = 0.033333, valign = "top" }

[view.lista.games]
divider = false
list = { x = 0.046875, y = 0.2, w = 0.565625, h = 0.65, row = 0.066667, selected = 0.066667, gap = 0.0, font = 0.033333, selected_font = 0.033333, radius = 0, cursor = "▶" }
panel = { x = 0.68125, y = 0.2, w = 0.275, h = 0.5 }
art = { x = 0.68125, y = 0.2, w = 0.275, h = 0.5, fit = "contain" }
meta = { x = 0.68125, y = 0.75, w = 0.275, h = 0.05, size = 0.033333 }

[view.vitrine]
name = { en = "Showcase", pt_BR = "Vitrine", es = "Escaparate", ru = "Витрина" }

[view.vitrine.systems]
title = "screen"
list = { radius = 0 }
info = { title = 0.033333, meta = 0.033333 }

[view.vitrine.games]
list = { font = 0.033333, selected_font = 0.033333, cursor = "▶" }
'''


README = '''# {title}

{en}

Theme for the [ArchR launcher](https://github.com/archr-linux). Install it
from Settings › Appearance › Theme store, or copy this folder to
`/storage/.config/archr-launcher/themes/`.

---

{pt}

Tema para o launcher do ArchR. Instale pela Loja de temas (Configurações ›
Aparência) ou copie esta pasta para `/storage/.config/archr-launcher/themes/`.

## License / Licença

- Art and sounds (`images/`, `frames/`, `decor/`, `systems/`, `sounds/`,
  `preview.png`, `screenshots/`): original work by ArchR, CC BY 4.0
  (`LICENSE`). Drawn and synthesized in code; no third-party material.
- Fonts (`fonts/`): {fonts}, SIL Open Font License 1.1 (`fonts/*-OFL.txt`),
  from the Google Fonts repository (github.com/google/fonts).{note}
'''

LICENSE_HEAD = '''{title} theme for the ArchR launcher
Copyright 2026 ArchR

The art and sounds of this theme (images/, frames/, decor/, systems/,
sounds/, preview.png, screenshots/) are licensed under the Creative Commons
Attribution 4.0 International license, reproduced below.

The fonts in fonts/ keep their own license, the SIL Open Font License 1.1,
in the *-OFL.txt file next to each font.

'''


def build(theme, out, fonts, cc):
    out.mkdir(parents=True, exist_ok=True)
    if theme == "praia":
        praia_art(out)
        toml, title = PRAIA_TOML, "Praia"
        names = ["PixelifySans-SemiBold.ttf", "PixelifySans-OFL.txt"]
        font_note = ("Pixelify Sans SemiBold", "\n  The 5, the Cyrillic О and П, the non-breaking hyphen and ▶ were "
                     "added or redrawn\n  by ArchR (allowed by the OFL; the font has no Reserved Font Name).")
        en = "Pixel-art beach: sand panels, wooden bars, a turquoise sea and chiptune sounds."
        pt = "Praia em pixel-art: painéis de areia, barras de madeira, mar turquesa e sons chiptune."
    else:
        retro_art(out)
        toml, title = RETRO_TOML, "Retro"
        names = ["PressStart2P-Regular.ttf", "PressStart2P-OFL.txt"]
        font_note = ("Press Start 2P (unmodified)", "")
        en = "8-bit windows in the Sweetie 16 palette (CC0), with a blinking cursor and chiptune sounds."
        pt = "Janelas 8 bits na paleta Sweetie 16 (CC0), com cursor piscando e sons chiptune."
    (out / "theme.toml").write_text(toml, encoding="utf-8")
    for name in names:
        (out / "fonts").mkdir(exist_ok=True)
        shutil.copy(fonts / name, out / "fonts" / name)
    for name in CONSOLES:
        ink = B["outline"] if theme == "praia" else (0, 0, 0, 255)
        save(P.console(name, ink), out, f"systems/{name}.png")
    sons.main(theme, out / "sounds")
    (out / "README.md").write_text(
        README.format(title=title, en=en, pt=pt, fonts=font_note[0], note=font_note[1]), encoding="utf-8")
    (out / "LICENSE").write_text(LICENSE_HEAD.format(title=title) + cc.read_text(encoding="utf-8"),
                                 encoding="utf-8")


if __name__ == "__main__":
    build(sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3]), Path(sys.argv[4]))
