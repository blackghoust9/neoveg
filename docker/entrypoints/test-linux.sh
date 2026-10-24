#!/usr/bin/env bash
# Runs the quality gates inside a virtual X server (Xvfb), so UI code that needs a display can run.
set -euo pipefail
cd /workspace
exec xvfb-run -a -s "-screen 0 1280x720x24 -nolisten tcp" /usr/local/bin/gates-linux.sh
