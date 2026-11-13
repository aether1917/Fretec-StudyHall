"""Generate app / tray icons for Fretec StudyHall (monochrome).

Run: python scripts/gen-icons.py
Requires Pillow.
"""

from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
ICONS = ROOT / "src-tauri" / "icons"
ICONS.mkdir(parents=True, exist_ok=True)

BG = (20, 20, 20, 255)
FG = (237, 237, 237, 255)


def draw_app_icon(size: int) -> Image.Image:
    scale = 4
    canvas = size * scale
    img = Image.new("RGBA", (canvas, canvas), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    radius = int(canvas * 0.24)
    draw.rounded_rectangle((0, 0, canvas - 1, canvas - 1), radius=radius, fill=BG)

    pad = int(canvas * 0.2)
    box = (pad, pad, canvas - pad, canvas - pad)
    ring = max(2, int(canvas * 0.035))
    draw.ellipse(box, outline=FG, width=ring)

    cx = cy = canvas // 2
    # clock hands
    hand = max(2, int(canvas * 0.03))
    draw.line((cx, cy, cx, int(cy - canvas * 0.22)), fill=FG, width=hand)
    draw.line((cx, cy, int(cx + canvas * 0.17), cy), fill=FG, width=hand)
    # centre dot
    dot = max(2, int(canvas * 0.02))
    draw.ellipse((cx - dot, cy - dot, cx + dot, cy + dot), fill=FG)
    # top highlight tick (pomodoro marker)
    draw.rounded_rectangle(
        (int(cx - canvas * 0.06), int(canvas * 0.10), int(cx + canvas * 0.06), int(canvas * 0.14)),
        radius=int(canvas * 0.02),
        fill=FG,
    )
    return img.resize((size, size), Image.LANCZOS)


def draw_tray_icon(size: int) -> Image.Image:
    scale = 8
    canvas = size * scale
    img = Image.new("RGBA", (canvas, canvas), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    pad = int(canvas * 0.12)
    ring = max(2, int(canvas * 0.09))
    draw.ellipse((pad, pad, canvas - pad, canvas - pad), outline=FG, width=ring)
    cx = cy = canvas // 2
    hand = max(2, int(canvas * 0.08))
    draw.line((cx, cy, cx, int(cy - canvas * 0.24)), fill=FG, width=hand)
    draw.line((cx, cy, int(cx + canvas * 0.18), cy), fill=FG, width=hand)
    return img.resize((size, size), Image.LANCZOS)


def main() -> None:
    master = draw_app_icon(1024)
    for size in (32, 128, 256, 512, 1024):
        master.resize((size, size), Image.LANCZOS).save(ICONS / f"{size}x{size}.png")
    master.resize((256, 256), Image.LANCZOS).save(ICONS / "128x128@2x.png")
    master.save(
        ICONS / "icon.ico",
        format="ICO",
        sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )
    for size in (16, 32):
        draw_tray_icon(size).save(ICONS / f"tray-{size}.png")
    print("icons written to", ICONS)


if __name__ == "__main__":
    main()
