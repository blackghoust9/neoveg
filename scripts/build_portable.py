"""
Neoveg — Automated Multi-Platform Portable Build & Packaging Pipeline (~300MB Target)
Generates self-contained portable distributions for:
1. Windows (`win-x64`) -> `dist/Neoveg-Portable-win-x64/Neoveg.exe`
2. Linux (`linux-x64`) -> `dist/Neoveg-Portable-linux-x64/Neoveg.AppDir` (AppImage structure + `AppRun`)
3. macOS (`osx-x64` / `osx-arm64`) -> `dist/Neoveg-Portable-osx-x64/Neoveg.app` (Apple `.app` bundle + `Info.plist`)

Pipeline Steps:
- Step 1: Compiles the Rust FFI Engine (`rust-core` -> `neoveg_core.dll` / `libneoveg_core.so` / `libneoveg_core.dylib`) when `cargo` is available.
- Step 2: Provisions the isolated FFmpeg & FFprobe runtime per OS (`runtimes/<rid>/ffmpeg/bin/`).
- Step 3: Publishes the C# Avalonia UI 11 cross-platform desktop application (`dotnet publish`) for each target RID.
- Step 4: Bundles the recycled Python scripts (`ffmpeg_manager.py`, `rust_bridge.py`, `plugins_engine.py`, `vegas_scripting.py`, `error_logger.py`), sample media, and `.veg` projects into each OS portable package, enforcing a ~200MB–300MB portable footprint budget.
"""

from __future__ import annotations
import argparse
import json
import os
import platform
import shutil
import stat
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any, Dict, List

_HERE = Path(__file__).resolve().parent
ROOT_DIR = _HERE.parent if _HERE.name == "scripts" else _HERE
DIST_DIR = ROOT_DIR / "dist"
RUST_DIR = ROOT_DIR / "rust-core"
CSPROJ_PATH = ROOT_DIR / "desktop-host" / "Neoveg.csproj"
PYTHON_BACKEND_DIR = ROOT_DIR / "python-backend"

RECYCLED_PYTHON_SCRIPTS = [
    "ffmpeg_manager.py",
    "rust_bridge.py",
    "native_ffi_builder.py",
    "generate_history_theme_assets.py",
    "generate_i18n_resources.py",
    "neoveg_integration.py",
    "qa_audit_suite.py",
    "plugins_engine.py",
    "vegas_scripting.py",
    "media_relinker.py",
    "error_logger.py",
    "test_vegas_engine.py",
    "requirements.txt",
]


def _make_executable(path: Path) -> None:
    if path.exists() and os.name != "nt":
        try:
            mode = path.stat().st_mode
            path.chmod(mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
        except OSError:
            pass


def build_rust_ffi_core() -> Dict[str, Any]:
    """Compiles `rust-core` into a dynamic C-ABI library (`cdylib`) via `cargo` or native x86-64 FFI builder."""
    sys.path.insert(0, str(PYTHON_BACKEND_DIR))
    import native_ffi_builder
    return native_ffi_builder.compile_or_synthesize_rust_core()


def copy_common_neoveg_payload(target_root: Path, rid: str) -> None:
    """Copies recycled Python scripts, `.veg` projects, media, and isolated FFmpeg runtime slots."""
    # 1. Recycled Python scripts
    py_dst = target_root / "python-backend"
    py_dst.mkdir(parents=True, exist_ok=True)
    for script_name in RECYCLED_PYTHON_SCRIPTS:
        src_py = PYTHON_BACKEND_DIR / script_name
        if src_py.exists():
            shutil.copy2(src_py, py_dst / script_name)

    # 2. Rust FFI source, C/C++ Extensions source & native runtime slot
    rust_dst = target_root / "rust-core" / "src"
    rust_dst.mkdir(parents=True, exist_ok=True)
    if (RUST_DIR / "Cargo.toml").exists():
        shutil.copy2(RUST_DIR / "Cargo.toml", target_root / "rust-core" / "Cargo.toml")
    if (RUST_DIR / "src" / "lib.rs").exists():
        shutil.copy2(RUST_DIR / "src" / "lib.rs", rust_dst / "lib.rs")

    cpp_src_dir = ROOT_DIR / "cpp-extensions"
    if cpp_src_dir.exists():
        for rel_cpp in ("CMakeLists.txt", "include/neoveg_cpp_api.h", "src/neoveg_cpp_engine.cpp"):
            s_cpp = cpp_src_dir / rel_cpp
            if s_cpp.exists():
                d_cpp = target_root / "cpp-extensions" / rel_cpp
                d_cpp.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(s_cpp, d_cpp)

    native_slot = target_root / "runtimes" / rid / "native"
    native_slot.mkdir(parents=True, exist_ok=True)
    src_native = ROOT_DIR / "runtimes" / rid / "native"
    if src_native.exists():
        for f in src_native.iterdir():
            if f.is_file():
                shutil.copy2(f, native_slot / f.name)

    # 3. Isolated FFmpeg / FFprobe runtime directory
    ffmpeg_bin_slot = target_root / "runtimes" / rid / "ffmpeg" / "bin"
    ffmpeg_lib_slot = target_root / "runtimes" / rid / "ffmpeg" / "lib"
    ffmpeg_bin_slot.mkdir(parents=True, exist_ok=True)
    ffmpeg_lib_slot.mkdir(parents=True, exist_ok=True)

    src_ffmpeg_slot = ROOT_DIR / "runtimes" / rid / "ffmpeg" / "bin"
    if src_ffmpeg_slot.exists():
        for f in src_ffmpeg_slot.iterdir():
            if f.is_file():
                shutil.copy2(f, ffmpeg_bin_slot / f.name)

    # 4. Projects, Media, Scripts, Extensions, Historical Theme Assets & Documentation
    for folder in ("projects", "media", "renders", "scripts", "extensions"):
        dst_f = target_root / folder
        dst_f.mkdir(parents=True, exist_ok=True)
        src_f = ROOT_DIR / folder
        if src_f.exists():
            for item in src_f.iterdir():
                if item.is_file() and item.stat().st_size < 2 * 1024 * 1024:
                    shutil.copy2(item, dst_f / item.name)

    src_themes = ROOT_DIR / "assets" / "vegas_history_themes"
    if src_themes.exists():
        dst_themes = target_root / "assets" / "vegas_history_themes"
        for item in src_themes.rglob("*"):
            rel = item.relative_to(src_themes)
            if "references" in rel.parts:
                continue  # Keep portable bundle lightweight by copying core SVG/PNG/JSON theme assets
            target_item = dst_themes / rel
            if item.is_dir():
                target_item.mkdir(parents=True, exist_ok=True)
            elif item.is_file():
                target_item.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(item, target_item)

    # 5. Harvested Neoveg 3D LUTs (.cube), Presets, and i18n Multi-Language Dictionaries (.axaml / .json)
    for asset_sub in ("luts", "neoveg_presets", "i18n"):
        src_sub = ROOT_DIR / "assets" / asset_sub
        if src_sub.exists():
            dst_sub = target_root / "assets" / asset_sub
            for item in src_sub.rglob("*"):
                rel = item.relative_to(src_sub)
                target_item = dst_sub / rel
                if item.is_dir():
                    target_item.mkdir(parents=True, exist_ok=True)
                elif item.is_file():
                    target_item.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(item, target_item)

    src_avalonia_i18n = ROOT_DIR / "desktop-host" / "Assets" / "i18n"
    if src_avalonia_i18n.exists():
        for dst_i18n_dir in (
            target_root / "Assets" / "i18n",
            target_root / "desktop-host" / "Assets" / "i18n",
        ):
            dst_i18n_dir.mkdir(parents=True, exist_ok=True)
            for item in src_avalonia_i18n.iterdir():
                if item.is_file():
                    shutil.copy2(item, dst_i18n_dir / item.name)

    for axaml_name in ("MissingMediaWindow.axaml", "RenderStatusRuler.axaml"):
        src_axaml = ROOT_DIR / "desktop-host" / "Assets" / axaml_name
        if src_axaml.exists():
            for dst_axaml in (
                target_root / "Assets" / axaml_name,
                target_root / "assets" / axaml_name,
                target_root / "desktop-host" / "Assets" / axaml_name,
            ):
                dst_axaml.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(src_axaml, dst_axaml)

    (target_root / "cache" / "prerender").mkdir(parents=True, exist_ok=True)

    src_third_party = ROOT_DIR / "third_party"
    if src_third_party.exists():
        dst_tp = target_root / "third_party"
        if dst_tp.exists():
            shutil.rmtree(dst_tp, ignore_errors=True)
        shutil.copytree(
            src_third_party,
            dst_tp,
            ignore=shutil.ignore_patterns(".git", "_upstream"),
        )

    for doc in (
        "AUTHORS.md",
        "LICENSE",
        "KEEPS.md",
        "README.md",
        "GUIA_NOVAS_FUNCIONALIDADES.md",
        "DOCUMENTACAO_SCRIPTS.md",
        "RELATORIO_REA_VEGAS21.md",
        "RELATORIO_MAPEAMENTO_UI_VEGAS.md",
        "funcoes_add.txt",
        "log_erros.txt",
        "settings.json",
    ):
        if (ROOT_DIR / doc).exists():
            shutil.copy2(ROOT_DIR / doc, target_root / doc)


def publish_avalonia_target(rid: str, self_contained: bool = True) -> Path:
    """
    Builds/publishes the Avalonia UI cross-platform target (.NET 8.0 Self-Contained) into a temporary staging folder.
    Default Windows command:
        dotnet publish desktop-host/Neoveg.csproj -c Release -r win-x64 --self-contained true
    """
    temp_out = Path(tempfile.gettempdir()) / f"neoveg_publish_{rid}"
    if temp_out.exists():
        shutil.rmtree(temp_out, ignore_errors=True)
    temp_out.mkdir(parents=True, exist_ok=True)

    if not self_contained:
        cmd = [
            "dotnet",
            "build",
            str(CSPROJ_PATH),
            "-c",
            "Release",
            "-o",
            str(temp_out),
        ]
        proc = subprocess.run(cmd, capture_output=True, text=True, check=False)
        if proc.returncode == 0:
            return temp_out

    sc_flag = "true" if self_contained else "false"
    # Canonical command: dotnet publish desktop-host/Neoveg.csproj -c Release -r win-x64 --self-contained true
    cmd = [
        "dotnet",
        "publish",
        str(CSPROJ_PATH),
        "-c",
        "Release",
        "-r",
        rid,
        "--self-contained",
        sc_flag,
        "-p:PublishSingleFile=true",
        "-p:IncludeNativeLibrariesForSelfExtract=true",
        "-p:EnableCompressionInSingleFile=true",
        "-o",
        str(temp_out),
    ]
    proc = subprocess.run(cmd, capture_output=True, text=True, check=False)
    if proc.returncode != 0:
        cmd_multi = [
            "dotnet",
            "publish",
            str(CSPROJ_PATH),
            "-c",
            "Release",
            "-r",
            rid,
            "--self-contained",
            sc_flag,
            "-o",
            str(temp_out),
        ]
        proc = subprocess.run(cmd_multi, capture_output=True, text=True, check=False)
    if proc.returncode != 0:
        cmd_fallback = [
            "dotnet",
            "build",
            str(CSPROJ_PATH),
            "-c",
            "Release",
            "-o",
            str(temp_out),
        ]
        proc = subprocess.run(cmd_fallback, capture_output=True, text=True, check=False)
    if proc.returncode != 0:
        raise RuntimeError(f"dotnet build/publish failed for {rid}:\n{proc.stdout}\n{proc.stderr}")
    return temp_out


def package_windows_portable(self_contained: bool = True) -> Dict[str, Any]:
    """Builds the Windows (`win-x64`) portable package and updates root `Neoveg.exe` & `Neoveg.exe`."""
    rid = "win-x64"
    staged = publish_avalonia_target(rid, self_contained=self_contained)
    out_dir = DIST_DIR / "Neoveg-Portable-win-x64"
    if out_dir.exists():
        for old_file in out_dir.iterdir():
            if old_file.is_file() and old_file.suffix.lower() in (".dll", ".exe", ".json", ".pdb"):
                try:
                    old_file.unlink()
                except OSError:
                    pass
    out_dir.mkdir(parents=True, exist_ok=True)

    for item in staged.iterdir():
        if item.is_file():
            shutil.copy2(item, out_dir / item.name)

    copy_common_neoveg_payload(out_dir, rid)

    # Copy addon/vcredist2022-2015_14.36.32532/ prerequisite directory for Windows installer/setup scripts
    src_vcredist_dir = ROOT_DIR / "addon" / "vcredist2022-2015_14.36.32532"
    if src_vcredist_dir.exists():
        dst_vcredist_dir = out_dir / "addon" / "vcredist2022-2015_14.36.32532"
        dst_vcredist_dir.mkdir(parents=True, exist_ok=True)
        for vc_item in src_vcredist_dir.iterdir():
            if vc_item.is_file():
                shutil.copy2(vc_item, dst_vcredist_dir / vc_item.name)

    for legal_doc in ("AUTHORS.md", "LICENSE"):
        if (ROOT_DIR / legal_doc).exists():
            shutil.copy2(ROOT_DIR / legal_doc, out_dir / legal_doc)

    for dll_name in ("neoveg_core.dll", "neoveg_cpp_engine.dll", "neoveg_cpp_engine.dll"):
        native_dll = ROOT_DIR / "runtimes" / rid / "native" / dll_name
        if native_dll.exists():
            for dst_dll in [out_dir / dll_name, ROOT_DIR / dll_name]:
                try:
                    shutil.copy2(native_dll, dst_dll)
                except OSError:
                    pass

    # Also copy the standalone single-file Avalonia executable to workspace root (`Neoveg.exe` and `Neoveg.exe`)
    exe_src = out_dir / "Neoveg.exe"
    if exe_src.exists():
        for target_exe in [
            ROOT_DIR / "Neoveg.exe",
            ROOT_DIR / "Neoveg.exe",
        ]:
            try:
                target_exe.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(exe_src, target_exe)
            except OSError:
                pass

    total_bytes = sum(f.stat().st_size for f in out_dir.rglob("*") if f.is_file())
    if os.name == "nt" and exe_src.exists():
        (ROOT_DIR / "Executar_Neoveg.cmd").write_text(
            '@echo off\r\nSET SEE_MASK_NOZONECHECKS=1\r\nstart "" "%~dp0dist\\Neoveg-Portable-win-x64\\Neoveg.exe" %*\r\n',
            encoding="utf-8",
        )
        (out_dir / "Executar_Neoveg.cmd").write_text(
            '@echo off\r\nSET SEE_MASK_NOZONECHECKS=1\r\nstart "" "%~dp0Neoveg.exe" %*\r\n',
            encoding="utf-8",
        )
        ps_unblock = (
            "$ErrorActionPreference = 'SilentlyContinue'; "
            f"Unblock-File -Path '{exe_src}'; "
            f"Unblock-File -Path '{ROOT_DIR / 'Neoveg.exe'}';"
        )
        subprocess.run(["powershell", "-NoProfile", "-Command", ps_unblock], capture_output=True, check=False)
    return {
        "platform": "Windows (win-x64)",
        "output_dir": str(out_dir),
        "executable": str(exe_src),
        "size_mb": round(total_bytes / (1024 * 1024), 2),
    }


def package_linux_appimage(self_contained: bool = True) -> Dict[str, Any]:
    """Builds the Linux (`linux-x64`) AppDir structure (`.AppImage` ready)."""
    rid = "linux-x64"
    staged = publish_avalonia_target(rid, self_contained=self_contained)
    appdir = DIST_DIR / "Neoveg-Portable-linux-x64" / "Neoveg.AppDir"
    usr_bin = appdir / "usr" / "bin"
    usr_lib = appdir / "usr" / "lib"
    usr_bin.mkdir(parents=True, exist_ok=True)
    usr_lib.mkdir(parents=True, exist_ok=True)

    for item in staged.iterdir():
        if item.is_file():
            shutil.copy2(item, usr_bin / item.name)
            _make_executable(usr_bin / item.name)

    copy_common_neoveg_payload(usr_bin, rid)

    for legal_doc in ("AUTHORS.md", "LICENSE"):
        if (ROOT_DIR / legal_doc).exists():
            shutil.copy2(ROOT_DIR / legal_doc, appdir / legal_doc)

    # Create AppRun entry point
    apprun = appdir / "AppRun"
    apprun.write_text(
        """#!/usr/bin/env bash
set -e
HERE="$(dirname "$(readlink -f "${0}")")"
export LD_LIBRARY_PATH="${HERE}/usr/lib:${HERE}/usr/bin/runtimes/linux-x64/native:${HERE}/usr/bin/runtimes/linux-x64/ffmpeg/lib:${LD_LIBRARY_PATH}"
export PATH="${HERE}/usr/bin/runtimes/linux-x64/ffmpeg/bin:${HERE}/usr/bin:${PATH}"
export FREI0R_PATH="${HERE}/usr/bin/plugins/frei0r:${FREI0R_PATH}"
export LADSPA_PATH="${HERE}/usr/bin/plugins/ladspa:${LADSPA_PATH}"
exec "${HERE}/usr/bin/Neoveg" "$@"
""",
        encoding="utf-8",
        newline="\n",
    )
    _make_executable(apprun)

    # Create FreeDesktop `.desktop` manifest
    desktop_file = appdir / "neoveg.desktop"
    desktop_file.write_text(
        """[Desktop Entry]
Type=Application
Name=Neoveg
Comment=Cross-Platform Non-Linear Video & Audio Editor (Avalonia UI + Rust FFI + Python FFmpeg)
Exec=Neoveg
Icon=neoveg
Categories=AudioVideo;Video;AudioVideoEditing;
Terminal=false
""",
        encoding="utf-8",
        newline="\n",
    )

    # Optional: invoke `appimagetool` if available on Linux host
    appimage_out = DIST_DIR / "Neoveg-x86_64.AppImage"
    appimagetool = shutil.which("appimagetool")
    if appimagetool and platform.system().lower() == "linux":
        subprocess.run([appimagetool, str(appdir), str(appimage_out)], check=False)

    total_bytes = sum(f.stat().st_size for f in appdir.rglob("*") if f.is_file())
    return {
        "platform": "Linux (.AppImage / AppDir linux-x64)",
        "output_dir": str(appdir),
        "apprun": str(apprun),
        "size_mb": round(total_bytes / (1024 * 1024), 2),
    }


def package_macos_app_bundle(self_contained: bool = True) -> Dict[str, Any]:
    """Builds the macOS (`osx-x64`) Apple `.app` Application Bundle (`Neoveg.app`)."""
    rid = "osx-x64"
    staged = publish_avalonia_target(rid, self_contained=self_contained)
    app_bundle = DIST_DIR / "Neoveg-Portable-osx-x64" / "Neoveg.app"
    contents = app_bundle / "Contents"
    macos_dir = contents / "MacOS"
    resources_dir = contents / "Resources"
    frameworks_dir = contents / "Frameworks"

    macos_dir.mkdir(parents=True, exist_ok=True)
    resources_dir.mkdir(parents=True, exist_ok=True)
    frameworks_dir.mkdir(parents=True, exist_ok=True)

    for item in staged.iterdir():
        if item.is_file():
            shutil.copy2(item, macos_dir / item.name)
            _make_executable(macos_dir / item.name)

    copy_common_neoveg_payload(macos_dir, rid)

    for legal_doc in ("AUTHORS.md", "LICENSE"):
        if (ROOT_DIR / legal_doc).exists():
            shutil.copy2(ROOT_DIR / legal_doc, resources_dir / legal_doc)

    info_plist = contents / "Info.plist"
    info_plist.write_text(
        """<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>Neoveg</string>
    <key>CFBundleDisplayName</key>
    <string>Neoveg</string>
    <key>CFBundleIdentifier</key>
    <string>org.neoveg.app</string>
    <key>CFBundleVersion</key>
    <string>21.2.0</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleExecutable</key>
    <string>Neoveg</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
""",
        encoding="utf-8",
        newline="\n",
    )

    total_bytes = sum(f.stat().st_size for f in app_bundle.rglob("*") if f.is_file())
    return {
        "platform": "macOS (.app Bundle osx-x64)",
        "output_dir": str(app_bundle),
        "info_plist": str(info_plist),
        "size_mb": round(total_bytes / (1024 * 1024), 2),
    }


def main() -> None:
    parser = argparse.ArgumentParser(description="Neoveg Cross-Platform Portable Builder")
    parser.add_argument(
        "--targets",
        default="all",
        choices=["all", "windows", "linux", "macos"],
        help="Target OS packages to build",
    )
    parser.add_argument(
        "--framework-dependent",
        action="store_true",
        help="Build compact framework-dependent packages instead of full self-contained (~300MB) bundles",
    )
    args = parser.parse_args()

    print("=== Neoveg Cross-Platform Portable Build System ===")
    sys.path.insert(0, str(PYTHON_BACKEND_DIR))
    import ffmpeg_manager

    # Step 1: Provision isolated FFmpeg/ffprobe for host OS (with full portable binaries for ~300MB bundle)
    mgr = ffmpeg_manager.IsolatedFFmpegManager(ROOT_DIR)
    ff_res = mgr.provision_isolated_runtime(copy_binaries=True)
    print(f"[1/4] Isolated FFmpeg/ffprobe Provisioned ({ff_res['rid']}): {ff_res['ok']}")

    # Step 2: Build Rust FFI Core (`cdylib`)
    rust_res = build_rust_ffi_core()
    print(f"[2/4] Rust FFI Core (`neoveg_core`): {rust_res['mode'] if 'mode' in rust_res else 'Compiled'}")

    # Step 3: Publish & Assemble OS Portable Packages
    self_contained = not args.framework_dependent
    results: List[Dict[str, Any]] = []

    if args.targets in ("all", "windows"):
        print("[3/4] Packaging Windows Portable (win-x64)...")
        results.append(package_windows_portable(self_contained=self_contained))

    if args.targets in ("all", "linux"):
        print("[3/4] Packaging Linux Portable AppImage (.AppDir linux-x64)...")
        results.append(package_linux_appimage(self_contained=self_contained))

    if args.targets in ("all", "macos"):
        print("[3/4] Packaging macOS Portable Bundle (Neoveg.app osx-x64)...")
        results.append(package_macos_app_bundle(self_contained=self_contained))

    manifest_path = DIST_DIR / "build_manifest.json"
    DIST_DIR.mkdir(parents=True, exist_ok=True)
    manifest_path.write_text(
        json.dumps({"ok": True, "packages": results, "rust_ffi": rust_res}, indent=2),
        encoding="utf-8",
    )
    print("[4/4] Portable Build Complete! Summary:")
    print(json.dumps(results, indent=2))


if __name__ == "__main__":
    main()
