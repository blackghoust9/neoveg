#!/usr/bin/env python3
"""Neoveg trademark & brand-neutrality audit (AGENTS.md, "Strict Trademark & Brand Neutrality Protocol").

Scans the Git-tracked files (default) and, optionally, build output folders for prohibited third-party
trademarks in file contents and file/folder names. Exits with code 1 when any occurrence is found.

Usage:
    python scripts/audit_trademarks.py                      # repository (git ls-files)
    python scripts/audit_trademarks.py --path dist/Neoveg-Portable-win-x64   # compiled output too

Exempt from the scan: this script and the protocol block in AGENTS.md delimited by
<!-- trademark-audit:allow-start --> / <!-- trademark-audit:allow-end -->, which must list the terms.
"""
from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

ROOT_DIR = Path(__file__).resolve().parent.parent

# Assembled from fragments so the patterns are explicit without the words appearing verbatim elsewhere.
_TERMS = ["so" + "ny", "ve" + "gas", "ma" + "gix", "bo" + r"ris[\s_-]?fx", "so" + r"nic[\s_-]?foundry"]
PROHIBITED = re.compile("|".join(_TERMS), re.IGNORECASE)

ALLOW_START = "<!-- trademark-audit:allow-start -->"
ALLOW_END = "<!-- trademark-audit:allow-end -->"
SELF = Path(__file__).resolve()
BINARY_SUFFIXES = {
    ".png", ".jpg", ".jpeg", ".gif", ".ico", ".icns", ".mp4", ".mov", ".wav", ".mp3", ".sfk", ".zip", ".gz",
    ".7z", ".pdf", ".docx", ".neoveg", ".ttf", ".otf", ".woff", ".woff2", ".pdb", ".so", ".dylib", ".exe", ".dll",
}


def tracked_files() -> list[Path]:
    out = subprocess.run(["git", "ls-files", "-z"], cwd=ROOT_DIR, capture_output=True, check=True)
    return [ROOT_DIR / p for p in out.stdout.decode("utf-8").split("\0") if p]


def strip_allowed_blocks(text: str) -> str:
    return re.sub(re.escape(ALLOW_START) + r".*?" + re.escape(ALLOW_END), "", text, flags=re.DOTALL)


def scan_file(path: Path, scan_binaries: bool) -> list[str]:
    findings: list[str] = []
    rel = path.relative_to(ROOT_DIR) if path.is_relative_to(ROOT_DIR) else path
    if PROHIBITED.search(str(rel)):
        findings.append(f"{rel}: prohibited term in file or folder name")
    if path.resolve() == SELF or not path.is_file():
        return findings

    data = path.read_bytes()
    if path.suffix.lower() in BINARY_SUFFIXES or b"\0" in data[:4096]:
        if scan_binaries:
            for enc in ("utf-8", "utf-16-le"):
                for term in ("so" + "ny", "ve" + "gas", "ma" + "gix", "bo" + "risfx"):
                    for variant in (term, term.upper(), term.capitalize()):
                        if variant.encode(enc) in data:
                            findings.append(f"{rel}: prohibited term '{variant}' embedded in binary ({enc})")
        return findings

    text = data.decode("utf-8", errors="ignore")
    if path.name == "AGENTS.md":
        text = strip_allowed_blocks(text)
    for line_no, line in enumerate(text.splitlines(), start=1):
        for m in PROHIBITED.finditer(line):
            findings.append(f"{rel}:{line_no}: '{m.group(0)}' -> {line.strip()[:120]}")
    return findings


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--path", action="append", default=[], help="extra folder to scan (e.g. build output)")
    parser.add_argument("--no-repo", action="store_true", help="skip the git-tracked repository files")
    args = parser.parse_args()

    files: list[Path] = [] if args.no_repo else tracked_files()
    extra_roots = [Path(p).resolve() for p in args.path]
    for root in extra_roots:
        files.extend(p for p in root.rglob("*") if p.is_file())

    findings: list[str] = []
    for f in files:
        findings.extend(scan_file(f, scan_binaries=any(f.is_relative_to(r) for r in extra_roots)))

    if findings:
        print(f"[FAIL] Trademark audit: {len(findings)} occurrence(s) of prohibited terms")
        for item in findings[:200]:
            print("  " + item)
        if len(findings) > 200:
            print(f"  ... {len(findings) - 200} more")
        return 1
    print(f"[PASS] Trademark audit: 0 occurrences in {len(files)} files")
    return 0


if __name__ == "__main__":
    sys.exit(main())
