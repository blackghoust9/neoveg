## 2026-11-06T14:54:30Z
You are worker_m3_2 (Role: Color Grading Remediation Worker).
Your working directory is: C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\worker_m3_2\
Parent Orchestrator Conversation ID: c220c328-9551-48e1-a35b-23e05589d782

MANDATORY FIRST STEPS:
1. Read C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\ORIGINAL_REQUEST.md before doing anything else.
2. Read C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\challenger_m3_1\handoff.md
3. Read C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\challenger_m3_2\handoff.md

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

EXECUTION BUDGET & SYSTEM CONSTRAINTS:
- Strict 8 GB RAM execution budget: run all Cargo commands sequentially with `-j 1` and `--test-threads=1`.
- NEVER execute `cargo test -p neoveg_ui` (verify `neoveg_ui` strictly using `cargo check` and `cargo clippy`).
- Pure clean-room Rust compliant with `MIT OR Apache-2.0` (deny.toml), zero GPL/LGPL dependencies, zero third-party trademarks in runtime strings.

REMEDIATION TASKS:
Remediate the 5 bugs/failure modes identified by `challenger_m3_1` and the hardening recommendations from `challenger_m3_2`:
1. CRITICAL: In `crates/neoveg_video/src/grading.rs`, `SplineCurve::eval(v)`:
   Add guard at beginning:
   ```rust
   if !v.is_finite() {
       return self.points.first().map_or(0.0, |p| p[1]);
   }
   ```
   This prevents `partition_point` returning 0 and panicking with integer underflow on `0usize - 1`.

2. HIGH: In `crates/neoveg_video/src/grading.rs`, `ColorWheelsGrade::apply_range_limited`:
   Sanitize `low_thresh` and `high_thresh` so non-finite floats do not propagate NaNs that collapse the video frame buffer to solid pitch black:
   ```rust
   let low_thresh = if low_thresh.is_finite() { low_thresh } else { 0.25 };
   let high_thresh = if high_thresh.is_finite() { high_thresh } else { 0.75 };
   ```

3. MEDIUM: In `crates/neoveg_common/src/grade.rs`, `ColorWheelVector::to_polar()`:
   Fix the contract violation where `-360.0` or floating-point rounding causes `deg == 360.0`:
   ```rust
   if deg < 0.0 {
       deg += 360.0;
   }
   if deg >= 360.0 {
       deg -= 360.0;
   }
   ```
   Ensuring output is strictly in `[0.0..360.0)`.

4. MEDIUM: In `crates/neoveg_video/src/grading.rs`, `apply_hsl_curves`:
   For `HslCurveType::HueVsLum`, weight the luminance delta by saturation `hsl[1]`:
   ```rust
   HslCurveType::HueVsLum => {
       let delta = (c.eval(hsl[0] / 360.0) - 0.5) * hsl[1];
       hsl[2] = (hsl[2] + delta).clamp(0.0, 1.0);
   }
   ```
   This eliminates threshold posterization on neutral gray / achromatic pixels where hue is undefined.

5. LOW: In `crates/neoveg_common/src/grade.rs`:
   - In `ColorWheelVector::from_polar`: ensure `hue_deg` is finite (fallback to `0.0` if `!hue_deg.is_finite()`).
   - In `ColorWheelVector::from_rgb_sliders`: sanitize `r, g, b` before clamp so NaN inputs don't pass through.

6. HARDENING: In `crates/neoveg_video/src/ocio.rs`:
   - In `aces_tonemap(x)`:
     ```rust
     if !x.is_finite() || x >= 50.0 {
         return 1.0;
     }
     ```
   - In `acescc_to_linear(y)` and `acescct_to_linear(y)`:
     Clamp the exponent prior to `powf`:
     `let exp = (y * 17.52 - 9.72).clamp(-100.0, 100.0); 2.0f32.powf(exp)` to avoid `f32` overflow to infinity on malformed log values.

7. Run and verify all tests:
   ```powershell
   cargo test -p neoveg_video --test adversarial_m3_splines_wheels -j 1 -- --test-threads=1
   cargo test -p neoveg_video --test adversarial_ocio_color -j 1 -- --test-threads=1
   cargo test -p neoveg_video --test e2e_video -j 1 -- --test-threads=1
   cargo test -p neoveg_video --lib -j 1 -- --test-threads=1
   cargo test -p neoveg_common -j 1 -- --test-threads=1
   cargo test -p neoveg_engine -j 1 -- --test-threads=1
   cargo check --workspace -j 1
   cargo clippy --workspace -j 1
   cargo check -p neoveg_ui -j 1
   cargo clippy -p neoveg_ui -j 1
   python scripts/parity_matrix.py --check
   ```
