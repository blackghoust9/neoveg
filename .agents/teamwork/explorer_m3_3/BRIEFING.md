# BRIEFING — 2026-11-06T13:58:00Z

## Mission
Milestone 3 Integration & UI Architecture Explorer: Investigate engine integration, GUI wiring/scopes, and CPU execution for Color Grading & OCIO/ACES.

## 🔒 My Identity
- Archetype: explorer
- Roles: investigation, synthesis
- Working directory: C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\explorer_m3_3
- Original parent: c220c328-9551-48e1-a35b-23e05589d782
- Milestone: Milestone 3

## 🔒 Key Constraints
- Read-only investigation — do NOT implement
- Sequential execution under -j 1 and --test-threads=1
- Never execute cargo test -p neoveg_ui
- Zero third-party trademarks in runtime strings or comments
- No files modified outside .agents/teamwork/explorer_m3_3/

## Current Parent
- Conversation ID: c220c328-9551-48e1-a35b-23e05589d782
- Updated: 2026-11-06T13:44:37Z

## Investigation State
- **Explored paths**:
  - `crates/neoveg_engine/src/`: `timeline.rs`, `video_effects.rs`, `command.rs`, `project_file.rs`
  - `crates/neoveg_common/src/`: `grade.rs`, `commands.rs`, `project.rs`
  - `crates/neoveg_video/src/`: `grading.rs`, `lut.rs`, `vfx.rs`, `worker.rs`, `scopes.rs`, `tests/e2e_video.rs`
  - `crates/neoveg_ui/src/`: `app.rs`, `curve_editor.rs`, `scopes_panel.rs`, `footer_tabs.rs`, `dock.rs`, `app/composite.rs`
  - `manuais/markdown/`: `manual_vegaspro21_en.md` (lines 33640-35240)
- **Key findings**:
  - `neoveg_engine`: `Timeline` currently stores project-level `video_grade: Grade` and `video_lut: Option<String>`. `TrackEvent` has `video_effects: Vec<TrackEffect>` and `pan_crop: PanCrop`, but does not yet store dedicated per-event color grades. `VideoFxTarget` supports `Event`, `Track`, `Media`, and `Project`.
  - `neoveg_video`: `grading.rs` has basic `apply_grade` and `apply_look` with 1-pixel repeated cache. `lut.rs` supports 1D & 3D `.cube` tables with trilinear interpolation. `vfx.rs` already contains full `curve_luts` (256-step monotone cubic Hermite) and `three_way_color_px`. `tests/e2e_video.rs` contains tested implementations of `ColorWheelsGrade`, `pick_white_balance`, `curve_luts`, and `match_color_statistics`.
  - `neoveg_ui`: `app.rs` has a simple modal dialog `color_grade_window` committing project-level `timeline.video_grade`. Dock lacks `Tab::ColorGrading`. `curve_editor.rs` provides an interactive curve canvas for `vfx::COLOR_CURVES`.
  - CPU execution: 1D LUT precomputation (256 entries = 768 B) fits in L1 cache for instant O(1) channel transfer without repeated `powf()`. 3D LUTs benefit from run-length pixel memoization and proxy downsampling. Contiguous 4-byte RGBA chunks vectorize via SSE2/AVX2.
- **Unexplored areas**: None for M3 architecture scope. Ready for handoff synthesis.

## Key Decisions Made
- Recommend hybrid engine representation: typed `EventColorGrade` / `ColorWheelsGrade` in `neoveg_common::grade` + first-class field on `TrackEvent` with serde default, along with generalized `EditCommand::SetColorGrade { target: VideoFxTarget, ... }`.
- Recommend adding `Tab::ColorGrading` to `neoveg_ui` dock with target selector matching VEGAS 4-level architecture (Event FX, Track FX, Media FX, Video Output FX).
- Recommend 1D LUT precomputation strategy for CPU performance on limited hardware.

## Artifact Index
- DISPATCH.md — Task instruction from parent
- BRIEFING.md — Persistent working memory
- progress.md — Liveness heartbeat
- handoff.md — 5-component structured investigation report
