# BRIEFING — 2026-11-06T13:59:53Z

## Mission
Implement Milestone 3 (Color Grading per Event, Primary 4-Way Color Wheels, White Balance Neutral Picker, RGB & 6 HSL Transfer Curves, OpenColorIO/ACES Pipeline, and Color Match Across Clips — Features 11–16) on branch `preview/version-2.1.0`.

## 🔒 My Identity
- Archetype: worker
- Roles: implementer, qa, specialist
- Working directory: C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\worker_m3_1\
- Original parent: c220c328-9551-48e1-a35b-23e05589d782
- Milestone: Milestone 3 (Color Grading & Pipeline)

## 🔒 Key Constraints
- Run all Cargo commands sequentially with `-j 1` and `--test-threads=1`.
- NEVER execute `cargo test -p neoveg_ui` (verify `neoveg_ui` strictly using `cargo check -p neoveg_ui -j 1` and `cargo clippy -p neoveg_ui -j 1`).
- Clean-room Rust only (MIT OR Apache-2.0, zero GPL/LGPL, zero proprietary C++ libraries).
- Zero third-party trademarks in runtime strings or code identifiers.
- Genuine implementation only: DO NOT hardcode test results, expected outputs, or dummy facades.
- All verification commands must pass:
  1. `cargo check --workspace -j 1`
  2. `cargo clippy --workspace -j 1`
  3. `cargo test -p neoveg_common -j 1 -- --test-threads=1`
  4. `cargo test -p neoveg_video --lib -j 1 -- --test-threads=1`
  5. `cargo test -p neoveg_video --test e2e_video -j 1 -- --test-threads=1`
  6. `cargo test -p neoveg_engine -j 1 -- --test-threads=1`
  7. `python scripts/parity_matrix.py --check`

## Current Parent
- Conversation ID: c220c328-9551-48e1-a35b-23e05589d782
- Updated: not yet

## Task Summary
- **What to build**: Full implementation of Milestone 3:
  1. Primary 4-Way Color Wheels (`ColorWheelVector`, `ColorWheelsGrade`, polar/RGB conversion, numpad adjustments, master luma scaling).
  2. White Balance Neutral Picker (`pick_white_balance`, `pick_white_balance_from_region`, `pick_black_point`, `pick_white_point`).
  3. RGB & 6 HSL Transfer Curves (`SplineCurve` monotone Hermite spline, `RgbCurves` with 256-step LUT cache, `HslCurves` with periodic hue wrapping, `apply_rgb_curves`, `apply_hsl_curves`).
  4. OpenColorIO / ACES Pipeline (`neoveg_video::ocio`: `AcesSpace`, analytical S-Log/ACES log formulas, AP0/AP1/Rec709 matrices, RRT+ODT tone mapping fit, `AcesTransform`, 3D LUT generation).
  5. Color Match Across Clips (`ColorMatcher` trait, `StatisticalColorMatcher`, `match_color`, `match_color_statistics`).
  6. Per-Event Grade Integration (`EventColorGrade` in `neoveg_common::grade`, `TrackEvent.color_grade` in `neoveg_engine::timeline`, `apply_event_grade` in `neoveg_video::grading`).
- **Success criteria**: All features genuinely implemented, all cargo checks/clippy/tests pass cleanly.
- **Interface contracts**: `PROJECT.md`, `spec_miner_m3_2/handoff.md`, `explorer_m3_1/handoff.md`.
- **Code layout**: `crates/neoveg_common/src/grade.rs`, `crates/neoveg_video/src/grading.rs`, `crates/neoveg_video/src/ocio.rs`, `crates/neoveg_engine/src/timeline.rs`.

## Change Tracker
- **Files modified**: None yet
- **Build status**: Pending initial run
- **Pending issues**: None

## Quality Status
- **Build/test result**: Pending
- **Lint status**: Pending
- **Tests added/modified**: Pending

## Loaded Skills
None loaded from orchestrator dispatch.

## Key Decisions Made
- Clean-room analytical implementations for all color spaces and transfer curves.

## Artifact Index
- `DISPATCH.md` — Task assignment
- `BRIEFING.md` — Persistent working memory
