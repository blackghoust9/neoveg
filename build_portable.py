#!/usr/bin/env python3
"""Neoveg packaging and quality driver (Python standard library only).

Neoveg is a pure Rust workspace, so this script only orchestrates `cargo` and packs the result.

    python build_portable.py                 # QA gates, release build and portable packages for this OS
    python build_portable.py build --ffmpeg  # same, with video decoding (needs FFMPEG_DIR / LIBCLANG_PATH)
    python build_portable.py build --targets all --skip-qa
    python build_portable.py qa              # fmt + clippy + tests + release build, writes the QA report
    python build_portable.py check           # verify the toolchain (and FFmpeg variables with --ffmpeg)

For every target it produces, under `dist/`:

  * `Neoveg-Portable-<rid>/`            the staged portable folder (executable, assets, licenses, launcher)
  * `Neoveg_v<version>_<rid>_Portable.zip` (Windows) or `.tar.gz` (Linux, macOS)
  * `neoveg_v<version>_<rid>[.exe]`      the standalone executable (on Windows built with a static C
                                         runtime, so it needs no Visual C++ redistributable)
  * `SHA256SUMS.txt` and `build_manifest.json`

Targets (`--targets`): `host` (default), `all`, or a comma list of `win-x64`, `linux-x64`, `osx-x64`,
`osx-arm64`. A target other than the host needs its Rust target installed (`rustup target add ...`)
and a working linker, because Neoveg links native windowing and audio libraries.

The QA gates follow AGENTS.md section 3: `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`. Each step is
tagged with its ISO/IEC 25010 pillar and written to `logs/neoveg.log`, `coverage/qa/qa_report.json`
and `docs/qa_report.js` (rendered by `docs/index.html`).
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import time
import zipfile
from pathlib import Path
from typing import Any, Dict, List, Optional

ROOT = Path(__file__).resolve().parent
DIST = ROOT / "dist"
# Cargo may write to another directory (the Docker images set CARGO_TARGET_DIR)
TARGET_DIR = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
LOG_FILE = ROOT / "logs" / "neoveg.log"
QA_JSON = ROOT / "coverage" / "qa" / "qa_report.json"
QA_JS = ROOT / "docs" / "qa_report.js"

for _stream in (sys.stdout, sys.stderr):
    # Windows consoles default to a legacy code page; keep non-ASCII log text from crashing
    _reconfigure = getattr(_stream, "reconfigure", None)
    if callable(_reconfigure):
        _reconfigure(encoding="utf-8", errors="replace")

# rid -> Rust target triple, archive format, executable name
TARGETS: Dict[str, Dict[str, str]] = {
    "win-x64": {"triple": "x86_64-pc-windows-msvc", "archive": "zip", "exe": "neoveg.exe"},
    "linux-x64": {"triple": "x86_64-unknown-linux-gnu", "archive": "tar.gz", "exe": "neoveg"},
    "osx-x64": {"triple": "x86_64-apple-darwin", "archive": "tar.gz", "exe": "neoveg"},
    "osx-arm64": {"triple": "aarch64-apple-darwin", "archive": "tar.gz", "exe": "neoveg"},
}

# Files copied into every package (source -> name inside the package)
PAYLOAD_FILES = {
    "LICENSE": "LICENSE",
    "AUTHORS.md": "AUTHORS.md",
    "RELEASE_NOTES.md": "RELEASE_NOTES.md",
    "README.md": "README.md",
    "third_party/README.md": "third_party/README.md",
    "third_party/filmcraft/LICENSE-MIT": "third_party/filmcraft/LICENSE-MIT",
    "third_party/filmcraft/README.md": "third_party/filmcraft/README.md",
}

PILLAR_LABELS = {
    "Performance": "Eficiencia de Desempenho",
    "Reliability": "Confiabilidade",
    "Usability": "Usabilidade",
    "Portability": "Portabilidade",
    "Maintainability": "Manutenibilidade",
}
STANDARDS = [
    "ABNT NBR ISO/IEC 25010",
    "ABNT NBR ISO/IEC/IEEE 29119",
    "ABNT NBR ISO/IEC 12207",
    "ABNT NBR ISO/IEC 14764",
    "ABNT NBR ISO/IEC 27001",
]


# ----------------------------------------------------------------------------- helpers


def log(level: str, message: str, source: str = "build_portable") -> None:
    """Prints a line and appends it to logs/neoveg.log."""
    stamp = dt.datetime.now().strftime("%Y-%m-%d %H:%M:%S.%f")[:-3]
    line = f"[{stamp}] [{level}] [{source}] {message}"
    print(f"[{level}] {message}")
    try:
        LOG_FILE.parent.mkdir(parents=True, exist_ok=True)
        with LOG_FILE.open("a", encoding="utf-8") as handle:
            handle.write(line + "\n")
    except OSError:
        pass


def run(cmd: List[str], env: Optional[Dict[str, str]] = None, check: bool = True) -> int:
    """Runs a command in the repository root, streaming its output."""
    print("  $ " + " ".join(cmd))
    merged = {**os.environ, **(env or {})}
    result = subprocess.run(cmd, cwd=ROOT, env=merged)
    if check and result.returncode != 0:
        raise RuntimeError(f"command failed with exit code {result.returncode}: {' '.join(cmd)}")
    return result.returncode


def capture(cmd: List[str]) -> Optional[str]:
    try:
        out = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, check=True)
        return out.stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return None


def workspace_version() -> str:
    text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    match = re.search(r"\[workspace\.package\][^\[]*?version\s*=\s*\"([^\"]+)\"", text, re.S)
    if not match:
        raise RuntimeError("version not found in [workspace.package] of Cargo.toml")
    return match.group(1)


def host_rid() -> str:
    system = platform.system()
    machine = platform.machine().lower()
    if system == "Windows":
        return "win-x64"
    if system == "Linux":
        return "linux-x64"
    if system == "Darwin":
        return "osx-arm64" if machine in ("arm64", "aarch64") else "osx-x64"
    raise RuntimeError(f"unsupported host system: {system}")


def host_triple() -> Optional[str]:
    text = capture(["rustc", "-vV"])
    if not text:
        return None
    match = re.search(r"^host:\s*(\S+)", text, re.M)
    return match.group(1) if match else None


def installed_targets() -> List[str]:
    """Rust targets with a standard library installed (works with or without rustup)."""
    text = capture(["rustup", "target", "list", "--installed"])
    if text:
        return text.split()
    # Toolchains installed by an MSI / distro package have no rustup: read the sysroot instead
    sysroot = capture(["rustc", "--print", "sysroot"])
    if not sysroot:
        return []
    rustlib = Path(sysroot) / "lib" / "rustlib"
    if not rustlib.is_dir():
        return []
    return sorted(d.name for d in rustlib.iterdir() if (d / "lib").is_dir())


def target_hint(triple: str) -> str:
    if capture(["rustup", "--version"]):
        return f"rustup target add {triple}"
    return f"install the standard library for {triple} (this toolchain has no rustup)"


def resolve_targets(spec: str) -> List[str]:
    if spec == "host":
        return [host_rid()]
    if spec == "all":
        return list(TARGETS)
    rids = [item.strip() for item in spec.split(",") if item.strip()]
    unknown = [rid for rid in rids if rid not in TARGETS]
    if unknown:
        raise SystemExit(f"unknown target(s): {', '.join(unknown)}; choose from {', '.join(TARGETS)}")
    return rids


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


# ----------------------------------------------------------------------------- check


def ffmpeg_environment() -> List[str]:
    """Problems with the FFmpeg build variables (empty when everything is in place)."""
    problems = []
    ffmpeg_dir = os.environ.get("FFMPEG_DIR")
    libclang = os.environ.get("LIBCLANG_PATH")
    if not ffmpeg_dir or not Path(ffmpeg_dir).is_dir():
        problems.append("FFMPEG_DIR must point to the FFmpeg 7.1.x shared build (bin, include, lib)")
    elif not (Path(ffmpeg_dir) / "include").is_dir():
        problems.append(f"FFMPEG_DIR has no include folder: {ffmpeg_dir}")
    if not libclang or not Path(libclang).is_dir():
        problems.append("LIBCLANG_PATH must point to the LLVM 19.x bin folder (LLVM 22+ breaks ffmpeg-sys-next)")
    return problems


def cmd_check(args: argparse.Namespace) -> int:
    ok = True
    for tool in ("cargo", "rustc"):
        version = capture([tool, "--version"])
        print(f"{'OK  ' if version else 'FAIL'} {tool}: {version or 'not found'}")
        ok = ok and bool(version)
    # rustup is optional (MSI and distro toolchains have none); the components are probed directly
    rustup = capture(["rustup", "--version"])
    print(f"INFO rustup: {rustup or 'not installed (optional)'}")
    for component, cmd in (("clippy", ["cargo", "clippy", "--version"]), ("rustfmt", ["cargo", "fmt", "--version"])):
        version = capture(cmd)
        print(f"{'OK  ' if version else 'FAIL'} component {component}: {version or 'not found'}")
        ok = ok and bool(version)
    print(f"INFO host target: {host_triple()} ({host_rid()})")
    if args.ffmpeg:
        problems = ffmpeg_environment()
        for problem in problems:
            print(f"FAIL {problem}")
        print("OK   FFmpeg variables" if not problems else "")
        ok = ok and not problems
    return 0 if ok else 1


# ----------------------------------------------------------------------------- QA


def release_env(triple: str) -> Dict[str, str]:
    """Environment of the release build; QA and packaging share it so the second build is cached."""
    if "windows-msvc" not in triple:
        return {}
    # Static C runtime: the portable exe does not need the Visual C++ redistributable
    flags = os.environ.get("RUSTFLAGS", "")
    return {"RUSTFLAGS": (flags + " -C target-feature=+crt-static").strip()}


def qa_steps(args: argparse.Namespace) -> List[Dict[str, object]]:
    maintain = "ABNT NBR ISO/IEC 25010 (Manutenibilidade) / ISO/IEC 14764"
    testing = "ABNT NBR ISO/IEC/IEEE 29119 (Testes) / ISO/IEC 25010 (Confiabilidade)"
    portable = "ABNT NBR ISO/IEC 25010 (Portabilidade) - build nativo pela toolchain unica"
    test_cmd = ["cargo", "test", "--workspace"]
    steps: List[Dict[str, object]] = [
        {"name": "rustfmt", "pillar": "Maintainability", "standard": maintain,
         "cmd": ["cargo", "fmt", "--all", "--", "--check"]},
        {"name": "Clippy (-D warnings)", "pillar": "Maintainability", "standard": maintain,
         "cmd": ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"]},
        {"name": "Testes do workspace", "pillar": "Reliability", "standard": testing, "cmd": test_cmd},
    ]
    if getattr(args, "ffmpeg", False):
        steps.append({"name": "Testes com FFmpeg", "pillar": "Reliability", "standard": testing,
                      "cmd": test_cmd + ["--features", "neoveg_ui/ffmpeg"]})
    # Same command and RUSTFLAGS as the package build, so packaging reuses this compilation
    release_cmd = ["cargo", "build", "--release", "-p", "neoveg_ui"]
    if getattr(args, "ffmpeg", False):
        release_cmd += ["--features", "ffmpeg"]
    steps.append({"name": "Build release (neoveg_ui)", "pillar": "Portability", "standard": portable,
                  "cmd": release_cmd, "env": release_env(host_triple() or "")})
    return steps


def write_qa_report(steps: List[Dict[str, object]], outcome: str) -> None:
    pillars: Dict[str, Dict[str, Any]] = {}
    for step in steps:
        entry = pillars.setdefault(
            str(step["pillar"]),
            {"label": PILLAR_LABELS[str(step["pillar"])], "status": "pass", "passed": 0, "total": 0},
        )
        entry["total"] += 1
        if step["status"] == "pass":
            entry["passed"] += 1
        else:
            entry["status"] = "fail"
    report = {
        "generated_at": dt.datetime.now().astimezone().isoformat(),
        "platform": platform.system().lower(),
        "outcome": outcome,
        "standards": STANDARDS,
        "pillars": pillars,
        "steps": [{k: v for k, v in s.items() if k not in ("cmd", "env")} for s in steps],
        "environment": {"rustc": capture(["rustc", "--version"]), "cargo": capture(["cargo", "--version"])},
    }
    QA_JSON.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps(report, indent=2, ensure_ascii=False)
    QA_JSON.write_text(text + "\n", encoding="utf-8")
    QA_JS.parent.mkdir(parents=True, exist_ok=True)
    QA_JS.write_text("window.NEOVEG_QA_REPORT = " + text + ";\n", encoding="utf-8")


def cmd_qa(args: argparse.Namespace) -> int:
    steps = qa_steps(args)
    failed = False
    for step in steps:
        source = f"QA/{step['pillar']}"
        log("INFO", f"START {step['name']} - {step['standard']}", source)
        started = time.monotonic()
        code = run(list(step["cmd"]), step.get("env"), check=False)  # type: ignore[arg-type]
        step["duration_ms"] = int((time.monotonic() - started) * 1000)
        step["status"] = "pass" if code == 0 else "fail"
        log("INFO" if code == 0 else "ERROR", f"{'PASS' if code == 0 else 'FAIL'} {step['name']}", source)
        if code != 0:
            failed = True
            break  # later steps would only repeat the failure
    write_qa_report(steps, "fail" if failed else "pass")
    print(f"QA report: {QA_JSON.relative_to(ROOT)}")
    return 1 if failed else 0


# ----------------------------------------------------------------------------- build


def cargo_build(rid: str, ffmpeg: bool) -> Path:
    """Builds the release binary for `rid` and returns its path."""
    info = TARGETS[rid]
    triple = info["triple"]
    is_host = triple == host_triple()
    if not is_host and triple not in installed_targets():
        raise RuntimeError(f"Rust target {triple} is not installed: {target_hint(triple)}")
    cmd = ["cargo", "build", "--release", "-p", "neoveg_ui"]
    env = release_env(triple)
    if ffmpeg:
        cmd += ["--features", "ffmpeg"]
    if not is_host:
        cmd += ["--target", triple]
    log("INFO", f"building {rid} ({triple}){' with FFmpeg' if ffmpeg else ''}")
    run(cmd, env)
    base = TARGET_DIR / ("release" if is_host else f"{triple}/release")
    binary = base / info["exe"]
    if not binary.is_file():
        raise RuntimeError(f"expected binary not found: {binary}")
    return binary


def copy_payload(root: Path) -> None:
    for source, name in PAYLOAD_FILES.items():
        src = ROOT / source
        if src.is_file():
            dst = root / name
            dst.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(src, dst)
    # The folder is `assets/` in Git but `Assets/` in AGENTS.md: accept both (Linux is case-sensitive)
    i18n = next((d for d in (ROOT / "assets" / "i18n", ROOT / "Assets" / "i18n") if d.is_dir()), None)
    if i18n is not None:
        # Only the runtime resources (JSON); the .axaml files belong to the legacy C# UI
        target = root / "Assets" / "i18n"
        target.mkdir(parents=True, exist_ok=True)
        for item in i18n.glob("*.json"):
            shutil.copy2(item, target / item.name)


def copy_ffmpeg_runtime(rid: str, destination: Path) -> None:
    """Copies the FFmpeg shared libraries next to the executable (direct launches must not rely on PATH)."""
    ffmpeg_dir = os.environ.get("FFMPEG_DIR")
    if not ffmpeg_dir:
        return
    base = Path(ffmpeg_dir)
    if rid == "win-x64":
        patterns = [(base / "bin", "*.dll")]
    elif rid == "linux-x64":
        patterns = [(base / "lib", "libav*.so*"), (base / "lib", "libsw*.so*")]
    else:
        log("WARN", f"FFmpeg libraries are not bundled for {rid}; install FFmpeg 7.1 with Homebrew")
        return
    destination.mkdir(parents=True, exist_ok=True)
    count = 0
    for folder, pattern in patterns:
        for lib in folder.glob(pattern):
            if lib.is_file():
                shutil.copy2(lib, destination / lib.name)
                count += 1
    log("INFO", f"{count} FFmpeg libraries copied for {rid}")


LINUX_LAUNCHER = """#!/bin/sh
# Neoveg portable launcher: finds the bundled FFmpeg libraries without touching the system.
HERE="$(cd "$(dirname "$0")" && pwd)"
export LD_LIBRARY_PATH="$HERE/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$HERE/neoveg" "$@"
"""

MAC_PLIST = """<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Neoveg</string>
  <key>CFBundleDisplayName</key><string>Neoveg</string>
  <key>CFBundleIdentifier</key><string>org.neoveg.app</string>
  <key>CFBundleExecutable</key><string>neoveg</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>{version}</string>
  <key>CFBundleVersion</key><string>{version}</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
"""


def stage_package(rid: str, binary: Path, version: str, ffmpeg: bool) -> Path:
    """Builds dist/Neoveg-Portable-<rid>/ from the compiled binary."""
    stage = DIST / f"Neoveg-Portable-{rid}"
    if stage.exists():
        shutil.rmtree(stage)
    stage.mkdir(parents=True)
    exe = TARGETS[rid]["exe"]
    if rid.startswith("osx"):
        macos = stage / "Neoveg.app" / "Contents" / "MacOS"
        macos.mkdir(parents=True)
        shutil.copy2(binary, macos / exe)
        (stage / "Neoveg.app" / "Contents" / "Info.plist").write_text(
            MAC_PLIST.format(version=version), encoding="utf-8"
        )
        copy_payload(stage / "Neoveg.app" / "Contents" / "Resources")
        copy_payload(stage)
        if ffmpeg:
            copy_ffmpeg_runtime(rid, macos)
        return stage
    shutil.copy2(binary, stage / exe)
    copy_payload(stage)
    if rid == "linux-x64":
        launcher = stage / "neoveg.sh"
        launcher.write_text(LINUX_LAUNCHER, encoding="utf-8", newline="\n")
        if ffmpeg:
            copy_ffmpeg_runtime(rid, stage / "lib")
    elif ffmpeg:
        copy_ffmpeg_runtime(rid, stage)
    return stage


def make_archive(stage: Path, rid: str, version: str) -> Path:
    kind = TARGETS[rid]["archive"]
    name = f"Neoveg_v{version}_{rid}_Portable.{kind}"
    archive = DIST / name
    if archive.exists():
        archive.unlink()
    executables = {TARGETS[rid]["exe"], "neoveg.sh"}
    if kind == "zip":
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as bundle:
            for path in sorted(stage.rglob("*")):
                if path.is_file():
                    bundle.write(path, Path(stage.name) / path.relative_to(stage))
    else:
        def keep_exec_bit(info: tarfile.TarInfo) -> tarfile.TarInfo:
            if Path(info.name).name in executables:
                info.mode = 0o755
            return info

        with tarfile.open(archive, "w:gz") as bundle:
            bundle.add(stage, arcname=stage.name, filter=keep_exec_bit)
    return archive


# Folders of the C# / C++ / Python era that old packaging runs left inside dist/Neoveg-Portable-*
LEGACY_PACKAGE_MARKERS = ("desktop-host", "cpp-extensions", "runtimes", "Neoveg.AppDir")


def prune_legacy_dist() -> None:
    """Removes portable folders of the previous architecture; a Rust package has none of these."""
    for folder in sorted(DIST.glob("Neoveg-Portable-*")):
        if folder.is_dir() and any((folder / m).exists() for m in LEGACY_PACKAGE_MARKERS):
            shutil.rmtree(folder, ignore_errors=True)
            log("INFO", f"removed legacy package folder {folder.name}")


def cmd_build(args: argparse.Namespace) -> int:
    global DIST
    if args.dist:
        DIST = Path(args.dist).resolve()
    version = workspace_version()
    rids = resolve_targets(args.targets)
    if args.ffmpeg:
        problems = ffmpeg_environment()
        if problems:
            for problem in problems:
                log("ERROR", problem)
            return 1
    if not args.skip_qa:
        log("INFO", "running the quality gates before packaging (use --skip-qa to bypass)")
        if cmd_qa(args) != 0:
            log("ERROR", "quality gates failed; no package was produced")
            return 1
    DIST.mkdir(parents=True, exist_ok=True)
    prune_legacy_dist()
    manifest: Dict[str, object] = {
        "version": version,
        "generated_at": dt.datetime.now().astimezone().isoformat(),
        "ffmpeg": bool(args.ffmpeg),
        "packages": [],
    }
    failures: List[str] = []
    for rid in rids:
        try:
            binary = cargo_build(rid, args.ffmpeg)
            if args.ffmpeg:
                log("WARN", f"{rid}: the standalone executable needs the FFmpeg shared libraries "
                            "on PATH; the portable package already bundles them")
            exe_name = f"neoveg_v{version}_{rid}" + (".exe" if rid == "win-x64" else "")
            standalone = DIST / exe_name
            shutil.copy2(binary, standalone)
            stage = stage_package(rid, binary, version, args.ffmpeg)
            files = [standalone]
            if not args.no_archive:
                files.append(make_archive(stage, rid, version))
            for path in files:
                manifest["packages"].append(  # type: ignore[union-attr]
                    {"target": rid, "file": path.name, "bytes": path.stat().st_size, "sha256": sha256(path)}
                )
                log("INFO", f"{path.name}: {path.stat().st_size / 1_048_576:.1f} MiB")
        except (RuntimeError, OSError) as error:
            log("ERROR", f"{rid}: {error}")
            failures.append(rid)
    (DIST / "build_manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    lines = [f"{p['sha256']}  {p['file']}" for p in manifest["packages"]]  # type: ignore[union-attr]
    (DIST / "SHA256SUMS.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")
    if args.sync:
        sync = ROOT / "scripts" / "sync_gdrive.ps1"
        if sync.is_file() and platform.system() == "Windows":
            run(["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", str(sync)], check=False)
        else:
            log("WARN", "--sync needs Windows and scripts/sync_gdrive.ps1")
    if failures:
        log("ERROR", f"failed targets: {', '.join(failures)}")
        return 1
    log("INFO", f"packages written to {DIST}")
    return 0


# ----------------------------------------------------------------------------- main


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(description="Neoveg packaging and QA driver")
    sub = parser.add_subparsers(dest="command")

    def common(p: argparse.ArgumentParser) -> None:
        p.add_argument("--ffmpeg", action="store_true", help="build with the ffmpeg feature (video decoding)")

    build = sub.add_parser("build", help="QA gates, release build, portable folders, archives and the exe")
    common(build)
    build.add_argument("--targets", default="host", help="host (default), all, or win-x64,linux-x64,osx-x64,osx-arm64")
    build.add_argument("--skip-qa", action="store_true", help="skip fmt/clippy/tests (not for releases)")
    build.add_argument("--no-archive", action="store_true", help="only stage the folders and the exe")
    build.add_argument("--dist", default="", help="output folder (default: dist/ in the repository)")
    build.add_argument("--sync", action="store_true", help="mirror the workspace to the cloud folder afterwards (Windows)")
    qa = sub.add_parser("qa", help="run the quality gates and write the QA report")
    common(qa)
    check = sub.add_parser("check", help="verify the toolchain")
    common(check)

    argv = sys.argv[1:] if argv is None else argv
    if not argv or argv[0] not in ("build", "qa", "check", "-h", "--help"):
        argv = ["build"] + argv  # `python build_portable.py --ffmpeg` means build
    args = parser.parse_args(argv)
    handlers = {"build": cmd_build, "qa": cmd_qa, "check": cmd_check}
    return handlers[args.command](args)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except RuntimeError as error:
        log("ERROR", str(error))
        sys.exit(1)
