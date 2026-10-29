#!/usr/bin/env python3
"""Original pixel art for the Retro and Praia themes, drawn in code.

Everything is drawn at 1x on small RGBA images and grown by whole multiples
with nearest neighbour (`grow`). Nothing is traced or copied: consoles are
generic shapes in plain colours, without any real logo.

Needs Pillow. Used by the theme generators and the mock-ups.
"""
import math
import random

from PIL import Image, ImageDraw

CLEAR = (0, 0, 0, 0)


def rgb(hex_color, alpha=255):
    hex_color = hex_color.lstrip("#")
    if len(hex_color) == 8:
        alpha = int(hex_color[6:], 16)
    return tuple(int(hex_color[i:i + 2], 16) for i in (0, 2, 4)) + (alpha,)


# Sweetie 16 (CC0, GrafxKid): the Retro palette.
SWEETIE = {name: rgb(value) for name, value in {
    "black": "1a1c2c", "plum": "5d275d", "red": "b13e53", "orange": "ef7d57",
    "yellow": "ffcd75", "lime": "a7f070", "green": "38b764", "teal": "257179",
    "navy": "29366f", "blue": "3b5dc9", "sky": "41a6f6", "cyan": "73eff7",
    "white": "f4f4f4", "silver": "94b0c2", "slate": "566c86", "ink": "333c57",
}.items()}

# Praia: sand, sea and sunset, chosen for this theme.
BEACH = {name: rgb(value) for name, value in {
    "sand_hi": "ffe9c4", "sand_lt": "fbd7a3", "sand": "f4c28a", "sand_md": "e8a666",
    "sand_dk": "cf8448", "wood_hi": "c98a52", "wood": "a8683a", "wood_md": "8a512c",
    "brown": "6b3b20", "outline": "3f2214", "straw_hi": "ffd88a", "straw": "f0ac4c",
    "straw_md": "d9862f", "straw_dk": "a85c22", "foam": "ffffff", "foam_sh": "c9f4f2",
    "sea_hi": "8ff3ea", "sea_lt": "45d6dc", "sea": "1fb2c8", "sea_md": "1690b3",
    "sea_dk": "11709a", "deep": "0b4f78", "abyss": "083a5e", "coral": "ff6f59",
    "coral_dk": "c2413a", "sun": "ffd35a", "sun_dk": "e59b2f", "palm": "6fc94a",
    "palm_md": "3f9d3a", "palm_dk": "23683a", "pink": "f7a8a0", "pink_dk": "d26c6c",
    "purple": "7a5cc4", "purple_dk": "4d3a8c", "white": "fbfbf6", "night": "1b2a3c",
    "shadow": "00000040",
}.items()}


def canvas(w, h, fill=CLEAR):
    return Image.new("RGBA", (w, h), fill)


def grow(image, k):
    return image.resize((image.width * k, image.height * k), Image.NEAREST)


def rect(img, x, y, w, h, c):
    if w > 0 and h > 0:
        ImageDraw.Draw(img).rectangle([x, y, x + w - 1, y + h - 1], fill=c)


def px(img, x, y, c):
    if 0 <= x < img.width and 0 <= y < img.height:
        img.putpixel((x, y), c)


def disc(img, cx, cy, r, c):
    """Pixel disc centred on (cx, cy) (half-integers allowed)."""
    for y in range(math.floor(cy - r), math.ceil(cy + r) + 1):
        for x in range(math.floor(cx - r), math.ceil(cx + r) + 1):
            if (x + 0.5 - cx) ** 2 + (y + 0.5 - cy) ** 2 <= r * r:
                px(img, x, y, c)


def chunky(img, x, y, w, h, c, cut=1):
    """Rectangle with `cut` pixel steps taken off each corner."""
    for row in range(h):
        inset = max(0, cut - row, cut - (h - 1 - row))
        rect(img, x + inset, y + row, w - 2 * inset, 1, c)


def outline(img, c, diagonal=False):
    """A one pixel border around everything opaque."""
    out = img.copy()
    src = img.load()
    near = [(1, 0), (-1, 0), (0, 1), (0, -1)]
    if diagonal:
        near += [(1, 1), (1, -1), (-1, 1), (-1, -1)]
    for y in range(img.height):
        for x in range(img.width):
            if src[x, y][3]:
                continue
            if any(0 <= x + dx < img.width and 0 <= y + dy < img.height and src[x + dx, y + dy][3]
                   for dx, dy in near):
                out.putpixel((x, y), c)
    return out


def padded(img, pad=1):
    out = canvas(img.width + 2 * pad, img.height + 2 * pad)
    out.alpha_composite(img, (pad, pad))
    return out


def paste(dst, src, x, y):
    dst.alpha_composite(src, (int(x), int(y)))


def text_mask(width, height):
    return canvas(width, height)


# ---------------------------------------------------------------- textures

def sand(w, h, seed=1, p=BEACH):
    """Sand with a sparse speckle: no clumps, never two dark grains touching."""
    rnd = random.Random(seed)
    img = canvas(w, h, p["sand"])
    for y in range(h):
        for x in range(w):
            r = rnd.random()
            if r < 0.05:
                px(img, x, y, p["sand_md"])
            elif r < 0.07:
                px(img, x, y, p["sand_lt"])
            elif r < 0.085 and x % 2 == y % 2:
                px(img, x, y, p["sand_dk"])
    return img


def ripples(img, seed=2, p=BEACH, every=150):
    """Wind ripple marks on sand: short darker arcs with a lit edge."""
    rnd = random.Random(seed)
    for _ in range(img.width * img.height // every):
        x, y = rnd.randrange(2, img.width - 6), rnd.randrange(2, img.height - 3)
        if img.getpixel((x, y))[3] == 0 or img.getpixel((x + 4, y))[3] == 0:
            continue
        for dx, dy in [(0, 0), (1, 1), (2, 1), (3, 1), (4, 0)]:
            px(img, x + dx, y + dy, p["sand_md"])
        for dx in (1, 2, 3):
            px(img, x + dx, y + 2, p["sand_lt"])
    return img


def caustics(w, h, p=BEACH, cell=14, seed=5):
    """Shallow water: a turquoise floor laced with bright caustic lines."""
    rnd = random.Random(seed)
    points = [(rnd.uniform(0, w), rnd.uniform(0, h)) for _ in range(max(4, w * h // (cell * cell)))]
    img = canvas(w, h)
    for y in range(h):
        depth = y / max(1, h - 1)
        base = p["sea_lt"] if depth < 0.35 else p["sea"] if depth < 0.7 else p["sea_md"]
        for x in range(w):
            d = sorted((x - a) ** 2 + (y - b) ** 2 for a, b in points)[:2]
            edge = math.sqrt(d[1]) - math.sqrt(d[0])
            c = base
            if edge < 1.0:
                c = p["sea_hi"]
            elif edge < 1.8 and (x + y) % 2:
                c = p["sea_hi"] if depth < 0.5 else p["sea_lt"]
            px(img, x, y, c)
    return img


def wood(w, h, seed=4, p=BEACH, plank=7, vertical=False):
    """Deck planks: lit top edge, dark seam, grain streaks and nails."""
    if vertical:
        return wood(h, w, seed, p, plank).transpose(Image.Transpose.ROTATE_90)
    rnd = random.Random(seed)
    img = canvas(w, h, p["wood"])
    for top in range(0, h, plank):
        rows = min(plank, h - top)
        shift = rnd.randrange(0, 40)
        for y in range(top, top + rows):
            r = y - top
            for x in range(w):
                c = p["wood"]
                if r == 0:
                    c = p["wood_hi"]
                elif r == rows - 1:
                    c = p["brown"]
                px(img, x, y, c)
        for _ in range(w // 6):
            x, y = rnd.randrange(w), top + rnd.randrange(1, max(2, rows - 1))
            for dx in range(rnd.randint(2, 6)):
                px(img, x + dx, y, p["wood_md"])
        for x in range(shift % 48, w, 48):
            rect(img, x, top, 1, rows, p["outline"])
            for nx in (x - 2, x + 2):
                px(img, nx, top + rows // 2 - 1, p["outline"])
    return img


def thatch(w, h, seed=6, p=BEACH):
    """A straw roof edge: layered strands ending in a ragged fringe."""
    rnd = random.Random(seed)
    img = canvas(w, h)
    for x in range(w):
        length = h - rnd.randint(0, 4)
        for y in range(length):
            band = y * 4 // h
            c = [p["straw_hi"], p["straw"], p["straw"], p["straw_md"]][band]
            if (x + y // 3) % 5 == 0:
                c = p["straw_md"] if band < 3 else p["straw_dk"]
            px(img, x, y, c)
        px(img, x, length - 1, p["straw_dk"])
    for y in range(3, h - 4, 5):
        for x in range(w):
            if (x + y) % 4 == 0:
                px(img, x, y, p["straw_md"])
    return outline(padded(img), p["outline"])


def drop_shadow(sprite, alpha=70):
    """The sprite's silhouette, for a flat shadow on the ground."""
    shade = canvas(sprite.width, sprite.height)
    mask = sprite.getchannel("A").point(lambda a: alpha if a else 0)
    shade.putalpha(mask)
    return shade


def sea(w, h, p=BEACH, rays=True):
    """Deep water in dithered bands, light rays and a few sparkles."""
    bands = [p["sea"], p["sea_md"], p["sea_dk"], p["deep"], p["abyss"]]
    img = canvas(w, h, bands[0])
    step = h / len(bands)
    for y in range(h):
        band = min(int(y / step), len(bands) - 1)
        into = y - band * step
        for x in range(w):
            c = bands[band]
            # Two dithered rows where one band meets the next.
            if band + 1 < len(bands) and into > step - 3 and (x + y) % 2 == 0:
                c = bands[band + 1]
            px(img, x, y, c)
    if rays:
        for start in range(-h, w, 64):
            for y in range(h):
                for x in range(start + y // 2, start + y // 2 + 10):
                    if 0 <= x < w and (x + y) % 3:
                        r, g, b, a = img.getpixel((x, y))
                        px(img, x, y, (min(r + 14, 255), min(g + 22, 255), min(b + 18, 255), a))
    rnd = random.Random(7)
    for _ in range(w * h // 900):
        x, y = rnd.randrange(w), rnd.randrange(h // 2)
        for dx, dy in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)]:
            px(img, x + dx, y + dy, p["sea_hi"] if (dx or dy) else p["foam"])
    return img


def foam(w, h, seed=3, p=BEACH):
    """A shore line: foam on top, shallow and then deeper water below."""
    rnd = random.Random(seed)
    img = canvas(w, h)
    crest = [0] * w
    level = 1
    for x in range(w):
        if rnd.random() < 0.25:
            level = max(0, min(3, level + rnd.choice([-1, 1])))
        crest[x] = level
    for x in range(w):
        top = crest[x]
        for y in range(top, h):
            depth = y - top
            c = (p["foam"] if depth < 2 else p["foam_sh"] if depth < 3 else
                 p["sea_hi"] if depth < 5 else p["sea_lt"] if depth < 8 else p["sea"])
            px(img, x, y, c)
        if rnd.random() < 0.12:
            px(img, x, top + 4, p["foam"])
    return img


# ------------------------------------------------------------------ frames

def ragged(w, h, seed, p=BEACH, inset=True):
    """A sand slab with an uneven edge, dark outline, light top-left and a
    darker lower lip; `inset` carves a recessed well inside."""
    rnd = random.Random(seed)
    body = sand(w, h, seed, p)
    mask = canvas(w, h)
    rect(mask, 0, 0, w, h, (255, 255, 255, 255))
    # Wobble: runs of 0 or 1 pixel bites along each edge, round corners.
    for edge in range(4):
        length = w if edge % 2 == 0 else h
        bite, run = 0, 0
        for i in range(length):
            if run == 0:
                bite, run = rnd.choice([0, 0, 1, 1, 2]), rnd.randint(3, 9)
            run -= 1
            for d in range(bite):
                x, y = [(i, d), (w - 1 - d, i), (i, h - 1 - d), (d, i)][edge]
                px(mask, x, y, CLEAR)
    for cx, cy in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)]:
        for d in range(4):
            for e in range(4 - d):
                px(mask, abs(cx - d), abs(cy - e), CLEAR)
    shape = canvas(w, h)
    shape.paste(body, (0, 0), mask)
    src = shape.load()
    # Light on the top-left inner rim, shade on the bottom-right.
    for y in range(h):
        for x in range(w):
            if not src[x, y][3]:
                continue
            up = y == 0 or not src[x, y - 1][3]
            left = x == 0 or not src[x - 1, y][3]
            down2 = y + 2 >= h or not src[x, y + 2][3]
            right = x + 1 >= w or not src[x + 1, y][3]
            if up or left:
                px(shape, x, y, p["sand_hi"])
            elif down2:
                px(shape, x, y, p["sand_dk"])
            elif right:
                px(shape, x, y, p["sand_md"])
    if inset and w > 16 and h > 16:
        well(shape, 5, 5, w - 10, h - 12, seed + 1, p)
    return outline(padded(shape), p["outline"])


def well(img, x, y, w, h, seed, p=BEACH):
    """A recessed sand area: shaded top-left lip, lit bottom-right lip."""
    inner = sand(w, h, seed, p)
    rect(inner, 0, 0, w, 1, p["sand_dk"])
    rect(inner, 0, 0, 1, h, p["sand_dk"])
    rect(inner, 0, 1, w, 1, p["sand_md"])
    rect(inner, 1, 0, 1, h, p["sand_md"])
    rect(inner, 0, h - 1, w, 1, p["sand_hi"])
    rect(inner, w - 1, 0, 1, h, p["sand_hi"])
    for c in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)]:
        px(inner, *c, CLEAR)
    paste(img, inner, x, y)


def button(w, h, p=BEACH, selected=False, seed=11):
    """A raised sand button; the selected one is a turquoise wave crest."""
    img = canvas(w, h)
    if selected:
        chunky(img, 0, 0, w, h, p["sea_lt"], 2)
        rect(img, 2, 1, w - 4, 1, p["sea_hi"])
        rect(img, 1, 2, w - 2, 1, p["sea_hi"])
        rect(img, 2, h - 3, w - 4, 1, p["sea"])
        rect(img, 2, h - 2, w - 4, 1, p["sea_md"])
        rnd = random.Random(seed)
        for _ in range(w // 9):
            x, y = rnd.randrange(3, w - 3), rnd.randrange(3, h - 4)
            px(img, x, y, p["sea_hi"])
        framed = outline(padded(img), p["foam"])
        return outline(padded(framed), p["deep"])
    body = sand(w, h, seed, p)
    mask = canvas(w, h)
    chunky(mask, 0, 0, w, h, (255, 255, 255, 255), 2)
    img.paste(body, (0, 0), mask)
    rect(img, 2, 0, w - 4, 1, p["sand_hi"])
    rect(img, 1, 1, w - 2, 1, p["sand_lt"])
    rect(img, 2, h - 2, w - 4, 1, p["sand_md"])
    rect(img, 2, h - 1, w - 4, 1, p["sand_dk"])
    return outline(padded(img), p["outline"])


def window(w, h, title_h, p=SWEETIE, body="navy", bar="white", border=2, stripes=0.55):
    """Retro window: thick light border, a title bar with grip stripes on
    its right part and a close box, and a solid black drop shadow."""
    img = canvas(w + 3, h + 3)
    rect(img, 3, 3, w, h, (0, 0, 0, 255))
    rect(img, 0, 0, w, h, p[bar])
    if title_h:
        for y in range(border + 2, border + title_h - 2, 2):
            rect(img, int(w * stripes), y, w - int(w * stripes) - 14, 1, p["silver"])
        rect(img, w - 11, border + 1, 8, title_h - 2, p["ink"])
        rect(img, w - 10, border + 2, 6, title_h - 4, p[bar])
    rect(img, border, border + title_h, w - 2 * border, h - 2 * border - title_h, p[body])
    # Bevel: one lighter row under the title bar, one darker row at the bottom.
    rect(img, border, border + title_h, w - 2 * border, 1, p["blue"])
    rect(img, border, h - border - 1, w - 2 * border, 1, p["ink"])
    return img


def night(w, h, p=SWEETIE, seed=3):
    """Retro backdrop: black sky dithered into navy at the bottom, a dot grid
    and a sprinkle of stars."""
    img = canvas(w, h, p["black"])
    for y in range(h // 2, h):
        t = (y - h // 2) / (h // 2)
        for x in range(w):
            if (x * 7 + y * 3) % 16 < int(t * 10):
                px(img, x, y, p["ink"])
    for y in range(0, h, 8):
        for x in range(0, w, 8):
            px(img, x, y, p["ink"])
    rnd = random.Random(seed)
    for _ in range(w * h // 1500):
        x, y = rnd.randrange(w), rnd.randrange(h * 2 // 3)
        px(img, x, y, p["silver"] if rnd.random() < 0.3 else p["slate"])
        if rnd.random() < 0.15:
            for dx, dy in [(1, 0), (-1, 0), (0, 1), (0, -1)]:
                px(img, x + dx, y + dy, p["slate"])
            px(img, x, y, p["white"])
    return img


# ----------------------------------------------------------------- decor

def palm(p=BEACH):
    img = canvas(30, 34)
    for i in range(18):
        x = 15 + round(3 * math.sin(i / 7))
        y = 33 - i
        c = p["wood"] if (i // 2) % 2 else p["brown"]
        rect(img, x, y, 3, 1, c)
        px(img, x, y, p["brown"])
    top = (16 + round(3 * math.sin(17 / 7)), 15)
    fronds = [(-1.0, 1.6), (-0.55, 1.2), (-0.1, 0.9), (0.35, 1.2), (0.8, 1.6), (1.15, 2.0), (-1.35, 2.0)]
    for angle, droop in fronds:
        for t in range(11):
            x = top[0] + math.sin(angle) * t * 1.15
            y = top[1] - math.cos(angle) * t * 0.9 + droop * (t / 6) ** 2
            px(img, round(x), round(y), p["palm_md"])
            px(img, round(x), round(y) - 1, p["palm"])
            if t % 3 == 1:
                px(img, round(x), round(y) + 1, p["palm_dk"])
    for dx in (-1, 2):
        disc(img, top[0] + dx + 0.5, top[1] + 2.5, 1.2, p["brown"])
    return outline(img, p["outline"])


def crab(p=BEACH):
    img = canvas(18, 12)
    chunky(img, 4, 5, 10, 5, p["coral"], 2)
    rect(img, 6, 5, 6, 1, p["pink"])
    for x in (6, 11):
        rect(img, x, 2, 1, 3, p["coral_dk"])
        px(img, x, 1, p["white"])
        px(img, x, 2, p["outline"])
    for side, x in ((-1, 1), (1, 14)):
        disc(img, x + 1.5, 4.5, 1.6, p["coral"])
        px(img, x + (2 if side < 0 else 1), 3, p["pink"])
        for leg in range(3):
            px(img, 4 + (side > 0) * 9 + side * 1 + leg * side, 10 + (leg % 2), p["coral_dk"])
    return outline(img, p["outline"])


def shell(p=BEACH):
    img = canvas(11, 10)
    for y in range(7):
        half = round(math.sqrt(max(0, 25 - (6 - y) ** 2 * 0.7)))
        rect(img, 5 - half, y + 1, 2 * half + 1, 1, p["pink"])
    for x in range(1, 10, 2):
        for y in range(2, 8):
            if img.getpixel((x, y))[3]:
                px(img, x, y, p["pink_dk"])
    rect(img, 4, 8, 3, 2, p["pink"])
    px(img, 5, 1, p["white"])
    return outline(img, p["outline"])


def starfish(p=BEACH):
    img = canvas(13, 13)
    points = []
    for k in range(10):
        r = 6.2 if k % 2 == 0 else 2.6
        a = -math.pi / 2 + k * math.pi / 5
        points.append((6 + r * math.cos(a), 6.4 + r * math.sin(a)))
    ImageDraw.Draw(img).polygon(points, fill=p["sun_dk"])
    for k in range(0, 10, 2):
        a = -math.pi / 2 + k * math.pi / 5
        px(img, round(6 + 2.5 * math.cos(a)), round(6.4 + 2.5 * math.sin(a)), p["sun"])
    px(img, 6, 6, p["sun"])
    return outline(img, p["outline"])


def ball(p=BEACH):
    img = canvas(12, 12)
    colours = [p["coral"], p["white"], p["sea"], p["white"], p["sun"], p["white"]]
    for y in range(12):
        for x in range(12):
            dx, dy = x + 0.5 - 6, y + 0.5 - 6
            if dx * dx + dy * dy <= 30:
                sector = int(((math.atan2(dy, dx) + math.pi) / (2 * math.pi)) * 6) % 6
                px(img, x, y, colours[sector])
    disc(img, 6, 6, 1.2, p["white"])
    px(img, 3, 3, p["white"])
    px(img, 4, 3, p["white"])
    return outline(img, p["outline"])


def surfboard(p=BEACH):
    img = canvas(8, 24)
    for y in range(24):
        half = 3 * math.sin(math.pi * (y + 0.5) / 24) ** 0.6
        rect(img, round(4 - half), y, max(1, round(2 * half)), 1, p["sun"])
    rect(img, 3, 2, 2, 20, p["coral"])
    rect(img, 3, 6, 1, 12, p["coral_dk"])
    return outline(img, p["outline"])


def bucket(p=BEACH):
    img = canvas(14, 14)
    for y in range(8):
        rect(img, 2 + y // 4, 5 + y, 10 - 2 * (y // 4), 1, p["sea_lt"] if y else p["sea_hi"])
    rect(img, 2, 7, 10, 2, p["sun"])
    for t in range(9):
        a = math.pi * t / 8
        px(img, round(7 - 5 * math.cos(a)), round(5 - 3 * math.sin(a)), p["outline"])
    rect(img, 11, 1, 1, 7, p["wood"])
    rect(img, 10, 0, 3, 2, p["coral"])
    return outline(img, p["outline"])


DECOR = {"palm": palm, "crab": crab, "shell": shell, "starfish": starfish,
         "ball": ball, "surfboard": surfboard, "bucket": bucket}


# --------------------------------------------------------------- consoles

def _console(draw_fn, w=58, h=42, ink=(26, 28, 44, 255)):
    img = canvas(w, h)
    draw_fn(img)
    return outline(padded(img), ink)


GREY = {"hi": rgb("e8e8e4"), "lt": rgb("c9c9c4"), "md": rgb("9a9a98"), "dk": rgb("5c5c62"),
        "ink": rgb("2a2a33"), "red": rgb("d83a3a"), "dkred": rgb("8f2128"),
        "purple": rgb("6a4fb8"), "lilac": rgb("a99ad8"), "green": rgb("8fbf3a"),
        "lcd": rgb("9bbc0f"), "lcd_dk": rgb("306230"), "lcd_md": rgb("8bac0f"),
        "blue": rgb("3b6fd8"), "yellow": rgb("f2c230"), "teal": rgb("2aa198"),
        "black": rgb("26262c"), "coal": rgb("3a3a42"), "white": rgb("f6f6f2"),
        "pink": rgb("ff77a8"), "orange": rgb("ff9f3a"), "sky": rgb("6ec7ff"),
        "brown": rgb("8a5a35"), "wood": rgb("b07a48"), "moon": rgb("f5e9a8")}
G = GREY


def pad_controller(img, x, y, body, buttons, w=20, h=9):
    chunky(img, x, y, w, h, body, 2)
    rect(img, x + 3, y + 4, 5, 1, G["ink"])
    rect(img, x + 5, y + 2, 1, 5, G["ink"])
    for i, c in enumerate(buttons):
        px(img, x + w - 4 - (i % 2) * 3, y + 3 + (i // 2) * 2, c)
        px(img, x + w - 3 - (i % 2) * 3, y + 3 + (i // 2) * 2, c)


def nes(img):
    rect(img, 4, 8, 50, 22, G["lt"])
    rect(img, 4, 8, 50, 2, G["hi"])
    rect(img, 4, 20, 50, 10, G["dk"])
    rect(img, 8, 11, 30, 6, G["md"])
    rect(img, 10, 13, 26, 2, G["dk"])
    for i in range(4):
        rect(img, 42, 11 + i * 2, 9, 1, G["dk"])
    rect(img, 7, 23, 4, 4, G["ink"])
    rect(img, 13, 23, 4, 4, G["ink"])
    rect(img, 40, 24, 5, 2, G["red"])
    rect(img, 47, 24, 5, 2, G["md"])
    pad_controller(img, 30, 32, G["lt"], [G["red"], G["red"]])


def snes(img):
    chunky(img, 6, 10, 46, 18, G["lt"], 3)
    rect(img, 8, 10, 42, 2, G["hi"])
    chunky(img, 16, 6, 26, 6, G["md"], 1)
    rect(img, 18, 7, 22, 2, G["dk"])
    rect(img, 10, 18, 7, 3, G["purple"])
    rect(img, 41, 18, 7, 3, G["lilac"])
    rect(img, 20, 22, 18, 3, G["md"])
    pad_controller(img, 4, 31, G["lt"], [G["purple"], G["lilac"], G["lilac"], G["purple"]], 24, 9)


def handheld(img, body, screen, x=17, w=24, h=38):
    chunky(img, x, 2, w, h, body, 2)
    rect(img, x + 1, 2, w - 2, 1, G["hi"])
    rect(img, x + 3, 5, w - 6, 15, G["coal"])
    rect(img, x + 5, 7, w - 10, 11, screen)
    rect(img, x + 4, 26, 5, 1, G["ink"])
    rect(img, x + 6, 24, 1, 5, G["ink"])
    rect(img, x + w - 8, 26, 2, 2, G["red"])
    rect(img, x + w - 5, 24, 2, 2, G["red"])
    for i in range(3):
        rect(img, x + w - 7 + i * 2, 33, 1, 4, G["md"])


def gb(img):
    handheld(img, G["lt"], G["lcd"])
    rect(img, 24, 10, 10, 2, G["lcd_dk"])
    rect(img, 26, 13, 6, 3, G["lcd_md"])


def gbc(img):
    handheld(img, G["purple"], G["sky"])
    rect(img, 22, 13, 14, 4, G["green"])
    disc(img, 31, 10.5, 1.5, G["yellow"])


def gba(img):
    chunky(img, 3, 10, 52, 24, G["purple"], 5)
    rect(img, 8, 10, 42, 1, G["lilac"])
    rect(img, 17, 13, 24, 17, G["coal"])
    rect(img, 19, 15, 20, 13, G["sky"])
    rect(img, 19, 24, 20, 4, G["green"])
    disc(img, 33.5, 18.5, 1.5, G["yellow"])
    rect(img, 7, 20, 5, 1, G["ink"])
    rect(img, 9, 18, 1, 5, G["ink"])
    rect(img, 46, 18, 2, 2, G["lilac"])
    rect(img, 49, 21, 2, 2, G["lilac"])


def genesis(img):
    chunky(img, 4, 10, 50, 18, G["black"], 3)
    rect(img, 6, 10, 46, 1, G["coal"])
    disc(img, 17, 18, 6, G["coal"])
    disc(img, 17, 18, 4, G["black"])
    disc(img, 17, 18, 2, G["coal"])
    rect(img, 28, 13, 20, 4, G["coal"])
    rect(img, 30, 14, 16, 2, G["ink"])
    rect(img, 28, 22, 22, 2, G["red"])
    pad_controller(img, 30, 31, G["black"], [G["coal"], G["coal"], G["red"], G["coal"]], 22, 9)


def n64(img):
    chunky(img, 6, 12, 46, 16, G["coal"], 4)
    rect(img, 8, 12, 42, 1, G["md"])
    rect(img, 20, 8, 18, 6, G["dk"])
    rect(img, 22, 9, 14, 2, G["ink"])
    for i, c in enumerate([G["red"], G["yellow"], G["green"], G["blue"]]):
        rect(img, 12 + i * 10, 23, 5, 2, c)
    # A generic three-grip pad.
    rect(img, 18, 31, 22, 5, G["md"])
    rect(img, 18, 35, 4, 5, G["md"])
    rect(img, 27, 35, 4, 5, G["md"])
    rect(img, 36, 35, 4, 5, G["md"])
    px(img, 34, 32, G["yellow"])
    px(img, 36, 33, G["blue"])
    px(img, 22, 33, G["ink"])


def psx(img):
    chunky(img, 6, 8, 46, 22, G["lt"], 3)
    rect(img, 8, 8, 42, 1, G["hi"])
    disc(img, 22, 18, 8, G["md"])
    disc(img, 22, 18, 6.5, G["lt"])
    disc(img, 22, 18, 2, G["md"])
    rect(img, 36, 12, 4, 3, G["md"])
    rect(img, 43, 12, 4, 3, G["md"])
    rect(img, 36, 24, 12, 2, G["dk"])
    pad_controller(img, 30, 32, G["lt"], [G["teal"], G["red"], G["pink"], G["blue"]], 22, 9)


def pico8(img):
    chunky(img, 12, 4, 34, 34, G["black"], 3)
    rect(img, 15, 7, 28, 22, G["ink"])
    for i, c in enumerate([G["red"], G["orange"], G["yellow"], G["green"], G["sky"], G["pink"]]):
        rect(img, 17 + i * 4, 9, 3, 3, c)
    heart = ["0110110", "1111111", "1111111", "0111110", "0011100", "0001000"]
    for y, row in enumerate(heart):
        for x, bit in enumerate(row):
            if bit == "1":
                px(img, 25 + x, 15 + y, G["pink"])
    rect(img, 15, 31, 28, 4, G["coal"])
    rect(img, 17, 32, 6, 2, G["red"])


def scummvm(img):
    rect(img, 6, 4, 46, 30, G["coal"])
    rect(img, 8, 6, 42, 22, G["sky"])
    rect(img, 8, 22, 42, 6, G["green"])
    rect(img, 30, 11, 10, 15, G["brown"])
    rect(img, 32, 13, 6, 13, G["wood"])
    px(img, 36, 19, G["yellow"])
    disc(img, 15, 11, 3, G["yellow"])
    arrow = ["1", "11", "121", "1221", "12221", "122221", "1221", "1001"]
    for y, row in enumerate(arrow):
        for x, bit in enumerate(row):
            if bit != "0":
                px(img, 20 + x, 18 + y, G["ink"] if bit == "1" else G["white"])
    for i in range(5):
        rect(img, 8 + i * 9, 30, 7, 3, G["md"])


def tools(img):
    for t in range(22):
        rect(img, 14 + t, 34 - t, 3, 3, G["md"])
    disc(img, 38, 10, 6, G["md"])
    disc(img, 39, 9, 3, CLEAR)
    rect(img, 39, 4, 6, 4, CLEAR)
    for t in range(18):
        rect(img, 40 - t, 33 - t, 2, 2, G["yellow"])
    rect(img, 38, 31, 6, 6, G["red"])
    disc(img, 15, 13, 7, G["dk"])
    disc(img, 15, 13, 3, G["lt"])
    for a in range(8):
        x = 15 + 8 * math.cos(a * math.pi / 4)
        y = 13 + 8 * math.sin(a * math.pi / 4)
        rect(img, round(x) - 1, round(y) - 1, 2, 2, G["dk"])


def ports(img):
    chunky(img, 8, 26, 42, 12, G["black"], 2)
    rect(img, 10, 26, 38, 1, G["coal"])
    rect(img, 27, 12, 4, 15, G["md"])
    disc(img, 29, 9, 6, G["red"])
    disc(img, 27.5, 7.5, 2, G["white"])
    rect(img, 37, 30, 4, 4, G["yellow"])
    rect(img, 43, 30, 4, 4, G["sky"])
    rect(img, 13, 31, 8, 2, G["coal"])


def moonlight(img):
    rect(img, 8, 10, 40, 26, G["coal"])
    rect(img, 10, 12, 36, 20, G["ink"])
    rect(img, 24, 36, 8, 3, G["md"])
    rect(img, 18, 39, 20, 2, G["md"])
    disc(img, 22, 21, 7, G["moon"])
    disc(img, 25.5, 18.5, 6, G["ink"])
    for x, y in [(36, 15), (40, 22), (33, 27), (14, 14)]:
        px(img, x, y, G["white"])
    for i in range(3):
        rect(img, 40 + i * 4, 6 - i * 2, 2, 2 + i * 2, G["sky"])


def music(img):
    for x in (16, 34):
        rect(img, x, 8, 3, 22, G["ink"])
    rect(img, 16, 6, 21, 4, G["ink"])
    disc(img, 13, 31, 5, G["pink"])
    disc(img, 31, 31, 5, G["pink"])
    disc(img, 11.5, 29.5, 1.5, G["white"])
    disc(img, 29.5, 29.5, 1.5, G["white"])
    rect(img, 42, 18, 12, 18, G["coal"])
    disc(img, 48, 23, 3, G["md"])
    disc(img, 48, 31, 4, G["md"])
    disc(img, 48, 31, 2, G["ink"])


CONSOLES = {"nes": nes, "snes": snes, "gb": gb, "gbc": gbc, "gba": gba, "genesis": genesis,
            "n64": n64, "psx": psx, "pico-8": pico8, "scummvm": scummvm, "tools": tools,
            "ports": ports, "moonlight": moonlight, "music": music}


def console(name, ink=(26, 28, 44, 255)):
    return _console(CONSOLES[name], ink=ink)
