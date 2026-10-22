"""
Neoveg — Neoveg (`kdenlive_vegas_fork`) Asset Harvester, LUT/Proxy/OTIO Engine & Workspace Cleaner
Harvests and integrates all reusable components from `G:\\Meu Drive\\vp-rev-eng\\kdenlive_vegas_fork\\kdenlive_vegas`:
  1. 4 Neoveg Theme Profiles (`UltraWhite` Vegas 8, `ClassicCinza` Vegas 10–13, `JorgeTheUser` Vegas 14, `ActualBlack` Vegas 15+)
     with their exact 16-color palettes from `src/mainwindow.cpp` and `.qss` stylesheets from `data/themes/`.
  2. 4 Broadcast 3D `.cube` Color Grading LUTs (`TEAL_ORANGE.cube`, `CINEMATIC.cube`, `BLUE_TINT.cube`, `GREEN_TINT.cube`)
     from `data/luts/` for FFmpeg `lut3d` hardware/software color grading.
  3. Multi-OS Hardware Proxy & Timeline Preview Encoding Profiles (`encodingprofiles.rc`: NVENC, AMF, QSV, VAAPI, VideoToolbox, ProRes, DNxHD)
     and Camera Proxy Rules (`externalproxies.rc`: Sony PXW, GoPro LRV, DJI LRF, Insta360 LRV).
  4. Vegas Keyboard Shortcut Map (`data/shortcuts/Vegas`: `S` Cut, `D` Select, `J/L` Seek, `I/O` Mark In/Out, `V/B` Insert/Overwrite, `Z` Lift)
     and Docking Layout (`data/layouts/vegas.json`).
  5. AI & Media Python Addons (`data/scripts/`: `checkgpu.py`, `otiointerface.py`, `automask` SAM2 Smart Mask, `whisper` & `vosk` Speech-to-Text).
  6. Backs up and removes obsolete legacy folders (`inutil/`, `frontend/`, `win-x64/`, `tests/`, `__pycache__/`) into `backup_v1_anterior/legado_removido/`.
"""

from __future__ import annotations
import configparser
import json
import shutil
from pathlib import Path
from typing import Any, Dict, List

ROOT_DIR = Path(__file__).resolve().parent.parent
NEOVEG_SRC_DIR = Path(r"G:\Meu Drive\vp-rev-eng\kdenlive_vegas_fork\kdenlive_vegas")
ASSETS_LUTS_DIR = ROOT_DIR / "assets" / "luts"
ASSETS_NEOVEG_DIR = ROOT_DIR / "assets" / "neoveg_presets"
ADDONS_PY_DIR = ROOT_DIR / "python-backend" / "neoveg_addons"
BACKUP_DIR = ROOT_DIR / "backup_v1_anterior"


NEOVEG_CPP_PALETTES: List[Dict[str, Any]] = [
    {
        "id": "neoveg_ultrawhite_v8",
        "era_folder": "02_era_sony_v4_v13",
        "era_number": 2,
        "name": "Neoveg: UltraWhite (Sony Vegas 8.0 Light)",
        "short_badge": "NEOVEG • ULTRAWHITE V8",
        "years": "2007–2008",
        "vendor": "Neoveg Fork (src/mainwindow.cpp • UltraWhite.qss)",
        "versions": ["Sony Vegas Pro 8.0 (Neoveg UltraWhite)"],
        "description": "Perfil UltraWhite importado do fork Neoveg (mainwindow.cpp + UltraWhite.qss): superfície clara #F8F8F8, destaque azul #8CB8FF, faixas vídeo #5F7FCB e áudio #4F6F87.",
        "palette": {
            "window_bg": "#F8F8F8",
            "menu_bg": "#EDEDED",
            "toolbar_bg": "#EFEFEF",
            "dock_header_bg": "#D8D8D8",
            "dock_panel_bg": "#F1F1F1",
            "dock_border": "#C9C9C9",
            "timeline_bg": "#FFFFFF",
            "ruler_bg": "#EDEDED",
            "track_video_bg": "#5F7FCB",
            "track_audio_bg": "#4F6F87",
            "clip_header_bg": "#2B3A55",
            "accent_primary": "#5F7FCB",
            "accent_secondary": "#8CB8FF",
            "timecode_bg": "#1F1F1F",
            "timecode_fg": "#8DC4D8",
            "playhead_color": "#D9460F",
            "waveform_color": "#95D8AC",
            "button_bg": "#EFEFEF",
            "button_fg": "#1F1F1F",
            "text_primary": "#1F1F1F",
            "text_secondary": "#4A5568",
            "status_bar_bg": "#E2E2E2",
        },
        "reference_files": ["maxresdefault.png", "10image.png"],
    },
    {
        "id": "neoveg_classic_cinza_v10_13",
        "era_folder": "02_era_sony_v4_v13",
        "era_number": 2,
        "name": "Neoveg: ClassicCinza (Sony Vegas 10–13)",
        "short_badge": "NEOVEG • CLASSIC CINZA",
        "years": "2010–2015",
        "vendor": "Neoveg Fork (src/mainwindow.cpp • ClassicCinza.qss)",
        "versions": ["Vegas Pro 10", "Vegas Pro 11", "Vegas Pro 12", "Vegas Pro 13"],
        "description": "Perfil ClassicCinza importado do fork Neoveg (mainwindow.cpp + ClassicCinza.qss): cinza clássico #CFCFCF/#BFBFBF, realce #6F93C5, vídeo #5676B7 e áudio #4A5E73.",
        "palette": {
            "window_bg": "#CFCFCF",
            "menu_bg": "#BFBFBF",
            "toolbar_bg": "#C5C5C5",
            "dock_header_bg": "#B5B5B5",
            "dock_panel_bg": "#C3C3C3",
            "dock_border": "#9A9A9A",
            "timeline_bg": "#DADADA",
            "ruler_bg": "#BFBFBF",
            "track_video_bg": "#5676B7",
            "track_audio_bg": "#4A5E73",
            "clip_header_bg": "#253549",
            "accent_primary": "#5676B7",
            "accent_secondary": "#6F93C5",
            "timecode_bg": "#22252A",
            "timecode_fg": "#7BA7B4",
            "playhead_color": "#F37021",
            "waveform_color": "#88BB93",
            "button_bg": "#C5C5C5",
            "button_fg": "#1A1A1A",
            "text_primary": "#1A1A1A",
            "text_secondary": "#3A3A3A",
            "status_bar_bg": "#B5B5B5",
        },
        "reference_files": ["image.png"],
    },
    {
        "id": "neoveg_jorgetheuser_v14",
        "era_folder": "03_era_magix_v14_v22",
        "era_number": 3,
        "name": "Neoveg: JorgeTheUser / JustTheUser (Vegas 14)",
        "short_badge": "NEOVEG • JORGETHEUSER V14",
        "years": "2016–2017",
        "vendor": "Neoveg Fork (src/mainwindow.cpp • JorgeTheUser.qss)",
        "versions": ["VEGAS Pro 14.0 (JorgeTheUser / JustTheUser)"],
        "description": "Perfil JorgeTheUser importado do fork Neoveg (mainwindow.cpp + JorgeTheUser.qss): grafite médio #3F434B/#32363F, destaque #7895C8, vídeo #6D87CC e áudio #4D667C.",
        "palette": {
            "window_bg": "#3F434B",
            "menu_bg": "#343842",
            "toolbar_bg": "#343842",
            "dock_header_bg": "#2B2F37",
            "dock_panel_bg": "#32363F",
            "dock_border": "#2A2E36",
            "timeline_bg": "#2B2F37",
            "ruler_bg": "#232323",
            "track_video_bg": "#4D5F8E",
            "track_audio_bg": "#3D5163",
            "clip_header_bg": "#1F2329",
            "accent_primary": "#6D87CC",
            "accent_secondary": "#7895C8",
            "timecode_bg": "#232323",
            "timecode_fg": "#4F8C98",
            "playhead_color": "#57A976",
            "waveform_color": "#57A976",
            "button_bg": "#484D57",
            "button_fg": "#F0F0F0",
            "text_primary": "#ECECEC",
            "text_secondary": "#CBD5E1",
            "status_bar_bg": "#2B2F37",
        },
        "reference_files": ["beginner-tips-for-vegas-pro-3.png"],
    },
    {
        "id": "neoveg_actualblack_v15",
        "era_folder": "03_era_magix_v14_v22",
        "era_number": 3,
        "name": "Neoveg: ActualBlack (VEGAS Pro 15+ Dark)",
        "short_badge": "NEOVEG • ACTUALBLACK V15+",
        "years": "2017–2025",
        "vendor": "Neoveg Fork (src/mainwindow.cpp • ActualBlack.qss)",
        "versions": ["VEGAS Pro 15+ (ActualBlack)"],
        "description": "Perfil ActualBlack importado do fork Neoveg (mainwindow.cpp + ActualBlack.qss): superfície escura #1F2023/#16181C, destaque #5D84C9, vídeo #5F7FD1 e áudio #4D6983.",
        "palette": {
            "window_bg": "#1F2023",
            "menu_bg": "#23262C",
            "toolbar_bg": "#23262C",
            "dock_header_bg": "#20232A",
            "dock_panel_bg": "#16181C",
            "dock_border": "#17191D",
            "timeline_bg": "#151515",
            "ruler_bg": "#20232A",
            "track_video_bg": "#3A4E7A",
            "track_audio_bg": "#35495B",
            "clip_header_bg": "#101216",
            "accent_primary": "#5D84C9",
            "accent_secondary": "#4B8FA0",
            "timecode_bg": "#151515",
            "timecode_fg": "#58B57A",
            "playhead_color": "#5D84C9",
            "waveform_color": "#58B57A",
            "button_bg": "#2B2F36",
            "button_fg": "#EFEFEF",
            "text_primary": "#E8E8E8",
            "text_secondary": "#94A3B8",
            "status_bar_bg": "#1A1D22",
        },
        "reference_files": ["8image.png", "icones-vegas.png"],
    },
]


def harvest_neoveg_assets() -> Dict[str, Any]:
    """
    Copies reusable themes, 3D LUTs, shortcuts, layouts, encoding/proxy profiles, and Python AI scripts
    from `G:\\Meu Drive\\vp-rev-eng\\kdenlive_vegas_fork\\kdenlive_vegas` into Neoveg and `backup_v1_anterior/`.
    """
    ASSETS_LUTS_DIR.mkdir(parents=True, exist_ok=True)
    ASSETS_NEOVEG_DIR.mkdir(parents=True, exist_ok=True)
    (ASSETS_NEOVEG_DIR / "themes_qss").mkdir(parents=True, exist_ok=True)
    ADDONS_PY_DIR.mkdir(parents=True, exist_ok=True)

    harvest_backup = BACKUP_DIR / "neoveg_harvest_backup"
    harvest_backup.mkdir(parents=True, exist_ok=True)

    harvested: Dict[str, List[str]] = {
        "luts_3d": [],
        "themes_qss": [],
        "presets_and_layouts": [],
        "python_ai_scripts": [],
    }

    if not NEOVEG_SRC_DIR.exists():
        return {"ok": False, "reason": "kdenlive_vegas_fork not found", "harvested": harvested}

    # 1. Harvest 3D .cube LUTs (`data/luts/*.cube`)
    luts_src = NEOVEG_SRC_DIR / "data" / "luts"
    if luts_src.exists():
        for cube_file in luts_src.glob("*.cube"):
            dst = ASSETS_LUTS_DIR / cube_file.name
            shutil.copy2(cube_file, dst)
            shutil.copy2(cube_file, harvest_backup / cube_file.name)
            harvested["luts_3d"].append(cube_file.name)

    # 2. Harvest QSS Themes (`data/themes/*.qss`)
    themes_src = NEOVEG_SRC_DIR / "data" / "themes"
    if themes_src.exists():
        for qss_file in themes_src.glob("*.qss"):
            dst = ASSETS_NEOVEG_DIR / "themes_qss" / qss_file.name
            shutil.copy2(qss_file, dst)
            shutil.copy2(qss_file, harvest_backup / qss_file.name)
            harvested["themes_qss"].append(qss_file.name)

    # 3. Harvest Vegas Layout, Shortcuts, Render/Transcode Profiles, Slide Animations & Camera Proxy Profiles
    for rel_src, dst_name in [
        ("data/layouts/vegas.json", "vegas_dock_layout.json"),
        ("data/shortcuts/Vegas", "vegas_shortcuts.xml"),
        ("data/shortcuts/Premiere", "premiere_shortcuts.xml"),
        ("data/encodingprofiles.rc", "encodingprofiles.rc"),
        ("data/kdenlivetranscodingrc", "kdenlivetranscodingrc"),
        ("data/profiles.xml", "render_profiles.xml"),
        ("data/externalproxies.rc", "externalproxies.rc"),
        ("data/camcorderfilters.rc", "camcorderfilters.rc"),
        ("data/slideanimations.rc", "slideanimations.rc"),
        ("data/kdenliveeffectscategory.rc", "kdenliveeffectscategory.rc"),
        ("data/preferred_effects.txt", "preferred_effects.txt"),
        ("src/scopes/colorscopes/colorconstants.h", "colorconstants.h"),
        ("README_PT-BR.md", "NEOVEG_README_PT-BR.md"),
    ]:
        src_p = NEOVEG_SRC_DIR / rel_src
        if src_p.exists():
            dst_p = ASSETS_NEOVEG_DIR / dst_name
            shutil.copy2(src_p, dst_p)
            shutil.copy2(src_p, harvest_backup / dst_name)
            harvested["presets_and_layouts"].append(dst_name)

    # 3B. Harvest Transition WebP Previews (`data/transitions/previews/*.webp`) & Generator XMLs (`data/generators/*.xml`)
    trans_prev_src = NEOVEG_SRC_DIR / "data" / "transitions" / "previews"
    if trans_prev_src.exists():
        dst_prev_dir = ASSETS_NEOVEG_DIR / "transition_previews"
        dst_prev_dir.mkdir(parents=True, exist_ok=True)
        harvested["transition_previews"] = []
        for webp in trans_prev_src.glob("*.webp"):
            shutil.copy2(webp, dst_prev_dir / webp.name)
            harvested["transition_previews"].append(webp.name)

    gen_xml_src = NEOVEG_SRC_DIR / "data" / "generators"
    if gen_xml_src.exists():
        dst_gen_dir = ASSETS_NEOVEG_DIR / "generators"
        dst_gen_dir.mkdir(parents=True, exist_ok=True)
        harvested["generators_xml"] = []
        for gxml in gen_xml_src.glob("*.xml"):
            shutil.copy2(gxml, dst_gen_dir / gxml.name)
            harvested["generators_xml"].append(gxml.name)

    # 4. Harvest Python AI / GPU / OTIO Scripts (`data/scripts/`)
    scripts_src = NEOVEG_SRC_DIR / "data" / "scripts"
    if scripts_src.exists():
        for py_file in scripts_src.rglob("*.py"):
            rel = py_file.relative_to(scripts_src)
            dst_py = ADDONS_PY_DIR / rel
            dst_py.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(py_file, dst_py)
            harvested["python_ai_scripts"].append(str(rel).replace("\\", "/"))

    # Write manifest of harvested Neoveg assets
    manifest_file = ASSETS_NEOVEG_DIR / "neoveg_harvest_manifest.json"
    manifest_file.write_text(
        json.dumps(
            {
                "source_repository": str(NEOVEG_SRC_DIR),
                "harvested": harvested,
                "neoveg_themes": [t["id"] for t in NEOVEG_CPP_PALETTES],
            },
            indent=2,
            ensure_ascii=False,
        ),
        encoding="utf-8",
    )
    return {"ok": True, "harvested": harvested, "manifest": str(manifest_file)}


def backup_and_remove_unnecessary_project_files() -> Dict[str, Any]:
    """
    Moves obsolete legacy directories (`inutil`, `frontend`, `win-x64`, `tests`, `__pycache__`)
    from the workspace root into `backup_v1_anterior/legado_removido/` so the workspace stays clean
    with a 100% complete backup.
    """
    legacy_backup_root = BACKUP_DIR / "legado_removido"
    legacy_backup_root.mkdir(parents=True, exist_ok=True)

    obsolete_targets = [
        "inutil",
        "frontend",
        "win-x64",
        "tests",
        "__pycache__",
    ]

    moved: List[str] = []
    for folder_name in obsolete_targets:
        src_folder = ROOT_DIR / folder_name
        if src_folder.exists():
            dst_folder = legacy_backup_root / folder_name
            if dst_folder.exists():
                shutil.rmtree(dst_folder, ignore_errors=True)
            shutil.copytree(src_folder, dst_folder, dirs_exist_ok=True)
            shutil.rmtree(src_folder, ignore_errors=True)
            moved.append(folder_name)

    return {
        "ok": True,
        "backed_up_to": str(legacy_backup_root),
        "removed_from_root": moved,
    }


def list_bundled_3d_luts() -> List[Dict[str, str]]:
    """Returns all harvested `.cube` 3D LUT files available in `assets/luts/`."""
    out: List[Dict[str, str]] = []
    if ASSETS_LUTS_DIR.exists():
        for f in sorted(ASSETS_LUTS_DIR.glob("*.cube")):
            out.append(
                {
                    "id": f"lut3d.{f.stem.lower()}",
                    "name": f"3D LUT: {f.stem.replace('_', ' ').title()} (.cube)",
                    "file": f.name,
                    "path": str(f.resolve()),
                }
            )
    return out


def load_neoveg_encoding_profiles() -> Dict[str, Dict[str, str]]:
    """Parses `assets/neoveg_presets/encodingprofiles.rc` into structured FFmpeg proxy/preview profiles."""
    rc_path = ASSETS_NEOVEG_DIR / "encodingprofiles.rc"
    if not rc_path.exists():
        return {}
    parser = configparser.RawConfigParser()
    parser.optionxform = str  # preserve case
    parser.read(rc_path, encoding="utf-8")
    return {sec: dict(parser.items(sec)) for sec in parser.sections()}


def export_project_to_otio(project: Dict[str, Any], out_path: Path) -> Path:
    """
    Exports a Neoveg `.veg` project dictionary to an OpenTimelineIO (`.otio`) compatible JSON file
    (inspired by `kdenlive_vegas/data/scripts/otiointerface.py`).
    """
    fps = float(project.get("fps", 29.97))
    otio_tracks: List[Dict[str, Any]] = []
    for tr in project.get("tracks", []):
        children: List[Dict[str, Any]] = []
        for clip in tr.get("clips", []):
            dur_frames = int(round(float(clip.get("duration", 1.0)) * fps))
            trim_frames = int(round(float(clip.get("trimIn", 0.0)) * fps))
            children.append(
                {
                    "OTIO_SCHEMA": "Clip.2",
                    "name": clip.get("name", "Clip"),
                    "source_range": {
                        "OTIO_SCHEMA": "TimeRange.1",
                        "start_time": {"OTIO_SCHEMA": "RationalTime.1", "value": trim_frames, "rate": fps},
                        "duration": {"OTIO_SCHEMA": "RationalTime.1", "value": dur_frames, "rate": fps},
                    },
                    "media_reference": {
                        "OTIO_SCHEMA": "ExternalReference.1",
                        "target_url": clip.get("mediaFile") or f"generator://{clip.get('generatorType', 'solid')}",
                    },
                }
            )
        otio_tracks.append(
            {
                "OTIO_SCHEMA": "Track.1",
                "name": tr.get("name", "Track"),
                "kind": "Video" if tr.get("type") == "video" else "Audio",
                "children": children,
            }
        )

    otio_doc = {
        "OTIO_SCHEMA": "Timeline.1",
        "name": project.get("name", "Neoveg_Timeline"),
        "global_start_time": {"OTIO_SCHEMA": "RationalTime.1", "value": 0, "rate": fps},
        "tracks": {
            "OTIO_SCHEMA": "Stack.1",
            "name": "tracks",
            "children": otio_tracks,
        },
    }
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(otio_doc, indent=2, ensure_ascii=False), encoding="utf-8")
    return out_path


if __name__ == "__main__":
    h_res = harvest_neoveg_assets()
    c_res = backup_and_remove_unnecessary_project_files()
    print(json.dumps({"harvest": h_res, "cleanup": c_res, "luts": list_bundled_3d_luts()}, indent=2))
