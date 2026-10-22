#!/usr/bin/env bash
# ==============================================================================
# Neoveg - Automated Cross-Platform Self-Contained Portable Build Pipeline
# (scripts/build_all_platforms.sh)
#
# Publishes .NET 8 Self-Contained binaries, bundles Rust (neoveg_core) and C++
# (neoveg_cpp_engine) native libraries (.dll, .so, .dylib) and launcher scripts
# (neoveg.bat / neoveg.sh), and generates portable archives in dist/:
#   - dist/Neoveg_v1.0.0_win-x64_Portable.zip
#   - dist/Neoveg_v1.0.0_linux-x64_Portable.tar.gz
#   - dist/Neoveg_v1.0.0_osx-x64_Portable.tar.gz
#   - dist/Neoveg_v1.0.0_osx-arm64_Portable.tar.gz
#   - dist/Neoveg_v1.0.0_osx-universal_Portable.tar.gz
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist"
CSPROJ_PATH="${ROOT_DIR}/desktop-host/Neoveg.csproj"

mkdir -p "${DIST_DIR}"

echo "================================================================================"
echo " NEOVEG - MULTI-PLATFORM SELF-CONTAINED PORTABLE RELEASE BUILDER (Bash)"
echo "================================================================================"

# 1. Build / Synthesize Native Rust (neoveg_core) & C++ (neoveg_cpp_engine) FFI Libraries
echo "[1/4] Building & staging Rust (neoveg_core) and C++ (neoveg_cpp_engine) FFI libraries..."
python3 "${ROOT_DIR}/python-backend/native_ffi_builder.py" || python "${ROOT_DIR}/python-backend/native_ffi_builder.py"

# 2. Publish .NET 8 Self-Contained Binaries for all 4 Target RIDs
echo "[2/4] Publishing .NET 8 Self-Contained binaries..."

dotnet publish desktop-host/Neoveg.csproj -c Release -r win-x64 --self-contained true -o "${DIST_DIR}/Neoveg_v1.0.0_win-x64_Portable"
dotnet publish desktop-host/Neoveg.csproj -c Release -r linux-x64 --self-contained true -o "${DIST_DIR}/Neoveg_v1.0.0_linux-x64_Portable"
dotnet publish desktop-host/Neoveg.csproj -c Release -r osx-x64 --self-contained true -o "${DIST_DIR}/Neoveg_v1.0.0_osx-x64_Portable"
dotnet publish desktop-host/Neoveg.csproj -c Release -r osx-arm64 --self-contained true -o "${DIST_DIR}/Neoveg_v1.0.0_osx-arm64_Portable"

# 3. Bundle Native Shared Libraries, Launcher Scripts, Assets & Docs
bundle_platform() {
    local rid="$1"
    local folder="$2"
    local launcher="$3"
    shift 3
    local libs=("$@")

    local out_dir="${DIST_DIR}/${folder}"
    mkdir -p "${out_dir}/runtimes/${rid}/native" "${out_dir}/scripts" "${out_dir}/Assets/i18n"

    for lib in "${libs[@]}"; do
        if [[ -f "${ROOT_DIR}/runtimes/${rid}/native/${lib}" ]]; then
            cp -f "${ROOT_DIR}/runtimes/${rid}/native/${lib}" "${out_dir}/${lib}"
            cp -f "${ROOT_DIR}/runtimes/${rid}/native/${lib}" "${out_dir}/runtimes/${rid}/native/${lib}"
        fi
    done

    if [[ -f "${ROOT_DIR}/scripts/${launcher}" ]]; then
        cp -f "${ROOT_DIR}/scripts/${launcher}" "${out_dir}/${launcher}"
        cp -f "${ROOT_DIR}/scripts/${launcher}" "${out_dir}/scripts/${launcher}"
        if [[ "${launcher}" == "neoveg.sh" ]]; then
            chmod +x "${out_dir}/neoveg.sh" "${out_dir}/scripts/neoveg.sh" || true
            [[ -f "${out_dir}/Neoveg" ]] && chmod +x "${out_dir}/Neoveg" || true
        fi
    fi

    if [[ -d "${ROOT_DIR}/desktop-host/Assets/i18n" ]]; then
        cp -rf "${ROOT_DIR}/desktop-host/Assets/i18n/"* "${out_dir}/Assets/i18n/" || true
    fi

    for doc in AUTHORS.md LICENSE REQUIREMENTS.md RELEASE_NOTES.md README.md; do
        [[ -f "${ROOT_DIR}/${doc}" ]] && cp -f "${ROOT_DIR}/${doc}" "${out_dir}/${doc}" || true
    done
}

echo "[3/4] Bundling native FFI libraries (.dll/.so/.dylib) and launcher scripts..."
bundle_platform "win-x64"   "Neoveg_v1.0.0_win-x64_Portable"   "neoveg.bat" "neoveg_core.dll" "neoveg_cpp_engine.dll"
bundle_platform "linux-x64" "Neoveg_v1.0.0_linux-x64_Portable" "neoveg.sh"  "libneoveg_core.so" "libneoveg_cpp_engine.so"
bundle_platform "osx-x64"   "Neoveg_v1.0.0_osx-x64_Portable"   "neoveg.sh"  "libneoveg_core.dylib" "libneoveg_cpp_engine.dylib"
bundle_platform "osx-arm64" "Neoveg_v1.0.0_osx-arm64_Portable" "neoveg.sh"  "libneoveg_core.dylib" "libneoveg_cpp_engine.dylib"

# 4. Compress Portable Archives in dist/
echo "[4/4] Compressing Portable Release Archives in dist/..."
python3 -c "
import pathlib, zipfile
dist = pathlib.Path('${DIST_DIR}')
win_dir = dist / 'Neoveg_v1.0.0_win-x64_Portable'
zip_path = dist / 'Neoveg_v1.0.0_win-x64_Portable.zip'
with zipfile.ZipFile(zip_path, 'w', zipfile.ZIP_DEFLATED) as zf:
    for f in win_dir.rglob('*'):
        if f.is_file():
            zf.write(f, f.relative_to(win_dir))
"
tar -czf "${DIST_DIR}/Neoveg_v1.0.0_linux-x64_Portable.tar.gz" -C "${DIST_DIR}" "Neoveg_v1.0.0_linux-x64_Portable"
tar -czf "${DIST_DIR}/Neoveg_v1.0.0_osx-x64_Portable.tar.gz" -C "${DIST_DIR}" "Neoveg_v1.0.0_osx-x64_Portable"
tar -czf "${DIST_DIR}/Neoveg_v1.0.0_osx-arm64_Portable.tar.gz" -C "${DIST_DIR}" "Neoveg_v1.0.0_osx-arm64_Portable"
tar -czf "${DIST_DIR}/Neoveg_v1.0.0_osx-universal_Portable.tar.gz" -C "${DIST_DIR}" "Neoveg_v1.0.0_osx-x64_Portable" "Neoveg_v1.0.0_osx-arm64_Portable"

echo "[OK] All cross-platform portable packages generated in ${DIST_DIR}!"
