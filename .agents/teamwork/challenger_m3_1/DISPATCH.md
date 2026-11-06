# Task Assignment — challenger_m3_1: Color Wheels, Curves & Splines Adversarial Stress Testing

## Mission
Author and execute empirical adversarial stress tests against Milestone 3 Color Wheels (`ColorWheelVector`, `ColorWheelsGrade`), `SplineCurve` monotone cubic Hermite interpolation, and the 6 `HslCurves`.

## Working Directory
`C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\challenger_m3_1`

## Required Reading
- `C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\ORIGINAL_REQUEST.md`
- `C:\Users\paulomatos\Documents\GitHub\neoveg\PROJECT.md`
- `C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\worker_m3_1\handoff.md`

## Adversarial Focus & Edge Cases to Test
1. **Color Wheel Vectors**:
   - Extreme angles ($\theta = -720^\circ, 1080^\circ$, near zero, $360^\circ$).
   - Extreme magnitudes ($> 1.0$, negative, NaN/infinity sanitation).
   - Numpad adjustments: wrapping, clamping, sequential additions and subtractions.
   - Range-limited wheels: inverted thresholds ($T_{low} > T_{high}$), extreme values ($< 0.0$, $> 1.0$).
2. **Monotone Cubic Hermite Spline (`SplineCurve`)**:
   - Collinear points, flat horizontal curves, vertical-like step curves, points placed in reverse order.
   - Duplicate or near-duplicate x-coordinates ($|x_1 - x_2| < 10^{-4}$).
   - Boundary values: $x < 0.0$ and $x > 1.0$.
   - Monotonicity stress: verify that for any monotonically increasing control points, `eval(x)` is strictly non-decreasing across 10,000 random samples.
3. **6 HSL Curves**:
   - `HueVsHue`: verify continuity and lack of discontinuity across the $360^\circ / 0^\circ$ boundary (red wrap).
   - Solid white, solid black, and zero-saturation grayscale inputs through all 6 HSL curves.
4. **Execution Constraints**:
   - Sequential execution with `-j 1` and `--test-threads=1`.
   - Never run `cargo test -p neoveg_ui`.

Deliver your findings and adversarial tests in `handoff.md` with an explicit verdict (`APPROVE` or `REQUEST_CHANGES`) and notify parent via `send_message`.

## 2026-11-06T14:37:50Z
You are challenger_m3_1, assigned to empirically stress-test Milestone 3 Color Wheels, Curves, Splines & HSL periodic wrapping.

