# Handoff Report — Milestone 3 (Color Wheels, Curves & White Balance) Review

**Agent**: `reviewer_m3_1`  
**Roles**: `reviewer`, `critic`  
**Milestone**: Milestone 3 (Features 11–16)  
**Branch**: `preview/version-2.1.0`  
**Recipient**: `c220c328-9551-48e1-a35b-23e05589d782` (parent orchestrator)  
**Verdict**: **APPROVE**  

---

## 1. Observation

### 1.1 Direct Source Code Observations
- **`crates/neoveg_common/src/grade.rs`**:
  - Lines 105–115: `ColorWheelChannel` enum defined with 7 channels (`Red`, `Green`, `Blue`, `Cyan`, `Magenta`, `Yellow`, `Master`).
  - Lines 118–201: `ColorWheelVector` implements polar conversions (`from_polar`, `to_polar`) with $r = \text{sat} \cdot \cos\theta, y = \text{sat} \cdot \sin\theta$ and angle normalization to $[0.0..360.0)^\circ$.
  - Lines 178–201: Zero-sum chromatic projections implemented via `from_rgb_sliders` and `to_rgb_sliders` ensuring $\Delta r + \Delta g + \Delta b = 0$.
  - Lines 204–227: Numpad channel adjustments matching hexagonal color wheel geometry (Red at $0^\circ$, Yellow at $60^\circ$, Green at $120^\circ$, Cyan at $180^\circ$, Blue at $240^\circ$, Magenta at $300^\circ$).
  - Lines 313–350: Primary 4-way grading (`ColorWheelsGrade::apply_to_rgb`) implementing offset (additive global pedestal), gain (multiplicative scale), lift (shadow pedestal tapering linearly to 0 at 1.0), and gamma (midtones power function).
  - Lines 539–573: `EventColorGrade` container integrating color wheels, white balance, utility controls, RGB curves, HSL curves, LUT references, and OCIO configuration.
  
- **`crates/neoveg_video/src/grading.rs`**:
  - Lines 24–99: `ColorWheelsExt` implemented for `ColorWheelsGrade`, featuring `apply_to_frame` and `apply_range_limited` with $C^1$ smoothstep ramps ($3t^2 - 2t^3$) between shadows ($\le \text{low\_thresh}$), highlights ($\ge \text{high\_thresh}$), and midtones.
  - Lines 120–191: Neutral tone sampling functions `pick_white_balance`, `pick_white_balance_from_region`, `pick_black_point`, and `pick_white_point`. Dark pixel protection explicitly enforces `if y <= 1e-4 || !y.is_finite() { return (0.0, 0.0); }`.
  - Lines 197–316: `SplineCurve` implements the Fritsch-Carlson monotone cubic Hermite algorithm with weighted harmonic interior slopes (Brodlie/Butland formula) and bounded end slopes, guaranteeing strict monotonicity without overshoot.
  - Lines 319–352: `RgbCurves` generates 256-step lookup tables (`generate_luts_256`) occupying 768 bytes, completely resident in L1 cache, enabling $O(1)$ lookup per pixel during frame processing.
  - Lines 354–431, 479–520: All 6 secondary HSL curves implemented (`HueVsHue`, `HueVsSat`, `HueVsLum`, `LumVsSat`, `SatVsSat`, `SatVsLum`). `HueVsHue` strictly implements periodic angular wrapping via `rem_euclid(1.0)` mapping $0.0 \equiv 1.0$ ($0^\circ \equiv 360^\circ$).
  - Lines 527–594: `ColorMatcher` trait and `StatisticalColorMatcher` compute channel mean and standard deviation with `.max(1e-6)` numerical variance guards and bounded gain scaling.

- **`crates/neoveg_video/src/ocio.rs`**:
  - Lines 17–38, 42–53: `AcesSpace` and `ViewTransform` enums defined without third-party trademark names.
  - Lines 59–99: Analytical 3x3 gamut conversion matrices between AP0, AP1, Rec.709, Camera S-Gamut, and Camera S-Gamut3.Cine.
  - Lines 114–250: Forward and inverse analytical transfer functions for Camera S-Log1, S-Log2, S-Log3, ACEScc, ACEScct, Rec.709, and sRGB.
  - Lines 254–262: Narkowicz analytical ACES RRT + ODT tone mapping operator.
  - Lines 270–468: `AcesTransform` supporting per-pixel conversion, frame transformation, and 3D `.cube` LUT generation (`generate_3d_lut`).

- **`crates/neoveg_engine/src/timeline.rs` & `command.rs`**:
  - `TrackEvent` defines `pub color_grade: Option<EventColorGrade>` with `#[serde(default, skip_serializing_if = "Option::is_none")]` preserving full backward compatibility.
  - `EditCommand::SetEventColorGrade` implemented with complete `execute` and `undo` support.
  - `EditCommand::SplitEventAtPlayhead` clones `color_grade` to right event split.

### 1.2 Verbatim Command Execution Outputs
1. `cargo check --workspace -j 1`:
   ```
   Finished `dev` profile [unoptimized] target(s) in 1.12s
   Exit code: 0
   ```
2. `cargo clippy --workspace -j 1`:
   ```
   Finished `dev` profile [unoptimized] target(s) in 0.94s
   Exit code: 0 (0 warnings, 0 errors)
   ```
3. `cargo test -p neoveg_common -j 1 -- --test-threads=1`:
   ```
   test result: ok. 163 passed; 0 failed; 0 ignored (unit)
   test result: ok. 80 passed; 0 failed; 0 ignored (integration)
   Total: 243 passed, 0 failed.
   Exit code: 0
   ```
4. `cargo test -p neoveg_engine -j 1 -- --test-threads=1`:
   ```
   test result: ok. 428 passed; 0 failed; 0 ignored (unit)
   test result: ok. 140 passed; 0 failed; 0 ignored (integration)
   test result: ok. 48 passed; 0 failed; 0 ignored (scenarios)
   Total: 616 passed, 0 failed.
   Exit code: 0
   ```
5. `cargo test -p neoveg_video --lib -j 1 -- --test-threads=1`:
   ```
   test result: ok. 194 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 32.28s
   Exit code: 0
   ```
6. `cargo test -p neoveg_video --test e2e_video -j 1 -- --test-threads=1`:
   ```
   test result: ok. 157 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.56s
   Exit code: 0
   ```
7. `cargo check -p neoveg_ui -j 1`:
   ```
   Finished `dev` profile [unoptimized] target(s) in 0.87s
   Exit code: 0
   ```
8. `cargo clippy -p neoveg_ui -j 1`:
   ```
   Finished `dev` profile [unoptimized] target(s) in 0.92s
   Exit code: 0 (0 warnings, 0 errors)
   ```
9. `python scripts/parity_matrix.py --check`:
   ```
   872 comandos da varredura; 0 sem linha própria (a classificar)
   Exit code: 0
   ```
10. Trademark regex audit (`git grep -i -E "sony|davinci|panasonic|canon|adobe|vegas"` on M3 files):
   ```
   Exit code: 1 (0 matches found)
   ```

---

## 2. Logic Chain

1. **Integrity Audit**:
   - Every mathematical function was inspected for hardcoded return values, facade stubs, or copy-pasted external code. None were found.
   - Vector conversions, spline derivations, analytical logarithmic curves, and matrix transforms compute actual floating-point mathematics from first principles.
   - Zero third-party trademarks exist in code, comments, or UI strings.

2. **Mathematical Rigor**:
   - Polar $\leftrightarrow$ Cartesian round-trips correctly clamp radius, handle origin atan2 singularity, and map negative angles to $[0..360)^\circ$.
   - The zero-sum projection $(r_0 + g_0 + b_0 = 0)$ is preserved analytically: $g = -0.5x + \frac{\sqrt{3}}{2}y$ and $b = -0.5x - \frac{\sqrt{3}}{2}y$ sum with $r = x$ to zero identically.
   - Fritsch-Carlson cubic Hermite slopes guarantee monotonicity across arbitrary non-uniform control points.
   - `HueVsHue` modulo wrapping via `rem_euclid(1.0)` prevents discontinuity when grading red hues around $0^\circ / 360^\circ$.
   - Neutral tone picker avoids division-by-zero on black pixels through the $Y \le 10^{-4}$ threshold.

3. **Performance & Hardware Constraints**:
   - `RgbCurves::generate_luts_256` caches curve evaluations into three 256-byte tables (768 bytes total), which reside in L1 cache and avoid per-pixel spline evaluation on 4K frames.
   - All Cargo commands execute strictly sequentially with `-j 1` and `--test-threads=1`, respecting the 8 GB RAM budget without process contention.
   - `neoveg_ui` was audited strictly with `cargo check` and `cargo clippy`, avoiding any headless GUI test execution.

---

## 3. Caveats

- **SIMD / GPU Acceleration**: Current transformations execute on CPU in pure Rust. While performant and deterministic, GPU compute shaders (WGSL/Vulkan) can consume the exported 3D LUT cubes and matrix coefficients for live playback on 8K timelines.
- **External `.ocio` Config Parser**: The current OCIO implementation implements standard ACES spaces analytically rather than parsing external `.ocio` configuration files.

---

## 4. Conclusion

Milestone 3 (Features 11–16) is fully and authentically implemented according to specification. All automated checks, unit tests, integration tests, clippy lints, parity matrix checks, and trademark audits pass cleanly with 0 errors and 0 warnings.

**Verdict**: **APPROVE**

---

## 5. Verification Method

Execute the following commands sequentially in PowerShell from the repository root:

```powershell
# 1. Compilation and Linting
cargo check --workspace -j 1
cargo clippy --workspace -j 1

# 2. Common crate tests (vector math, polar conversions, serde)
cargo test -p neoveg_common -j 1 -- --test-threads=1

# 3. Engine crate tests (timeline commands, undo/redo, split event)
cargo test -p neoveg_engine -j 1 -- --test-threads=1

# 4. Video crate tests (grading, splines, curves, OCIO, color match)
cargo test -p neoveg_video --lib -j 1 -- --test-threads=1
cargo test -p neoveg_video --test e2e_video -j 1 -- --test-threads=1

# 5. UI compilation check (do NOT run cargo test on neoveg_ui)
cargo check -p neoveg_ui -j 1
cargo clippy -p neoveg_ui -j 1

# 6. Parity matrix check
python scripts/parity_matrix.py --check
```

---

## Quality Review Report

### Findings

#### [Minor] Finding 1: SplineCurve NaN input handling
- **What**: Calling `SplineCurve::eval(f32::NAN)` can cause an underflow panic (`0usize - 1`) in debug builds.
- **Where**: `crates/neoveg_video/src/grading.rs`, line 294.
- **Why**: IEEE 754 comparisons with `NaN` evaluate to `false`, causing `partition_point` to return index 0, which then subtracts 1.
- **Suggestion**: Add `if !v.is_finite() { return 0.0; }` as an initial guard in `SplineCurve::eval`. (Non-blocking as normal frame pixel inputs $0..255$ are finite).

#### [Minor] Finding 2: from_rgb_sliders NaN input handling
- **What**: Passing `NaN` channels to `ColorWheelVector::from_rgb_sliders` preserves `NaN` in `x` and `y`.
- **Where**: `crates/neoveg_common/src/grade.rs`, lines 187–188.
- **Why**: `f32::clamp` does not replace `NaN` with neutral values.
- **Suggestion**: Use `if x.is_finite() { x.clamp(-1.0, 1.0) } else { 0.0 }` defensively in `from_rgb_sliders`.

#### [Minor] Finding 3: pick_white_balance_from_region boundary clamping
- **What**: If $rx \ge frame.width$, `x0 = rx.min(frame.width - 1)` samples the right border pixel rather than early-returning `(0.0, 0.0)`.
- **Where**: `crates/neoveg_video/src/grading.rs`, lines 140–143.
- **Why**: Coordinates outside the frame bounds should represent an invalid selection.
- **Suggestion**: Add `if rx >= frame.width || ry >= frame.height { return (0.0, 0.0); }` to early return checks.

### Verified Claims
- Claim: Polar $\leftrightarrow$ Cartesian round-trip maintains angle and magnitude within $10^{-4}$ tolerance $\rightarrow$ verified via `color_wheel_vector_polar_and_rgb_roundtrip` and `m3_f12_color_wheels_polar_conversion` $\rightarrow$ PASS.
- Claim: Zero-sum chromatic offsets sum to zero $\rightarrow$ verified via `m3_f12_color_wheels_rgb_sliders` $\rightarrow$ PASS.
- Claim: Spline curve strictly monotonic without overshoot $\rightarrow$ verified via `m3_f14_spline_curve_arbitrary_points` $\rightarrow$ PASS.
- Claim: HSL `HueVsHue` wraps smoothly across $360^\circ$ $\rightarrow$ verified via `m3_f14_hsl_curves_periodic_wrapping` $\rightarrow$ PASS.
- Claim: White balance neutral sampler protects against dark pixel division by zero $\rightarrow$ verified via `f13_t2_01_white_balance_pure_black_no_division_by_zero` $\rightarrow$ PASS.
- Claim: Backward compatibility of project serialization $\rightarrow$ verified via `test_project_serialization` and serde skip attributes $\rightarrow$ PASS.

---

## Adversarial Review Report

### Challenge Summary
**Overall risk assessment**: **LOW**

### Challenges

#### [Low] Challenge 1: Extreme Non-Finite Pixel Inputs to Spline Evaluation
- **Assumption challenged**: Control points and evaluated values will always be finite floats.
- **Attack scenario**: Upstream HDR pipeline passes `f32::NAN` to `SplineCurve::eval`.
- **Blast radius**: Debug build panic on `partition_point(...) - 1`.
- **Mitigation**: Add defensive guard `if !v.is_finite() { return 0.0; }`.

#### [Low] Challenge 2: Statistical Color Match on Zero Variance Targets
- **Assumption challenged**: Target frame has measurable color variance.
- **Attack scenario**: Matching a flat solid color frame ($std = 0$).
- **Blast radius**: Division by zero or NaN variance.
- **Result**: Successfully defended by the implementation via `.max(1e-4)` and `.max(1e-6)` guards in `tgt_std` and `sum_sq / n - mean^2`.
