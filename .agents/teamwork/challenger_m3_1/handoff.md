# Empirical Adversarial Challenge Report — Milestone 3 (Color Wheels, Splines & Curves)

**Agent**: `challenger_m3_1`  
**Role**: Empirical Challenger (`critic`, `specialist`)  
**Target Work Product**: Milestone 3 Color Wheels (`ColorWheelVector`, `ColorWheelsGrade`), `SplineCurve` monotone cubic Hermite interpolation, and 6 `HslCurves` implemented by `worker_m3_1`  
**Recipient**: `c220c328-9551-48e1-a35b-23e05589d782` (parent orchestrator)  
**Verdict**: **REQUEST_CHANGES**  

---

## 1. Observation

Adversarial stress harness implemented in:  
`crates/neoveg_video/tests/adversarial_m3_splines_wheels.rs` (15 empirical stress tests).

Executed under strict budget constraints:
`cargo test -p neoveg_video --test adversarial_m3_splines_wheels -j 1 -- --test-threads=1`

### Finding 1 (CRITICAL): Integer Underflow Panic in `SplineCurve::eval(f32::NAN)`
- **File**: `crates/neoveg_video/src/grading.rs:288-294`
- **Code**:
  ```rust
  288: if v <= self.points[0][0] {
  289:     return self.points[0][1];
  290: }
  291: if v >= self.points[n - 1][0] {
  292:     return self.points[n - 1][1];
  293: }
  294: let k = self.points.partition_point(|p| p[0] <= v) - 1;
  ```
- **Verbatim Error**:
  ```text
  thread 'adv_spline_boundary_values_and_extrapolation' (2916) panicked at crates\neoveg_video\src\grading.rs:294:17:
  attempt to subtract with overflow
  note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
  ```
- **Observed Mechanism**: When `v = f32::NAN`, both `v <= self.points[0][0]` and `v >= self.points[n - 1][0]` evaluate to `false`. Execution proceeds to line 294. In `self.points.partition_point(|p| p[0] <= v)`, `p[0] <= f32::NAN` is `false` for every element, returning index `0`. The subtraction `0usize - 1` underflows, triggering an immediate panic and crashing the thread/process.

### Finding 2 (HIGH): Frame Corruption to Pitch Black on NaN Range-Limited Thresholds
- **File**: `crates/neoveg_video/src/grading.rs:46-97`
- **Code**:
  ```rust
  46: let low = low_thresh.clamp(0.0, 0.5);
  47: let high = high_thresh.clamp(0.5, 1.0);
  ...
  57: let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
  ...
  76: x: self.lift.x * w_lift,
  ...
  96: px[0] = (out[0].clamp(0.0, 1.0) * 255.0).round() as u8;
  ```
- **Verbatim Test Result**:
  ```text
  assertion `left != right` failed: NaN thresholds caused all pixels to corrupt to solid black [0, 0, 0, 255]!
    left: [0, 0, 0, 255]
   right: [0, 0, 0, 255]
  ```
- **Observed Mechanism**: In Rust, `f32::NAN.clamp(lo, hi)` evaluates to `NaN`. When `low_thresh` or `high_thresh` is `NaN`, `b - a` is `NaN`, and `t` becomes `NaN`. Consequently, `w_lift` and `w_gain` become `NaN`. In `weighted`, `self.lift.x * w_lift` evaluates to `0.0 * NaN = NaN`. In `apply_to_rgb`, `out[0] += (lift_m + lr) * ...` accumulates `NaN`. Finally, `(NaN * 255.0).round() as u8` converts to `0`, causing all pixels in the entire video frame to collapse to solid pitch black `[0, 0, 0, 255]`.

### Finding 3 (MEDIUM): Contract Violation in `ColorWheelVector::to_polar()` Returning `360.0`
- **File**: `crates/neoveg_common/src/grade.rs:166-174`
- **Contract Specification**:
  ```rust
  /// Converts the vector to polar coordinates: `(hue_deg, sat, master)`.
  /// `hue_deg` is in `[0.0..360.0)`.
  pub fn to_polar(&self) -> (f32, f32, f32)
  ```
- **Code**:
  ```rust
  168: let sat = (self.x * self.x + self.y * self.y).sqrt().clamp(0.0, 1.0);
  169: let mut deg = self.y.atan2(self.x).to_degrees();
  170: if deg < 0.0 {
  171:     deg += 360.0;
  172: }
  173: (deg, sat, self.master)
  ```
- **Verbatim Test Result**:
  ```text
  thread 'adv_wheel_vector_extreme_angles' (20900) panicked at crates\neoveg_video\tests\adversarial_m3_splines_wheels.rs:37:9:
  Angle must be in [0.0..360.0), got 360 for input angle -360
  ```
- **Observed Mechanism**: For `hue_deg = -360.0`, floating-point inexactness in `to_radians()` gives $y = -6.5567 \times 10^{-8}$. Then `atan2(y, x).to_degrees()` evaluates to a tiny negative angle $\approx -0.000005^\circ$. In single-precision `f32`, `(-0.000005 + 360.0)` rounds up to exactly `360.0`. Because there is no check `if deg >= 360.0 { deg -= 360.0; }`, `to_polar()` returns `360.0`, directly violating the documented half-open interval contract `[0.0..360.0)`.

### Finding 4 (MEDIUM): Achromatic Discontinuity and Posterization in `HueVsLum`
- **File**: `crates/neoveg_video/src/grading.rs:440, 498-501`
- **Code**:
  ```rust
  // rgb_to_hsl (grading.rs:440)
  if d < 1e-6 {
      return [0.0, 0.0, l];
  }
  ...
  // apply_hsl_curves (grading.rs:498-501)
  HslCurveType::HueVsLum => {
      let delta = c.eval(hsl[0] / 360.0) - 0.5;
      hsl[2] = (hsl[2] + delta).clamp(0.0, 1.0);
  }
  ```
- **Verbatim Output**:
  ```text
  Neutral gray [128, 128, 128] after Red HueVsLum: [205, 205, 205, 255]
  Nearly neutral gray [128, 129, 128] after Red HueVsLum: [128, 129, 128, 255]
  ```
- **Observed Mechanism**: In `rgb_to_hsl`, when saturation is zero ($d < 10^{-6}$), hue is undefined and arbitrarily returned as `0.0` (Red). In `HueVsLum`, the luminance delta is computed from `hsl[0]` without scaling by saturation `hsl[1]`. Consequently, a pure neutral gray pixel `[128, 128, 128]` is treated as saturated Red, receiving the full Red luminance boost (jumping to `[205, 205, 205]`), while an adjacent near-gray pixel `[128, 129, 128]` (hue $\approx 120^\circ$ Green) receives 0 boost, creating a sudden 76-level luminance jump and severe visual posterization.

### Finding 5 (LOW): Non-finite Hue / RGB Sliders in `ColorWheelVector::from_polar` and `from_rgb_sliders`
- **File**: `crates/neoveg_common/src/grade.rs:154-163, 178-191`
- **Observed Mechanism**: `from_polar` sanitizes `sat` and `master` with `is_finite()`, but does NOT sanitize `hue_deg`. If `hue_deg` is `NaN`, `x` and `y` become `NaN`. Similarly, `from_rgb_sliders` sanitizes `master`, but does not sanitize `r, g, b` before calling `.clamp(-1.0, 1.0)` which returns `NaN`.

---

## 2. Logic Chain

1. **Panic Invariant**: In Rust production code, pure numerical grading functions must NEVER panic on any input, including `NaN`, $\pm\infty$, or out-of-range floats. `SplineCurve::eval(f32::NAN)` panics on underflow (`0usize - 1`), violating this invariant and creating a critical reliability flaw.
2. **Data Integrity Invariant**: Range-limited color wheel grading must degrade gracefully or no-op on non-finite thresholds. Instead, `apply_range_limited` corrupts the entire frame buffer into pitch black (`[0, 0, 0, 255]`).
3. **Contract Adherence**: `ColorWheelVector::to_polar()` explicitly documents `[0.0..360.0)`. Any downstream code using `deg` to index a 360-element histogram or lookup table without bounds modulo will panic or read out of bounds when `deg == 360.0`.
4. **Color Science Integrity**: Grayscale/achromatic pixels have undefined hue. In `HueVsLum`, unweighted hue evaluation creates extreme threshold discontinuities across neutral tones.
5. **Mitigations are straightforward**:
   - In `SplineCurve::eval`: add `if !v.is_finite() { return self.points.first().map_or(0.0, |p| p[1]); }`.
   - In `apply_range_limited`: guard `low_thresh` and `high_thresh` with `is_finite()` fallback (e.g. `0.25` and `0.75`).
   - In `ColorWheelVector::to_polar`: add `if deg >= 360.0 { deg = 0.0; }`.
   - In `apply_hsl_curves`: weight `HueVsLum` by saturation: `let delta = (c.eval(hsl[0] / 360.0) - 0.5) * hsl[1];`.
   - In `ColorWheelVector::from_polar`: ensure `hue_deg` is finite before computing trigonometric functions.

---

## 3. Caveats

- **Monotonicity Verification**: Tested across 10,000 randomized monotonic control point evaluations using a deterministic LCG fuzz generator (`adv_spline_monotonicity_stress_fuzzing_10000_samples`). Zero monotonicity violations were observed for valid finite inputs. The Fritsch-Carlson algorithm implementation is mathematically sound for valid control points.
- **Hardware Acceleration**: All stress testing was conducted in CPU software mode per repository policy. SIMD/GPU shader implementations will require equivalent bounds checks when ported.

---

## 4. Conclusion

**Verdict: REQUEST_CHANGES**

Milestone 3 cannot be approved in its current state due to:
1. **CRITICAL**: Integer underflow panic in `SplineCurve::eval(f32::NAN)`.
2. **HIGH**: Video frame pitch-black corruption in `ColorWheelsGrade::apply_range_limited` on NaN thresholds.
3. **MEDIUM**: `ColorWheelVector::to_polar()` contract violation returning `360.0`.
4. **MEDIUM**: `HueVsLum` achromatic discontinuity causing severe luminance banding on neutral gray pixels.

`worker_m3_1` must implement the mitigations identified in the Logic Chain and update the test suite accordingly.

---

## 5. Verification Method

To independently reproduce and verify all findings:

```powershell
# Run the adversarial stress suite authored for Milestone 3
cargo test -p neoveg_video --test adversarial_m3_splines_wheels -j 1 -- --test-threads=1
```

### Invalidation Conditions
- Any panic when `SplineCurve::eval(f32::NAN)` is called.
- `apply_range_limited` producing `[0, 0, 0, 255]` on a non-black frame when thresholds are `f32::NAN`.
- `ColorWheelVector::from_polar(-360.0, 0.75, 0.2).to_polar()` returning `deg >= 360.0`.
- `apply_hsl_curves` altering neutral gray luminance by $> 50$ levels under pure Red `HueVsLum`.
