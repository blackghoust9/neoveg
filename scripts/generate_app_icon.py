#!/usr/bin/env python3
"""Draws the Neoveg application icon, the "Nv" mark, as `assets/app_icon/app_icon.png` and `.ico`.

The mark is original artwork made from plain polygons (no font and no third-party image): a white
N and an azure v on a dark navy square with a thin accent bar. It is the icon of the program in the
system (the executable, the window, the task bar), not a picture shown inside the startup screen.

Usage:
    python scripts/generate_app_icon.py [--out DIR]

Needs Pillow. The drawing is made at 1024 x 1024 and scaled down, so every size is crisp.
"""
from __future__ import annotations

import argparse
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
SIZE = 1024
BACKGROUND = (0x12, 0x1A, 0x24, 255)
LETTER_N = (0xF2, 0xF5, 0xF8, 255)
LETTER_V = (0x3E, 0xA6, 0xE0, 255)
ACCENT_BAR = (0x29, 0x80, 0xB9, 255)
ICO_SIZES = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]


def draw_mark() -> Image.Image:
    image = Image.new("RGBA", (SIZE, SIZE), BACKGROUND)
    d = ImageDraw.Draw(image)
    # N: two stems and the diagonal that joins them
    d.rectangle([170, 230, 270, 794], fill=LETTER_N)
    d.rectangle([470, 230, 570, 794], fill=LETTER_N)
    d.polygon([(170, 230), (270, 230), (570, 794), (470, 794)], fill=LETTER_N)
    # v: one filled V
    d.polygon(
        [(610, 420), (706, 420), (755, 650), (804, 420), (900, 420), (803, 794), (707, 794)],
        fill=LETTER_V,
    )
    # a thin accent bar under both letters
    d.rectangle([170, 850, 900, 884], fill=ACCENT_BAR)
    return image


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", default=str(ROOT / "assets" / "app_icon"))
    args = ap.parse_args()
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    master = draw_mark()
    master.resize((512, 512), Image.LANCZOS).save(out / "app_icon.png", optimize=True)
    master.save(out / "app_icon.ico", format="ICO", sizes=ICO_SIZES)
    print(f"gravado {out / 'app_icon.png'} e {out / 'app_icon.ico'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
