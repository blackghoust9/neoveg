#!/usr/bin/env python3
"""Converte os ícones vetoriais SVG do Neoveg em todos os formatos secundários.

Lê os arquivos `.svg` autorais em `crates/neoveg_ui/assets/icons/` (24x24, `currentColor`)
e exporta cópias organizadas para `<destino>/` (padrão: `D:\\ambiente_neoveg\\assets\\icons`):

* `svg/<nome>.svg`           — cópia fiel do SVG vetorial puro (`currentColor`).
* `png/<nome>_32x32.png`     — PNG RGBA 32x32 com fundo transparente.
* `png/<nome>_64x64.png`     — PNG RGBA 64x64 com fundo transparente.
* `png/<nome>_128x128.png`   — PNG RGBA 128x128 com fundo transparente.
* `bmp/<nome>.bmp`           — BMP 32-bit BGRA (`BITMAPV4HEADER`) 128x128 com canal alpha.
* `jpg/<nome>.jpg`           — JPEG 128x128 (4:4:4) composto sobre fundo `#1e1e1e`.

Uso:
    python scripts/generate_icon_formats.py [--src DIR] [--out DIR] [--color HEX]
"""
from __future__ import annotations

import argparse
import io
import os
import shutil
import struct
import subprocess
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_SRC = ROOT / "crates" / "neoveg_ui" / "assets" / "icons"
DEFAULT_ENV = r"D:\ambiente_neoveg"
PNG_SIZES = (32, 64, 128)
RASTER_BASE_SIZE = 128
JPG_BG_RGBA = (0x1E, 0x1E, 0x1E, 0xFF)
DEFAULT_INK_HEX = "#e6e6e6"


def render_svg_rgba(svg_text: str, size: int, ink_hex: str) -> Image.Image:
    """Rasteriza um SVG 24x24 (substituindo `currentColor`) para `Image` RGBA `size x size`."""
    colored_bytes = svg_text.replace("currentColor", ink_hex).encode("utf-8")

    try:
        import pymupdf  # type: ignore[import-untyped]

        doc = pymupdf.open(stream=colored_bytes, filetype="svg")
        page = doc[0]
        rect = page.rect
        sx = size / rect.width if rect.width else 1.0
        sy = size / rect.height if rect.height else 1.0
        pix = page.get_pixmap(matrix=pymupdf.Matrix(sx, sy), alpha=True)
        return Image.frombytes("RGBA", (pix.width, pix.height), pix.samples)
    except ImportError:
        pass

    try:
        import cairosvg  # type: ignore[import-untyped]

        png_bytes = cairosvg.svg2png(
            bytestring=colored_bytes,
            output_width=size,
            output_height=size,
        )
        return Image.open(io.BytesIO(png_bytes)).convert("RGBA")
    except ImportError:
        pass

    resvg_bin = shutil.which("resvg")
    if resvg_bin:
        proc = subprocess.run(
            [resvg_bin, "-w", str(size), "-h", str(size), "-", "-c"],
            input=colored_bytes,
            capture_output=True,
            check=True,
        )
        return Image.open(io.BytesIO(proc.stdout)).convert("RGBA")

    raise RuntimeError(
        "Nenhum motor SVG encontrado: instale 'pymupdf' (pip install pymupdf), "
        "'cairosvg' ou o binário 'resvg'."
    )


def save_bmp_32bit_alpha(im: Image.Image, dest: Path) -> None:
    """Grava BMP 32-bit BGRA com BITMAPV4HEADER e máscaras explícitas de transparência alpha."""
    rgba = im.convert("RGBA")
    width, height = rgba.size
    raw_rgba = rgba.tobytes()

    # Converte linhas RGBA (top-down) em BGRA bottom-up conforme especificação DIB/BMP.
    stride = width * 4
    rows_bgra = bytearray(width * height * 4)
    for y in range(height):
        src_row = raw_rgba[y * stride : (y + 1) * stride]
        dst_y = height - 1 - y
        dst_off = dst_y * stride
        # Permuta R e B mantendo G e A: RGBA -> BGRA
        rows_bgra[dst_off + 0 : dst_off + stride : 4] = src_row[2::4]
        rows_bgra[dst_off + 1 : dst_off + stride : 4] = src_row[1::4]
        rows_bgra[dst_off + 2 : dst_off + stride : 4] = src_row[0::4]
        rows_bgra[dst_off + 3 : dst_off + stride : 4] = src_row[3::4]

    dib_size = 108  # BITMAPV4HEADER
    pixel_offset = 14 + dib_size
    image_size = len(rows_bgra)
    file_size = pixel_offset + image_size
    bi_bitfields = 3
    lcs_srgb = 0x73524742  # 'sRGB'

    file_header = struct.pack("<2sIHHI", b"BM", file_size, 0, 0, pixel_offset)
    dib_header = struct.pack(
        "<IiiHH11I36s3I",
        dib_size,
        width,
        height,
        1,  # biPlanes
        32,  # biBitCount (32-bit com alpha)
        bi_bitfields,
        image_size,
        2835,  # ~72 DPI X
        2835,  # ~72 DPI Y
        0,
        0,
        0x00FF0000,  # máscara R
        0x0000FF00,  # máscara G
        0x000000FF,  # máscara B
        0xFF000000,  # máscara Alpha
        lcs_srgb,
        b"\x00" * 36,  # endpoints CIEXYZTRIPLE (não usados em sRGB)
        0,
        0,
        0,
    )
    dest.write_bytes(file_header + dib_header + rows_bgra)


def save_jpg_dark_bg(im: Image.Image, dest: Path) -> None:
    """Compõe o ícone RGBA sobre fundo sólido #1e1e1e e exporta JPEG 4:4:4 de alta nitidez."""
    rgba = im.convert("RGBA")
    bg = Image.new("RGBA", rgba.size, JPG_BG_RGBA)
    composited = Image.alpha_composite(bg, rgba).convert("RGB")
    composited.save(dest, format="JPEG", quality=95, subsampling=0)


def export_all(src_dir: Path, out_dir: Path, ink_hex: str) -> int:
    if not src_dir.is_dir():
        print(f"erro: pasta de origem não encontrada: {src_dir}", file=sys.stderr)
        return 1

    if out_dir.anchor and not Path(out_dir.anchor).exists():
        print(
            f"aviso: unidade {out_dir.anchor} não existe nesta máquina; "
            f"use --out <pasta> ou defina NEOVEG_ENV_DIR (destino {out_dir} ignorado).",
            file=sys.stderr,
        )
        return 0

    svg_out = out_dir / "svg"
    png_out = out_dir / "png"
    bmp_out = out_dir / "bmp"
    jpg_out = out_dir / "jpg"
    for folder in (svg_out, png_out, bmp_out, jpg_out):
        folder.mkdir(parents=True, exist_ok=True)

    svgs = sorted(src_dir.glob("*.svg"))
    if not svgs:
        print(f"erro: nenhum arquivo .svg encontrado em {src_dir}", file=sys.stderr)
        return 1

    for svg_path in svgs:
        stem = svg_path.stem
        svg_text = svg_path.read_text(encoding="utf-8")

        # 1. Cópia SVG normalizada em LF
        (svg_out / f"{stem}.svg").write_bytes(svg_text.replace("\r\n", "\n").encode("utf-8"))

        # 2. PNGs transparentes em 32x32, 64x64 e 128x128
        rendered_128: Image.Image | None = None
        for size in PNG_SIZES:
            im = render_svg_rgba(svg_text, size, ink_hex)
            im.save(png_out / f"{stem}_{size}x{size}.png", format="PNG")
            if size == RASTER_BASE_SIZE:
                rendered_128 = im

        assert rendered_128 is not None

        # 3. BMP 32-bit BGRA com transparência alpha
        save_bmp_32bit_alpha(rendered_128, bmp_out / f"{stem}.bmp")

        # 4. JPG com fundo #1e1e1e
        save_jpg_dark_bg(rendered_128, jpg_out / f"{stem}.jpg")

    print(
        f"Exportados {len(svgs)} ícones para {out_dir} "
        f"(svg={len(svgs)}, png={len(svgs) * len(PNG_SIZES)}, bmp={len(svgs)}, jpg={len(svgs)})."
    )
    return 0


def main() -> int:
    default_out = Path(os.environ.get("NEOVEG_ENV_DIR", DEFAULT_ENV)) / "assets" / "icons"
    ap = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    ap.add_argument("--src", default=str(DEFAULT_SRC), help="Pasta com os SVGs fonte")
    ap.add_argument("--out", default=str(default_out), help="Pasta raiz de saída dos formatos")
    ap.add_argument("--color", default=DEFAULT_INK_HEX, help="Cor hexadecimal para rasterização")
    args = ap.parse_args()
    return export_all(Path(args.src), Path(args.out), args.color)


if __name__ == "__main__":
    raise SystemExit(main())
