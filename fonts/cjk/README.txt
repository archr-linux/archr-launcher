Arch R UI SC — interface-only Simplified Chinese fallback

Source: Noto Sans CJK SC Regular, release Sans2.004 (version 2.004).
Copyright: © 2014-2021 Adobe (http://www.adobe.com/).
License: SIL Open Font License 1.1; see OFL.txt.

Upstream font:
https://raw.githubusercontent.com/notofonts/noto-cjk/Sans2.004/Sans/OTF/SimplifiedChinese/NotoSansCJKsc-Regular.otf
Upstream license:
https://raw.githubusercontent.com/notofonts/noto-cjk/Sans2.004/LICENSE
Upstream font SHA-256:
2c76254f6fc379fddfce0a7e84fb5385bb135d3e399294f6eeb6680d0365b74b

The bundled derivative is renamed Arch R UI SC. It contains the non-ASCII
characters used in locales/zh_CN.toml, their uppercase forms, and the language
autonym. pending-glyphs.txt reserves six additional glyphs for the image and
save-sync translations from f691c97 that will be merged later; it is a tracked
build input, not a runtime catalog. ASCII/Latin/Cyrillic remain in the existing
theme fonts. The original
font's copyright and the full OFL text are retained in the subset's metadata,
so they also travel inside the executables that embed it.

Rebuild after changing Chinese translations:
  python -I tools/subset-cjk.py /path/to/NotoSansCJKsc-Regular.otf
Requires Python 3.11+ and fonttools 4.61.1. Download the upstream font into a
separate directory first; the tool validates its SHA-256 and uses no network.
The catalog-glyph coverage test fails when a new character needs subsetting.

The fallback is lazy and selected per missing glyph, not per interface locale.
Existing font metrics and shaping remain unchanged for covered text, including
Latin runs inside a Chinese label. It is embedded once in each executable;
there is no dependency on host fonts or a theme's font directory.

This is intentionally NOT a complete Chinese font. Arbitrary Chinese game
names, filenames, scraped descriptions, module text, or user input may contain
characters outside the subset. Full dynamic-content coverage would require a
larger font. The shared on-screen keyboard remains Latin/symbol input (no
Pinyin/IME); existing Unicode values can be displayed, submitted and deleted
at character boundaries.
