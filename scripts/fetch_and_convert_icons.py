#!/usr/bin/env python3
"""Pipeline de busca e conversao de icones BMP open-source do Neoveg (AGENTS.md secao 35).

Busca definicoes vetoriais exclusivamente de bibliotecas open-source permissivas aprovadas:
  * Lucide (`lucide`, ISC / MIT-compat)        — https://lucide.dev
  * Phosphor Icons (`ph`, MIT)                 — https://phosphoricons.com
  * Remix Icon (`ri`, Apache-2.0)              — https://remixicon.com
  * Material Symbols (`material-symbols`, Apache-2.0) — https://fonts.google.com/icons

Utiliza o endpoint em lote oficial do Iconify (`/<prefix>.json?icons=...`, apenas 4 requisicoes
HTTP para os 99 icones), rasteriza cada icone com superamostragem 4x, normaliza o RGB em branco
puro nao-pre-multiplicado (`#ffffff` onde `alpha > 0`, `0` onde `alpha == 0`) para suportar
tingimento dinamico em todos os temas claros/escuros do `egui`, e grava `.bmp` de 32 bits BGRA
(`BITMAPV4HEADER` com canal alpha) em `crates/neoveg_ui/assets/icons/<nome>.bmp`.

Uso:
    python scripts/fetch_and_convert_icons.py [--size 64] [--cache-dir DIR] [--purge-svg] [--with-color]
"""
from __future__ import annotations

import argparse
from collections import defaultdict
import io
import json
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
from generate_icon_formats import save_bmp_32bit_alpha  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUT = ROOT / "crates" / "neoveg_ui" / "assets" / "icons"

# Conjuntos open-source permissivos homologados pela secao 35 do AGENTS.md.
APPROVED_LIBRARIES: dict[str, tuple[str, str]] = {
    "lucide": ("Lucide Icons", "ISC"),
    "ph": ("Phosphor Icons", "MIT"),
    "ri": ("Remix Icon", "Apache-2.0"),
    "material-symbols": ("Material Symbols", "Apache-2.0"),
}

# Mapeamento completo dos 99 icones funcionais do Neoveg -> (colecao_aprovada, nome_do_icone).
ICON_MAP: dict[str, tuple[str, str]] = {
    # 1. Timeline & Edicao (17)
    "split": ("ri", "split-cells-horizontal"),
    "trim_start": ("lucide", "arrow-left-to-line"),
    "trim_end": ("lucide", "arrow-right-to-line"),
    "auto_ripple": ("material-symbols", "double-arrow"),
    "snapping": ("lucide", "magnet"),
    "envelope_volume": ("lucide", "activity"),
    "envelope_pan": ("lucide", "move-horizontal"),
    "pitch_stretch": ("ph", "arrows-out-line-horizontal"),
    "group": ("lucide", "combine"),
    "ungroup": ("lucide", "ungroup"),
    "crossfade": ("material-symbols", "transition-dissolve-outline"),
    "undo": ("lucide", "undo-2"),
    "redo": ("lucide", "redo-2"),
    "zoom_in": ("lucide", "zoom-in"),
    "zoom_out": ("lucide", "zoom-out"),
    "keyframe_add": ("lucide", "diamond-plus"),
    "keyframe_delete": ("lucide", "diamond-minus"),
    # 2. Eventos de Video e Audio (6)
    "pan_crop": ("lucide", "crop"),
    "event_fx": ("lucide", "sparkles"),
    "mute_event": ("ph", "speaker-simple-slash"),
    "lock_event": ("lucide", "lock"),
    "active_take": ("lucide", "layers"),
    "normalize_audio": ("material-symbols", "graphic-eq"),
    # 3. Cabecalho de Faixas / Track Header (11)
    "mute": ("lucide", "volume-x"),
    "solo": ("lucide", "headphones"),
    "mute_track": ("lucide", "volume-x"),
    "solo_track": ("lucide", "headphones"),
    "track_fx": ("ri", "equalizer-line"),
    "arm_record": ("ri", "record-circle-line"),
    "track_motion": ("material-symbols", "picture-in-picture-alt-outline"),
    "phase_invert": ("lucide", "circle-slash"),
    "bypass_fx": ("ph", "lightning-slash"),
    "add_audio_track": ("ri", "music-2-line"),
    "add_video_track": ("ri", "video-add-line"),
    # 4. Barra de Transporte (10)
    "transport_play": ("ph", "play-fill"),
    "transport_pause": ("ph", "pause-fill"),
    "transport_stop": ("ph", "stop-fill"),
    "transport_record": ("ph", "record-fill"),
    "goto_start": ("ph", "skip-back-fill"),
    "goto_end": ("ph", "skip-forward-fill"),
    "loop_region": ("ph", "repeat-bold"),
    "shuttle_reverse": ("ph", "rewind-fill"),
    "shuttle_hold": ("ph", "pause-circle"),
    "shuttle_forward": ("ph", "fast-forward-fill"),
    # 5. Janelas, Ferramentas (egui_dock) & Acoes de Painel (20)
    "project_media": ("ri", "folder-video-line"),
    "trimmer": ("lucide", "clapperboard"),
    "video_scopes": ("ph", "chart-polar"),
    "mixing_console": ("ri", "sound-module-line"),
    "explorer": ("lucide", "folder-tree"),
    "master_bus": ("lucide", "speaker"),
    "loudness_meter": ("lucide", "gauge"),
    "video_preview": ("lucide", "monitor-play"),
    "timeline_view": ("material-symbols", "view-timeline-outline"),
    "color_grading": ("lucide", "palette"),
    "chroma_key": ("lucide", "pipette"),
    "captions": ("lucide", "captions"),
    "media_generators": ("lucide", "wand-sparkles"),
    "import_media": ("lucide", "import"),
    "export_render": ("lucide", "film"),
    "reset": ("lucide", "rotate-ccw"),
    "add": ("lucide", "plus"),
    "remove": ("lucide", "minus"),
    "move_up": ("lucide", "arrow-up"),
    "move_down": ("lucide", "arrow-down"),
    # 6. Ferramentas, arquivo, marcadores e aplicativo (35)
    "new_project": ("lucide", "file-plus"),
    "open_project": ("lucide", "folder-open"),
    "save_project": ("lucide", "save"),
    "project_properties": ("lucide", "file-cog"),
    "cut": ("lucide", "scissors"),
    "copy": ("lucide", "copy"),
    "paste": ("lucide", "clipboard-paste"),
    "delete": ("lucide", "trash-2"),
    "help": ("lucide", "circle-help"),
    "tool_normal": ("lucide", "mouse-pointer"),
    "tool_shuffle": ("lucide", "shuffle"),
    "tool_slip": ("material-symbols", "swap-horiz"),
    "tool_slide": ("material-symbols", "compare-arrows"),
    "tool_stretch": ("lucide", "maximize-2"),
    "tool_split_trim": ("material-symbols", "splitscreen-right-outline"),
    "tool_envelope": ("ph", "bezier-curve"),
    "tool_selection": ("lucide", "square-dashed-mouse-pointer"),
    "tool_zoom": ("lucide", "search"),
    "quantize": ("lucide", "grid-3x3"),
    "metronome": ("ph", "metronome"),
    "marker": ("lucide", "bookmark"),
    "region": ("ph", "brackets-square"),
    "scrub": ("lucide", "audio-lines"),
    "step_forward": ("lucide", "step-forward"),
    "step_back": ("lucide", "step-back"),
    "insert_time": ("material-symbols", "more-time"),
    "reverse": ("lucide", "rewind"),
    "velocity": ("ph", "speedometer"),
    "fade_in": ("ph", "chart-line-up"),
    "fade_out": ("ph", "chart-line-down"),
    "channels": ("ri", "equalizer-2-line"),
    "subproject": ("lucide", "workflow"),
    "preferences": ("lucide", "settings"),
    "welcome": ("lucide", "house"),
    "recent_projects": ("lucide", "history"),
}


def validate_sources() -> None:
    if len(ICON_MAP) != 99:
        raise ValueError(f"Esperados 99 icones no ICON_MAP, encontrados {len(ICON_MAP)}")
    for stem, (prefix, icon_name) in ICON_MAP.items():
        if prefix not in APPROVED_LIBRARIES:
            raise ValueError(
                f"{stem}: biblioteca '{prefix}' ({icon_name}) fora das fontes aprovadas "
                f"{sorted(APPROVED_LIBRARIES)}"
            )


def fetch_collection_svgs(
    prefix: str, icon_names: list[str], render_px: int, cache_dir: Path | None
) -> dict[str, str]:
    """Baixa todos os icones solicitados de `prefix` em uma unica chamada JSON ao Iconify."""
    result: dict[str, str] = {}
    missing: list[str] = []
    for name in icon_names:
        if cache_dir is not None:
            cached = cache_dir / f"{prefix}__{name}.svg"
            if cached.is_file():
                result[name] = cached.read_text(encoding="utf-8")
                continue
        missing.append(name)

    if not missing:
        return result

    joined = ",".join(sorted(set(missing)))
    url = f"https://api.iconify.design/{prefix}.json?icons={joined}"
    payload: dict | None = None
    last_err: Exception | None = None
    for attempt in range(4):
        try:
            req = urllib.request.Request(
                url, headers={"User-Agent": "neoveg-open-source-bmp-pipeline/1.0"}
            )
            with urllib.request.urlopen(req, timeout=30) as resp:
                payload = json.loads(resp.read().decode("utf-8"))
            break
        except (urllib.error.URLError, TimeoutError, json.JSONDecodeError) as exc:
            last_err = exc
            time.sleep(0.8 * (attempt + 1))

    if payload is None:
        raise RuntimeError(f"Falha ao obter lote {url}: {last_err}")

    not_found = payload.get("not_found") or []
    if not_found:
        raise RuntimeError(f"Icones inexistentes em '{prefix}': {not_found}")

    icons_dict: dict[str, dict] = payload.get("icons", {})
    aliases_dict: dict[str, dict] = payload.get("aliases", {})
    default_w = payload.get("width", 24)
    default_h = payload.get("height", 24)
    default_left = payload.get("left", 0)
    default_top = payload.get("top", 0)

    for name in missing:
        entry = icons_dict.get(name)
        if entry is None and name in aliases_dict:
            parent = aliases_dict[name].get("parent")
            if parent and parent in icons_dict:
                entry = {**icons_dict[parent], **aliases_dict[name]}
        if entry is None or "body" not in entry:
            raise RuntimeError(f"Icone '{prefix}:{name}' ausente na resposta JSON do Iconify")

        w = entry.get("width", default_w)
        h = entry.get("height", default_h)
        left = entry.get("left", default_left)
        top = entry.get("top", default_top)
        svg = (
            f'<svg xmlns="http://www.w3.org/2000/svg" '
            f'width="{render_px}" height="{render_px}" '
            f'viewBox="{left} {top} {w} {h}">{entry["body"]}</svg>'
        )
        result[name] = svg
        if cache_dir is not None:
            cache_dir.mkdir(parents=True, exist_ok=True)
            (cache_dir / f"{prefix}__{name}.svg").write_text(svg, encoding="utf-8")

    return result


def rasterize_monochrome_white_rgba(svg_text: str, size: int) -> Image.Image:
    """Rasteriza SVG com superamostragem 4x e normaliza RGB para branco puro nao-pre-multiplicado."""
    supersample = max(size * 4, 256)
    colored_bytes = svg_text.replace("currentColor", "#ffffff").encode("utf-8")

    hi_res: Image.Image | None = None
    try:
        import pymupdf  # type: ignore[import-untyped]

        doc = pymupdf.open(stream=colored_bytes, filetype="svg")
        page = doc[0]
        rect = page.rect
        sx = supersample / rect.width if rect.width else 1.0
        sy = supersample / rect.height if rect.height else 1.0
        pix = page.get_pixmap(matrix=pymupdf.Matrix(sx, sy), alpha=True)
        hi_res = Image.frombytes("RGBA", (pix.width, pix.height), pix.samples)
    except ImportError:
        pass

    if hi_res is None:
        try:
            import cairosvg  # type: ignore[import-untyped]

            png_bytes = cairosvg.svg2png(
                bytestring=colored_bytes,
                output_width=supersample,
                output_height=supersample,
            )
            hi_res = Image.open(io.BytesIO(png_bytes)).convert("RGBA")
        except ImportError:
            pass

    if hi_res is None:
        resvg_bin = shutil.which("resvg")
        if resvg_bin:
            proc = subprocess.run(
                [resvg_bin, "-w", str(supersample), "-h", str(supersample), "-", "-c"],
                input=colored_bytes,
                capture_output=True,
                check=True,
            )
            hi_res = Image.open(io.BytesIO(proc.stdout)).convert("RGBA")

    if hi_res is None:
        raise RuntimeError(
            "Nenhum motor SVG disponivel: instale 'pymupdf' (pip install pymupdf), "
            "'cairosvg' ou o binario 'resvg'."
        )

    resized = hi_res.resize((size, size), Image.Resampling.LANCZOS)
    _, _, _, alpha = resized.split()
    # Garante RGB = (255, 255, 255) nao-pre-multiplicado em todo pixel com alpha > 0,
    # permitindo tingimento multiplicativo exato (`Image::tint`) em temas claros e escuros.
    mask = alpha.point(lambda a: 255 if a > 0 else 0)
    return Image.merge("RGBA", (mask, mask, mask, alpha))


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    ap.add_argument("--out", type=Path, default=DEFAULT_OUT, help="Pasta destino dos .bmp")
    ap.add_argument("--size", type=int, default=64, help="Dimensao quadrada em pixels (padrao: 64)")
    ap.add_argument(
        "--cache-dir",
        type=Path,
        default=None,
        help="Pasta opcional para cache local dos SVGs baixados (fora de crates/)",
    )
    ap.add_argument(
        "--purge-svg",
        action="store_true",
        help="Remove arquivos .svg antigos de crates/neoveg_ui/assets/icons/",
    )
    ap.add_argument(
        "--with-color",
        action="store_true",
        help="Atualiza tambem os 27 icones coloridos MIT (flat-color-icons) em assets/icons_color/",
    )
    args = ap.parse_args()

    validate_sources()
    out_dir: Path = args.out
    out_dir.mkdir(parents=True, exist_ok=True)

    temp_ctx = None
    cache_dir = args.cache_dir
    if cache_dir is None:
        temp_ctx = tempfile.TemporaryDirectory(prefix="neoveg_oss_icons_")
        cache_dir = Path(temp_ctx.name)

    try:
        by_prefix: dict[str, list[str]] = defaultdict(list)
        for prefix, icon_name in ICON_MAP.values():
            by_prefix[prefix].append(icon_name)

        render_px = max(args.size * 4, 256)
        svg_by_spec: dict[tuple[str, str], str] = {}
        for prefix, names in sorted(by_prefix.items()):
            fetched = fetch_collection_svgs(prefix, names, render_px, cache_dir)
            for name, svg in fetched.items():
                svg_by_spec[(prefix, name)] = svg

        counts_by_lib: dict[str, int] = {k: 0 for k in APPROVED_LIBRARIES}
        for stem, spec in sorted(ICON_MAP.items()):
            counts_by_lib[spec[0]] += 1
            im = rasterize_monochrome_white_rgba(svg_by_spec[spec], args.size)
            save_bmp_32bit_alpha(im, out_dir / f"{stem}.bmp")

        removed_svgs = 0
        if args.purge_svg:
            for old_svg in out_dir.glob("*.svg"):
                old_svg.unlink()
                removed_svgs += 1

        summary = ", ".join(f"{k}={v}" for k, v in counts_by_lib.items())
        print(
            f"Convertidos {len(ICON_MAP)} icones BMP ({args.size}x{args.size} 32-bit BGRA) "
            f"em {out_dir} [{summary}]"
            + (f"; removidos {removed_svgs} .svg antigos." if removed_svgs else ".")
        )

        if args.with_color:
            import fetch_color_icons

            rc = fetch_color_icons.main_with_args(size=args.size)
            if rc != 0:
                return rc
    finally:
        if temp_ctx is not None:
            temp_ctx.cleanup()

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
