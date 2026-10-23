"""
Neovegject Interchange & Native `.neoveg` Container Engine (`neoveg` Python Backend).
Mirrors `Neoveg.ProjectInterchange` in C#:
1. Official `.neoveg` Native Container (ZIP archive containing `project.json`, `media_pool.json`, `history.json`).
2. Legacy Neoveg `.veg` Import via Rust FFI (`neoveg_inspect_veg_container`).
3. Final Cut Pro 7 / Adobe Premiere Pro / DaVinci Resolve XML (`.xml` — `xmeml version="5"`).
4. Final Cut Pro X / DaVinci Resolve Structured XML (`.fcpxml` — `fcpxml version="1.10"`).
5. CMX3600 Edit Decision List (`.edl`).
"""

from __future__ import annotations

import hashlib
import json
import math
import os
import urllib.parse
import xml.etree.ElementTree as ET
import zipfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple


# =============================================================================
# 1. NATIVE `.neoveg` CONTAINER (ZIP: project.json + media_pool.json + history.json)
# =============================================================================

def save_neoveg_container(
    project: Dict[str, Any],
    target_path: str | Path,
    workspace_root: Optional[str | Path] = None,
) -> Path:
    """
    Saves a Neovegject into a `.neoveg` ZIP container housing:
    - `project.json`: API version, metadata (resolution, FPS, color space, audio sample rate),
      tracks, groups, markers, regions, and timeline events.
    - `media_pool.json`: Absolute and relative media paths, GUIDs, and cache states.
    - `history.json`: Undo/Redo action stack.
    """
    out_path = Path(target_path)
    if out_path.suffix.lower() != ".neoveg":
        out_path = out_path.with_suffix(".neoveg")
    out_path.parent.mkdir(parents=True, exist_ok=True)

    base_dir = Path(workspace_root) if workspace_root else out_path.parent
    now_iso = datetime.now(timezone.utc).isoformat()

    proj_doc = dict(project)
    proj_doc.setdefault("apiVersion", "2.1.0")
    proj_doc.setdefault("format", "neoveg-container")
    proj_doc.setdefault("width", 1920)
    proj_doc.setdefault("height", 1080)
    proj_doc.setdefault("fps", 29.97)
    proj_doc.setdefault("colorSpace", "Rec.709 Standard")
    proj_doc.setdefault("sampleRate", 48000)
    proj_doc.setdefault("audioBitDepth", 24)
    proj_doc.setdefault("tracks", [])
    proj_doc.setdefault("markers", [])
    proj_doc.setdefault("regions", [])

    # Build media_pool.json
    media_items: List[Dict[str, Any]] = []
    seen_keys = set()

    for m in proj_doc.get("mediaPool", []):
        m_id = str(m.get("id") or m.get("guid") or m.get("name") or "")
        abs_path = str(m.get("filePath") or m.get("absolutePath") or m.get("fileName") or "")
        rel_path = _try_relpath(abs_path, base_dir)
        sfk_path = f"{abs_path}.sfk" if abs_path else ""
        media_items.append({
            "guid": _deterministic_guid(f"{m_id}|{abs_path}"),
            "id": m_id,
            "name": m.get("name") or Path(abs_path).name,
            "fileName": m.get("fileName") or (Path(abs_path).name if abs_path else ""),
            "absolutePath": abs_path,
            "relativePath": rel_path,
            "mediaType": m.get("type") or m.get("mediaType") or "video",
            "duration": float(m.get("duration", 5.0)),
            "width": int(m.get("width", proj_doc["width"])),
            "height": int(m.get("height", proj_doc["height"])),
            "fps": float(m.get("fps", proj_doc["fps"])),
            "sampleRate": int(m.get("sampleRate", proj_doc["sampleRate"])),
            "cacheState": "offline" if m.get("isOffline") else ("sfk_cached" if (sfk_path and os.path.exists(sfk_path)) else "cached"),
            "sfkCachePath": sfk_path,
            "isOffline": bool(m.get("isOffline", False)),
        })
        seen_keys.add(m_id)

    for tr in proj_doc.get("tracks", []):
        for c in tr.get("clips", []):
            key = str(c.get("mediaId") or c.get("mediaFile") or c.get("id") or "")
            if key in seen_keys:
                continue
            seen_keys.add(key)
            abs_path = str(c.get("mediaFile") or "")
            rel_path = _try_relpath(abs_path, base_dir)
            sfk_path = f"{abs_path}.sfk" if abs_path else ""
            media_items.append({
                "guid": _deterministic_guid(key),
                "id": str(c.get("mediaId") or c.get("id")),
                "name": str(c.get("name", "Clip")),
                "fileName": Path(abs_path).name if abs_path else str(c.get("name", "Clip")),
                "absolutePath": abs_path,
                "relativePath": rel_path,
                "mediaType": str(c.get("mediaType") or tr.get("type") or "video"),
                "duration": float(c.get("duration", 5.0)),
                "width": int(proj_doc["width"]),
                "height": int(proj_doc["height"]),
                "fps": float(proj_doc["fps"]),
                "sampleRate": int(proj_doc["sampleRate"]),
                "cacheState": "offline" if c.get("isOffline") else ("sfk_cached" if (sfk_path and os.path.exists(sfk_path)) else "timeline_cached"),
                "sfkCachePath": sfk_path,
                "isOffline": bool(c.get("isOffline", False)),
            })

    media_pool_doc = {
        "schemaVersion": proj_doc["apiVersion"],
        "savedAtUtc": now_iso,
        "items": media_items,
    }

    undo_stack = list(proj_doc.get("undoStack") or [])
    if not undo_stack:
        undo_stack.append({
            "id": "act_save_01",
            "timestampUtc": now_iso,
            "actionType": "SaveNeoveg",
            "description": f"Projeto '{proj_doc.get('name', 'Neoveg')}' salvo no formato nativo .neoveg",
        })
    redo_stack = list(proj_doc.get("redoStack") or [])

    history_doc = {
        "schemaVersion": proj_doc["apiVersion"],
        "savedAtUtc": now_iso,
        "undoStack": undo_stack,
        "redoStack": redo_stack,
    }

    with zipfile.ZipFile(out_path, mode="w", compression=zipfile.ZIP_DEFLATED) as zf:
        zf.writestr("project.json", json.dumps(proj_doc, indent=2, ensure_ascii=False))
        zf.writestr("media_pool.json", json.dumps(media_pool_doc, indent=2, ensure_ascii=False))
        zf.writestr("history.json", json.dumps(history_doc, indent=2, ensure_ascii=False))

    return out_path


def load_neoveg_container(file_path: str | Path) -> Tuple[Dict[str, Any], Dict[str, Any], Dict[str, Any]]:
    """
    Loads a `.neoveg` container and returns `(project_doc, media_pool_doc, history_doc)`.
    """
    fp = Path(file_path)
    if not fp.exists():
        raise FileNotFoundError(f"Arquivo .neoveg não encontrado: {fp}")

    if zipfile.is_zipfile(fp):
        with zipfile.ZipFile(fp, mode="r") as zf:
            names = set(zf.namelist())
            if "project.json" not in names:
                raise ValueError(f"Contêiner .neoveg inválido ('project.json' ausente): {fp}")
            project_doc = json.loads(zf.read("project.json").decode("utf-8"))
            media_pool_doc = (
                json.loads(zf.read("media_pool.json").decode("utf-8"))
                if "media_pool.json" in names
                else {"schemaVersion": "2.1.0", "items": []}
            )
            history_doc = (
                json.loads(zf.read("history.json").decode("utf-8"))
                if "history.json" in names
                else {"schemaVersion": "2.1.0", "undoStack": [], "redoStack": []}
            )
            project_doc["undoStack"] = history_doc.get("undoStack", [])
            project_doc["redoStack"] = history_doc.get("redoStack", [])
            return project_doc, media_pool_doc, history_doc

    # Fallback for plain JSON .neoveg
    project_doc = json.loads(fp.read_text(encoding="utf-8"))
    return project_doc, {"schemaVersion": "2.1.0", "items": project_doc.get("mediaPool", [])}, {
        "schemaVersion": "2.1.0",
        "undoStack": project_doc.get("undoStack", []),
        "redoStack": project_doc.get("redoStack", []),
    }


# =============================================================================
# 2. FCP7 XML (`.xml` — Final Cut Pro 7 / Premiere Pro / DaVinci Resolve `xmeml v5`)
# =============================================================================

def export_fcp7_xml_to_string(project: Dict[str, Any]) -> str:
    fps = float(project.get("fps") or 29.97)
    timebase, ntsc = _get_fcp7_rate(fps)
    ntsc_str = "TRUE" if ntsc else "FALSE"
    width = int(project.get("width") or 1920)
    height = int(project.get("height") or 1080)
    sample_rate = int(project.get("sampleRate") or 48000)
    bit_depth = int(project.get("audioBitDepth") or 24)
    color_space = str(project.get("colorSpace") or "Rec.709 Standard")
    seq_name = str(project.get("name") or "Neoveg_Sequence")

    total_dur_sec = _compute_project_duration(project)
    seq_dur_frames = _sec_to_frames(total_dur_sec, fps)

    root = ET.Element("xmeml", {"version": "5"})
    seq = ET.SubElement(root, "sequence", {"id": f"seq_{seq_name}"})
    ET.SubElement(seq, "name").text = seq_name
    ET.SubElement(seq, "duration").text = str(seq_dur_frames)

    rate_el = ET.SubElement(seq, "rate")
    ET.SubElement(rate_el, "timebase").text = str(timebase)
    ET.SubElement(rate_el, "ntsc").text = ntsc_str

    tc_el = ET.SubElement(seq, "timecode")
    tc_rate = ET.SubElement(tc_el, "rate")
    ET.SubElement(tc_rate, "timebase").text = str(timebase)
    ET.SubElement(tc_rate, "ntsc").text = ntsc_str
    ET.SubElement(tc_el, "string").text = "00:00:00:00"
    ET.SubElement(tc_el, "frame").text = "0"
    ET.SubElement(tc_el, "displayformat").text = "DF" if ntsc else "NDF"

    media_el = ET.SubElement(seq, "media")
    video_el = ET.SubElement(media_el, "video")
    v_fmt = ET.SubElement(video_el, "format")
    v_sc = ET.SubElement(v_fmt, "samplecharacteristics")
    v_rate = ET.SubElement(v_sc, "rate")
    ET.SubElement(v_rate, "timebase").text = str(timebase)
    ET.SubElement(v_rate, "ntsc").text = ntsc_str
    ET.SubElement(v_sc, "width").text = str(width)
    ET.SubElement(v_sc, "height").text = str(height)
    ET.SubElement(v_sc, "anamorphic").text = "FALSE"
    ET.SubElement(v_sc, "pixelaspectratio").text = "square"
    ET.SubElement(v_sc, "fielddominance").text = "none"
    ET.SubElement(v_sc, "colordepth").text = "24"
    ET.SubElement(v_sc, "colorSpace").text = color_space

    audio_el = ET.SubElement(media_el, "audio")
    ET.SubElement(audio_el, "numOutputChannels").text = "2"
    a_fmt = ET.SubElement(audio_el, "format")
    a_sc = ET.SubElement(a_fmt, "samplecharacteristics")
    ET.SubElement(a_sc, "depth").text = str(bit_depth)
    ET.SubElement(a_sc, "samplerate").text = str(sample_rate)

    for tr in project.get("tracks", []):
        is_audio = str(tr.get("type", "video")).lower() == "audio"
        parent_sec = audio_el if is_audio else video_el

        tr_attrs = {
            "MZ.TrackName": str(tr.get("name", "Track")),
            "neovegId": str(tr.get("id", "tr_1")),
            "neovegColor": str(tr.get("color", "#2563eb")),
            "neovegHeight": f"{float(tr.get('height', 78.0)):.2f}",
            "neovegLevel": f"{float(tr.get('level', 1.0)):.6f}",
            "neovegPan": f"{float(tr.get('pan', 0.0)):.6f}",
            "neovegBlendMode": str(tr.get("blendMode") or "sourceAlpha"),
            "solo": "TRUE" if tr.get("solo") else "FALSE",
        }
        if tr.get("groupId"):
            tr_attrs["groupId"] = str(tr["groupId"])
        if tr.get("groupName"):
            tr_attrs["groupName"] = str(tr["groupName"])
        if tr.get("groupColor"):
            tr_attrs["groupColor"] = str(tr["groupColor"])

        tr_el = ET.SubElement(parent_sec, "track", tr_attrs)
        ET.SubElement(tr_el, "name").text = str(tr.get("name", "Track"))
        ET.SubElement(tr_el, "enabled").text = "FALSE" if tr.get("muted") else "TRUE"
        ET.SubElement(tr_el, "locked").text = "TRUE" if tr.get("locked") else "FALSE"

        sorted_clips = sorted(tr.get("clips", []), key=lambda x: float(x.get("start", 0.0)))
        for idx, c in enumerate(sorted_clips):
            start_sec = float(c.get("start", 0.0))
            dur_sec = float(c.get("duration", 5.0))
            end_sec = start_sec + dur_sec
            trim_in_sec = float(c.get("trimIn", 0.0))
            trim_out_sec = trim_in_sec + dur_sec
            opacity = float(c.get("opacity", 1.0))
            gain = float(c.get("gain", 1.0))

            start_fr = _sec_to_frames(start_sec, fps)
            end_fr = _sec_to_frames(end_sec, fps)
            in_fr = _sec_to_frames(trim_in_sec, fps)
            out_fr = _sec_to_frames(trim_out_sec, fps)
            dur_fr = max(1, out_fr - in_fr)

            c_id = str(c.get("id") or f"clip_{idx + 1}")
            media_file = str(c.get("mediaFile") or c.get("name") or "media.mp4")
            file_id = str(c.get("mediaId") or f"file_{c_id}")

            clip_attrs = {
                "id": c_id,
                "neovegStartSec": f"{start_sec:.6f}",
                "neovegDurationSec": f"{dur_sec:.6f}",
                "neovegTrimInSec": f"{trim_in_sec:.6f}",
                "neovegFadeInSec": f"{float(c.get('fadeIn', 0.25)):.6f}",
                "neovegFadeOutSec": f"{float(c.get('fadeOut', 0.25)):.6f}",
                "neovegPlaybackRate": f"{float(c.get('playbackRate', 1.0)):.4f}",
                "neovegColor": str(c.get("color") or tr.get("color") or "#2563eb"),
                "neovegMediaType": str(c.get("mediaType") or ("audio" if is_audio else "video")),
            }
            if c.get("generatorType"):
                clip_attrs["neovegGeneratorType"] = str(c["generatorType"])
            if c.get("titleText"):
                clip_attrs["neovegTitleText"] = str(c["titleText"])

            clip_el = ET.SubElement(tr_el, "clipitem", clip_attrs)
            ET.SubElement(clip_el, "name").text = str(c.get("name", "Clip"))
            ET.SubElement(clip_el, "enabled").text = "FALSE" if c.get("muted") else "TRUE"
            ET.SubElement(clip_el, "duration").text = str(dur_fr)

            c_rate = ET.SubElement(clip_el, "rate")
            ET.SubElement(c_rate, "timebase").text = str(timebase)
            ET.SubElement(c_rate, "ntsc").text = ntsc_str

            ET.SubElement(clip_el, "start").text = str(start_fr)
            ET.SubElement(clip_el, "end").text = str(end_fr)
            ET.SubElement(clip_el, "in").text = str(in_fr)
            ET.SubElement(clip_el, "out").text = str(out_fr)

            file_el = ET.SubElement(clip_el, "file", {"id": file_id})
            ET.SubElement(file_el, "name").text = Path(media_file).name
            ET.SubElement(file_el, "pathurl").text = _to_file_url(media_file)
            f_rate = ET.SubElement(file_el, "rate")
            ET.SubElement(f_rate, "timebase").text = str(timebase)
            ET.SubElement(f_rate, "ntsc").text = ntsc_str
            ET.SubElement(file_el, "duration").text = str(max(dur_fr, out_fr))

            # Opacity filter (0..100)
            flt_op = ET.SubElement(clip_el, "filter")
            eff_op = ET.SubElement(flt_op, "effect")
            ET.SubElement(eff_op, "name").text = "Basic Motion / Opacity"
            ET.SubElement(eff_op, "effectid").text = "opacity"
            ET.SubElement(eff_op, "effectcategory").text = "motion"
            ET.SubElement(eff_op, "effecttype").text = "motion"
            ET.SubElement(eff_op, "mediatype").text = "video"
            prm_op = ET.SubElement(eff_op, "parameter")
            ET.SubElement(prm_op, "parameterid").text = "opacity"
            ET.SubElement(prm_op, "name").text = "opacity"
            ET.SubElement(prm_op, "valuemin").text = "0"
            ET.SubElement(prm_op, "valuemax").text = "100"
            ET.SubElement(prm_op, "value").text = f"{opacity * 100.0:.4f}"

            # Audio Level filter
            gain_db = 20.0 * math.log10(gain) if gain > 0.0001 else -96.0
            flt_au = ET.SubElement(clip_el, "filter")
            eff_au = ET.SubElement(flt_au, "effect")
            ET.SubElement(eff_au, "name").text = "Audio Levels"
            ET.SubElement(eff_au, "effectid").text = "audiolevels"
            ET.SubElement(eff_au, "effectcategory").text = "audiolevels"
            ET.SubElement(eff_au, "effecttype").text = "audiolevels"
            ET.SubElement(eff_au, "mediatype").text = "audio"
            prm_au = ET.SubElement(eff_au, "parameter")
            ET.SubElement(prm_au, "parameterid").text = "level"
            ET.SubElement(prm_au, "name").text = "Level"
            ET.SubElement(prm_au, "valuemin").text = "0"
            ET.SubElement(prm_au, "valuemax").text = "4"
            ET.SubElement(prm_au, "value").text = f"{gain:.6f}"
            ET.SubElement(prm_au, "valueDb").text = f"{gain_db:.3f}"

            # Crossfade transition item if overlapping with next clip
            if idx + 1 < len(sorted_clips):
                nxt = sorted_clips[idx + 1]
                nxt_start = float(nxt.get("start", 0.0))
                nxt_end = nxt_start + float(nxt.get("duration", 5.0))
                if end_sec > nxt_start + 0.001:
                    tr_item = ET.SubElement(tr_el, "transitionitem")
                    tr_r = ET.SubElement(tr_item, "rate")
                    ET.SubElement(tr_r, "timebase").text = str(timebase)
                    ET.SubElement(tr_r, "ntsc").text = ntsc_str
                    ET.SubElement(tr_item, "start").text = str(_sec_to_frames(nxt_start, fps))
                    ET.SubElement(tr_item, "end").text = str(_sec_to_frames(min(end_sec, nxt_end), fps))
                    ET.SubElement(tr_item, "alignment").text = "center"
                    tr_eff = ET.SubElement(tr_item, "effect")
                    ET.SubElement(tr_eff, "name").text = "Cross Dissolve"
                    ET.SubElement(tr_eff, "effectid").text = "Cross Dissolve"
                    ET.SubElement(tr_eff, "effectcategory").text = "Dissolve"
                    ET.SubElement(tr_eff, "effecttype").text = "transition"
                    ET.SubElement(tr_eff, "mediatype").text = "audio" if is_audio else "video"

    for mk in project.get("markers", []):
        mk_time = float(mk.get("time", 0.0))
        mk_el = ET.SubElement(
            seq,
            "marker",
            {
                "neovegTimeSec": f"{mk_time:.6f}",
                "neovegColor": str(mk.get("color") or "#f97316"),
            },
        )
        ET.SubElement(mk_el, "name").text = str(mk.get("label", "Marker"))
        ET.SubElement(mk_el, "comment").text = str(mk.get("label", "Marker"))
        ET.SubElement(mk_el, "in").text = str(_sec_to_frames(mk_time, fps))
        ET.SubElement(mk_el, "out").text = "-1"

    ET.indent(root, space="  ")
    body = ET.tostring(root, encoding="unicode")
    return f'<?xml version="1.0" encoding="UTF-8"?>\n<!DOCTYPE xmeml>\n{body}\n'


def export_fcp7_xml(project: Dict[str, Any], output_path: str | Path) -> Path:
    out = Path(output_path)
    if out.suffix.lower() != ".xml":
        out = out.with_suffix(".xml")
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(export_fcp7_xml_to_string(project), encoding="utf-8")
    return out


def import_fcp7_xml_from_string(xml_content: str) -> Dict[str, Any]:
    root = ET.fromstring(xml_content)
    seq = root.find(".//sequence")
    if seq is None:
        raise ValueError("XML FCP7 inválido: elemento <sequence> não encontrado.")

    seq_name = seq.findtext("name") or "Imported_FCP7_Sequence"
    rate_el = seq.find("rate")
    timebase = int(rate_el.findtext("timebase") or "30") if rate_el is not None else 30
    ntsc = (rate_el.findtext("ntsc") or "FALSE").upper() == "TRUE" if rate_el is not None else False
    fps = _resolve_fps_from_fcp7(timebase, ntsc)

    v_sc = seq.find("./media/video/format/samplecharacteristics")
    width = int(v_sc.findtext("width") or "1920") if v_sc is not None else 1920
    height = int(v_sc.findtext("height") or "1080") if v_sc is not None else 1080
    color_space = (v_sc.findtext("colorSpace") or "Rec.709 Standard") if v_sc is not None else "Rec.709 Standard"

    a_sc = seq.find("./media/audio/format/samplecharacteristics")
    sample_rate = int(a_sc.findtext("samplerate") or "48000") if a_sc is not None else 48000
    bit_depth = int(a_sc.findtext("depth") or "24") if a_sc is not None else 24

    tracks: List[Dict[str, Any]] = []
    video_sec = seq.find("./media/video")
    if video_sec is not None:
        for idx, tr_el in enumerate(video_sec.findall("track"), start=1):
            tracks.append(_parse_fcp7_track(tr_el, "video", idx, fps))

    audio_sec = seq.find("./media/audio")
    if audio_sec is not None:
        for idx, tr_el in enumerate(audio_sec.findall("track"), start=1):
            tracks.append(_parse_fcp7_track(tr_el, "audio", idx, fps))

    markers: List[Dict[str, Any]] = []
    for mk_el in seq.findall("marker"):
        if "neovegTimeSec" in mk_el.attrib:
            t_sec = float(mk_el.attrib["neovegTimeSec"])
        else:
            t_sec = _frames_to_sec(int(mk_el.findtext("in") or "0"), fps)
        markers.append({
            "time": round(t_sec, 6),
            "label": mk_el.findtext("name") or mk_el.findtext("comment") or "Marker",
            "color": mk_el.attrib.get("neovegColor", "#f97316"),
        })

    return {
        "apiVersion": "2.1.0",
        "name": seq_name,
        "width": width,
        "height": height,
        "fps": fps,
        "colorSpace": color_space,
        "sampleRate": sample_rate,
        "audioBitDepth": bit_depth,
        "tracks": tracks,
        "markers": markers,
        "regions": [],
        "mediaPool": [],
    }


def import_fcp7_xml(xml_path: str | Path) -> Dict[str, Any]:
    return import_fcp7_xml_from_string(Path(xml_path).read_text(encoding="utf-8"))


def _parse_fcp7_track(tr_el: ET.Element, track_type: str, index: int, fps: float) -> Dict[str, Any]:
    name = tr_el.findtext("name") or tr_el.attrib.get("MZ.TrackName") or f"{track_type.title()} Track {index}"
    enabled = (tr_el.findtext("enabled") or "TRUE").upper() != "FALSE"
    locked = (tr_el.findtext("locked") or "FALSE").upper() == "TRUE"
    solo = tr_el.attrib.get("solo", "FALSE").upper() == "TRUE"

    clips: List[Dict[str, Any]] = []
    transitions: List[Dict[str, Any]] = []

    for tr_item in tr_el.findall("transitionitem"):
        tr_s = _frames_to_sec(int(tr_item.findtext("start") or "0"), fps)
        tr_e = _frames_to_sec(int(tr_item.findtext("end") or "0"), fps)
        transitions.append({
            "name": tr_item.findtext("./effect/name") or "Cross Dissolve",
            "start": round(tr_s, 6),
            "end": round(tr_e, 6),
            "duration": round(max(0.0, tr_e - tr_s), 6),
        })

    for c_el in tr_el.findall("clipitem"):
        start_fr = int(c_el.findtext("start") or "0")
        end_fr = int(c_el.findtext("end") or str(start_fr + 30))
        in_fr = int(c_el.findtext("in") or "0")
        out_fr = int(c_el.findtext("out") or str(in_fr + max(1, end_fr - start_fr)))

        start_sec = (
            float(c_el.attrib["neovegStartSec"])
            if "neovegStartSec" in c_el.attrib
            else _frames_to_sec(start_fr, fps)
        )
        dur_sec = (
            float(c_el.attrib["neovegDurationSec"])
            if "neovegDurationSec" in c_el.attrib
            else _frames_to_sec(max(1, end_fr - start_fr), fps)
        )
        trim_in_sec = (
            float(c_el.attrib["neovegTrimInSec"])
            if "neovegTrimInSec" in c_el.attrib
            else _frames_to_sec(in_fr, fps)
        )

        opacity = 1.0
        gain = 1.0
        for flt in c_el.findall("filter"):
            eff = flt.find("effect")
            if eff is None:
                continue
            eff_id = (eff.findtext("effectid") or "").lower()
            if eff_id == "opacity":
                for prm in eff.findall("parameter"):
                    if (prm.findtext("parameterid") or "").lower() == "opacity":
                        opacity = max(0.0, min(1.0, float(prm.findtext("value") or "100.0") / 100.0))
            elif eff_id == "audiolevels":
                for prm in eff.findall("parameter"):
                    if (prm.findtext("parameterid") or "").lower() == "level":
                        gain = max(0.0, min(4.0, float(prm.findtext("value") or "1.0")))

        file_el = c_el.find("file")
        path_url = file_el.findtext("pathurl") if file_el is not None else None
        media_file = _from_file_url(path_url) or (file_el.findtext("name") if file_el is not None else None)
        c_enabled = (c_el.findtext("enabled") or "TRUE").upper() != "FALSE"

        clips.append({
            "id": c_el.attrib.get("id", f"clip_{len(clips) + 1}"),
            "name": c_el.findtext("name") or "Clip",
            "mediaId": file_el.attrib.get("id", "") if file_el is not None else "",
            "mediaFile": media_file,
            "mediaType": c_el.attrib.get("neovegMediaType", track_type),
            "generatorType": c_el.attrib.get("neovegGeneratorType"),
            "titleText": c_el.attrib.get("neovegTitleText"),
            "color": c_el.attrib.get("neovegColor", "#2563eb"),
            "start": round(start_sec, 6),
            "duration": round(dur_sec, 6),
            "end": round(start_sec + dur_sec, 6),
            "trimIn": round(trim_in_sec, 6),
            "outPoint": round(trim_in_sec + dur_sec, 6),
            "fadeIn": float(c_el.attrib.get("neovegFadeInSec", 0.25)),
            "fadeOut": float(c_el.attrib.get("neovegFadeOutSec", 0.25)),
            "playbackRate": float(c_el.attrib.get("neovegPlaybackRate", 1.0)),
            "opacity": round(opacity, 4),
            "gain": round(gain, 6),
            "muted": not c_enabled,
        })

    return {
        "id": tr_el.attrib.get("neovegId", f"tr_{track_type[0]}{index}"),
        "name": name,
        "type": track_type,
        "color": tr_el.attrib.get("neovegColor", "#0891b2" if track_type == "audio" else "#2563eb"),
        "height": float(tr_el.attrib.get("neovegHeight", 78.0)),
        "level": float(tr_el.attrib.get("neovegLevel", 1.0)),
        "pan": float(tr_el.attrib.get("neovegPan", 0.0)),
        "blendMode": tr_el.attrib.get("neovegBlendMode", "sourceAlpha"),
        "groupId": tr_el.attrib.get("groupId"),
        "groupName": tr_el.attrib.get("groupName"),
        "groupColor": tr_el.attrib.get("groupColor"),
        "muted": not enabled,
        "solo": solo,
        "locked": locked,
        "clips": clips,
        "transitions": transitions,
    }


# =============================================================================
# 3. FCPX FCPXML (`.fcpxml` — Final Cut Pro X / DaVinci Resolve `fcpxml v1.10`)
# =============================================================================

def export_fcpxml_to_string(project: Dict[str, Any]) -> str:
    fps = float(project.get("fps") or 29.97)
    width = int(project.get("width") or 1920)
    height = int(project.get("height") or 1080)
    sample_rate = int(project.get("sampleRate") or 48000)
    seq_name = str(project.get("name") or "Neoveg_FCPX_Project")
    frame_dur = "1001/30000s" if abs(fps - 29.97) < 0.05 else f"100/{int(round(fps * 100))}s"

    root = ET.Element("fcpxml", {"version": "1.10"})
    resources = ET.SubElement(root, "resources")
    ET.SubElement(
        resources,
        "format",
        {
            "id": "r1",
            "name": f"FFVideoFormat{height}p{int(round(fps))}",
            "frameDuration": frame_dur,
            "width": str(width),
            "height": str(height),
            "colorSpace": "1-1-1 (Rec. 709)",
        },
    )

    library = ET.SubElement(root, "library")
    event = ET.SubElement(library, "event", {"name": "Neoveg Export"})
    proj_el = ET.SubElement(event, "project", {"name": seq_name})
    seq_el = ET.SubElement(
        proj_el,
        "sequence",
        {
            "format": "r1",
            "duration": _format_fcpx_rational(_compute_project_duration(project)),
            "tcStart": "0s",
            "tcFormat": "NDF",
            "audioLayout": "stereo",
            "audioRate": f"{sample_rate // 1000}k",
            "neovegFps": f"{fps:.4f}",
        },
    )
    spine = ET.SubElement(seq_el, "spine")

    asset_counter = 2
    for t_idx, tr in enumerate(project.get("tracks", [])):
        is_audio = str(tr.get("type", "video")).lower() == "audio"
        lane = -(t_idx + 1) if is_audio else t_idx

        for c in sorted(tr.get("clips", []), key=lambda x: float(x.get("start", 0.0))):
            asset_id = f"r{asset_counter}"
            asset_counter += 1
            media_file = str(c.get("mediaFile") or c.get("name") or "media.mp4")
            dur_sec = float(c.get("duration", 5.0))
            trim_in_sec = float(c.get("trimIn", 0.0))
            start_sec = float(c.get("start", 0.0))
            opacity = float(c.get("opacity", 1.0))
            gain = float(c.get("gain", 1.0))
            gain_db = 20.0 * math.log10(gain) if gain > 0.0001 else -96.0

            ET.SubElement(
                resources,
                "asset",
                {
                    "id": asset_id,
                    "name": Path(media_file).name,
                    "src": _to_file_url(media_file),
                    "start": "0s",
                    "duration": _format_fcpx_rational(max(dur_sec + trim_in_sec, 5.0)),
                    "hasVideo": "0" if is_audio else "1",
                    "hasAudio": "1",
                },
            )

            ac_el = ET.SubElement(
                spine,
                "asset-clip",
                {
                    "id": str(c.get("id", f"clip_{asset_counter}")),
                    "name": str(c.get("name", "Clip")),
                    "ref": asset_id,
                    "lane": str(lane),
                    "offset": _format_fcpx_rational(start_sec),
                    "duration": _format_fcpx_rational(dur_sec),
                    "start": _format_fcpx_rational(trim_in_sec),
                    "enabled": "0" if (tr.get("muted") or c.get("muted")) else "1",
                    "neovegTrackId": str(tr.get("id", f"tr_{t_idx + 1}")),
                    "neovegTrackName": str(tr.get("name", "Track")),
                    "neovegTrackType": str(tr.get("type", "video")),
                    "neovegTrackMuted": "1" if tr.get("muted") else "0",
                    "neovegTrackSolo": "1" if tr.get("solo") else "0",
                    "neovegTrackLevel": f"{float(tr.get('level', 1.0)):.6f}",
                    "neovegGainLinear": f"{gain:.6f}",
                    "neovegOpacity": f"{opacity:.4f}",
                },
            )
            ET.SubElement(ac_el, "adjust-blend", {"amount": f"{opacity:.4f}"})
            ET.SubElement(ac_el, "adjust-volume", {"amount": f"{gain_db:.3f}dB"})

    for mk in project.get("markers", []):
        ET.SubElement(
            spine,
            "marker",
            {
                "start": _format_fcpx_rational(float(mk.get("time", 0.0))),
                "duration": frame_dur,
                "value": str(mk.get("label", "Marker")),
                "neovegColor": str(mk.get("color", "#f97316")),
            },
        )

    ET.indent(root, space="  ")
    body = ET.tostring(root, encoding="unicode")
    return f'<?xml version="1.0" encoding="UTF-8"?>\n<!DOCTYPE fcpxml>\n{body}\n'


def export_fcpxml(project: Dict[str, Any], output_path: str | Path) -> Path:
    out = Path(output_path)
    if out.suffix.lower() != ".fcpxml":
        out = out.with_suffix(".fcpxml")
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(export_fcpxml_to_string(project), encoding="utf-8")
    return out


def import_fcpxml_from_string(xml_content: str) -> Dict[str, Any]:
    root = ET.fromstring(xml_content)
    proj_el = root.find(".//project")
    seq_el = root.find(".//sequence")
    if seq_el is None:
        raise ValueError("FCPXML inválido: elemento <sequence> não encontrado.")

    fmt_el = root.find(".//format")
    width = int(fmt_el.attrib.get("width", "1920")) if fmt_el is not None else 1920
    height = int(fmt_el.attrib.get("height", "1080")) if fmt_el is not None else 1080
    fps = float(seq_el.attrib.get("neovegFps", "29.97"))

    asset_map: Dict[str, str] = {}
    for asset in root.findall(".//asset"):
        aid = asset.attrib.get("id")
        src = _from_file_url(asset.attrib.get("src")) or asset.attrib.get("name")
        if aid and src:
            asset_map[aid] = src

    tracks_by_id: Dict[str, Dict[str, Any]] = {}
    tracks: List[Dict[str, Any]] = []
    markers: List[Dict[str, Any]] = []

    spine = seq_el.find("spine")
    if spine is not None:
        for ac in spine.findall("asset-clip"):
            tr_id = ac.attrib.get("neovegTrackId") or f"tr_lane_{ac.attrib.get('lane', '0')}"
            tr_name = ac.attrib.get("neovegTrackName") or f"Track {tr_id}"
            tr_type = ac.attrib.get("neovegTrackType") or ("audio" if int(ac.attrib.get("lane", "0")) < 0 else "video")

            if tr_id not in tracks_by_id:
                tr_obj = {
                    "id": tr_id,
                    "name": tr_name,
                    "type": tr_type,
                    "muted": ac.attrib.get("neovegTrackMuted") == "1",
                    "solo": ac.attrib.get("neovegTrackSolo") == "1",
                    "level": float(ac.attrib.get("neovegTrackLevel", "1.0")),
                    "clips": [],
                }
                tracks_by_id[tr_id] = tr_obj
                tracks.append(tr_obj)

            start_sec = _parse_fcpx_rational(ac.attrib.get("offset", "0s"))
            dur_sec = _parse_fcpx_rational(ac.attrib.get("duration", "5s"))
            trim_in_sec = _parse_fcpx_rational(ac.attrib.get("start", "0s"))
            opacity = float(ac.attrib.get("neovegOpacity") or ac.find("./adjust-blend").attrib.get("amount", "1.0"))
            gain = float(ac.attrib.get("neovegGainLinear", "1.0"))

            tracks_by_id[tr_id]["clips"].append({
                "id": ac.attrib.get("id", "clip_1"),
                "name": ac.attrib.get("name", "Clip"),
                "mediaFile": asset_map.get(ac.attrib.get("ref", "")),
                "mediaType": tr_type,
                "start": round(start_sec, 6),
                "duration": round(dur_sec, 6),
                "end": round(start_sec + dur_sec, 6),
                "trimIn": round(trim_in_sec, 6),
                "outPoint": round(trim_in_sec + dur_sec, 6),
                "opacity": round(opacity, 4),
                "gain": round(gain, 6),
            })

        for mk in spine.findall("marker"):
            markers.append({
                "time": round(_parse_fcpx_rational(mk.attrib.get("start", "0s")), 6),
                "label": mk.attrib.get("value", "Marker"),
                "color": mk.attrib.get("neovegColor", "#f97316"),
            })

    return {
        "apiVersion": "2.1.0",
        "name": proj_el.attrib.get("name", "Imported_FCPXML_Project") if proj_el is not None else "Imported_FCPXML_Project",
        "width": width,
        "height": height,
        "fps": fps,
        "tracks": tracks,
        "markers": markers,
        "regions": [],
    }


def import_fcpxml(fcpxml_path: str | Path) -> Dict[str, Any]:
    return import_fcpxml_from_string(Path(fcpxml_path).read_text(encoding="utf-8"))


# =============================================================================
# 4. CMX3600 EDL (`.edl`) EXPORT
# =============================================================================

def export_cmx3600_edl_to_string(project: Dict[str, Any]) -> str:
    fps = float(project.get("fps") or 29.97)
    drop_frame = abs(fps - 29.97) < 0.05 or abs(fps - 59.94) < 0.05
    lines = [
        f"TITLE: {project.get('name', 'Neoveg_Sequence')}",
        f"FCM: {'DROP FRAME' if drop_frame else 'NON-DROP FRAME'}",
        "",
    ]
    ev_num = 1
    for tr in project.get("tracks", []):
        is_audio = str(tr.get("type", "video")).lower() == "audio"
        ch_code = "AA   " if is_audio else "V    "
        for c in sorted(tr.get("clips", []), key=lambda x: float(x.get("start", 0.0))):
            start_s = float(c.get("start", 0.0))
            dur_s = float(c.get("duration", 5.0))
            trim_in = float(c.get("trimIn", 0.0))
            src_in = _format_smpte(trim_in, fps, drop_frame)
            src_out = _format_smpte(trim_in + dur_s, fps, drop_frame)
            rec_in = _format_smpte(start_s, fps, drop_frame)
            rec_out = _format_smpte(start_s + dur_s, fps, drop_frame)
            gain = float(c.get("gain", 1.0))
            gain_db = 20.0 * math.log10(gain) if gain > 0.0001 else -96.0
            opacity = float(c.get("opacity", 1.0)) * 100.0
            lines.append(f"{ev_num:03d}  AX       {ch_code} C        {src_in} {src_out} {rec_in} {rec_out}")
            lines.append(f"* FROM CLIP NAME: {c.get('name', 'Clip')}")
            if c.get("mediaFile"):
                lines.append(f"* SOURCE FILE: {c['mediaFile']}")
            lines.append(f"* TRACK NAME: {tr.get('name', 'Track')} | OPACITY: {opacity:.1f}% | AUDIO LEVEL: {gain_db:+.2f} DB")
            lines.append("")
            ev_num += 1
    return "\n".join(lines) + "\n"


def export_cmx3600_edl(project: Dict[str, Any], output_path: str | Path) -> Path:
    out = Path(output_path)
    if out.suffix.lower() != ".edl":
        out = out.with_suffix(".edl")
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(export_cmx3600_edl_to_string(project), encoding="utf-8")
    return out


# =============================================================================
# HELPERS
# =============================================================================

def _try_relpath(abs_path: str, base_dir: Path) -> str:
    if not abs_path:
        return ""
    try:
        return os.path.relpath(abs_path, base_dir).replace("\\", "/")
    except ValueError:
        return abs_path.replace("\\", "/")


def _deterministic_guid(seed: str) -> str:
    h = hashlib.sha256(seed.encode("utf-8")).hexdigest()
    return f"{h[0:8]}-{h[8:12]}-{h[12:16]}-{h[16:20]}-{h[20:32]}"


def _get_fcp7_rate(fps: float) -> Tuple[int, bool]:
    if abs(fps - 23.976) < 0.05:
        return 24, True
    if abs(fps - 29.97) < 0.05:
        return 30, True
    if abs(fps - 59.94) < 0.05:
        return 60, True
    return max(1, int(round(fps))), False


def _resolve_fps_from_fcp7(timebase: int, ntsc: bool) -> float:
    if ntsc:
        if timebase == 24:
            return 23.976
        if timebase == 30:
            return 29.97
        if timebase == 60:
            return 59.94
    return float(max(1, timebase))


def _sec_to_frames(seconds: float, fps: float) -> int:
    return int(round(max(0.0, seconds) * fps))


def _frames_to_sec(frames: int, fps: float) -> float:
    return max(0, frames) / max(1.0, fps)


def _compute_project_duration(project: Dict[str, Any]) -> float:
    max_end = 0.0
    for tr in project.get("tracks", []):
        for c in tr.get("clips", []):
            end = float(c.get("start", 0.0)) + float(c.get("duration", 0.0))
            if end > max_end:
                max_end = end
    return max_end if max_end > 0.1 else 8.0


def _format_fcpx_rational(seconds: float) -> str:
    ms = int(round(max(0.0, seconds) * 1000.0))
    return f"{ms}/1000s"


def _parse_fcpx_rational(val: Optional[str]) -> float:
    if not val:
        return 0.0
    s = val.strip().rstrip("sS")
    if "/" in s:
        num_s, den_s = s.split("/", 1)
        den = float(den_s)
        return float(num_s) / den if abs(den) > 1e-9 else 0.0
    return float(s)


def _format_smpte(seconds: float, fps: float, drop_frame: bool) -> str:
    tb = max(1, int(round(fps)))
    total_frames = max(0, int(round(seconds * fps)))
    ff = total_frames % tb
    total_sec = total_frames // tb
    ss = total_sec % 60
    mm = (total_sec // 60) % 60
    hh = total_sec // 3600
    sep = ";" if drop_frame else ":"
    return f"{hh:02d}:{mm:02d}:{ss:02d}{sep}{ff:02d}"


def _to_file_url(path: str) -> str:
    if not path:
        return "file://localhost/media.mp4"
    if path.lower().startswith("file://"):
        return path
    norm = path.replace("\\", "/").lstrip("/")
    return f"file://localhost/{urllib.parse.quote(norm, safe='/:._-')}"


def _from_file_url(url: Optional[str]) -> Optional[str]:
    if not url:
        return None
    s = url.strip()
    for prefix in ("file://localhost/", "file:///", "file://"):
        if s.lower().startswith(prefix):
            s = s[len(prefix):]
            break
    return urllib.parse.unquote(s)
