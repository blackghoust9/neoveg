## 2026-11-06T13:44:24Z

You are `spec_miner_m3_2`, working in directory `C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\spec_miner_m3_2`.

MANDATORY: Read the original user request before starting work:
`C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\ORIGINAL_REQUEST.md`

## Mission: VEGAS Pro Color Grading Specification Mining
Mine exact specifications, mathematical formulas, and parameter behaviors for Milestone 3 (Color Grading & OCIO/ACES):
- Consult reference manuals in `C:\Users\paulomatos\Documents\GitHub\neoveg\manuais` (search for Color Grading, Color Wheels, Curves, White Balance, Color Match, OpenColorIO, ACES, S-Log).
- Consult `results.md` in repository root for domain models and edge cases relating to features 11–16.

## Specific Requirements to Mine
1. **Primary 4-Way Color Wheels**:
   - Lift (Shadows), Gamma (Midtones), Gain (Highlights), Offset (Global).
   - Parameter representations: Angle/Magnitude vs Cartestian (X, Y), Luma adjustment slider ranges, Saturation factor.
   - Mathematical blending formula in linear and non-linear color space: how Lift, Gamma, Gain, and Offset affect pixel values.
2. **White Balance Neutral Picker**:
   - Eyedropper sampling behavior: given sampled neutral RGB $(R_{sample}, G_{sample}, B_{sample})$, what are the temperature and tint adjustment formulas that neutralize the sampled pixel?
3. **Curves (RGB & 6 HSL Curves)**:
   - Master RGB, Red, Green, Blue spline curve interpolation (smooth monotonicity, clamping).
   - 6 HSL curves: Hue vs Hue, Hue vs Saturation, Hue vs Luminance, Luminance vs Saturation, Saturation vs Saturation, Saturation vs Luminance.
4. **OpenColorIO (OCIO) & ACES Workflow**:
   - ACES version support (ACES 1.0, 1.1, 1.2).
   - IDTs: S-Log1, S-Log2, S-Log3 / S-Gamut, S-Gamut3, S-Gamut3.Cine formulas.
   - Working color spaces: ACEScc (logarithmic), ACEScct (with toe).
   - ODT: Output Device Transform to Rec.709 and sRGB with highlight rolloff.
   - Pure CPU software fallbacks: exact mathematical transfer functions for CPU evaluation without live OCIO C++ library or GPU shaders.
5. **Color Match Across Clips**:
   - Algorithm for matching target clip to reference clip: luminance histogram/percentile matching, color balance alignment (mean and covariance of RGB channels).
6. **VEGAS Pro Semantic Parity & Trademarks**:
   - Identify behaviors that must adhere strictly to VEGAS Pro semantics.
   - Document any semantic notes for `README.md`.
   - Ensure all runtime strings and identifiers are completely generic and free of third-party trademarks (e.g. use "Camera S-Log", "Interchange ACES", etc.).

Write your structured specification report to `C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\spec_miner_m3_2\handoff.md` and send a message back.
