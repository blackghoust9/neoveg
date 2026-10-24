#!/usr/bin/env bash
# Cross-compiles Neoveg for macOS (Intel and Apple Silicon) with osxcross and packs it into dist/macos/.
#
# 1. compiles every test binary of the workspace for both targets (they cannot run on Linux, but a
#    compile error on macOS-only code paths is caught here);
# 2. builds the release binaries and the packages through build_portable.py.
set -euo pipefail
cd /workspace

BIN=/opt/osxcross/target/bin
SDKROOT=$(ls -d /opt/osxcross/target/SDK/MacOSX*.sdk | head -1)
export SDKROOT
export LD_LIBRARY_PATH="/opt/osxcross/target/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

# Linkers and C toolchain per target (osxcross wrappers: o64 = x86_64, oa64 = aarch64)
export CARGO_TARGET_X86_64_APPLE_DARWIN_LINKER="$BIN/o64-clang"
export CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER="$BIN/oa64-clang"
export CC_x86_64_apple_darwin="$BIN/o64-clang"
export CXX_x86_64_apple_darwin="$BIN/o64-clang++"
export AR_x86_64_apple_darwin="$(ls "$BIN"/x86_64-apple-darwin*-ar | head -1)"
export CC_aarch64_apple_darwin="$BIN/oa64-clang"
export CXX_aarch64_apple_darwin="$BIN/oa64-clang++"
export AR_aarch64_apple_darwin="$(ls "$BIN"/aarch64-apple-darwin*-ar | head -1)"
# Crates that run bindgen against Apple headers (coreaudio-sys through cpal)
export BINDGEN_EXTRA_CLANG_ARGS_x86_64_apple_darwin="--sysroot=$SDKROOT -I$SDKROOT/usr/include"
export BINDGEN_EXTRA_CLANG_ARGS_aarch64_apple_darwin="--sysroot=$SDKROOT -I$SDKROOT/usr/include"

echo "[INFO] $(rustc --version), SDK $(basename "$SDKROOT")"

for target in x86_64-apple-darwin aarch64-apple-darwin; do
    echo "[INFO] compiling the workspace tests for $target"
    cargo test --locked --workspace --no-run --target "$target"
done

python3 build_portable.py build --skip-qa --targets osx-x64,osx-arm64 --dist /dist/macos

if [ -n "${HOST_UID:-}" ] && [ "${HOST_UID}" != "0" ]; then
    chown -R "${HOST_UID}:${HOST_GID:-$HOST_UID}" /dist 2>/dev/null || true
fi
ls -la /dist/macos
