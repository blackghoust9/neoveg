#!/usr/bin/env bash
# Neoveg Cross-Platform Portable Builder (Linux / macOS)
# Builds portable distributions (~200MB-300MB) for Windows (.exe), Linux (.AppImage), and macOS (.app)
set -e
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

PYTHON_BIN="python3"
if ! command -v python3 >/dev/null 2>&1; then
    PYTHON_BIN="python"
fi

"${PYTHON_BIN}" "${ROOT_DIR}/build_portable.py" "$@"
