# BRIEFING — 2026-11-06T14:40:00Z

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
- **What to build**: Full genuine implementation of Milestone 3:
  1. Primary 4-Way Color Wheels (`ColorWheelVector`, `ColorWheelsGrade`, polar/RGB conversion, numpad adjustments, master luma scaling).
  2. White Balance Neutral Picker (`pick_white_balance`, `pick_white_balance_from_region`, `pick_black_point`, `pick_white_point`).
  3. RGB & 6 HSL Transfer Curves (`SplineCurve` monotone Hermite spline, `RgbCurves` with 256-step LUT cache, `HslCurves` with periodic hue wrapping, `apply_rgb_curves`, `apply_hsl_curves`).
  4. OpenColorIO / ACES Pipeline (`neoveg_video::ocio`: `AcesSpace`, analytical S-Log/ACES log formulas, AP0/AP1/Rec709 matrices, RRT+ODT tone mapping fit, `AcesTransform`, 3D LUT generation).
  5. Color Match Across Clips (`ColorMatcher` trait, `StatisticalColorMatcher`, `match_color`, `match_color_statistics`).
  6. Per-Event Grade Integration (`EventColorGrade` in `neoveg_common::grade`, `TrackEvent.color_grade` in `neoveg_engine::timeline`, `apply_event_grade` in `neoveg_video::grading`).
- **Success criteria**: All features genuinely implemented, all cargo checks/clippy/tests pass cleanly.
- **Interface contracts**: `PROJECT.md`, `spec_miner_m3_2/handoff.md`, `explorer_m3_1/handoff.md`.
- **Code layout**: `crates/neoveg_common/src/grade.rs`, `crates/neoveg_video/src/grading.rs`, `crates/neoveg_video/src/ocio.rs`, `crates/neoveg_engine/src/timeline.rs`, `crates/neoveg_engine/src/command.rs`.

## Change Tracker
- **Files modified**:
  - `crates/neoveg_common/src/grade.rs`: Core types (`ColorWheelVector`, `ColorWheelsGrade`, `WhiteBalanceGrade`, `RgbCurvesData`, `HslCurvesData`, `OcioSettings`, `EventColorGrade`).
  - `crates/neoveg_engine/src/timeline.rs`: Added `pub color_grade: Option<EventColorGrade>` to `TrackEvent`.
  - `crates/neoveg_engine/src/command.rs`: Added `SetEventColorGrade` edit command and preserved grades across splits.
  - `crates/neoveg_video/src/lut.rs`: Added `Lut::from_3d_data`.
  - `crates/neoveg_video/src/ocio.rs`: ACES spaces, view transforms, chromatic adaptation, analytical curves, RRT tone mapper, 3D LUT generation.
  - `crates/neoveg_video/src/grading.rs`: Fritsch-Carlson monotone Hermite splines, RGB 256-step LUT cache, 6 HSL curves with periodic hue wrapping, neutral pickers, statistical color matcher, and full grade pipeline.
  - `crates/neoveg_video/tests/e2e_video.rs`: 13 end-to-end integration tests validating all Milestone 3 features.
- **Build status**: All cargo checks and tests passing cleanly across workspace.
- **Pending issues**: None.

## Quality Status
- **Build/test result**: Pass (neoveg_common: 243 passed; neoveg_engine: 616 passed; neoveg_video lib: 194 passed; neoveg_video e2e: 157 passed).
- **Lint status**: 0 clippy warnings workspace-wide.
- **Tests added/modified**: 13 comprehensive E2E tests + module tests in `grade.rs`, `ocio.rs`, and `grading.rs`.

## Loaded Skills
None loaded from orchestrator dispatch.

## Key Decisions Made
- `EventColorGrade` and basic vectors placed in `neoveg_common::grade` to avoid cyclic or illegal dependencies from `neoveg_engine` into `neoveg_video`.
- High-performance pixel processing with monotone cubic Hermite spline interpolation and periodic wrapping for angular channels.
- Full analytical clean-room implementation of ACES transforms and camera log curves.

## Artifact Index
- `DISPATCH.md` — Task assignment
- `BRIEFING.md` — Persistent working memory
- `progress.md` — Liveness heartbeat and milestone tracking
- `handoff.md` — Self-contained 5-component handoff report
