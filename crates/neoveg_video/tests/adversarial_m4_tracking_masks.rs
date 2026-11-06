//! Empirical Adversarial Stress Test Suite for Milestone 4:
//! - Feature 17 (Motion Tracking): Degenerate point sets (Perspective singular/collinear/coincident),
//!   off-screen ROIs, extreme rapid motion & scene cuts, non-finite timestamps & NaN coordinates.
//! - Feature 18 (Geometric & Bézier Event Masks): Massive feather radius, self-intersecting Bézier
//!   figure-8 loops, empty keys, out-of-bounds opacity (> 1.0, < 0.0, NaN), In/Out/Both feather bounds.
//! - Feature 19 (Smart Mask): Conflicting prompt points, zero-area bboxes, pure black/white/noise frames,
//!   vectorize_to_bezier stability.
//!
//! Strict sequential execution: `-j 1` and `--test-threads=1`.

use neoveg_common::fx_mask::{FxMask, MaskShape, MAX_POINTS, MIN_POINTS};
use neoveg_common::mask::{EventMask, EventMaskKey, MaskFeatherType};
use neoveg_video::frame::FrameBuffer;
use neoveg_video::mask::{apply_event_mask, compute_event_mask_coverage};
use neoveg_video::smart_mask::{
    SegmentationPrompt, SmartMaskMatte, SmartMaskPrecision,
};
use neoveg_video::tracking::{
    MotionTrajectory, TrackKeyframe, TrackingMode, TrackingPrecision, TrackingRegion,
};

// ============================================================================
// Helper Utilities
// ============================================================================

fn make_test_frame(w: u32, h: u32, pattern: &str) -> FrameBuffer {
    let mut frame = FrameBuffer::solid(w, h, [0, 0, 0, 255]);
    match pattern {
        "black" => {}
        "white" => {
            frame = FrameBuffer::solid(w, h, [255, 255, 255, 255]);
        }
        "noise" => {
            let mut lcg: u32 = 123456789;
            for px in frame.rgba.as_chunks_mut::<4>().0 {
                lcg = lcg.wrapping_mul(1664525).wrapping_add(1013904223);
                let r = (lcg >> 24) as u8;
                lcg = lcg.wrapping_mul(1664525).wrapping_add(1013904223);
                let g = (lcg >> 24) as u8;
                lcg = lcg.wrapping_mul(1664525).wrapping_add(1013904223);
                let b = (lcg >> 24) as u8;
                px[0] = r;
                px[1] = g;
                px[2] = b;
                px[3] = 255;
            }
        }
        "checker" => {
            for y in 0..h {
                for x in 0..w {
                    let idx = (y * w + x) as usize * 4;
                    let val = if ((x / 8) + (y / 8)) % 2 == 0 { 240 } else { 20 };
                    frame.rgba[idx] = val;
                    frame.rgba[idx + 1] = val;
                    frame.rgba[idx + 2] = val;
                    frame.rgba[idx + 3] = 255;
                }
            }
        }
        _ => {}
    }
    frame
}

// ============================================================================
// Feature 17: Motion Tracking Adversarial Verification
// ============================================================================

#[test]
fn adv_tracking_degenerate_coincident_points_perspective() {
    // 4 coincident corner pins (all identical [0.5, 0.5])
    let coincident_pins = [[0.5f32, 0.5f32]; 4];
    let kf0 = TrackKeyframe {
        time_s: 0.0,
        center_x: 0.5,
        center_y: 0.5,
        scale_x: 1.0,
        scale_y: 1.0,
        rotation_deg: 0.0,
        confidence: 1.0,
        corner_pins: Some(coincident_pins),
    };
    let kf1 = TrackKeyframe {
        time_s: 1.0,
        center_x: 0.5,
        center_y: 0.5,
        scale_x: 1.0,
        scale_y: 1.0,
        rotation_deg: 0.0,
        confidence: 1.0,
        corner_pins: Some(coincident_pins),
    };

    let mut traj = MotionTrajectory::new(
        TrackingMode::Perspective,
        TrackingRegion::new(0.25, 0.25, 0.5, 0.5),
    );
    traj.keyframes.push(kf0);
    traj.keyframes.push(kf1);

    // Sample across timestamps
    let sample_mid = traj.sample_at(0.5).expect("must sample");
    assert_eq!(sample_mid.corner_pins, Some(coincident_pins));
    assert!(sample_mid.center_x.is_finite());
    assert!(sample_mid.center_y.is_finite());

    // Convert to Pan/Crop & Mask keys
    let pc = traj.to_pan_crop();
    assert_eq!(pc.keys.len(), 2);
    for key in &pc.keys {
        assert!(key.frame.width > 0.0 && key.frame.width.is_finite());
        assert!(key.frame.height > 0.0 && key.frame.height.is_finite());
    }

    let mask_keys = traj.to_mask_keys(&FxMask::default());
    assert_eq!(mask_keys.len(), 2);
}

#[test]
fn adv_tracking_degenerate_collinear_points_perspective() {
    // 4 collinear corner pins along diagonal y = x
    let collinear_pins = [
        [0.2f32, 0.2f32],
        [0.4f32, 0.4f32],
        [0.6f32, 0.6f32],
        [0.8f32, 0.8f32],
    ];
    let kf0 = TrackKeyframe {
        time_s: 0.0,
        center_x: 0.5,
        center_y: 0.5,
        scale_x: 1.0,
        scale_y: 1.0,
        rotation_deg: 0.0,
        confidence: 0.8,
        corner_pins: Some(collinear_pins),
    };
    let kf1 = TrackKeyframe {
        time_s: 2.0,
        center_x: 0.6,
        center_y: 0.6,
        scale_x: 1.1,
        scale_y: 1.1,
        rotation_deg: 10.0,
        confidence: 0.7,
        corner_pins: Some(collinear_pins),
    };

    let mut traj = MotionTrajectory::new(
        TrackingMode::Perspective,
        TrackingRegion::new(0.2, 0.2, 0.6, 0.6),
    );
    traj.keyframes.push(kf0);
    traj.keyframes.push(kf1);

    let sample = traj.sample_at(1.0).expect("must sample");
    let pins = sample.corner_pins.expect("pins must be present");
    for pin in &pins {
        assert!(pin[0].is_finite());
        assert!(pin[1].is_finite());
    }
}

#[test]
fn adv_tracking_offscreen_bounding_box_clamping() {
    // 1. Clamping via TrackingRegion::new
    let r1 = TrackingRegion::new(-2.0, -5.0, 10.0, 15.0);
    assert_eq!(r1.x, 0.0);
    assert_eq!(r1.y, 0.0);
    assert_eq!(r1.width, 1.0);
    assert_eq!(r1.height, 1.0);

    let r2 = TrackingRegion::new(2.5, 3.0, -1.0, -0.5);
    assert_eq!(r2.x, 1.0);
    assert_eq!(r2.y, 1.0);
    assert_eq!(r2.width, 0.01);
    assert_eq!(r2.height, 0.01);

    // 2. Unclamped raw struct passed to track_step: must not panic or read out-of-bounds
    let raw_region = TrackingRegion {
        x: -5.0,
        y: -10.0,
        width: 20.0,
        height: 20.0,
    };
    let mut traj = MotionTrajectory::new(TrackingMode::Location, raw_region);

    let frame_a = make_test_frame(32, 32, "checker");
    let frame_b = make_test_frame(32, 32, "checker");

    let kf = traj.track_step(
        &frame_a,
        &frame_b,
        0.0,
        0.1,
        TrackingPrecision::Accurate,
    );
    // Center must be clamped within [0.0, 1.0]
    assert!(kf.center_x >= 0.0 && kf.center_x <= 1.0);
    assert!(kf.center_y >= 0.0 && kf.center_y <= 1.0);
    assert!(kf.confidence.is_finite());
}

#[test]
fn adv_tracking_extreme_rapid_motion_and_scene_cuts() {
    let frame_a = make_test_frame(64, 64, "checker");
    let frame_b = make_test_frame(64, 64, "noise"); // Sudden scene cut to noise

    let mut traj = MotionTrajectory::new(
        TrackingMode::LocationAndRotation,
        TrackingRegion::new(0.3, 0.3, 0.4, 0.4),
    );

    for prec in [
        TrackingPrecision::Fast,
        TrackingPrecision::Balanced,
        TrackingPrecision::Accurate,
    ] {
        let kf = traj.track_step(&frame_a, &frame_b, 0.0, 1.0, prec);
        // Robustness: must not panic, confidence must be low/bounded, values finite
        assert!(kf.center_x.is_finite());
        assert!(kf.center_y.is_finite());
        assert!(kf.rotation_deg.is_finite());
        assert!(kf.confidence >= 0.0 && kf.confidence <= 1.0);
    }
}

#[test]
fn adv_tracking_non_finite_timestamps_and_nan_coordinates() {
    let mut traj = MotionTrajectory::new(
        TrackingMode::LocationAndScale,
        TrackingRegion::default(),
    );

    // Empty trajectory
    assert!(traj.sample_at(0.0).is_none());
    assert!(traj.sample_at(f64::NAN).is_none());
    assert!(traj.sample_at(f64::INFINITY).is_none());
    assert!(traj.sample_at(f64::NEG_INFINITY).is_none());

    // Single keyframe with NaN coordinates
    traj.keyframes.push(TrackKeyframe {
        time_s: 0.0,
        center_x: f32::NAN,
        center_y: f32::NAN,
        scale_x: f32::NAN,
        scale_y: f32::NAN,
        rotation_deg: f32::NAN,
        confidence: f32::NAN,
        corner_pins: None,
    });

    // sample_at on single keyframe must return it without panicking
    let sampled = traj.sample_at(f64::NAN).expect("single key returns");
    assert!(sampled.center_x.is_nan());

    // Pan/crop conversion must not panic
    let pc = traj.to_pan_crop();
    assert_eq!(pc.keys.len(), 1);

    // Mask conversion must not panic
    let mask_keys = traj.to_mask_keys(&FxMask::default());
    assert_eq!(mask_keys.len(), 1);

    // Multi-keyframe with non-finite query timestamps
    traj.keyframes.push(TrackKeyframe {
        time_s: 2.0,
        center_x: 0.8,
        center_y: 0.8,
        scale_x: 1.0,
        scale_y: 1.0,
        rotation_deg: 0.0,
        confidence: 1.0,
        corner_pins: None,
    });

    let s_nan = traj.sample_at(f64::NAN);
    assert!(s_nan.is_some());
    let s_inf = traj.sample_at(f64::INFINITY);
    assert!(s_inf.is_some());
    let s_neginf = traj.sample_at(f64::NEG_INFINITY);
    assert!(s_neginf.is_some());
}

#[test]
fn adv_tracking_zero_dimension_and_mismatched_frames() {
    let mut traj = MotionTrajectory::new(
        TrackingMode::Perspective,
        TrackingRegion::default(),
    );

    let zero_w = FrameBuffer {
        width: 0,
        height: 32,
        rgba: Vec::new(),
    };
    let valid_frame = make_test_frame(32, 32, "white");
    let diff_frame = make_test_frame(64, 64, "white");

    // Zero width frame
    let kf1 = traj.track_step(
        &zero_w,
        &valid_frame,
        0.0,
        0.1,
        TrackingPrecision::Balanced,
    );
    assert_eq!(kf1.confidence, 0.0);

    // Mismatched dimensions
    let kf2 = traj.track_step(
        &valid_frame,
        &diff_frame,
        0.1,
        0.2,
        TrackingPrecision::Balanced,
    );
    assert_eq!(kf2.confidence, 0.0);
}

// ============================================================================
// Feature 18: Geometric & Bézier Event Masks Adversarial Verification
// ============================================================================

#[test]
fn adv_mask_massive_feather_radius_exceeding_dimensions() {
    let test_feathers = [
        0.5f32,
        1.0,
        10.0,
        1000.0,
        f32::INFINITY,
        f32::NAN,
        -5.0,
    ];

    for &f in &test_feathers {
        let mask = FxMask {
            shape: MaskShape::Rectangle,
            center: [0.5, 0.5],
            size: [0.5, 0.5],
            rotation_deg: 0.0,
            feather: f,
            invert: false,
            points: Vec::new(),
        };

        let cov = compute_event_mask_coverage(
            &mask,
            32,
            32,
            MaskFeatherType::Both,
            false,
        );
        assert_eq!(cov.len(), 32 * 32);
        for &val in &cov {
            assert!(
                (0.0..=1.0).contains(&val) && val.is_finite(),
                "Coverage must be bounded and finite for feather={f}, got {val}"
            );
        }

        // Apply to actual frame
        let mut frame = FrameBuffer::solid(16, 16, [200, 100, 50, 255]);
        let event_mask = EventMask {
            enabled: true,
            invert: false,
            keys: vec![EventMaskKey {
                time: 0.0,
                mask,
                opacity: 1.0,
                feather_type: MaskFeatherType::Both,
            }],
        };
        apply_event_mask(&mut frame, &event_mask, 0.0);
        for px in frame.rgba.as_chunks::<4>().0 {
            // Verify frame data is intact and alpha is accessible
            let _alpha = px[3];
        }
    }
}

#[test]
fn adv_mask_self_intersecting_bezier_spline_figure_8() {
    // Figure-8 self-intersecting Bézier spline
    let figure_8_points = vec![
        [0.2f32, 0.2f32],
        [0.8f32, 0.8f32],
        [0.8f32, 0.2f32],
        [0.2f32, 0.8f32],
    ];

    let mask = FxMask {
        shape: MaskShape::Bezier,
        center: [0.5, 0.5],
        size: [1.0, 1.0],
        rotation_deg: 0.0,
        feather: 0.1,
        invert: false,
        points: figure_8_points,
    };

    for feather_type in [
        MaskFeatherType::None,
        MaskFeatherType::In,
        MaskFeatherType::Out,
        MaskFeatherType::Both,
    ] {
        let cov = compute_event_mask_coverage(&mask, 48, 48, feather_type, false);
        assert_eq!(cov.len(), 48 * 48);
        for &v in &cov {
            assert!((0.0..=1.0).contains(&v) && v.is_finite());
        }

        let mut frame = FrameBuffer::solid(48, 48, [255, 255, 255, 255]);
        let event_mask = EventMask {
            enabled: true,
            invert: false,
            keys: vec![EventMaskKey {
                time: 0.0,
                mask: mask.clone(),
                opacity: 1.0,
                feather_type,
            }],
        };
        apply_event_mask(&mut frame, &event_mask, 0.0);
    }
}

#[test]
fn adv_mask_empty_keys_and_out_of_bounds_opacity() {
    let mut frame = FrameBuffer::solid(16, 16, [100, 100, 100, 255]);

    // 1. Empty keys: must be a safe no-op
    let empty_mask = EventMask {
        enabled: true,
        invert: false,
        keys: Vec::new(),
    };
    apply_event_mask(&mut frame, &empty_mask, 0.0);
    assert_eq!(frame.pixel(8, 8)[3], 255);

    // 2. Opacity < 0.0: must clamp to fully transparent
    let mut frame_neg = FrameBuffer::solid(16, 16, [100, 100, 100, 255]);
    let neg_mask = EventMask {
        enabled: true,
        invert: false,
        keys: vec![EventMaskKey {
            time: 0.0,
            mask: FxMask::default(),
            opacity: -2.5,
            feather_type: MaskFeatherType::None,
        }],
    };
    apply_event_mask(&mut frame_neg, &neg_mask, 0.0);
    assert_eq!(frame_neg.pixel(8, 8)[3], 0);

    // 3. Opacity > 1.0: must clamp factor to 1.0 without overflowing alpha 255
    let mut frame_high = FrameBuffer::solid(16, 16, [100, 100, 100, 255]);
    let high_mask = EventMask {
        enabled: true,
        invert: false,
        keys: vec![EventMaskKey {
            time: 0.0,
            mask: FxMask::default(),
            opacity: 10.0,
            feather_type: MaskFeatherType::None,
        }],
    };
    apply_event_mask(&mut frame_high, &high_mask, 0.0);
    assert_eq!(frame_high.pixel(8, 8)[3], 255);

    // 4. Opacity NaN: must not panic
    let mut frame_nan = FrameBuffer::solid(16, 16, [100, 100, 100, 255]);
    let nan_mask = EventMask {
        enabled: true,
        invert: false,
        keys: vec![EventMaskKey {
            time: 0.0,
            mask: FxMask::default(),
            opacity: f32::NAN,
            feather_type: MaskFeatherType::None,
        }],
    };
    apply_event_mask(&mut frame_nan, &nan_mask, 0.0);
}

#[test]
fn adv_mask_feather_types_in_out_both_boundary_behavior() {
    let mask = FxMask {
        shape: MaskShape::Rectangle,
        center: [0.5, 0.5],
        size: [0.5, 0.5],
        rotation_deg: 0.0,
        feather: 0.15,
        invert: false,
        points: Vec::new(),
    };

    let cov_none = compute_event_mask_coverage(&mask, 32, 32, MaskFeatherType::None, false);
    let cov_both = compute_event_mask_coverage(&mask, 32, 32, MaskFeatherType::Both, false);
    let cov_in = compute_event_mask_coverage(&mask, 32, 32, MaskFeatherType::In, false);
    let cov_out = compute_event_mask_coverage(&mask, 32, 32, MaskFeatherType::Out, false);

    // Mathematical invariant checks:
    // - In feather: cov_in <= hard_fill (never expands outside)
    // - Out feather: cov_out >= hard_fill (never shrinks inside)
    for i in 0..32 * 32 {
        let hard = cov_none[i];
        let c_in = cov_in[i];
        let c_out = cov_out[i];
        let c_both = cov_both[i];

        assert!(
            c_in <= hard + 1e-4,
            "In-feathering must not expand outside hard shape"
        );
        assert!(
            c_out >= hard - 1e-4,
            "Out-feathering must not diminish shape interior"
        );
        assert!((0.0..=1.0).contains(&c_both));
    }
}

#[test]
fn adv_mask_zero_dimension_frame() {
    let mask = FxMask::default();
    let cov_zero_w = compute_event_mask_coverage(&mask, 0, 32, MaskFeatherType::Both, false);
    assert!(cov_zero_w.is_empty());
    let cov_zero_h = compute_event_mask_coverage(&mask, 32, 0, MaskFeatherType::Both, false);
    assert!(cov_zero_h.is_empty());

    let mut zero_frame = FrameBuffer {
        width: 0,
        height: 0,
        rgba: Vec::new(),
    };
    let event_mask = EventMask {
        enabled: true,
        invert: false,
        keys: vec![EventMaskKey {
            time: 0.0,
            mask,
            opacity: 1.0,
            feather_type: MaskFeatherType::Both,
        }],
    };
    // Must be safe no-op
    apply_event_mask(&mut zero_frame, &event_mask, 0.0);
}

// ============================================================================
// Feature 19: Smart Mask Adversarial Verification
// ============================================================================

#[test]
fn adv_smart_mask_conflicting_point_prompts() {
    let frame = make_test_frame(32, 32, "white");

    // Positive and negative points placed at exact identical coordinate [0.5, 0.5]
    let prompt = SegmentationPrompt {
        bounding_box: None,
        positive_points: vec![[0.5, 0.5]],
        negative_points: vec![[0.5, 0.5]],
    };

    let matte = SmartMaskMatte::segment_frame(&frame, &prompt, SmartMaskPrecision::High);
    assert_eq!(matte.width, 32);
    assert_eq!(matte.height, 32);

    // Spec §4 Edge Case 9: Negative exclusion point takes precedence over positive point
    let center_idx = (16 * 32 + 16) as usize;
    assert_eq!(
        matte.alpha_channel[center_idx], 0,
        "Negative prompt point must take precedence over conflicting positive prompt"
    );
}

#[test]
fn adv_smart_mask_zero_area_and_negative_bounding_box() {
    let frame = make_test_frame(32, 32, "checker");

    // 1. Zero-area bbox (0 x 0)
    let prompt_zero = SegmentationPrompt {
        bounding_box: Some([0.5, 0.5, 0.0, 0.0]),
        positive_points: Vec::new(),
        negative_points: Vec::new(),
    };
    let matte_zero = SmartMaskMatte::segment_frame(&frame, &prompt_zero, SmartMaskPrecision::High);
    assert_eq!(matte_zero.alpha_channel.len(), 32 * 32);

    // 2. Inverted / negative width/height bbox
    let prompt_neg = SegmentationPrompt {
        bounding_box: Some([0.8, 0.8, -0.6, -0.6]),
        positive_points: Vec::new(),
        negative_points: Vec::new(),
    };
    let matte_neg = SmartMaskMatte::segment_frame(&frame, &prompt_neg, SmartMaskPrecision::Low);
    assert_eq!(matte_neg.alpha_channel.len(), 32 * 32);

    // 3. Completely off-screen bbox
    let prompt_off = SegmentationPrompt {
        bounding_box: Some([5.0, 5.0, 2.0, 2.0]),
        positive_points: Vec::new(),
        negative_points: Vec::new(),
    };
    let matte_off = SmartMaskMatte::segment_frame(&frame, &prompt_off, SmartMaskPrecision::Low);
    assert_eq!(matte_off.alpha_channel.len(), 32 * 32);
}

#[test]
fn adv_smart_mask_pure_black_white_noise_frames() {
    let black_frame = make_test_frame(32, 32, "black");
    let white_frame = make_test_frame(32, 32, "white");
    let noise_frame = make_test_frame(32, 32, "noise");

    let prompt = SegmentationPrompt {
        bounding_box: Some([0.25, 0.25, 0.5, 0.5]),
        positive_points: vec![[0.5, 0.5]],
        negative_points: vec![[0.1, 0.1]],
    };

    for (name, frame) in [
        ("black", black_frame),
        ("white", white_frame),
        ("noise", noise_frame),
    ] {
        let matte = SmartMaskMatte::segment_frame(&frame, &prompt, SmartMaskPrecision::High);
        assert_eq!(matte.width, 32);
        assert_eq!(matte.height, 32);
        assert_eq!(matte.alpha_channel.len(), 32 * 32);

        // Vectorize to Bézier spline
        for max_pts in [0, 4, 8, 32, 100] {
            let bezier = matte.vectorize_to_bezier(max_pts);
            assert_eq!(bezier.shape, MaskShape::Bezier);
            assert!(
                bezier.points.len() >= MIN_POINTS,
                "Points count {} must be >= MIN_POINTS for {name}",
                bezier.points.len()
            );
            assert!(
                bezier.points.len() <= MAX_POINTS,
                "Points count {} must be <= MAX_POINTS for {name}",
                bezier.points.len()
            );
            for p in &bezier.points {
                assert!(p[0].is_finite());
                assert!(p[1].is_finite());
            }
        }
    }
}

#[test]
fn adv_smart_mask_apply_to_frame_dimension_mismatch() {
    let mut frame = make_test_frame(16, 16, "white");
    let matte_32 = SmartMaskMatte {
        width: 32,
        height: 32,
        alpha_channel: vec![255; 32 * 32],
    };

    // Mismatched dimensions: must safely no-op without altering frame or panicking
    matte_32.apply_to_frame(&mut frame, false);
    assert_eq!(frame.pixel(0, 0)[3], 255);
}
