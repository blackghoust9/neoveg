#!/usr/bin/env python3
"""Baixa ícones coloridos do conjunto Flat Color Icons (MIT, Icons8) via Iconify e grava BMP.

Cada ícone do Neoveg (arquivo em `crates/neoveg_ui/assets/icons/`) ganha, quando existe um
equivalente claro, um BMP 32-bit com alpha em `crates/neoveg_ui/assets/icons_color/<nome>.bmp`.
Os demais ícones continuam monocromáticos. Os SVG baixados vão para uma pasta temporária
e nunca entram no repositório.

Uso: python scripts/fetch_color_icons.py [--tmp DIR] [--size 64]
"""
from __future__ import annotations

import argparse
import sys
import tempfile
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from generate_icon_formats import render_svg_rgba, save_bmp_32bit_alpha  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "crates" / "neoveg_ui" / "assets" / "icons_color"
SET = "flat-color-icons"

# Nome do ícone do Neoveg -> nome no conjunto Flat Color Icons (só equivalências claras).
MAP = {
    "undo": "undo",
    "redo": "redo",
    "add": "plus",
    "remove": "minus",
    "lock_event": "lock",
    "import_media": "import",
    "export_render": "export",
    "open_project": "opened-folder",
    "project_media": "folder",
    "new_project": "document",
    "recent_projects": "clock",
    "preferences": "settings",
    "help": "faq",
    "delete": "empty-trash",
    "trimmer": "film",
    "add_video_track": "video-file",
    "add_audio_track": "audio-file",
    "welcome": "home",
    "goto_start": "previous",
    "goto_end": "next",
    "marker": "bookmark",
    "timeline_view": "timeline",
    "master_bus": "speaker",
    "subproject": "tree-structure",
    "project_properties": "view-details",
    "media_generators": "clapperboard",
    "video_preview": "camcorder",
}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--tmp", type=Path, default=None)
    ap.add_argument("--size", type=int, default=64)
    args = ap.parse_args()
    tmp = args.tmp or Path(tempfile.mkdtemp(prefix="neoveg_color_icons_"))
    tmp.mkdir(parents=True, exist_ok=True)
    OUT.mkdir(parents=True, exist_ok=True)
    known = {p.stem for p in (ROOT / "crates/neoveg_ui/assets/icons").glob("*.svg")}
    for ours, theirs in MAP.items():
        if ours not in known:
            print(f"ignorado (não existe no Neoveg): {ours}", file=sys.stderr)
            continue
        url = f"https://api.iconify.design/{SET}/{theirs}.svg"
        req = urllib.request.Request(url, headers={"User-Agent": "neoveg-icon-fetch/1.0"})
        svg = urllib.request.urlopen(req, timeout=30).read().decode("utf-8")
        if "<svg" not in svg:
            print(f"falhou: {url}", file=sys.stderr)
            return 1
        (tmp / f"{ours}.svg").write_text(svg, encoding="utf-8")
        im = render_svg_rgba(svg, args.size, "#000000")
        save_bmp_32bit_alpha(im, OUT / f"{ours}.bmp")
    print(f"{len(MAP)} BMP em {OUT} (SVG de origem em {tmp})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
