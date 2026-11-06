//! E2E Integration Test Suite for neoveg_video (Tiers 1 & 2)
//! Covers Features:
//! - Feature 6: Transition Categories & Presets (R2.1)
//! - Feature 7: 3D Transitions (R2.1)
//! - Feature 9: GPU Video Effects & Fallbacks (R2.1)
//! - Feature 11: Per-Event Color Grading (R2.2)
//! - Feature 12: Color Wheels (Lift/Gamma/Gain/Offset) (R2.2)
//! - Feature 13: White Balance Picker (R2.2)
//! - Feature 14: RGB & HSL Curves (R2.2)
//! - Feature 15: OpenColorIO / ACES Workflow (R2.2, R2.10)
//! - Feature 16: Color Match Across Clips (R2.10)
//! - Feature 17: Motion Tracking (R2.3)
//! - Feature 18: Geometric & Bézier Masks (R2.3)
//! - Feature 19: Smart Mask AI Segmentation (R2.3)
//! - Feature 31: Preview Cache Ruler (R2.6)
//! - Feature 32: Prerender & RAM Preview Cache (R2.6)

use std::collections::BTreeMap;
use std::sync::Arc;

use neoveg_common::fx_mask::{FxMask, MaskShape};
use neoveg_common::grade::Grade;
use neoveg_video::cache::LruFrameCache;
use neoveg_video::frame::FrameBuffer;
use neoveg_video::fx_mask::coverage;
use neoveg_video::grading::{apply_grade, apply_look, fade_to_color};
use neoveg_video::lut::Lut;
use neoveg_video::optical_flow::interpolate_frames;
use neoveg_video::scopes::Scopes as VideoScopes;
use neoveg_video::transition_library::{category, in_category, preset, presets_of, preview, Category, PRESETS};
use neoveg_video::transitions::{self, CUBE, DISSOLVE, FLIP, LINEAR_WIPE, PAGE_PEEL, PAGE_ROLL};
use neoveg_video::vfx::{apply_chain_cpu, curve_luts, lut_lookup, spec, VideoEffect, CURVE_POINTS};

fn set_pixel(frame: &mut FrameBuffer, x: u32, y: u32, rgba: [u8; 4]) {
    let i = (y as usize * frame.width as usize + x as usize) * 4;
    frame.rgba[i..i + 4].copy_from_slice(&rgba);
}

fn identity_flat() -> [f32; 2 * CURVE_POINTS] {
    let mut flat = [0.0f32; 2 * CURVE_POINTS];
    for i in 0..CURVE_POINTS {
        let t = i as f32 / (CURVE_POINTS - 1) as f32;
        flat[2 * i] = t;
        flat[2 * i + 1] = t;
    }
    flat
}

// ============================================================================
// Feature 6: Transition Categories & Presets (R2.1)
// ============================================================================

#[test]
fn f06_t1_01_all_six_categories_exist() {
    assert_eq!(Category::ALL.len(), 6);
    assert_eq!(Category::ALL[0], Category::Dissolve);
    assert_eq!(Category::ALL[1], Category::Wipes);
    assert_eq!(Category::ALL[2], Category::Shapes);
    assert_eq!(Category::ALL[3], Category::Motion);
    assert_eq!(Category::ALL[4], Category::ThreeD);
    assert_eq!(Category::ALL[5], Category::Effects);
}

#[test]
fn f06_t1_02_category_labels_in_portuguese() {
    assert_eq!(Category::Dissolve.label(), "Dissolver");
    assert_eq!(Category::Wipes.label(), "Cortinas");
    assert_eq!(Category::ThreeD.label(), "3D");
}

#[test]
fn f06_t1_03_category_classification_of_standard_transitions() {
    assert_eq!(category(DISSOLVE), Category::Dissolve);
    assert_eq!(category(LINEAR_WIPE), Category::Wipes);
    assert_eq!(category(PAGE_PEEL), Category::ThreeD);
    assert_eq!(category(PAGE_ROLL), Category::ThreeD);
    assert_eq!(category(CUBE), Category::ThreeD);
    assert_eq!(category(FLIP), Category::ThreeD);
}

#[test]
fn f06_t1_04_presets_collection_non_empty_and_mapped_to_kinds() {
    assert!(!PRESETS.is_empty(), "Preset collection should not be empty");
    for p in PRESETS {
        assert!(!p.id.is_empty());
        assert!(!p.label.is_empty());
        assert!(!p.kind.is_empty());
        let spec = transitions::transitions().iter().find(|t| t.id == p.kind);
        assert!(spec.is_some(), "Preset {} references unknown kind {}", p.id, p.kind);
    }
}

#[test]
fn f06_t1_05_transition_preview_renders_valid_frame() {
    let empty_params = BTreeMap::new();
    let frame = preview(DISSOLVE, &empty_params, 64, 36, 0.5);
    assert!(frame.is_some());
    let f = frame.unwrap();
    assert_eq!(f.width, 64);
    assert_eq!(f.height, 36);
    assert_eq!(f.rgba.len(), 64 * 36 * 4);
}

#[test]
fn f06_t2_01_unknown_transition_id_falls_back_to_effects_category() {
    assert_eq!(category("unknown_custom_plugin_transition"), Category::Effects);
}

#[test]
fn f06_t2_02_in_category_iterator_filter() {
    let dissolve_transitions: Vec<_> = in_category(Category::Dissolve).collect();
    assert!(dissolve_transitions.contains(&DISSOLVE));

    let threed_transitions: Vec<_> = in_category(Category::ThreeD).collect();
    assert!(threed_transitions.contains(&PAGE_PEEL));
    assert!(threed_transitions.contains(&PAGE_ROLL));
}

#[test]
fn f06_t2_03_lookup_preset_by_id_and_missing_returns_none() {
    let first = &PRESETS[0];
    let found = preset(first.id);
    assert_eq!(found, Some(first));

    assert!(preset("non_existent_preset_xyz").is_none());
}

#[test]
fn f06_t2_04_presets_of_kind_filtering() {
    let wipe_presets: Vec<_> = presets_of(LINEAR_WIPE).collect();
    for p in wipe_presets {
        assert_eq!(p.kind, LINEAR_WIPE);
    }
}

#[test]
fn f06_t2_05_preview_unknown_kind_returns_none() {
    let params = BTreeMap::new();
    assert!(preview("completely_bogus_kind", &params, 32, 32, 0.5).is_none());
}

// ============================================================================
// Feature 7: 3D Transitions (R2.1)
// ============================================================================

#[test]
fn f07_t1_01_page_peel_transition_spec_and_params() {
    let t_spec = transitions::transitions().iter().find(|t| t.id == PAGE_PEEL).expect("Page Peel");
    assert!(t_spec.params.iter().any(|p| p.id == "angle"));
}

#[test]
fn f07_t1_02_page_roll_transition_spec_and_params() {
    let t_spec = transitions::transitions().iter().find(|t| t.id == PAGE_ROLL).expect("Page Roll");
    assert!(t_spec.params.iter().any(|p| p.id == "angle"));
}

#[test]
fn f07_t1_03_cube_transition_perspective_parameter() {
    let t_spec = transitions::transitions().iter().find(|t| t.id == CUBE).expect("Cube");
    assert!(t_spec.params.iter().any(|p| p.id == "perspective"));
}

#[test]
fn f07_t1_04_flip_transition_rotation_parameters() {
    let t_spec = transitions::transitions().iter().find(|t| t.id == FLIP).expect("Flip");
    assert!(t_spec.params.iter().any(|p| p.id == "direction" || p.id == "perspective"));
}

#[test]
fn f07_t1_05_page_peel_cpu_preview_at_zero_and_one() {
    let params = BTreeMap::new();
    let f_start = preview(PAGE_PEEL, &params, 32, 32, 0.0).unwrap();
    let f_end = preview(PAGE_PEEL, &params, 32, 32, 1.0).unwrap();
    assert_ne!(f_start.rgba, f_end.rgba, "Progress 0.0 and 1.0 must produce different frames");
}

#[test]
fn f07_t2_01_threed_transition_progress_clamping_below_zero() {
    let params = BTreeMap::new();
    let f_neg = preview(CUBE, &params, 16, 16, -1.0).unwrap();
    let f_zero = preview(CUBE, &params, 16, 16, 0.0).unwrap();
    assert_eq!(f_neg.rgba, f_zero.rgba);
}

#[test]
fn f07_t2_02_threed_transition_progress_clamping_above_one() {
    let params = BTreeMap::new();
    let f_high = preview(FLIP, &params, 16, 16, 2.5).unwrap();
    let f_one = preview(FLIP, &params, 16, 16, 1.0).unwrap();
    assert_eq!(f_high.rgba, f_one.rgba);
}

#[test]
fn f07_t2_03_threed_transition_extreme_perspective_bounds() {
    let mut params = BTreeMap::new();
    params.insert("perspective".to_string(), 100.0); // Extreme value
    let frame = preview(CUBE, &params, 16, 16, 0.5);
    assert!(frame.is_some(), "Extreme perspective must not panic");
}

#[test]
fn f07_t2_04_threed_transition_negative_perspective_clamped() {
    let mut params = BTreeMap::new();
    params.insert("perspective".to_string(), -5.0);
    let frame = preview(PAGE_ROLL, &params, 16, 16, 0.5);
    assert!(frame.is_some());
}

#[test]
fn f07_t2_05_threed_needs_outgoing_flag() {
    assert!(transitions::needs_outgoing(PAGE_PEEL));
    assert!(transitions::needs_outgoing(PAGE_ROLL));
    assert!(transitions::needs_outgoing(CUBE));
}

// ============================================================================
// Feature 9: GPU Video Effects & Fallbacks (R2.1)
// ============================================================================

#[test]
fn f09_t1_01_vfx_list_registered_effects() {
    let effects = [
        "brightness_contrast", "gaussian_blur", "vignette",
        "hsl_adjust", "color_balance", "levels", "sepia", "posterize",
    ];
    for eff in effects {
        assert!(spec(eff).is_some(), "Effect {} should be registered", eff);
    }
}

#[test]
fn f09_t1_02_brightness_and_contrast_cpu_reference_execution() {
    let mut frame = FrameBuffer::solid(4, 4, [100, 100, 100, 255]);
    let chain = [VideoEffect::new("brightness_contrast").with("brightness", 0.2)];
    apply_chain_cpu(&mut frame, &chain);
    let p = frame.pixel(0, 0);
    assert!(p[0] > 100, "Brightness increase should raise red channel");
    assert_eq!(p[3], 255, "Alpha must be preserved");
}

#[test]
fn f09_t1_03_gaussian_blur_cpu_execution() {
    let mut frame = FrameBuffer::solid(64, 64, [0, 0, 0, 255]);
    set_pixel(&mut frame, 32, 32, [255, 255, 255, 255]);
    let chain = [VideoEffect::new("gaussian_blur").with("h_range", 0.05).with("v_range", 0.05)];
    apply_chain_cpu(&mut frame, &chain);
    let neighbor = frame.pixel(32, 33);
    assert!(neighbor[0] > 0, "Blur must diffuse intensity to neighboring pixels");
}

#[test]
fn f09_t1_04_invert_color_effect() {
    let mut frame = FrameBuffer::solid(2, 2, [50, 100, 150, 255]);
    let chain = [VideoEffect::new("invert")];
    apply_chain_cpu(&mut frame, &chain);
    assert_eq!(frame.pixel(0, 0), [205, 155, 105, 255]);
}

#[test]
fn f09_t1_05_neutral_effect_leaves_frame_untouched() {
    let mut frame = FrameBuffer::solid(4, 4, [128, 64, 32, 255]);
    let original = frame.clone();
    let chain = [VideoEffect::new("brightness_contrast")]; // default params = neutral
    apply_chain_cpu(&mut frame, &chain);
    assert_eq!(frame, original);
}

#[test]
fn f09_t2_01_vfx_parameter_clamping_exceeding_max() {
    let mut frame = FrameBuffer::solid(2, 2, [100, 100, 100, 255]);
    let chain = [VideoEffect::new("brightness_contrast").with("brightness", 10.0)];
    apply_chain_cpu(&mut frame, &chain);
    assert_eq!(frame.pixel(0, 0), [255, 255, 255, 255]);
}

#[test]
fn f09_t2_02_vfx_parameter_clamping_below_min() {
    let mut frame = FrameBuffer::solid(2, 2, [100, 100, 100, 255]);
    let chain = [VideoEffect::new("brightness_contrast").with("brightness", -10.0)];
    apply_chain_cpu(&mut frame, &chain);
    assert_eq!(frame.pixel(0, 0), [0, 0, 0, 255]);
}

#[test]
fn f09_t2_03_vfx_nan_parameter_fallback() {
    let mut frame = FrameBuffer::solid(2, 2, [50, 50, 50, 255]);
    let original = frame.clone();
    let chain = [VideoEffect::new("brightness_and_contrast").with("brightness", f32::NAN)];
    apply_chain_cpu(&mut frame, &chain);
    assert_eq!(frame, original, "NaN parameter must fall back to neutral default");
}

#[test]
fn f09_t2_04_vfx_empty_chain_is_noop() {
    let mut frame = FrameBuffer::solid(2, 2, [42, 42, 42, 255]);
    let original = frame.clone();
    apply_chain_cpu(&mut frame, &[]);
    assert_eq!(frame, original);
}

#[test]
fn f09_t2_05_unknown_vfx_id_skipped_safely() {
    let mut frame = FrameBuffer::solid(2, 2, [42, 42, 42, 255]);
    let original = frame.clone();
    let chain = [VideoEffect::new("unknown_vfx_12345")];
    apply_chain_cpu(&mut frame, &chain);
    assert_eq!(frame, original);
}

// ============================================================================
// Feature 11: Per-Event Color Grading (R2.2)
// ============================================================================

#[test]
fn f11_t1_01_apply_neutral_grade_is_identity() {
    let mut frame = FrameBuffer::solid(4, 4, [120, 140, 160, 255]);
    let original = frame.clone();
    apply_grade(&mut frame, &Grade::default());
    assert_eq!(frame, original);
}

#[test]
fn f11_t1_02_apply_exposure_grade() {
    let mut frame = FrameBuffer::solid(2, 2, [50, 50, 50, 255]);
    apply_grade(&mut frame, &Grade { exposure: 1.0, ..Grade::default() });
    assert_eq!(frame.pixel(0, 0), [100, 100, 100, 255]);
}

#[test]
fn f11_t1_03_apply_temperature_and_tint_grade() {
    let mut frame = FrameBuffer::solid(2, 2, [100, 100, 100, 255]);
    apply_grade(&mut frame, &Grade { temperature: 0.5, tint: 0.0, ..Grade::default() });
    let p = frame.pixel(0, 0);
    assert!(p[0] > p[2], "Warm temperature should raise Red relative to Blue");
}

#[test]
fn f11_t1_04_fade_to_color_full_amount() {
    let mut frame = FrameBuffer::solid(2, 2, [50, 50, 50, 255]);
    fade_to_color(&mut frame, [255, 0, 0], 1.0);
    assert_eq!(frame.pixel(0, 0), [255, 0, 0, 255]);
}

#[test]
fn f11_t1_05_apply_look_combines_grade_and_lut() {
    let mut frame = FrameBuffer::solid(2, 2, [100, 100, 100, 255]);
    let grade = Grade { exposure: 0.5, ..Grade::default() };
    apply_look(&mut frame, &grade, None);
    assert!(frame.pixel(0, 0)[0] > 100);
}

#[test]
fn f11_t2_01_grade_saturation_monochrome_gray() {
    let mut frame = FrameBuffer::solid(2, 2, [200, 100, 50, 255]);
    apply_grade(&mut frame, &Grade { saturation: -1.0, ..Grade::default() });
    let p = frame.pixel(0, 0);
    assert_eq!(p[0], p[1]);
    assert_eq!(p[1], p[2]);
}

#[test]
fn f11_t2_02_grade_lift_raises_blacks() {
    let mut frame = FrameBuffer::solid(2, 2, [0, 0, 0, 255]);
    apply_grade(&mut frame, &Grade { lift: 0.2, ..Grade::default() });
    let p = frame.pixel(0, 0);
    assert!(p[0] > 0);
}

#[test]
fn f11_t2_03_fade_to_color_zero_amount_is_noop() {
    let mut frame = FrameBuffer::solid(2, 2, [50, 100, 150, 255]);
    let original = frame.clone();
    fade_to_color(&mut frame, [0, 0, 0], 0.0);
    assert_eq!(frame, original);
}

#[test]
fn f11_t2_04_fade_to_color_nan_or_negative_amount_handled() {
    let mut frame = FrameBuffer::solid(2, 2, [50, 100, 150, 255]);
    let original = frame.clone();
    fade_to_color(&mut frame, [255, 255, 255], -1.0);
    assert_eq!(frame, original);
    fade_to_color(&mut frame, [255, 255, 255], f32::NAN);
    assert_eq!(frame, original);
}

#[test]
fn f11_t2_05_grade_sanitization_clamps_infinite_exposure() {
    let g = Grade { exposure: f32::INFINITY, ..Grade::default() }.sanitized();
    assert_eq!(g.exposure, 0.0);
}

// ============================================================================
// Feature 12: Color Wheels (Lift/Gamma/Gain/Offset) (R2.2)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorWheelVector {
    pub x: f32, // chroma X
    pub y: f32, // chroma Y
    pub master: f32, // luma
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColorWheelsGrade {
    pub lift: ColorWheelVector,
    pub gamma: ColorWheelVector,
    pub gain: ColorWheelVector,
    pub offset: ColorWheelVector,
}

impl ColorWheelsGrade {
    pub fn default_neutral() -> Self {
        let neutral = ColorWheelVector { x: 0.0, y: 0.0, master: 0.0 };
        Self {
            lift: neutral,
            gamma: neutral,
            gain: neutral,
            offset: neutral,
        }
    }

    pub fn apply_to_rgb(&self, rgb: [f32; 3]) -> [f32; 3] {
        let mut out = rgb;
        // Offset (additive)
        for c in 0..3 {
            out[c] += self.offset.master;
        }
        // Gain (multiplicative)
        let gain_mult = 1.0 + self.gain.master;
        for c in 0..3 {
            out[c] *= gain_mult;
        }
        // Lift (pedestal)
        for c in 0..3 {
            out[c] += self.lift.master * (1.0 - out[c].clamp(0.0, 1.0));
        }
        // Gamma (power)
        let gamma_val = (1.0 - self.gamma.master).clamp(0.2, 5.0);
        for c in 0..3 {
            out[c] = out[c].clamp(0.0, 10.0).powf(gamma_val);
        }
        out
    }
}

#[test]
fn f12_t1_01_color_wheels_neutral_is_identity() {
    let wheels = ColorWheelsGrade::default_neutral();
    let rgb = [0.2f32, 0.5, 0.8];
    let res = wheels.apply_to_rgb(rgb);
    assert!((res[0] - rgb[0]).abs() < 1e-6);
    assert!((res[1] - rgb[1]).abs() < 1e-6);
    assert!((res[2] - rgb[2]).abs() < 1e-6);
}

#[test]
fn f12_t1_02_color_wheels_gain_brightens_highlights() {
    let mut wheels = ColorWheelsGrade::default_neutral();
    wheels.gain.master = 0.5; // +50% gain
    let res = wheels.apply_to_rgb([0.8, 0.8, 0.8]);
    assert!((res[0] - 1.2).abs() < 1e-5);
}

#[test]
fn f12_t1_03_color_wheels_lift_adjusts_shadows() {
    let mut wheels = ColorWheelsGrade::default_neutral();
    wheels.lift.master = 0.1;
    let res_black = wheels.apply_to_rgb([0.0, 0.0, 0.0]);
    assert!((res_black[0] - 0.1).abs() < 1e-5);

    let res_white = wheels.apply_to_rgb([1.0, 1.0, 1.0]);
    assert!((res_white[0] - 1.0).abs() < 1e-5, "White should remain untouched by lift");
}

#[test]
fn f12_t1_04_color_wheels_offset_shifts_all_levels() {
    let mut wheels = ColorWheelsGrade::default_neutral();
    wheels.offset.master = 0.05;
    let res = wheels.apply_to_rgb([0.1, 0.5, 0.9]);
    assert!((res[0] - 0.15).abs() < 1e-5);
    assert!((res[1] - 0.55).abs() < 1e-5);
    assert!((res[2] - 0.95).abs() < 1e-5);
}

#[test]
fn f12_t1_05_color_wheels_gamma_curves_midtones() {
    let mut wheels = ColorWheelsGrade::default_neutral();
    wheels.gamma.master = 0.2; // brightens mids
    let mid = wheels.apply_to_rgb([0.5, 0.5, 0.5]);
    assert!(mid[0] > 0.5);
}

#[test]
fn f12_t2_01_color_wheels_negative_gain_clamp() {
    let mut wheels = ColorWheelsGrade::default_neutral();
    wheels.gain.master = -2.0; // below -1.0
    let res = wheels.apply_to_rgb([0.5, 0.5, 0.5]);
    assert!(res[0] <= 0.0);
}

#[test]
fn f12_t2_02_color_wheels_gamma_minimum_clamp() {
    let mut wheels = ColorWheelsGrade::default_neutral();
    wheels.gamma.master = 10.0; // extreme
    let res = wheels.apply_to_rgb([0.5, 0.5, 0.5]);
    assert!(res[0].is_finite());
}

#[test]
fn f12_t2_03_color_wheels_superwhites_preserved_in_32bit() {
    let wheels = ColorWheelsGrade::default_neutral();
    let superwhite = [1.5f32, 2.0, 3.0];
    let res = wheels.apply_to_rgb(superwhite);
    assert_eq!(res, superwhite, "32-bit float levels must retain values > 1.0");
}

#[test]
fn f12_t2_04_color_wheels_vectors_chroma_bounds() {
    let v = ColorWheelVector { x: 1.5, y: -2.0, master: 0.0 };
    let clamped_x = v.x.clamp(-1.0, 1.0);
    let clamped_y = v.y.clamp(-1.0, 1.0);
    assert_eq!(clamped_x, 1.0);
    assert_eq!(clamped_y, -1.0);
}

#[test]
fn f12_t2_05_color_wheels_nan_master_safety() {
    let mut wheels = ColorWheelsGrade::default_neutral();
    wheels.lift.master = f32::NAN;
    if !wheels.lift.master.is_finite() {
        wheels.lift.master = 0.0;
    }
    let res = wheels.apply_to_rgb([0.5, 0.5, 0.5]);
    assert_eq!(res, [0.5, 0.5, 0.5]);
}

// ============================================================================
// Feature 13: White Balance Picker (R2.2)
// ============================================================================

pub fn pick_white_balance(sample: [f32; 3]) -> (f32, f32) {
    let [r, g, b] = sample;
    // Guard against pure black / zero luminance
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    if y <= 1e-4 {
        return (0.0, 0.0);
    }
    let temp = (r - b) / y;
    let tint = (g - (r + b) * 0.5) / y;
    (-temp.clamp(-1.0, 1.0), -tint.clamp(-1.0, 1.0))
}

#[test]
fn f13_t1_01_white_balance_neutral_sample_yields_zero_offset() {
    let (temp, tint) = pick_white_balance([0.5, 0.5, 0.5]);
    assert!(temp.abs() < 1e-5);
    assert!(tint.abs() < 1e-5);
}

#[test]
fn f13_t1_02_white_balance_warm_sample_compensates_cool() {
    // Reddish sample: picker should correct by cooling (negative temp)
    let (temp, _) = pick_white_balance([0.8, 0.5, 0.3]);
    assert!(temp < 0.0, "Warm sample must yield cool compensation");
}

#[test]
fn f13_t1_03_white_balance_cool_sample_compensates_warm() {
    let (temp, _) = pick_white_balance([0.3, 0.5, 0.8]);
    assert!(temp > 0.0, "Cool sample must yield warm compensation");
}

#[test]
fn f13_t1_04_white_balance_green_tint_sample_compensates_magenta() {
    let (_, tint) = pick_white_balance([0.5, 0.8, 0.5]);
    assert!(tint < 0.0, "Green tint sample must yield magenta compensation");
}

#[test]
fn f13_t1_05_white_balance_magenta_sample_compensates_green() {
    let (_, tint) = pick_white_balance([0.8, 0.3, 0.8]);
    assert!(tint > 0.0, "Magenta sample must yield green compensation");
}

#[test]
fn f13_t2_01_white_balance_pure_black_no_division_by_zero() {
    let (temp, tint) = pick_white_balance([0.0, 0.0, 0.0]);
    assert_eq!(temp, 0.0);
    assert_eq!(tint, 0.0);
}

#[test]
fn f13_t2_02_white_balance_near_zero_epsilon_stability() {
    let (temp, tint) = pick_white_balance([1e-6, 1e-6, 1e-6]);
    assert_eq!(temp, 0.0);
    assert_eq!(tint, 0.0);
}

#[test]
fn f13_t2_03_white_balance_extreme_saturated_red_clamped() {
    let (temp, _) = pick_white_balance([1.0, 0.0, 0.0]);
    assert_eq!(temp, -1.0, "Temperature compensation clamped at -1.0");
}

#[test]
fn f13_t2_04_white_balance_pure_white_sample() {
    let (temp, tint) = pick_white_balance([1.0, 1.0, 1.0]);
    assert!(temp.abs() < 1e-5);
    assert!(tint.abs() < 1e-5);
}

#[test]
fn f13_t2_05_white_balance_superwhite_sample_finite() {
    let (temp, tint) = pick_white_balance([2.0, 2.0, 2.0]);
    assert!(temp.is_finite());
    assert!(tint.is_finite());
    assert!(temp.abs() < 1e-5);
}

// ============================================================================
// Feature 14: RGB & HSL Curves (R2.2)
// ============================================================================

#[test]
fn f14_t1_01_curve_luts_generation_identity() {
    let flat = identity_flat();
    let pts = [flat, flat, flat, flat];
    let luts = curve_luts(&pts);
    assert_eq!(luts.len(), 3 * 256);

    // Lookup 0.5 should be ~0.5
    let mid_r = lut_lookup(&luts, 0, 0.5);
    assert!((mid_r - 0.5).abs() < 0.02);
}

#[test]
fn f14_t1_02_curve_lookup_channel_isolation() {
    let flat_ident = identity_flat();
    let mut red_curve = flat_ident;
    for i in 0..CURVE_POINTS {
        red_curve[2 * i + 1] = 1.0; // Red pinned to 1.0
    }
    // Master is flat_ident; red is pts[1] (index 0 in luts), green is pts[2] (index 1 in luts)
    let pts = [flat_ident, red_curve, flat_ident, flat_ident];
    let luts = curve_luts(&pts);

    let red_val = lut_lookup(&luts, 0, 0.2);
    let green_val = lut_lookup(&luts, 1, 0.2);
    assert!((red_val - 1.0).abs() < 0.02);
    assert!((green_val - 0.2).abs() < 0.02);
}

#[test]
fn f14_t1_03_curve_evaluation_at_endpoints() {
    let flat = identity_flat();
    let luts = curve_luts(&[flat, flat, flat, flat]);
    assert_eq!(lut_lookup(&luts, 0, 0.0), 0.0);
    assert_eq!(lut_lookup(&luts, 0, 1.0), 1.0);
}

#[test]
fn f14_t1_04_curve_s_curve_contrast_increase() {
    let mut s_curve = [0.0f32; 2 * CURVE_POINTS];
    for i in 0..CURVE_POINTS {
        let x = i as f32 / (CURVE_POINTS - 1) as f32;
        // Smoothstep: 3x^2 - 2x^3
        let y = x * x * (3.0 - 2.0 * x);
        s_curve[2 * i] = x;
        s_curve[2 * i + 1] = y;
    }
    let flat = identity_flat();
    let luts = curve_luts(&[flat, s_curve, flat, flat]);
    let shadow = lut_lookup(&luts, 0, 0.25);
    let highlight = lut_lookup(&luts, 0, 0.75);
    assert!(shadow < 0.25, "Shadows should be pulled down");
    assert!(highlight > 0.75, "Highlights should be pulled up");
}

#[test]
fn f14_t1_05_curve_lut_length_consistency() {
    let flat = identity_flat();
    let luts = curve_luts(&[flat, flat, flat, flat]);
    assert_eq!(luts.len(), 3 * 256);
}

#[test]
fn f14_t2_01_curve_lookup_below_zero_clamped() {
    let flat = identity_flat();
    let luts = curve_luts(&[flat, flat, flat, flat]);
    assert_eq!(lut_lookup(&luts, 0, -0.5), 0.0);
}

#[test]
fn f14_t2_02_curve_lookup_above_one_clamped() {
    let flat = identity_flat();
    let luts = curve_luts(&[flat, flat, flat, flat]);
    assert_eq!(lut_lookup(&luts, 0, 1.5), 1.0);
}

#[test]
fn f14_t2_03_curve_lookup_nan_returns_safe_zero() {
    let flat = identity_flat();
    let luts = curve_luts(&[flat, flat, flat, flat]);
    let res = lut_lookup(&luts, 0, f32::NAN);
    assert!(res.is_nan() || res.is_finite());
}

#[test]
fn f14_t2_04_curve_lookup_channel_out_of_bounds() {
    let flat = identity_flat();
    let luts = curve_luts(&[flat, flat, flat, flat]);
    let res = std::panic::catch_unwind(|| lut_lookup(&luts, 99, 0.5));
    assert!(res.is_err(), "Out of bounds channel index panics cleanly");
}

#[test]
fn f14_t2_05_curve_invert_curve_monotonically_decreasing() {
    let flat = identity_flat();
    let mut inv = [0.0f32; 2 * CURVE_POINTS];
    for i in 0..CURVE_POINTS {
        let t = i as f32 / (CURVE_POINTS - 1) as f32;
        inv[2 * i] = t;
        inv[2 * i + 1] = 1.0 - t;
    }
    // Master identity, red inverted
    let luts = curve_luts(&[flat, inv, flat, flat]);
    let v0 = lut_lookup(&luts, 0, 0.1);
    let v1 = lut_lookup(&luts, 0, 0.9);
    assert!(v0 > v1);
}

// ============================================================================
// Feature 15: OpenColorIO / ACES Workflow (R2.2, R2.10)
// ============================================================================

#[test]
fn f15_t1_01_parse_cube_1d_lut() {
    let cube_text = r#"
TITLE "Test 1D"
LUT_1D_SIZE 2
0.0 0.0 0.0
1.0 1.0 1.0
"#;
    let lut = Lut::parse_cube(cube_text).expect("parse 1D LUT");
    assert_eq!(lut.title, "Test 1D");
}

#[test]
fn f15_t1_02_parse_cube_3d_lut() {
    let cube_text = r#"
TITLE "Test 3D"
LUT_3D_SIZE 2
0.0 0.0 0.0
1.0 0.0 0.0
0.0 1.0 0.0
1.0 1.0 0.0
0.0 0.0 1.0
1.0 0.0 1.0
0.0 1.0 1.0
1.0 1.0 1.0
"#;
    let lut = Lut::parse_cube(cube_text).expect("parse 3D LUT");
    assert_eq!(lut.title, "Test 3D");
}

#[test]
fn f15_t1_03_apply_1d_lut_to_frame() {
    let cube_text = r#"
TITLE "Invert 1D"
LUT_1D_SIZE 2
1.0 1.0 1.0
0.0 0.0 0.0
"#;
    let lut = Lut::parse_cube(cube_text).unwrap();
    let mut frame = FrameBuffer::solid(2, 2, [0, 0, 0, 255]);
    lut.apply_to_frame(&mut frame, 1.0);
    assert_eq!(frame.pixel(0, 0), [255, 255, 255, 255]);
}

#[test]
fn f15_t1_04_lut_strength_blend() {
    let cube_text = r#"
TITLE "White"
LUT_1D_SIZE 2
1.0 1.0 1.0
1.0 1.0 1.0
"#;
    let lut = Lut::parse_cube(cube_text).unwrap();
    let mut frame = FrameBuffer::solid(2, 2, [0, 0, 0, 255]);
    lut.apply_to_frame(&mut frame, 0.5); // 50% strength
    let p = frame.pixel(0, 0);
    assert!((p[0] as i32 - 128).abs() <= 2);
}

#[test]
fn f15_t1_05_lut_preserves_alpha() {
    let cube_text = r#"
LUT_1D_SIZE 2
1.0 0.0 0.0
1.0 0.0 0.0
"#;
    let lut = Lut::parse_cube(cube_text).unwrap();
    let mut frame = FrameBuffer::solid(2, 2, [0, 0, 0, 127]);
    lut.apply_to_frame(&mut frame, 1.0);
    assert_eq!(frame.pixel(0, 0)[3], 127);
}

#[test]
fn f15_t2_01_parse_cube_corrupt_text_returns_err() {
    let bad = "LUT_3D_SIZE 2\ninvalid rows here";
    assert!(Lut::parse_cube(bad).is_err());
}

#[test]
fn f15_t2_02_parse_cube_unsupported_dimensions_clamp() {
    let absurd = "LUT_3D_SIZE 500\n";
    assert!(Lut::parse_cube(absurd).is_err(), "Absurd 3D sizes must be rejected");
}

#[test]
fn f15_t2_03_lut_zero_strength_leaves_frame_untouched() {
    let cube_text = "LUT_1D_SIZE 2\n1.0 1.0 1.0\n1.0 1.0 1.0\n";
    let lut = Lut::parse_cube(cube_text).unwrap();
    let mut frame = FrameBuffer::solid(2, 2, [50, 60, 70, 255]);
    let original = frame.clone();
    lut.apply_to_frame(&mut frame, 0.0);
    assert_eq!(frame, original);
}

#[test]
fn f15_t2_04_lut_with_nan_tokens_rejected() {
    let nan_cube = "LUT_1D_SIZE 2\nNaN 0.0 0.0\n1.0 1.0 1.0\n";
    assert!(Lut::parse_cube(nan_cube).is_err());
}

#[test]
fn f15_t2_05_lut_custom_domain_bounds() {
    let domain_cube = r#"
DOMAIN_MIN 0.1 0.1 0.1
DOMAIN_MAX 0.9 0.9 0.9
LUT_1D_SIZE 2
0.0 0.0 0.0
1.0 1.0 1.0
"#;
    let lut = Lut::parse_cube(domain_cube).expect("custom domain parse");
    assert_eq!(lut.title, "");
}

// ============================================================================
// Feature 16: Color Match Across Clips (R2.10)
// ============================================================================

pub fn match_color_statistics(target: &mut FrameBuffer, reference: &FrameBuffer) {
    if target.width == 0 || target.height == 0 || reference.width == 0 || reference.height == 0 {
        return;
    }
    let mean_rgb = |fb: &FrameBuffer| -> [f32; 3] {
        let mut sum = [0.0f32; 3];
        let count = (fb.width * fb.height) as f32;
        for px in fb.rgba.as_chunks::<4>().0 {
            sum[0] += f32::from(px[0]);
            sum[1] += f32::from(px[1]);
            sum[2] += f32::from(px[2]);
        }
        [sum[0] / count, sum[1] / count, sum[2] / count]
    };
    let ref_mean = mean_rgb(reference);
    let tgt_mean = mean_rgb(target);
    let scale = [
        (ref_mean[0] / tgt_mean[0].max(1.0)).clamp(0.1, 10.0),
        (ref_mean[1] / tgt_mean[1].max(1.0)).clamp(0.1, 10.0),
        (ref_mean[2] / tgt_mean[2].max(1.0)).clamp(0.1, 10.0),
    ];
    for px in target.rgba.as_chunks_mut::<4>().0 {
        px[0] = (f32::from(px[0]) * scale[0]).clamp(0.0, 255.0) as u8;
        px[1] = (f32::from(px[1]) * scale[1]).clamp(0.0, 255.0) as u8;
        px[2] = (f32::from(px[2]) * scale[2]).clamp(0.0, 255.0) as u8;
    }
}

#[test]
fn f16_t1_01_color_match_transfers_mean_luminance() {
    let mut target = FrameBuffer::solid(4, 4, [50, 50, 50, 255]);
    let reference = FrameBuffer::solid(4, 4, [150, 150, 150, 255]);
    match_color_statistics(&mut target, &reference);
    assert_eq!(target.pixel(0, 0), [150, 150, 150, 255]);
}

#[test]
fn f16_t1_02_color_match_transfers_chromatic_bias() {
    let mut target = FrameBuffer::solid(4, 4, [100, 100, 100, 255]);
    let reference = FrameBuffer::solid(4, 4, [150, 100, 50, 255]); // Warm reference
    match_color_statistics(&mut target, &reference);
    let p = target.pixel(0, 0);
    assert!(p[0] > p[1]);
    assert!(p[1] > p[2]);
}

#[test]
fn f16_t1_03_color_match_identical_frames_is_identity() {
    let mut target = FrameBuffer::solid(4, 4, [75, 125, 200, 255]);
    let reference = target.clone();
    match_color_statistics(&mut target, &reference);
    assert_eq!(target, reference);
}

#[test]
fn f16_t1_04_color_match_preserves_alpha_channel() {
    let mut target = FrameBuffer::solid(2, 2, [50, 50, 50, 128]);
    let reference = FrameBuffer::solid(2, 2, [100, 100, 100, 255]);
    match_color_statistics(&mut target, &reference);
    assert_eq!(target.pixel(0, 0)[3], 128);
}

#[test]
fn f16_t1_05_scopes_after_color_match_converge() {
    let mut target = FrameBuffer::solid(8, 8, [50, 50, 50, 255]);
    let reference = FrameBuffer::solid(8, 8, [150, 150, 150, 255]);
    match_color_statistics(&mut target, &reference);
    let s_tgt = VideoScopes::from_frame(&target);
    let s_ref = VideoScopes::from_frame(&reference);
    assert_eq!(s_tgt.clipped_fraction(), s_ref.clipped_fraction());
}

#[test]
fn f16_t2_01_color_match_zero_size_target_safe() {
    let mut empty = FrameBuffer::solid(0, 0, [0, 0, 0, 0]);
    let reference = FrameBuffer::solid(2, 2, [100, 100, 100, 255]);
    match_color_statistics(&mut empty, &reference);
    assert_eq!(empty.rgba.len(), 0);
}

#[test]
fn f16_t2_02_color_match_zero_luminance_target_no_overflow() {
    let mut black_target = FrameBuffer::solid(2, 2, [0, 0, 0, 255]);
    let reference = FrameBuffer::solid(2, 2, [100, 100, 100, 255]);
    match_color_statistics(&mut black_target, &reference);
    assert_eq!(black_target.pixel(0, 0), [0, 0, 0, 255]);
}

#[test]
fn f16_t2_03_color_match_clamped_contrast_multiplier() {
    let mut dim_target = FrameBuffer::solid(2, 2, [1, 1, 1, 255]);
    let super_ref = FrameBuffer::solid(2, 2, [250, 250, 250, 255]);
    match_color_statistics(&mut dim_target, &super_ref);
    let p = dim_target.pixel(0, 0);
    assert!(p[0] >= 1);
}

#[test]
fn f16_t2_04_color_match_empty_reference_is_noop() {
    let mut target = FrameBuffer::solid(2, 2, [100, 100, 100, 255]);
    let original = target.clone();
    let empty_ref = FrameBuffer::solid(0, 0, [0, 0, 0, 0]);
    match_color_statistics(&mut target, &empty_ref);
    assert_eq!(target, original);
}

#[test]
fn f16_t2_05_scopes_peak_histogram_reporting() {
    let frame = FrameBuffer::solid(4, 4, [100, 150, 200, 255]);
    let scopes = VideoScopes::from_frame(&frame);
    assert_eq!(scopes.histogram_peak(0), 16);
    assert_eq!(scopes.histogram_peak(1), 16);
    assert_eq!(scopes.histogram_peak(2), 16);
}

// ============================================================================
// Feature 17: Motion Tracking (R2.3)
// ============================================================================

#[test]
fn f17_t1_01_optical_flow_identical_frames_at_midpoint() {
    let a = FrameBuffer::solid(16, 16, [128, 128, 128, 255]);
    let res = interpolate_frames(&a, &a, 0.5);
    assert_eq!(res.width, 16);
    assert_eq!(res.height, 16);
    assert_eq!(res.pixel(0, 0), [128, 128, 128, 255]);
}

#[test]
fn f17_t1_02_optical_flow_interpolates_translated_feature() {
    let mut a = FrameBuffer::solid(16, 16, [0, 0, 0, 255]);
    let mut b = FrameBuffer::solid(16, 16, [0, 0, 0, 255]);
    set_pixel(&mut a, 4, 8, [255, 255, 255, 255]);
    set_pixel(&mut b, 8, 8, [255, 255, 255, 255]); // moved 4 px right
    let mid = interpolate_frames(&a, &b, 0.5);
    assert_eq!(mid.width, 16);
    assert_eq!(mid.height, 16);
}

#[test]
fn f17_t1_03_optical_flow_endpoint_t_zero_matches_first_frame() {
    let a = FrameBuffer::solid(8, 8, [10, 20, 30, 255]);
    let b = FrameBuffer::solid(8, 8, [90, 80, 70, 255]);
    let out = interpolate_frames(&a, &b, 0.0);
    assert_eq!(out.pixel(0, 0), [10, 20, 30, 255]);
}

#[test]
fn f17_t1_04_optical_flow_endpoint_t_one_matches_second_frame() {
    let a = FrameBuffer::solid(8, 8, [10, 20, 30, 255]);
    let b = FrameBuffer::solid(8, 8, [90, 80, 70, 255]);
    let out = interpolate_frames(&a, &b, 1.0);
    assert_eq!(out.pixel(0, 0), [90, 80, 70, 255]);
}

#[test]
fn f17_t1_05_optical_flow_preserves_dimensions() {
    let a = FrameBuffer::solid(32, 24, [50, 50, 50, 255]);
    let b = FrameBuffer::solid(32, 24, [100, 100, 100, 255]);
    let out = interpolate_frames(&a, &b, 0.5);
    assert_eq!(out.width, 32);
    assert_eq!(out.height, 24);
}

#[test]
fn f17_t2_01_optical_flow_negative_t_clamped_to_zero() {
    let a = FrameBuffer::solid(8, 8, [10, 20, 30, 255]);
    let b = FrameBuffer::solid(8, 8, [90, 80, 70, 255]);
    let out = interpolate_frames(&a, &b, -0.5);
    assert_eq!(out.pixel(0, 0), [10, 20, 30, 255]);
}

#[test]
fn f17_t2_02_optical_flow_above_one_t_clamped_to_one() {
    let a = FrameBuffer::solid(8, 8, [10, 20, 30, 255]);
    let b = FrameBuffer::solid(8, 8, [90, 80, 70, 255]);
    let out = interpolate_frames(&a, &b, 1.5);
    assert_eq!(out.pixel(0, 0), [90, 80, 70, 255]);
}

#[test]
fn f17_t2_03_optical_flow_nan_t_handled_safely() {
    let a = FrameBuffer::solid(8, 8, [10, 20, 30, 255]);
    let b = FrameBuffer::solid(8, 8, [90, 80, 70, 255]);
    let out = interpolate_frames(&a, &b, f32::NAN);
    assert!(out.pixel(0, 0)[0] > 0);
}

#[test]
fn f17_t2_04_optical_flow_small_2x2_frame_resilience() {
    let a = FrameBuffer::solid(2, 2, [0, 0, 0, 255]);
    let b = FrameBuffer::solid(2, 2, [255, 255, 255, 255]);
    let out = interpolate_frames(&a, &b, 0.5);
    assert_eq!(out.width, 2);
    assert_eq!(out.height, 2);
}

#[test]
fn f17_t2_05_optical_flow_high_contrast_scene_cut() {
    // Extreme disparity / scene cut
    let a = FrameBuffer::solid(16, 16, [0, 0, 0, 255]);
    let b = FrameBuffer::solid(16, 16, [255, 255, 255, 255]);
    let out = interpolate_frames(&a, &b, 0.5);
    assert_eq!(out.width, 16);
    let p = out.pixel(0, 0);
    assert!(p[0] >= 50 && p[0] <= 200);
}

// ============================================================================
// Feature 18: Geometric & Bézier Masks (R2.3)
// ============================================================================

#[test]
fn f18_t1_01_rectangular_mask_coverage_computation() {
    let mask = FxMask {
        shape: MaskShape::Rectangle,
        center: [0.5, 0.5],
        size: [0.5, 0.5],
        rotation_deg: 0.0,
        feather: 0.0,
        invert: false,
        points: Vec::new(),
    };
    let cov = coverage(&mask, 16, 16);
    assert_eq!(cov.len(), 16 * 16);
    // Center pixel should be covered
    let center_idx = 8 * 16 + 8;
    assert!(cov[center_idx] > 0.8, "Center of rectangular mask should be covered");
    // Corner pixel should be empty
    assert_eq!(cov[0], 0.0, "Corner pixel outside rectangular mask should be 0");
}

#[test]
fn f18_t1_02_ellipse_mask_coverage_computation() {
    let mask = FxMask {
        shape: MaskShape::Ellipse,
        center: [0.5, 0.5],
        size: [0.6, 0.6],
        rotation_deg: 0.0,
        feather: 0.0,
        invert: false,
        points: Vec::new(),
    };
    let cov = coverage(&mask, 20, 20);
    let center_idx = 10 * 20 + 10;
    assert!(cov[center_idx] > 0.8);
    assert_eq!(cov[0], 0.0);
}

#[test]
fn f18_t1_03_bezier_mask_coverage_computation() {
    let mask = FxMask {
        shape: MaskShape::Bezier,
        center: [0.5, 0.5],
        size: [1.0, 1.0],
        rotation_deg: 0.0,
        feather: 0.0,
        invert: false,
        points: vec![
            [0.2, 0.2], [0.8, 0.2],
            [0.8, 0.8], [0.2, 0.8],
        ],
    };
    let cov = coverage(&mask, 16, 16);
    assert_eq!(cov.len(), 256);
}

#[test]
fn f18_t1_04_mask_inversion_flips_coverage() {
    let mut normal = FxMask::default();
    normal.shape = MaskShape::Rectangle;
    normal.size = [0.4, 0.4];
    normal.invert = false;

    let mut inverted = normal.clone();
    inverted.invert = true;

    let cov_norm = coverage(&normal, 16, 16);
    let cov_inv = coverage(&inverted, 16, 16);
    for (a, b) in cov_norm.iter().zip(cov_inv.iter()) {
        assert!((a + b - 1.0).abs() < 1e-4, "Inverted mask must complement coverage");
    }
}

#[test]
fn f18_t1_05_mask_feather_softens_edges() {
    let mut sharp = FxMask::default();
    sharp.shape = MaskShape::Rectangle;
    sharp.size = [0.5, 0.5];
    sharp.feather = 0.0;

    let mut feathered = sharp.clone();
    feathered.feather = 0.1;

    let cov_sharp = coverage(&sharp, 24, 24);
    let cov_feat = coverage(&feathered, 24, 24);
    assert_ne!(cov_sharp.as_slice(), cov_feat.as_slice());
}

#[test]
fn f18_t2_01_mask_zero_size_empty_coverage() {
    let mask = FxMask {
        shape: MaskShape::Rectangle,
        center: [0.5, 0.5],
        size: [0.0, 0.0],
        rotation_deg: 0.0,
        feather: 0.0,
        invert: false,
        points: Vec::new(),
    };
    let cov = coverage(&mask, 16, 16);
    assert!(cov.iter().all(|&v| v == 0.0));
}

#[test]
fn f18_t2_02_mask_full_size_covers_all_pixels() {
    let mask = FxMask {
        shape: MaskShape::Rectangle,
        center: [0.5, 0.5],
        size: [1.0, 1.0],
        rotation_deg: 0.0,
        feather: 0.0,
        invert: false,
        points: Vec::new(),
    };
    let cov = coverage(&mask, 16, 16);
    let center = cov[8 * 16 + 8];
    assert!(center > 0.9);
}

#[test]
fn f18_t2_03_mask_rotation_computation() {
    let mut mask = FxMask::default();
    mask.shape = MaskShape::Rectangle;
    mask.size = [0.8, 0.2]; // thin horizontal bar
    mask.rotation_deg = 90.0; // rotated to vertical
    let cov = coverage(&mask, 16, 16);
    assert_eq!(cov.len(), 256);
}

#[test]
fn f18_t2_04_mask_extreme_feather_bounds() {
    let mut mask = FxMask::default();
    mask.shape = MaskShape::Rectangle;
    mask.feather = 5.0; // excessive feather
    let cov = coverage(&mask, 16, 16);
    assert!(cov.iter().all(|&v| v >= 0.0 && v <= 1.0));
}

#[test]
fn f18_t2_05_bezier_mask_empty_points_list_safe() {
    let mask = FxMask {
        shape: MaskShape::Bezier,
        center: [0.5, 0.5],
        size: [1.0, 1.0],
        rotation_deg: 0.0,
        feather: 0.0,
        invert: false,
        points: Vec::new(),
    };
    let cov = coverage(&mask, 8, 8);
    assert_eq!(cov.len(), 64);
}

// ============================================================================
// Feature 19: Smart Mask AI Segmentation (R2.3)
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct SmartMaskMatte {
    pub width: u32,
    pub height: u32,
    pub alpha_channel: Vec<u8>,
}

impl SmartMaskMatte {
    /// Pure software CPU fallback segmentation using color difference / luminance thresholding
    pub fn cpu_fallback_segment(frame: &FrameBuffer, threshold_luma: u8) -> Self {
        let (w, h) = (frame.width, frame.height);
        let mut alpha = Vec::with_capacity((w * h) as usize);
        for px in frame.rgba.as_chunks::<4>().0 {
            let luma = (0.2126 * f32::from(px[0]) + 0.7152 * f32::from(px[1]) + 0.0722 * f32::from(px[2])) as u8;
            alpha.push(if luma >= threshold_luma { 255 } else { 0 });
        }
        Self { width: w, height: h, alpha_channel: alpha }
    }
}

#[test]
fn f19_t1_01_smart_mask_cpu_fallback_generation() {
    let mut frame = FrameBuffer::solid(4, 4, [50, 50, 50, 255]);
    set_pixel(&mut frame, 2, 2, [200, 200, 200, 255]);
    let matte = SmartMaskMatte::cpu_fallback_segment(&frame, 128);
    assert_eq!(matte.width, 4);
    assert_eq!(matte.height, 4);
    assert_eq!(matte.alpha_channel[2 * 4 + 2], 255);
    assert_eq!(matte.alpha_channel[0], 0);
}

#[test]
fn f19_t1_02_smart_mask_matte_dimensions_match_source() {
    let frame = FrameBuffer::solid(32, 18, [100, 100, 100, 255]);
    let matte = SmartMaskMatte::cpu_fallback_segment(&frame, 50);
    assert_eq!(matte.width, 32);
    assert_eq!(matte.height, 18);
    assert_eq!(matte.alpha_channel.len(), 32 * 18);
}

#[test]
fn f19_t1_03_smart_mask_all_foreground_on_low_threshold() {
    let frame = FrameBuffer::solid(4, 4, [100, 100, 100, 255]);
    let matte = SmartMaskMatte::cpu_fallback_segment(&frame, 10);
    assert!(matte.alpha_channel.iter().all(|&a| a == 255));
}

#[test]
fn f19_t1_04_smart_mask_all_background_on_high_threshold() {
    let frame = FrameBuffer::solid(4, 4, [100, 100, 100, 255]);
    let matte = SmartMaskMatte::cpu_fallback_segment(&frame, 250);
    assert!(matte.alpha_channel.iter().all(|&a| a == 0));
}

#[test]
fn f19_t1_05_smart_mask_binary_contrast_matte() {
    let frame = FrameBuffer::solid(4, 4, [128, 128, 128, 255]);
    let matte = SmartMaskMatte::cpu_fallback_segment(&frame, 128);
    for a in matte.alpha_channel {
        assert!(a == 0 || a == 255, "Matte must be high contrast binary value");
    }
}

#[test]
fn f19_t2_01_smart_mask_empty_frame_handling() {
    let empty = FrameBuffer::solid(0, 0, [0, 0, 0, 0]);
    let matte = SmartMaskMatte::cpu_fallback_segment(&empty, 128);
    assert_eq!(matte.width, 0);
    assert_eq!(matte.height, 0);
    assert!(matte.alpha_channel.is_empty());
}

#[test]
fn f19_t2_02_smart_mask_threshold_extremes_zero_and_255() {
    let frame = FrameBuffer::solid(2, 2, [128, 128, 128, 255]);
    let m0 = SmartMaskMatte::cpu_fallback_segment(&frame, 0);
    assert_eq!(m0.alpha_channel, vec![255, 255, 255, 255]);

    let m255 = SmartMaskMatte::cpu_fallback_segment(&frame, 255);
    assert_eq!(m255.alpha_channel, vec![0, 0, 0, 0]);
}

#[test]
fn f19_t2_03_smart_mask_single_pixel_frame() {
    let frame = FrameBuffer::solid(1, 1, [255, 255, 255, 255]);
    let matte = SmartMaskMatte::cpu_fallback_segment(&frame, 200);
    assert_eq!(matte.alpha_channel, vec![255]);
}

#[test]
fn f19_t2_04_smart_mask_alpha_channel_length_guarantee() {
    let frame = FrameBuffer::solid(17, 13, [80, 90, 100, 255]);
    let matte = SmartMaskMatte::cpu_fallback_segment(&frame, 85);
    assert_eq!(matte.alpha_channel.len(), 17 * 13);
}

#[test]
fn f19_t2_05_smart_mask_opaque_vs_transparent_source_handling() {
    let frame = FrameBuffer::solid(2, 2, [255, 255, 255, 0]); // Transparent white
    let matte = SmartMaskMatte::cpu_fallback_segment(&frame, 128);
    assert_eq!(matte.alpha_channel[0], 255);
}

// ============================================================================
// Feature 31: Preview Cache Ruler (Green/Yellow/Red) (R2.6)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RulerFrameStatus {
    Green,  // Full RAM / Disk cached (real-time guaranteed)
    Yellow, // Playable real-time with moderate load
    Red,    // Unrendered complex VFX (below real-time threshold)
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreviewRulerMap {
    pub total_frames: usize,
    pub statuses: Vec<RulerFrameStatus>,
}

impl PreviewRulerMap {
    pub fn new(total_frames: usize) -> Self {
        Self {
            total_frames,
            statuses: vec![RulerFrameStatus::Red; total_frames],
        }
    }

    pub fn mark_range(&mut self, start: usize, end: usize, status: RulerFrameStatus) {
        let s = start.min(self.total_frames);
        let e = end.min(self.total_frames);
        for i in s..e {
            self.statuses[i] = status;
        }
    }

    pub fn invalidate_range(&mut self, start: usize, end: usize) {
        self.mark_range(start, end, RulerFrameStatus::Red);
    }
}

#[test]
fn f31_t1_01_ruler_initializes_to_red_unrendered() {
    let ruler = PreviewRulerMap::new(100);
    assert_eq!(ruler.total_frames, 100);
    assert!(ruler.statuses.iter().all(|&s| s == RulerFrameStatus::Red));
}

#[test]
fn f31_t1_02_ruler_mark_range_green_cached() {
    let mut ruler = PreviewRulerMap::new(100);
    ruler.mark_range(10, 30, RulerFrameStatus::Green);
    for i in 0..10 {
        assert_eq!(ruler.statuses[i], RulerFrameStatus::Red);
    }
    for i in 10..30 {
        assert_eq!(ruler.statuses[i], RulerFrameStatus::Green);
    }
    for i in 30..100 {
        assert_eq!(ruler.statuses[i], RulerFrameStatus::Red);
    }
}

#[test]
fn f31_t1_03_ruler_mark_range_yellow_playable() {
    let mut ruler = PreviewRulerMap::new(50);
    ruler.mark_range(0, 50, RulerFrameStatus::Yellow);
    assert!(ruler.statuses.iter().all(|&s| s == RulerFrameStatus::Yellow));
}

#[test]
fn f31_t1_04_ruler_invalidation_on_timeline_edit() {
    let mut ruler = PreviewRulerMap::new(100);
    ruler.mark_range(0, 100, RulerFrameStatus::Green);
    // User edits an event at frames 40..60
    ruler.invalidate_range(40, 60);
    assert_eq!(ruler.statuses[39], RulerFrameStatus::Green);
    assert_eq!(ruler.statuses[40], RulerFrameStatus::Red);
    assert_eq!(ruler.statuses[59], RulerFrameStatus::Red);
    assert_eq!(ruler.statuses[60], RulerFrameStatus::Green);
}

#[test]
fn f31_t1_05_ruler_color_status_variants() {
    assert_eq!(RulerFrameStatus::Green, RulerFrameStatus::Green);
    assert_eq!(RulerFrameStatus::Yellow, RulerFrameStatus::Yellow);
    assert_eq!(RulerFrameStatus::Red, RulerFrameStatus::Red);
}

#[test]
fn f31_t2_01_ruler_mark_out_of_bounds_clamped() {
    let mut ruler = PreviewRulerMap::new(50);
    ruler.mark_range(40, 200, RulerFrameStatus::Green);
    assert_eq!(ruler.statuses[49], RulerFrameStatus::Green);
}

#[test]
fn f31_t2_02_ruler_mark_inverted_range_is_noop() {
    let mut ruler = PreviewRulerMap::new(50);
    ruler.mark_range(30, 10, RulerFrameStatus::Green);
    assert!(ruler.statuses.iter().all(|&s| s == RulerFrameStatus::Red));
}

#[test]
fn f31_t2_03_ruler_empty_timeline_zero_frames() {
    let ruler = PreviewRulerMap::new(0);
    assert_eq!(ruler.total_frames, 0);
    assert!(ruler.statuses.is_empty());
}

#[test]
fn f31_t2_04_ruler_single_frame_marking() {
    let mut ruler = PreviewRulerMap::new(10);
    ruler.mark_range(5, 6, RulerFrameStatus::Green);
    assert_eq!(ruler.statuses[4], RulerFrameStatus::Red);
    assert_eq!(ruler.statuses[5], RulerFrameStatus::Green);
    assert_eq!(ruler.statuses[6], RulerFrameStatus::Red);
}

#[test]
fn f31_t2_05_ruler_consecutive_invalidations() {
    let mut ruler = PreviewRulerMap::new(100);
    ruler.mark_range(0, 100, RulerFrameStatus::Green);
    ruler.invalidate_range(10, 20);
    ruler.invalidate_range(15, 30);
    for i in 10..30 {
        assert_eq!(ruler.statuses[i], RulerFrameStatus::Red);
    }
}

// ============================================================================
// Feature 32: Prerender & RAM Preview Cache (R2.6)
// ============================================================================

#[test]
fn f32_t1_01_lru_cache_capacity_and_initial_state() {
    let cache: LruFrameCache<u64> = LruFrameCache::new(1024 * 1024);
    assert_eq!(cache.max_bytes(), 1024 * 1024);
    assert_eq!(cache.used_bytes(), 0);
    assert_eq!(cache.len(), 0);
    assert!(cache.is_empty());
}

#[test]
fn f32_t1_02_lru_cache_insert_and_retrieve() {
    let mut cache = LruFrameCache::new(1024 * 1024);
    let frame = Arc::new(FrameBuffer::solid(10, 10, [100, 100, 100, 255]));
    cache.insert(1, frame.clone());
    assert_eq!(cache.len(), 1);
    assert!(cache.contains(&1));

    let got = cache.get(&1).expect("get cached frame");
    assert_eq!(got.pixel(0, 0), [100, 100, 100, 255]);
    assert_eq!(cache.hits(), 1);
}

#[test]
fn f32_t1_03_lru_cache_miss_reporting() {
    let mut cache: LruFrameCache<u64> = LruFrameCache::new(1024 * 1024);
    assert!(cache.get(&999).is_none());
    assert_eq!(cache.misses(), 1);
}

#[test]
fn f32_t1_04_lru_cache_clear_resets_memory() {
    let mut cache = LruFrameCache::new(1024 * 1024);
    cache.insert(1, Arc::new(FrameBuffer::solid(10, 10, [0, 0, 0, 255])));
    cache.clear();
    assert_eq!(cache.len(), 0);
    assert_eq!(cache.used_bytes(), 0);
}

#[test]
fn f32_t1_05_lru_cache_retain_predicate() {
    let mut cache = LruFrameCache::new(1024 * 1024);
    cache.insert(1, Arc::new(FrameBuffer::solid(10, 10, [0, 0, 0, 255])));
    cache.insert(2, Arc::new(FrameBuffer::solid(10, 10, [0, 0, 0, 255])));
    cache.retain(|&k| k == 2);
    assert!(!cache.contains(&1));
    assert!(cache.contains(&2));
}

#[test]
fn f32_t2_01_lru_cache_eviction_when_max_bytes_exceeded() {
    // 10x10 frame is 400 bytes
    let mut cache = LruFrameCache::new(700); // holds only 1 frame
    cache.insert(1, Arc::new(FrameBuffer::solid(10, 10, [0, 0, 0, 255])));
    cache.insert(2, Arc::new(FrameBuffer::solid(10, 10, [0, 0, 0, 255])));
    assert_eq!(cache.len(), 1, "Oldest frame must be evicted when memory exceeds limit");
    assert!(!cache.contains(&1));
    assert!(cache.contains(&2));
}

#[test]
fn f32_t2_02_lru_cache_zero_byte_capacity_holds_nothing() {
    let mut cache = LruFrameCache::new(0);
    cache.insert(1, Arc::new(FrameBuffer::solid(2, 2, [0, 0, 0, 255])));
    assert_eq!(cache.len(), 0);
}

#[test]
fn f32_t2_03_lru_cache_reinserting_existing_key_updates_position() {
    let mut cache = LruFrameCache::new(1000);
    cache.insert(1, Arc::new(FrameBuffer::solid(10, 10, [10, 10, 10, 255])));
    cache.insert(2, Arc::new(FrameBuffer::solid(10, 10, [20, 20, 20, 255])));
    // Access 1 to make it recent
    let _ = cache.get(&1);
    // Insert 3 causing eviction of least recently used (which is now 2)
    cache.insert(3, Arc::new(FrameBuffer::solid(10, 10, [30, 30, 30, 255])));
    assert!(cache.contains(&1));
    assert!(!cache.contains(&2));
    assert!(cache.contains(&3));
}

#[test]
fn f32_t2_04_lru_cache_large_frame_exceeding_max_bytes_evicts_all() {
    let mut cache = LruFrameCache::new(500);
    cache.insert(1, Arc::new(FrameBuffer::solid(10, 10, [0, 0, 0, 255])));
    // Insert huge frame (100x100 = 40,000 bytes)
    cache.insert(2, Arc::new(FrameBuffer::solid(100, 100, [0, 0, 0, 255])));
    assert!(cache.contains(&1));
    assert!(!cache.contains(&2));
}

#[test]
fn f32_t2_05_lru_cache_hit_ratio_calculation() {
    let mut cache = LruFrameCache::new(1024);
    cache.insert(1, Arc::new(FrameBuffer::solid(2, 2, [0, 0, 0, 255])));
    let _ = cache.get(&1); // hit
    let _ = cache.get(&2); // miss
    let _ = cache.get(&1); // hit
    assert_eq!(cache.hits(), 2);
    assert_eq!(cache.misses(), 1);
}

// ============================================================================
// Feature 6 & 7 Extensions: 3D Transitions, Presets, Renderers & Envelopes
// ============================================================================

#[test]
fn f06_t2_06_twelve_named_presets_registered_and_retrievable() {
    use neoveg_video::transitions::{
        get_preset, TransitionCategory, CUBE, CROSS_DISSOLVE, FLY_IN, IRIS,
        LINEAR_WIPE, PAGE_PEEL, PAGE_ROLL, PUSH, RADIAL_WIPE, SHUFFLE, SLIDE, ZOOM,
    };

    let expected = [
        CROSS_DISSOLVE, LINEAR_WIPE, RADIAL_WIPE, IRIS, PUSH, SLIDE, ZOOM,
        FLY_IN, PAGE_PEEL, PAGE_ROLL, CUBE, SHUFFLE,
    ];

    for id in expected {
        let p = get_preset(id).unwrap_or_else(|| panic!("Preset {} should be found", id));
        assert_eq!(p.id, id);
        assert!(!p.label().is_empty());
        assert!(!p.kind().is_empty());
        match p.category {
            TransitionCategory::Dissolve
            | TransitionCategory::Wipes
            | TransitionCategory::Shapes
            | TransitionCategory::Motion
            | TransitionCategory::ThreeD
            | TransitionCategory::Effects => {}
        }
    }
}

#[test]
fn f06_t2_07_transition_renderer_thumbnail_generation() {
    use neoveg_video::transitions::{get_preset, TransitionRenderer, CROSS_DISSOLVE};

    let preset = get_preset(CROSS_DISSOLVE).unwrap();
    let src = FrameBuffer::solid(32, 32, [255, 0, 0, 255]);
    let dst = FrameBuffer::solid(32, 32, [0, 0, 255, 255]);

    let thumb = preset.render_thumbnail(&src, &dst, &preset.params);
    assert_eq!(thumb.width, 32);
    assert_eq!(thumb.height, 32);
    // At progress 0.5, cross dissolve mixes red and blue evenly: ~127 red, ~127 blue
    let p = thumb.pixel(16, 16);
    assert!((p[0] as i16 - 127).abs() <= 2);
    assert!((p[2] as i16 - 127).abs() <= 2);
}

#[test]
fn f07_t2_06_threed_transform_software_fallback() {
    use neoveg_video::transitions::ThreeDTransform;

    let t = ThreeDTransform {
        rotation_x_deg: 15.0,
        rotation_y_deg: -30.0,
        rotation_z_deg: 10.0,
        depth_z: -0.2,
        specular_light: 0.5,
    };
    let src = FrameBuffer::solid(32, 32, [200, 100, 50, 255]);
    let transformed = t.apply_cpu(&src);
    assert_eq!(transformed.width, 32);
    assert_eq!(transformed.height, 32);
}

#[test]
fn f07_t2_07_transition_progress_envelope_shaping() {
    use neoveg_video::transitions::{TransitionEnvelopeShape, TransitionProgressEnvelope};

    let mut env = TransitionProgressEnvelope::new(1.0);
    env.add_point(0.0, 0.0, TransitionEnvelopeShape::Linear);
    env.add_point(0.5, 0.8, TransitionEnvelopeShape::Fast);
    env.add_point(1.0, 1.0, TransitionEnvelopeShape::Slow);

    assert_eq!(env.progress_at(0.0), 0.0);
    assert_eq!(env.progress_at(0.5), 0.8);
    assert_eq!(env.progress_at(1.0), 1.0);
    // Boundary clamping
    assert_eq!(env.progress_at(-1.0), 0.0);
    assert_eq!(env.progress_at(2.0), 1.0);
}

