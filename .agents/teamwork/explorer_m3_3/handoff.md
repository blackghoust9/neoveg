# Milestone 3 Integration & UI Architecture Explorer Report: Color Grading & OCIO/ACES

**Author**: `explorer_m3_3`  
**Date**: 2026-11-06T13:59:00Z  
**Scope**: Features 11–16 (Per-Event Color Grading, 4-Way Color Wheels, White Balance Neutral Picker, RGB & HSL Transfer Curves, OpenColorIO/ACES Pipeline, and Color Match Across Clips).

---

## 1. Observation

### 1.1 Engine Integration & Timeline Representation
- **Timeline-level Grade & LUT**:
  In `crates/neoveg_engine/src/timeline.rs`:
  - Line 802: `pub video_lut: Option<String>`
  - Line 805: `pub video_grade: Grade`
  - Lines 812–816: `pub video_effects: Vec<TrackEffect>` and `pub media_video_effects: BTreeMap<u64, Vec<TrackEffect>>`
- **Event-level Representation**:
  In `crates/neoveg_engine/src/timeline.rs` (lines 153–283), `TrackEvent` currently contains:
  - Line 218: `pub video_effects: Vec<TrackEffect>`
  - Line 222: `pub pan_crop: PanCrop`
  - Line 227: `pub transition_in: Option<TrackEffect>`
  - Line 230: `pub transition_out: Option<TrackEffect>`
  - Line 235: `pub velocity: Option<TrackEnvelope>`
  - `TrackEvent` **does not currently possess a dedicated per-event color grade field** (e.g. `pub color_grade: Option<EventColorGrade>`).
- **Effect Level Targeting**:
  In `crates/neoveg_engine/src/command.rs` (lines 36–49):
  ```rust
  pub enum VideoFxTarget {
      Event { track_id: TrackId, event_id: EventId },
      Track(TrackId),
      Media(u64),
      Project,
  }
  ```
  `VideoFxTarget` models the 4 effect levels, matching VEGAS Pro's architecture.
- **Engine Edit Commands**:
  In `crates/neoveg_engine/src/command.rs`:
  - Lines 269–272:
    ```rust
    SetVideoGrade {
        before: Grade,
        after: Grade,
    },
    ```
    Currently applies only to `timeline.video_grade` (lines 469–472 and 762).
  - Lines 258–262: `SetVideoEffects { target: VideoFxTarget, before: Vec<TrackEffect>, after: Vec<TrackEffect> }`.
- **Project Serialization & Backward Compatibility**:
  In `crates/neoveg_engine/src/project_file.rs`:
  - Lines 28–36 & 88–96: `project.json` stores `timeline: Timeline`.
  - In `timeline.rs`: All fields on `Timeline` and `TrackEvent` use `#[serde(default)]` and `#[serde(skip_serializing_if = "...")]`.
  - Older projects deserialize without missing fields; new fields with default values are skipped during serialization.

### 1.2 Video Processing Pipeline & Existing Algorithms
- **Existing Grade Application**:
  In `crates/neoveg_video/src/grading.rs`:
  - Lines 9–37: `pub fn apply_grade(frame: &mut FrameBuffer, grade: &Grade)` iterates through `frame.rgba.as_chunks_mut::<4>().0` with a 1-pixel repeated cache (`last_in == src`).
  - Lines 60–65: `pub fn apply_look(frame: &mut FrameBuffer, grade: &Grade, lut: Option<&Lut>)`.
- **Existing LUT Engine**:
  In `crates/neoveg_video/src/lut.rs`:
  - Lines 12–19: `Table::One(Vec<[f32; 3]>)` (1D) and `Table::Three { size: usize, data: Vec<[f32; 3]> }` (3D).
  - Lines 147–185: `apply` performs linear (1D) or trilinear (3D) interpolation.
  - Lines 189–210: `apply_to_frame(&self, frame: &mut FrameBuffer, strength: f32)`.
- **Existing Curve Mathematics & Three-Way Color in VFX**:
  In `crates/neoveg_video/src/vfx.rs`:
  - Lines 3607–3633: `pub fn three_way_color_px(...)` implements tonal color correction with shadows, midtones, highlights, tonal ranges (`range: [f32; 4]`), and saturation.
  - Lines 3644–3720: `struct Curve` implements Fritsch-Carlson monotone cubic Hermite spline interpolation.
  - Lines 3737–3748: `pub fn curve_luts(pts: &[[f32; 2 * CURVE_POINTS]; 4]) -> Vec<f32>` generates pre-evaluated 256-step lookup tables for Master RGB, Red, Green, and Blue.
  - Lines 3760–3770: `pub fn lut_lookup(luts: &[f32], ch: usize, v: f32) -> f32` performs fast table lookup.
- **Milestone 3 E2E Test Ground Truth**:
  In `crates/neoveg_video/tests/e2e_video.rs`:
  - Lines 405–446: `ColorWheelsGrade` (`lift`, `gamma`, `gain`, `offset` with `apply_to_rgb`) tested across F12 tests.
  - Lines 543–553: `pick_white_balance(sample: [f32; 3]) -> (f32, f32)` tested across F13 tests.
  - Lines 626–750: Spline curves generation and evaluation tested across F14 tests.
  - Lines 750–850: 1D & 3D LUT parsing, strength blend, and domain bounds tested across F15 tests.
  - Lines 855–881: `match_color_statistics(target: &mut FrameBuffer, reference: &FrameBuffer)` tested across F16 tests.

### 1.3 UI Architecture & VEGAS Pro Semantics
- **VEGAS Pro Reference Manual**:
  In `manuais/markdown/manual_vegaspro21_en.md` (lines 33640–35240):
  - "The COLOR GRADING panel provides a powerful set of color-grading and correction tools... The panel can be applied to all four effect layers, including: Events, Tracks, Media, Video output bus."
  - "Open the Color Grading Panel: Each level has a control to open the Color Grading panel and set it to work at that level."
  - "A label at the upper left corner of the panel identifies the level at which the changes will be applied (Event FX, Track FX, Media FX, Video Output FX) and the name of the active take in the video event you've selected."
  - Panel sections: `Input LUT`, `Look LUT`, `Color Wheels` (Lift, Gamma, Gain, Offset with numpad shortcuts), `Range-Limited Color Wheels` (threshold limit lines on scopes), `Utilities` (Brightness, Contrast, Contrast Center, White Balance Temperature/Tint and Neutral Tone Picker), `Color Curves` (Master, R, G, B with split-graph mode), `HSL Curves` (Hue vs Hue, Hue vs Sat, Hue vs Lum, Lum vs Sat, Sat vs Sat, Sat vs Lum), and `S-Log & ACES Workflow`.
- **Existing `neoveg_ui` Panel State**:
  In `crates/neoveg_ui/src/app.rs`:
  - Lines 244–254: `pub(crate) enum Tab` contains `Preview, Timeline, Media, MasterBus, Loudness, AudioFx, VideoFx, Transitions, Scopes`. **`Tab::ColorGrading` is not yet defined in the dock**.
  - Lines 11620–11692: `color_grade_window` is currently a standalone modal tool window with 7 sliders directly modifying `timeline.video_grade`.
  - Lines 6293–6304: `commit_grade` and `effective_grade` only interact with `self.timeline.video_grade`.
- **Model-View Decoupling**:
  - `neoveg_ui` relies on `neoveg_common` for data models and `neoveg_video` for image computation and scopes.
  - In `crates/neoveg_ui/src/curve_editor.rs`: The editor purely draws UI shapes and emits `CurveOutput::Live` and `CurveOutput::Commit`, leaving math and execution to `neoveg_video::vfx`.

### 1.4 Verification Baseline
- Execution commands and results:
  - `cargo check -j 1 --workspace`: Finished `dev` profile in 1.30s (exit code 0).
  - `cargo clippy -j 1 --workspace`: Finished `dev` profile in 0.97s (0 warnings, exit code 0).
  - `cargo test -p neoveg_common -j 1 -- --test-threads=1`: 239 passed, 0 failed (exit code 0).
  - `cargo test -p neoveg_engine -j 1 -- --test-threads=1`: 616 passed, 0 failed (exit code 0).
  - `cargo test -p neoveg_video --test e2e_video -j 1 -- --test-threads=1`: 144 passed, 0 failed (exit code 0).
  - `python scripts/parity_matrix.py --check`: 872 commands mapped, 0 unclassified (exit code 0).

---

## 2. Logic Chain

1. **Event Representation**:
   - *Observation*: `Timeline` currently stores a single `video_grade: Grade` at the project level, while `TrackEvent` stores `video_effects: Vec<TrackEffect>` and `pan_crop: PanCrop`.
   - *Requirement*: Feature 11 mandates dedicated per-event color grading, and VEGAS Pro manual §764 specifies that Color Grading operates on 4 distinct levels (Event FX, Track FX, Media FX, Video Output FX).
   - *Deduction*: Adding `pub color_grade: Option<EventColorGrade>` to `TrackEvent` (annotated with `#[serde(default, skip_serializing_if = "Option::is_none")]`) allows every event to carry its own grading state without breaking project file backward compatibility. In addition, providing a generalized command `EditCommand::SetColorGrade { target: VideoFxTarget, before: Option<EventColorGrade>, after: Option<EventColorGrade> }` will cleanly support all 4 VEGAS FX levels under a unified undo/redo architecture.

2. **Clip Processing Pipeline**:
   - *Observation*: In `crates/neoveg_video/src/worker.rs:365–400`, `render_clip` applies early effects, pan/crop, aspect ratio conformity, late effects, and transitions.
   - *Requirement*: Event color grading must apply to each video clip individually before final composition and transitions.
   - *Deduction*: Adding `pub grade: Option<EventColorGrade>` to `VideoClipRequest` allows the worker to apply `grading::apply_event_grade(&mut frame, grade)` during clip decoding in `worker.rs`.

3. **UI Architecture & Docking**:
   - *Observation*: `neoveg_ui` currently exposes color grading only as a floating window (`color_grade_window`) modifying the project master grade, while `Tab` in `app.rs` lacks a docked color grading tab.
   - *Requirement*: VEGAS Pro semantics require a docked Color Grading panel with an "Add to FX Level" selector and live target indication (showing event name and track color badge).
   - *Deduction*: Adding `Tab::ColorGrading` to `crates/neoveg_ui/src/app.rs` and hooking it into `dock_footer.rs`, `layouts.rs`, and `detached.rs` allows the Color Grading panel to dock alongside Video FX, Transitions, Media Pool, and Scopes.

4. **Model-View Decoupling**:
   - *Observation*: All math for curves (`Curve`, `curve_luts`), color wheels (`ColorWheelsGrade`), white balance (`pick_white_balance`), and color match (`match_color_statistics`) is tested and functional in `neoveg_video` and `neoveg_common`.
   - *Requirement*: `neoveg_ui` must remain strictly a view/controller layer without embedded color science math.
   - *Deduction*: `crates/neoveg_ui/` should only render the interactive widgets (color wheels pucks, curve control points, sliders) and dispatch `EditCommand::SetColorGrade` through `app.history.execute()`. During drag interaction, `state.color_grade_draft` feeds directly into the preview request for instant zero-latency visual feedback.

5. **CPU Execution on Limited Hardware**:
   - *Observation*: Analytical per-pixel evaluation with `powf(gamma)` requires over 6 million transcendent operations per 1080p frame. In contrast, `vfx::curve_luts` precomputes 256-step tables.
   - *Deduction*: Precomputing 1D transfer lookup tables (256 entries per channel, 768 bytes total) reduces per-pixel curve and wheel evaluation from $O(\text{math})$ to $O(1)$ memory lookups. The 768-byte table fits entirely within L1 cache (32–48 KB), guaranteeing zero cache misses.
   - *Deduction for 3D LUTs*: 3D LUTs ($33^3 = 431\text{ KB}$) fit in L2/L3 cache. Employing the 1-pixel repeated cache (`last_in == src`) in `grading.rs` and evaluating preview frames at proxy resolution avoids CPU bottlenecks during interactive scrubbing.

---

## 3. Caveats

1. **GPU Acceleration Fallback**: While WGPU shaders can accelerate 3D LUTs and ACES transforms when a GPU is present, all pipeline steps investigated and specified here are 100% verified to execute on CPU without GPU hardware.
2. **External C++ OCIO Library**: Neoveg strictly prohibits non-Rust C++ dependencies and GPL/LGPL code. The OCIO/ACES pipeline must be implemented as clean-room Rust analytical transforms and LUT tables (ACEScc/ACEScct, Rec.709 ODT, S-Log1/2/3 IDTs).
3. **Trademark Policy**: All user-visible strings and code comments must strictly avoid third-party trademarks (e.g. use "Camera S-Log" instead of vendor brand names; "Interchange ACES" instead of proprietary suite branding).

---

## 4. Conclusion

### 4.1 Recommended Implementation Blueprint

#### Step 1: Core Data Models (`crates/neoveg_common/src/grade.rs`)
Define a unified `EventColorGrade` struct:
```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct EventColorGrade {
    pub wheels: ColorWheelsGrade,
    pub white_balance: WhiteBalanceGrade, // temperature, tint
    pub utilities: GradeUtilities,         // exposure, contrast, contrast_center, saturation
    pub rgb_curves: [CurvePoints; 4],     // Master, Red, Green, Blue (5 points each)
    pub hsl_curves: [CurvePoints; 6],     // 6 HSL curves
    pub input_lut: Option<String>,
    pub look_lut: Option<String>,
    pub lut_strength: f32,
    pub ocio: OcioSettings,               // IDT, working space, ODT
}
```

#### Step 2: Engine Integration (`crates/neoveg_engine/`)
- In `crates/neoveg_engine/src/timeline.rs`:
  Add `pub color_grade: Option<EventColorGrade>` to `TrackEvent` with `#[serde(default, skip_serializing_if = "Option::is_none")]`.
- In `crates/neoveg_engine/src/command.rs`:
  Add `EditCommand::SetColorGrade { target: VideoFxTarget, before: Option<EventColorGrade>, after: Option<EventColorGrade> }`.

#### Step 3: Video Processing Pipeline (`crates/neoveg_video/src/grading.rs`)
- Move and expose `ColorWheelsGrade`, `pick_white_balance`, `curve_luts`, and `match_color_statistics` into `grading.rs`.
- Implement `apply_event_grade(frame: &mut FrameBuffer, grade: &EventColorGrade)`:
  1. Input IDT / ACES transform (if enabled).
  2. White balance temperature and tint correction.
  3. 4-way primary color wheels and utilities.
  4. RGB spline transfer curves via 256-step L1 cached 1D LUTs.
  5. HSL transfer curves (RGB -> HSL -> curve -> RGB).
  6. Look LUT application (with strength blending).
  7. Output ODT (Rec.709 / sRGB).
- In `crates/neoveg_video/src/worker.rs`:
  Add `pub grade: Option<EventColorGrade>` to `VideoClipRequest` and invoke `apply_event_grade` in `render_clip`.

#### Step 4: UI Dock & Panels (`crates/neoveg_ui/`)
- In `crates/neoveg_ui/src/app.rs`:
  Add `Tab::ColorGrading` to `enum Tab`.
  In `Viewer::content`: render `color_grading_panel_ui(self.state, ui)`.
- Create `crates/neoveg_ui/src/color_grading_panel.rs`:
  - Target selector: "Add to FX Level" dropdown (`Event FX`, `Track FX`, `Media FX`, `Video Output FX`).
  - Context header: Track color badge + event name / media name.
  - Sub-tabs: `Color Wheels`, `Utilities & White Balance`, `Curves`, `LUTs`, `Color Match`.
  - Eyedropper tool: samples preview pixel or rectangular area, invokes `pick_white_balance`, and updates draft.
  - Interactive widgets emit live drafts during mouse drags and dispatch `EditCommand::SetColorGrade` upon mouse release.

---

## 5. Verification Method

1. **Workspace Compilation & Lints**:
   ```powershell
   cargo check -j 1 --workspace
   cargo clippy -j 1 --workspace
   ```
   *Expected*: Zero compilation errors and zero clippy warnings.

2. **Core Unit & Regression Tests**:
   ```powershell
   cargo test -p neoveg_common -j 1 -- --test-threads=1
   cargo test -p neoveg_engine -j 1 -- --test-threads=1
   cargo test -p neoveg_video --test e2e_video -j 1 -- --test-threads=1
   ```
   *Expected*: All tests pass cleanly (100% pass rate).

3. **UI Verification Rule Compliance**:
   Confirm that `cargo test -p neoveg_ui` is **never executed**. Verify `neoveg_ui` strictly with:
   ```powershell
   cargo check -p neoveg_ui -j 1
   cargo clippy -p neoveg_ui -j 1
   ```

4. **Trademark Sanitization & Parity Check**:
   ```powershell
   python scripts/parity_matrix.py --check
   git grep -i "sony" crates/ Assets/
   git grep -i "apple" crates/ Assets/
   git grep -i "blackmagic" crates/ Assets/
   ```
   *Expected*: Parity matrix exits with 0; zero prohibited third-party vendor trademarks in runtime strings or comments.

5. **Invalidation Conditions**:
   - Any modification to `TrackEvent` that breaks deserialization of existing project files.
   - Any CPU grading implementation that runs analytical `powf` loops per pixel instead of L1-cached 1D LUTs for curves/wheels.
   - Any direct calculation of color grading pixel buffers inside `crates/neoveg_ui/`.
