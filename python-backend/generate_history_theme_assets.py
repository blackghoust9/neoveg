"""
Neoveg — Historical VEGAS Pro Visual References & Dynamic Theme Asset Generator
Builds `assets/vegas_history_themes/` covering all 4 historical eras of VEGAS Pro:
  1. `01_era_sonic_foundry_v1_v3` (1999–2002): Industrial Metallic Gray & Red/Orange accents
  2. `02_era_sony_v4_v13`         (2003–2015): Flaming Gold/Orange Gradient (v4–7) & Pro Silver/Dark Slate (v8–13)
  3. `03_era_magix_v14_v22`       (2016–2024): Modern Cyan Blue, Dark Carbon & Flat Design
  4. `04_era_borisfx_2026`        (2025–2026): Contemporary Electric Blue & Deep Obsidian Dark

Also inspects and organizes the visual references from `references/img/`, generates SVG & PNG
logos, vector icon sets, color swatch cards, and writes `assets/vegas_history_themes/theme_catalog.json`
consumed by the C# Avalonia UI dynamic theme engine (`VegasHistoryTheme.cs`).
"""

from __future__ import annotations
import json
import shutil
from pathlib import Path
from typing import Any, Dict, List, Tuple
from PIL import Image, ImageDraw

ROOT_DIR = Path(__file__).resolve().parent.parent
REF_IMG_DIR = ROOT_DIR / "references" / "img"
OUT_ROOT = ROOT_DIR / "assets" / "vegas_history_themes"


THEMES_DATA: List[Dict[str, Any]] = [
    {
        "id": "sonic_foundry_v1_v3",
        "era_folder": "01_era_sonic_foundry_v1_v3",
        "era_number": 1,
        "name": "Sonic Foundry Vegas 1–3 (1999–2002)",
        "short_badge": "SONIC FOUNDRY V3",
        "years": "1999–2002",
        "vendor": "Sonic Foundry, Inc. (Madison, WI)",
        "versions": ["Vegas Pro 1.0", "Vegas Audio 2.0", "Vegas Video 2.0", "Vegas Video 3.0"],
        "description": "Estética industrial clássica Win32 com cinza metálico escovado, bordas chanfradas 3D e detalhes vibrantes em vermelho carmim e laranja incandescente.",
        "palette": {
            "window_bg": "#B8B5AE",
            "menu_bg": "#D4D0C8",
            "toolbar_bg": "#C6C3BD",
            "dock_header_bg": "#84827E",
            "dock_panel_bg": "#CBC8C2",
            "dock_border": "#42413E",
            "timeline_bg": "#A6A39C",
            "ruler_bg": "#94918A",
            "track_video_bg": "#B4B9C2",
            "track_audio_bg": "#C2B4B2",
            "clip_header_bg": "#3A3835",
            "accent_primary": "#CC291B",
            "accent_secondary": "#FF6A00",
            "timecode_bg": "#181816",
            "timecode_fg": "#FF4500",
            "playhead_color": "#CC291B",
            "waveform_color": "#8B1A10",
            "button_bg": "#D4D0C8",
            "button_fg": "#181816",
            "text_primary": "#141413",
            "text_secondary": "#3E3D3A",
            "status_bar_bg": "#84827E",
        },
        "reference_files": [],
    },
    {
        "id": "sony_v4_v7_flaming_gold",
        "era_folder": "02_era_sony_v4_v13",
        "era_number": 2,
        "name": "Sony Vegas 4–7 Flaming Gold (2003–2007)",
        "short_badge": "SONY VEGAS 7 GOLD",
        "years": "2003–2007",
        "vendor": "Sony Pictures Digital / Sony Media Software",
        "versions": ["Vegas 4.0", "Sony Vegas 5.0", "Sony Vegas 6.0", "Sony Vegas 7.0"],
        "description": "Estética clássica com superfície Luna Silver/Bege e o icônico gradiente dourado/laranja flamejante nas barras de destaque, timecode âmbar e faixas azul-aço/vinho.",
        "palette": {
            "window_bg": "#DFDDD4",
            "menu_bg": "#ECE9D8",
            "toolbar_bg": "#E4E1D6",
            "dock_header_bg": "#C8C4B6",
            "dock_panel_bg": "#EAE7DC",
            "dock_border": "#9D998C",
            "timeline_bg": "#F2F0E8",
            "ruler_bg": "#DCD9CE",
            "track_video_bg": "#ADC2D9",
            "track_audio_bg": "#D4BBC0",
            "clip_header_bg": "#2B4C6F",
            "accent_primary": "#FF6D00",
            "accent_secondary": "#FFB300",
            "timecode_bg": "#23211D",
            "timecode_fg": "#FFB300",
            "playhead_color": "#E65100",
            "waveform_color": "#7E222A",
            "button_bg": "#DCD9CE",
            "button_fg": "#1B1A17",
            "text_primary": "#181715",
            "text_secondary": "#4E4C45",
            "status_bar_bg": "#D4D0C4",
        },
        "reference_files": [
            "maxresdefault.png",
            "10image.png",
            "11maxresdefault.png",
        ],
    },
    {
        "id": "sony_v8_v13_pro_silver_dark",
        "era_folder": "02_era_sony_v4_v13",
        "era_number": 2,
        "name": "Sony Vegas Pro 8–13 Prata/Escuro (2008–2015)",
        "short_badge": "SONY VEGAS PRO 12",
        "years": "2008–2015",
        "vendor": "Sony Creative Software",
        "versions": ["Vegas Pro 8.0", "Vegas Pro 9.0", "Vegas Pro 10.0", "Vegas Pro 11.0", "Vegas Pro 12.0", "Vegas Pro 13.0"],
        "description": "Tema Prata/Escuro (Pro Dark Slate & Silver) introduzido na geração Vegas Pro 9–13 com painéis cinza-ardósia, cabeçalhos azul-aço metálico e marcadores laranja vivo.",
        "palette": {
            "window_bg": "#5A5D62",
            "menu_bg": "#76797E",
            "toolbar_bg": "#6B6E73",
            "dock_header_bg": "#4E5156",
            "dock_panel_bg": "#64676C",
            "dock_border": "#3B3D42",
            "timeline_bg": "#787B80",
            "ruler_bg": "#5E6166",
            "track_video_bg": "#6B86A2",
            "track_audio_bg": "#866B74",
            "clip_header_bg": "#2B3A4A",
            "accent_primary": "#3B82F6",
            "accent_secondary": "#F37021",
            "timecode_bg": "#383A3E",
            "timecode_fg": "#EAECEF",
            "playhead_color": "#F37021",
            "waveform_color": "#1E3A5F",
            "button_bg": "#52555A",
            "button_fg": "#F3F4F6",
            "text_primary": "#F3F4F6",
            "text_secondary": "#D1D5DB",
            "status_bar_bg": "#4A4D52",
        },
        "reference_files": [
            "image.png",
            "2image.png",
            "uploads_2Fe3b81eff-fd87-4c3e-830b-bf63d38b9d17_2FSony_Vegas_Pro.png",
        ],
    },
    {
        "id": "magix_v14_v15_flat_gray",
        "era_folder": "03_era_magix_v14_v22",
        "era_number": 3,
        "name": "MAGIX VEGAS Pro 14–15 Flat Gray (2016–2017)",
        "short_badge": "MAGIX VEGAS 15",
        "years": "2016–2017",
        "vendor": "MAGIX Software GmbH / VEGAS Creative Software",
        "versions": ["VEGAS Pro 14.0", "VEGAS Pro 15.0"],
        "description": "Primeira geração MAGIX com ícones vetoriais planos (flat design) monocromáticos sobre superfície cinza médio neutra e destaques em azul ciano.",
        "palette": {
            "window_bg": "#7A7C80",
            "menu_bg": "#94969A",
            "toolbar_bg": "#888A8E",
            "dock_header_bg": "#686A6E",
            "dock_panel_bg": "#828488",
            "dock_border": "#525458",
            "timeline_bg": "#727478",
            "ruler_bg": "#66686C",
            "track_video_bg": "#6D879E",
            "track_audio_bg": "#8C6268",
            "clip_header_bg": "#32363C",
            "accent_primary": "#0088E8",
            "accent_secondary": "#00B4FF",
            "timecode_bg": "#4A4C50",
            "timecode_fg": "#FFFFFF",
            "playhead_color": "#00A2FF",
            "waveform_color": "#E2E8F0",
            "button_bg": "#686A6E",
            "button_fg": "#FFFFFF",
            "text_primary": "#F8FAFC",
            "text_secondary": "#E2E8F0",
            "status_bar_bg": "#5E6064",
        },
        "reference_files": [
            "beginner-tips-for-vegas-pro-3.png",
            "5image.png",
        ],
    },
    {
        "id": "magix_v16_v22_carbon_cyan",
        "era_folder": "03_era_magix_v14_v22",
        "era_number": 3,
        "name": "MAGIX VEGAS Pro 16–22 Carbon & Cyan (2018–2024)",
        "short_badge": "MAGIX VEGAS 21",
        "years": "2018–2024",
        "vendor": "MAGIX Software GmbH / VEGAS Creative Software",
        "versions": ["VEGAS Pro 16", "VEGAS Pro 17", "VEGAS Pro 18", "VEGAS Pro 19", "VEGAS Pro 20", "VEGAS Pro 21", "VEGAS Pro 22"],
        "description": "Estética moderna MAGIX em carvão escuro (Dark Carbon), azul ciano vibrante (#0078D7 / #18A0FB), design 100% plano, Color Grading Wheels e presets visuais.",
        "palette": {
            "window_bg": "#232326",
            "menu_bg": "#2D2D30",
            "toolbar_bg": "#28282C",
            "dock_header_bg": "#333337",
            "dock_panel_bg": "#26262A",
            "dock_border": "#3E3E42",
            "timeline_bg": "#1E1E21",
            "ruler_bg": "#2A2A2E",
            "track_video_bg": "#3A4E63",
            "track_audio_bg": "#5C2F35",
            "clip_header_bg": "#18181B",
            "accent_primary": "#0078D7",
            "accent_secondary": "#18A0FB",
            "timecode_bg": "#1A1A1D",
            "timecode_fg": "#18A0FB",
            "playhead_color": "#0099FF",
            "waveform_color": "#F3C6C8",
            "button_bg": "#333338",
            "button_fg": "#F1F5F9",
            "text_primary": "#F1F5F9",
            "text_secondary": "#9CA3AF",
            "status_bar_bg": "#1C1C1F",
        },
        "reference_files": [
            "icones-vegas.png",
            "8image.png",
            "color-grading-sony-vegas-8.png",
            "Vegas_Pro_Screenshot.png",
            "4a839cda-adb3-4375-b651-be8fb3b977c9.png",
            "filmmakers.png",
            "ss_08c8d4b9782e2c0dbb1fe9e49d4909eead36314a.1920x1080.png",
            "ss_64be9b7d4992f1d85ee1d99b5e5e05711431d769.1920x1080.png",
        ],
    },
    {
        "id": "borisfx_2026_electric_dark",
        "era_folder": "04_era_borisfx_2026",
        "era_number": 4,
        "name": "Boris FX / VEGAS Pro Atual (2025–2026 Electric Blue)",
        "short_badge": "BORIS FX • 2026",
        "years": "2025–2026",
        "vendor": "VEGAS Creative Software / Boris FX Ecosystem",
        "versions": ["VEGAS Pro 23.0", "VEGAS Pro 2026 Neoveg (BCC+ / AI Suite)"],
        "description": "Identidade visual contemporânea em preto obsidiana (#0E1015), azul elétrico (#00A3FF / #2563EB), cabeçalhos de faixa multicor de alto contraste e integração nativa BCC+/AI.",
        "palette": {
            "window_bg": "#0E1015",
            "menu_bg": "#13161E",
            "toolbar_bg": "#161A24",
            "dock_header_bg": "#1B202C",
            "dock_panel_bg": "#12161F",
            "dock_border": "#262F40",
            "timeline_bg": "#0B0D12",
            "ruler_bg": "#151922",
            "track_video_bg": "#172336",
            "track_audio_bg": "#191D2E",
            "clip_header_bg": "#090D16",
            "accent_primary": "#00A3FF",
            "accent_secondary": "#38BDF8",
            "timecode_bg": "#090B10",
            "timecode_fg": "#38BDF8",
            "playhead_color": "#00A3FF",
            "waveform_color": "#7DD3FC",
            "button_bg": "#1E2636",
            "button_fg": "#F8FAFC",
            "text_primary": "#F8FAFC",
            "text_secondary": "#94A3B8",
            "status_bar_bg": "#10141C",
        },
        "reference_files": [
            "3image.png",
            "4image.png",
            "6image.png",
            "7image.png",
            "VEGAS_Suite_UI_202.png",
        ],
    },
]


def _hex_to_rgb(hex_str: str) -> Tuple[int, int, int]:
    h = hex_str.lstrip("#")
    return int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)


def _sample_dominant_colors(img_path: Path, count: int = 6) -> List[str]:
    """Samples representative quantized RGB colors from a reference screenshot."""
    try:
        with Image.open(img_path) as im:
            small = im.convert("RGB").resize((64, 64))
            quant = small.quantize(colors=count)
            pal = quant.getpalette()[: count * 3]
            out: List[str] = []
            for i in range(0, len(pal), 3):
                out.append(f"#{pal[i]:02X}{pal[i+1]:02X}{pal[i+2]:02X}")
            return out
    except Exception:
        return []


def _generate_era_logo_svg(theme: Dict[str, Any]) -> str:
    pal = theme["palette"]
    acc1 = pal["accent_primary"]
    acc2 = pal["accent_secondary"]
    bg = pal["timecode_bg"]
    tid = theme["id"]

    if tid == "sonic_foundry_v1_v3":
        # Industrial Sonic Foundry anvil/wave emblem + beveled metallic badge
        return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" width="256" height="256">
  <defs>
    <linearGradient id="sf_metal" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="#E4E2DC"/>
      <stop offset="50%" stop-color="#B8B5AE"/>
      <stop offset="100%" stop-color="#6E6C66"/>
    </linearGradient>
    <linearGradient id="sf_fire" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="#FF7A00"/>
      <stop offset="100%" stop-color="#CC291B"/>
    </linearGradient>
  </defs>
  <rect x="8" y="8" width="240" height="240" fill="url(#sf_metal)" stroke="#42413E" stroke-width="8"/>
  <rect x="16" y="16" width="224" height="224" fill="none" stroke="#FFFFFF" stroke-width="3"/>
  <rect x="28" y="32" width="200" height="44" fill="#1E1E1C" stroke="#42413E" stroke-width="2"/>
  <text x="128" y="61" text-anchor="middle" font-family="Arial Black, sans-serif" font-size="20" fill="#FF6A00" letter-spacing="2">SONIC FOUNDRY</text>
  <polygon points="48,96 92,96 128,186 164,96 208,96 146,220 110,220" fill="url(#sf_fire)" stroke="#181816" stroke-width="4"/>
  <line x1="36" y1="152" x2="220" y2="152" stroke="#181816" stroke-width="4" stroke-dasharray="8,6"/>
</svg>"""

    if tid == "sony_v4_v7_flaming_gold":
        # Sony Vegas 4-7 iconic Flaming Gold/Orange crest
        return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" width="256" height="256">
  <defs>
    <linearGradient id="sony_flame" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stop-color="#FFE066"/>
      <stop offset="45%" stop-color="#FF8C00"/>
      <stop offset="100%" stop-color="#C62828"/>
    </linearGradient>
  </defs>
  <rect x="10" y="10" width="236" height="236" rx="18" fill="#23211D" stroke="#FF8C00" stroke-width="6"/>
  <path d="M 42,64 L 92,64 L 128,174 L 164,64 L 214,64 L 148,214 L 108,214 Z" fill="url(#sony_flame)" stroke="#FFF3B0" stroke-width="3"/>
  <text x="128" y="46" text-anchor="middle" font-family="Trebuchet MS, Arial, sans-serif" font-weight="bold" font-size="22" fill="#FFB300" letter-spacing="4">SONY VEGAS</text>
</svg>"""

    if tid == "sony_v8_v13_pro_silver_dark":
        # Sony Vegas Pro 8-13 Brushed Silver & Steel Blue V
        return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" width="256" height="256">
  <defs>
    <linearGradient id="sony_pro" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="#93C5FD"/>
      <stop offset="50%" stop-color="#3B82F6"/>
      <stop offset="100%" stop-color="#1D4ED8"/>
    </linearGradient>
  </defs>
  <rect x="10" y="10" width="236" height="236" rx="12" fill="#3B3D42" stroke="#9CA3AF" stroke-width="5"/>
  <polygon points="46,60 94,60 128,168 162,60 210,60 148,212 108,212" fill="url(#sony_pro)" stroke="#E5E7EB" stroke-width="3"/>
  <rect x="42" y="218" width="172" height="8" fill="#F37021"/>
  <text x="128" y="44" text-anchor="middle" font-family="Segoe UI, Arial, sans-serif" font-weight="bold" font-size="19" fill="#E5E7EB" letter-spacing="3">SONY VEGAS PRO</text>
</svg>"""

    if "magix" in tid:
        # MAGIX VEGAS Pro 14-22 Flat Geometric Cyan V Prism
        return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" width="256" height="256">
  <rect x="8" y="8" width="240" height="240" rx="8" fill="#1E1E21" stroke="{acc2}" stroke-width="5"/>
  <polygon points="44,52 96,52 136,184 112,212" fill="#0078D7"/>
  <polygon points="212,52 160,52 112,212 144,212" fill="#18A0FB"/>
  <text x="128" y="236" text-anchor="middle" font-family="Segoe UI, sans-serif" font-weight="bold" font-size="18" fill="#F1F5F9" letter-spacing="3">MAGIX VEGAS PRO</text>
</svg>"""

    # Boris FX / 2026 Electric Blue & Obsidian Dark
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" width="256" height="256">
  <defs>
    <linearGradient id="bfx_elec" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="#38BDF8"/>
      <stop offset="50%" stop-color="#00A3FF"/>
      <stop offset="100%" stop-color="#2563EB"/>
    </linearGradient>
  </defs>
  <rect x="8" y="8" width="240" height="240" rx="14" fill="{bg}" stroke="#00A3FF" stroke-width="5"/>
  <polygon points="42,48 94,48 128,162 162,48 214,48 148,206 108,206" fill="url(#bfx_elec)"/>
  <circle cx="198" cy="192" r="18" fill="#E5534B"/>
  <text x="128" y="234" text-anchor="middle" font-family="Segoe UI, Inter, sans-serif" font-weight="bold" font-size="17" fill="#38BDF8" letter-spacing="3">VEGAS 2026 • BORIS FX</text>
</svg>"""


def _generate_era_icons_svg(theme: Dict[str, Any]) -> str:
    pal = theme["palette"]
    fg = pal["text_primary"]
    acc = pal["accent_primary"]
    bg = pal["button_bg"]
    border = pal["dock_border"]
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 360 64" width="360" height="64">
  <rect width="360" height="64" fill="{pal['toolbar_bg']}"/>
  <!-- 1. Project / Welcome -->
  <rect x="8" y="8" width="48" height="48" fill="{bg}" stroke="{border}" stroke-width="2"/>
  <rect x="16" y="16" width="32" height="32" fill="none" stroke="{fg}" stroke-width="2.2"/>
  <line x1="16" y1="25" x2="48" y2="25" stroke="{acc}" stroke-width="2.5"/>
  <!-- 2. Save .veg -->
  <rect x="68" y="8" width="48" height="48" fill="{bg}" stroke="{border}" stroke-width="2"/>
  <polygon points="76,16 102,16 108,22 108,48 76,48" fill="none" stroke="{fg}" stroke-width="2.2"/>
  <rect x="83" y="33" width="18" height="15" fill="{acc}"/>
  <!-- 3. Split / Razor Cut (S) -->
  <rect x="128" y="8" width="48" height="48" fill="{bg}" stroke="{border}" stroke-width="2"/>
  <line x1="152" y1="12" x2="152" y2="52" stroke="{acc}" stroke-width="3"/>
  <rect x="135" y="22" width="12" height="20" fill="none" stroke="{fg}" stroke-width="2"/>
  <rect x="157" y="22" width="12" height="20" fill="none" stroke="{fg}" stroke-width="2"/>
  <!-- 4. Track Group Link (V+A) -->
  <rect x="188" y="8" width="48" height="48" fill="{bg}" stroke="{border}" stroke-width="2"/>
  <rect x="196" y="16" width="16" height="12" fill="{pal['track_video_bg']}" stroke="{fg}" stroke-width="1.5"/>
  <rect x="196" y="36" width="16" height="12" fill="{pal['track_audio_bg']}" stroke="{fg}" stroke-width="1.5"/>
  <path d="M 212,22 L 226,22 L 226,42 L 212,42" fill="none" stroke="{acc}" stroke-width="2.5"/>
  <!-- 5. Pan/Crop Crosshair -->
  <rect x="248" y="8" width="48" height="48" fill="{bg}" stroke="{border}" stroke-width="2"/>
  <rect x="256" y="18" width="32" height="28" fill="none" stroke="{fg}" stroke-width="2" stroke-dasharray="4,2"/>
  <circle cx="272" cy="32" r="5" fill="{acc}"/>
  <!-- 6. Render Master -->
  <rect x="308" y="8" width="44" height="48" fill="{acc}" stroke="{border}" stroke-width="2"/>
  <polygon points="322,20 322,44 342,32" fill="#FFFFFF"/>
</svg>"""


def _render_era_logo_png(theme: Dict[str, Any], out_path: Path) -> None:
    pal = theme["palette"]
    img = Image.new("RGBA", (256, 256), _hex_to_rgb(pal["window_bg"]) + (255,))
    draw = ImageDraw.Draw(img)
    border_rgb = _hex_to_rgb(pal["accent_primary"])
    draw.rectangle([6, 6, 249, 249], outline=border_rgb, width=6)
    draw.rectangle([18, 18, 237, 237], fill=_hex_to_rgb(pal["timecode_bg"]) + (255,), outline=_hex_to_rgb(pal["dock_border"]), width=2)

    # Draw stylized V emblem
    acc1 = _hex_to_rgb(pal["accent_primary"])
    acc2 = _hex_to_rgb(pal["accent_secondary"])
    draw.polygon([(48, 58), (94, 58), (128, 172), (162, 58), (208, 58), (146, 206), (110, 206)], fill=acc1, outline=acc2)
    draw.rectangle([36, 216, 220, 228], fill=acc2)
    clean_badge = theme["short_badge"].replace("–", "-").replace("•", "-")
    draw.text((42, 28), clean_badge, fill=_hex_to_rgb(pal["timecode_fg"]))
    img.save(out_path)


def _render_swatch_card_png(theme: Dict[str, Any], out_path: Path) -> None:
    pal = theme["palette"]
    w, h = 760, 260
    img = Image.new("RGB", (w, h), _hex_to_rgb(pal["window_bg"]))
    draw = ImageDraw.Draw(img)

    # Top header banner
    draw.rectangle([0, 0, w, 44], fill=_hex_to_rgb(pal["dock_header_bg"]))
    header_title = f"{theme['name']} - {theme['vendor']}".replace("–", "-").replace("•", "|")
    draw.text((14, 14), header_title, fill=_hex_to_rgb(pal["text_primary"]))

    # Draw 10 key color swatches
    keys = [
        ("window_bg", "Window"),
        ("toolbar_bg", "Toolbar"),
        ("dock_panel_bg", "Dock"),
        ("timeline_bg", "Timeline"),
        ("track_video_bg", "Video Trk"),
        ("track_audio_bg", "Audio Trk"),
        ("accent_primary", "Accent 1"),
        ("accent_secondary", "Accent 2"),
        ("timecode_bg", "SMPTE Bg"),
        ("timecode_fg", "SMPTE Fg"),
    ]
    box_w = 68
    for idx, (k, label) in enumerate(keys):
        x0 = 12 + idx * 74
        y0 = 58
        hex_val = pal[k]
        draw.rectangle([x0, y0, x0 + box_w, y0 + 82], fill=_hex_to_rgb(hex_val), outline=_hex_to_rgb(pal["dock_border"]), width=2)
        draw.rectangle([x0, y0 + 84, x0 + box_w, y0 + 124], fill=_hex_to_rgb(pal["timecode_bg"]))
        draw.text((x0 + 5, y0 + 88), label, fill=(240, 244, 248))
        draw.text((x0 + 5, y0 + 104), hex_val, fill=_hex_to_rgb(pal["accent_secondary"]))

    # Mock mini timeline preview bar at bottom
    draw.rectangle([12, 196, w - 12, 246], fill=_hex_to_rgb(pal["timeline_bg"]), outline=_hex_to_rgb(pal["dock_border"]), width=2)
    draw.rectangle([14, 198, 180, 221], fill=_hex_to_rgb(pal["track_video_bg"]))
    draw.text((20, 204), "1  Video Track", fill=_hex_to_rgb(pal["text_primary"]))
    draw.rectangle([14, 222, 180, 244], fill=_hex_to_rgb(pal["track_audio_bg"]))
    draw.text((20, 227), "2  Audio Track", fill=_hex_to_rgb(pal["text_primary"]))
    # Playhead line
    draw.line([(360, 196), (360, 246)], fill=_hex_to_rgb(pal["playhead_color"]), width=3)
    draw.rectangle([560, 202, 736, 240], fill=_hex_to_rgb(pal["timecode_bg"]), outline=_hex_to_rgb(pal["accent_primary"]), width=1)
    draw.text((578, 214), "00:01:24;18  SMPTE", fill=_hex_to_rgb(pal["timecode_fg"]))

    img.save(out_path)


def generate_all_assets() -> Dict[str, Any]:
    OUT_ROOT.mkdir(parents=True, exist_ok=True)
    catalog_entries: List[Dict[str, Any]] = []

    import neoveg_integration
    all_themes = list(THEMES_DATA) + list(neoveg_integration.NEOVEG_CPP_PALETTES)

    for theme in all_themes:
        era_dir = OUT_ROOT / theme["era_folder"]
        ref_out_dir = era_dir / "references"
        era_dir.mkdir(parents=True, exist_ok=True)
        ref_out_dir.mkdir(parents=True, exist_ok=True)

        tid = theme["id"]
        # 1. Generate SVG & PNG Logo
        svg_logo_path = era_dir / f"logo_{tid}.svg"
        png_logo_path = era_dir / f"logo_{tid}.png"
        svg_logo_path.write_text(_generate_era_logo_svg(theme), encoding="utf-8")
        _render_era_logo_png(theme, png_logo_path)

        # 2. Generate Vector Icons SVG
        svg_icons_path = era_dir / f"icons_{tid}.svg"
        svg_icons_path.write_text(_generate_era_icons_svg(theme), encoding="utf-8")

        # 3. Generate Swatch Card PNG
        swatch_path = era_dir / f"swatch_{tid}.png"
        _render_swatch_card_png(theme, swatch_path)

        # 4. Copy & analyze matching reference screenshots from `references/img/`
        copied_refs: List[Dict[str, Any]] = []
        for ref_name in theme["reference_files"]:
            src_ref = REF_IMG_DIR / ref_name
            if src_ref.exists():
                dst_ref = ref_out_dir / ref_name
                if not dst_ref.exists():
                    shutil.copy2(src_ref, dst_ref)
                sampled = _sample_dominant_colors(src_ref, count=6)
                copied_refs.append(
                    {
                        "file": ref_name,
                        "relative_path": f"{theme['era_folder']}/references/{ref_name}",
                        "sampled_dominant_hex": sampled,
                    }
                )

        # Also check for the long filename in references/img for Era 4 (Vegas Pro 23)
        if tid == "borisfx_2026_electric_dark" and REF_IMG_DIR.exists():
            for f in REF_IMG_DIR.iterdir():
                if "vegas-pro-23" in f.name.lower():
                    dst_ref = ref_out_dir / "vegas_pro_23_fx_scrolling.png"
                    if not dst_ref.exists():
                        shutil.copy2(f, dst_ref)
                    copied_refs.append(
                        {
                            "file": dst_ref.name,
                            "relative_path": f"{theme['era_folder']}/references/{dst_ref.name}",
                            "sampled_dominant_hex": _sample_dominant_colors(f, count=6),
                        }
                    )

        # 5. Write individual palette JSON
        palette_file = era_dir / f"palette_{tid}.json"
        entry = {
            **theme,
            "assets": {
                "logo_svg": f"{theme['era_folder']}/{svg_logo_path.name}",
                "logo_png": f"{theme['era_folder']}/{png_logo_path.name}",
                "icons_svg": f"{theme['era_folder']}/{svg_icons_path.name}",
                "swatch_png": f"{theme['era_folder']}/{swatch_path.name}",
                "palette_json": f"{theme['era_folder']}/{palette_file.name}",
            },
            "curated_references": copied_refs,
        }
        palette_file.write_text(json.dumps(entry, indent=2, ensure_ascii=False), encoding="utf-8")
        catalog_entries.append(entry)

    # Build combined comparison sheet across all historical themes
    comp_w, comp_h = 760, 260 * len(catalog_entries)
    comp_img = Image.new("RGB", (comp_w, comp_h), (16, 20, 28))
    for i, entry in enumerate(catalog_entries):
        sw_path = OUT_ROOT / entry["assets"]["swatch_png"]
        if sw_path.exists():
            with Image.open(sw_path) as sw:
                comp_img.paste(sw.convert("RGB"), (0, i * 260))
    comp_sheet_path = OUT_ROOT / "vegas_eras_comparison_sheet.png"
    comp_img.save(comp_sheet_path)

    # Write master `theme_catalog.json`
    catalog_path = OUT_ROOT / "theme_catalog.json"
    catalog_payload = {
        "schema_version": "2.0.0",
        "title": "Neoveg Historical VEGAS Pro Themes Catalog (1999–2026)",
        "default_theme_id": "borisfx_2026_electric_dark",
        "comparison_sheet": "vegas_eras_comparison_sheet.png",
        "themes": catalog_entries,
    }
    catalog_path.write_text(json.dumps(catalog_payload, indent=2, ensure_ascii=False), encoding="utf-8")

    # Write comprehensive Markdown documentation inside `assets/vegas_history_themes/README.md`
    readme_md = """# Acervo Histórico de Temas, Paletas, Ícones e Logotipos do VEGAS Pro (1999–2026)

Esta pasta (`assets/vegas_history_themes/`) reúne as referências visuais, paletas semânticas (`palette_*.json`), logotipos vetoriais (`.svg` e `.png`), conjuntos de ícones quadrados estilo Qt (`.svg`) e cartões de amostras (`swatch_*.png`) de todas as **4 grandes eras históricas do VEGAS Pro**, integrados diretamente ao seletor de temas dinâmicos do **Neoveg** em **C# (Avalonia UI)**.

---

## 1. Era Sonic Foundry (`01_era_sonic_foundry_v1_v3` — 1999 a 2002)
- **Versões**: Vegas Pro 1.0 (Audio), Vegas Video 2.0, Vegas Video 3.0
- **Estética**: Industrial utilitária Win32 com cinza metálico escovado (`#B8B5AE` / `#C6C3BD`), bordas chanfradas 3D (`#42413E`) e detalhes em vermelho carmim (`#CC291B`) e laranja incandescente (`#FF6A00`).
- **Ativos**:
  - `01_era_sonic_foundry_v1_v3/palette_sonic_foundry_v1_v3.json`
  - `01_era_sonic_foundry_v1_v3/logo_sonic_foundry_v1_v3.svg` / `.png`
  - `01_era_sonic_foundry_v1_v3/icons_sonic_foundry_v1_v3.svg`
  - `01_era_sonic_foundry_v1_v3/swatch_sonic_foundry_v1_v3.png`

---

## 2. Era Sony Creative Software (`02_era_sony_v4_v13` — 2003 a 2015)
Dividida em duas variantes históricas clássicas:
1. **Sony Vegas 4 a 7 — Gradiente Dourado/Laranja Flamejante (`sony_v4_v7_flaming_gold`)**:
   - Superfície clássica Luna Silver/Bege (`#ECE9D8` / `#DFDDD4`), gradiente dourado/laranja flamejante (`#FFB300` → `#FF6D00` → `#C62828`), faixas de vídeo azul-aço (`#ADC2D9`) e áudio vinho (`#7E222A`).
   - Referências extraídas: `maxresdefault.png`, `10image.png`, `11maxresdefault.png`.
2. **Sony Vegas Pro 8 a 13 — Tema Prata/Escuro Profissional (`sony_v8_v13_pro_silver_dark`)**:
   - Superfície ardósia/prata escura (`#5A5D62` / `#64676C`), faixas azul-aço (`#6B86A2`) e ameixa (`#866B74`), marcadores laranja vivo (`#F37021`) e barra de seleção azul (`#3B82F6`).
   - Referências extraídas: `image.png` (Vegas Pro 10.0), `2image.png` (DVD Architect Pro 5.2), `uploads_...Sony_Vegas_Pro.png` (Vegas Pro 12.0).

---

## 3. Era MAGIX (`03_era_magix_v14_v22` — 2016 a 2024)
Dividida em duas variantes da transição para o Flat Design:
1. **MAGIX VEGAS Pro 14 a 15 — Transição Flat Gray (`magix_v14_v15_flat_gray`)**:
   - Ícones planos monocromáticos sobre cinza neutro (`#7A7C80` / `#888A8E`) e destaques em ciano (`#0088E8`).
   - Referências extraídas: `beginner-tips-for-vegas-pro-3.png` (VEGAS Pro 14.0), `5image.png` (VEGAS Pro 15.0).
2. **MAGIX VEGAS Pro 16 a 22 — Dark Carbon & Azul Ciano (`magix_v16_v22_carbon_cyan`)**:
   - Estética escura em carvão (`#232326` / `#2D2D30`), azul ciano vibrante (`#0078D7` / `#18A0FB`), painel unificado de Color Grading e miniaturas de presets com olhar humano (`icones-vegas.png`).
   - Referências extraídas: `icones-vegas.png`, `8image.png` (VEGAS Pro 16.0), `ss_08c8...png` & `ss_64be...png` (VEGAS Pro 18.0), `color-grading-sony-vegas-8.png`, `4a839cda...png`, `filmmakers.png`.

---

## 4. Era Boris FX / Atual (`04_era_borisfx_2026` — 2025 a 2026)
- **Versões**: VEGAS Pro 23.0 / VEGAS Pro 2026 Neoveg (Integração nativa Boris FX Continuum `BCC+` e AI Smart Mask 2.0 / Z-Depth)
- **Estética**: Identidade contemporânea em preto obsidiana (`#0E1015` / `#12161F`), azul elétrico (`#00A3FF` / `#38BDF8`) e faixas multicor de alto contraste.
- **Referências extraídas**: `3image.png` (VEGAS Pro 23.0 com plugins `BCC+`), `4image.png`, `6image.png`, `7image.png`, `VEGAS_Suite_UI_202.png`.
"""
    (OUT_ROOT / "README.md").write_text(readme_md, encoding="utf-8")
    return {
        "ok": True,
        "catalog_path": str(catalog_path),
        "comparison_sheet": str(comp_sheet_path),
        "themes_count": len(catalog_entries),
    }


if __name__ == "__main__":
    res = generate_all_assets()
    print(json.dumps(res, indent=2))
