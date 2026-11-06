# Task Assignment — worker_m3_1: Milestone 3 Implementation

## Mission
Implement Milestone 3: Color Grading per Event, Primary 4-Way Color Wheels, White Balance Neutral Picker, RGB & 6 HSL Transfer Curves, OpenColorIO (OCIO) / ACES Pipeline, and Color Match Across Clips (Features 11–16) on branch `preview/version-2.1.0`.

## Working Directory
`C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\worker_m3_1`

## Mandatory Documents to Read
1. `C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\ORIGINAL_REQUEST.md` (Verbatim user requirements)
2. `C:\Users\paulomatos\Documents\GitHub\neoveg\PROJECT.md` (Architecture, contracts, code layout)
3. `C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\spec_miner_m3_2\handoff.md` (Full mathematical formulas, color wheel math, ACES matrices, transfer functions, numpad shortcuts)
4. `C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\explorer_m3_1\handoff.md` (Codebase blueprint, types, module structure)
5. `C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\explorer_m3_3\handoff.md` (Engine integration, CPU optimizations, UI model-view decoupling)

## MANDATORY INTEGRITY WARNING
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

## Detailed Requirements

### 1. Primary 4-Way Color Wheels (`crates/neoveg_video/src/grading.rs`)
- Export `ColorWheelVector` with:
  - `pub x: f32`, `pub y: f32`, `pub master: f32`
  - `from_polar(hue_deg: f32, sat: f32, master: f32) -> Self`
  - `to_polar(&self) -> (f32, f32, f32)`
  - `from_rgb_sliders(r: f32, g: f32, b: f32, master: f32) -> Self`
  - `to_rgb_sliders(&self) -> (f32, f32, f32, f32)`
  - Numpad manipulation helper methods (`adjust_channel`, etc.)
- Export `ColorWheelsGrade` with:
  - `pub lift: ColorWheelVector`, `pub gamma: ColorWheelVector`, `pub gain: ColorWheelVector`, `pub offset: ColorWheelVector`
  - `pub fn apply_to_rgb(&self, rgb: [f32; 3]) -> [f32; 3]`
  - `pub fn apply_to_frame(&self, frame: &mut FrameBuffer)`
  - `pub fn apply_range_limited(&self, frame: &mut FrameBuffer, low_thresh: f32, high_thresh: f32)`

### 2. White Balance Neutral Tone Picker (`crates/neoveg_video/src/grading.rs`)
- Export `pub fn pick_white_balance(sample: [f32; 3]) -> (f32, f32)` (temperature, tint)
- Export `pub fn pick_white_balance_from_region(frame: &FrameBuffer, rect: [u32; 4]) -> (f32, f32)` (averages rectangular region, guards against dark pixels $Y \le 10^{-4}$)
- Export `pub fn pick_black_point(sample: [f32; 3]) -> ColorWheelVector` and `pub fn pick_white_point(sample: [f32; 3]) -> ColorWheelVector`

### 3. RGB & 6 HSL Transfer Curves (`crates/neoveg_video/src/grading.rs`)
- Export `SplineCurve` using Fritsch-Carlson monotone cubic Hermite algorithm (preserves monotonicity, arbitrary $N \ge 2$ control points).
- Export `RgbCurves` (Master, Red, Green, Blue splines) with 256-step L1-cached 1D LUT evaluation for fast CPU execution.
- Export `HslCurves` and `HslCurveType`:
  - `HueVsHue` (with periodic wrapping around $360^\circ$ so $0^\circ \equiv 360^\circ$)
  - `HueVsSat`, `HueVsLum`, `LumVsSat`, `SatVsSat`, `SatVsLum`
- Export `apply_rgb_curves` and `apply_hsl_curves`.

### 4. OpenColorIO (OCIO) & ACES Pipeline (`crates/neoveg_video/src/ocio.rs` re-exported in `neoveg_video`)
- Pure CPU clean-room implementation:
  - `AcesSpace` enum: `Aces2065_1` (AP0), `AcesCg` (AP1 linear), `AcesCc` (AP1 log), `AcesCct` (AP1 log with toe), `AcesProxy`, `SLog1`, `SLog2`, `SLog3`, `Rec709`, `Srgb`.
  - 3x3 Conversion matrices ($\mathbf{M}_{AP0 \leftrightarrow AP1}$, $\mathbf{M}_{AP1 \to Rec709}$, etc.).
  - Analytical transfer functions for Camera S-Log1/2/3 and ACEScc/ACEScct.
  - Analytical RRT + ODT Rec.709 / sRGB highlight rolloff (Stephen Hill / Narkowicz fit).
  - `AcesTransform` struct with `transform_rgb(&self, rgb: [f32; 3]) -> [f32; 3]`, `apply_to_frame(&self, frame: &mut FrameBuffer)`, and `generate_3d_lut(&self, size: usize) -> Lut`.

### 5. Color Match Across Clips (`crates/neoveg_video/src/grading.rs`)
- Export `pub trait ColorMatcher: Send + Sync { fn analyze_and_match(&self, reference: &FrameBuffer, target: &FrameBuffer) -> ColorWheelsGrade; }`
- Export `StatisticalColorMatcher` implementing `ColorMatcher`.
- Export `pub fn match_color(target: &mut FrameBuffer, reference: &FrameBuffer, match_brightness: bool, strength: f32)`.
- Export `pub fn match_color_statistics(target: &mut FrameBuffer, reference: &FrameBuffer)` for backward compatibility.

### 6. Per-Event & Layered Color Grading Integration
- In `crates/neoveg_common/src/grade.rs`: Define `EventColorGrade` struct incorporating color wheels, white balance, utilities, curves, LUT references, and OCIO settings.
- In `crates/neoveg_engine/src/timeline.rs`: Add `pub color_grade: Option<EventColorGrade>` to `TrackEvent` with `#[serde(default, skip_serializing_if = "Option::is_none")]` (ensuring existing project files deserialize cleanly).
- In `crates/neoveg_video/src/grading.rs`: Implement `apply_event_grade(frame: &mut FrameBuffer, grade: &EventColorGrade)`.

### 7. Verification Constraints & Commands
- Strict 8 GB RAM execution budget: run all Cargo commands sequentially with `-j 1` and `--test-threads=1`.
- NEVER execute `cargo test -p neoveg_ui` (verify `neoveg_ui` strictly using `cargo check -p neoveg_ui -j 1` and `cargo clippy -p neoveg_ui -j 1`).
- Verification commands to run and document in `handoff.md`:
  1. `cargo check --workspace -j 1`
  2. `cargo clippy --workspace -j 1`
  3. `cargo test -p neoveg_common -j 1 -- --test-threads=1`
  4. `cargo test -p neoveg_video --lib -j 1 -- --test-threads=1`
  5. `cargo test -p neoveg_video --test e2e_video -j 1 -- --test-threads=1`
  6. `cargo test -p neoveg_engine -j 1 -- --test-threads=1`
  7. `python scripts/parity_matrix.py --check`
- Zero third-party trademarks in runtime strings or code identifiers.
- When done, write `handoff.md` and send message to parent.


## 2026-11-06T13:59:53Z
You are worker_m3_1, assigned to implement Milestone 3 (Color Grading per Event, Primary 4-Way Color Wheels, White Balance Neutral Picker, RGB & 6 HSL Transfer Curves, OpenColorIO/ACES Pipeline, and Color Match Across Clips — Features 11–16).

Working directory: C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\worker_m3_1\
Read your task description in: C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\worker_m3_1\DISPATCH.md
Also read:
- C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\ORIGINAL_REQUEST.md
- C:\Users\paulomatos\Documents\GitHub\neoveg\PROJECT.md
- C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\spec_miner_m3_2\handoff.md
- C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\explorer_m3_1\handoff.md
- C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\explorer_m3_3\handoff.md

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

CRITICAL EXECUTION BUDGET:
- Run all Cargo commands sequentially with `-j 1` and `--test-threads=1`.
- NEVER execute `cargo test -p neoveg_ui` (verify `neoveg_ui` strictly using `cargo check -p neoveg_ui -j 1` and `cargo clippy -p neoveg_ui -j 1`).
- Clean-room Rust only (MIT OR Apache-2.0, zero GPL/LGPL, zero proprietary C++ libraries).
- Zero third-party trademarks in runtime strings or code identifiers.

When all code and tests are implemented and passing, write `handoff.md` in your working directory and notify me via `send_message`.
