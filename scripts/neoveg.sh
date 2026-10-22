#!/usr/bin/env bash
# ==============================================================================
# Neoveg — Cross-Platform POSIX Launcher (Linux & macOS)
# Detects OS (`Linux` vs `Darwin` / macOS), configures `LD_LIBRARY_PATH` or
# `DYLD_LIBRARY_PATH` for Rust (`libneoveg_core.so` / `.dylib`) and C++
# (`libneoveg_cpp_engine.so` / `.dylib`) FFI libraries, initializes local cache
# (`~/.cache/neoveg/` on Linux, `~/Library/Caches/neoveg/` on macOS), and launches Neoveg.
# ==============================================================================
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
OS_KERNEL="$(uname -s)"
ARCH_NAME="$(uname -m)"

case "${OS_KERNEL}" in
    Linux*)
        NEOVEG_OS="Linux"
        if [[ "${ARCH_NAME}" == "aarch64" || "${ARCH_NAME}" == "arm64" ]]; then
            NEOVEG_RID="linux-arm64"
        else
            NEOVEG_RID="linux-x64"
        fi
        NEOVEG_CACHE_ROOT="${HOME}/.cache/neoveg"
        NEOVEG_FFI_EXT=".so"
        NATIVE_LIB_DIR="${ROOT_DIR}/runtimes/${NEOVEG_RID}/native"
        FFMPEG_LIB_DIR="${ROOT_DIR}/runtimes/${NEOVEG_RID}/ffmpeg/lib"
        FFMPEG_BIN_DIR="${ROOT_DIR}/runtimes/${NEOVEG_RID}/ffmpeg/bin"
        export LD_LIBRARY_PATH="${NATIVE_LIB_DIR}:${FFMPEG_LIB_DIR}:${ROOT_DIR}:${LD_LIBRARY_PATH:-}"
        export PATH="${FFMPEG_BIN_DIR}:${PATH}"
        ;;
    Darwin*)
        NEOVEG_OS="macOS"
        if [[ "${ARCH_NAME}" == "arm64" ]]; then
            NEOVEG_RID="osx-arm64"
        else
            NEOVEG_RID="osx-x64"
        fi
        NEOVEG_CACHE_ROOT="${HOME}/Library/Caches/neoveg"
        NEOVEG_FFI_EXT=".dylib"
        NATIVE_LIB_DIR="${ROOT_DIR}/runtimes/${NEOVEG_RID}/native"
        FFMPEG_LIB_DIR="${ROOT_DIR}/runtimes/${NEOVEG_RID}/ffmpeg/lib"
        FFMPEG_BIN_DIR="${ROOT_DIR}/runtimes/${NEOVEG_RID}/ffmpeg/bin"
        export DYLD_LIBRARY_PATH="${NATIVE_LIB_DIR}:${FFMPEG_LIB_DIR}:${ROOT_DIR}:${DYLD_LIBRARY_PATH:-}"
        export PATH="${FFMPEG_BIN_DIR}:${PATH}"
        ;;
    *)
        echo "[ERROR] Unsupported POSIX kernel: ${OS_KERNEL}. Use scripts/neoveg.bat on Windows." >&2
        exit 1
        ;;
esac

mkdir -p "${NEOVEG_CACHE_ROOT}/app_cache" "${NEOVEG_CACHE_ROOT}/logs" "${ROOT_DIR}/logs"

echo "[Neoveg] OS=${NEOVEG_OS} (${NEOVEG_RID}) | FFI=*${NEOVEG_FFI_EXT} | Cache=${NEOVEG_CACHE_ROOT}"

# Resolve compiled Neoveg binary or fallback to dotnet run
CANDIDATES=(
    "${ROOT_DIR}/Neoveg"
    "${ROOT_DIR}/dist/Neoveg-Portable-${NEOVEG_RID}/Neoveg.AppDir/usr/bin/Neoveg"
    "${ROOT_DIR}/dist/Neoveg-Portable-${NEOVEG_RID}/Neoveg.app/Contents/MacOS/Neoveg"
    "${ROOT_DIR}/desktop-host/bin/Release/net8.0/${NEOVEG_RID}/publish/Neoveg"
    "${ROOT_DIR}/desktop-host/bin/Release/net8.0/Neoveg"
    "${ROOT_DIR}/desktop-host/bin/Debug/net8.0/Neoveg"
)

for BIN_PATH in "${CANDIDATES[@]}"; do
    if [[ -f "${BIN_PATH}" ]]; then
        chmod +x "${BIN_PATH}" 2>/dev/null || true
        exec "${BIN_PATH}" "$@"
    fi
done

if command -v dotnet >/dev/null 2>&1; then
    exec dotnet run --project "${ROOT_DIR}/desktop-host/Neoveg.csproj" -c Release -- "$@"
else
    echo "[ERROR] Neoveg binary not found and 'dotnet' SDK is not installed." >&2
    exit 1
fi
