//! Empirical Adversarial Stress Test Suite for Milestone 3:
//! - Color Wheel Vectors: extreme angles, magnitudes, numpad stepping, range-limited thresholds
//! - Monotone Cubic Hermite Spline: collinearity, flat, steps, reverse, duplicates, bounds, monotonicity fuzzing
//! - 6 HSL Curves: HueVsHue 360/0 periodic wrap continuity, solid white/black/grayscale stability
//!
//! Strict sequential execution: `-j 1` and `--test-threads=1`.

use neoveg_common::grade::{
    ColorWheelChannel, ColorWheelVector, ColorWheelsGrade,
};
use neoveg_video::frame::FrameBuffer;
use neoveg_video::grading::{
    apply_hsl_curves, ColorWheelsExt, HslCurve, HslCurveType, HslCurves, SplineCurve,
};

// ============================================================================
// 1. Color Wheel Vectors & Primary Wheels
// ============================================================================

#[test]
fn adv_wheel_vector_extreme_angles() {
    // Angles: negative multiples, multiples > 360, zero, exact 360
    let test_angles = [
        -720.0f32, -540.0, -180.0, -90.0, -0.0, 0.0, 45.0, 90.0, 180.0,
        270.0, 360.0, 450.0, 720.0, 1080.0,
    ];

    for &angle in &test_angles {
        let v = ColorWheelVector::from_polar(angle, 0.75, 0.2);
        assert!(v.x.is_finite(), "x must be finite for angle {angle}");
        assert!(v.y.is_finite(), "y must be finite for angle {angle}");

        let (deg, sat, master) = v.to_polar();
        assert!(
            (sat - 0.75).abs() < 1e-4,
            "Saturation must match 0.75, got {sat} for angle {angle}"
        );
        assert!(
            (master - 0.2).abs() < 1e-4,
            "Master luma must match 0.2, got {master}"
        );

        // Expected normalized angle in [0..360)
        let expected_deg = angle.rem_euclid(360.0);
        let diff = (deg - expected_deg).abs();
        let circular_diff = diff.min(360.0 - diff);
        assert!(
            circular_diff < 1e-3,
            "Angle mismatch for {angle}: got {deg}, expected {expected_deg}"
        );
    }
}

#[test]
fn adv_wheel_vector_to_polar_boundary_360_bug() {
    // REMEDIATED: ColorWheelVector::to_polar() contract states `hue_deg in [0.0..360.0)`.
    // When hue_deg = -360.0, floating-point rounding is clamped so deg returns strictly in [0.0..360.0).
    let v = ColorWheelVector::from_polar(-360.0, 0.75, 0.2);
    let (deg, sat, master) = v.to_polar();
    assert!(
        deg >= 0.0 && deg < 360.0,
        "to_polar() must strictly return in [0.0..360.0), got {deg}"
    );
    assert_eq!(deg, 0.0);
    assert!((sat - 0.75).abs() < 1e-4);
    assert!((master - 0.2).abs() < 1e-4);
}

#[test]
fn adv_wheel_vector_extreme_magnitudes_and_sanitization() {
    // Magnitude > 1.0
    let v_over = ColorWheelVector::from_polar(90.0, 5.0, 2.5);
    let (_, sat_over, master_over) = v_over.to_polar();
    assert_eq!(sat_over, 1.0, "Saturation > 1.0 must clamp to 1.0");
    assert_eq!(master_over, 1.0, "Master luma > 1.0 must clamp to 1.0");

    // Negative magnitude
    let v_neg = ColorWheelVector::from_polar(45.0, -1.0, -3.0);
    let (_, sat_neg, master_neg) = v_neg.to_polar();
    assert_eq!(sat_neg, 0.0, "Negative saturation must clamp to 0.0");
    assert_eq!(master_neg, -1.0, "Master luma < -1.0 must clamp to -1.0");

    // Non-finite (NaN / Infinity) inputs
    let v_nan = ColorWheelVector::from_polar(f32::NAN, f32::NAN, f32::NAN);
    let sanitized = v_nan.sanitized();
    assert!(sanitized.is_neutral(), "NaN vector after sanitized() must become neutral");

    let v_inf = ColorWheelVector::from_polar(0.0, f32::INFINITY, f32::NEG_INFINITY);
    let sanitized_inf = v_inf.sanitized();
    assert!(sanitized_inf.is_neutral(), "Inf vector after sanitized() must become neutral");

    // Direct NaN field injection
    let mut v_dirty = ColorWheelVector {
        x: f32::NAN,
        y: f32::INFINITY,
        master: f32::NEG_INFINITY,
    };
    v_dirty = v_dirty.sanitized();
    assert_eq!(v_dirty.x, 0.0);
    assert_eq!(v_dirty.y, 0.0);
    assert_eq!(v_dirty.master, 0.0);
}

#[test]
fn adv_wheel_vector_from_polar_nan_hue_bug() {
    // REMEDIATED: from_polar protects hue_deg against NaN (fallback to 0.0).
    let v = ColorWheelVector::from_polar(f32::NAN, 0.5, 0.0);
    assert!(v.x.is_finite(), "hue_deg = NaN must produce finite x, got {}", v.x);
    assert!(v.y.is_finite(), "hue_deg = NaN must produce finite y, got {}", v.y);
    assert_eq!(v.x, 0.5);
    assert_eq!(v.y, 0.0);

    // Also verify from_rgb_sliders protects r, g, b against NaN:
    let v_rgb = ColorWheelVector::from_rgb_sliders(f32::NAN, f32::NAN, f32::NAN, f32::NAN);
    assert!(v_rgb.x.is_finite());
    assert!(v_rgb.y.is_finite());
    assert!(v_rgb.master.is_finite());
}

#[test]
fn adv_wheel_vector_numpad_adjustments_wrapping_and_clamping() {
    let mut v = ColorWheelVector::neutral();

    // 1. Clamping at upper boundary: Red +2.0
    v.adjust_channel(ColorWheelChannel::Red, 2.0);
    assert_eq!(v.x, 1.0, "Red displacement must clamp at +1.0");

    // 2. Clamping at lower boundary: Cyan +2.0 (Cyan subtracts from x)
    v.adjust_channel(ColorWheelChannel::Cyan, 2.5);
    assert_eq!(v.x, -1.0, "Cyan displacement must clamp at -1.0");

    // 3. Sequential additions and exact cancellation
    v.reset();
    assert!(v.is_neutral());
    v.adjust_red(0.35);
    v.adjust_cyan(0.35);
    assert!(v.x.abs() < 1e-6, "Equal Red and Cyan adjustments must cancel to 0.0, got {}", v.x);

    // 4. Perpendicular axis: Green and Magenta
    v.reset();
    v.adjust_green(0.4);
    assert!(v.y > 0.0);
    v.adjust_magenta(0.4);
    assert!(v.y.abs() < 1e-5, "Equal Green and Magenta adjustments must cancel y to 0.0");

    // 5. Blue and Yellow cancellation
    v.reset();
    v.adjust_blue(0.5);
    assert!(v.y < 0.0);
    v.adjust_yellow(0.5);
    assert!(v.y.abs() < 1e-5, "Equal Blue and Yellow adjustments must cancel y to 0.0");

    // 6. Master luma clamping
    v.reset();
    v.adjust_luma(5.0);
    assert_eq!(v.master, 1.0);
    v.adjust_luma(-10.0);
    assert_eq!(v.master, -1.0);
}

#[test]
fn adv_wheel_range_limited_inverted_and_extreme_thresholds() {
    let mut wheels = ColorWheelsGrade::default_neutral();
    wheels.lift.master = 0.2;
    wheels.gain.master = -0.2;

    // Test A: Inverted thresholds (T_low = 0.9 > T_high = 0.1)
    let mut frame = FrameBuffer::solid(4, 4, [128, 128, 128, 255]);
    wheels.apply_range_limited(&mut frame, 0.9, 0.1);
    for px in frame.rgba.as_chunks::<4>().0 {
        assert!(px[0] > 0 && px[0] < 255, "Inverted thresholds must not crash or produce black/blown pixels");
        assert_eq!(px[3], 255, "Alpha must remain untouched");
    }

    // Test B: Negative and extreme thresholds (< 0.0 and > 1.0)
    let mut frame_extreme = FrameBuffer::solid(4, 4, [100, 100, 100, 255]);
    wheels.apply_range_limited(&mut frame_extreme, -5.0, 10.0);
    for px in frame_extreme.rgba.as_chunks::<4>().0 {
        assert_eq!(px[3], 255);
    }
}

#[test]
fn adv_wheel_range_limited_nan_threshold_bug() {
    // REMEDIATED: NaN thresholds in apply_range_limited() safely fall back to 0.25 and 0.75,
    // preventing NaN propagation and protecting frame content.
    let mut wheels = ColorWheelsGrade::default_neutral();
    wheels.lift.master = 0.2;
    let mut frame_nan = FrameBuffer::solid(4, 4, [80, 80, 80, 255]);
    wheels.apply_range_limited(&mut frame_nan, f32::NAN, f32::NAN);
    let p_nan = frame_nan.pixel(0, 0);
    assert_ne!(
        p_nan,
        [0, 0, 0, 255],
        "NaN thresholds must not corrupt frame to solid pitch black"
    );
    assert!(p_nan[0] > 0);
}

// ============================================================================
// 2. Monotone Cubic Hermite Spline (`SplineCurve`)
// ============================================================================

#[test]
fn adv_spline_collinear_flat_steps_and_reverse_order() {
    // 1. Collinear points (exact line y = x)
    let collinear_pts = [
        [0.0, 0.0],
        [0.2, 0.2],
        [0.4, 0.4],
        [0.6, 0.6],
        [0.8, 0.8],
        [1.0, 1.0],
    ];
    let spline_collinear = SplineCurve::new(&collinear_pts);
    for i in 0..=100 {
        let x = i as f32 * 0.01;
        let y = spline_collinear.eval(x);
        assert!(
            (y - x).abs() < 1e-4,
            "Collinear points must evaluate to exact identity y=x: x={x}, y={y}"
        );
    }

    // 2. Flat horizontal curve (y = 0.45 everywhere)
    let flat_pts = [
        [0.0, 0.45],
        [0.25, 0.45],
        [0.5, 0.45],
        [0.75, 0.45],
        [1.0, 0.45],
    ];
    let spline_flat = SplineCurve::new(&flat_pts);
    for i in 0..=100 {
        let x = i as f32 * 0.01;
        let y = spline_flat.eval(x);
        assert!(
            (y - 0.45).abs() < 1e-5,
            "Flat curve must evaluate to 0.45 everywhere: x={x}, y={y}"
        );
    }

    // 3. Vertical-like step curve: jump from 0.0 to 1.0 over delta_x = 0.001
    let step_pts = [
        [0.0, 0.0],
        [0.499, 0.0],
        [0.501, 1.0],
        [1.0, 1.0],
    ];
    let spline_step = SplineCurve::new(&step_pts);
    assert_eq!(spline_step.eval(0.25), 0.0);
    assert_eq!(spline_step.eval(0.499), 0.0);
    assert_eq!(spline_step.eval(0.501), 1.0);
    assert_eq!(spline_step.eval(0.75), 1.0);
    let y_mid = spline_step.eval(0.5);
    assert!((0.0..=1.0).contains(&y_mid), "Step interpolation must not overshoot [0..1]: got {y_mid}");

    // 4. Reverse order input points
    let reverse_pts = [
        [1.0, 1.0],
        [0.75, 0.85],
        [0.5, 0.5],
        [0.25, 0.15],
        [0.0, 0.0],
    ];
    let sorted_pts = [
        [0.0, 0.0],
        [0.25, 0.15],
        [0.5, 0.5],
        [0.75, 0.85],
        [1.0, 1.0],
    ];
    let spline_rev = SplineCurve::new(&reverse_pts);
    let spline_fwd = SplineCurve::new(&sorted_pts);
    for i in 0..=50 {
        let x = i as f32 * 0.02;
        assert_eq!(
            spline_rev.eval(x),
            spline_fwd.eval(x),
            "Reverse points must evaluate identically to forward points at x={x}"
        );
    }
}

#[test]
fn adv_spline_duplicates_near_duplicates_and_empty() {
    // 1. Exact duplicate x-coordinates
    let dup_pts = [
        [0.0, 0.0],
        [0.5, 0.2],
        [0.5, 0.8], // duplicate x=0.5
        [1.0, 1.0],
    ];
    let s_dup = SplineCurve::new(&dup_pts);
    assert_eq!(s_dup.points().len(), 3, "Duplicate x must be collapsed");
    assert_eq!(s_dup.points()[1][1], 0.8, "Later point y must overwrite earlier duplicate");
    assert!(s_dup.eval(0.5).is_finite());

    // 2. Near-duplicate x-coordinates (|x1 - x2| < 1e-4)
    let near_pts = [
        [0.0, 0.0],
        [0.5, 0.3],
        [0.5 + 5e-5, 0.7],
        [1.0, 1.0],
    ];
    let s_near = SplineCurve::new(&near_pts);
    assert_eq!(s_near.points().len(), 3, "Near-duplicate |dx| < 1e-4 must be collapsed");

    // 3. Near-duplicate x-coordinates just above threshold (|x1 - x2| = 1.1e-4)
    let just_above = [
        [0.0, 0.0],
        [0.5, 0.3],
        [0.5 + 1.1e-4, 0.7],
        [1.0, 1.0],
    ];
    let s_above = SplineCurve::new(&just_above);
    assert_eq!(s_above.points().len(), 4, "Points with |dx| >= 1e-4 must be kept");
    for i in 0..=20 {
        let x = i as f32 * 0.05;
        let y = s_above.eval(x);
        assert!(y.is_finite(), "Evaluation near threshold points must be finite, got {y} at {x}");
    }

    // 4. Single point
    let s_single = SplineCurve::new(&[[0.5, 0.77]]);
    assert_eq!(s_single.eval(0.0), 0.77);
    assert_eq!(s_single.eval(0.5), 0.77);
    assert_eq!(s_single.eval(1.0), 0.77);

    // 5. Empty points
    let s_empty = SplineCurve::new(&[]);
    assert_eq!(s_empty.eval(0.25), 0.25);
    assert_eq!(s_empty.eval(0.8), 0.8);
}

#[test]
fn adv_spline_boundary_values_and_extrapolation() {
    let pts = [[0.0, 0.1], [0.5, 0.5], [1.0, 0.9]];
    let spline = SplineCurve::new(&pts);

    // Below 0.0
    assert_eq!(spline.eval(-0.5), 0.1, "x < 0.0 must clamp to first control point y");
    assert_eq!(spline.eval(-1000.0), 0.1);

    // Above 1.0
    assert_eq!(spline.eval(1.5), 0.9, "x > 1.0 must clamp to last control point y");
    assert_eq!(spline.eval(1000.0), 0.9);

    // Infinity handling
    assert_eq!(spline.eval(f32::NEG_INFINITY), 0.1);
    assert_eq!(spline.eval(f32::INFINITY), 0.9);
}

#[test]
fn adv_spline_nan_eval_underflow_panic_bug() {
    // REMEDIATED: SplineCurve::eval(f32::NAN) guards against non-finite floats,
    // safely returning the first control point y without underflow panic.
    let pts = [[0.0, 0.1], [0.5, 0.5], [1.0, 0.9]];
    let spline = SplineCurve::new(&pts);

    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        spline.eval(f32::NAN)
    }));

    assert!(
        res.is_ok(),
        "SplineCurve::eval(f32::NAN) must not panic with attempt to subtract with overflow"
    );
    assert_eq!(spline.eval(f32::NAN), 0.1);
}

#[test]
fn adv_spline_monotonicity_stress_fuzzing_10000_samples() {
    // Deterministic pseudo-random linear congruential generator (LCG)
    // to keep tests 100% reproducible and dependency-free.
    let mut seed = 123456789u64;
    let mut next_rand = || -> f32 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((seed >> 32) as u32 as f32) / (u32::MAX as f32)
    };

    // Test 50 distinct random monotonically non-decreasing curves
    for curve_idx in 0..50 {
        let num_pts = 3 + (next_rand() * 6.0) as usize; // 3 to 8 points
        let mut xs = Vec::with_capacity(num_pts);
        let mut ys = Vec::with_capacity(num_pts);

        xs.push(0.0);
        ys.push(next_rand() * 0.2); // y0 in [0..0.2]

        for _ in 1..num_pts - 1 {
            xs.push(next_rand());
            ys.push(next_rand());
        }
        xs.push(1.0);
        ys.push(0.8 + next_rand() * 0.2); // yn in [0.8..1.0]

        xs.sort_by(|a, b| a.total_cmp(b));
        ys.sort_by(|a, b| a.total_cmp(b)); // strictly non-decreasing control points

        let mut pts = Vec::with_capacity(num_pts);
        for i in 0..num_pts {
            pts.push([xs[i], ys[i]]);
        }

        let spline = SplineCurve::new(&pts);

        // Verify across 200 sequentially increasing evaluation points (50 curves * 200 = 10,000 evaluations)
        let mut prev_y = spline.eval(0.0);
        assert!((0.0..=1.0).contains(&prev_y));

        for step in 1..=200 {
            let x = step as f32 / 200.0;
            let y = spline.eval(x);

            assert!(
                y >= prev_y - 1e-5,
                "Curve {curve_idx} violated monotonicity at x={x}: y={y} < prev_y={prev_y}"
            );
            assert!(
                (0.0..=1.0).contains(&y),
                "Curve {curve_idx} exceeded bounds at x={x}: y={y}"
            );
            prev_y = y;
        }
    }
}

// ============================================================================
// 3. 6 HSL Curves: Periodic Wrapping & Grayscale Stability
// ============================================================================

#[test]
fn adv_hsl_hue_vs_hue_continuity_and_red_boundary_wrap() {
    // Hue vs Hue identity curve: [[0.0, 0.0], [1.0, 1.0]]
    let curve = HslCurve::new(HslCurveType::HueVsHue, &[[0.0, 0.0], [1.0, 1.0]]);

    // Near 0.0 (red) and near 1.0 (red 360 deg)
    let y_zero_plus = curve.eval(0.001); // ~0.36 degrees
    let y_one_minus = curve.eval(0.999); // ~359.64 degrees
    assert!((y_zero_plus - 0.001).abs() < 1e-3);
    assert!((y_one_minus - 0.999).abs() < 1e-3);

    // Periodic modular evaluations: x = 1.001 should equal x = 0.001
    let y_wrap_pos = curve.eval(1.001);
    assert!((y_wrap_pos - y_zero_plus).abs() < 1e-4);

    // Negative angle: x = -0.001 should equal x = 0.999
    let y_wrap_neg = curve.eval(-0.001);
    assert!((y_wrap_neg - y_one_minus).abs() < 1e-4);

    // Extreme angle multiples
    let y_wrap_large = curve.eval(10.25);
    assert!((y_wrap_large - curve.eval(0.25)).abs() < 1e-4);

    // Test RGB Frame application across red boundary
    // Red 1: hue ~ 1 degree: [255, 4, 0]
    let mut frame_red1 = FrameBuffer::solid(2, 2, [255, 4, 0, 255]);
    // Red 2: hue ~ 359 degrees: [255, 0, 4]
    let mut frame_red2 = FrameBuffer::solid(2, 2, [255, 0, 4, 255]);

    let curves = HslCurves::default();
    apply_hsl_curves(&mut frame_red1, &curves);
    apply_hsl_curves(&mut frame_red2, &curves);

    // Default identity curves must preserve red colors smoothly without boundary jump
    let p1 = frame_red1.pixel(0, 0);
    let p2 = frame_red2.pixel(0, 0);
    assert_eq!(p1[0], 255);
    assert_eq!(p2[0], 255);
    assert!((p1[1] as i32 - p2[2] as i32).abs() <= 1);
}

#[test]
fn adv_hsl_curves_solid_black_white_and_grayscale_stability() {
    let mut curves = HslCurves::default();
    // Aggressive curve adjustments on all 6 channels:
    // HueVsHue: shift
    curves.curves[0] = HslCurve::new(HslCurveType::HueVsHue, &[[0.0, 0.2], [1.0, 0.2]]);
    // HueVsSat: max boost
    curves.curves[1] = HslCurve::new(HslCurveType::HueVsSat, &[[0.0, 1.0], [1.0, 1.0]]);
    // LumVsSat: boost
    curves.curves[3] = HslCurve::new(HslCurveType::LumVsSat, &[[0.0, 0.8], [1.0, 0.8]]);
    // SatVsSat: preserve
    curves.curves[4] = HslCurve::new(HslCurveType::SatVsSat, &[[0.0, 0.0], [1.0, 1.0]]);

    // 1. Solid Black frame
    let mut black_frame = FrameBuffer::solid(4, 4, [0, 0, 0, 255]);
    apply_hsl_curves(&mut black_frame, &curves);
    let p_black = black_frame.pixel(0, 0);
    assert_eq!(p_black, [0, 0, 0, 255], "Solid black must remain black through curves");

    // 2. Solid White frame
    let mut white_frame = FrameBuffer::solid(4, 4, [255, 255, 255, 255]);
    apply_hsl_curves(&mut white_frame, &curves);
    let p_white = white_frame.pixel(0, 0);
    assert_eq!(p_white, [255, 255, 255, 255], "Solid white must remain white through curves");

    // 3. Grayscale gradient (saturation = 0 for all pixels)
    // With HueVsSat and SatVsSat default (sat 0 stays 0), gray pixels must remain perfectly neutral gray
    let mut neutral_curves = HslCurves::default();
    neutral_curves.curves[1] = HslCurve::new(HslCurveType::HueVsSat, &[[0.0, 1.0], [1.0, 1.0]]); // Boost sat for all hues

    let mut gray_frame = FrameBuffer::solid(256, 1, [0, 0, 0, 255]);
    for x in 0..256 {
        let v = x as u8;
        let i = x as usize * 4;
        gray_frame.rgba[i..i + 4].copy_from_slice(&[v, v, v, 255]);
    }

    apply_hsl_curves(&mut gray_frame, &neutral_curves);

    for x in 0..256 {
        let px = gray_frame.pixel(x, 0);
        assert_eq!(
            px[0], px[1],
            "Grayscale neutrality violated at x={x}: R={} != G={}",
            px[0], px[1]
        );
        assert_eq!(
            px[1], px[2],
            "Grayscale neutrality violated at x={x}: G={} != B={}",
            px[1], px[2]
        );
        assert_eq!(px[3], 255);
    }
}

#[test]
fn adv_hsl_curves_hue_vs_lum_achromatic_discontinuity() {
    // REMEDIATED: HueVsLum luminance delta is weighted by saturation (hsl[1]),
    // eliminating threshold posterization on neutral gray / achromatic pixels.
    let mut curves = HslCurves::default();
    curves.curves[2] = HslCurve::new(
        HslCurveType::HueVsLum,
        &[[0.0, 0.8], [0.1, 0.5], [0.9, 0.5], [1.0, 0.8]],
    );

    // Pixel A: exact neutral gray (128, 128, 128) -> rgb_to_hsl assigns hue = 0.0, sat = 0.0
    let mut frame_gray = FrameBuffer::solid(1, 1, [128, 128, 128, 255]);
    // Pixel B: nearly neutral gray with green +1 (128, 129, 128) -> rgb_to_hsl assigns hue = 120.0, sat ~ 0.0039
    let mut frame_near_gray = FrameBuffer::solid(1, 1, [128, 129, 128, 255]);

    apply_hsl_curves(&mut frame_gray, &curves);
    apply_hsl_curves(&mut frame_near_gray, &curves);

    let p_gray = frame_gray.pixel(0, 0);
    let p_near = frame_near_gray.pixel(0, 0);

    // Neutral gray has saturation 0.0, so HueVsLum delta is 0; it remains 128:
    assert_eq!(p_gray, [128, 128, 128, 255]);
    // Near-gray has tiny saturation, so delta is negligible; it remains within 1 level:
    assert!((p_near[0] as i32 - 128).abs() <= 1);
    assert!((p_near[1] as i32 - 129).abs() <= 1);

    let luma_gray = 0.2126 * p_gray[0] as f32 + 0.7152 * p_gray[1] as f32 + 0.0722 * p_gray[2] as f32;
    let luma_near = 0.2126 * p_near[0] as f32 + 0.7152 * p_near[1] as f32 + 0.0722 * p_near[2] as f32;
    assert!(
        (luma_gray - luma_near).abs() < 2.0,
        "Achromatic discontinuity eliminated: luma diff is {}",
        (luma_gray - luma_near).abs()
    );
}
