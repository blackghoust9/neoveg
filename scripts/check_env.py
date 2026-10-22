#!/usr/bin/env python3
"""
Neoveg — Developer Environment & Prerequisite Diagnostic Script (`scripts/check_env.py`)
Verifies the presence and version of:
1. .NET 8 SDK & Runtime (`dotnet`)
2. Rust Toolchain (`cargo` / `rustc` 1.75+)
3. CMake (`cmake` 3.20+) & C++17/C++20 Compiler (`cl`, `g++`, `clang++`, MSVC)
4. Python 3.10+ & `python-backend/requirements.txt` packages (`pytest`, `numpy`, `scipy`, `Pillow`, `requests`)
5. Visual C++ Redistributable & DirectX Runtimes on Windows (`vcruntime140.dll`, `msvcp140.dll`, `d3d11.dll`, `XAudio2_7.dll`)
   or System Shared Libraries on Linux/macOS
6. Docker (`docker` 24.0+ for headless QA & multi-stage builds)
7. Neoveg Native FFI Shared Libraries (`neoveg_core`, `neoveg_cpp_engine`)

Logs the diagnostic summary to `logs/neoveg.log` and prints a formatted status table to the terminal.
"""

from __future__ import annotations
import argparse
import ctypes.util
from datetime import datetime
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
from typing import Any, Dict, List, Optional, Tuple

ROOT_DIR = Path(__file__).resolve().parent.parent
LOGS_DIR = ROOT_DIR / "logs"
ACTIVE_LOG_FILE = LOGS_DIR / "neoveg.log"


def _run_cmd(cmd: List[str], timeout: int = 8) -> Tuple[bool, str]:
    try:
        proc = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
        out = (proc.stdout or "").strip() or (proc.stderr or "").strip()
        return proc.returncode == 0, out
    except Exception as exc:
        return False, str(exc)


def _parse_version_tuple(text: str) -> Tuple[int, ...]:
    match = re.search(r"(\d+)\.(\d+)(?:\.(\d+))?", text)
    if not match:
        return (0, 0, 0)
    return tuple(int(part) if part is not None else 0 for part in match.groups())


def check_dotnet() -> Dict[str, Any]:
    dotnet_bin = shutil.which("dotnet")
    if not dotnet_bin:
        for candidate in (
            Path(os.environ.get("ProgramFiles", r"C:\Program Files")) / "dotnet" / "dotnet.exe",
            Path("/usr/share/dotnet/dotnet"),
            Path("/usr/local/share/dotnet/dotnet"),
        ):
            if candidate.exists():
                dotnet_bin = str(candidate)
                break

    if not dotnet_bin:
        return {
            "component": ".NET 8 SDK",
            "status": "MISSING",
            "ok": False,
            "version": "Not found",
            "details": "Install .NET 8 SDK from https://dotnet.microsoft.com/download/dotnet/8.0",
        }

    ok, ver_out = _run_cmd([dotnet_bin, "--version"])
    _, sdks_out = _run_cmd([dotnet_bin, "--list-sdks"])
    has_net8 = "8." in ver_out or any(line.strip().startswith("8.") for line in sdks_out.splitlines())
    ver_tuple = _parse_version_tuple(ver_out)
    meets_min = has_net8 or ver_tuple >= (8, 0, 0)

    return {
        "component": ".NET 8 SDK",
        "status": "OK" if (ok and meets_min) else "WARN",
        "ok": bool(ok and meets_min),
        "version": ver_out.splitlines()[0] if ver_out else "Unknown",
        "details": "Ready for desktop-host (Neoveg.csproj)"
        if meets_min
        else "Upgrade to .NET 8.0+ (https://dotnet.microsoft.com/download/dotnet/8.0)",
    }


def check_rust_cargo() -> Dict[str, Any]:
    cargo_bin = shutil.which("cargo")
    if not cargo_bin:
        cargo_home = Path.home() / ".cargo" / "bin" / ("cargo.exe" if os.name == "nt" else "cargo")
        if cargo_home.exists():
            cargo_bin = str(cargo_home)

    if cargo_bin:
        ok, ver_out = _run_cmd([cargo_bin, "--version"])
        ver_tuple = _parse_version_tuple(ver_out)
        meets_min = ver_tuple >= (1, 75, 0)
        return {
            "component": "Rust / Cargo (1.75+)",
            "status": "OK" if (ok and meets_min) else "WARN",
            "ok": bool(ok and meets_min),
            "version": ver_out.splitlines()[0] if ver_out else "Unknown",
            "details": "Ready for rust-core (neoveg_core)"
            if meets_min
            else "Update Rust via `rustup update stable` (requires 1.75+)",
        }

    # Check if prebuilt neoveg_core native binary is present as fallback
    native_dll = ROOT_DIR / "runtimes" / "win-x64" / "native" / "neoveg_core.dll"
    return {
        "component": "Rust / Cargo (1.75+)",
        "status": "WARN" if native_dll.exists() else "MISSING",
        "ok": False,
        "version": "cargo not in PATH",
        "details": (
            "cargo CLI not in PATH (prebuilt neoveg_core binary present; install Rust via https://rustup.rs)"
            if native_dll.exists()
            else "Install Rust 1.75+ via https://sh.rustup.rs"
        ),
    }


def check_cmake_and_cpp() -> Dict[str, Any]:
    cmake_bin = shutil.which("cmake")
    if not cmake_bin and os.name == "nt":
        for candidate in (
            Path(os.environ.get("ProgramFiles", r"C:\Program Files")) / "CMake" / "bin" / "cmake.exe",
            Path(os.environ.get("ProgramFiles", r"C:\Program Files"))
            / "Microsoft Visual Studio"
            / "2022"
            / "Community"
            / "Common7"
            / "IDE"
            / "CommonExtensions"
            / "Microsoft"
            / "CMake"
            / "CMake"
            / "bin"
            / "cmake.exe",
            Path(os.environ.get("ProgramFiles(x86)", r"C:\Program Files (x86)"))
            / "Microsoft Visual Studio"
            / "2022"
            / "BuildTools"
            / "Common7"
            / "IDE"
            / "CommonExtensions"
            / "Microsoft"
            / "CMake"
            / "CMake"
            / "bin"
            / "cmake.exe",
        ):
            if candidate.exists():
                cmake_bin = str(candidate)
                break

    cmake_ver = "Not found"
    cmake_ok = False
    if cmake_bin:
        ok, out = _run_cmd([cmake_bin, "--version"])
        if ok and out:
            cmake_ver = out.splitlines()[0].replace("cmake version ", "").strip()
            cmake_ok = _parse_version_tuple(cmake_ver) >= (3, 20, 0)

    compiler_found: Optional[str] = None
    for comp in ("cl", "clang++", "g++", "c++"):
        if shutil.which(comp):
            compiler_found = comp
            break

    if not compiler_found and os.name == "nt":
        vswhere = (
            Path(os.environ.get("ProgramFiles(x86)", r"C:\Program Files (x86)"))
            / "Microsoft Visual Studio"
            / "Installer"
            / "vswhere.exe"
        )
        if vswhere.exists():
            ok, vs_out = _run_cmd([str(vswhere), "-latest", "-property", "displayName"])
            if ok and vs_out:
                compiler_found = vs_out.splitlines()[0].strip()

    status = "OK" if cmake_ok else ("WARN" if cmake_bin or compiler_found else "MISSING")
    return {
        "component": "CMake 3.20+ & C++17",
        "status": status,
        "ok": cmake_ok,
        "version": f"CMake {cmake_ver} | C++: {compiler_found or 'MSVC/System'}",
        "details": (
            "Ready for cpp-extensions (neoveg_cpp_engine)"
            if cmake_ok
            else "Install CMake 3.20+ (https://cmake.org/download/) and C++17 compiler"
        ),
    }


def check_python_and_packages() -> Dict[str, Any]:
    py_ver = f"{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}"
    py_ok = (sys.version_info.major, sys.version_info.minor) >= (3, 10)

    required_modules = {
        "pytest": "pytest",
        "numpy": "numpy",
        "scipy": "scipy",
        "Pillow": "PIL",
        "requests": "requests",
    }
    installed_pkgs: List[str] = []
    missing_pkgs: List[str] = []
    for pkg_label, mod_name in required_modules.items():
        if importlib.util.find_spec(mod_name) is not None:
            installed_pkgs.append(pkg_label)
        else:
            missing_pkgs.append(pkg_label)

    req_file = ROOT_DIR / "python-backend" / "requirements.txt"
    all_ok = py_ok and len(missing_pkgs) == 0 and req_file.exists()
    status = "OK" if all_ok else ("WARN" if py_ok else "MISSING")
    details = (
        f"Packages OK ({', '.join(installed_pkgs)})"
        if not missing_pkgs
        else f"Missing packages: {', '.join(missing_pkgs)} -> run `pip install -r python-backend/requirements.txt`"
    )

    return {
        "component": "Python 3.10+ & Pip Deps",
        "status": status,
        "ok": py_ok,
        "version": f"Python {py_ver}",
        "installed_packages": installed_pkgs,
        "missing_packages": missing_pkgs,
        "details": details,
    }


def check_cpp_redist_and_system_libs() -> Dict[str, Any]:
    sys_name = platform.system().lower()
    if sys_name == "windows":
        sys_dir = Path(os.environ.get("SystemRoot", r"C:\Windows")) / "System32"
        vc_dlls = ["vcruntime140.dll", "vcruntime140_1.dll", "msvcp140.dll", "msvcp140_1.dll"]
        dx_dlls = ["d3d11.dll", "dxgi.dll", "XAudio2_7.dll", "XAudio2_9.dll"]

        missing_vc = [dll for dll in vc_dlls if not (sys_dir / dll).exists()]
        present_dx = [dll for dll in dx_dlls if (sys_dir / dll).exists()]

        vc_ok = len(missing_vc) == 0
        dx_ok = "d3d11.dll" in present_dx and ("XAudio2_7.dll" in present_dx or "XAudio2_9.dll" in present_dx)
        all_ok = vc_ok and dx_ok

        return {
            "component": "C++ Redist & DirectX (Win)",
            "status": "OK" if all_ok else ("WARN" if vc_ok else "MISSING"),
            "ok": vc_ok,
            "version": "VC++ v143 + DirectX11/XAudio2" if all_ok else f"Missing: {', '.join(missing_vc) or 'XAudio2_7.dll'}",
            "details": (
                "vcruntime140.dll, msvcp140.dll, d3d11.dll & XAudio2 verified in System32"
                if all_ok
                else r"Run .\scripts\install_prerequisites.bat as Administrator"
            ),
        }

    if sys_name == "linux":
        libs = ["GL", "X11", "asound"]
        found = [lib for lib in libs if ctypes.util.find_library(lib)]
        ok = len(found) == len(libs)
        return {
            "component": "Linux System Libraries",
            "status": "OK" if ok else "WARN",
            "ok": ok,
            "version": f"Found {len(found)}/{len(libs)} ({', '.join(found)})",
            "details": (
                "OpenGL, X11, and ALSA shared libraries present"
                if ok
                else "Run: sudo apt install libgl1-mesa-dev libx11-dev libasound2-dev"
            ),
        }

    # macOS / Darwin
    ok, xcode_out = _run_cmd(["xcode-select", "-p"])
    return {
        "component": "macOS Xcode CLI Tools",
        "status": "OK" if ok else "WARN",
        "ok": ok,
        "version": xcode_out if ok else "Not found",
        "details": "Xcode Command Line Tools active" if ok else "Run: xcode-select --install",
    }


def check_docker() -> Dict[str, Any]:
    docker_bin = shutil.which("docker")
    if not docker_bin:
        return {
            "component": "Docker (24.0+ QA/CI)",
            "status": "WARN",
            "ok": False,
            "version": "Not installed",
            "details": "Optional for local dev; required for containerized QA (https://www.docker.com/products/docker-desktop)",
        }

    ok, ver_out = _run_cmd([docker_bin, "--version"])
    return {
        "component": "Docker (24.0+ QA/CI)",
        "status": "OK" if ok else "WARN",
        "ok": ok,
        "version": ver_out.splitlines()[0] if ver_out else "Detected",
        "details": "Ready for Dockerfile & docker-compose.yml",
    }


def check_neoveg_native_ffi() -> Dict[str, Any]:
    sys_name = platform.system().lower()
    if sys_name == "windows":
        rid, ext, prefix = "win-x64", ".dll", ""
    elif sys_name == "darwin":
        rid, ext, prefix = "osx-x64", ".dylib", "lib"
    else:
        rid, ext, prefix = "linux-x64", ".so", "lib"

    core_lib = ROOT_DIR / "runtimes" / rid / "native" / f"{prefix}neoveg_core{ext}"
    cpp_lib = ROOT_DIR / "runtimes" / rid / "native" / f"{prefix}neoveg_cpp_engine{ext}"
    both_exist = core_lib.exists() and cpp_lib.exists()

    return {
        "component": f"Native FFI ({rid})",
        "status": "OK" if both_exist else "WARN",
        "ok": both_exist,
        "version": f"{core_lib.name} + {cpp_lib.name}" if both_exist else "Not built yet",
        "details": (
            f"Compiled FFI binaries ready in runtimes/{rid}/native/"
            if both_exist
            else "Run `python scripts/build_portable.py` to compile native FFI libraries"
        ),
    }


def log_diagnostic_report(checks: List[Dict[str, Any]]) -> None:
    try:
        LOGS_DIR.mkdir(parents=True, exist_ok=True)
        ts = datetime.now().strftime("%Y-%m-%d %H:%M:%S.%f")[:-3]
        summary_parts = [f"{c['component']}={c['status']} ({c['version']})" for c in checks]
        missing = [c for c in checks if c["status"] == "MISSING"]
        level = "WARN" if missing else "INFO"
        line = f"[{ts}] [{level}] [Python/check_env] Pre-Flight Environment Diagnostic: {' | '.join(summary_parts)}\n"
        with ACTIVE_LOG_FILE.open("a", encoding="utf-8") as fh:
            fh.write(line)
    except OSError:
        pass


def run_environment_check(as_json: bool = False) -> Dict[str, Any]:
    checks = [
        check_dotnet(),
        check_rust_cargo(),
        check_cmake_and_cpp(),
        check_python_and_packages(),
        check_cpp_redist_and_system_libs(),
        check_docker(),
        check_neoveg_native_ffi(),
    ]
    log_diagnostic_report(checks)

    core_ok = all(c["status"] in ("OK", "WARN") for c in checks)
    report = {
        "ok": core_ok,
        "platform": f"{platform.system()} {platform.release()} ({platform.machine()})",
        "requirements_md_exists": (ROOT_DIR / "REQUIREMENTS.md").exists(),
        "python_requirements_txt_exists": (ROOT_DIR / "python-backend" / "requirements.txt").exists(),
        "checks": checks,
    }

    if as_json:
        print(json.dumps(report, indent=2, ensure_ascii=False))
        return report

    print("=" * 96)
    print(" NEOVEG - DEVELOPER ENVIRONMENT & PREREQUISITES DIAGNOSTIC SUMMARY")
    print(f" Host OS : {report['platform']}")
    print(f" Root Dir: {ROOT_DIR}")
    print("=" * 96)
    print(f"{'COMPONENT':<26} | {'STATUS':<8} | {'DETECTED VERSION':<32} | {'NOTES / ACTION'}")
    print("-" * 96)
    for item in checks:
        comp = item["component"][:26]
        status = f"[{item['status']}]"
        ver = str(item["version"])[:32]
        details = item["details"]
        print(f"{comp:<26} | {status:<8} | {ver:<32} | {details}")
    print("=" * 96)

    missing_items = [c for c in checks if c["status"] == "MISSING"]
    if missing_items:
        print(f"[!] {len(missing_items)} required prerequisite(s) missing. Please consult REQUIREMENTS.md.")
    else:
        print("[OK] Core Neoveg developer environment verified. Diagnostic logged to logs/neoveg.log.")

    return report


def main() -> None:
    parser = argparse.ArgumentParser(description="Neoveg Developer Environment Diagnostic Tool")
    parser.add_argument("--json", action="store_true", help="Output diagnostic summary as JSON")
    args = parser.parse_args()
    run_environment_check(as_json=args.json)


if __name__ == "__main__":
    main()
