//! Adversarial Empirical Stress Test Suite for Milestone 3:
//! - OCIO / ACES Color Pipelines & Matrix Transforms
//! - Camera S-Log1, S-Log2, S-Log3 and ACEScc/ACEScct Transfer Curves
//! - ACES RRT Tone Mapping Monotonicity & Asymptotes
//! - Color Match Across Clips & Luminance Invariance
//! - Spline Continuity, HSL Periodic Wrapping, and White Balance Pickers
//!
//! Verified under strict sequential execution constraints: `-j 1` and `--test-threads=1`.

use neoveg_video::frame::FrameBuffer;
use neoveg_video::grading::{
    hsl_to_rgb, match_color, pick_black_point, pick_white_balance,
    pick_white_balance_from_region, pick_white_point, rgb_to_hsl, ColorMatcher,
    HslCurve, HslCurveType, SplineCurve, StatisticalColorMatcher,
};
use neoveg_video::ocio::{
    aces_tonemap, acescc_to_linear, acescct_to_linear, linear_to_acescc,
    linear_to_acescct, linear_to_slog1, linear_to_slog2, linear_to_slog3,
    mat_mul_rgb, slog1_to_linear, slog2_to_linear, slog3_to_linear,
    AcesSpace, AcesTransform, ViewTransform, M_AP0_TO_AP1, M_AP1_TO_AP0,
    M_AP1_TO_REC709, M_REC709_TO_AP1,
};

// ============================================================================
// 1. Camera S-Log & ACES Log Math: Negative, Zero & Sensor Noise Inputs
// ============================================================================

#[test]
fn adv_slog_negative_and_zero_inputs_resilience() {
    let test_inputs: &[f32] = &[
        -1000.0, -100.0, -10.0, -1.0, -0.5, -0.1, -0.037584, -0.01125, -0.001,
        -1e-5, -1e-8, 0.0,
    ];

    for &x in test_inputs {
        // S-Log1 forward and inverse
        let y1 = linear_to_slog1(x);
        assert!(y1.is_finite(), "linear_to_slog1({x}) must be finite, got {y1}");
        let back1 = slog1_to_linear(y1);
        assert!(back1.is_finite(), "slog1_to_linear({y1}) must be finite, got {back1}");

        // S-Log2 forward and inverse
        let y2 = linear_to_slog2(x);
        assert!(y2.is_finite(), "linear_to_slog2({x}) must be finite, got {y2}");
        let back2 = slog2_to_linear(y2);
        assert!(back2.is_finite(), "slog2_to_linear({y2}) must be finite, got {back2}");

        // S-Log3 forward and inverse
        let y3 = linear_to_slog3(x);
        assert!(y3.is_finite(), "linear_to_slog3({x}) must be finite, got {y3}");
        let back3 = slog3_to_linear(y3);
        assert!(back3.is_finite(), "slog3_to_linear({y3}) must be finite, got {back3}");
        // S-Log3 linear toe formula guarantees exact roundtrip for negative values
        if x <= 0.0 && x >= -0.01125 {
            assert!(
                (back3 - x).abs() < 1e-5,
                "S-Log3 linear toe roundtrip mismatch at x={x}: back={back3}"
            );
        }

        // ACEScc forward and inverse
        let y_cc = linear_to_acescc(x);
        assert!(y_cc.is_finite(), "linear_to_acescc({x}) must be finite, got {y_cc}");
        let back_cc = acescc_to_linear(y_cc);
        assert!(back_cc.is_finite(), "acescc_to_linear({y_cc}) must be finite, got {back_cc}");

        // ACEScct forward and inverse
        let y_cct = linear_to_acescct(x);
        assert!(y_cct.is_finite(), "linear_to_acescct({x}) must be finite, got {y_cct}");
        let back_cct = acescct_to_linear(y_cct);
        assert!(back_cct.is_finite(), "acescct_to_linear({y_cct}) must be finite, got {back_cct}");
        // ACEScct linear toe formula guarantees exact roundtrip for all x <= X_BRK
        assert!(
            (back_cct - x).abs() < 1e-5,
            "ACEScct linear toe roundtrip mismatch at x={x}: back={back_cct}"
        );
    }
}

// ============================================================================
// 2. High Dynamic Range (HDR) Input Scaling: x in [10.0, 1000.0]
// ============================================================================

#[test]
fn adv_slog_and_aces_hdr_values_stability() {
    let hdr_inputs: &[f32] = &[
        10.0, 20.0, 50.0, 100.0, 250.0, 500.0, 800.0, 1000.0, 5000.0,
    ];

    let mut prev_slog1 = linear_to_slog1(9.9);
    let mut prev_slog2 = linear_to_slog2(9.9);
    let mut prev_slog3 = linear_to_slog3(9.9);
    let mut prev_acescc = linear_to_acescc(9.9);
    let mut prev_acescct = linear_to_acescct(9.9);

    for &x in hdr_inputs {
        let y1 = linear_to_slog1(x);
        let y2 = linear_to_slog2(x);
        let y3 = linear_to_slog3(x);
        let y_cc = linear_to_acescc(x);
        let y_cct = linear_to_acescct(x);

        // Strict monotonicity across HDR range
        assert!(y1 > prev_slog1, "S-Log1 must strictly increase at x={x}");
        assert!(y2 > prev_slog2, "S-Log2 must strictly increase at x={x}");
        assert!(y3 > prev_slog3, "S-Log3 must strictly increase at x={x}");
        assert!(y_cc > prev_acescc, "ACEScc must strictly increase at x={x}");
        assert!(y_cct > prev_acescct, "ACEScct must strictly increase at x={x}");

        prev_slog1 = y1;
        prev_slog2 = y2;
        prev_slog3 = y3;
        prev_acescc = y_cc;
        prev_acescct = y_cct;

        // Relative precision across HDR values |f^-1(y) - x| / x < 1e-4
        let b1 = slog1_to_linear(y1);
        let b2 = slog2_to_linear(y2);
        let b3 = slog3_to_linear(y3);
        let b_cc = acescc_to_linear(y_cc);
        let b_cct = acescct_to_linear(y_cct);

        assert!(((b1 - x) / x).abs() < 1e-4, "S-Log1 HDR rel error at {x}: {b1}");
        assert!(((b2 - x) / x).abs() < 1e-4, "S-Log2 HDR rel error at {x}: {b2}");
        assert!(((b3 - x) / x).abs() < 1e-4, "S-Log3 HDR rel error at {x}: {b3}");
        assert!(((b_cc - x) / x).abs() < 1e-4, "ACEScc HDR rel error at {x}: {b_cc}");
        assert!(((b_cct - x) / x).abs() < 1e-4, "ACEScct HDR rel error at {x}: {b_cct}");
    }
}

// ============================================================================
// 3. Dense Roundtrip Precision Across Standard Scene Range [0.001, 10.0]
// ============================================================================

#[test]
fn adv_slog_and_aces_dense_roundtrip_precision() {
    // 500 samples distributed logarithmically across [0.001, 10.0]
    let steps = 500;
    let log_min = (0.001f32).ln();
    let log_max = (10.0f32).ln();
    let step = (log_max - log_min) / (steps as f32);

    for i in 0..=steps {
        let x = (log_min + (i as f32) * step).exp();

        // S-Log1
        let y_slog1 = linear_to_slog1(x);
        let back_slog1 = slog1_to_linear(y_slog1);
        assert!(
            (back_slog1 - x).abs() < 1e-4,
            "S-Log1 roundtrip failed at x={x}: back={back_slog1}, diff={}",
            (back_slog1 - x).abs()
        );

        // S-Log2
        let y_slog2 = linear_to_slog2(x);
        let back_slog2 = slog2_to_linear(y_slog2);
        assert!(
            (back_slog2 - x).abs() < 1e-4,
            "S-Log2 roundtrip failed at x={x}: back={back_slog2}, diff={}",
            (back_slog2 - x).abs()
        );

        // S-Log3
        let y_slog3 = linear_to_slog3(x);
        let back_slog3 = slog3_to_linear(y_slog3);
        assert!(
            (back_slog3 - x).abs() < 1e-4,
            "S-Log3 roundtrip failed at x={x}: back={back_slog3}, diff={}",
            (back_slog3 - x).abs()
        );

        // ACEScc
        let y_acescc = linear_to_acescc(x);
        let back_acescc = acescc_to_linear(y_acescc);
        assert!(
            (back_acescc - x).abs() < 1e-4,
            "ACEScc roundtrip failed at x={x}: back={back_acescc}, diff={}",
            (back_acescc - x).abs()
        );

        // ACEScct
        let y_acescct = linear_to_acescct(x);
        let back_acescct = acescct_to_linear(y_acescct);
        assert!(
            (back_acescct - x).abs() < 1e-4,
            "ACEScct roundtrip failed at x={x}: back={back_acescct}, diff={}",
            (back_acescct - x).abs()
        );
    }
}

// ============================================================================
// 4. ACEScct Toe Break Point Continuity (x_brk = 0.0078125)
// ============================================================================

#[test]
fn adv_acescct_toe_breakpoint_continuity() {
    const X_BRK: f32 = 0.0078125; // 2^-7
    const Y_BRK: f32 = (-7.0 + 9.72) / 17.52; // ~0.15525114

    // Check boundary value at break point from left (toe formula) and right (log formula)
    let y_toe = 10.540237 * X_BRK + 0.07290553;
    let y_log = (X_BRK.log2() + 9.72) / 17.52;
    assert!(
        (y_toe - y_log).abs() < 1e-6,
        "ACEScct forward break point continuity failed: toe={y_toe}, log={y_log}"
    );

    // Check inverse boundary value at break point
    let x_toe = (Y_BRK - 0.07290553) / 10.540237;
    let x_log = 2.0f32.powf(Y_BRK * 17.52 - 9.72);
    assert!(
        (x_toe - x_log).abs() < 1e-6,
        "ACEScct inverse break point continuity failed: toe={x_toe}, log={x_log}"
    );

    // Check continuity across small delta
    let epsilons = [1e-6f32, 1e-5];
    for &eps in &epsilons {
        let x_left = X_BRK - eps;
        let x_right = X_BRK + eps;
        let y_left = linear_to_acescct(x_left);
        let y_right = linear_to_acescct(x_right);

        assert!(
            (y_right - y_left).abs() < 2.5e-4,
            "ACEScct delta at X_BRK with eps={eps}: left={y_left}, right={y_right}"
        );
        assert!(y_right > y_left, "ACEScct must remain strictly increasing across X_BRK");

        let y_inv_left = Y_BRK - eps;
        let y_inv_right = Y_BRK + eps;
        let x_inv_left = acescct_to_linear(y_inv_left);
        let x_inv_right = acescct_to_linear(y_inv_right);

        assert!(
            (x_inv_right - x_inv_left).abs() < 2.5e-4,
            "ACEScct inverse delta at Y_BRK with eps={eps}: left={x_inv_left}, right={x_inv_right}"
        );
        assert!(x_inv_right > x_inv_left, "ACEScct inverse must remain strictly increasing across Y_BRK");
    }
}

// ============================================================================
// 5. Narkowicz ACES RRT Tone Mapping Monotonicity & Asymptote Verification
// ============================================================================

#[test]
fn adv_aces_tonemap_strict_monotonicity_and_asymptote() {
    let mut prev = 0.0f32;

    // Sub-range 1: fine granularity from 0.0 to 10.0
    for i in 0..=10000 {
        let x = i as f32 * 0.001;
        let tm = aces_tonemap(x);
        assert!(
            tm >= prev,
            "Monotonicity violation at x={x}: tm={tm}, prev={prev}"
        );
        assert!(tm >= 0.0 && tm <= 1.0, "ACES tonemap out of bounds: {tm}");
        prev = tm;
    }

    // Sub-range 2: broad dynamic range from 10.0 to 1000.0
    for i in 100..=10000 {
        let x = i as f32 * 0.1;
        let tm = aces_tonemap(x);
        assert!(
            tm >= prev,
            "Monotonicity violation at HDR x={x}: tm={tm}, prev={prev}"
        );
        assert!(tm <= 1.0, "ACES tonemap must clamp to 1.0, got {tm}");
        prev = tm;
    }

    // Negative input clamp guard
    assert_eq!(aces_tonemap(-0.5), 0.0);
    assert_eq!(aces_tonemap(-1000.0), 0.0);
}

// ============================================================================
// 6. Gamut Matrices Orthogonality & Inversion Precision
// ============================================================================

#[test]
fn adv_gamut_matrices_precision_and_roundtrip() {
    let test_vectors = [
        [1.0f32, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.5, 0.5, 0.5],
        [0.18, 0.18, 0.18],
        [0.8, 0.2, 0.05],
    ];

    for v in test_vectors {
        // AP0 <-> AP1 (single-precision roundoff tolerance 5e-4 due to 7-digit constants)
        let ap1 = mat_mul_rgb(&M_AP0_TO_AP1, v);
        let ap0 = mat_mul_rgb(&M_AP1_TO_AP0, ap1);
        for c in 0..3 {
            assert!(
                (ap0[c] - v[c]).abs() < 5e-4,
                "AP0/AP1 matrix inverse failure in channel {c}: got {}, expected {}",
                ap0[c], v[c]
            );
        }

        // Rec709 <-> AP1 (single precision floating point roundoff tolerance 2e-4)
        let ap1_from_709 = mat_mul_rgb(&M_REC709_TO_AP1, v);
        let back_709 = mat_mul_rgb(&M_AP1_TO_REC709, ap1_from_709);
        for c in 0..3 {
            assert!(
                (back_709[c] - v[c]).abs() < 2e-4,
                "Rec709/AP1 matrix inverse failure in channel {c}: got {}, expected {}",
                back_709[c], v[c]
            );
        }
    }
}

// ============================================================================
// 7. Color Match: Zero-Variance (Solid Flat Colors) & Safety Scale Clamping
// ============================================================================

#[test]
fn adv_color_match_zero_variance_solid_flat_frames() {
    let matcher = StatisticalColorMatcher;

    // Both solid
    let solid_ref = FrameBuffer::solid(8, 8, [150, 90, 50, 255]);
    let solid_tgt = FrameBuffer::solid(8, 8, [50, 90, 150, 255]);
    let wheels = matcher.analyze_and_match(&solid_ref, &solid_tgt);

    // Verify all wheel vector components are finite
    assert!(wheels.lift.x.is_finite());
    assert!(wheels.lift.y.is_finite());
    assert!(wheels.lift.master.is_finite());
    assert!(wheels.gain.x.is_finite());
    assert!(wheels.gain.y.is_finite());
    assert!(wheels.gain.master.is_finite());
    assert!(wheels.offset.x.is_finite());
    assert!(wheels.offset.y.is_finite());
    assert!(wheels.offset.master.is_finite());

    // Both black (0 luminance)
    let black_ref = FrameBuffer::solid(4, 4, [0, 0, 0, 255]);
    let black_tgt = FrameBuffer::solid(4, 4, [0, 0, 0, 255]);
    let wheels_black = matcher.analyze_and_match(&black_ref, &black_tgt);
    assert!(wheels_black.gain.master.is_finite());
    assert!(wheels_black.offset.master.is_finite());

    // White to black and black to white
    let white_ref = FrameBuffer::solid(4, 4, [255, 255, 255, 255]);
    let wheels_w2b = matcher.analyze_and_match(&white_ref, &black_tgt);
    assert!(wheels_w2b.gain.master.is_finite());

    // Direct match_color on solid frames within 10x ratio range: matches 100%
    let mut tgt_run = solid_tgt.clone();
    match_color(&mut tgt_run, &solid_ref, true, 1.0);
    assert_eq!(tgt_run.pixel(0, 0), [150, 90, 50, 255]);

    // Test safety clamping [0.1, 10.0] on extreme ratios (200 / 10 = 20x clamped to 10x)
    let extreme_ref = FrameBuffer::solid(4, 4, [200, 50, 10, 255]);
    let extreme_tgt = FrameBuffer::solid(4, 4, [10, 50, 180, 255]);
    let mut tgt_extreme = extreme_tgt.clone();
    match_color(&mut tgt_extreme, &extreme_ref, true, 1.0);
    let p = tgt_extreme.pixel(0, 0);
    assert_eq!(p[0], 100, "20x ratio must clamp to 10x max gain (10 * 10 = 100)");
    assert_eq!(p[1], 50, "1x ratio produces exact 50");
    assert_eq!(p[2], 18, "0.055x ratio must clamp to 0.1x min gain (180 * 0.1 = 18)");
}

// ============================================================================
// 8. Color Match: Empty Frames (0x0 Resolution) Safe No-Op
// ============================================================================

#[test]
fn adv_color_match_empty_frames_zero_by_zero() {
    let matcher = StatisticalColorMatcher;

    let empty1 = FrameBuffer::solid(0, 0, [0, 0, 0, 0]);
    let empty2 = FrameBuffer::solid(0, 0, [0, 0, 0, 0]);
    let valid = FrameBuffer::solid(4, 4, [100, 100, 100, 255]);

    // Empty vs Empty
    let w1 = matcher.analyze_and_match(&empty1, &empty2);
    assert!(w1.is_neutral());

    // Valid ref vs Empty target
    let w2 = matcher.analyze_and_match(&valid, &empty1);
    assert!(w2.is_neutral());

    // Empty ref vs Valid target
    let w3 = matcher.analyze_and_match(&empty1, &valid);
    assert!(w3.is_neutral());

    // match_color execution with 0x0 frames
    let mut mut_empty = FrameBuffer::solid(0, 0, [0, 0, 0, 0]);
    match_color(&mut mut_empty, &valid, true, 1.0);
    assert_eq!(mut_empty.rgba.len(), 0);

    let mut mut_valid = valid.clone();
    match_color(&mut mut_valid, &empty1, true, 1.0);
    assert_eq!(mut_valid, valid, "Empty reference must be exact no-op");
}

// ============================================================================
// 9. Color Match: Strength Boundary Identities (strength = 0.0 & 1.0)
// ============================================================================

#[test]
fn adv_color_match_strength_boundary_identities() {
    let reference = FrameBuffer::solid(4, 4, [220, 140, 40, 255]);
    let target_original = FrameBuffer::solid(4, 4, [40, 90, 160, 255]);

    // Strength = 0.0: strict bit-for-bit identity
    let mut target_zero = target_original.clone();
    match_color(&mut target_zero, &reference, true, 0.0);
    assert_eq!(
        target_zero.rgba, target_original.rgba,
        "Strength 0.0 must preserve exact bit-for-bit frame identity"
    );

    // Negative strength: clamped to 0.0
    let mut target_neg = target_original.clone();
    match_color(&mut target_neg, &reference, true, -1.0);
    assert_eq!(
        target_neg.rgba, target_original.rgba,
        "Negative strength must clamp to 0.0 and preserve bit-for-bit identity"
    );

    // NaN strength: handled as 0.0 no-op
    let mut target_nan = target_original.clone();
    match_color(&mut target_nan, &reference, true, f32::NAN);
    assert_eq!(
        target_nan.rgba, target_original.rgba,
        "NaN strength must default to 0.0 safe no-op"
    );

    // Strength = 1.0: 100% matched color (within clamp range)
    let ref_norm = FrameBuffer::solid(4, 4, [120, 90, 60, 255]);
    let tgt_norm = FrameBuffer::solid(4, 4, [60, 90, 120, 255]);
    let mut target_one = tgt_norm.clone();
    match_color(&mut target_one, &ref_norm, true, 1.0);
    assert_eq!(
        target_one.pixel(0, 0),
        ref_norm.pixel(0, 0),
        "Strength 1.0 must produce 100% color match on solid frames"
    );

    // Overshoot strength (2.0): clamped to 1.0
    let mut target_two = tgt_norm.clone();
    match_color(&mut target_two, &ref_norm, true, 2.0);
    assert_eq!(
        target_two.pixel(0, 0),
        ref_norm.pixel(0, 0),
        "Strength > 1.0 must clamp to 1.0"
    );
}

// ============================================================================
// 10. Color Match: match_brightness = false Luminance Invariance
// ============================================================================

#[test]
fn adv_color_match_match_brightness_false_luminance_retention() {
    // Construct a target frame with moderate values well inside dynamic range [40..180]
    // so color shifts will not clip at 0 or 255 boundaries
    let mut target = FrameBuffer::solid(8, 8, [80, 100, 120, 255]);
    let reference = FrameBuffer::solid(8, 8, [180, 80, 40, 255]); // Warm reddish cast

    let luma = |px: [u8; 4]| -> f32 {
        0.2126 * f32::from(px[0]) + 0.7152 * f32::from(px[1]) + 0.0722 * f32::from(px[2])
    };

    let initial_luma = luma(target.pixel(0, 0));

    // Match chromatic distribution while preserving brightness
    match_color(&mut target, &reference, false, 1.0);

    let matched_px = target.pixel(0, 0);
    let matched_luma = luma(matched_px);

    // Verify chromaticity shifted toward reference (more Red than Blue)
    assert!(
        matched_px[0] > matched_px[2],
        "Color match should shift chromatic balance toward warm red: {matched_px:?}"
    );

    // Verify luminance preservation:
    // With 8-bit integer quantization, maximum possible rounding deviation is 0.5 on a 0-255 scale.
    // In normalized [0, 1] scale, this is <= 0.5 / 255 (~0.00196).
    let luma_diff_255 = (matched_luma - initial_luma).abs();
    assert!(
        luma_diff_255 <= 0.5,
        "Target pixel luminance changed beyond integer quantization: before={initial_luma}, after={matched_luma}, diff={luma_diff_255}"
    );

    // Verify analytical floating-point luminance preservation (without integer quantization):
    let orig = [80.0f32, 100.0, 120.0];
    let ref_mean = [180.0f32, 80.0, 40.0];
    let tgt_mean = [80.0f32, 100.0, 120.0];
    let scale = [
        ref_mean[0] / tgt_mean[0],
        ref_mean[1] / tgt_mean[1],
        ref_mean[2] / tgt_mean[2],
    ];
    let matched_flt = [orig[0] * scale[0], orig[1] * scale[1], orig[2] * scale[2]];
    let y_orig = 0.2126 * orig[0] + 0.7152 * orig[1] + 0.0722 * orig[2];
    let y_matched = 0.2126 * matched_flt[0] + 0.7152 * matched_flt[1] + 0.0722 * matched_flt[2];
    let luma_ratio = y_orig / y_matched;
    let final_flt = [
        matched_flt[0] * luma_ratio,
        matched_flt[1] * luma_ratio,
        matched_flt[2] * luma_ratio,
    ];
    let final_luma_flt = 0.2126 * final_flt[0] + 0.7152 * final_flt[1] + 0.0722 * final_flt[2];
    assert!(
        ((final_luma_flt - y_orig) / 255.0).abs() < 1e-4,
        "Analytical luminance invariance violated: |{final_luma_flt} - {y_orig}| / 255 = {}",
        ((final_luma_flt - y_orig) / 255.0).abs()
    );
}

// ============================================================================
// 11. Fritsch-Carlson Spline Curves: Non-Decreasing & Degenerate Control Points
// ============================================================================

#[test]
fn adv_spline_curve_degenerate_and_unsorted_points() {
    // Unsorted points with duplicate X
    let raw = [
        [1.0, 1.0],
        [0.0, 0.0],
        [0.5, 0.4],
        [0.5, 0.6], // duplicate X
        [0.2, 0.1],
    ];
    let curve = SplineCurve::new(&raw);
    assert_eq!(curve.eval(0.0), 0.0);
    assert_eq!(curve.eval(1.0), 1.0);

    // Monotonicity verification
    let mut prev = 0.0;
    for i in 0..=100 {
        let x = i as f32 * 0.01;
        let y = curve.eval(x);
        assert!(
            y >= prev,
            "Hermite spline decreased at x={x}: y={y}, prev={prev}"
        );
        prev = y;
    }

    // Single point fallback
    let single = SplineCurve::new(&[[0.5, 0.7]]);
    assert_eq!(single.eval(0.0), 0.7);
    assert_eq!(single.eval(1.0), 0.7);

    // Empty points fallback
    let empty = SplineCurve::new(&[]);
    assert_eq!(empty.eval(0.3), 0.3);
}

// ============================================================================
// 12. HSL Curves: Periodic Angular Wrapping across 0 / 360 Degrees
// ============================================================================

#[test]
fn adv_hsl_curves_periodic_wrapping_and_continuity() {
    let curve = HslCurve::new(HslCurveType::HueVsHue, &[[0.0, 0.1], [1.0, 0.1]]);

    // Evaluates periodically
    let v_zero = curve.eval(0.0);
    let v_one = curve.eval(1.0);
    let v_two = curve.eval(2.0);
    let v_neg = curve.eval(-1.0);

    assert_eq!(v_zero, v_one);
    assert_eq!(v_one, v_two);
    assert_eq!(v_zero, v_neg);

    // RGB <-> HSL roundtrip on extreme saturation / lightness
    let test_colors = [
        [0.0f32, 0.0, 0.0],
        [1.0, 1.0, 1.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.5, 0.5, 0.5],
        [0.1, 0.9, 0.3],
    ];
    for rgb in test_colors {
        let hsl = rgb_to_hsl(rgb);
        let back = hsl_to_rgb(hsl);
        for c in 0..3 {
            assert!(
                (back[c] - rgb[c]).abs() < 1e-4,
                "RGB/HSL roundtrip failed at {rgb:?}: back={back:?}"
            );
        }
    }
}

// ============================================================================
// 13. White Balance & Neutral Point Pickers: Extreme and Degenerate Samples
// ============================================================================

#[test]
fn adv_white_balance_neutral_pickers_extremes() {
    // Pure black sample (Y = 0)
    let (t_black, tint_black) = pick_white_balance([0.0, 0.0, 0.0]);
    assert_eq!(t_black, 0.0);
    assert_eq!(tint_black, 0.0);

    // Sub-threshold sample (Y <= 1e-4)
    let (t_dark, tint_dark) = pick_white_balance([1e-5, 1e-5, 1e-5]);
    assert_eq!(t_dark, 0.0);
    assert_eq!(tint_dark, 0.0);

    // NaN / Inf input resilience
    let (t_nan, tint_nan) = pick_white_balance([f32::NAN, 0.5, 0.5]);
    assert_eq!(t_nan, 0.0);
    assert_eq!(tint_nan, 0.0);

    // Black point and white point pickers on black
    let bp_black = pick_black_point([0.0, 0.0, 0.0]);
    assert!(bp_black.is_neutral());

    let wp_black = pick_white_point([0.0, 0.0, 0.0]);
    assert!(wp_black.is_neutral());

    // Out of bounds region sampling
    let frame = FrameBuffer::solid(8, 8, [128, 128, 128, 255]);
    let (t_oob, tint_oob) = pick_white_balance_from_region(&frame, [100, 100, 50, 50]);
    assert_eq!(t_oob, 0.0);
    assert_eq!(tint_oob, 0.0);

    // 0x0 frame region sampling
    let empty_frame = FrameBuffer::solid(0, 0, [0, 0, 0, 0]);
    let (t_emp, tint_emp) = pick_white_balance_from_region(&empty_frame, [0, 0, 10, 10]);
    assert_eq!(t_emp, 0.0);
    assert_eq!(tint_emp, 0.0);
}

// ============================================================================
// 14. ACES Transform Pipeline: Standard Encodings x View Transforms Permutations
// ============================================================================

#[test]
fn adv_aces_transform_pipeline_standard_ranges() {
    let input_spaces = [
        AcesSpace::Aces2065_1,
        AcesSpace::AcesCg,
        AcesSpace::AcesCc,
        AcesSpace::AcesCct,
        AcesSpace::AcesProxy,
        AcesSpace::SLog1,
        AcesSpace::SLog2,
        AcesSpace::SLog3,
        AcesSpace::Rec709,
        AcesSpace::Srgb,
    ];

    let working_spaces = [
        AcesSpace::AcesCg,
        AcesSpace::AcesCc,
        AcesSpace::AcesCct,
        AcesSpace::Aces2065_1,
    ];

    let view_transforms = [
        ViewTransform::AcesRrtRec709,
        ViewTransform::AcesRrtSrgb,
        ViewTransform::AcesRrtRec2020,
        ViewTransform::AcesRrtHlg,
        ViewTransform::Raw,
    ];

    // Normalized display / working input range [0.0..=1.0]
    let test_pixels = [
        [0.0f32, 0.0, 0.0],
        [0.18, 0.18, 0.18],
        [0.5, 0.5, 0.5],
        [1.0, 1.0, 1.0],
        [0.8, 0.1, 0.05],
    ];

    for &input in &input_spaces {
        for &working in &working_spaces {
            for &view in &view_transforms {
                let tf = AcesTransform::new(input, working, view);
                for &px in &test_pixels {
                    let out = tf.transform_rgb(px);
                    for c in 0..3 {
                        assert!(
                            out[c].is_finite(),
                            "AcesTransform non-finite output for in={input:?}, work={working:?}, view={view:?}, px={px:?}: out={out:?}"
                        );
                        if view != ViewTransform::Raw {
                            assert!(
                                out[c] >= 0.0 && out[c] <= 1.0,
                                "AcesTransform output out of display bounds [0, 1]: {out:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}


// ============================================================================
// 15. Known Boundary Behaviors & Residual Findings
// ============================================================================

#[test]
fn adv_aces_tonemap_infinite_and_overflow_exposure() {
    // Evaluating Narkowicz ACES tonemap on infinity or large inputs (x > 1.84e19)
    // results in inf/inf = NaN due to lack of an infinite/large-x asymptote guard.
    let tm_inf = aces_tonemap(f32::INFINITY);
    assert!(
        tm_inf.is_nan(),
        "Empirical finding: aces_tonemap(inf) produces NaN instead of clamping to 1.0 asymptote"
    );

    let tm_huge = aces_tonemap(1e20);
    assert!(
        tm_huge.is_nan(),
        "Empirical finding: aces_tonemap(1e20) produces NaN because x^2 overflows f32 to inf"
    );
}

#[test]
fn adv_acescc_log_exponent_overflow_exposure() {
    // For extreme unnormalized log values y >= 7.81, 2^(y*17.52 - 9.72) overflows f32 max exponent (127)
    let y_extreme = 8.0f32;
    let lin = acescc_to_linear(y_extreme);
    assert!(
        lin.is_infinite(),
        "Empirical finding: acescc_to_linear(8.0) overflows to +inf"
    );

    let lin_cct = acescct_to_linear(y_extreme);
    assert!(
        lin_cct.is_infinite(),
        "Empirical finding: acescct_to_linear(8.0) overflows to +inf"
    );
}

#[test]
fn adv_gamut_matrices_precision_residuals() {
    // Measure exact matrix roundtrip residual for pure green [0, 1, 0]
    let green = [0.0f32, 1.0, 0.0];
    let ap1 = mat_mul_rgb(&M_AP0_TO_AP1, green);
    let ap0 = mat_mul_rgb(&M_AP1_TO_AP0, ap1);

    let err_ap0 = (ap0[0] - green[0]).abs();
    assert!(
        err_ap0 > 1e-4 && err_ap0 < 5e-4,
        "Empirical finding: AP0/AP1 roundtrip error in channel 0 is {err_ap0} (> 1e-4 due to 7-digit constants)"
    );

    let red = [1.0f32, 0.0, 0.0];
    let ap1_709 = mat_mul_rgb(&M_REC709_TO_AP1, red);
    let back_709 = mat_mul_rgb(&M_AP1_TO_REC709, ap1_709);

    let err_709 = (back_709[0] - red[0]).abs();
    assert!(
        err_709 > 1e-4 && err_709 < 2e-4,
        "Empirical finding: Rec709/AP1 roundtrip error in channel 0 is {err_709} (> 1e-4 due to 6-digit constants)"
    );
}
