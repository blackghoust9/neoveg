"""
OpenNeoveg — Python Scripting API (`neoveg` module)
Inspired by Neoveg's official Scripting API, enabling users to automate
timeline editing, batch FX, beat markers, ripple cuts, and normalization using Python.
"""

from __future__ import annotations
import copy
import io
import math
from contextlib import redirect_stdout
from typing import Any, Dict, List


class NeovegProjectWrapper:
    """Object-oriented wrapper around the Neoveg JSON project state for user scripts."""

    def __init__(self, state: Dict[str, Any]) -> None:
        self._state = copy.deepcopy(state)
        self.logs: List[str] = []

    @property
    def cursor(self) -> float:
        return float(self._state.get("playhead", 0.0))

    @cursor.setter
    def cursor(self, val: float) -> None:
        self._state["playhead"] = max(0.0, float(val))

    @property
    def fps(self) -> float:
        return float(self._state.get("fps", 30.0))

    @property
    def tracks(self) -> List[Dict[str, Any]]:
        return self._state.get("tracks", [])

    @property
    def markers(self) -> List[Dict[str, Any]]:
        return self._state.setdefault("markers", [])

    @property
    def regions(self) -> List[Dict[str, Any]]:
        return self._state.setdefault("regions", [])

    def log(self, message: str) -> None:
        self.logs.append(str(message))
        print(message)

    def add_marker(self, time_sec: float, label: str, color: str = "#f97316") -> Dict[str, Any]:
        marker = {
            "id": f"m_{len(self.markers) + 1}_{int(time_sec * 100)}",
            "time": round(max(0.0, float(time_sec)), 3),
            "label": str(label),
            "color": color,
        }
        self.markers.append(marker)
        self.markers.sort(key=lambda m: m["time"])
        return marker

    def add_region(self, start_sec: float, end_sec: float, label: str) -> Dict[str, Any]:
        reg = {
            "id": f"reg_{len(self.regions) + 1}",
            "start": round(max(0.0, float(start_sec)), 3),
            "end": round(max(float(start_sec) + 0.1, float(end_sec)), 3),
            "label": str(label),
        }
        self.regions.append(reg)
        return reg

    def remove_gaps(self, track_index: int = -1) -> int:
        """Ripple-closes gaps between sequential clips on the specified track (or all tracks)."""
        moved = 0
        for idx, track in enumerate(self.tracks):
            if track_index >= 0 and idx != track_index:
                continue
            clips = track.get("clips", [])
            clips.sort(key=lambda c: float(c.get("start", 0.0)))
            cursor_t = 0.0
            for clip in clips:
                old_start = float(clip.get("start", 0.0))
                if abs(old_start - cursor_t) > 0.01:
                    clip["start"] = round(cursor_t, 3)
                    moved += 1
                cursor_t = float(clip["start"]) + float(clip.get("duration", 1.0))
        return moved

    def normalize_audio_gains(self, target_gain: float = 1.0) -> int:
        count = 0
        for track in self.tracks:
            if track.get("type") == "audio":
                for clip in track.get("clips", []):
                    clip["gain"] = round(float(target_gain), 2)
                    count += 1
        return count

    def apply_crossfade_all(self, overlap_sec: float = 0.75, curve: str = "smooth") -> int:
        """Overlaps adjacent clips on video/audio tracks by `overlap_sec` to create Neoveg Auto-Crossfades."""
        ov = max(0.1, min(3.0, float(overlap_sec)))
        applied = 0
        for track in self.tracks:
            clips = track.get("clips", [])
            clips.sort(key=lambda c: float(c.get("start", 0.0)))
            for i in range(1, len(clips)):
                prev_c = clips[i - 1]
                curr_c = clips[i]
                prev_end = float(prev_c["start"]) + float(prev_c["duration"])
                if float(curr_c["duration"]) > ov * 1.5 and float(prev_c["duration"]) > ov * 1.5:
                    curr_c["start"] = round(max(0.0, prev_end - ov), 3)
                    prev_c["fadeOut"] = round(ov, 3)
                    prev_c["fadeOutCurve"] = curve
                    curr_c["fadeIn"] = round(ov, 3)
                    curr_c["fadeInCurve"] = curve
                    applied += 1
        return applied

    def rename_track(self, track_index: int, new_name: str) -> bool:
        """Renames a track by index (0-based)."""
        if 0 <= track_index < len(self.tracks):
            cleaned = str(new_name).strip()[:64]
            if cleaned:
                self.tracks[track_index]["name"] = cleaned
                return True
        return False

    def group_tracks(self, track_indices: List[int], group_name: str, group_color: str = "#f59e0b") -> int:
        """Groups the specified video and audio track indices together."""
        gid = f"grp_{abs(hash(group_name)) % 10000}"
        count = 0
        for idx in track_indices:
            if 0 <= idx < len(self.tracks):
                self.tracks[idx]["groupId"] = gid
                self.tracks[idx]["groupName"] = str(group_name).strip()[:48]
                self.tracks[idx]["groupColor"] = group_color
                count += 1
        return count

    def to_dict(self) -> Dict[str, Any]:
        return self._state


PRESET_SCRIPTS: Dict[str, Dict[str, str]] = {
    "group_av_tracks": {
        "name": "Group & Rename Video + Audio Tracks (Neoveg Toolkit / HOS)",
        "description": "Renames and groups Video + Audio tracks into a synchronized Neoveg Track Group.",
        "code": (
            "# Neoveg Python Scripting API — Group & Rename Video + Audio Tracks\n"
            "for idx, tr in enumerate(neoveg.tracks):\n"
            "    kind = 'VIDEO' if tr.get('type') == 'video' else 'AUDIO'\n"
            "    neoveg.rename_track(idx, f'{kind} {idx + 1} - Master Scene')\n"
            "indices = list(range(len(neoveg.tracks)))\n"
            "cnt = neoveg.group_tracks(indices, 'Grupo 1: Master Program (V+A)', '#f59e0b')\n"
            "neoveg.log(f'Agrupadas e renomeadas {cnt} faixas de Vídeo e Áudio no Grupo 1.')\n"
        ),
    },
    "beat_markers": {
        "name": "Add 120 BPM Beat Grid Markers",
        "description": "Generates Neoveg timeline markers on every bar (4 beats) at 120 BPM.",
        "code": (
            "# Neoveg Python Scripting API — Beat Grid Generator\n"
            "bpm = 120.0\n"
            "seconds_per_bar = (60.0 / bpm) * 4\n"
            "neoveg.markers.clear()\n"
            "for bar in range(1, 13):\n"
            "    t = (bar - 1) * seconds_per_bar\n"
            "    neoveg.add_marker(t, f'Bar {bar}', '#f97316')\n"
            "neoveg.log(f'Created {len(neoveg.markers)} bar markers at {bpm} BPM.')\n"
        ),
    },
    "auto_crossfade": {
        "name": "Auto-Crossfade Sequential Events (0.8s Smooth)",
        "description": "Overlaps sequential clips by 0.80s and applies Neoveg Smooth S-Curve crossfades.",
        "code": (
            "# Neoveg Python Scripting API — Auto-Crossfade All Sequential Events\n"
            "count = neoveg.apply_crossfade_all(overlap_sec=0.8, curve='smooth')\n"
            "neoveg.log(f'Applied 0.8s Smooth S-Curve auto-crossfade to {count} transitions.')\n"
        ),
    },
    "ripple_close_gaps": {
        "name": "Ripple Delete All Timeline Gaps",
        "description": "Closes all empty spaces between clips across every track.",
        "code": (
            "# Neoveg Python Scripting API — Ripple Close Gaps\n"
            "moved = neoveg.remove_gaps()\n"
            "neoveg.log(f'Ripple-shifted {moved} timeline events to close all gaps.')\n"
        ),
    },
    "cinema_grade": {
        "name": "Apply Cinema Teal & Orange Grade + Vignette",
        "description": "Adds Color Corrector and Vignette FX to the master Video Track 1.",
        "code": (
            "# Neoveg Python Scripting API — Batch Cinema Look\n"
            "applied = 0\n"
            "for track in neoveg.tracks:\n"
            "    if track.get('type') == 'video':\n"
            "        for clip in track.get('clips', []):\n"
            "            fx_list = clip.setdefault('fx', [])\n"
            "            fx_list.append({\n"
            "                'id': f'fx_grade_{clip[\"id\"]}',\n"
            "                'type': 'colorGrade',\n"
            "                'name': 'Neoveg Color Corrector (Teal/Orange)',\n"
            "                'enabled': True,\n"
            "                'params': {'liftR': -0.06, 'liftB': 0.08, 'gainR': 1.12, 'gainB': 0.92, 'saturation': 1.18}\n"
            "            })\n"
            "            applied += 1\n"
            "neoveg.log(f'Applied Cinema Teal & Orange Grade to {applied} video events.')\n"
        ),
    },
    "normalize_audio": {
        "name": "Normalize All Audio Events to 0.0 dB + Soft Fades",
        "description": "Sets all audio clips to 0.0 dB unity gain and adds 150ms de-click fades.",
        "code": (
            "# Neoveg Python Scripting API — Audio Normalization & De-Click Fades\n"
            "count = neoveg.normalize_audio_gains(1.0)\n"
            "for track in neoveg.tracks:\n"
            "    if track.get('type') == 'audio':\n"
            "        for clip in track.get('clips', []):\n"
            "            clip['fadeIn'] = max(float(clip.get('fadeIn', 0)), 0.15)\n"
            "            clip['fadeOut'] = max(float(clip.get('fadeOut', 0)), 0.15)\n"
            "neoveg.log(f'Normalized {count} audio events to 0.0 dB with >=150ms fades.')\n"
        ),
    },
}


def execute_neoveg_script(code: str, project_state: Dict[str, Any]) -> Dict[str, Any]:
    wrapper = NeovegProjectWrapper(project_state)
    stdout_buf = io.StringIO()
    safe_globals = {
        "__builtins__": {
            "range": range,
            "len": len,
            "min": min,
            "max": max,
            "abs": abs,
            "round": round,
            "int": int,
            "float": float,
            "str": str,
            "bool": bool,
            "list": list,
            "dict": dict,
            "enumerate": enumerate,
            "sorted": sorted,
            "sum": sum,
            "print": print,
        },
        "math": math,
        "neoveg": wrapper,
        "project": wrapper,
    }
    try:
        with redirect_stdout(stdout_buf):
            exec(code, safe_globals, {})
        out_text = stdout_buf.getvalue().strip()
        return {
            "ok": True,
            "project": wrapper.to_dict(),
            "logs": wrapper.logs,
            "stdout": out_text,
        }
    except Exception as exc:
        return {
            "ok": False,
            "error": f"{type(exc).__name__}: {exc}",
            "project": project_state,
            "logs": wrapper.logs,
            "stdout": stdout_buf.getvalue().strip(),
        }
