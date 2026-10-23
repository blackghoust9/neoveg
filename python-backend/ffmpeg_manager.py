"""
Neoveg — Cross-Platform Isolated FFmpeg & FFprobe Process Manager (Python)
Manages `ffmpeg` and `ffprobe` execution in an isolated environment per operating system:
- Windows (`win-x64`, `win-arm64`): `CREATE_NO_WINDOW`, isolated `PATH`, `;`-separated `FREI0R_PATH` / `LADSPA_PATH`
- Linux (`linux-x64`, `linux-arm64`): `LD_LIBRARY_PATH`, `start_new_session=True`, `chmod +x`, `:`-separated plugin paths
- macOS (`osx-x64`, `osx-arm64`): `DYLD_LIBRARY_PATH`, `start_new_session=True`, `chmod +x`, `:`-separated plugin paths

Prioritizes portable bundled binaries in `runtimes/<rid>/ffmpeg/bin/` before falling back to system PATH.
Also exposes a JSON CLI interface so the C# Avalonia UI host can invoke media probing,
VFR-to-CFR normalization, waveform extraction, and multi-track FFmpeg rendering directly.
"""

from __future__ import annotations
import json
import os
import platform
import shutil
import stat
import subprocess
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional

ROOT_DIR = Path(__file__).resolve().parent.parent
RUNTIMES_DIR = ROOT_DIR / "runtimes"
MEDIA_DIR = ROOT_DIR / "media"
RENDERS_DIR = ROOT_DIR / "renders"
PLUGINS_DIR = ROOT_DIR / "plugins"

MEDIA_DIR.mkdir(parents=True, exist_ok=True)
RENDERS_DIR.mkdir(parents=True, exist_ok=True)


def detect_runtime_rid() -> str:
    """Detects the .NET / Neoveg Runtime Identifier (RID) for the current OS & CPU."""
    sys_name = platform.system().lower()
    machine = platform.machine().lower()
    is_arm = "arm" in machine or "aarch64" in machine

    if sys_name == "windows":
        return "win-arm64" if is_arm else "win-x64"
    if sys_name == "darwin":
        return "osx-arm64" if is_arm else "osx-x64"
    return "linux-arm64" if is_arm else "linux-x64"


class IsolatedFFmpegManager:
    """Cross-platform isolated controller for FFmpeg and FFprobe."""

    def __init__(self, root_dir: Optional[Path] = None) -> None:
        self.root_dir = Path(root_dir) if root_dir else ROOT_DIR
        self.rid = detect_runtime_rid()
        self.is_windows = platform.system().lower() == "windows"
        self.is_macos = platform.system().lower() == "darwin"
        self.is_linux = platform.system().lower() == "linux"
        self.exe_suffix = ".exe" if self.is_windows else ""

    def get_isolated_bin_dir(self, rid: Optional[str] = None) -> Path:
        target_rid = rid or self.rid
        return self.root_dir / "runtimes" / target_rid / "ffmpeg" / "bin"

    def get_isolated_lib_dir(self, rid: Optional[str] = None) -> Path:
        target_rid = rid or self.rid
        return self.root_dir / "runtimes" / target_rid / "ffmpeg" / "lib"

    @staticmethod
    def _ensure_executable(binary_path: Path) -> None:
        if binary_path.exists() and os.name != "nt":
            try:
                mode = binary_path.stat().st_mode
                binary_path.chmod(mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
            except OSError:
                pass

    def resolve_binary(self, tool_name: str) -> Optional[Path]:
        """
        Resolves `ffmpeg` or `ffprobe` from the isolated portable runtime directory first,
        then falls back to system PATH.
        """
        fname = f"{tool_name}{self.exe_suffix}"
        candidates = [
            self.get_isolated_bin_dir() / fname,
            self.root_dir / "runtimes" / "ffmpeg" / fname,
            self.root_dir / "bin" / fname,
        ]
        for candidate in candidates:
            if candidate.exists() and candidate.is_file():
                self._ensure_executable(candidate)
                return candidate

        manifest_file = self.get_isolated_bin_dir() / "runtime_manifest.json"
        if manifest_file.exists():
            try:
                mdata = json.loads(manifest_file.read_text(encoding="utf-8"))
                linked_path = mdata.get("binaries", {}).get(tool_name)
                if linked_path:
                    binary_path = Path(linked_path)
                    if not binary_path.is_absolute():
                        binary_path = self.root_dir / binary_path
                    if binary_path.exists():
                        return binary_path
            except Exception:
                pass

        sys_which = shutil.which(tool_name)
        if sys_which:
            return Path(sys_which)
        return None

    def provision_isolated_runtime(self, copy_binaries: bool = False) -> Dict[str, Any]:
        """
        Ensures the isolated runtime folder (`runtimes/<rid>/ffmpeg/bin/`) is created and
        writes the isolated runtime manifest (or copies the binaries when building portable archives).
        """
        bin_dir = self.get_isolated_bin_dir()
        lib_dir = self.get_isolated_lib_dir()
        bin_dir.mkdir(parents=True, exist_ok=True)
        lib_dir.mkdir(parents=True, exist_ok=True)

        staged: Dict[str, str] = {}
        for tool in ("ffmpeg", "ffprobe"):
            target = bin_dir / f"{tool}{self.exe_suffix}"
            sys_bin = shutil.which(tool)
            if tool == "ffmpeg" and copy_binaries and sys_bin:
                need_copy = not target.exists() or target.stat().st_size < 1024 * 1024
                if need_copy:
                    try:
                        shutil.copy2(sys_bin, target)
                        self._ensure_executable(target)
                    except OSError:
                        pass
            elif tool == "ffprobe" and target.exists() and target.stat().st_size > 50 * 1024 * 1024:
                # Keep portable bundle within ~300MB budget by referencing ffprobe via runtime_manifest.json
                try:
                    target.unlink()
                except OSError:
                    pass
            resolved = target if target.exists() else (Path(sys_bin) if sys_bin else None)
            if resolved:
                try:
                    staged[tool] = resolved.resolve().relative_to(self.root_dir.resolve()).as_posix()
                except ValueError:
                    staged[tool] = str(resolved)

        manifest_file = bin_dir / "runtime_manifest.json"
        manifest_file.write_text(
            json.dumps(
                {
                    "rid": self.rid,
                    "os": platform.system(),
                    "isolated_bin_dir": bin_dir.relative_to(self.root_dir).as_posix(),
                    "isolated_lib_dir": lib_dir.relative_to(self.root_dir).as_posix(),
                    "binaries": staged,
                },
                indent=2,
            ),
            encoding="utf-8",
        )

        return {
            "ok": bool(staged.get("ffmpeg") and staged.get("ffprobe")),
            "rid": self.rid,
            "isolated_bin_dir": str(bin_dir),
            "staged_binaries": staged,
        }

    def build_isolated_env(self) -> Dict[str, str]:
        """
        Constructs an OS-isolated environment dictionary for spawning FFmpeg/FFprobe
        without leaking or conflicting with host libraries.
        """
        env = os.environ.copy()
        path_sep = ";" if self.is_windows else ":"

        bin_dir = str(self.get_isolated_bin_dir())
        lib_dir = str(self.get_isolated_lib_dir())
        env["PATH"] = f"{bin_dir}{path_sep}{env.get('PATH', '')}"

        # Configure OS-specific dynamic library isolation
        if self.is_linux:
            ld_prev = env.get("LD_LIBRARY_PATH", "")
            env["LD_LIBRARY_PATH"] = f"{lib_dir}:{ld_prev}" if ld_prev else lib_dir
        elif self.is_macos:
            dyld_prev = env.get("DYLD_LIBRARY_PATH", "")
            env["DYLD_LIBRARY_PATH"] = f"{lib_dir}:{dyld_prev}" if dyld_prev else lib_dir

        # Configure isolated Frei0r & LADSPA plugin paths per OS
        frei0r_local = str(PLUGINS_DIR / "frei0r")
        ladspa_local = str(PLUGINS_DIR / "ladspa")
        env["FREI0R_PATH"] = (
            f"{frei0r_local}{path_sep}{env['FREI0R_PATH']}"
            if env.get("FREI0R_PATH")
            else frei0r_local
        )
        env["LADSPA_PATH"] = (
            f"{ladspa_local}{path_sep}{env['LADSPA_PATH']}"
            if env.get("LADSPA_PATH")
            else ladspa_local
        )
        env["NEOVEG_ISOLATED_RUNTIME"] = self.rid
        return env

    def run_isolated(
        self,
        tool: str,
        args: List[str],
        timeout: int = 90,
    ) -> subprocess.CompletedProcess[str]:
        """Executes `ffmpeg` or `ffprobe` in an OS-isolated process group."""
        bin_path = self.resolve_binary(tool)
        if not bin_path:
            raise FileNotFoundError(f"Binary '{tool}' not found in isolated runtime or system PATH.")

        cmd = [str(bin_path), *args]
        kwargs: Dict[str, Any] = {
            "capture_output": True,
            "text": True,
            "encoding": "utf-8",
            "errors": "replace",
            "timeout": timeout,
            "env": self.build_isolated_env(),
            "check": False,
        }

        if self.is_windows:
            creation_flags = getattr(subprocess, "CREATE_NO_WINDOW", 0x08000000) | getattr(
                subprocess, "CREATE_NEW_PROCESS_GROUP", 0x00000200
            )
            kwargs["creationflags"] = creation_flags
        else:
            kwargs["start_new_session"] = True

        return subprocess.run(cmd, **kwargs)

    def get_status(self) -> Dict[str, Any]:
        """Returns diagnostic telemetry for the isolated FFmpeg/FFprobe manager."""
        ffmpeg_bin = self.resolve_binary("ffmpeg")
        ffprobe_bin = self.resolve_binary("ffprobe")
        version_line = "Unavailable"
        if ffmpeg_bin:
            try:
                proc = self.run_isolated("ffmpeg", ["-version"], timeout=5)
                if proc.stdout:
                    version_line = proc.stdout.splitlines()[0].strip()
            except Exception as exc:
                version_line = f"Error: {exc}"

        return {
            "ok": bool(ffmpeg_bin and ffprobe_bin),
            "os": platform.system(),
            "arch": platform.machine(),
            "rid": self.rid,
            "ffmpeg_path": str(ffmpeg_bin) if ffmpeg_bin else None,
            "ffprobe_path": str(ffprobe_bin) if ffprobe_bin else None,
            "is_bundled_portable": bool(
                (self.get_isolated_bin_dir() / "runtime_manifest.json").exists()
                or (ffmpeg_bin and str(self.root_dir / "runtimes") in str(ffmpeg_bin))
            ),
            "version": version_line,
        }

    def probe_media(self, media_path: str | Path) -> Dict[str, Any]:
        """Inspects a media file using isolated `ffprobe` and returns normalized stream metadata."""
        p = Path(media_path)
        if not p.exists():
            return {"ok": False, "error": f"File not found: {p}"}

        try:
            proc = self.run_isolated(
                "ffprobe",
                [
                    "-v",
                    "quiet",
                    "-print_format",
                    "json",
                    "-show_format",
                    "-show_streams",
                    str(p),
                ],
                timeout=12,
            )
            data = json.loads(proc.stdout or "{}")
            streams = data.get("streams", [])
            fmt = data.get("format", {})

            v_stream = next((s for s in streams if s.get("codec_type") == "video"), None)
            a_stream = next((s for s in streams if s.get("codec_type") == "audio"), None)
            duration = float(fmt.get("duration", 0.0) or 5.0)

            width = int(v_stream.get("width", 0)) if v_stream else 0
            height = int(v_stream.get("height", 0)) if v_stream else 0
            codec = (
                v_stream.get("codec_name")
                if v_stream
                else (a_stream.get("codec_name") if a_stream else "unknown")
            )

            fps = 29.97
            is_vfr = False
            if v_stream:
                r_rate = v_stream.get("r_frame_rate", "30/1")
                avg_rate = v_stream.get("avg_frame_rate", r_rate)
                is_vfr = r_rate != avg_rate
                if "/" in r_rate:
                    num, den = r_rate.split("/", 1)
                    if float(den or 1) > 0:
                        fps = round(float(num) / float(den), 3)

            sample_rate = int(a_stream.get("sample_rate", 48000)) if a_stream else 48000
            channels = int(a_stream.get("channels", 2)) if a_stream else 2

            return {
                "ok": True,
                "fileName": p.name,
                "filePath": str(p.resolve()),
                "type": "video" if v_stream else "audio",
                "duration": round(duration, 3),
                "width": width,
                "height": height,
                "fps": fps,
                "isVfr": is_vfr,
                "codec": str(codec).upper(),
                "sampleRate": sample_rate,
                "channels": channels,
            }
        except Exception as exc:
            return {"ok": False, "error": str(exc)}

    def transcode_vfr_to_cfr(
        self, input_path: str | Path, output_path: str | Path, target_fps: float = 29.97
    ) -> Dict[str, Any]:
        """Converts Variable Frame Rate (VFR) footage into Constant Frame Rate (CFR) using isolated FFmpeg."""
        proc = self.run_isolated(
            "ffmpeg",
            [
                "-y",
                "-i",
                str(input_path),
                "-vf",
                f"fps={target_fps:.2f}",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-crf",
                "20",
                "-c:a",
                "aac",
                str(output_path),
            ],
            timeout=120,
        )
        return {
            "ok": proc.returncode == 0 and Path(output_path).exists(),
            "returncode": proc.returncode,
            "output_path": str(output_path),
        }


def _cli_main() -> None:
    """Command-line entry point invoked by C# Avalonia UI (`FFmpegPythonService`)."""
    mgr = IsolatedFFmpegManager()
    cmd = sys.argv[1] if len(sys.argv) > 1 else "status"

    if cmd == "status":
        print(json.dumps(mgr.get_status()))
    elif cmd == "provision":
        print(json.dumps(mgr.provision_isolated_runtime()))
    elif cmd == "probe" and len(sys.argv) > 2:
        print(json.dumps(mgr.probe_media(sys.argv[2])))
    elif cmd == "render" and len(sys.argv) > 2:
        import rust_bridge
        import xml_interchange
        import zipfile

        proj_arg = sys.argv[2]
        out_name = sys.argv[3] if len(sys.argv) > 3 else "Neoveg_Master_Render.mp4"
        proj_path = Path(proj_arg)
        if proj_path.exists():
            if zipfile.is_zipfile(proj_path):
                project, _, _ = xml_interchange.load_neoveg_container(proj_path)
            else:
                project = json.loads(proj_path.read_text(encoding="utf-8"))
        else:
            project = json.loads(proj_arg)
        out_path = RENDERS_DIR / out_name
        plan = rust_bridge.compile_project_to_ffmpeg(project, MEDIA_DIR, out_path)
        proc = mgr.run_isolated("ffmpeg", plan["args"][1:], timeout=120)
        print(
            json.dumps(
                {
                    "ok": proc.returncode == 0 and out_path.exists(),
                    "output_path": str(out_path),
                    "size_bytes": out_path.stat().st_size if out_path.exists() else 0,
                    "command_str": plan["command_str"],
                    "stderr_tail": (proc.stderr or "")[-500:],
                }
            )
        )
    else:
        print(json.dumps({"ok": False, "error": f"Unknown command: {cmd}"}))


if __name__ == "__main__":
    _cli_main()
