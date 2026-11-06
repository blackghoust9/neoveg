//! Adversarial Stress Test Suite for 3D Transitions, ThreeDTransform, Envelopes & Thumbnails
//! Verified under execution constraints: sequential `-j 1` and `--test-threads=1`.

use std::collections::BTreeMap;
use neoveg_video::frame::FrameBuffer;
use neoveg_video::transitions::{
    get_preset, ThreeDTransform, TransitionCategory, TransitionEnvelopePoint,
    TransitionEnvelopeShape, TransitionProgressEnvelope, TransitionRenderer,
};

// ============================================================================
// 1. Extreme Angle Parameters & Ray Intersection
// ============================================================================

#[test]
fn adv_threed_transform_extreme_and_perpendicular_angles() {
    let angles: &[f32] = &[
        0.0,
        90.0,
        -90.0,
        180.0,
        -180.0,
        270.0,
        -270.0,
        360.0,
        -360.0,
        450.0,
        -450.0,
        720.0,
        -720.0,
        100000.0,
        -100000.0,
        0.000001,
        -0.000001,
        89.99999,
        90.00001,
    ];

    let src = FrameBuffer::solid(16, 16, [120, 80, 200, 255]);

    for &rx in angles {
        for &ry in angles {
            for &rz in &[0.0, 90.0, -90.0, 180.0, 720.0] {
                let transform = ThreeDTransform::new()
                    .with_rotations(rx, ry, rz)
                    .with_depth(0.0)
                    .with_specular(0.5);

                let mat = transform.rotation_matrix();
                for row in 0..3 {
                    for col in 0..3 {
                        assert!(mat[row][col].is_finite(), "Rotation matrix element must be finite for rx={rx}, ry={ry}, rz={rz}");
                    }
                }

                // apply_cpu must never panic on extreme/tangent angles
                let out = transform.apply_cpu(&src);
                assert_eq!(out.width, 16);
                assert_eq!(out.height, 16);
                assert_eq!(out.rgba.len(), 16 * 16 * 4);
            }
        }
    }
}

#[test]
fn adv_threed_transform_nan_and_inf_rotations() {
    let src = FrameBuffer::solid(8, 8, [50, 100, 150, 255]);
    let weird_angles = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY];

    for &angle in &weird_angles {
        let t = ThreeDTransform::new().with_rotations(angle, 0.0, 0.0);
        let out = t.apply_cpu(&src);
        assert_eq!(out.width, 8);
        assert_eq!(out.height, 8);

        let t2 = ThreeDTransform::new().with_rotations(0.0, angle, 0.0);
        let out2 = t2.apply_cpu(&src);
        assert_eq!(out2.width, 8);
        assert_eq!(out2.height, 8);

        let t3 = ThreeDTransform::new().with_rotations(0.0, 0.0, angle);
        let out3 = t3.apply_cpu(&src);
        assert_eq!(out3.width, 8);
        assert_eq!(out3.height, 8);
    }
}

#[test]
fn adv_threed_transform_extreme_depth_and_specular() {
    let src = FrameBuffer::solid(12, 12, [255, 200, 100, 255]);
    let depths = [-10000.0, -100.0, -1.0, 0.0, 1.0, 5.0, 10.0, 100.0, 10000.0, f32::INFINITY, f32::NEG_INFINITY, f32::NAN];
    let specs = [-10.0, -0.5, 0.0, 0.5, 1.0, 2.0, 100.0, f32::INFINITY, f32::NEG_INFINITY, f32::NAN];

    for &depth in &depths {
        for &spec in &specs {
            let t = ThreeDTransform::new()
                .with_rotations(20.0, -15.0, 5.0)
                .with_depth(depth)
                .with_specular(spec);

            let out = t.apply_cpu(&src);
            assert_eq!(out.width, 12);
            assert_eq!(out.height, 12);
            assert_eq!(out.rgba.len(), 12 * 12 * 4);

            // Also test directly constructed struct with out-of-range specular
            let raw_t = ThreeDTransform {
                rotation_x_deg: 10.0,
                rotation_y_deg: 20.0,
                rotation_z_deg: 30.0,
                depth_z: depth,
                specular_light: spec,
            };
            let raw_out = raw_t.apply_cpu(&src);
            assert_eq!(raw_out.width, 12);
            assert_eq!(raw_out.height, 12);
        }
    }
}

// ============================================================================
// 2. 3D Transitions Stress: FlyIn, Shuffle, Cube, PageRoll, PagePeel
// ============================================================================

#[test]
fn adv_transitions_all_3d_presets_adversarial_progress() {
    let src = FrameBuffer::solid(16, 16, [255, 0, 0, 255]);
    let dst = FrameBuffer::solid(16, 16, [0, 255, 0, 255]);

    let progress_values = [
        -100.0,
        -1.0,
        -0.0001,
        0.0,
        0.00001,
        0.25,
        0.5,
        0.75,
        0.99999,
        1.0,
        1.0001,
        2.0,
        100.0,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ];

    let presets_3d = ["fly_in", "3d_fly_in", "shuffle", "3d_shuffle", "cube", "page_roll", "page_peel"];

    for &preset_id in &presets_3d {
        let preset = get_preset(preset_id).expect("3D preset must exist");
        assert_eq!(preset.category, TransitionCategory::ThreeD);

        for &p in &progress_values {
            let res = preset.render(&src, &dst, p, &BTreeMap::new());
            assert!(res.is_ok(), "Preset {preset_id} failed on progress {p}");
            let frame = res.unwrap();
            assert_eq!(frame.width, 16);
            assert_eq!(frame.height, 16);
            assert_eq!(frame.rgba.len(), 16 * 16 * 4);
        }
    }
}

#[test]
fn adv_transitions_cube_adversarial_params() {
    let src = FrameBuffer::solid(16, 16, [100, 150, 200, 255]);
    let dst = FrameBuffer::solid(16, 16, [200, 150, 100, 255]);
    let cube_preset = get_preset("cube").unwrap();

    let angles = [-720.0, -360.0, -90.0, 0.0, 90.0, 180.0, 270.0, 360.0, 720.0, 99999.0, f32::NAN];
    let perspectives = [-10.0, -1.0, 0.0, 0.01, 0.5, 0.99, 1.0, 2.0, 100.0, f32::NAN, f32::INFINITY];

    for &a in &angles {
        for &persp in &perspectives {
            let params = BTreeMap::from([
                ("angle".to_string(), a),
                ("perspective".to_string(), persp),
            ]);
            let res = cube_preset.render(&src, &dst, 0.5, &params);
            assert!(res.is_ok(), "Cube render failed for angle={a}, persp={persp}");
            let f = res.unwrap();
            assert_eq!(f.width, 16);
            assert_eq!(f.height, 16);
        }
    }
}

#[test]
fn adv_transitions_page_roll_and_peel_adversarial_params() {
    let src = FrameBuffer::solid(16, 16, [10, 20, 30, 255]);
    let dst = FrameBuffer::solid(16, 16, [210, 220, 230, 255]);

    let roll_preset = get_preset("page_roll").unwrap();
    let peel_preset = get_preset("page_peel").unwrap();

    let angles = [-720.0, -360.0, -90.0, 0.0, 45.0, 90.0, 180.0, 270.0, 360.0, 720.0, f32::NAN];
    let radii = [-5.0, -0.1, 0.0, 0.0001, 0.08, 0.5, 1.0, 10.0, 100.0, f32::NAN, f32::INFINITY];

    for &a in &angles {
        for &r in &radii {
            let roll_params = BTreeMap::from([
                ("angle".to_string(), a),
                ("radius".to_string(), r),
            ]);
            let res = roll_preset.render(&src, &dst, 0.5, &roll_params);
            assert!(res.is_ok(), "PageRoll failed for angle={a}, radius={r}");
            let f = res.unwrap();
            assert_eq!(f.width, 16);
            assert_eq!(f.height, 16);
        }

        let peel_params = BTreeMap::from([("angle".to_string(), a)]);
        let res = peel_preset.render(&src, &dst, 0.5, &peel_params);
        assert!(res.is_ok(), "PagePeel failed for angle={a}");
        let f = res.unwrap();
        assert_eq!(f.width, 16);
        assert_eq!(f.height, 16);
    }
}

#[test]
fn adv_transitions_fly_in_and_shuffle_extreme_parameters() {
    let src = FrameBuffer::solid(16, 16, [11, 22, 33, 255]);
    let dst = FrameBuffer::solid(16, 16, [44, 55, 66, 255]);

    let fly_in = get_preset("fly_in").unwrap();
    let shuffle = get_preset("shuffle").unwrap();

    let far_zs = [-100.0, -1.0, 0.0, 4.0, 10.0, 100.0, 10000.0, f32::NAN, f32::INFINITY];
    let rotations = [-720.0, -90.0, 0.0, 90.0, 360.0, 1000.0, f32::NAN];
    let speculars = [-10.0, 0.0, 0.5, 1.0, 50.0, f32::NAN];

    for &fz in &far_zs {
        for &rx in &rotations {
            for &spec in &speculars {
                let params = BTreeMap::from([
                    ("far_z".to_string(), fz),
                    ("rotation_x".to_string(), rx),
                    ("rotation_y".to_string(), 45.0),
                    ("rotation_z".to_string(), -15.0),
                    ("specular_light".to_string(), spec),
                ]);
                let res = fly_in.render(&src, &dst, 0.5, &params);
                assert!(res.is_ok(), "FlyIn failed for far_z={fz}, rx={rx}, spec={spec}");
            }
        }
    }

    let extra_spins = [-50.0, -1.0, 0.0, 1.0, 10.0, 1000.0, f32::NAN];
    let staggers = [-10.0, 0.0, 0.2, 1.0, 50.0, f32::NAN];

    for &es in &extra_spins {
        for &stag in &staggers {
            let params = BTreeMap::from([
                ("extra_spins".to_string(), es),
                ("stagger".to_string(), stag),
                ("specular_light".to_string(), 0.5),
            ]);
            let res = shuffle.render(&src, &dst, 0.5, &params);
            assert!(res.is_ok(), "Shuffle failed for extra_spins={es}, stagger={stag}");
        }
    }
}

// ============================================================================
// 3. Transition Progress Envelope Stress
// ============================================================================

#[test]
fn adv_envelope_empty_points() {
    let env = TransitionProgressEnvelope {
        points: Vec::new(),
        enabled: true,
    };
    assert!(!env.is_active());
    // Must return 1.0 and not panic for any time
    for t in &[-10.0, 0.0, 0.5, 1.0, 10.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let p = env.progress_at(*t);
        assert_eq!(p, 1.0);
        assert!((0.0..=1.0).contains(&p));
    }
}

#[test]
fn adv_envelope_single_point() {
    for shape in [
        TransitionEnvelopeShape::Linear,
        TransitionEnvelopeShape::Fast,
        TransitionEnvelopeShape::Slow,
        TransitionEnvelopeShape::Smooth,
        TransitionEnvelopeShape::Sharp,
        TransitionEnvelopeShape::Hold,
    ] {
        let env = TransitionProgressEnvelope {
            points: vec![TransitionEnvelopePoint {
                time: 0.5,
                progress: 0.75,
                shape,
            }],
            enabled: true,
        };
        assert!(env.is_active());

        // Single point should return the clamped point progress regardless of time
        for t in &[-10.0, 0.0, 0.49, 0.5, 0.51, 1.0, 10.0, f64::INFINITY, f64::NEG_INFINITY] {
            let p = env.progress_at(*t);
            assert_eq!(p, 0.75);
            assert!((0.0..=1.0).contains(&p));
        }
        // NaN time returns 1.0
        assert_eq!(env.progress_at(f64::NAN), 1.0);
    }
}

#[test]
fn adv_envelope_duplicate_timestamps() {
    let mut env = TransitionProgressEnvelope::new(1.0);
    // add_point with same timestamp should update existing point rather than duplicate
    env.add_point(0.0, 0.2, TransitionEnvelopeShape::Fast);
    assert_eq!(env.points.len(), 2);
    assert_eq!(env.points[0].progress, 0.2);

    // Exact or near-duplicate timestamp
    env.add_point(0.5, 0.4, TransitionEnvelopeShape::Linear);
    env.add_point(0.5 + 1e-7, 0.6, TransitionEnvelopeShape::Smooth);
    // Within 1e-6, it should overwrite
    assert_eq!(env.points.len(), 3);
    assert_eq!(env.points[1].progress, 0.6);

    // Manually constructed envelope with identical timestamps
    let manual_dup = TransitionProgressEnvelope {
        points: vec![
            TransitionEnvelopePoint { time: 0.0, progress: 0.0, shape: TransitionEnvelopeShape::Linear },
            TransitionEnvelopePoint { time: 0.5, progress: 0.3, shape: TransitionEnvelopeShape::Linear },
            TransitionEnvelopePoint { time: 0.5, progress: 0.8, shape: TransitionEnvelopeShape::Linear },
            TransitionEnvelopePoint { time: 1.0, progress: 1.0, shape: TransitionEnvelopeShape::Linear },
        ],
        enabled: true,
    };
    for t in &[0.0, 0.25, 0.5, 0.75, 1.0] {
        let p = manual_dup.progress_at(*t);
        assert!((0.0..=1.0).contains(&p));
    }
}

#[test]
fn adv_envelope_nan_and_infinite_times() {
    let mut env = TransitionProgressEnvelope::new(2.0);
    env.add_point(0.5, 0.25, TransitionEnvelopeShape::Fast);
    env.add_point(1.5, 0.85, TransitionEnvelopeShape::Smooth);

    assert_eq!(env.progress_at(f64::NAN), 1.0);
    assert_eq!(env.progress_at(f64::INFINITY), 1.0);
    assert_eq!(env.progress_at(f64::NEG_INFINITY), 0.0);

    // Negative initial ramp time in new()
    let neg_ramp = TransitionProgressEnvelope::new(-5.0);
    assert_eq!(neg_ramp.points[1].time, 1.0); // Should fallback to 1.0

    let nan_ramp = TransitionProgressEnvelope::new(f64::NAN);
    assert_eq!(nan_ramp.points[1].time, 1.0); // Fallback to 1.0

    let inf_ramp = TransitionProgressEnvelope::new(f64::INFINITY);
    assert_eq!(inf_ramp.points[1].time, 1.0); // Fallback to 1.0
}

#[test]
fn adv_envelope_unconditional_clamping_all_shapes() {
    let shapes = [
        TransitionEnvelopeShape::Linear,
        TransitionEnvelopeShape::Fast,
        TransitionEnvelopeShape::Slow,
        TransitionEnvelopeShape::Smooth,
        TransitionEnvelopeShape::Sharp,
        TransitionEnvelopeShape::Hold,
    ];

    for shape in shapes {
        // Test direct shape evaluation on out-of-range u
        for u in &[-10.0, -1.0, -0.01, 0.0, 0.25, 0.5, 0.75, 1.0, 1.01, 2.0, 100.0] {
            let val = shape.eval(*u);
            assert!(
                (0.0..=1.0).contains(&val),
                "Shape {:?} produced {} on u={}",
                shape,
                val,
                u
            );
        }

        // Test envelope progress_at with out-of-range point progress
        let env = TransitionProgressEnvelope {
            points: vec![
                TransitionEnvelopePoint { time: 0.0, progress: -5.0, shape },
                TransitionEnvelopePoint { time: 1.0, progress: 10.0, shape },
            ],
            enabled: true,
        };

        for step in 0..=50 {
            let t = step as f64 / 50.0;
            let p = env.progress_at(t);
            assert!(
                (0.0..=1.0).contains(&p),
                "Shape {:?} envelope produced progress {} at t={}",
                shape,
                p,
                t
            );
        }
    }
}

#[test]
fn adv_envelope_out_of_order_timestamps() {
    let env_reversed = TransitionProgressEnvelope {
        points: vec![
            TransitionEnvelopePoint { time: 5.0, progress: 0.8, shape: TransitionEnvelopeShape::Linear },
            TransitionEnvelopePoint { time: 1.0, progress: 0.2, shape: TransitionEnvelopeShape::Linear },
        ],
        enabled: true,
    };
    for t in &[-1.0, 0.0, 2.0, 4.0, 6.0] {
        let p = env_reversed.progress_at(*t);
        assert!((0.0..=1.0).contains(&p));
    }
}

/// EMPIRICAL BUG REMEDIATION:
/// Calling `add_point(f64::NAN, ...)` is safely rejected or guarded against non-finite timestamps.
/// When `progress_at(0.0)` is called, bounds checking prevents integer underflow panic on `idx - 1`,
/// and returns a valid clamped progress in 0.0..=1.0.
#[test]
fn adv_envelope_nan_timestamp_causes_underflow_panic() {
    let mut env = TransitionProgressEnvelope::new(1.0);
    env.add_point(f64::NAN, 0.5, TransitionEnvelopeShape::Linear);
    // Non-finite timestamps are guarded; progress_at must remain clamped in 0.0..=1.0 without panicking
    let p = env.progress_at(0.0);
    assert!((0.0..=1.0).contains(&p));
}

/// EMPIRICAL VULNERABILITY REMEDIATION:
/// Calling `add_point` with infinite timestamps is guarded, and `dt.is_finite()` prevents `inf / inf = NaN`.
/// `progress_at` guarantees a finite value unconditionally clamped to 0.0..=1.0.
#[test]
fn adv_envelope_infinite_timestamp_exposes_nan_propagation() {
    let mut env_inf = TransitionProgressEnvelope::new(1.0);
    env_inf.add_point(f64::NEG_INFINITY, 0.5, TransitionEnvelopeShape::Linear);
    let pinf = env_inf.progress_at(-10.0);
    // Non-finite timestamps are guarded; progress_at must remain clamped in 0.0..=1.0 without leaking NaN
    assert!((0.0..=1.0).contains(&pinf));
    assert!(!pinf.is_nan());
}

// ============================================================================
// 4. Thumbnail Generation Robustness (Empty, 1x1, Mismatched Buffers)
// ============================================================================

#[test]
fn adv_render_thumbnail_never_panics_on_empty_and_1x1_buffers() {
    let empty_0x0 = FrameBuffer::solid(0, 0, [0, 0, 0, 0]);
    let empty_0x10 = FrameBuffer::solid(0, 10, [0, 0, 0, 0]);
    let empty_10x0 = FrameBuffer::solid(10, 0, [0, 0, 0, 0]);
    let single_1x1_a = FrameBuffer::solid(1, 1, [255, 0, 0, 255]);
    let single_1x1_b = FrameBuffer::solid(1, 1, [0, 0, 255, 255]);
    let normal_16x16 = FrameBuffer::solid(16, 16, [0, 255, 0, 255]);

    let presets = [
        "cross_dissolve",
        "linear_wipe",
        "radial_wipe",
        "iris",
        "push",
        "slide",
        "zoom",
        "3d_fly_in",
        "page_peel",
        "page_roll",
        "cube",
        "3d_shuffle",
    ];

    for &preset_id in &presets {
        let preset = get_preset(preset_id).unwrap();

        // 1. 0x0 source and target -> must not panic
        let thumb = preset.render_thumbnail(&empty_0x0, &empty_0x0, &BTreeMap::new());
        assert_eq!(thumb.rgba.len(), (thumb.width * thumb.height * 4) as usize);

        // 2. 0x10 source and target -> must not panic
        let thumb = preset.render_thumbnail(&empty_0x10, &empty_0x10, &BTreeMap::new());
        assert_eq!(thumb.rgba.len(), (thumb.width * thumb.height * 4) as usize);

        // 3. 10x0 source and target -> must not panic
        let thumb = preset.render_thumbnail(&empty_10x0, &empty_10x0, &BTreeMap::new());
        assert_eq!(thumb.rgba.len(), (thumb.width * thumb.height * 4) as usize);

        // 4. 1x1 source and target -> must not panic, valid 1x1 buffer
        let thumb = preset.render_thumbnail(&single_1x1_a, &single_1x1_b, &BTreeMap::new());
        assert_eq!(thumb.width, 1);
        assert_eq!(thumb.height, 1);
        assert_eq!(thumb.rgba.len(), 4);

        // 5. Mismatched: 0x0 source, 16x16 target -> must not panic
        let thumb = preset.render_thumbnail(&empty_0x0, &normal_16x16, &BTreeMap::new());
        assert_eq!(thumb.rgba.len(), (thumb.width * thumb.height * 4) as usize);

        // 6. Mismatched: 16x16 source, 0x0 target -> must not panic
        let thumb = preset.render_thumbnail(&normal_16x16, &empty_0x0, &BTreeMap::new());
        assert_eq!(thumb.rgba.len(), (thumb.width * thumb.height * 4) as usize);

        // 7. Mismatched: 1x1 source, 16x16 target -> must not panic
        let thumb = preset.render_thumbnail(&single_1x1_a, &normal_16x16, &BTreeMap::new());
        assert_eq!(thumb.rgba.len(), (thumb.width * thumb.height * 4) as usize);

        // 8. Mismatched: 16x16 source, 1x1 target -> must not panic
        let thumb = preset.render_thumbnail(&normal_16x16, &single_1x1_a, &BTreeMap::new());
        assert_eq!(thumb.rgba.len(), (thumb.width * thumb.height * 4) as usize);
    }
}
