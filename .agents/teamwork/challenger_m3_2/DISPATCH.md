# Task Assignment — challenger_m3_2: OCIO/ACES Math, S-Log Curves & Color Match Stress Testing

## Mission
Author and execute empirical adversarial stress tests against the OCIO/ACES pipeline (`crates/neoveg_video/src/ocio.rs`), camera S-Log forward/inverse transfer curves, and `ColorMatcher` / `match_color`.

## Working Directory
`C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\challenger_m3_2`

## Required Reading
- `C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\ORIGINAL_REQUEST.md`
- `C:\Users\paulomatos\Documents\GitHub\neoveg\PROJECT.md`
- `C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\worker_m3_1\handoff.md`

## Adversarial Focus & Edge Cases to Test
1. **Camera S-Log Curves & ACES Log Math**:
   - Negative and zero linear inputs (sensor black clipping, camera noise).
   - High dynamic range values ($x \in [10.0, 1000.0]$).
   - Roundtrip precision across S-Log1, S-Log2, S-Log3, ACEScc, ACEScct: $|f^{-1}(f(x)) - x| < 10^{-4}$ for scene range $[0.001, 10.0]$.
   - Continuous transition at ACEScct toe break point ($x_{brk} = 0.0078125$).
   - Narkowicz ACES RRT tone mapping monotonicity: verify strictly non-decreasing from 0 to 1000.
2. **Color Match Across Clips**:
   - Flat/solid color reference or target (zero variance / division by zero guard).
   - Empty frames ($0 \times 0$ resolution).
   - `match_brightness = false`: verify that output target luminance strictly matches input target luminance within $10^{-4}$.
   - `strength = 0.0`: verify exact bit-for-bit identity with target frame.
   - `strength = 1.0`: verify 100% matched color.
3. **Execution Constraints**:
   - Sequential execution with `-j 1` and `--test-threads=1`.
   - Never run `cargo test -p neoveg_ui`.

Deliver your findings and adversarial tests in `handoff.md` with an explicit verdict (`APPROVE` or `REQUEST_CHANGES`) and notify parent via `send_message`.


## 2026-11-06T14:38:02Z
From: c220c328-9551-48e1-a35b-23e05589d782
Content: You are challenger_m3_2, assigned to empirically stress-test Milestone 3 OCIO/ACES Math, S-Log Curves & Color Match.

Working directory: C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\challenger_m3_2\
Read your task description in: C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\challenger_m3_2\DISPATCH.md
Also read:
- C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\ORIGINAL_REQUEST.md
- C:\Users\paulomatos\Documents\GitHub\neoveg\PROJECT.md
- C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\worker_m3_1\handoff.md

CRITICAL EXECUTION BUDGET:
- Run all Cargo commands sequentially with `-j 1` and `--test-threads=1`.
- NEVER execute `cargo test -p neoveg_ui`.

Deliver your findings and adversarial tests in `handoff.md` with an explicit verdict (APPROVE or REQUEST_CHANGES) and notify parent via `send_message`.
