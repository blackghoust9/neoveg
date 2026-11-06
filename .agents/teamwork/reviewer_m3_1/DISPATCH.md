# Task Assignment — reviewer_m3_1: Color Wheels, Curves & White Balance Review

## Mission
Independently review the Milestone 3 implementation of 4-Way Color Wheels, Transfer Curves (Monotone Cubic Hermite Splines, RGB and 6 HSL curves), and White Balance Neutral Tone Picker in `neoveg_common` and `neoveg_video::grading`.

## Working Directory
`C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\reviewer_m3_1`

## Required Reading
- `C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\ORIGINAL_REQUEST.md`
- `C:\Users\paulomatos\Documents\GitHub\neoveg\PROJECT.md`
- `C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\worker_m3_1\handoff.md`

## Review Focus
1. **Mathematical Soundness**:
   - `ColorWheelVector`: polar $\leftrightarrow$ Cartesian $\leftrightarrow$ zero-sum RGB conversions.
   - Numpad channel adjustments matching VEGAS Pro semantics.
   - `SplineCurve`: Fritsch-Carlson monotone cubic Hermite algorithm guarantees strict monotonicity without overshoot.
   - `HslCurves`: periodic angular wrapping at $360^\circ$ for `HueVsHue` (`rem_euclid(1.0)`).
   - `pick_white_balance`: neutral temperature/tint formulas, region averaging, and $Y \le 10^{-4}$ dark pixel protection.
2. **Performance & Memory**:
   - 256-step L1 cached 1D LUTs for RGB curves.
3. **Execution Budget & Lints**:
   - Verify that all builds and tests pass cleanly under `-j 1` and `--test-threads=1`.
   - Never run `cargo test -p neoveg_ui` (run `cargo check -p neoveg_ui -j 1`).
4. **Clean-room & Trademarks**:
   - Zero proprietary third-party vendor trademarks in code, comments, or UI strings.

Write `handoff.md` in your working directory with an explicit verdict (`APPROVE` or `REQUEST_CHANGES`) and notify parent via `send_message`.


## 2026-11-06T14:37:29Z
You are reviewer_m3_1, assigned to review Milestone 3 (Color Wheels, Curves & White Balance).

Working directory: C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\reviewer_m3_1\
Read your task description in: C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\reviewer_m3_1\DISPATCH.md
Also read:
- C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\ORIGINAL_REQUEST.md
- C:\Users\paulomatos\Documents\GitHub\neoveg\PROJECT.md
- C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\worker_m3_1\handoff.md

CRITICAL EXECUTION BUDGET:
- Run all Cargo commands sequentially with `-j 1` and `--test-threads=1`.
- NEVER execute `cargo test -p neoveg_ui` (verify `neoveg_ui` strictly using `cargo check -p neoveg_ui -j 1`).

When done, write `handoff.md` with an explicit verdict (APPROVE or REQUEST_CHANGES) and notify parent via `send_message`.
