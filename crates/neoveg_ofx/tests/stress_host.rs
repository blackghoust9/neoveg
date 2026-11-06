//! Adversarial Stress & Robustness Tests for OpenFX Host Context
//!
//! Stress-testing OfxHostContext and DefaultOfxHost across:
//! - Malformed / corrupted paths passed to load_bundle
//! - Unknown plugin identifiers passed to execute_effect and fallback_cpu_execute
//! - Zero-dimension or non-standard aspect ratio FrameBuffers
//! - Extreme parameter values (NaN, infinity, extreme negative factors)

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use neoveg_ofx::{DefaultOfxHost, FrameBuffer, OfxHostContext, PluginReport};

fn host_exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_neoveg_ofx_host"))
}

fn create_sample_mock_plugin(id: &str) -> PluginReport {
    PluginReport {
        identifier: id.to_string(),
        api: "OfxImageEffectPluginAPI".to_string(),
        version: [1, 4],
        label: format!("Mock {id}"),
        grouping: "Filter".to_string(),
        description: "Mock test plugin".to_string(),
        contexts: vec!["OfxImageEffectContextFilter".to_string()],
        params: vec![],
        clips: vec!["Source".to_string(), "Output".to_string()],
        problem: None,
    }
}

// ============================================================================
// 1. Malformed / Corrupted Paths passed to load_bundle
// ============================================================================

#[test]
fn stress_bundle_path_nonexistent() {
    let mut host = DefaultOfxHost::new().with_host_exe(host_exe());
    let res = host.load_bundle(Path::new("Z:/definitely_does_not_exist_at_all.ofx"));
    assert!(res.is_err(), "Non-existent path must fail");
    let err = res.unwrap_err();
    assert!(err.contains("does not exist"));
}

#[test]
fn stress_bundle_path_empty_string() {
    let mut host = DefaultOfxHost::new().with_host_exe(host_exe());
    let res = host.load_bundle(Path::new(""));
    assert!(res.is_err(), "Empty path must fail");
    let err = res.unwrap_err();
    assert!(err.contains("does not exist") || err.contains("não existe"));
}

#[test]
fn stress_bundle_path_empty_directory() {
    let temp_dir = std::env::temp_dir().join(format!("neoveg_ofx_stress_empty_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);

    let mut host = DefaultOfxHost::new().with_host_exe(host_exe());
    let res = host.load_bundle(&temp_dir);
    assert!(res.is_err(), "Empty directory cannot be loaded as bundle");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn stress_bundle_path_corrupt_binary_file() {
    let temp_file = std::env::temp_dir().join(format!("neoveg_corrupt_{}.ofx", std::process::id()));
    // Write random corrupt bytes
    fs::write(&temp_file, b"\x7F\x45\x4C\x46\x00\xFF\xFF\xDEADBEEF\x00\x11\x22\x33").unwrap();

    let mut host = DefaultOfxHost::new().with_host_exe(host_exe());
    let res = host.load_bundle(&temp_file);
    assert!(res.is_err(), "Corrupt binary file must be rejected gracefully");

    let _ = fs::remove_file(&temp_file);
}

#[test]
fn stress_bundle_path_zero_byte_file() {
    let temp_file = std::env::temp_dir().join(format!("neoveg_zero_{}.ofx", std::process::id()));
    fs::write(&temp_file, b"").unwrap();

    let mut host = DefaultOfxHost::new().with_host_exe(host_exe());
    let res = host.load_bundle(&temp_file);
    assert!(res.is_err(), "0-byte file must be rejected gracefully without panic");

    let _ = fs::remove_file(&temp_file);
}

#[test]
fn stress_bundle_path_text_file() {
    let temp_file = std::env::temp_dir().join(format!("neoveg_text_{}.ofx", std::process::id()));
    fs::write(&temp_file, b"This is plain text and definitely not a valid binary bundle.").unwrap();

    let mut host = DefaultOfxHost::new().with_host_exe(host_exe());
    let res = host.load_bundle(&temp_file);
    assert!(res.is_err(), "Text file must be rejected cleanly");

    let _ = fs::remove_file(&temp_file);
}

// ============================================================================
// 2. Unknown Plugin Identifiers passed to execute_effect and fallback_cpu_execute
// ============================================================================

#[test]
fn stress_execute_unknown_plugin_on_empty_host() {
    let host = DefaultOfxHost::new();
    let src = FrameBuffer::solid(4, 4, [10, 20, 30, 255]);
    let params = BTreeMap::new();

    let res = host.execute_effect("nonexistent.plugin.id", &src, &params);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(err.contains("not found"), "Error was: {err}");
}

#[test]
fn stress_execute_unknown_plugin_with_registered_plugins() {
    let mut host = DefaultOfxHost::new();
    let report = create_sample_mock_plugin("com.neoveg.sample.invert");
    host.register_plugin(report, PathBuf::from("mock.ofx"));

    let src = FrameBuffer::solid(4, 4, [10, 20, 30, 255]);
    let params = BTreeMap::new();

    let res = host.execute_effect("com.other.vendor.blur", &src, &params);
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("not found"));
}

#[test]
fn stress_execute_empty_and_garbage_identifiers() {
    let host = DefaultOfxHost::new();
    let src = FrameBuffer::solid(2, 2, [0, 0, 0, 255]);
    let params = BTreeMap::new();

    assert!(host.execute_effect("", &src, &params).is_err());
    assert!(host.execute_effect(" \t\n ", &src, &params).is_err());
    let long_garbage = "A".repeat(10000);
    assert!(host.execute_effect(&long_garbage, &src, &params).is_err());
}

#[test]
fn stress_fallback_cpu_execute_unknown_identifiers() {
    let host = DefaultOfxHost::new();
    let src = FrameBuffer::solid(4, 4, [12, 34, 56, 255]);
    let params = BTreeMap::new();

    // Unknown plugin identifier must return unmodified buffer without crashing
    let out1 = host.fallback_cpu_execute("com.completely.unknown", &src, &params);
    assert_eq!(out1, src);

    let out2 = host.fallback_cpu_execute("", &src, &params);
    assert_eq!(out2, src);

    let out3 = host.fallback_cpu_execute("!@#$%^&*()_+", &src, &params);
    assert_eq!(out3, src);
}

// ============================================================================
// 3. Zero-Dimension or Non-Standard Aspect Ratio FrameBuffers
// ============================================================================

#[test]
fn stress_zero_dimensions_framebuffer() {
    let mut host = DefaultOfxHost::new();
    let report = create_sample_mock_plugin("com.neoveg.invert");
    host.register_plugin(report, PathBuf::from("mock.ofx"));
    let params = BTreeMap::new();

    // 0x0
    let fb_0x0 = FrameBuffer::new(0, 0);
    let res_0x0 = host.execute_effect("com.neoveg.invert", &fb_0x0, &params).expect("0x0 must not panic");
    assert_eq!(res_0x0.width, 0);
    assert_eq!(res_0x0.height, 0);
    assert!(res_0x0.rgba.is_empty());

    // 0x100
    let fb_0x100 = FrameBuffer::new(0, 100);
    let res_0x100 = host.execute_effect("com.neoveg.invert", &fb_0x100, &params).expect("0x100 must not panic");
    assert_eq!(res_0x100.width, 0);
    assert_eq!(res_0x100.height, 100);

    // 100x0
    let fb_100x0 = FrameBuffer::new(100, 0);
    let res_100x0 = host.execute_effect("com.neoveg.invert", &fb_100x0, &params).expect("100x0 must not panic");
    assert_eq!(res_100x0.width, 100);
    assert_eq!(res_100x0.height, 0);
}

#[test]
fn stress_empty_rgba_with_nonzero_dimensions() {
    let host = DefaultOfxHost::new();
    let malformed = FrameBuffer {
        width: 10,
        height: 10,
        rgba: Vec::new(), // empty rgba
    };
    let params = BTreeMap::new();
    let out = host.fallback_cpu_execute("invert", &malformed, &params);
    assert_eq!(out.width, 10);
    assert_eq!(out.height, 10);
    assert!(out.rgba.is_empty());
}

#[test]
fn stress_misaligned_rgba_slice() {
    let host = DefaultOfxHost::new();
    // Non-multiple of 4
    let misaligned = FrameBuffer {
        width: 2,
        height: 2,
        rgba: vec![1, 2, 3, 4, 5], // 5 bytes instead of 16
    };
    let params = BTreeMap::new();
    let out = host.fallback_cpu_execute("invert", &misaligned, &params);
    // Should process available 4-byte chunk and not panic on remaining byte
    assert_eq!(out.rgba.len(), 5);
}

#[test]
fn stress_extreme_aspect_ratios() {
    let mut host = DefaultOfxHost::new();
    host.register_plugin(create_sample_mock_plugin("invert"), PathBuf::from("mock.ofx"));
    host.register_plugin(create_sample_mock_plugin("brightness"), PathBuf::from("mock.ofx"));
    host.register_plugin(create_sample_mock_plugin("grayscale"), PathBuf::from("mock.ofx"));

    let mut params = BTreeMap::new();
    params.insert("amount".to_string(), 1.0);
    params.insert("gain".to_string(), 2.0);

    // 1x1 single pixel
    let fb_1x1 = FrameBuffer::solid(1, 1, [100, 150, 200, 255]);
    let out_1x1 = host.execute_effect("invert", &fb_1x1, &params).unwrap();
    assert_eq!(out_1x1.pixel(0, 0), [155, 105, 55, 255]);

    // 1x10000 extremely tall
    let fb_tall = FrameBuffer::solid(1, 10000, [50, 50, 50, 255]);
    let out_tall = host.execute_effect("brightness", &fb_tall, &params).unwrap();
    assert_eq!(out_tall.pixel(0, 9999), [100, 100, 100, 255]);

    // 10000x1 extremely wide
    let fb_wide = FrameBuffer::solid(10000, 1, [50, 50, 50, 255]);
    let out_wide = host.execute_effect("brightness", &fb_wide, &params).unwrap();
    assert_eq!(out_wide.pixel(9999, 0), [100, 100, 100, 255]);

    // 21:9 ultra-wide (e.g. 210 x 90)
    let fb_21_9 = FrameBuffer::solid(210, 90, [30, 60, 90, 255]);
    let out_21_9 = host.execute_effect("grayscale", &fb_21_9, &params).unwrap();
    let expected_gray = (0.299f32 * 30.0 + 0.587 * 60.0 + 0.114 * 90.0).clamp(0.0, 255.0) as u8;
    assert_eq!(out_21_9.pixel(100, 50), [expected_gray, expected_gray, expected_gray, 255]);

    // 9:16 vertical video (e.g. 90 x 160)
    let fb_9_16 = FrameBuffer::solid(90, 160, [10, 20, 30, 255]);
    let out_9_16 = host.execute_effect("invert", &fb_9_16, &params).unwrap();
    assert_eq!(out_9_16.pixel(45, 80), [245, 235, 225, 255]);

    // Prime dimensions (17 x 31)
    let fb_prime = FrameBuffer::solid(17, 31, [100, 100, 100, 255]);
    let out_prime = host.execute_effect("invert", &fb_prime, &params).unwrap();
    assert_eq!(out_prime.pixel(16, 30), [155, 155, 155, 255]);
}

#[test]
fn stress_framebuffer_out_of_bounds_pixels() {
    let mut fb = FrameBuffer::solid(5, 5, [1, 2, 3, 255]);
    assert_eq!(fb.pixel(5, 5), [0, 0, 0, 0]);
    assert_eq!(fb.pixel(100, 100), [0, 0, 0, 0]);
    assert_eq!(fb.pixel(u32::MAX, u32::MAX), [0, 0, 0, 0]);

    // set_pixel out of bounds must silently do nothing without panic
    fb.set_pixel(5, 5, [255, 255, 255, 255]);
    fb.set_pixel(u32::MAX, u32::MAX, [255, 255, 255, 255]);
    assert_eq!(fb.pixel(4, 4), [1, 2, 3, 255]);
}

// ============================================================================
// 4. Extreme Parameter Values (NaN, Infinity, Extreme Negatives, Huge Numbers)
// ============================================================================

#[test]
fn stress_invert_nan_parameter() {
    let mut host = DefaultOfxHost::new();
    host.register_plugin(create_sample_mock_plugin("invert"), PathBuf::from("mock.ofx"));

    let src = FrameBuffer::solid(2, 2, [100, 150, 200, 255]);
    let mut params = BTreeMap::new();
    params.insert("amount".to_string(), f32::NAN);

    let out = host.execute_effect("invert", &src, &params).expect("NaN param must not crash");
    // When NaN is passed, float arithmetic with NaN saturates to 0 in Rust as u8
    for chunk in out.rgba.as_chunks::<4>().0 {
        assert_eq!(chunk[3], 255, "Alpha channel preserved");
    }
}

#[test]
fn stress_invert_infinity_parameters() {
    let mut host = DefaultOfxHost::new();
    host.register_plugin(create_sample_mock_plugin("invert"), PathBuf::from("mock.ofx"));

    let src = FrameBuffer::solid(2, 2, [100, 150, 200, 255]);
    let mut params = BTreeMap::new();

    // Positive infinity: clamps to 1.0
    params.insert("amount".to_string(), f32::INFINITY);
    let out_inf = host.execute_effect("invert", &src, &params).unwrap();
    assert_eq!(out_inf.pixel(0, 0), [155, 105, 55, 255]);

    // Negative infinity: clamps to 0.0 (identity)
    params.insert("amount".to_string(), f32::NEG_INFINITY);
    let out_neginf = host.execute_effect("invert", &src, &params).unwrap();
    assert_eq!(out_neginf.pixel(0, 0), [100, 150, 200, 255]);
}

#[test]
fn stress_invert_extreme_negative_and_positive_amounts() {
    let mut host = DefaultOfxHost::new();
    host.register_plugin(create_sample_mock_plugin("invert"), PathBuf::from("mock.ofx"));

    let src = FrameBuffer::solid(2, 2, [100, 150, 200, 255]);
    let mut params = BTreeMap::new();

    // -1e9 clamps to 0.0 (identity)
    params.insert("amount".to_string(), -1e9);
    let out_neg = host.execute_effect("invert", &src, &params).unwrap();
    assert_eq!(out_neg.pixel(0, 0), [100, 150, 200, 255]);

    // +1e9 clamps to 1.0 (full invert)
    params.insert("amount".to_string(), 1e9);
    let out_pos = host.execute_effect("invert", &src, &params).unwrap();
    assert_eq!(out_pos.pixel(0, 0), [155, 105, 55, 255]);
}

#[test]
fn stress_gain_and_brightness_nan_parameter() {
    let mut host = DefaultOfxHost::new();
    host.register_plugin(create_sample_mock_plugin("brightness"), PathBuf::from("mock.ofx"));

    let src = FrameBuffer::solid(2, 2, [100, 150, 200, 255]);
    let mut params = BTreeMap::new();
    params.insert("brightness".to_string(), f32::NAN);

    let out = host.execute_effect("brightness", &src, &params).expect("NaN must not panic");
    // NaN clamps to 0 in saturating cast to u8
    for chunk in out.rgba.as_chunks::<4>().0 {
        assert_eq!(chunk[0], 0);
        assert_eq!(chunk[1], 0);
        assert_eq!(chunk[2], 0);
        assert_eq!(chunk[3], 255);
    }
}

#[test]
fn stress_gain_and_brightness_infinity_parameter() {
    let mut host = DefaultOfxHost::new();
    host.register_plugin(create_sample_mock_plugin("gain"), PathBuf::from("mock.ofx"));

    let src = FrameBuffer::solid(2, 2, [10, 20, 30, 255]);
    let mut params = BTreeMap::new();
    params.insert("gain".to_string(), f32::INFINITY);

    let out = host.execute_effect("gain", &src, &params).expect("INFINITY must not panic");
    // Non-zero values * INFINITY -> INFINITY, clamps to 255
    assert_eq!(out.pixel(0, 0), [255, 255, 255, 255]);

    // Zero values: 0.0 * INFINITY is NaN, saturating cast to u8 gives 0
    let src_zero = FrameBuffer::solid(2, 2, [0, 0, 0, 255]);
    let out_zero = host.execute_effect("gain", &src_zero, &params).expect("0.0 * INFINITY must not panic");
    assert_eq!(out_zero.pixel(0, 0), [0, 0, 0, 255]);
}

#[test]
fn stress_gain_and_brightness_extreme_negative_factors() {
    let mut host = DefaultOfxHost::new();
    host.register_plugin(create_sample_mock_plugin("gain"), PathBuf::from("mock.ofx"));

    let src = FrameBuffer::solid(2, 2, [100, 150, 200, 255]);
    let mut params = BTreeMap::new();

    // Extreme negative factor: clamps to 0.0
    params.insert("gain".to_string(), -1e12);
    let out_neg = host.execute_effect("gain", &src, &params).unwrap();
    assert_eq!(out_neg.pixel(0, 0), [0, 0, 0, 255]);

    // Negative infinity: clamps to 0.0
    params.insert("gain".to_string(), f32::NEG_INFINITY);
    let out_neginf = host.execute_effect("gain", &src, &params).unwrap();
    assert_eq!(out_neginf.pixel(0, 0), [0, 0, 0, 255]);
}

#[test]
fn stress_gain_and_brightness_huge_positive_factor() {
    let mut host = DefaultOfxHost::new();
    host.register_plugin(create_sample_mock_plugin("gain"), PathBuf::from("mock.ofx"));

    let src = FrameBuffer::solid(2, 2, [1, 1, 1, 255]);
    let mut params = BTreeMap::new();
    params.insert("gain".to_string(), 1e12);

    let out = host.execute_effect("gain", &src, &params).unwrap();
    assert_eq!(out.pixel(0, 0), [255, 255, 255, 255]);
}

#[test]
fn stress_grayscale_channel_boundary_conditions() {
    let mut host = DefaultOfxHost::new();
    host.register_plugin(create_sample_mock_plugin("grayscale"), PathBuf::from("mock.ofx"));
    let params = BTreeMap::new();

    // Pure black
    let black = FrameBuffer::solid(2, 2, [0, 0, 0, 255]);
    let out_black = host.execute_effect("grayscale", &black, &params).unwrap();
    assert_eq!(out_black.pixel(0, 0), [0, 0, 0, 255]);

    // Pure white
    let white = FrameBuffer::solid(2, 2, [255, 255, 255, 255]);
    let out_white = host.execute_effect("grayscale", &white, &params).unwrap();
    assert_eq!(out_white.pixel(0, 0), [255, 255, 255, 255]);

    // Pure Red: 0.299 * 255 = 76.245 -> 76
    let red = FrameBuffer::solid(2, 2, [255, 0, 0, 255]);
    let out_red = host.execute_effect("grayscale", &red, &params).unwrap();
    assert_eq!(out_red.pixel(0, 0), [76, 76, 76, 255]);
}

#[test]
fn stress_empty_params_map_uses_clean_defaults() {
    let mut host = DefaultOfxHost::new();
    host.register_plugin(create_sample_mock_plugin("invert"), PathBuf::from("mock.ofx"));
    host.register_plugin(create_sample_mock_plugin("gain"), PathBuf::from("mock.ofx"));
    host.register_plugin(create_sample_mock_plugin("grayscale"), PathBuf::from("mock.ofx"));

    let src = FrameBuffer::solid(2, 2, [100, 150, 200, 255]);
    let empty_params = BTreeMap::new();

    // Invert default amount = 1.0 (full invert)
    let out_inv = host.execute_effect("invert", &src, &empty_params).unwrap();
    assert_eq!(out_inv.pixel(0, 0), [155, 105, 55, 255]);

    // Gain default factor = 1.0 (identity)
    let out_gain = host.execute_effect("gain", &src, &empty_params).unwrap();
    assert_eq!(out_gain.pixel(0, 0), [100, 150, 200, 255]);

    // Grayscale
    let out_gray = host.execute_effect("grayscale", &src, &empty_params).unwrap();
    let expected_gray = (0.299 * 100.0 + 0.587 * 150.0 + 0.114 * 200.0) as u8;
    assert_eq!(out_gray.pixel(0, 0), [expected_gray, expected_gray, expected_gray, 255]);
}

// ============================================================================
// 5. Concurrency, Case Sensitivity & High-Resolution Stress
// ============================================================================

#[test]
fn stress_concurrent_threaded_effect_execution() {
    use std::sync::Arc;
    use std::thread;

    let mut host = DefaultOfxHost::new();
    host.register_plugin(create_sample_mock_plugin("com.neoveg.invert"), PathBuf::from("mock.ofx"));
    let host = Arc::new(host);

    let mut handles = Vec::new();
    for thread_idx in 0..8 {
        let host_clone = Arc::clone(&host);
        let handle = thread::spawn(move || {
            let src = FrameBuffer::solid(16, 16, [thread_idx * 10, 50, 100, 255]);
            let mut params = BTreeMap::new();
            params.insert("amount".to_string(), 1.0);
            let out = host_clone.execute_effect("com.neoveg.invert", &src, &params).unwrap();
            assert_eq!(out.pixel(0, 0)[0], 255 - thread_idx * 10);
        });
        handles.push(handle);
    }

    for h in handles {
        h.join().expect("Worker thread must succeed");
    }
}

#[test]
fn stress_case_insensitivity_plugin_matching() {
    let mut host = DefaultOfxHost::new();
    host.register_plugin(create_sample_mock_plugin("InVeRt_Effect"), PathBuf::from("mock.ofx"));
    host.register_plugin(create_sample_mock_plugin("BRIGHTNESS_BOOST"), PathBuf::from("mock.ofx"));
    host.register_plugin(create_sample_mock_plugin("GRAYSCALE_FILTER"), PathBuf::from("mock.ofx"));

    let src = FrameBuffer::solid(2, 2, [50, 100, 150, 255]);
    let mut params = BTreeMap::new();
    params.insert("amount".to_string(), 1.0);
    params.insert("brightness".to_string(), 2.0);

    let out_inv = host.execute_effect("InVeRt_Effect", &src, &params).unwrap();
    assert_eq!(out_inv.pixel(0, 0), [205, 155, 105, 255]);

    let out_bright = host.execute_effect("BRIGHTNESS_BOOST", &src, &params).unwrap();
    assert_eq!(out_bright.pixel(0, 0), [100, 200, 255, 255]);

    let out_gray = host.execute_effect("GRAYSCALE_FILTER", &src, &params).unwrap();
    let expected = (0.299 * 50.0 + 0.587 * 100.0 + 0.114 * 150.0) as u8;
    assert_eq!(out_gray.pixel(0, 0), [expected, expected, expected, 255]);
}

#[test]
fn stress_subnormal_float_parameters() {
    let mut host = DefaultOfxHost::new();
    host.register_plugin(create_sample_mock_plugin("gain"), PathBuf::from("mock.ofx"));

    let src = FrameBuffer::solid(2, 2, [100, 100, 100, 255]);
    let mut params = BTreeMap::new();
    params.insert("gain".to_string(), f32::MIN_POSITIVE); // ~1.175e-38

    let out = host.execute_effect("gain", &src, &params).unwrap();
    assert_eq!(out.pixel(0, 0), [0, 0, 0, 255]);
}

#[test]
fn stress_4k_resolution_buffer_processing() {
    let mut host = DefaultOfxHost::new();
    host.register_plugin(create_sample_mock_plugin("invert"), PathBuf::from("mock.ofx"));

    // 3840 x 2160 UHD 4K
    let src = FrameBuffer::solid(3840, 2160, [100, 150, 200, 255]);
    let mut params = BTreeMap::new();
    params.insert("amount".to_string(), 1.0);

    let out = host.execute_effect("invert", &src, &params).unwrap();
    assert_eq!(out.width, 3840);
    assert_eq!(out.height, 2160);
    assert_eq!(out.pixel(0, 0), [155, 105, 55, 255]);
    assert_eq!(out.pixel(3839, 2159), [155, 105, 55, 255]);
}

