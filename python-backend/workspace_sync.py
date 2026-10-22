"""
Neoveg — Workspace & Directory Synchronization Engine
Implements Section 7 (Workspace & Directory Synchronization Protocol) of AGENTS.md:
- Verifies read/write permissions on `G:\\Meu Drive\\vp-rev-eng\\teste\\teste2`.
- Mirrors/synchronizes updated C# source files, Rust crates (`rust-core/`), C++ extensions (`cpp-extensions/`),
  Python scripts (`python-backend/`), `Assets/i18n/`, and project documentation to Google Drive while
  honoring `.gitignore`. Local build outputs may still be copied to C# output directories and
  `%LOCALAPPDATA%\\Neoveg\\AppCache\\`; ignored files are not mirrored to Google Drive.
"""

from __future__ import annotations
import os
import shutil
import tempfile
from fnmatch import fnmatchcase
from pathlib import Path
from typing import Any, Dict, List

ROOT_DIR = Path(__file__).resolve().parent.parent
GDRIVE_TARGET_DIR = Path(r"G:\Meu Drive\vp-rev-eng\teste\teste2")
IGNORED_DIRECTORY_NAMES = {
    ".git",
    ".vs",
    ".idea",
    "AppCache",
    "bin",
    "build",
    "CMakeFiles",
    "env",
    "logs",
    "obj",
    "out",
    "target",
    "temp",
    "venv",
    ".venv",
    ".pytest_cache",
    "__pycache__",
}
IGNORED_FILE_PATTERNS = (
    "*.log",
    "*.sfk",
    "*.sfvp0",
    "*.tmp",
    "*.user",
    "*.userosscache",
    "*.sln.docstates",
    "*.rs.bk",
    "*.dir",
    "*.VC.db",
    "*.opendb",
    "*.ipch",
    "*.idb",
    "*.pdb",
    "*.ilk",
    "*.exp",
    "*.obj",
    "*.o",
    "*.lib",
    "*.a",
    "*.exe",
    "*.dll",
    "*.so",
    "*.dylib",
    "*.pyc",
    "*.pyo",
    "*.pyd",
    "*.class",
    "*.egg-info",
    "*.dir",
    "*.swp",
    "*.swo",
    "*.gslides",
    "*.gdoc",
    "*.gsheet",
)
ALLOWED_VSCODE_FILES = {"settings.json", "tasks.json", "launch.json", "cmake-kits.json"}


def _is_ignored_for_sync(item: Path) -> bool:
    relative_path = item.relative_to(ROOT_DIR)
    if any(part in IGNORED_DIRECTORY_NAMES for part in relative_path.parts[:-1]):
        return True
    if any(
        fnmatchcase(part, "*.egg-info")
        or fnmatchcase(part, "*.dir")
        or part.lower().startswith("testresult")
        or part.lower() in {"debug", "release"}
        for part in relative_path.parts[:-1]
    ):
        return True
    if any(fnmatchcase(item.name, pattern) for pattern in IGNORED_FILE_PATTERNS):
        return True
    if item.name in {".DS_Store", "Thumbs.db", "desktop.ini", "CMakeCache.txt", "Makefile", "cmake_install.cmake"}:
        return True
    if relative_path.parts[:1] == (".vscode",) and item.name not in ALLOWED_VSCODE_FILES:
        return True
    if relative_path.parts[:2] == ("docs", "coverage"):
        return True
    return False


def _copy_if_newer_or_missing(src: Path, dst: Path) -> bool:
    if not src.exists() or not src.is_file():
        return False
    try:
        if src.resolve() == dst.resolve():
            return False
    except OSError:
        pass
    dst.parent.mkdir(parents=True, exist_ok=True)
    try:
        if not dst.exists() or src.stat().st_size != dst.stat().st_size or abs(src.stat().st_mtime - dst.stat().st_mtime) > 1.0:
            shutil.copy2(src, dst)
            return True
    except OSError:
        pass
    return False


def _sync_tree(src_dir: Path, dst_dir: Path, skip_filenames: tuple[str, ...] = ()) -> int:
    if not src_dir.exists():
        return 0
    copied = 0
    for item in src_dir.rglob("*"):
        if item.name in skip_filenames:
            continue
        if item.is_file() and not _is_ignored_for_sync(item):
            rel = item.relative_to(src_dir)
            if _copy_if_newer_or_missing(item, dst_dir / rel):
                copied += 1
    return copied


def synchronize_workspace() -> Dict[str, Any]:
    """Executes the Section 7 Workspace & Directory Synchronization Protocol."""
    target_accessible = GDRIVE_TARGET_DIR.exists() and os.access(GDRIVE_TARGET_DIR, os.R_OK | os.W_OK)
    logs_dir = ROOT_DIR / "logs"
    logs_dir.mkdir(parents=True, exist_ok=True)

    neoveg_log = logs_dir / "neoveg.log"
    crash_log = logs_dir / "crash.log"
    if not neoveg_log.exists():
        neoveg_log.write_text("", encoding="utf-8")
    if not crash_log.exists():
        crash_log.write_text("", encoding="utf-8")

    synced_files = 0
    if target_accessible and ROOT_DIR.resolve() != GDRIVE_TARGET_DIR.resolve():
        for doc in (".gitignore", "AGENTS.md", "AUTHORS.md", "LICENSE", "README.md", "Neoveg.sln", "requirements.txt", "build_portable.py"):
            if _copy_if_newer_or_missing(ROOT_DIR / doc, GDRIVE_TARGET_DIR / doc):
                synced_files += 1
        for folder in ("desktop-host", "rust-core", "cpp-extensions", "python-backend", "Assets", "scripts"):
            synced_files += _sync_tree(ROOT_DIR / folder, GDRIVE_TARGET_DIR / folder)

    # Mirror compiled C# binaries from %TEMP%\NeovegBin\Debug\net8.0 and net10.0
    temp_bin_root = Path(tempfile.gettempdir()) / "NeovegBin" / "Debug"
    native_slot = ROOT_DIR / "runtimes" / "win-x64" / "native"
    csharp_out_dirs: List[Path] = [
        ROOT_DIR / "desktop-host" / "bin" / "Debug" / "net8.0",
        ROOT_DIR / "desktop-host" / "bin" / "Debug" / "net10.0",
    ]
    mirrored_outputs: List[str] = []
    for out_dir in csharp_out_dirs:
        tfm = out_dir.name
        out_dir.mkdir(parents=True, exist_ok=True)
        temp_tfm_dir = temp_bin_root / tfm
        if temp_tfm_dir.exists():
            for compiled_file in temp_tfm_dir.glob("*"):
                if compiled_file.is_file():
                    _copy_if_newer_or_missing(compiled_file, out_dir / compiled_file.name)

        # Remove any stale RenderStatusRuler.axaml inside bin/.../Assets/ to prevent AVLN2002 in DesignTimeBuild
        stale_axaml = out_dir / "Assets" / "RenderStatusRuler.axaml"
        if stale_axaml.exists():
            try:
                stale_axaml.unlink()
            except OSError:
                pass

        for dll_name in ("neoveg_core.dll", "neoveg_cpp_engine.dll", "WebView2Loader.dll"):
            src_dll = native_slot / dll_name
            if src_dll.exists():
                _copy_if_newer_or_missing(src_dll, out_dir / dll_name)
                _copy_if_newer_or_missing(src_dll, out_dir / "runtimes" / "win-x64" / "native" / dll_name)
        _sync_tree(ROOT_DIR / "Assets", out_dir / "Assets", skip_filenames=("RenderStatusRuler.axaml",))
        _sync_tree(ROOT_DIR / "scripts", out_dir / "scripts")
        mirrored_outputs.append(str(out_dir))

    # Also update standalone binaries in ROOT_DIR and %LOCALAPPDATA%\Neoveg\AppCache\
    net10_src = temp_bin_root / "net10.0"
    if not net10_src.exists():
        net10_src = ROOT_DIR / "desktop-host" / "bin" / "Debug" / "net10.0"

    local_app_data = os.environ.get("LOCALAPPDATA")
    app_cache_dir = Path(local_app_data) / "Neoveg" / "AppCache" if local_app_data else None

    if net10_src.exists():
        for bin_item in net10_src.glob("*"):
            if bin_item.is_file() and bin_item.suffix.lower() in (".exe", ".dll", ".json", ".pdb"):
                _copy_if_newer_or_missing(bin_item, ROOT_DIR / bin_item.name)
                if bin_item.name.lower() == "neoveg.exe":
                    _copy_if_newer_or_missing(bin_item, ROOT_DIR / "neoveg.exe")
                if app_cache_dir is not None:
                    _copy_if_newer_or_missing(bin_item, app_cache_dir / bin_item.name)

    if app_cache_dir is not None:
        app_cache_dir.mkdir(parents=True, exist_ok=True)
        for dll_name in ("neoveg_core.dll", "neoveg_cpp_engine.dll", "WebView2Loader.dll"):
            src_dll = native_slot / dll_name
            if src_dll.exists():
                _copy_if_newer_or_missing(src_dll, app_cache_dir / dll_name)
                _copy_if_newer_or_missing(src_dll, app_cache_dir / "runtimes" / "win-x64" / "native" / dll_name)
        _sync_tree(ROOT_DIR / "Assets", app_cache_dir / "Assets", skip_filenames=("RenderStatusRuler.axaml",))
        _sync_tree(ROOT_DIR / "scripts", app_cache_dir / "scripts")

    return {
        "ok": True,
        "target_path": str(GDRIVE_TARGET_DIR),
        "target_accessible": target_accessible,
        "synced_files": synced_files,
        "csharp_output_dirs": mirrored_outputs,
        "logs_dir": str(logs_dir),
    }


if __name__ == "__main__":
    print(synchronize_workspace())
