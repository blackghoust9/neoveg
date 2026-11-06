# Milestone 3 Codebase Exploration & Implementation Blueprint: Color Grading & OCIO/ACES

**Author**: `explorer_m3_1` (Milestone 3 Explorer & Synthesizer)  
**Parent Conversation ID**: `c220c328-9551-48e1-a35b-23e05589d782`  
**Working Directory**: `C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\explorer_m3_1`  
**Date**: 2026-11-06  
**Scope**: Milestone 3 — Color Grading per Event, Primary 4-Way Color Wheels, White Balance Neutral Picker, RGB & 6 HSL Transfer Curves, OpenColorIO (OCIO) / ACES Pipeline, Color Match Across Clips (Features 11–16).

---

## 1. Observation

Direct inspections of the repository and reference manuals reveal the following baseline state and gaps across workspace crates:

### 1.1 Existing Types and Implementations
1. **`crates/neoveg_common/src/grade.rs`** (Lines 7–103):
   - Defines basic `Grade` struct:
     ```rust
     pub struct Grade {
         pub exposure: f32,    // -4.0..=4.0 EV
         pub temperature: f32, // -1.0..=1.0 (cool to warm)
         pub tint: f32,        // -1.0..=1.0 (green to magenta)
         pub lift: f32,        // -0.5..=0.5
         pub contrast: f32,    // -1.0..=1.0
         pub gamma: f32,       // 0.2..=3.0 (default 1.0)
         pub saturation: f32,  // -1.0..=1.0
     }
     ```
   - Provides methods: `is_neutral()`, `sanitized()`, `apply(&self, rgb: [f32; 3]) -> [f32; 3]`.
   - Used currently as project-wide grade in `Timeline.video_grade` (`neoveg_engine/src/timeline.rs:805`).

2. **`crates/neoveg_video/src/grading.rs`** (Lines 1–66):
   - Provides:
     - `pub fn apply_grade(frame: &mut FrameBuffer, grade: &Grade)`: per-pixel cache-accelerated RGBA8 application.
     - `pub fn fade_to_color(frame: &mut FrameBuffer, color: [u8; 3], amount: f32)`: linear blend toward color.
     - `pub fn apply_look(frame: &mut FrameBuffer, grade: &Grade, lut: Option<&Lut>)`: grade followed by LUT.

3. **`crates/neoveg_video/src/lut.rs`** (Lines 20–211):
   - Implements Adobe/Resolve `.cube` text format parser: 1D tables (`Vec<[f32; 3]>`) and 3D tables (`size³`, red varying fastest).
   - Provides `Lut::apply(&self, rgb: [f32; 3]) -> [f32; 3]` with linear (1D) or trilinear (3D) interpolation and `apply_to_frame(&self, frame: &mut FrameBuffer, strength: f32)`.

4. **`crates/neoveg_video/src/vfx.rs`** (Lines 72–1550, 3163–3766):
   - Declares effect IDs: `HSL_ADJUST = "hsl_adjust"`, `COLOR_BALANCE = "color_balance"`, `THREE_WAY_COLOR = "three_way_color"`, `COLOR_CURVES = "color_curves"`.
   - Lines 3163–3197: `rgb_to_hsl(c: [f32; 3]) -> [f32; 3]` and `hsl_to_rgb(hsl: [f32; 3]) -> [f32; 3]`.
   - Lines 3607–3633: `pub fn three_way_color_px(c: [f32; 3], sh: [f32; 3], md: [f32; 3], hi: [f32; 3], range: [f32; 4], sat: f32) -> [f32; 3]`.
   - Lines 3644–3766: Private `struct Curve` implementing Fritsch-Carlson monotone cubic Hermite spline interpolation over 5 fixed control points, plus `pub fn curve_luts(pts: &[[f32; 10]; 4]) -> Vec<f32>` and `pub fn lut_lookup(luts: &[f32], ch: usize, v: f32) -> f32`.

5. **`crates/neoveg_video/src/worker.rs`** (Lines 44, 57–86, 142–156, 363–378, 650, 1281):
   - `CompositeFrameRequest` has `pub grade: Grade` and `pub lut: Option<Arc<Lut>>` applied globally to the final composite.
   - `VideoClipRequest` represents a timeline event and holds `pub effects: Vec<VideoEffect>`, `pub media_look: MediaProps`, `pub transitions: Vec<TransitionRequest>`, but lacks a direct event-level `Grade` field.
   - `finish_clip` executes clip-level video effects before compositing.

6. **`crates/neoveg_video/tests/e2e_video.rs`** (Lines 398–881):
   - Existing Tier 1 & 2 tests for F11–F16 run and pass (`90 passed; 0 failed` in 0.05s), but critical types and algorithms are declared locally inside the test file rather than in the production crate:
     - Lines 399–446: Local `ColorWheelVector` and `ColorWheelsGrade` with `apply_to_rgb`.
     - Lines 543–553: Local `pick_white_balance(sample: [f32; 3]) -> (f32, f32)`.
     - Lines 855–881: Local `match_color_statistics(target: &mut FrameBuffer, reference: &FrameBuffer)`.
     - Lines 738–850: F15 tests only verify `.cube` LUT parsing; no ACES spaces or camera IDT transforms are tested.

7. **Interface Contract in `PROJECT.md`** (Lines 110–112):
   ```rust
   - `neoveg_video::grading`:
     - `pub struct ColorWheelsGrade { pub lift: ColorWheelVector, pub gamma: ColorWheelVector, pub gain: ColorWheelVector, pub offset: ColorWheelVector }`
     - `pub trait ColorMatcher: Send + Sync { fn analyze_and_match(&self, reference: &FrameBuffer, target: &FrameBuffer) -> ColorWheelsGrade; }`
   ```
   Neither `ColorWheelsGrade` nor `ColorMatcher` is currently exported in `neoveg_video::grading`.

8. **Authoritative References Consulted**:
   - `manuais/markdown/manual_vegaspro21_en.md` (Lines 769, 1149–1217, 22865–22950, 33630–35466): 4-layer grading hierarchy (Event, Track, Media, Output), 4 color wheels with luma sliders, numpad 10-key shortcuts, range-limited wheels, neutral tone picker, 6 HSL curve types, ACES 1.2/1.0/0.7 spaces, camera S-Log1/2/3 IDTs, and Color Match plug-in (`match_brightness` and `strength`).
   - `spec_miner_m3_2/handoff.md`: Full mathematical specifications, transformation matrices, and boundary conditions.

---

## 2. Logic Chain

From the observations, the requirements for Milestone 3 map directly into specific gaps in `neoveg_video` and `neoveg_common`:

### 2.1 Feature 11: Dedicated Per-Event Color Grading
- **Observation**: `Timeline.video_grade` and `CompositeFrameRequest.grade` are applied globally at the output stage (`worker.rs:1281`). Events (`TrackEvent` / `VideoClipRequest`) run effects via `effects: Vec<VideoEffect>`.
- **Reasoning**: To fulfill VEGAS Pro semantics ("Color grading applied to all four effect layers: Events, Tracks, Media, Video output bus"):
  1. Primary color grading must be capable of running per event on the decoded frame prior to compositing.
  2. In `neoveg_video::grading`, provide first-class grading evaluators `apply_color_wheels`, `apply_look`, and `apply_event_grade`.
  3. In `neoveg_video::vfx`, register a standard `"color_grading"` effect type (or ensure `"three_way_color"` and `"color_curves"` integrate with `ColorWheelsGrade`) so that an event's `video_effects` can carry full 4-way grading, curves, and ACES transforms across Event FX, Track FX, Media FX, and Output FX layers without breaking timeline serialization.

### 2.2 Feature 12: Primary 4-Way Color Wheels
- **Observation**: `ColorWheelsGrade` and `ColorWheelVector` currently exist only inside `crates/neoveg_video/tests/e2e_video.rs`.
- **Reasoning**:
  1. Move and expand `ColorWheelVector` and `ColorWheelsGrade` into `crates/neoveg_video/src/grading.rs`.
  2. Implement dual representations:
     - **Cartesian**: $(x, y) \in [-1.0, 1.0]^2$ with $x^2 + y^2 \le 1.0$, master luma $m \in [-1.0, 1.0]$.
     - **Polar (Wheel)**: Hue angle $\theta = \text{atan2}(y, x) \in [0, 360^\circ)$, Saturation radius $\rho = \sqrt{x^2 + y^2} \in [0.0, 1.0]$, master luma $m$.
     - **RGB Sliders**: Direct offsets $(\Delta R, \Delta G, \Delta B)$ preserving luminance plus master luma:
       $$\begin{bmatrix} \Delta R \\ \Delta G \\ \Delta B \end{bmatrix} = \begin{bmatrix} 1.0 & 0.0 \\ -0.5 & \frac{\sqrt{3}}{2} \\ -0.5 & -\frac{\sqrt{3}}{2} \end{bmatrix} \begin{bmatrix} x \\ y \end{bmatrix}$$
  3. Formula for 4-way pixel evaluation:
     - **Offset** (additive): $out_c = in_c + \text{offset.master} + \Delta\text{offset}_c$.
     - **Gain** (multiplicative): $out_c = out_c \cdot (1.0 + \text{gain.master} + \Delta\text{gain}_c)$.
     - **Lift** (pedestal/shadows): $out_c = out_c + (\text{lift.master} + \Delta\text{lift}_c) \cdot (1.0 - \text{clamp}(out_c, 0.0, 1.0))$.
     - **Gamma** (midtones power): $out_c = \text{clamp}(out_c, 0.0, 10.0)^{\gamma_c}$, where $\gamma_c = \text{clamp}(1.0 - \text{gamma.master} - \Delta\text{gamma}_c, 0.2, 5.0)$.
  4. Range-Limited (RL) Color Wheels:
     - Low threshold $T_{low} \in [0.0, 0.5]$ (default 0.25) and High threshold $T_{high} \in [0.5, 1.0]$ (default 0.75).
     - Cubic smoothstep blending isolating Lift to $Y \le T_{low}$, Gain to $Y \ge T_{high}$, and Gamma to $T_{low} < Y < T_{high}$.
  5. Keyboard Numpad Shortcuts:
     - Provide helper methods `adjust_channel(&mut self, wheel_idx, channel, delta)` and `reset_wheel(&mut self, wheel_idx)` for `Num 0..9, +, -, ., Enter`.

### 2.3 Feature 13: White Balance Neutral Tone Picker
- **Observation**: `pick_white_balance` is currently a private test helper in `e2e_video.rs`.
- **Reasoning**:
  1. Promote `pick_white_balance(sample: [f32; 3]) -> (f32, f32)` to a public function in `neoveg_video::grading`.
  2. Implement `pick_white_balance_from_region(frame: &FrameBuffer, rect: [u32; 4]) -> (f32, f32)` to average rectangular sampling regions and eliminate single-pixel sensor noise.
  3. Guard against dark pixels: if luminance $Y \le 10^{-4}$, return neutral `(0.0, 0.0)`.
  4. Temperature formula: $T = -((R - B) / Y).clamp(-1.0, 1.0)$ (cooling warm red cast, warming cool blue cast).
  5. Tint formula: $t = -((G - (R + B) * 0.5) / Y).clamp(-1.0, 1.0)$ (subtracting green with magenta, correcting magenta with green).
  6. Provide `pick_black_point` and `pick_white_point` to calibrate Lift and Gain wheels.

### 2.4 Feature 14: RGB & 6 HSL Transfer Curves
- **Observation**: `vfx.rs` has a private `struct Curve` restricted to 5 fixed points for RGB master/channels. No HSL curves exist in `neoveg_video`.
- **Reasoning**:
  1. Create a public, generalized `SplineCurve` struct in `neoveg_video::grading` (or a dedicated `curves` module) using the Fritsch-Carlson monotone cubic Hermite algorithm:
     - Guarantees monotonicity: dragging points never creates unphysical waves or overshoot.
     - Accepts arbitrary number of control points ($N \ge 2$).
     - Clamps boundary extrapolation to $[0.0, 1.0]$.
  2. Implement `RgbCurves`: Master, Red, Green, Blue splines with 256-step LUT compilation.
  3. Implement the 6 HSL Curves (`HslCurves`):
     - `Hue vs Hue`: Crucial requirement: **periodic boundary wrapping** ($0^\circ \equiv 360^\circ$, red continuous loop). The spline evaluation must wrap around $360^\circ$.
     - `Hue vs Saturation`: Multiplicative factor on saturation centered at 1.0.
     - `Hue vs Luminance`: Lightness offset based on hue.
     - `Luminance vs Saturation`: Saturation modulation based on luma (e.g. shadow desaturation).
     - `Saturation vs Saturation`: Selective saturation curve (boosting low-saturation pastels).
     - `Saturation vs Luminance`: Luma modulation based on saturation.
  4. Provide `apply_rgb_curves(&mut FrameBuffer, &RgbCurves)` and `apply_hsl_curves(&mut FrameBuffer, &HslCurves)` with optimized 256/360-bin LUT precalculation.

### 2.5 Feature 15: OpenColorIO (OCIO) & ACES Pipeline
- **Observation**: Current LUT support in `neoveg_video::lut` is limited to `.cube` file interpolation. There are no built-in ACES color spaces, camera IDTs, or scene-linear transforms.
- **Reasoning**:
  1. Add a clean-room, pure-CPU module `crates/neoveg_video/src/ocio.rs` (re-exported in `neoveg_video::grading` and `lib.rs`).
  2. Color Spaces (`AcesSpace` enum):
     - `Aces2065_1`: AP0 linear (SMPTE ST 2065-1).
     - `AcesCg`: AP1 linear (CGI and VFX compositing).
     - `AcesCc`: AP1 logarithmic (color grading).
     - `AcesCct`: AP1 logarithmic with linear toe (DPX/Cineon emulation).
     - `AcesProxy`: AP1 integer-log preview.
     - Camera IDTs: `SLog1`, `SLog2`, `SLog3` (Sony S-Gamut, S-Gamut3, S-Gamut3.Cine), `Rec709`, `Srgb`.
  3. Exact 3x3 Primary Matrices:
     - $\mathbf{M}_{AP0 \to AP1}$ and $\mathbf{M}_{AP1 \to AP0}$.
     - $\mathbf{M}_{SGamut \to AP0}$, $\mathbf{M}_{SGamut3 \to AP0}$, $\mathbf{M}_{SGamut3Cine \to AP0}$.
     - $\mathbf{M}_{AP1 \to Rec709(D65)}$.
  4. Transfer Functions:
     - Camera S-Log1, S-Log2, and S-Log3 forward and inverse equations.
     - ACEScc: $\log_2$ curve with linear extension for $x \le 0$.
     - ACEScct: $\log_2$ curve with smooth linear toe below $x_{brk} = 0.0078125$.
     - Rec.709 OETF and sRGB EOTF equations.
  5. Pure CPU Analytical RRT + ODT Fit:
     - Narkowicz / Stephen Hill ACES tone curve fit:
       $$f_{tonemap}(x) = \text{clamp}\left(\frac{x \cdot (2.51 \cdot x + 0.03)}{x \cdot (2.43 \cdot x + 0.59) + 0.14}, 0.0, 1.0\right)$$
     - Allows 100% deterministic, high-performance CPU frame rendering in headless CI environments without requiring external C++ OCIO libraries or GPU shaders.
  6. LUT Generation:
     - Method `generate_3d_lut(&self, size: usize) -> Lut` to bake any ACES transform chain into a `neoveg_video::lut::Lut`.

### 2.6 Feature 16: Color Match Across Clips
- **Observation**: `PROJECT.md` line 112 specifies `pub trait ColorMatcher: Send + Sync { fn analyze_and_match(&self, reference: &FrameBuffer, target: &FrameBuffer) -> ColorWheelsGrade; }`, but only a naive local test function exists in `e2e_video.rs`.
- **Reasoning**:
  1. Implement `pub trait ColorMatcher: Send + Sync` in `crates/neoveg_video/src/grading.rs`.
  2. Implement `StatisticalColorMatcher` which analyzes reference and target:
     - Computes mean and standard deviation for R, G, B and Luminance.
     - Computes corresponding `ColorWheelsGrade`:
       - Lift (shadow pedestal): derived from baseline shift and bottom quartile difference.
       - Gain (highlight multiplier): derived from upper dynamic range ratio.
       - Gamma (midtones): derived from variance / contrast ratio.
       - Offset: global luminance calibration.
  3. Implement `pub fn match_color(target: &mut FrameBuffer, reference: &FrameBuffer, match_brightness: bool, strength: f32)`:
     - Direct image transfer using statistical normalization.
     - `match_brightness == true`: transfers both luminance distribution and color balance.
     - `match_brightness == false`: matches chromatic distribution while strictly renormalizing output pixels to the target frame's original luminance.
     - `strength`: linear mix between original and matched frame.
     - Preserves target alpha channel.
     - Guards against empty frames, zero-variance frames, and extreme values.

---

## 3. Caveats

1. **Read-Only Investigation**: As an explorer agent, no production code has been modified in `crates/neoveg_video/` or `crates/neoveg_common/`. All implementation recommendations are delivered in this handoff.
2. **CPU Execution Budget**: All proposed algorithms are strictly CPU-friendly, avoiding heavy allocations in per-pixel loops. 1D LUTs (256/360 bins) are recommended for curve and transfer function evaluations on full frames.
3. **Trademark Compliance**: No third-party trademarks may appear in runtime UI strings or code identifiers. Use generic terms: `"Camera S-Log"`, `"Camera S-Gamut"`, `"Interchange ACES"`.
4. **RAM Constraints**: Testing must remain sequential with `-j 1` and `--test-threads=1`. `neoveg_ui` tests must never be run.

---

## 4. Conclusion & Clean-Room Implementation Recommendations

The codebase contains solid fundamentals (`Grade`, `Lut`, `Scopes`, basic VFX curves) but Milestone 3 requirements are currently incomplete in the production library, with several key contracts existing only as ad-hoc test code in `tests/e2e_video.rs`.

### Implementation Blueprint for the Worker

#### File: `crates/neoveg_video/src/grading.rs`
1. **Define Core Types**:
   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
   pub struct ColorWheelVector {
       pub x: f32,      // Chroma X [-1.0..=1.0]
       pub y: f32,      // Chroma Y [-1.0..=1.0]
       pub master: f32, // Luma [-1.0..=1.0]
   }

   #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
   pub struct ColorWheelsGrade {
       pub lift: ColorWheelVector,
       pub gamma: ColorWheelVector,
       pub gain: ColorWheelVector,
       pub offset: ColorWheelVector,
   }
   ```
2. **Implement Methods on `ColorWheelVector` & `ColorWheelsGrade`**:
   - `from_polar(hue_deg: f32, sat: f32, master: f32) -> Self`
   - `to_polar(&self) -> (f32, f32, f32)`
   - `from_rgb_sliders(r: f32, g: f32, b: f32, master: f32) -> Self`
   - `to_rgb_sliders(&self) -> (f32, f32, f32, f32)`
   - `default_neutral() -> Self`
   - `apply_to_rgb(&self, rgb: [f32; 3]) -> [f32; 3]`
   - `apply_to_frame(&self, frame: &mut FrameBuffer)`
   - `apply_range_limited(&self, frame: &mut FrameBuffer, low_thresh: f32, high_thresh: f32)`

3. **White Balance Picker**:
   - `pub fn pick_white_balance(sample: [f32; 3]) -> (f32, f32)`
   - `pub fn pick_white_balance_from_region(frame: &FrameBuffer, rect: [u32; 4]) -> (f32, f32)`
   - `pub fn pick_black_point(sample: [f32; 3]) -> ColorWheelVector`
   - `pub fn pick_white_point(sample: [f32; 3]) -> ColorWheelVector`

4. **Curves & 6 HSL Curves**:
   - `pub struct SplineCurve { points: Vec<(f32, f32)>, ... }` (Fritsch-Carlson monotone Hermite)
   - `pub struct RgbCurves { pub master: SplineCurve, pub red: SplineCurve, pub green: SplineCurve, pub blue: SplineCurve }`
   - `pub enum HslCurveType { HueVsHue, HueVsSat, HueVsLum, LumVsSat, SatVsSat, SatVsLum }`
   - `pub struct HslCurve { pub curve_type: HslCurveType, pub spline: SplineCurve }`
   - `pub struct HslCurves { pub curves: [HslCurve; 6] }`
   - `pub fn apply_rgb_curves(frame: &mut FrameBuffer, curves: &RgbCurves)`
   - `pub fn apply_hsl_curves(frame: &mut FrameBuffer, curves: &HslCurves)`

5. **Color Match Across Clips**:
   - `pub trait ColorMatcher: Send + Sync { fn analyze_and_match(&self, reference: &FrameBuffer, target: &FrameBuffer) -> ColorWheelsGrade; }`
   - `pub struct StatisticalColorMatcher;`
   - `pub fn match_color(target: &mut FrameBuffer, reference: &FrameBuffer, match_brightness: bool, strength: f32)`
   - `pub fn match_color_statistics(target: &mut FrameBuffer, reference: &FrameBuffer)` (for backwards compatibility with existing tests)

#### File: `crates/neoveg_video/src/ocio.rs` (New Module, Re-exported in `lib.rs` and `grading.rs`)
1. **Enums**:
   - `pub enum AcesSpace { Aces2065_1, AcesCg, AcesCc, AcesCct, AcesProxy, SLog1, SLog2, SLog3, Rec709, Srgb }`
   - `pub enum ViewTransform { AcesRrtRec709, AcesRrtSrgb, AcesRrtRec2020, AcesRrtHlg, Raw }`
2. **Matrices**:
   - Constants for $\mathbf{M}_{AP0 \leftrightarrow AP1}$, $\mathbf{M}_{AP1 \to Rec709}$, $\mathbf{M}_{SGamut \to AP0}$, etc.
3. **Transfer Functions**:
   - Forward and inverse for S-Log1, S-Log2, S-Log3, ACEScc, ACEScct, Rec.709, sRGB.
4. **Tone Mapping**:
   - Pure CPU analytical ACES RRT + ODT fit function.
5. **Pipeline Struct**:
   - `pub struct AcesTransform { pub input_space: AcesSpace, pub working_space: AcesSpace, pub view_transform: ViewTransform }`
   - `pub fn transform_rgb(&self, rgb: [f32; 3]) -> [f32; 3]`
   - `pub fn apply_to_frame(&self, frame: &mut FrameBuffer)`
   - `pub fn generate_3d_lut(&self, size: usize) -> Lut`

#### Integration with `neoveg_video::vfx` & `worker`
- Wire `ColorWheelsGrade` and `Curves` into `vfx.rs` so they can be executed per event inside `finish_clip` via standard `TrackEffect` entries, satisfying the per-event grading requirement.

---

## 5. Verification Method

To independently verify the exploration findings and subsequent implementation:

1. **Verify Existing Workspace Check & Clippy**:
   ```powershell
   cargo check --workspace -j 1
   cargo check -p neoveg_ui -j 1
   ```
   *Expected*: Passes with exit code 0.

2. **Run Existing M3 E2E Tests**:
   ```powershell
   cargo test -p neoveg_video --test e2e_video -j 1 -- --test-threads=1 f11
   cargo test -p neoveg_video --test e2e_video -j 1 -- --test-threads=1 f12
   cargo test -p neoveg_video --test e2e_video -j 1 -- --test-threads=1 f13
   cargo test -p neoveg_video --test e2e_video -j 1 -- --test-threads=1 f14
   cargo test -p neoveg_video --test e2e_video -j 1 -- --test-threads=1 f15
   cargo test -p neoveg_video --test e2e_video -j 1 -- --test-threads=1 f16
   ```
   *Expected*: All 60 feature tests pass.

3. **Verify Parity Matrix**:
   ```powershell
   python scripts/parity_matrix.py --check
   ```
   *Expected*: 0 unclassified commands.

4. **Invalidation Conditions**:
   - If any `cargo test -p neoveg_ui` is invoked (violates strict 8 GB RAM execution budget).
   - If proprietary third-party trademarks are added to code or UI strings.
   - If `ColorWheelsGrade` or `ColorMatcher` remains absent from `neoveg_video::grading` exports.
