#!/usr/bin/env bash
# Runs the whole Neoveg Rust workspace through the isolated Docker environment (docker/):
#
#   1. checks that Docker is installed and running;
#   2. builds the images and runs rustfmt + clippy + cargo test under Xvfb (Linux container);
#   3. verifies that every test passed;
#   4. builds the optimized Linux binaries into dist/linux and, with --macos, the macOS ones
#      (Intel and Apple Silicon, cross-compiled) into dist/macos.
#
# Usage: scripts/test_rust_crossplatform.sh [--macos] [--ffmpeg] [--no-cache] [--tests-only]
#   --macos       also cross-compile for macOS (needs the SDK package in docker/sdk, see docker/sdk/README.md)
#   --ffmpeg      test and build with the `ffmpeg` feature (needs FFmpeg 7.1 dev libraries in the image)
#   --no-cache    rebuild the images from scratch
#   --tests-only  skip the release builds
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

MACOS=0
NO_CACHE=()
TESTS_ONLY=0
for arg in "$@"; do
    case "$arg" in
        --macos) MACOS=1 ;;
        --ffmpeg) export NEOVEG_FFMPEG=1 ;;
        --no-cache) NO_CACHE=(--no-cache) ;;
        --tests-only) TESTS_ONLY=1 ;;
        -h | --help) sed -n '2,15p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "unknown option: $arg" >&2; exit 2 ;;
    esac
done

fail() { echo "[ERROR] $*" >&2; exit 1; }
info() { echo "[INFO] $*"; }

# ---- 1. Docker ---------------------------------------------------------------------------------
command -v docker >/dev/null 2>&1 || fail "Docker is not installed. Install Docker Desktop or Docker Engine: https://docs.docker.com/get-docker/"
docker info >/dev/null 2>&1 || fail "Docker is installed but its daemon is not running. Start Docker and try again."
if docker compose version >/dev/null 2>&1; then
    COMPOSE=(docker compose)
elif command -v docker-compose >/dev/null 2>&1; then
    COMPOSE=(docker-compose)
else
    fail "Docker Compose was not found (neither 'docker compose' nor 'docker-compose')."
fi
COMPOSE+=(-f docker/docker-compose.yml)
if [ "$MACOS" = 1 ]; then
    COMPOSE+=(--profile macos)
    ls docker/sdk/MacOSX*.sdk.tar.* >/dev/null 2>&1 || fail "no macOS SDK package in docker/sdk/ (see docker/sdk/README.md)"
fi

HOST_UID="$(id -u 2>/dev/null || echo 0)"
HOST_GID="$(id -g 2>/dev/null || echo 0)"
export HOST_UID HOST_GID
mkdir -p dist/reports dist/linux logs
rm -f dist/reports/test-linux.txt

# ---- 2. images -----------------------------------------------------------------------------------
info "building the images"
SERVICES=(test-rust-linux)
[ "$TESTS_ONLY" = 1 ] || SERVICES+=(build-rust-linux-release)
[ "$MACOS" = 1 ] && [ "$TESTS_ONLY" = 0 ] && SERVICES+=(build-rust-macos-release)
# ${arr[@]+...} keeps empty arrays safe with `set -u` on the bash 3.2 that macOS ships
"${COMPOSE[@]}" build ${NO_CACHE[@]+"${NO_CACHE[@]}"} "${SERVICES[@]}"

# ---- 3. tests --------------------------------------------------------------------------------------
info "running rustfmt, clippy and cargo test in the Linux container"
"${COMPOSE[@]}" run --rm --no-deps test-rust-linux 2>&1 | tee logs/docker-test-linux.log \
    || fail "the quality gates failed; see logs/docker-test-linux.log"
REPORT=dist/reports/test-linux.txt
[ -f "$REPORT" ] || fail "the container did not write $REPORT"
grep -q '^fmt=ok$' "$REPORT" && grep -q '^clippy=ok$' "$REPORT" && grep -q '^tests=ok$' "$REPORT" \
    && grep -q '^failed=0$' "$REPORT" || fail "tests are not 100% green; see $REPORT"
PASSED="$(sed -n 's/^passed=//p' "$REPORT")"
[ "${PASSED:-0}" -gt 0 ] || fail "no test ran; see logs/docker-test-linux.log"
info "Linux: $PASSED tests passed, 0 failed (rustfmt and clippy clean)"

# ---- 4. release builds --------------------------------------------------------------------------------
if [ "$TESTS_ONLY" = 0 ]; then
    info "building the optimized Linux binaries"
    "${COMPOSE[@]}" run --rm --no-deps build-rust-linux-release || fail "the Linux release build failed"
    if [ "$MACOS" = 1 ]; then
        mkdir -p dist/macos
        info "cross-compiling for macOS (Intel and Apple Silicon)"
        "${COMPOSE[@]}" run --rm --no-deps build-rust-macos-release || fail "the macOS cross build failed"
    fi
fi

# ---- 5. artifacts --------------------------------------------------------------------------------------
sha() { if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1"; else shasum -a 256 "$1"; fi; }
info "artifacts in dist/:"
status=0
if [ "$TESTS_ONLY" = 0 ]; then
    for pattern in "dist/linux/neoveg_v*_linux-x64" "dist/linux/Neoveg_v*_linux-x64_Portable.tar.gz"; do
        # shellcheck disable=SC2086
        ls $pattern >/dev/null 2>&1 || { echo "[ERROR] missing $pattern" >&2; status=1; continue; }
        for f in $pattern; do sha "$f"; done
    done
    if [ "$MACOS" = 1 ]; then
        for rid in osx-x64 osx-arm64; do
            ls dist/macos/Neoveg_v*_"$rid"_Portable.tar.gz >/dev/null 2>&1 || { echo "[ERROR] missing macOS $rid package" >&2; status=1; continue; }
            for f in dist/macos/Neoveg_v*_"$rid"_Portable.tar.gz dist/macos/neoveg_v*_"$rid"; do sha "$f"; done
        done
    fi
fi
[ "$status" = 0 ] || fail "some artifacts were not produced"
info "done"
