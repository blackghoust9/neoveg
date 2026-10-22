#!/usr/bin/env python3
"""
Neoveg — Third-Party GitHub Dependency Installer, Updater & Native Binary Stager (`scripts/update_third_party.py`)
Automates:
  1. Downloading / shallow-cloning or verifying third-party audio/video SDKs in `third_party/`:
     - OpenFX (`AcademySoftwareFoundation/openfx`) — BSD-3-Clause / Apache-2.0
     - VST3 SDK (`steinbergmedia/vst3sdk`) — GPL-3.0-or-later
     - Voukoder (`Vouk/voukoder`) — LGPL-2.1-or-later / GPL-2.0
     - Whisper.cpp (`ggerganov/whisper.cpp`) — MIT
     - CorridorKey (`CorridorDigital/CorridorKey`) — MIT
  2. Compiling C++ (`neoveg_cpp_engine`, `neoveg-whisper-cli`) and Rust (`neoveg_core`) native binaries.
  3. Staging compiled binaries and `third_party/` metadata into `dist/` portable packages.
"""

from __future__ import annotations
import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any, Dict, List

ROOT_DIR = Path(__file__).resolve().parent.parent
THIRD_PARTY_DIR = ROOT_DIR / "third_party"
CPP_EXT_DIR = ROOT_DIR / "cpp-extensions"
DIST_DIR = ROOT_DIR / "dist"
PY_BACKEND_DIR = ROOT_DIR / "python-backend"

if str(PY_BACKEND_DIR) not in sys.path:
    sys.path.insert(0, str(PY_BACKEND_DIR))

THIRD_PARTY_REPOS: List[Dict[str, Any]] = [
    {
        "id": "openfx",
        "name": "OpenFX Core SDK (ofxCore)",
        "repo_url": "https://github.com/AcademySoftwareFoundation/openfx.git",
        "branch": "main",
        "license": "BSD-3-Clause / Apache-2.0",
        "local_dir": THIRD_PARTY_DIR / "openfx",
        "required_files": [
            "include/ofxCore.h",
            "include/ofxImageEffect.h",
        ],
    },
    {
        "id": "vst3sdk",
        "name": "Steinberg VST3 Audio Processor SDK",
        "repo_url": "https://github.com/steinbergmedia/vst3sdk.git",
        "branch": "master",
        "license": "GPL-3.0-or-later",
        "local_dir": THIRD_PARTY_DIR / "vst3sdk",
        "required_files": [
            "pluginterfaces/vst/ivstaudioprocessor.h",
        ],
    },
    {
        "id": "voukoder",
        "name": "Voukoder Pro Hardware Encoder Connector",
        "repo_url": "https://github.com/Vouk/voukoder.git",
        "branch": "master",
        "license": "LGPL-2.1-or-later / GPL-2.0",
        "local_dir": THIRD_PARTY_DIR / "voukoder",
        "required_files": [
            "include/voukoder_bridge.h",
        ],
    },
    {
        "id": "whisper.cpp",
        "name": "Whisper.cpp GGML AI Speech-to-Text",
        "repo_url": "https://github.com/ggerganov/whisper.cpp.git",
        "branch": "master",
        "license": "MIT",
        "local_dir": THIRD_PARTY_DIR / "whisper.cpp",
        "required_files": [
            "include/whisper.h",
        ],
    },
    {
        "id": "corridorkey",
        "name": "CorridorKey Green Screen Matte & Despill Engine",
        "repo_url": "https://github.com/CorridorDigital/CorridorKey.git",
        "branch": "main",
        "license": "MIT",
        "local_dir": THIRD_PARTY_DIR / "corridorkey",
        "required_files": [
            "corridorkey_manifest.json",
        ],
    },
]


def sync_third_party_repositories(attempt_git_clone: bool = False) -> List[Dict[str, Any]]:
    """
    Verifies or clones each third-party GitHub dependency into `third_party/`.
    When `attempt_git_clone` is True and `git` is available, performs shallow `git clone --depth 1`
    into a `_upstream` subfolder if not already cloned, while preserving the vendored C/C++ headers.
    """
    THIRD_PARTY_DIR.mkdir(parents=True, exist_ok=True)
    git_bin = shutil.which("git")
    statuses: List[Dict[str, Any]] = []

    for mod in THIRD_PARTY_REPOS:
        mod_dir: Path = mod["local_dir"]
        mod_dir.mkdir(parents=True, exist_ok=True)

        git_synced = False
        if attempt_git_clone and git_bin:
            upstream_dir = mod_dir / "_upstream"
            try:
                if (upstream_dir / ".git").exists():
                    subprocess.run(
                        [git_bin, "-C", str(upstream_dir), "pull", "--ff-only"],
                        capture_output=True,
                        text=True,
                        timeout=25,
                        check=False,
                    )
                    git_synced = True
                else:
                    res = subprocess.run(
                        [git_bin, "clone", "--depth", "1", mod["repo_url"], str(upstream_dir)],
                        capture_output=True,
                        text=True,
                        timeout=30,
                        check=False,
                    )
                    git_synced = res.returncode == 0
            except Exception:
                git_synced = False

        verified_files: List[str] = []
        missing_files: List[str] = []
        for rel_f in mod["required_files"]:
            fp = mod_dir / rel_f
            if fp.exists():
                verified_files.append(rel_f)
            else:
                missing_files.append(rel_f)

        statuses.append({
            "id": mod["id"],
            "name": mod["name"],
            "repository": mod["repo_url"],
            "license": mod["license"],
            "localDir": str(mod_dir.relative_to(ROOT_DIR)).replace("\\", "/"),
            "gitSynced": git_synced,
            "verifiedFiles": verified_files,
            "missingFiles": missing_files,
            "ready": len(missing_files) == 0,
        })

    return statuses


def compile_native_third_party_bridges() -> Dict[str, Any]:
    """
    Compiles the Rust FFI (`neoveg_core`) and C++17 (`neoveg_cpp_engine` + `whisper.cpp` bridge)
    shared libraries via `native_ffi_builder.py` and CMake.
    """
    import native_ffi_builder

    rust_res = native_ffi_builder.compile_or_synthesize_rust_core()
    cpp_res = native_ffi_builder.compile_or_synthesize_cpp_extensions()

    return {
        "rust_core": rust_res,
        "cpp_engine": cpp_res,
    }


def stage_into_dist_packages(lock_data: Dict[str, Any]) -> List[str]:
    """
    Copies compiled C++/Rust native binaries (`neoveg_cpp_engine`, `neoveg_core`),
    `third_party/` headers & `README.md`, and `cpp-extensions/whisper.cpp/` into all
    portable packages inside `dist/`.
    """
    DIST_DIR.mkdir(parents=True, exist_ok=True)
    lock_file = THIRD_PARTY_DIR / "third_party_lock.json"
    lock_file.write_text(json.dumps(lock_data, indent=2, ensure_ascii=False), encoding="utf-8")

    targets = [p for p in DIST_DIR.iterdir() if p.is_dir() and p.name.startswith("Neoveg-Portable-")]
    if not targets:
        default_win = DIST_DIR / "Neoveg-Portable-win-x64"
        default_win.mkdir(parents=True, exist_ok=True)
        targets = [default_win]

    staged_targets: List[str] = []
    for target_root in targets:
        # 1. Stage `third_party/` directory (README.md, headers, lockfile)
        dst_tp = target_root / "third_party"
        if dst_tp.exists():
            shutil.rmtree(dst_tp, ignore_errors=True)
        shutil.copytree(
            THIRD_PARTY_DIR,
            dst_tp,
            ignore=shutil.ignore_patterns(".git", "_upstream"),
            dirs_exist_ok=True,
        )

        # 2. Stage `cpp-extensions/` (including `whisper.cpp/` and compiled binaries)
        dst_cpp = target_root / "cpp-extensions"
        if CPP_EXT_DIR.exists():
            if dst_cpp.exists():
                shutil.rmtree(dst_cpp, ignore_errors=True)
            shutil.copytree(
                CPP_EXT_DIR,
                dst_cpp,
                ignore=shutil.ignore_patterns("CMakeFiles", "*.obj", "*.o", "*.exp", "*.lib"),
                dirs_exist_ok=True,
            )

        # 3. Stage `runtimes/` native binaries into `target_root/runtimes/` and root if Windows
        runtimes_src = ROOT_DIR / "runtimes"
        if runtimes_src.exists():
            dst_rt = target_root / "runtimes"
            dst_rt.mkdir(parents=True, exist_ok=True)
            for rid_dir in runtimes_src.iterdir():
                if rid_dir.is_dir():
                    dst_rid = dst_rt / rid_dir.name / "native"
                    dst_rid.mkdir(parents=True, exist_ok=True)
                    src_nat = rid_dir / "native"
                    if src_nat.exists():
                        for f in src_nat.iterdir():
                            if f.is_file():
                                shutil.copy2(f, dst_rid / f.name)

        win_native = ROOT_DIR / "runtimes" / "win-x64" / "native"
        if "win" in target_root.name.lower() and win_native.exists():
            for dll_name in ("neoveg_core.dll", "neoveg_cpp_engine.dll", "neoveg_cpp_engine.dll"):
                src_dll = win_native / dll_name
                if src_dll.exists():
                    shutil.copy2(src_dll, target_root / dll_name)

        staged_targets.append(str(target_root.relative_to(ROOT_DIR)).replace("\\", "/"))

    return staged_targets


def run_update_pipeline(
    attempt_git_clone: bool = False,
    build_binaries: bool = True,
    stage_dist: bool = True,
) -> Dict[str, Any]:
    modules = sync_third_party_repositories(attempt_git_clone=attempt_git_clone)
    build_info: Dict[str, Any] = {}
    if build_binaries:
        build_info = compile_native_third_party_bridges()

    payload: Dict[str, Any] = {
        "ok": all(m["ready"] for m in modules),
        "project": "Neoveg",
        "thirdPartyRoot": "third_party/",
        "modules": modules,
        "nativeBuild": build_info,
        "stagedDistPackages": [],
    }

    if stage_dist:
        payload["stagedDistPackages"] = stage_into_dist_packages(payload)

    return payload


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Neoveg — Third-Party GitHub Dependency Installer & Native Binary Builder"
    )
    parser.add_argument(
        "--sync-git",
        action="store_true",
        help="Attempt shallow git clone/pull from upstream GitHub repositories.",
    )
    parser.add_argument(
        "--offline",
        action="store_true",
        help="Use vendored headers/manifests in third_party/ without network requests.",
    )
    parser.add_argument(
        "--build",
        action="store_true",
        default=True,
        help="Compile native C++ (neoveg_cpp_engine + whisper.cpp) and Rust (neoveg_core) binaries.",
    )
    parser.add_argument(
        "--stage-dist",
        action="store_true",
        default=True,
        help="Stage compiled binaries and third_party/ into dist/ portable packages.",
    )
    args = parser.parse_args()

    res = run_update_pipeline(
        attempt_git_clone=(args.sync_git and not args.offline),
        build_binaries=args.build,
        stage_dist=args.stage_dist,
    )
    print(json.dumps(res, indent=2, ensure_ascii=False))
    return 0 if res.get("ok") else 1


if __name__ == "__main__":
    raise SystemExit(main())
