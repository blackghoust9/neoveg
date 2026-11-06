# Handoff Report — Challenger M2-1: 3D Transforms, Transitions & Envelopes Stress Tester

## 1. Observation

- **Adversarial stress test execution**:
  - Test harness created: `crates/neoveg_video/tests/adversarial_transitions.rs`.
  - Executed under project constraints: sequential execution `-j 1` and `--test-threads=1`, never invoking `neoveg_ui`.
  - Command: `cargo test -p neoveg_video --test adversarial_transitions -j 1 -- --test-threads=1`
  - Output:
    ```
    running 16 tests
    test adv_envelope_duplicate_timestamps ... ok
    test adv_envelope_empty_points ... ok
    test adv_envelope_infinite_timestamp_exposes_nan_propagation ... ok
    test adv_envelope_nan_and_infinite_times ... ok
    test adv_envelope_nan_timestamp_causes_underflow_panic - should panic ... ok
    test adv_envelope_out_of_order_timestamps ... ok
    test adv_envelope_single_point ... ok
    test adv_envelope_unconditional_clamping_all_shapes ... ok
    test adv_render_thumbnail_never_panics_on_empty_and_1x1_buffers ... ok
    test adv_threed_transform_extreme_and_perpendicular_angles ... ok
    test adv_threed_transform_extreme_depth_and_specular ... ok
    test adv_threed_transform_nan_and_inf_rotations ... ok
    test adv_transitions_all_3d_presets_adversarial_progress ... ok
    test adv_transitions_cube_adversarial_params ... ok
    test adv_transitions_fly_in_and_shuffle_extreme_parameters ... ok
    test adv_transitions_page_roll_and_peel_adversarial_params ... ok

    test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.28s
    ```

- **Observed Vulnerability 1 — Integer Underflow Panic**:
  - Source file: `crates/neoveg_video/src/transitions.rs:319:31`
  - Verbatim code:
    ```rust
    let idx = self.points.partition_point(|pt| pt.time < time);
    let p1 = &self.points[idx];
    let p0 = &self.points[idx - 1];
    ```
  - Verbatim panic when `add_point(f64::NAN, 0.5, ...)` is called and `progress_at(0.0)` is queried:
    ```
    thread 'adv_envelope_nan_timestamp_causes_underflow_panic' (9408) panicked at crates\neoveg_video\src\transitions.rs:319:31:
    attempt to subtract with overflow
    ```

- **Observed Vulnerability 2 — NaN Leakage / Violation of [0.0..=1.0] Clamping Guarantee**:
  - Source file: `crates/neoveg_video/src/transitions.rs:326-329`
  - Verbatim code:
    ```rust
    let u = ((time - p0.time) / dt) as f32;
    let factor = p0.shape.eval(u);
    let value = p0.progress + (p1.progress - p0.progress) * factor;
    value.clamp(0.0, 1.0)
    ```
  - When `add_point(f64::NEG_INFINITY, 0.5, ...)` is called and `progress_at(-10.0)` is evaluated:
    - `dt = p1.time - p0.time = 0.0 - (-inf) = +inf`
    - `u = ((-10.0 - (-inf)) / +inf) = +inf / +inf = NaN`
    - `factor = shape.eval(NaN) = NaN`
    - In Rust standard library, `f32::NAN.clamp(0.0, 1.0)` returns `NaN`.
    - `progress_at` outputs `NaN`.
    - Assertion `(0.0..=1.0).contains(&p)` returns `false`.

- **Observed Passing Resilient Components**:
  - `ThreeDTransform`: Tested with 1,805 angle combinations including exact perpendicular tangents (0, 90, -90, 180, 270, 360, 450, 720, -720, 100000, NaN, Inf), depths from -10000 to +10000, and specular light out of range. All ray-plane intersections and CPU rasterizations execute safely without panics.
  - 3D Transitions (`FlyIn`, `Shuffle`, `Cube`, `PageRoll`, `PagePeel`): Tested with extreme parameters, out-of-range progress (-100 to 100), NaN, and Inf. All returned valid frame buffers without panics.
  - `render_thumbnail`: Tested with 0x0, 0x10, 10x0, 1x1, and mismatched dimensions across all 12 presets (`cross_dissolve`, `linear_wipe`, `radial_wipe`, `iris`, `push`, `slide`, `zoom`, `3d_fly_in`, `page_peel`, `page_roll`, `cube`, `3d_shuffle`). All execute safely without panics.

---

## 2. Logic Chain

1. In `crates/neoveg_video/src/transitions.rs:285-301`:
   - `add_point(&mut self, time: f64, progress: f32, shape: TransitionEnvelopeShape)` inserts points using `partition_point(|pt| pt.time < time)`.
   - It does not validate `time.is_finite()`.
   - When `time` is `f64::NAN`, both `(pt.time - NaN).abs() < 1e-6` and `pt.time < NaN` evaluate to `false` for every element.
   - Consequently, `pos` is evaluated as `0`, and the point with `time = NaN` is inserted at index 0 of `self.points`.
2. In `crates/neoveg_video/src/transitions.rs:308-319`:
   - In `progress_at(&self, time: f64)`:
   - When querying at `time = 0.0` (or any `time <= points[1].time`), `time <= self.points[0].time` evaluates to `0.0 <= NaN`, which is `false`.
   - The method does not early-return and proceeds to `self.points.partition_point(|pt| pt.time < time)`.
   - For `points[0]` (`NaN`), `NaN < 0.0` evaluates to `false`.
   - `partition_point` returns `0`.
   - Line 319 executes `let p0 = &self.points[idx - 1]`.
   - Because `idx == 0`, `idx - 1` performs `0usize - 1usize`, resulting in an arithmetic overflow panic (`attempt to subtract with overflow`).
3. In `crates/neoveg_video/src/transitions.rs:321-329`:
   - If an infinite point is added (`f64::NEG_INFINITY` or `f64::INFINITY`), `dt` evaluates to `+inf`.
   - Division of infinite expressions results in `u = NaN`.
   - In Rust, `f32::NAN.clamp(0.0, 1.0)` does not return a clamped number; it evaluates to `NaN`.
   - As a result, `progress_at` outputs `NaN`, violating the explicit contract that progress is unconditionally clamped to `0.0..=1.0`.

---

## 3. Caveats

- Hardware GPU shader pipelines (`vfx_gpu.rs`) were not tested on physical GPU hardware because the test environment is headless CPU; software CPU fallbacks were verified.
- No other defects were identified in `ThreeDTransform`, 3D transitions, or `render_thumbnail`.

---

## 4. Conclusion

**Verdict: REQUEST_CHANGES.**

While 3D transforms, 3D transition geometries, and thumbnail rendering are robust and panic-free, `TransitionProgressEnvelope` contains a critical integer underflow panic on `NaN` timestamps and leaks `NaN` progress when given non-finite timestamps.

### Required Changes for Worker M2:

In `crates/neoveg_video/src/transitions.rs`:

1. **Guard `add_point` against non-finite timestamps and progress**:
   ```rust
   pub fn add_point(&mut self, time: f64, progress: f32, shape: TransitionEnvelopeShape) {
       if !time.is_finite() {
           return;
       }
       let p = if progress.is_finite() {
           progress.clamp(0.0, 1.0)
       } else {
           1.0
       };
       ...
   ```

2. **Harden `progress_at` with defensive index bounds and finite time checks**:
   ```rust
   pub fn progress_at(&self, time: f64) -> f32 {
       if !self.enabled || self.points.is_empty() || !time.is_finite() {
           return 1.0;
       }

       if time <= self.points[0].time {
           return self.points[0].progress.clamp(0.0, 1.0);
       }

       let last = self.points.last().unwrap();
       if time >= last.time {
           return last.progress.clamp(0.0, 1.0);
       }

       let idx = self.points.partition_point(|pt| pt.time < time);
       if idx == 0 {
           return self.points[0].progress.clamp(0.0, 1.0);
       }
       if idx >= self.points.len() {
           return last.progress.clamp(0.0, 1.0);
       }
       let p1 = &self.points[idx];
       let p0 = &self.points[idx - 1];

       let dt = p1.time - p0.time;
       if !dt.is_finite() || dt.abs() < 1e-6 {
           return p0.progress.clamp(0.0, 1.0);
       }

       let u = ((time - p0.time) / dt) as f32;
       let factor = p0.shape.eval(u);
       let value = p0.progress + (p1.progress - p0.progress) * factor;
       if value.is_nan() {
           p0.progress.clamp(0.0, 1.0)
       } else {
           value.clamp(0.0, 1.0)
       }
   }
   ```

3. **Harden `TransitionEnvelopeShape::eval`**:
   ```rust
   pub fn eval(self, u: f32) -> f32 {
       let u = if u.is_nan() { 0.0 } else { u.clamp(0.0, 1.0) };
       ...
   ```

---

## 5. Verification Method

To verify the test suite:
```bash
cargo check -j 1 --workspace
cargo clippy -j 1 --workspace
cargo test -p neoveg_video --test adversarial_transitions -j 1 -- --test-threads=1
cargo test -p neoveg_video --lib -j 1 -- --test-threads=1
cargo test -p neoveg_video --test e2e_video -j 1 -- --test-threads=1
python scripts/parity_matrix.py --check
```

Invalidation conditions:
- Any panic or non-finite output from `TransitionProgressEnvelope::progress_at` on adversarial timestamps.
- Any regression in existing transition unit/e2e tests.
