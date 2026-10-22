"""
OpenVegas Pro — Python Media Backend, FFmpeg Pipeline & Studio HTTP Server
Serves the TypeScript/JS Vegas Pro NLE UI, manages real media assets, extracts
PCM waveforms via the Rust Core bridge, executes Vegas Python scripts, and
renders multi-track timelines via FFmpeg.
"""

from __future__ import annotations
import json
import math
import mimetypes
import os
import shutil
import struct
import subprocess
import sys
import time
import urllib.parse
import wave
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any, Dict, List, Optional

from rust_bridge import (
    analyze_wav_peaks,
    compile_project_to_ffmpeg,
    detect_auto_crossfades,
    format_smpte,
    get_rust_status,
    group_tracks_in_project,
    rename_track_in_project,
    sync_group_mute_solo_in_project,
    ungroup_track_in_project,
)
from vegas_scripting import PRESET_SCRIPTS, execute_vegas_script
from error_logger import init_error_log, log_error, log_exception, read_error_log
from plugins_engine import (
    FREI0R_PLUGINS,
    LADSPA_PLUGINS,
    create_empty_project,
    diagnose_and_fix_vegas_problems,
    ensure_default_projects,
    list_saved_projects,
    load_saved_project,
    probe_plugin_environment,
    save_project_to_disk,
)

ROOT_DIR = Path(__file__).resolve().parent.parent
FRONTEND_DIR = ROOT_DIR / "frontend"
MEDIA_DIR = ROOT_DIR / "media"
RENDERS_DIR = ROOT_DIR / "renders"

MEDIA_DIR.mkdir(parents=True, exist_ok=True)
RENDERS_DIR.mkdir(parents=True, exist_ok=True)


def get_ffmpeg_version() -> str:
    ffmpeg_bin = shutil.which("ffmpeg")
    if not ffmpeg_bin:
        return "not found"
    try:
        proc = subprocess.run(
            [ffmpeg_bin, "-version"],
            capture_output=True,
            text=True,
            timeout=3,
            check=False,
        )
        first_line = (proc.stdout or "").splitlines()[0] if proc.stdout else "ffmpeg available"
        return first_line.strip()
    except Exception:
        return "ffmpeg available"


def generate_synth_wav(path: Path, mode: str = "groove", duration: float = 8.0) -> None:
    """Generates a stereo 48kHz 16-bit PCM WAV with real musical dynamics and transients."""
    if path.exists():
        return
    sr = 48000
    n_frames = int(sr * duration)
    buf = bytearray(n_frames * 4)  # 2 channels * 2 bytes
    bpm = 120.0
    beat_sec = 60.0 / bpm

    for i in range(n_frames):
        t = i / float(sr)
        if mode == "groove":
            # 120 BPM electronic kick + bassline + hi-hat transient pattern
            beat_pos = (t % beat_sec) / beat_sec
            bar_pos = (t % (beat_sec * 4.0)) / (beat_sec * 4.0)
            kick_env = math.exp(-beat_pos * 14.0)
            kick_freq = 52.0 + 95.0 * math.exp(-beat_pos * 28.0)
            kick = math.sin(2.0 * math.pi * kick_freq * t) * kick_env * 0.65

            # Bass note progression
            notes = [55.0, 55.0, 65.41, 73.42]
            note_idx = int(bar_pos * 4.0) % 4
            bass_freq = notes[note_idx]
            sub_pos = (t % (beat_sec * 0.5)) / (beat_sec * 0.5)
            bass_env = math.exp(-sub_pos * 4.5) * (0.8 if beat_pos > 0.2 else 0.3)
            bass = (
                math.sin(2.0 * math.pi * bass_freq * t) * 0.6
                + math.sin(2.0 * math.pi * bass_freq * 2.0 * t) * 0.25
            ) * bass_env * 0.45

            val_l = max(-0.95, min(0.95, kick + bass))
            val_r = max(-0.95, min(0.95, kick + bass * 0.92))
        else:
            # Warm stereo ambient chord pad (A minor 9th)
            freqs = [220.0, 261.63, 329.63, 392.00, 493.88]
            env = min(1.0, t / 0.8) * min(1.0, (duration - t) / 1.0)
            lfo = 0.85 + 0.15 * math.sin(2.0 * math.pi * 0.5 * t)
            sig_l = sum(math.sin(2.0 * math.pi * f * t + idx * 0.3) for idx, f in enumerate(freqs)) / len(freqs)
            sig_r = sum(math.sin(2.0 * math.pi * (f * 1.002) * t + idx * 0.7) for idx, f in enumerate(freqs)) / len(freqs)
            val_l = max(-0.9, min(0.9, sig_l * env * lfo * 0.65))
            val_r = max(-0.9, min(0.9, sig_r * env * lfo * 0.65))

        s_l = int(val_l * 32767.0)
        s_r = int(val_r * 32767.0)
        struct.pack_into("<hh", buf, i * 4, s_l, s_r)

    with wave.open(str(path), "wb") as wf:
        wf.setnchannels(2)
        wf.setsampwidth(2)
        wf.setframerate(sr)
        wf.writeframes(buf)


def ensure_sample_media() -> List[Dict[str, Any]]:
    """Generates real playable MP4 and WAV sample media files using FFmpeg and Python DSP."""
    generate_synth_wav(MEDIA_DIR / "synth_bass_120bpm.wav", mode="groove", duration=8.0)
    generate_synth_wav(MEDIA_DIR / "ambient_pad_chord.wav", mode="pad", duration=8.0)

    ffmpeg_bin = shutil.which("ffmpeg")
    if ffmpeg_bin:
        samples = [
            (
                MEDIA_DIR / "vegas_smpte_bars.mp4",
                [
                    ffmpeg_bin,
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    "smptehdbars=size=1280x720:rate=30:duration=6",
                    "-f",
                    "lavfi",
                    "-i",
                    "sine=frequency=440:sample_rate=48000:duration=6",
                    "-vf",
                    "drawbox=x=0:y=580:w=1280:h=100:color=black@0.65:t=fill",
                    "-af",
                    "volume=0.12",
                    "-c:v",
                    "libx264",
                    "-preset",
                    "ultrafast",
                    "-pix_fmt",
                    "yuv420p",
                    "-c:a",
                    "aac",
                ],
            ),
            (
                MEDIA_DIR / "studio_broadcast_slate.mp4",
                [
                    ffmpeg_bin,
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc2=size=1280x720:rate=30:duration=6",
                    "-f",
                    "lavfi",
                    "-i",
                    "sine=frequency=330:sample_rate=48000:duration=6",
                    "-af",
                    "volume=0.10",
                    "-c:v",
                    "libx264",
                    "-preset",
                    "ultrafast",
                    "-pix_fmt",
                    "yuv420p",
                    "-c:a",
                    "aac",
                ],
            ),
        ]
        for out_file, cmd in samples:
            if not out_file.exists():
                try:
                    subprocess.run(
                        [*cmd, str(out_file)],
                        capture_output=True,
                        timeout=15,
                        check=False,
                    )
                except Exception:
                    pass

    return list_media_assets()


def probe_media_file(path: Path) -> Dict[str, Any]:
    ext = path.suffix.lower()
    is_audio = ext in {".wav", ".mp3", ".ogg", ".flac", ".aac", ".m4a"}
    is_image = ext in {".png", ".jpg", ".jpeg", ".webp", ".bmp"}
    media_type = "audio" if is_audio else ("image" if is_image else "video")

    info: Dict[str, Any] = {
        "id": f"media_{path.stem}",
        "name": path.name,
        "fileName": path.name,
        "url": f"/media/{urllib.parse.quote(path.name)}",
        "type": media_type,
        "sizeBytes": path.stat().st_size if path.exists() else 0,
        "duration": 6.0 if not is_image else 5.0,
        "width": 1280 if not is_audio else 0,
        "height": 720 if not is_audio else 0,
        "fps": 30.0 if media_type == "video" else 0.0,
        "codec": ext.lstrip(".").upper(),
        "sampleRate": 48000,
        "channels": 2,
        "peaks": [],
    }

    if ext == ".wav" and path.exists():
        try:
            wav_data = analyze_wav_peaks(path, 96)
            info["duration"] = wav_data["duration_sec"]
            info["sampleRate"] = wav_data["sample_rate"]
            info["channels"] = wav_data["channels"]
            info["peakDb"] = wav_data["peak_db"]
            info["rmsDb"] = wav_data["rms_db"]
            info["peaks"] = wav_data["peaks"]
            return info
        except Exception:
            pass

    ffprobe_bin = shutil.which("ffprobe")
    if ffprobe_bin and path.exists():
        try:
            proc = subprocess.run(
                [
                    ffprobe_bin,
                    "-v",
                    "quiet",
                    "-print_format",
                    "json",
                    "-show_format",
                    "-show_streams",
                    str(path),
                ],
                capture_output=True,
                text=True,
                timeout=4,
                check=False,
            )
            if proc.stdout:
                pdata = json.loads(proc.stdout)
                fmt = pdata.get("format", {})
                if "duration" in fmt:
                    info["duration"] = round(float(fmt["duration"]), 3)
                for st in pdata.get("streams", []):
                    if st.get("codec_type") == "video":
                        info["width"] = int(st.get("width", 1280))
                        info["height"] = int(st.get("height", 720))
                        info["codec"] = st.get("codec_name", info["codec"]).upper()
                    elif st.get("codec_type") == "audio":
                        info["sampleRate"] = int(st.get("sample_rate", 48000))
                        info["channels"] = int(st.get("channels", 2))
        except Exception:
            pass

    return info


def list_media_assets() -> List[Dict[str, Any]]:
    items: List[Dict[str, Any]] = []
    if not MEDIA_DIR.exists():
        return items
    for f in sorted(MEDIA_DIR.iterdir()):
        if f.is_file() and not f.name.startswith("."):
            items.append(probe_media_file(f))
    return items


class OpenVegasHTTPHandler(BaseHTTPRequestHandler):
    server_version = "OpenVegasStudio/1.0"

    def log_message(self, format: str, *args: Any) -> None:
        # Quiet clean logging
        pass

    def _send_json(self, data: Any, status: int = 200) -> None:
        raw = json.dumps(data, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(raw)))
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")
        self.end_headers()
        self.wfile.write(raw)

    def _read_json_body(self) -> Dict[str, Any]:
        length = int(self.headers.get("Content-Length", "0"))
        if length <= 0:
            return {}
        raw = self.rfile.read(length)
        return json.loads(raw.decode("utf-8"))

    def do_OPTIONS(self) -> None:
        self.send_response(204)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")
        self.end_headers()

    def do_GET(self) -> None:
        parsed = urllib.parse.urlparse(self.path)
        req_path = urllib.parse.unquote(parsed.path)

        if req_path == "/api/status":
            self._send_json(
                {
                    "ok": True,
                    "app": "OpenVegas Pro NLE Studio",
                    "python_version": sys.version.split()[0],
                    "ffmpeg": get_ffmpeg_version(),
                    "rust_core": get_rust_status(),
                    "plugins": probe_plugin_environment(),
                    "workspace": str(ROOT_DIR),
                }
            )
            return

        if req_path == "/api/projects/list":
            self._send_json({"ok": True, "projects": ensure_default_projects()})
            return

        if req_path == "/api/plugins/catalog":
            self._send_json(
                {
                    "ok": True,
                    "environment": probe_plugin_environment(),
                    "frei0r": FREI0R_PLUGINS,
                    "ladspa": LADSPA_PLUGINS,
                }
            )
            return

        if req_path == "/api/diagnostics/errors":
            self._send_json({"ok": True, "lines": read_error_log()})
            return

        if req_path == "/api/media/list":
            self._send_json({"ok": True, "media": list_media_assets()})
            return

        if req_path == "/api/scripts/presets":
            self._send_json({"ok": True, "presets": PRESET_SCRIPTS})
            return

        if req_path == "/api/renders/list":
            renders = []
            for f in sorted(RENDERS_DIR.iterdir(), key=lambda p: p.stat().st_mtime, reverse=True):
                if f.is_file():
                    renders.append(
                        {
                            "name": f.name,
                            "url": f"/renders/{urllib.parse.quote(f.name)}",
                            "sizeBytes": f.stat().st_size,
                        }
                    )
            self._send_json({"ok": True, "renders": renders})
            return

        if req_path.startswith("/media/"):
            rel = req_path[len("/media/") :]
            target = (MEDIA_DIR / rel).resolve()
            if target.exists() and target.is_file() and MEDIA_DIR in target.parents:
                self._serve_file_with_range(target)
                return
            self.send_error(404, "Media file not found")
            return

        if req_path.startswith("/renders/"):
            rel = req_path[len("/renders/") :]
            target = (RENDERS_DIR / rel).resolve()
            if target.exists() and target.is_file() and RENDERS_DIR in target.parents:
                self._serve_file_with_range(target)
                return
            self.send_error(404, "Render file not found")
            return

        # Serve static frontend files
        if req_path == "/" or req_path == "/index.html":
            self._serve_static(FRONTEND_DIR / "public" / "index.html")
            return

        candidate_public = (FRONTEND_DIR / "public" / req_path.lstrip("/")).resolve()
        if candidate_public.exists() and candidate_public.is_file():
            self._serve_static(candidate_public)
            return

        candidate_dist = (FRONTEND_DIR / "dist" / req_path.lstrip("/")).resolve()
        if candidate_dist.exists() and candidate_dist.is_file():
            self._serve_static(candidate_dist)
            return

        self.send_error(404, f"Not found: {req_path}")

    def do_POST(self) -> None:
        parsed = urllib.parse.urlparse(self.path)
        req_path = parsed.path

        if req_path == "/api/timeline/compile":
            body = self._read_json_body()
            project = body.get("project", {})
            updated_proj, crossfades = detect_auto_crossfades(project)
            out_file = RENDERS_DIR / body.get("outputName", "vegas_master_render.mp4")
            plan = compile_project_to_ffmpeg(updated_proj, MEDIA_DIR, out_file)
            self._send_json(
                {
                    "ok": True,
                    "project": updated_proj,
                    "crossfades": crossfades,
                    "ffmpeg_plan": plan,
                    "rust_core": get_rust_status(),
                }
            )
            return

        if req_path == "/api/render/run":
            body = self._read_json_body()
            project = body.get("project", {})
            out_name = body.get("outputName", f"OpenVegas_Render_{int(time.time())}.mp4")
            safe_name = "".join(c for c in out_name if c.isalnum() or c in "._-") or "render.mp4"
            if not safe_name.endswith((".mp4", ".webm", ".wav")):
                safe_name += ".mp4"
            out_path = RENDERS_DIR / safe_name

            updated_proj, crossfades = detect_auto_crossfades(project)
            plan = compile_project_to_ffmpeg(updated_proj, MEDIA_DIR, out_path)
            t0 = time.time()
            try:
                proc = subprocess.run(
                    plan["command"],
                    capture_output=True,
                    text=True,
                    timeout=90,
                    check=False,
                )
                elapsed = round(time.time() - t0, 2)
                if proc.returncode == 0 and out_path.exists():
                    self._send_json(
                        {
                            "ok": True,
                            "fileName": safe_name,
                            "url": f"/renders/{urllib.parse.quote(safe_name)}",
                            "sizeBytes": out_path.stat().st_size,
                            "elapsedSec": elapsed,
                            "crossfadesCount": len(crossfades),
                            "filterComplex": plan["filter_complex"],
                            "commandStr": plan["command_str"],
                        }
                    )
                else:
                    self._send_json(
                        {
                            "ok": False,
                            "error": (proc.stderr or "FFmpeg returned non-zero status")[-800:],
                            "commandStr": plan["command_str"],
                        },
                        status=500,
                    )
            except Exception as exc:
                self._send_json({"ok": False, "error": str(exc)}, status=500)
            return

        if req_path == "/api/scripts/execute":
            body = self._read_json_body()
            code = body.get("code", "")
            project = body.get("project", {})
            res = execute_vegas_script(code, project)
            if res.get("ok"):
                res["project"], res["crossfades"] = detect_auto_crossfades(res["project"])
            self._send_json(res)
            return

        if req_path == "/api/media/analyze-wav":
            body = self._read_json_body()
            fname = body.get("fileName", "")
            target = (MEDIA_DIR / fname).resolve()
            if not target.exists():
                self._send_json({"ok": False, "error": "File not found"}, status=404)
                return
            try:
                data = analyze_wav_peaks(target, int(body.get("buckets", 128)))
                self._send_json({"ok": True, **data})
            except Exception as exc:
                self._send_json({"ok": False, "error": str(exc)}, status=500)
            return

        if req_path == "/api/media/import-path":
            body = self._read_json_body()
            src_str = body.get("filePath", "")
            src_path = Path(src_str)
            if not src_path.exists() or not src_path.is_file():
                self._send_json({"ok": False, "error": f"File not found: {src_str}"}, status=404)
                return
            dest_path = MEDIA_DIR / src_path.name
            if src_path.resolve() != dest_path.resolve():
                shutil.copy2(src_path, dest_path)
            asset_info = probe_media_file(dest_path)
            self._send_json({"ok": True, "asset": asset_info})
            return

        if req_path == "/api/projects/load":
            body = self._read_json_body()
            fname = body.get("fileName", "OpenVegas_Broadcast_Demo.veg")
            proj = load_saved_project(fname)
            if not proj:
                self._send_json({"ok": False, "error": f"Project not found: {fname}"}, status=404)
                return
            proj, xfades = detect_auto_crossfades(proj)
            self._send_json({"ok": True, "project": proj, "crossfades": xfades})
            return

        if req_path == "/api/projects/save":
            body = self._read_json_body()
            proj = body.get("project", {})
            fname = body.get("fileName")
            res = save_project_to_disk(proj, fname)
            self._send_json({**res, "projects": list_saved_projects()})
            return

        if req_path == "/api/projects/new":
            body = self._read_json_body()
            settings = body.get("settings", {})
            proj = create_empty_project(settings)
            if body.get("saveImmediately", True):
                save_project_to_disk(proj, proj.get("name"))
            self._send_json({"ok": True, "project": proj, "projects": list_saved_projects()})
            return

        if req_path == "/api/tracks/rename":
            body = self._read_json_body()
            proj = body.get("project", {})
            track_id = body.get("trackId", "")
            new_name = body.get("newName", "")
            updated = rename_track_in_project(proj, track_id, new_name)
            self._send_json({"ok": True, "project": updated})
            return

        if req_path == "/api/tracks/group":
            body = self._read_json_body()
            proj = body.get("project", {})
            action = body.get("action", "group")
            if action == "ungroup":
                updated = ungroup_track_in_project(proj, body.get("trackId", ""))
            elif action == "sync":
                updated = sync_group_mute_solo_in_project(
                    proj,
                    body.get("groupId", ""),
                    body.get("muted"),
                    body.get("solo"),
                )
            else:
                updated = group_tracks_in_project(
                    proj,
                    body.get("trackIds", []),
                    body.get("groupId", f"grp_{int(time.time())}"),
                    body.get("groupName", "Grupo A/V"),
                    body.get("groupColor", "#f59e0b"),
                )
            self._send_json({"ok": True, "project": updated})
            return

        if req_path == "/api/diagnostics/fix-problems":
            body = self._read_json_body()
            proj = body.get("project", {})
            report = diagnose_and_fix_vegas_problems(proj)
            self._send_json(report)
            return

        if req_path == "/api/diagnostics/log-error":
            body = self._read_json_body()
            entry = log_error(
                body.get("category", "FRONTEND"),
                body.get("message", "Unknown UI error"),
                body.get("detail", ""),
            )
            self._send_json({"ok": True, "entry": entry})
            return

        self.send_error(404, f"Unknown endpoint: {req_path}")

    def _serve_static(self, file_path: Path) -> None:
        if not file_path.exists():
            self.send_error(404, "File not found")
            return
        ctype, _ = mimetypes.guess_type(str(file_path))
        if file_path.suffix == ".js":
            ctype = "application/javascript; charset=utf-8"
        elif file_path.suffix == ".css":
            ctype = "text/css; charset=utf-8"
        elif file_path.suffix == ".html":
            ctype = "text/html; charset=utf-8"
        data = file_path.read_bytes()
        self.send_response(200)
        self.send_header("Content-Type", ctype or "application/octet-stream")
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(data)

    def _serve_file_with_range(self, file_path: Path) -> None:
        ctype, _ = mimetypes.guess_type(str(file_path))
        file_size = file_path.stat().st_size
        range_hdr = self.headers.get("Range")

        if range_hdr and range_hdr.startswith("bytes="):
            try:
                ranges = range_hdr.replace("bytes=", "").split("-")
                start = int(ranges[0]) if ranges[0] else 0
                end = int(ranges[1]) if len(ranges) > 1 and ranges[1] else file_size - 1
                end = min(end, file_size - 1)
                length = max(0, end - start + 1)
                with open(file_path, "rb") as f:
                    f.seek(start)
                    chunk = f.read(length)
                self.send_response(206)
                self.send_header("Content-Type", ctype or "application/octet-stream")
                self.send_header("Accept-Ranges", "bytes")
                self.send_header("Content-Range", f"bytes {start}-{end}/{file_size}")
                self.send_header("Content-Length", str(len(chunk)))
                self.end_headers()
                self.wfile.write(chunk)
                return
            except Exception:
                pass

        data = file_path.read_bytes()
        self.send_response(200)
        self.send_header("Content-Type", ctype or "application/octet-stream")
        self.send_header("Accept-Ranges", "bytes")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)


def run_server(host: str = "127.0.0.1", port: int = 8765) -> None:
    init_error_log()
    ensure_default_projects()
    print("[OpenVegas Pro] Ensuring sample video & audio media assets...")
    assets = ensure_sample_media()
    print(f"[OpenVegas Pro] Ready with {len(assets)} media assets in {MEDIA_DIR}")
    print(f"[OpenVegas Pro] FFmpeg: {get_ffmpeg_version()}")
    print(f"[OpenVegas Pro] Rust Core: {get_rust_status().get('engine')} ({get_rust_status().get('mode')})")
    server = ThreadingHTTPServer((host, port), OpenVegasHTTPHandler)
    print(f"[OpenVegas Pro] Studio running at http://{host}:{port}")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\n[OpenVegas Pro] Shutting down studio server.")
        server.server_close()


if __name__ == "__main__":
    port_arg = int(os.environ.get("OPENVEGAS_PORT", "8765"))
    run_server(port=port_arg)
