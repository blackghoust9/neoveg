# Task Assignment: spec_miner_m6_1

## Objective
Specification mining from VEGAS Pro reference manuals (`manuais/`) and reverse-engineering records (`results.md`) for Milestone 6 (Features 35–42: L/J cuts, simultaneous fades, sync links, multicam, takes, shortcuts, and dialogs).


## 2026-11-06T18:39:20Z
[Message] timestamp=2026-11-06T18:39:20Z sender=c220c328-9551-48e1-a35b-23e05589d782 priority=MESSAGE_PRIORITY_HIGH content=You are spec_miner_m6_1, Specification Miner for Milestone 6 (Features 35–42: Timeline Editing, Multicam & Takes).
Your working directory is: C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\spec_miner_m6_1\
Your parent orchestrator conversation ID is: c220c328-9551-48e1-a35b-23e05589d782

MANDATORY INPUTS (read all before investigating):
- C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\ORIGINAL_REQUEST.md
- C:\Users\paulomatos\Documents\GitHub\neoveg\PROJECT.md
- C:\Users\paulomatos\Documents\GitHub\neoveg\manuais\markdown\manual_vegaspro21_en.md
- C:\Users\paulomatos\Documents\GitHub\neoveg\manuais\markdown\VEGAS_Pro_18_EN_Keyboard_shortcuts.md
- C:\Users\paulomatos\Documents\GitHub\neoveg\results.md

TASK:
Mine precise, authoritative specifications, formulas, edge cases, keyboard shortcuts, and UI workflows from the VEGAS Pro reference manuals and reverse-engineering records for:
1. Feature 35 — L-Cut & J-Cut Split Editing:
   - Behavior of independent audio vs video edge trimming (split edits).
   - "Ignore Event Grouping" mode (Ctrl+Shift+U) and per-event slip/trim behavior.
   - Ripple edit semantics when trimming one track in an L/J cut pair.
2. Feature 36 — Simultaneous Audio/Video Fades:
   - Behavior when dragging fade handles on grouped/linked events.
   - Micro-quickfades on audio cuts (preventing clicks/pops, standard 10ms ramp).
   - 5 fade curve types (Fast, Slow, Smooth, Sharp, Linear) and their mathematical transfer functions.
3. Feature 37 — Persistent Sync Links:
   - How sync links track relative offset between audio and video events (VP12/VP13).
   - Visual indicators for out-of-sync events (e.g. red header badge, out-of-sync time delta display).
   - "Synchronize" context menu action (restores original alignment).
4. Feature 38 — Multicamera Audio/Timecode Sync:
   - Multicam synchronization algorithms: timecode matching vs audio waveform cross-correlation.
   - Tolerance thresholds, sample rate requirements, and handling multiple unaligned takes.
5. Feature 39 — Multicamera Track & Live Angle Switching:
   - Container track model, multi-camera split view (2x2, 3x3, 4x4 layout).
   - Real-time click-to-cut and numeric keypad shortcuts (Numpad 1..9).
   - Cut-to-angle vs switch-angle modes.
6. Feature 40 — Alternative Event Takes System:
   - Adding takes to an event container, active take selection.
   - Cycling takes with 'T' (next take) and 'Shift+T' (previous take).
   - "Slip all takes" command (maintains relative take offsets).
7. Generic clean-room terminology:
   - Confirm generic naming for any proprietary terms (zero third-party trademarks).

Deliver a structured handoff report in `handoff.md` and notify orchestrator via `send_message`.
