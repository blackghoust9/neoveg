#!/usr/bin/env bash
# Quality gates of AGENTS.md section 3: rustfmt, clippy (-D warnings) and the whole test suite.
# Writes /dist/reports/test-linux.txt so the host script can verify the outcome without parsing the
# console output. Exit code 0 only when every step passed.
set -uo pipefail
cd /workspace

REPORT_DIR=/dist/reports
mkdir -p "$REPORT_DIR"
LOG=$(mktemp)
FEATURES=()
if [ "${NEOVEG_FFMPEG:-0}" = "1" ]; then
    FEATURES=(--features neoveg_ui/ffmpeg)
fi

echo "[INFO] $(rustc --version) / $(cargo --version) / DISPLAY=${DISPLAY:-unset}"

status() { if [ "$1" -eq 0 ]; then echo ok; else echo FAILED; fi; }

echo "[INFO] cargo fmt --all -- --check"
cargo fmt --all -- --check
fmt_code=$?

echo "[INFO] cargo clippy --workspace --all-targets -- -D warnings"
cargo clippy --locked --workspace --all-targets "${FEATURES[@]}" -- -D warnings
clippy_code=$?

echo "[INFO] cargo test --workspace"
cargo test --locked --workspace "${FEATURES[@]}" 2>&1 | tee "$LOG"
test_code=${PIPESTATUS[0]}

# Sum the "test result: ok. N passed; M failed; ..." lines of every test binary
read -r passed failed < <(awk '/^test result:/ {
    for (i = 1; i <= NF; i++) {
        if ($i == "passed;") p += $(i - 1)
        if ($i == "failed;") f += $(i - 1)
    }
} END { print p + 0, f + 0 }' "$LOG")

{
    echo "platform=linux-x64"
    echo "rustc=$(rustc --version)"
    echo "ffmpeg_feature=${NEOVEG_FFMPEG:-0}"
    echo "fmt=$(status $fmt_code)"
    echo "clippy=$(status $clippy_code)"
    echo "tests=$(status $test_code)"
    echo "passed=$passed"
    echo "failed=$failed"
} | tee "$REPORT_DIR/test-linux.txt"

if [ -n "${HOST_UID:-}" ] && [ "${HOST_UID}" != "0" ]; then
    chown -R "${HOST_UID}:${HOST_GID:-$HOST_UID}" /dist 2>/dev/null || true
fi

if [ $fmt_code -ne 0 ] || [ $clippy_code -ne 0 ] || [ $test_code -ne 0 ] || [ "$failed" != "0" ]; then
    echo "[ERROR] quality gates failed" >&2
    exit 1
fi
echo "[INFO] quality gates passed ($passed tests)"
