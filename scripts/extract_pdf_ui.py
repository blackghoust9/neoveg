"""Renders the pages of the reference manuals that show the user interface, for visual study.

Rendering all 5,000+ pages would produce gigabytes of mostly text, so the pages are ranked first: a
page scores for every interface keyword in its text and for every large embedded picture
(screenshots and diagrams), and only the best pages of each manual are rendered.

Usage:
    python scripts/extract_pdf_ui.py [--pdfs DIR] [--out DIR] [--per-pdf 40] [--dpi 150] [--all]

The images are for local study only (the manuals are copyrighted): the output folder is outside the
repository and nothing produced here is committed. Build tooling, not application code.
"""

import argparse
import re
import sys
from pathlib import Path

try:
    import pymupdf as fitz  # PyMuPDF >= 1.24
except ImportError:  # older releases
    import fitz

DEFAULT_PDFS = Path(r"G:\Meu Drive\vp-rev-eng\teste\teste2\pdfs")
PREFERRED_OUT = Path(r"D:\ambiente_neoveg\pdf_ui_references")
FALLBACK_OUT = Path.home() / "AppData" / "Local" / "Neoveg" / "pdf_ui_references"

# Words that mark a page describing a window or control of the interface
KEYWORDS = [
    "timeline", "track list", "track header", "trimmer", "mixing console", "master bus",
    "video preview", "project media", "event header", "fader", "channel strip", "toolbar",
    "scopes", "vectorscope", "histogram", "window docking", "plug-in chooser", "pan/crop",
    "transitions window", "video fx", "audio fx", "ruler", "envelope", "cursor",
]
MIN_IMAGE_SIDE = (300, 150)  # pixels: smaller pictures are icons and bullets


def page_score(page) -> int:
    text = (page.get_text() or "").lower()
    score = sum(min(text.count(word), 3) for word in KEYWORDS)
    big = 0
    for image in page.get_images(full=True):
        width, height = image[2], image[3]
        if width >= MIN_IMAGE_SIDE[0] and height >= MIN_IMAGE_SIDE[1]:
            big += 1
    # A screenshot counts much more than a mention
    return score + 6 * min(big, 4) if big else score // 3


def pick_out_dir(requested):
    if requested:
        return Path(requested)
    return PREFERRED_OUT if PREFERRED_OUT.drive and Path(PREFERRED_OUT.drive + "\\").exists() else FALLBACK_OUT


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--pdfs", default=str(DEFAULT_PDFS))
    ap.add_argument("--out", default=None)
    ap.add_argument("--per-pdf", type=int, default=40, help="pages rendered per manual")
    ap.add_argument("--dpi", type=int, default=150)
    ap.add_argument("--all", action="store_true", help="render every page (very large)")
    args = ap.parse_args()

    pdf_dir = Path(args.pdfs)
    out_dir = pick_out_dir(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)
    print(f"Output folder: {out_dir}")
    rendered = 0
    for pdf_file in sorted(pdf_dir.glob("*.pdf")):
        try:
            doc = fitz.open(pdf_file)
            if doc.needs_pass:
                doc.authenticate("")
        except Exception as e:  # an unreadable manual must not stop the others
            print(f"  skipped {pdf_file.name}: {e}")
            continue
        print(f"Processing {pdf_file.name} ({len(doc)} pages)")
        scored = []
        for number in range(len(doc)):
            try:
                scored.append((page_score(doc[number]), number))
            except Exception:
                continue
        scored.sort(key=lambda s: (-s[0], s[1]))
        chosen = range(len(doc)) if args.all else sorted(n for s, n in scored[: args.per_pdf] if s > 0)
        stem = re.sub(r"[^A-Za-z0-9_.-]+", "_", pdf_file.stem)
        for number in chosen:
            target = out_dir / f"{stem}_page_{number + 1:04d}.png"
            if target.exists():
                continue
            try:
                doc[number].get_pixmap(dpi=args.dpi).save(str(target))
                rendered += 1
            except Exception as e:
                print(f"  page {number + 1}: {e}")
    print(f"Done: {rendered} images in {out_dir}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
