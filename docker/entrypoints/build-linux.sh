#!/usr/bin/env bash
# Optimized Linux build: portable folder, tar.gz and standalone executable in dist/linux/.
set -euo pipefail
cd /workspace

ARGS=(build --skip-qa --targets linux-x64 --dist /dist/linux)
if [ "${NEOVEG_FFMPEG:-0}" = "1" ]; then
    ARGS+=(--ffmpeg)
    # The Debian packages install FFmpeg system-wide; build_portable.py bundles from FFMPEG_DIR
    export FFMPEG_DIR="${FFMPEG_DIR:-/usr}"
    export LIBCLANG_PATH="${LIBCLANG_PATH:-$(dirname "$(find /usr/lib -name 'libclang*.so*' | head -1)")}"
fi

python3 build_portable.py "${ARGS[@]}"

if [ -n "${HOST_UID:-}" ] && [ "${HOST_UID}" != "0" ]; then
    chown -R "${HOST_UID}:${HOST_GID:-$HOST_UID}" /dist 2>/dev/null || true
fi
ls -la /dist/linux
