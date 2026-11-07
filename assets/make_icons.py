"""Draws NoDesk's tray icons (a break-timer ring) at 1024 px, saves multi-size .ico files.

Run: python assets/make_icons.py  (needs Pillow). Output is committed, so builds never need Python.
"""
from pathlib import Path

from PIL import Image, ImageDraw

SIZE = 1024
SIZES = [(s, s) for s in (16, 20, 24, 32, 40, 48, 64, 128, 256)]
TEAL, GREY = (45, 212, 191, 255), (148, 163, 184, 255)


def ring(color, sweep):
    """Thick ring with a gap at the top: the countdown arc; `sweep` degrees drawn clockwise from 12 o'clock."""
    img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    pad, width = 120, 150
    d.arc([pad, pad, SIZE - pad, SIZE - pad], -90 + (360 - sweep), 270, fill=color, width=width)
    return img, d


def app_icon():
    """Teal ring, 300 degrees left, plus a dot: time running toward a break."""
    img, d = ring(TEAL, 300)
    r = 110
    d.ellipse([SIZE / 2 - r, SIZE / 2 - r, SIZE / 2 + r, SIZE / 2 + r], fill=TEAL)
    return img


def paused_icon():
    """Grey full ring with two pause bars."""
    img, d = ring(GREY, 360)
    w, h, gap = 90, 300, 70
    for x in (SIZE / 2 - gap - w, SIZE / 2 + gap):
        d.rounded_rectangle([x, SIZE / 2 - h / 2, x + w, SIZE / 2 + h / 2], radius=30, fill=GREY)
    return img


if __name__ == "__main__":
    out = Path(__file__).parent
    for name, make in (("nodesk.ico", app_icon), ("paused.ico", paused_icon)):
        make().save(out / name, sizes=SIZES)
        print("wrote", out / name)
