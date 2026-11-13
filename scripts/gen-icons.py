"""Generate Material-style app icon options and the production icon set.

Run: python scripts/gen-icons.py
Requires Pillow.
"""

from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
ICONS = ROOT / "src-tauri" / "icons"
OPTIONS = ROOT / "out" / "icons"
ICONS.mkdir(parents=True, exist_ok=True)
OPTIONS.mkdir(parents=True, exist_ok=True)

DARK = (20, 20, 20, 255)
CREAM = (245, 241, 232, 255)
WHITE = (237, 237, 237, 255)
RED = (239, 83, 80, 255)
GREEN = (102, 187, 106, 255)
BLACK = (10, 10, 10, 255)

SCALE = 4


def canvas(size: int):
    box = size * SCALE
    img = Image.new("RGBA", (box, box), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    radius = int(box * 0.23)
    draw.rounded_rectangle((0, 0, box - 1, box - 1), radius=radius, fill=DARK)
    return img, draw, box


def finish(img: Image.Image, size: int) -> Image.Image:
    return img.resize((size, size), Image.LANCZOS)


def triangle(draw, cx, cy, r, fill):
    draw.polygon(
        [(cx - r * 0.62, cy - r), (cx - r * 0.62, cy + r), (cx + r, cy)],
        fill=fill,
    )


def option_flip(size):
    img, draw, s = canvas(size)
    card_w = int(s * 0.30)
    card_h = int(s * 0.43)
    gap = int(s * 0.06)
    left = (s - card_w * 2 - gap) // 2
    top = int(s * 0.285)
    radius = int(s * 0.075)
    for index in range(2):
        x = left + index * (card_w + gap)
        draw.rounded_rectangle(
            (x, top, x + card_w, top + card_h), radius=radius, fill=CREAM
        )
    # flip hinge
    draw.rounded_rectangle(
        (left, int(s * 0.485), left + card_w * 2 + gap, int(s * 0.507)),
        radius=int(s * 0.011),
        fill=RED,
    )
    return finish(img, size)


def option_timer_play(size):
    img, draw, s = canvas(size)
    cx = cy = s // 2
    ring = int(s * 0.075)
    pad = int(s * 0.23)
    draw.ellipse(
        (pad, pad, s - pad, s - pad), outline=CREAM, width=ring
    )
    triangle(draw, cx + int(s * 0.03), cy, int(s * 0.12), RED)
    # 12 o'clock marker
    draw.ellipse(
        (cx - int(s * 0.035), pad - int(s * 0.035),
         cx + int(s * 0.035), pad + int(s * 0.035)),
        fill=RED,
    )
    return finish(img, size)


def option_tomato(size):
    img, draw, s = canvas(size)
    cx = cy = int(s * 0.54)
    r = int(s * 0.27)
    draw.ellipse((cx - r, cy - r, cx + r, cy + r), fill=RED)
    # three simple leaves above the fruit
    top = cy - r
    draw.ellipse(
        (cx - int(s * 0.035), top - int(s * 0.09),
         cx + int(s * 0.035), top + int(s * 0.03)),
        fill=GREEN,
    )
    draw.ellipse(
        (cx - int(s * 0.13), top - int(s * 0.02),
         cx - int(s * 0.01), top + int(s * 0.06)),
        fill=GREEN,
    )
    draw.ellipse(
        (cx + int(s * 0.01), top - int(s * 0.02),
         cx + int(s * 0.13), top + int(s * 0.06)),
        fill=GREEN,
    )
    # clock hands
    hand = int(s * 0.028)
    draw.rounded_rectangle(
        (cx - hand // 2, cy - int(s * 0.16), cx + hand // 2, cy),
        radius=hand,
        fill=CREAM,
    )
    draw.rounded_rectangle(
        (cx, cy - hand // 2, cx + int(s * 0.12), cy + hand // 2),
        radius=hand,
        fill=CREAM,
    )
    return finish(img, size)


def option_flow(size):
    img, draw, s = canvas(size)
    left = int(s * 0.20)
    width = int(s * 0.45)
    bar = int(s * 0.105)
    gap = int(s * 0.06)
    top = int(s * 0.30)
    colors = [CREAM, RED, GREEN]
    for index, color in enumerate(colors):
        y = top + index * (bar + gap)
        draw.rounded_rectangle(
            (left, y, left + width, y + bar),
            radius=bar // 2,
            fill=color,
        )
    triangle(draw, int(s * 0.80), int(s * 0.50), int(s * 0.10), CREAM)
    return finish(img, size)


def option_monogram(size):
    img, draw, s = canvas(size)
    draw.rounded_rectangle(
        (0, 0, s - 1, s - 1), radius=int(s * 0.23), fill=CREAM
    )
    font = ImageFont.truetype(
        r"C:\Windows\Fonts\arialbd.ttf", int(s * 0.52)
    )
    text = "S"
    bbox = draw.textbbox((0, 0), text, font=font)
    tw = bbox[2] - bbox[0]
    th = bbox[3] - bbox[1]
    draw.text(
        ((s - tw) / 2 - bbox[0], (s - th) / 2 - bbox[1] - int(s * 0.02)),
        text,
        font=font,
        fill=BLACK,
    )
    draw.ellipse(
        (int(s * 0.66), int(s * 0.66), int(s * 0.78), int(s * 0.78)),
        fill=RED,
    )
    return finish(img, size)


def option_book(size):
    img, draw, s = canvas(size)
    page_w = int(s * 0.27)
    page_h = int(s * 0.36)
    gap = int(s * 0.02)
    left = (s - page_w * 2 - gap) // 2
    top = int(s * 0.30)
    radius = int(s * 0.04)
    for index in range(2):
        x = left + index * (page_w + gap)
        draw.rounded_rectangle(
            (x, top, x + page_w, top + page_h), radius=radius, fill=CREAM
        )
    # spine
    draw.rectangle(
        (left + page_w, top, left + page_w + gap, top + page_h), fill=DARK
    )
    triangle(draw, int(s * 0.50), int(s * 0.52), int(s * 0.10), RED)
    return finish(img, size)


VARIANTS = {
    "1-flip-clock": option_flip,
    "2-timer-play": option_timer_play,
    "3-tomato": option_tomato,
    "4-flow": option_flow,
    "5-monogram": option_monogram,
    "6-book": option_book,
}


def build_options():
    previews = []
    for name, func in VARIANTS.items():
        icon = func(1024)
        icon.save(OPTIONS / f"{name}.png")
        previews.append((name, icon))
    # contact sheet 3 x 2
    cell = 360
    pad = 24
    label_h = 40
    sheet = Image.new(
        "RGBA", (pad * 4 + cell * 3, pad * 3 + (cell + label_h) * 2), (0, 0, 0, 0)
    )
    font = ImageFont.truetype(r"C:\Windows\Fonts\arialbd.ttf", 26)
    draw = ImageDraw.Draw(sheet)
    for index, (name, icon) in enumerate(previews):
        col = index % 3
        row = index // 3
        x = pad + col * (cell + pad)
        y = pad + row * (cell + label_h + pad)
        sheet.alpha_composite(icon.resize((cell, cell), Image.LANCZOS), (x, y))
        draw.text((x + 8, y + cell + 6), name, font=font, fill=(255, 255, 255, 255))
    sheet.save(OPTIONS / "options.png")


def build_production(icon: Image.Image):
    sizes = {
        "32x32.png": 32,
        "128x128.png": 128,
        "128x128@2x.png": 256,
        "256x256.png": 256,
        "512x512.png": 512,
        "1024x1024.png": 1024,
    }
    for name, size in sizes.items():
        icon.resize((size, size), Image.LANCZOS).save(ICONS / name)
    icon.resize((256, 256), Image.LANCZOS).save(
        ICONS / "icon.ico",
        sizes=[(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )
    # monochrome tray glyph
    tray = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    draw = ImageDraw.Draw(tray)
    draw.ellipse((8, 8, 56, 56), outline=WHITE, width=6)
    draw.line((32, 32, 32, 14), fill=WHITE, width=5)
    draw.line((32, 32, 46, 32), fill=WHITE, width=5)
    for size, name in [(16, "tray-16.png"), (32, "tray-32.png")]:
        tray.resize((size, size), Image.LANCZOS).save(ICONS / name)


def content_layer(box: int) -> Image.Image:
    """Transparent layer with only the flow bars and play glyph."""
    s = box
    layer = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    draw = ImageDraw.Draw(layer)
    left = int(s * 0.18)
    width = int(s * 0.45)
    bar = int(s * 0.10)
    gap = int(s * 0.06)
    top = int(s * 0.30)
    for index, color in enumerate([CREAM, RED, GREEN]):
        y = top + index * (bar + gap)
        draw.rounded_rectangle(
            (left, y, left + width, y + bar), radius=bar // 2, fill=color
        )
    triangle(draw, int(s * 0.80), int(s * 0.50), int(s * 0.10), CREAM)
    return layer


def fit_content(size: int, ratio: float) -> Image.Image:
    """Content scaled to `ratio` of the canvas, centered, transparent."""
    big = content_layer(2048)
    bbox = big.getbbox()
    if bbox is None:
        return Image.new("RGBA", (size, size), (0, 0, 0, 0))
    content = big.crop(bbox)
    target_w = int(size * ratio)
    target_h = int(content.height * target_w / content.width)
    if target_h > int(size * ratio):
        target_h = int(size * ratio)
        target_w = int(content.width * target_h / content.height)
    content = content.resize((target_w, target_h), Image.LANCZOS)
    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    canvas.alpha_composite(
        content, ((size - target_w) // 2, (size - target_h) // 2)
    )
    return canvas


def build_android():
    res = ROOT / "src-tauri" / "gen" / "android" / "app" / "src" / "main" / "res"
    if not res.exists():
        return
    densities = {
        "mdpi": (48, 108),
        "hdpi": (72, 162),
        "xhdpi": (96, 216),
        "xxhdpi": (144, 324),
        "xxxhdpi": (192, 432),
    }
    for density, (legacy, fg) in densities.items():
        folder = res / f"mipmap-{density}"
        folder.mkdir(parents=True, exist_ok=True)
        option_flow(legacy).resize((legacy, legacy), Image.LANCZOS).save(
            folder / "ic_launcher.png"
        )
        # circular legacy icon
        round_icon = Image.new("RGBA", (legacy, legacy), (0, 0, 0, 0))
        draw = ImageDraw.Draw(round_icon)
        draw.ellipse((0, 0, legacy - 1, legacy - 1), fill=DARK)
        round_icon.alpha_composite(fit_content(legacy, 0.58))
        round_icon.save(folder / "ic_launcher_round.png")
        fit_content(fg, 0.58).save(folder / "ic_launcher_foreground.png")

    colors = res / "values"
    colors.mkdir(parents=True, exist_ok=True)
    (colors / "ic_launcher_background.xml").write_text(
        '<?xml version="1.0" encoding="utf-8"?>\n'
        '<resources>\n'
        '  <color name="ic_launcher_background">#141414</color>\n'
        "</resources>\n",
        encoding="utf-8",
    )

    # preview: adaptive icon under circle / squircle / rounded-square masks
    preview = Image.new("RGBA", (900, 360), (255, 255, 255, 255))
    base = Image.new("RGBA", (300, 300), DARK)
    base.alpha_composite(fit_content(300, 0.58))
    masks = []
    circle = Image.new("L", (300, 300), 0)
    ImageDraw.Draw(circle).ellipse((0, 0, 299, 299), fill=255)
    masks.append(circle)
    squircle = Image.new("L", (300, 300), 0)
    ImageDraw.Draw(squircle).rounded_rectangle((0, 0, 299, 299), radius=88, fill=255)
    masks.append(squircle)
    square = Image.new("L", (300, 300), 0)
    ImageDraw.Draw(square).rounded_rectangle((0, 0, 299, 299), radius=42, fill=255)
    masks.append(square)
    for index, mask in enumerate(masks):
        icon = Image.new("RGBA", (300, 300), (0, 0, 0, 0))
        icon.paste(base, (0, 0), mask)
        preview.alpha_composite(icon, (30 + index * 290, 30))
    (OPTIONS / "android-preview.png").parent.mkdir(parents=True, exist_ok=True)
    preview.save(OPTIONS / "android-preview.png")


if __name__ == "__main__":
    build_options()
    build_production(option_flow(1024))
    build_android()
    print("icons written to", OPTIONS)
