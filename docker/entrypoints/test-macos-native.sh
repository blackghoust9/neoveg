#!/usr/bin/env bash
# Builds and tests Neoveg natively inside the Docker-OSX guest (macOS under QEMU/KVM) over SSH.
# Writes /dist/reports/test-macos-native.txt and the native executable to /dist/macos-native/.
set -uo pipefail

HOST="${OSX_HOST:-docker-osx}"
PORT="${OSX_PORT:-10022}"
USER_NAME="${OSX_USER:-user}"
export SSHPASS="${OSX_PASSWORD:-alpine}"
SSH_OPTS=(-p "$PORT" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR)
REMOTE='$HOME/neoveg_src'
mac() { sshpass -e ssh "${SSH_OPTS[@]}" "$USER_NAME@$HOST" "$@"; }

mkdir -p /dist/reports /dist/macos-native
REPORT=/dist/reports/test-macos-native.txt
LOG=$(mktemp)
fail() { echo "[ERROR] $1" >&2; printf 'tests=FAILED\npassed=0\nfailed=0\nreason=%s\n' "$1" >"$REPORT"; exit 1; }

echo "[INFO] waiting for SSH on $HOST:$PORT (the first macOS boot takes several minutes)"
for _ in $(seq 1 120); do mac true 2>/dev/null && break; sleep 10; done
mac true 2>/dev/null || fail "macOS guest did not accept SSH (enable Remote Login in the image, see docker/osx/README.md)"

echo "[INFO] copying the sources into the guest"
mac "rm -rf $REMOTE && mkdir -p $REMOTE" || fail "cannot prepare $REMOTE"
tar -C /workspace --exclude=./target --exclude=./dist --exclude=./.git --exclude=./.neoveg_env -cf - . \
    | mac "tar -C $REMOTE -xf -" || fail "source copy failed"

echo "[INFO] cargo test --workspace (macOS $(mac 'sw_vers -productVersion' 2>/dev/null))"
FEATURES=""
[ "${NEOVEG_FFMPEG:-0}" = "1" ] && FEATURES="--features neoveg_ui/ffmpeg"
mac "export PATH=\"\$HOME/.cargo/bin:\$PATH\"; command -v cargo >/dev/null || { curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal; export PATH=\"\$HOME/.cargo/bin:\$PATH\"; }; cd $REMOTE && cargo test --locked --workspace $FEATURES" 2>&1 | tee "$LOG"
test_code=${PIPESTATUS[0]}

read -r passed failed < <(awk '/^test result:/ { for (i = 1; i <= NF; i++) { if ($i == "passed;") p += $(i - 1); if ($i == "failed;") f += $(i - 1) } } END { print p + 0, f + 0 }' "$LOG")
{ echo "tests=$([ "$test_code" -eq 0 ] && echo ok || echo FAILED)"; echo "passed=$passed"; echo "failed=$failed"; } >"$REPORT"
[ "$test_code" -eq 0 ] || exit 1

echo "[INFO] cargo build --release -p neoveg_ui"
mac "export PATH=\"\$HOME/.cargo/bin:\$PATH\"; cd $REMOTE && cargo build --locked --release -p neoveg_ui $FEATURES" || fail "native release build failed"
ARCH=$(mac 'uname -m' | tr -d '\r')
sshpass -e scp "${SSH_OPTS[@]/-p/-P}" "$USER_NAME@$HOST:neoveg_src/target/release/neoveg" "/dist/macos-native/neoveg_macos_$ARCH" || fail "cannot fetch the executable"
ls -la /dist/macos-native
