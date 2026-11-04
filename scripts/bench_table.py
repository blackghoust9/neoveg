#!/usr/bin/env python3
"""Turns the Criterion results of `playback_bench` into the markdown table of README.md.

Run the benchmark first:

    cargo bench -p neoveg_video --bench playback_bench

then:

    python scripts/bench_table.py            # prints the table and the machine line
    python scripts/bench_table.py --readme   # also rewrites the block between the markers in README.md

The block of README.md is the text between `<!-- bench:begin -->` and `<!-- bench:end -->`. Times are
the median of the Criterion samples, in milliseconds per frame. The frames-per-second figures are
1000 / the time of the stages of that kind of frame: a ceiling of what the CPU path alone could do
(no decoding, no GPU).
Python standard library only.
"""
from __future__ import annotations

import argparse
import json
import os
import platform
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BEGIN = "<!-- bench:begin -->"
END = "<!-- bench:end -->"
GROUPS = ("generate", "grade", "scopes", "crossfade")
RESOLUTIONS = (
    ("480p", 854, 480),
    ("720p", 1280, 720),
    ("1080p", 1920, 1080),
    ("1440p", 2560, 1440),
    ("4K", 3840, 2160),
    ("8K", 7680, 4320),
)


def criterion_dir() -> Path:
    base = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    return base / "criterion"


def median_ms(group: str, name: str) -> float | None:
    path = criterion_dir() / group / name / "new" / "estimates.json"
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    return data["median"]["point_estimate"] / 1e6  # ns -> ms


def cpu_name() -> str:
    try:
        if os.name == "nt":
            out = subprocess.run(
                ["powershell", "-NoProfile", "-Command", "(Get-CimInstance Win32_Processor | Select-Object -First 1).Name"],
                capture_output=True,
                text=True,
                timeout=20,
            ).stdout.strip()
            if out:
                return out
        elif Path("/proc/cpuinfo").exists():
            for line in Path("/proc/cpuinfo").read_text().splitlines():
                if line.startswith("model name"):
                    return line.split(":", 1)[1].strip()
    except (OSError, subprocess.SubprocessError):
        pass
    return platform.processor() or platform.machine()


def machine_line() -> str:
    threads = os.cpu_count() or 0
    return f"{cpu_name()} ({threads} threads), {platform.system()} {platform.release()}, Rust {rustc_version()}"


def rustc_version() -> str:
    try:
        return subprocess.run(["rustc", "--version"], capture_output=True, text=True, timeout=20).stdout.split()[1]
    except (OSError, subprocess.SubprocessError, IndexError):
        return "?"


def table() -> str | None:
    rows = []
    for name, width, height in RESOLUTIONS:
        generate, grade, scopes, crossfade = (median_ms(g, name) for g in GROUPS)
        if None in (generate, grade, scopes, crossfade):
            return None
        plain = grade + scopes  # a frame that is just shown, with colour correction and scopes
        fade = plain + crossfade  # a frame inside a transition
        rows.append(
            f"| {name} | {width}x{height} | {fmt(generate)} | {fmt(grade)} | {fmt(scopes)} | {fmt(crossfade)} "
            f"| {fmt(plain)} ({1000.0 / plain:.0f}) | {fmt(fade)} ({1000.0 / fade:.1f}) |"
        )
    head = (
        "| Resolução | Tamanho | Gerar quadro | Correção de cor | Scopes | Crossfade (CPU) "
        "| Quadro normal: cor + scopes (quadros/s) | Quadro em transição: + crossfade (quadros/s) |\n"
        "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |"
    )
    return head + "\n" + "\n".join(rows)


def fmt(ms: float) -> str:
    return f"{ms:.2f} ms" if ms < 100 else f"{ms:.0f} ms"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--readme", action="store_true", help="rewrite the block between the bench markers in README.md")
    args = ap.parse_args()

    built = table()
    if built is None:
        print("Sem resultados em target/criterion: rode antes `cargo bench -p neoveg_video --bench playback_bench`.", file=sys.stderr)
        return 1
    machine = machine_line()
    print(built)
    print()
    print(machine)
    if args.readme:
        path = ROOT / "README.md"
        text = path.read_text(encoding="utf-8")
        if BEGIN not in text or END not in text:
            print(f"README.md não tem os marcadores {BEGIN} / {END}", file=sys.stderr)
            return 1
        head, rest = text.split(BEGIN, 1)
        _, tail = rest.split(END, 1)
        block = f"{BEGIN}\n{built}\n\nMedido em: {machine}.\n{END}"
        path.write_text(head + block + tail, encoding="utf-8", newline="")
        print("README.md atualizado")
    return 0


if __name__ == "__main__":
    sys.exit(main())
