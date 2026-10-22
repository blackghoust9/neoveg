"""
OpenVegas Pro 21 — Studio Launcher
Launches the native desktop executable `OpenVegasPro21.exe` (or starts the headless backend server if requested).
"""

from __future__ import annotations
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
FRONTEND_BUILD = ROOT / "frontend" / "build.mjs"
RUST_DIR = ROOT / "rust-core"
BACKEND_DIR = ROOT / "python-backend"
NATIVE_EXE = ROOT / "OpenVegasPro21.exe"


def main() -> None:
    print("=" * 68)
    print("  OPEN VEGAS PRO 21 — NATIVE DESKTOP STUDIO (Rust • Python • TS)")
    print("=" * 68)

    # 1. Compile TypeScript Frontend
    node_bin = shutil.which("node")
    if node_bin and FRONTEND_BUILD.exists():
        print("[1/3] Compiling TypeScript Frontend (frontend/src/*.ts -> bundle.js)...")
        subprocess.run([node_bin, str(FRONTEND_BUILD)], check=False)

    # 2. Compile Rust Core if Cargo is available
    cargo_bin = shutil.which("cargo")
    if cargo_bin and (RUST_DIR / "Cargo.toml").exists():
        print("[2/3] Building Rust Core Engine (openvegas-core)...")
        subprocess.run([cargo_bin, "build", "--release"], cwd=str(RUST_DIR), check=False)
    else:
        print("[2/3] Rust Core crate ready at ./rust-core.")

    # 3. Launch Native Desktop Executable (.exe) unless headless mode is requested
    if os.environ.get("OPENVEGAS_HEADLESS") != "1" and NATIVE_EXE.exists():
        print(f"[3/3] Launching Native Windows Executable: {NATIVE_EXE}")
        subprocess.Popen([str(NATIVE_EXE)], cwd=str(ROOT))
        return

    port = int(os.environ.get("OPENVEGAS_PORT", "8765"))
    sys.path.insert(0, str(BACKEND_DIR))
    from server import run_server  # type: ignore

    run_server(host="127.0.0.1", port=port)


if __name__ == "__main__":
    main()
