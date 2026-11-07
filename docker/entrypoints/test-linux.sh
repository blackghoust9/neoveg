#!/usr/bin/env bash
# Runs the quality gates inside a virtual X server (Xvfb), so UI code that needs a display can run.
#
# Xvfb is started by hand instead of `xvfb-run`: as PID 1 of a container, xvfb-run waits forever for the
# SIGUSR1 handshake (the kernel does not deliver signals without a handler to PID 1).
set -euo pipefail
cd /workspace

export DISPLAY="${DISPLAY:-:99}"
Xvfb "$DISPLAY" -screen 0 1280x720x24 -nolisten tcp >/tmp/xvfb.log 2>&1 &
XVFB_PID=$!
trap 'kill "$XVFB_PID" 2>/dev/null || true' EXIT

# Wait (up to 10 s) for the X socket
socket="/tmp/.X11-unix/X${DISPLAY#:}"
for _ in $(seq 1 100); do
    [ -S "$socket" ] && break
    kill -0 "$XVFB_PID" 2>/dev/null || { echo "[ERROR] Xvfb exited:" >&2; cat /tmp/xvfb.log >&2; exit 1; }
    sleep 0.1
done
[ -S "$socket" ] || { echo "[ERROR] Xvfb did not start" >&2; cat /tmp/xvfb.log >&2; exit 1; }
echo "[INFO] Xvfb running on $DISPLAY"

/usr/local/bin/gates-linux.sh
