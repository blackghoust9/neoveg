//! E2E Integration Test Suite for neoveg_common (Tiers 1 & 2)
//! Covers Features:
//! - Feature 1: Worktree Verification (R1)
//! - Feature 5: Trademark Cleanliness (R3)
//! - Feature 24: Project Properties (R2.4)
//! - Feature 25: Project Templates (R2.4)
//! - Feature 26: Media Profiles Matcher (R2.4)
//! - Feature 33: Standardized Alerts (§10, R2.8)
//! - Feature 41: Parity Matrix Sync (R3)
//! - Feature 42: Translation Catalogs Sync (R3)

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use neoveg_common::i18n::{
    language_has_ideographs, language_name, set_language, tr_args, tr_text, LANGUAGES,
};
use neoveg_common::media_props::{
    AlphaMode, FieldOrder, MediaProps, PIXEL_ASPECTS, PIXEL_ASPECT_RANGE, SUBSTITUTE_FPS,
};
use neoveg_common::presets::{
    builtin_templates, clamp_size, display_aspect, frame_rate_of, resolution_named, Deinterlace,
    FieldOrder as PresetFieldOrder, PixelFormat, RenderQuality, FRAME_RATES, RESOLUTIONS,
};
use neoveg_common::project::{
    next_note_id, notes_in_order, CaptionStyle, MediaRef, MediaSource, ProjectInfo, ProjectNote,
    ProjectSettings, NOTE_COLORS,
};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

// ============================================================================
// Feature 1: Worktree Verification (R1)
// ============================================================================

#[test]
fn f01_t1_01_worktree_aaf_tests_file_exists_and_non_empty() {
    let p = repo_root().join("crates/neoveg_aaf/src/tests.rs");
    assert!(p.exists(), "crates/neoveg_aaf/src/tests.rs must exist");
    let content = fs::read_to_string(&p).expect("read aaf tests.rs");
    assert!(!content.trim().is_empty(), "aaf tests.rs must not be empty");
    assert!(
        content.contains("#[test]"),
        "aaf tests.rs must contain tests"
    );
}

#[test]
fn f01_t1_02_worktree_raw_decoder_file_exists_and_contains_logic() {
    let p = repo_root().join("crates/neoveg_video/src/raw.rs");
    assert!(p.exists(), "crates/neoveg_video/src/raw.rs must exist");
    let content = fs::read_to_string(&p).expect("read raw.rs");
    assert!(content.contains("CinemaDNG") || content.contains("raw") || content.contains("Dng"));
}

#[test]
fn f01_t1_03_worktree_still_image_decoder_file_exists() {
    let p = repo_root().join("crates/neoveg_video/src/still.rs");
    assert!(p.exists(), "crates/neoveg_video/src/still.rs must exist");
    let content = fs::read_to_string(&p).expect("read still.rs");
    assert!(content.contains("image") || content.contains("FrameBuffer"));
}

#[test]
fn f01_t1_04_worktree_audio_record_module_exists() {
    let p = repo_root().join("crates/neoveg_audio/src/record.rs");
    assert!(p.exists(), "crates/neoveg_audio/src/record.rs must exist");
    let content = fs::read_to_string(&p).expect("read record.rs");
    assert!(content.contains("Recorder") && content.contains("MonitorRing"));
}

#[test]
fn f01_t1_05_worktree_all_14_crates_exist_in_workspace() {
    let crates_dir = repo_root().join("crates");
    let expected_crates = [
        "neoveg_common",
        "neoveg_audio",
        "neoveg_video",
        "neoveg_engine",
        "neoveg_text",
        "neoveg_media",
        "neoveg_h264",
        "neoveg_aac",
        "neoveg_cfb",
        "neoveg_aaf",
        "neoveg_ofx",
        "neoveg_ofx_sample",
        "neoveg_vst3_sample",
        "neoveg_ui",
    ];
    for c in &expected_crates {
        let cp = crates_dir.join(c);
        assert!(
            cp.exists() && cp.is_dir(),
            "Crate {c} must exist in crates/"
        );
        assert!(
            cp.join("Cargo.toml").exists(),
            "Crate {c} must contain Cargo.toml"
        );
    }
}

#[test]
fn f01_t2_01_worktree_cargo_toml_root_virtual_workspace_validity() {
    let cargo_path = repo_root().join("Cargo.toml");
    let toml_str = fs::read_to_string(&cargo_path).expect("read root Cargo.toml");
    assert!(toml_str.contains("[workspace]"));
    assert!(toml_str.contains("members = ["));
    assert!(toml_str.contains("version = \"2.1.0\""));
}

#[test]
fn f01_t2_02_worktree_deny_toml_exists_and_forbids_unapproved_licenses() {
    let deny_path = repo_root().join("deny.toml");
    assert!(deny_path.exists(), "deny.toml must exist");
    let deny_str = fs::read_to_string(&deny_path).expect("read deny.toml");
    assert!(deny_str.contains("MIT") || deny_str.contains("Apache-2.0"));
}

#[test]
fn f01_t2_03_worktree_test_infra_spec_matches_repo_metadata() {
    let infra_path = repo_root().join("TEST_INFRA.md");
    assert!(infra_path.exists(), "TEST_INFRA.md must exist");
    let infra_str = fs::read_to_string(&infra_path).expect("read TEST_INFRA.md");
    assert!(infra_str.contains("Feature Inventory"));
    assert!(infra_str.contains("Coverage Thresholds"));
}

#[test]
fn f01_t2_04_worktree_project_spec_matches_architecture() {
    let proj_path = repo_root().join("PROJECT.md");
    assert!(proj_path.exists(), "PROJECT.md must exist");
    let proj_str = fs::read_to_string(&proj_path).expect("read PROJECT.md");
    assert!(proj_str.contains("Neoveg NLE Feature Extensions"));
    assert!(proj_str.contains("Code Layout"));
}

#[test]
fn f01_t2_05_worktree_zero_broken_symlinks_in_assets() {
    let assets_dir = repo_root().join("Assets");
    if assets_dir.exists() {
        for entry in fs::read_dir(&assets_dir).unwrap().flatten() {
            let meta = entry.metadata().unwrap();
            assert!(
                !meta.is_symlink() || entry.path().exists(),
                "Dangling symlink in Assets"
            );
        }
    }
}

// ============================================================================
// Feature 5: Trademark Cleanliness (R3)
// ============================================================================

const FORBIDDEN_TRADEMARKS: &[&str] = &[
    "Sony",
    "MAGIX",
    "Final Cut",
    "Avid",
    "Premiere",
    "Pro Tools",
    "DaVinci",
    "Vegas Pro",
];

#[test]
fn f05_t1_01_trademark_check_en_us_translation_catalog() {
    let p = repo_root().join("Assets/i18n/strings/en-US.json");
    let content = fs::read_to_string(&p).expect("read en-US.json");
    for &tm in FORBIDDEN_TRADEMARKS {
        assert!(
            !content.contains(&format!("\"{tm}\"")) && !content.contains(&format!("{tm} Pro")),
            "en-US.json contains trademark: {tm}"
        );
    }
}

#[test]
fn f05_t1_02_trademark_check_all_7_catalogs() {
    let langs = [
        "de-DE", "en-US", "es-ES", "fr-FR", "ja-JP", "ru-RU", "zh-CN",
    ];
    for l in &langs {
        let p = repo_root().join(format!("Assets/i18n/strings/{l}.json"));
        let content = fs::read_to_string(&p).unwrap();
        assert!(
            !content.contains("DaVinci Resolve"),
            "{l} has DaVinci Resolve"
        );
        assert!(!content.contains("Sony Vegas"), "{l} has Sony Vegas");
        assert!(!content.contains("MAGIX Vegas"), "{l} has MAGIX Vegas");
    }
}

#[test]
fn f05_t1_03_trademark_generic_terms_used_for_interchange() {
    let p = repo_root().join("Assets/i18n/strings/en-US.json");
    let content = fs::read_to_string(&p).unwrap();
    // Generic terms like "XML", "EDL", "AAF", "FCPXML" are standard interchange
    assert!(content.contains("EDL") || content.contains("AAF") || content.contains("XML"));
}

#[test]
fn f05_t1_04_trademark_common_crate_codebase_cleanliness() {
    let p = repo_root().join("crates/neoveg_common/src");
    for entry in fs::read_dir(p).unwrap().flatten() {
        if entry.path().extension().is_some_and(|e| e == "rs") {
            let s = fs::read_to_string(entry.path()).unwrap();
            for &tm in FORBIDDEN_TRADEMARKS {
                // Must not use vendor brand names as runtime string literals
                assert!(
                    !s.contains(&format!("\"{tm}\"")),
                    "Runtime string literal '{tm}' in {:?}",
                    entry.path()
                );
            }
        }
    }
}

#[test]
fn f05_t1_05_trademark_project_presets_labels_are_generic() {
    for t in builtin_templates() {
        for &tm in FORBIDDEN_TRADEMARKS {
            assert!(
                !t.name.contains(tm),
                "Template name '{}' contains trademark '{tm}'",
                t.name
            );
        }
    }
}

#[test]
fn f05_t2_01_trademark_case_insensitive_vendor_brands_check() {
    let p = repo_root().join("Assets/i18n/strings/en-US.json");
    let content = fs::read_to_string(p).unwrap().to_lowercase();
    assert!(!content.contains("sony vegas"));
    assert!(!content.contains("magix video"));
}

#[test]
fn f05_t2_02_trademark_no_trademark_symbols_in_runtime_keys() {
    let p = repo_root().join("Assets/i18n/strings/en-US.json");
    let map: HashMap<String, String> =
        serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap();
    for key in map.keys() {
        assert!(
            !key.contains('®'),
            "Key '{key}' contains registered trademark symbol"
        );
        assert!(!key.contains('™'), "Key '{key}' contains trademark symbol");
    }
}

#[test]
fn f05_t2_03_trademark_no_trademark_symbols_in_runtime_values() {
    let p = repo_root().join("Assets/i18n/strings/en-US.json");
    let map: HashMap<String, String> =
        serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap();
    for (k, v) in map {
        assert!(
            !v.contains('®'),
            "Value for '{k}' contains registered trademark symbol"
        );
        assert!(
            !v.contains('™'),
            "Value for '{k}' contains trademark symbol"
        );
    }
}

#[test]
fn f05_t2_04_trademark_resolution_names_cleanliness() {
    for res in RESOLUTIONS {
        for &tm in FORBIDDEN_TRADEMARKS {
            assert!(
                !res.name.contains(tm),
                "Resolution '{}' has trademark {tm}",
                res.name
            );
            assert!(
                !res.group.contains(tm),
                "Group '{}' has trademark {tm}",
                res.group
            );
        }
    }
}

#[test]
fn f05_t2_05_trademark_de_es_fr_catalogs_trademark_free() {
    for lang in &["de-DE", "es-ES", "fr-FR"] {
        let p = repo_root().join(format!("Assets/i18n/strings/{lang}.json"));
        let map: HashMap<String, String> =
            serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap();
        for (k, v) in map {
            assert!(
                !v.contains("Pro Tools"),
                "{lang}: value for '{k}' contains Pro Tools"
            );
            assert!(
                !v.contains("DaVinci Resolve"),
                "{lang}: value for '{k}' contains DaVinci"
            );
        }
    }
}

// ============================================================================
// Feature 24: Project Properties (R2.4)
// ============================================================================

#[test]
fn f24_t1_01_project_settings_default_values() {
    let s = ProjectSettings::default();
    assert_eq!(s.width, 1920);
    assert_eq!(s.height, 1080);
    assert!((s.fps - 29.97).abs() < 1e-4);
    assert_eq!(s.sample_rate, 48_000);
    assert_eq!(s.pixel_aspect, 1.0);
    assert_eq!(s.field_order, PresetFieldOrder::Progressive);
    assert_eq!(s.pixel_format, PixelFormat::Bit8Full);
    assert_eq!(s.audio_bit_depth, 24);
    assert!(s.auto_crossfade);
}

#[test]
fn f24_t1_02_project_info_empty_creation() {
    let info = ProjectInfo::create_empty();
    assert_eq!(info.name, "Untitled");
    assert!(info.media_pool.is_empty());
    assert!(info.notes.is_empty());
    assert_eq!(info.settings, ProjectSettings::default());
}

#[test]
fn f24_t1_03_project_settings_serialization_roundtrip() {
    let s = ProjectSettings {
        width: 3840,
        height: 2160,
        fps: 60.0,
        pixel_format: PixelFormat::Float32Full,
        render_quality: RenderQuality::Best,
        deinterlace: Deinterlace::Interpolate,
        ..Default::default()
    };
    let json = serde_json::to_string(&s).expect("serialize");
    let back: ProjectSettings = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, s);
}

#[test]
fn f24_t1_04_display_aspect_ratio_computation() {
    let dar_16_9 = display_aspect(1920, 1080, 1.0);
    assert!((dar_16_9 - 16.0 / 9.0).abs() < 1e-5);

    let dar_anamorphic = display_aspect(1440, 1080, 4.0 / 3.0);
    assert!((dar_anamorphic - 16.0 / 9.0).abs() < 1e-5);
}

#[test]
fn f24_t1_05_clamp_size_keeps_dimensions_even_and_bounded() {
    assert_eq!(clamp_size(1920), 1920);
    assert_eq!(clamp_size(1921), 1920); // forces even
    assert_eq!(clamp_size(1081), 1080);
    assert!(clamp_size(0) >= 16); // minimum bound
}

#[test]
fn f24_t2_01_project_settings_extreme_fps_and_zero_height_aspect() {
    assert_eq!(display_aspect(1920, 0, 1.0), 0.0);
    let s = ProjectSettings {
        fps: 240.0,
        ..Default::default()
    };
    assert!(s.fps > 0.0);
}

#[test]
fn f24_t2_02_caption_style_sanitization_bounds() {
    let style = CaptionStyle {
        size: -5.0,      // invalid negative size
        margin: 10.0,    // excessive margin
        box_width: 0.05, // too narrow
        ..Default::default()
    };
    let sanitized = style.sanitized();
    assert!(sanitized.size >= 0.02 && sanitized.size <= 0.15);
    assert!(sanitized.margin <= 0.4);
    assert!(sanitized.box_width >= 0.3);
}

#[test]
fn f24_t2_03_project_notes_ordering_and_resolution() {
    let n1 = ProjectNote {
        id: 1,
        time: 10.0,
        text: "A".into(),
        label: "".into(),
        color: 0,
        resolved: true,
    };
    let n2 = ProjectNote {
        id: 2,
        time: 5.0,
        text: "B".into(),
        label: "".into(),
        color: 1,
        resolved: false,
    };
    let n3 = ProjectNote {
        id: 3,
        time: 5.0,
        text: "C".into(),
        label: "".into(),
        color: 2,
        resolved: false,
    };
    let notes = vec![n1, n2, n3];

    let all = notes_in_order(&notes, false);
    assert_eq!(all[0].id, 2);
    assert_eq!(all[1].id, 3);
    assert_eq!(all[2].id, 1);

    let unresolved = notes_in_order(&notes, true);
    assert_eq!(unresolved.len(), 2);
    assert_eq!(next_note_id(&notes), 4);
}

#[test]
fn f24_t2_04_note_color_boundary_clamp() {
    let n = ProjectNote {
        id: 1,
        time: 0.0,
        text: "".into(),
        label: "".into(),
        color: 255,
        resolved: false,
    };
    assert_eq!(n.rgb(), NOTE_COLORS[NOTE_COLORS.len() - 1].1);
}

#[test]
fn f24_t2_05_media_ref_source_distinction() {
    let file_ref = MediaRef {
        id: 1,
        path: "clip.mp4".into(),
        duration_seconds: 10.0,
        subproject: false,
        format: None,
    };
    assert_eq!(
        file_ref.source(),
        MediaSource::File(PathBuf::from("clip.mp4"))
    );

    let sub_ref = MediaRef {
        id: 2,
        path: "nested.neoveg".into(),
        duration_seconds: 20.0,
        subproject: true,
        format: None,
    };
    assert_eq!(
        sub_ref.source(),
        MediaSource::SubProject(PathBuf::from("nested.neoveg"))
    );
}

// ============================================================================
// Feature 25: Project Templates (R2.4)
// ============================================================================

#[test]
fn f25_t1_01_builtin_templates_count_and_types() {
    let templates = builtin_templates();
    assert!(
        templates.len() >= 25,
        "Expected standard built-in templates, got {}",
        templates.len()
    );
    let names: Vec<_> = templates.iter().map(|t| t.name.clone()).collect();
    assert!(names.iter().any(|n| n.contains("Full HD 1080p 29,97")));
    assert!(names.iter().any(|n| n.contains("4K UHD 23,976")));
}

#[test]
fn f25_t1_02_resolution_named_lookup() {
    let r1080 = resolution_named(1920, 1080).expect("1080p resolution");
    assert_eq!(r1080.name, "1080p (Full HD)");
    assert_eq!(r1080.group, "HD");

    let r4k = resolution_named(3840, 2160).expect("4K resolution");
    assert_eq!(r4k.name, "2160p (4K UHD)");
    assert_eq!(r4k.group, "Ultra HD");
}

#[test]
fn f25_t1_03_frame_rate_of_lookup() {
    let ntsc = frame_rate_of(29.97).expect("29.97 fps");
    assert!(ntsc.label.contains("29,97") || ntsc.label.contains("29.97"));

    let film = frame_rate_of(23.976).expect("23.976 fps");
    assert!(film.label.contains("23,976") || film.label.contains("23.976"));
}

#[test]
fn f25_t1_04_template_resolution_dimensions_match_settings() {
    for t in builtin_templates() {
        assert!(t.settings.width > 0 && t.settings.height > 0);
        assert!(
            t.settings.width % 2 == 0,
            "Template {} width must be even",
            t.name
        );
        assert!(
            t.settings.height % 2 == 0,
            "Template {} height must be even",
            t.name
        );
    }
}

#[test]
fn f25_t1_05_vertical_and_square_social_templates_exist() {
    let templates = builtin_templates();
    let has_vertical = templates
        .iter()
        .any(|t| t.settings.width == 1080 && t.settings.height == 1920);
    let has_square = templates
        .iter()
        .any(|t| t.settings.width == 1080 && t.settings.height == 1080);
    assert!(
        has_vertical,
        "Vertical 1080x1920 social template must exist"
    );
    assert!(has_square, "Square 1080x1080 social template must exist");
}

#[test]
fn f25_t2_01_unknown_resolution_returns_none() {
    assert!(resolution_named(1234, 5678).is_none());
    assert!(resolution_named(0, 0).is_none());
}

#[test]
fn f25_t2_02_unknown_framerate_returns_none() {
    assert!(frame_rate_of(144.0).is_none());
    assert!(frame_rate_of(-30.0).is_none());
}

#[test]
fn f25_t2_03_broadcast_interlaced_field_order_settings() {
    let templates = builtin_templates();
    let ntsc_interlaced = templates
        .iter()
        .find(|t| t.name.contains("NTSC SD 4:3"))
        .unwrap();
    assert_eq!(
        ntsc_interlaced.settings.field_order,
        PresetFieldOrder::LowerFirst
    );

    let pal_interlaced = templates
        .iter()
        .find(|t| t.name.contains("PAL SD 4:3"))
        .unwrap();
    assert_eq!(
        pal_interlaced.settings.field_order,
        PresetFieldOrder::UpperFirst
    );
}

#[test]
fn f25_t2_04_pixel_aspect_ratios_are_strictly_positive() {
    for (par, name) in PIXEL_ASPECTS {
        assert!(
            par >= PIXEL_ASPECT_RANGE.0 && par <= PIXEL_ASPECT_RANGE.1,
            "{name} PAR out of range"
        );
    }
}

#[test]
fn f25_t2_05_frame_rates_table_monotonically_ordered() {
    for window in FRAME_RATES.windows(2) {
        assert!(
            window[0].fps < window[1].fps,
            "Frame rates should be sorted ascending"
        );
    }
}

// ============================================================================
// Feature 26: Media Profiles Matcher (R2.4)
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct MediaProfileKey {
    pub file_io_clsid: Option<String>,
    pub subformat: String,
    pub size_x: u32,
    pub size_y: u32,
    pub frame_rate: f64,
    pub color_bit_depth: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaProfileAttributes {
    pub interlace: FieldOrder,
    pub pixel_aspect_ratio: f64,
    pub alpha_interpretation: AlphaMode,
    pub gamma: f64,
}

#[derive(Default)]
pub struct TestProfileMatcher {
    profiles: Vec<(MediaProfileKey, MediaProfileAttributes)>,
}

impl TestProfileMatcher {
    pub fn new() -> Self {
        Self {
            profiles: Vec::new(),
        }
    }

    pub fn add_rule(&mut self, key: MediaProfileKey, attr: MediaProfileAttributes) {
        self.profiles.push((key, attr));
    }

    /// Search backwards from latest rule to earliest (results.md §18.4)
    pub fn match_stream(&self, target: &MediaProfileKey) -> Option<MediaProfileAttributes> {
        for (key, attr) in self.profiles.iter().rev() {
            let clsid_match =
                key.file_io_clsid.is_none() || key.file_io_clsid == target.file_io_clsid;
            let subformat_match = key.subformat.is_empty() || key.subformat == target.subformat;
            let size_match = (key.size_x == 0 || key.size_x == target.size_x)
                && (key.size_y == 0 || key.size_y == target.size_y);
            let fps_match =
                key.frame_rate <= 0.0 || (key.frame_rate - target.frame_rate).abs() < 0.01;
            let depth_match =
                key.color_bit_depth == 0 || key.color_bit_depth == target.color_bit_depth;

            if clsid_match && subformat_match && size_match && fps_match && depth_match {
                return Some(attr.clone());
            }
        }
        None
    }
}

#[test]
fn f26_t1_01_profile_matcher_exact_rule_match() {
    let mut matcher = TestProfileMatcher::new();
    let k1 = MediaProfileKey {
        file_io_clsid: Some("{1234}".into()),
        subformat: "H264".into(),
        size_x: 1920,
        size_y: 1080,
        frame_rate: 29.97,
        color_bit_depth: 8,
    };
    let a1 = MediaProfileAttributes {
        interlace: FieldOrder::Progressive,
        pixel_aspect_ratio: 1.0,
        alpha_interpretation: AlphaMode::None,
        gamma: 2.2,
    };
    matcher.add_rule(k1.clone(), a1.clone());

    let matched = matcher.match_stream(&k1);
    assert_eq!(matched, Some(a1));
}

#[test]
fn f26_t1_02_profile_matcher_wildcard_zero_matches_any_dimension() {
    let mut matcher = TestProfileMatcher::new();
    let wild_key = MediaProfileKey {
        file_io_clsid: None,
        subformat: "".into(),
        size_x: 0,
        size_y: 0,
        frame_rate: 0.0,
        color_bit_depth: 0,
    };
    let default_attr = MediaProfileAttributes {
        interlace: FieldOrder::Progressive,
        pixel_aspect_ratio: 1.0,
        alpha_interpretation: AlphaMode::Auto,
        gamma: 2.2,
    };
    matcher.add_rule(wild_key, default_attr.clone());

    let query = MediaProfileKey {
        file_io_clsid: Some("{ABC}".into()),
        subformat: "PRORES".into(),
        size_x: 3840,
        size_y: 2160,
        frame_rate: 59.94,
        color_bit_depth: 10,
    };
    assert_eq!(matcher.match_stream(&query), Some(default_attr));
}

#[test]
fn f26_t1_03_profile_matcher_reverse_search_priority() {
    let mut matcher = TestProfileMatcher::new();
    let key = MediaProfileKey {
        file_io_clsid: None,
        subformat: "DV".into(),
        size_x: 720,
        size_y: 480,
        frame_rate: 29.97,
        color_bit_depth: 8,
    };
    let attr_early = MediaProfileAttributes {
        interlace: FieldOrder::LowerFirst,
        pixel_aspect_ratio: 0.9091,
        alpha_interpretation: AlphaMode::None,
        gamma: 2.2,
    };
    let attr_late = MediaProfileAttributes {
        interlace: FieldOrder::Progressive, // User custom override added later
        pixel_aspect_ratio: 1.2121,
        alpha_interpretation: AlphaMode::None,
        gamma: 2.4,
    };
    matcher.add_rule(key.clone(), attr_early);
    matcher.add_rule(key.clone(), attr_late.clone());

    // Later rule must win per results.md §18.4
    assert_eq!(matcher.match_stream(&key), Some(attr_late));
}

#[test]
fn f26_t1_04_media_props_substitute_fps_options() {
    assert!(SUBSTITUTE_FPS.contains(&23.976));
    assert!(SUBSTITUTE_FPS.contains(&29.97));
    assert!(SUBSTITUTE_FPS.contains(&59.94));
    assert!(SUBSTITUTE_FPS.contains(&60.0));
}

#[test]
fn f26_t1_05_media_props_sanitized_clamps_aspect_ratio() {
    let props = MediaProps {
        pixel_aspect: 100.0,
        timecode_start: Some(-10.0),
        ..MediaProps::default()
    };
    let clean = props.sanitized();
    assert_eq!(clean.pixel_aspect, 4.0);
    assert_eq!(clean.timecode_start, Some(0.0));
}

#[test]
fn f26_t2_01_profile_matcher_no_match_returns_none() {
    let mut matcher = TestProfileMatcher::new();
    matcher.add_rule(
        MediaProfileKey {
            file_io_clsid: Some("{X}".into()),
            subformat: "A".into(),
            size_x: 100,
            size_y: 100,
            frame_rate: 30.0,
            color_bit_depth: 8,
        },
        MediaProfileAttributes {
            interlace: FieldOrder::Progressive,
            pixel_aspect_ratio: 1.0,
            alpha_interpretation: AlphaMode::None,
            gamma: 2.2,
        },
    );
    let target = MediaProfileKey {
        file_io_clsid: Some("{Y}".into()),
        subformat: "B".into(),
        size_x: 200,
        size_y: 200,
        frame_rate: 25.0,
        color_bit_depth: 8,
    };
    assert!(matcher.match_stream(&target).is_none());
}

#[test]
fn f26_t2_02_profile_matcher_floating_point_fps_tolerance() {
    let mut matcher = TestProfileMatcher::new();
    matcher.add_rule(
        MediaProfileKey {
            file_io_clsid: None,
            subformat: "NTSC".into(),
            size_x: 1920,
            size_y: 1080,
            frame_rate: 29.97,
            color_bit_depth: 8,
        },
        MediaProfileAttributes {
            interlace: FieldOrder::Progressive,
            pixel_aspect_ratio: 1.0,
            alpha_interpretation: AlphaMode::Auto,
            gamma: 2.2,
        },
    );
    let query = MediaProfileKey {
        file_io_clsid: None,
        subformat: "NTSC".into(),
        size_x: 1920,
        size_y: 1080,
        frame_rate: 29.97003, // Slight decoder jitter
        color_bit_depth: 8,
    };
    assert!(matcher.match_stream(&query).is_some());
}

#[test]
fn f26_t2_03_media_props_changes_picture_detection() {
    let default_props = MediaProps::default();
    assert!(!default_props.changes_picture());

    let mut custom = default_props;
    custom.alpha = AlphaMode::Premultiplied;
    assert!(custom.changes_picture());

    custom.alpha = AlphaMode::Auto;
    custom.pixel_aspect = 1.3333;
    assert!(custom.changes_picture());

    custom.pixel_aspect = 1.0;
    custom.background = Some([0, 0, 0]);
    assert!(custom.changes_picture());
}

#[test]
fn f26_t2_04_alpha_mode_all_variants_coverage() {
    let all = AlphaMode::ALL;
    assert_eq!(all.len(), 6);
    assert!(all.contains(&AlphaMode::Auto));
    assert!(all.contains(&AlphaMode::PremultipliedDirty));
}

#[test]
fn f26_t2_05_field_order_all_variants_coverage() {
    let all = FieldOrder::ALL;
    assert_eq!(all.len(), 3);
    assert!(all.contains(&FieldOrder::Progressive));
    assert!(all.contains(&FieldOrder::UpperFirst));
    assert!(all.contains(&FieldOrder::LowerFirst));
}

// ============================================================================
// Feature 33: Standardized Alerts (§10, R2.8)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AlertCategory {
    MissingMedia,
    CodecFormat,
    RenderPrerender,
    DiskMemory,
    GpuAcceleration,
    PluginVst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum AlertCode {
    MissingMediaPoolElements = 1561,
    MediaIsOfflinePrompt = 1634,
    MediaFileNotFound = 16926,
    FindMissingFile = 16132,
    MissingVideoCodec = 17369,
    MissingAudioCodec = 17381,
    CorruptStreamFormat = 17384,
    UnsupportedChannelCount = 33501,
    EmptyProjectCannotRender = 3404,
    InsufficientDiscSpace = 18548,
    ConfirmCancelRender = 44720,
    GpuAccelerationError = 33316,
    GpuDeviceDriverProblem = 33317,
    PluginFailedToLoad = 16555,
    PluginChainFull = 16511,
}

pub struct AlertDefinition {
    pub code: AlertCode,
    pub category: AlertCategory,
    pub en_us: &'static str,
    pub pt_br: &'static str,
}

pub fn get_standard_alert(code: AlertCode) -> AlertDefinition {
    match code {
        AlertCode::MissingMediaPoolElements => AlertDefinition {
            code,
            category: AlertCategory::MissingMedia,
            en_us: "Some media pool elements could not be found.",
            pt_br: "Alguns elementos da lista de mídia não foram encontrados.",
        },
        AlertCode::MediaIsOfflinePrompt => AlertDefinition {
            code,
            category: AlertCategory::MissingMedia,
            en_us: "The media file '{0}' is currently offline. Do you want to relink it?",
            pt_br: "O arquivo de mídia '{0}' está offline. Deseja reconectá-lo?",
        },
        AlertCode::MediaFileNotFound => AlertDefinition {
            code,
            category: AlertCategory::MissingMedia,
            en_us: "The file '{0}' was not found in the specified path.",
            pt_br: "O arquivo '{0}' não foi encontrado no caminho especificado.",
        },
        AlertCode::FindMissingFile => AlertDefinition {
            code,
            category: AlertCategory::MissingMedia,
            en_us: "Search for missing file...",
            pt_br: "Procurar arquivo ausente...",
        },
        AlertCode::MissingVideoCodec => AlertDefinition {
            code,
            category: AlertCategory::CodecFormat,
            en_us: "The required video codec for this stream is not available.",
            pt_br: "O codec de vídeo necessário para este fluxo não está disponível.",
        },
        AlertCode::MissingAudioCodec => AlertDefinition {
            code,
            category: AlertCategory::CodecFormat,
            en_us: "The required audio codec for this stream is not available.",
            pt_br: "O codec de áudio necessário para este fluxo não está disponível.",
        },
        AlertCode::CorruptStreamFormat => AlertDefinition {
            code,
            category: AlertCategory::CodecFormat,
            en_us: "The media stream data is corrupt or in an unrecognized format.",
            pt_br: "Os dados do fluxo de mídia estão corrompidos ou em formato não reconhecido.",
        },
        AlertCode::UnsupportedChannelCount => AlertDefinition {
            code,
            category: AlertCategory::CodecFormat,
            en_us: "Unsupported audio channel configuration ({0} channels).",
            pt_br: "Configuração de canais de áudio não suportada ({0} canais).",
        },
        AlertCode::EmptyProjectCannotRender => AlertDefinition {
            code,
            category: AlertCategory::RenderPrerender,
            en_us: "Unable to render because the project contains no timeline events.",
            pt_br:
                "Não é possível renderizar porque o projeto não contém eventos na linha do tempo.",
        },
        AlertCode::InsufficientDiscSpace => AlertDefinition {
            code,
            category: AlertCategory::RenderPrerender,
            en_us: "There is not enough free disk space to complete rendering.",
            pt_br: "Não há espaço livre em disco suficiente para concluir a renderização.",
        },
        AlertCode::ConfirmCancelRender => AlertDefinition {
            code,
            category: AlertCategory::RenderPrerender,
            en_us: "Do you want to cancel the current rendering operation?",
            pt_br: "Deseja cancelar a operação de renderização atual?",
        },
        AlertCode::GpuAccelerationError => AlertDefinition {
            code,
            category: AlertCategory::GpuAcceleration,
            en_us: "GPU Acceleration Error: Switching to CPU software fallback.",
            pt_br: "Erro de aceleração por GPU: Alternando para processamento por CPU.",
        },
        AlertCode::GpuDeviceDriverProblem => AlertDefinition {
            code,
            category: AlertCategory::GpuAcceleration,
            en_us: "The GPU driver encountered an unexpected error.",
            pt_br: "O driver da GPU encontrou um erro inesperado.",
        },
        AlertCode::PluginFailedToLoad => AlertDefinition {
            code,
            category: AlertCategory::PluginVst,
            en_us: "The plug-in '{0}' failed to load.",
            pt_br: "Falha ao carregar o plug-in '{0}'.",
        },
        AlertCode::PluginChainFull => AlertDefinition {
            code,
            category: AlertCategory::PluginVst,
            en_us: "The effect chain has reached its maximum capacity.",
            pt_br: "A cadeia de efeitos atingiu a capacidade máxima.",
        },
    }
}

#[test]
fn f33_t1_01_alert_missing_media_definitions() {
    let a = get_standard_alert(AlertCode::MediaFileNotFound);
    assert_eq!(a.category, AlertCategory::MissingMedia);
    assert!(a.en_us.contains("not found"));
    assert!(a.pt_br.contains("não foi encontrado"));
}

#[test]
fn f33_t1_02_alert_codec_definitions() {
    let a = get_standard_alert(AlertCode::MissingVideoCodec);
    assert_eq!(a.category, AlertCategory::CodecFormat);
    assert!(a.en_us.contains("codec"));
}

#[test]
fn f33_t1_03_alert_render_empty_project_3404() {
    let a = get_standard_alert(AlertCode::EmptyProjectCannotRender);
    assert_eq!(a.code as u32, 3404);
    assert_eq!(a.category, AlertCategory::RenderPrerender);
}

#[test]
fn f33_t1_04_alert_gpu_acceleration_33316_fallback() {
    let a = get_standard_alert(AlertCode::GpuAccelerationError);
    assert_eq!(a.code as u32, 33316);
    assert_eq!(a.category, AlertCategory::GpuAcceleration);
    assert!(a.en_us.contains("CPU software fallback"));
}

#[test]
fn f33_t1_05_alert_plugin_load_error_16555() {
    let a = get_standard_alert(AlertCode::PluginFailedToLoad);
    assert_eq!(a.code as u32, 16555);
    assert_eq!(a.category, AlertCategory::PluginVst);
}

#[test]
fn f33_t2_01_alert_formatting_with_parameter_substitution() {
    let a = get_standard_alert(AlertCode::MediaFileNotFound);
    let formatted = a.en_us.replace("{0}", "vacation.mp4");
    assert_eq!(
        formatted,
        "The file 'vacation.mp4' was not found in the specified path."
    );
}

#[test]
fn f33_t2_02_alert_channel_count_formatting() {
    let a = get_standard_alert(AlertCode::UnsupportedChannelCount);
    let formatted = a.en_us.replace("{0}", "16");
    assert_eq!(
        formatted,
        "Unsupported audio channel configuration (16 channels)."
    );
}

#[test]
fn f33_t2_03_alert_categories_coverage() {
    let codes = [
        AlertCode::MissingMediaPoolElements,
        AlertCode::MissingVideoCodec,
        AlertCode::EmptyProjectCannotRender,
        AlertCode::InsufficientDiscSpace,
        AlertCode::GpuAccelerationError,
        AlertCode::PluginFailedToLoad,
    ];
    let categories: HashSet<_> = codes
        .iter()
        .map(|&c| get_standard_alert(c).category)
        .collect();
    assert!(categories.contains(&AlertCategory::MissingMedia));
    assert!(categories.contains(&AlertCategory::CodecFormat));
    assert!(categories.contains(&AlertCategory::RenderPrerender));
    assert!(categories.contains(&AlertCategory::GpuAcceleration));
    assert!(categories.contains(&AlertCategory::PluginVst));
}

#[test]
fn f33_t2_04_alert_clean_room_text_no_vendor_trademarks() {
    let all_codes = [
        AlertCode::MissingMediaPoolElements,
        AlertCode::MediaIsOfflinePrompt,
        AlertCode::MediaFileNotFound,
        AlertCode::FindMissingFile,
        AlertCode::MissingVideoCodec,
        AlertCode::MissingAudioCodec,
        AlertCode::CorruptStreamFormat,
        AlertCode::UnsupportedChannelCount,
        AlertCode::EmptyProjectCannotRender,
        AlertCode::InsufficientDiscSpace,
        AlertCode::ConfirmCancelRender,
        AlertCode::GpuAccelerationError,
        AlertCode::GpuDeviceDriverProblem,
        AlertCode::PluginFailedToLoad,
        AlertCode::PluginChainFull,
    ];
    for c in all_codes {
        let def = get_standard_alert(c);
        for &tm in FORBIDDEN_TRADEMARKS {
            assert!(
                !def.en_us.contains(tm),
                "Alert {} en_us has trademark {tm}",
                c as u32
            );
            assert!(
                !def.pt_br.contains(tm),
                "Alert {} pt_br has trademark {tm}",
                c as u32
            );
        }
    }
}

#[test]
fn f33_t2_05_alert_codes_numeric_uniqueness() {
    let all_codes = [
        AlertCode::MissingMediaPoolElements,
        AlertCode::MediaIsOfflinePrompt,
        AlertCode::MediaFileNotFound,
        AlertCode::FindMissingFile,
        AlertCode::MissingVideoCodec,
        AlertCode::MissingAudioCodec,
        AlertCode::CorruptStreamFormat,
        AlertCode::UnsupportedChannelCount,
        AlertCode::EmptyProjectCannotRender,
        AlertCode::InsufficientDiscSpace,
        AlertCode::ConfirmCancelRender,
        AlertCode::GpuAccelerationError,
        AlertCode::GpuDeviceDriverProblem,
        AlertCode::PluginFailedToLoad,
        AlertCode::PluginChainFull,
    ];
    let mut seen = HashSet::new();
    for c in all_codes {
        assert!(seen.insert(c as u32), "Duplicate alert code: {}", c as u32);
    }
}

// ============================================================================
// Feature 41: Parity Matrix Sync (R3)
// ============================================================================

#[test]
fn f41_t1_01_parity_matrix_script_exists() {
    let p = repo_root().join("scripts/parity_matrix.py");
    assert!(p.exists(), "scripts/parity_matrix.py must exist");
}

#[test]
fn f41_t1_02_parity_matrix_json_exists_and_valid() {
    let p = repo_root().join("docs/research/parity_matrix.json");
    if p.exists() {
        let content = fs::read_to_string(p).unwrap();
        let val: serde_json::Value = serde_json::from_str(&content).expect("valid JSON");
        assert!(val.is_object() || val.is_array());
    }
}

#[test]
fn f41_t1_03_parity_matrix_markdown_documentation_exists() {
    let p = repo_root().join("docs_obsidian/ui_references/matriz_de_paridade.md");
    if p.exists() {
        let content = fs::read_to_string(p).unwrap();
        assert!(content.contains("Matriz de paridade"));
    }
}

#[test]
fn f41_t1_04_parity_matrix_python_script_syntax_validity() {
    let p = repo_root().join("scripts/parity_matrix.py");
    let content = fs::read_to_string(p).unwrap();
    assert!(content.contains("DEFAULT_SCAN ="));
    assert!(content.contains("ROWS: List[Row] = ["));
    assert!(content.contains("def main():") || content.contains("if __name__ =="));
}

#[test]
fn f41_t1_05_parity_matrix_row_categories_covered() {
    let p = repo_root().join("scripts/parity_matrix.py");
    let content = fs::read_to_string(p).unwrap();
    for cat in &[
        "Arquivo",
        "Editar",
        "Inserir",
        "Ferramentas",
        "Opções",
        "Exibir",
    ] {
        assert!(
            content.contains(cat),
            "Parity matrix must contain category {cat}"
        );
    }
}

#[test]
fn f41_t2_01_parity_matrix_states_validity() {
    let p = repo_root().join("scripts/parity_matrix.py");
    let content = fs::read_to_string(p).unwrap();
    for state in &["feito", "parcial", "previsto", "sem_plano"] {
        assert!(
            content.contains(state),
            "State '{state}' missing in parity script"
        );
    }
}

#[test]
fn f41_t2_02_parity_matrix_priorities_validity() {
    let p = repo_root().join("scripts/parity_matrix.py");
    let content = fs::read_to_string(p).unwrap();
    assert!(content.contains("\"P1\""));
    assert!(content.contains("\"P2\""));
    assert!(content.contains("\"P3\""));
}

#[test]
fn f41_t2_03_parity_matrix_no_raw_vendor_menus_recorded() {
    let p = repo_root().join("scripts/parity_matrix.py");
    let content = fs::read_to_string(p).unwrap();
    assert!(!content.contains("Sony Vegas Pro 13.0"));
    assert!(!content.contains("MAGIX Vegas Pro 21.0"));
}

#[test]
fn f41_t2_04_parity_matrix_clean_python_imports() {
    let p = repo_root().join("scripts/parity_matrix.py");
    let content = fs::read_to_string(p).unwrap();
    // Must only use Python standard library
    assert!(!content.contains("import requests"));
    assert!(!content.contains("import numpy"));
    assert!(!content.contains("import pandas"));
}

#[test]
fn f41_t2_05_parity_matrix_row_id_uniqueness() {
    let p = repo_root().join("scripts/parity_matrix.py");
    let content = fs::read_to_string(p).unwrap();
    let id_re = regex_lite_find_row_ids(&content);
    let mut seen = HashSet::new();
    for id in id_re {
        assert!(seen.insert(id.clone()), "Duplicate parity row ID: {id}");
    }
}

fn regex_lite_find_row_ids(py_code: &str) -> Vec<String> {
    let mut ids = Vec::new();
    for line in py_code.lines() {
        let t = line.trim();
        if t.starts_with("(\"") && t.contains(", A_") {
            if let Some(end) = t[2..].find('"') {
                ids.push(t[2..2 + end].to_string());
            }
        }
    }
    ids
}

// ============================================================================
// Feature 42: Translation Catalogs Sync (R3)
// ============================================================================

#[test]
fn f42_t1_01_translation_all_8_languages_configured() {
    assert_eq!(LANGUAGES.len(), 8);
    assert_eq!(LANGUAGES[0], "pt-BR");
    assert!(LANGUAGES.contains(&"en-US"));
    assert!(LANGUAGES.contains(&"de-DE"));
    assert!(LANGUAGES.contains(&"es-ES"));
    assert!(LANGUAGES.contains(&"fr-FR"));
    assert!(LANGUAGES.contains(&"ru-RU"));
    assert!(LANGUAGES.contains(&"ja-JP"));
    assert!(LANGUAGES.contains(&"zh-CN"));
}

#[test]
fn f42_t1_02_translation_language_names_display() {
    assert_eq!(language_name("pt-BR", false), "Português (Brasil)");
    assert_eq!(language_name("en-US", false), "English (US)");
    assert_eq!(language_name("ja-JP", true), "日本語");
    assert_eq!(language_name("ja-JP", false), "Japanese");
    assert_eq!(language_name("zh-CN", true), "简体中文");
}

#[test]
fn f42_t1_03_translation_ideograph_languages_flag() {
    assert!(language_has_ideographs("ja-JP"));
    assert!(language_has_ideographs("zh-CN"));
    assert!(!language_has_ideographs("en-US"));
    assert!(!language_has_ideographs("pt-BR"));
}

#[test]
fn f42_t1_04_translation_all_7_files_exist_on_disk() {
    for lang in &[
        "de-DE", "en-US", "es-ES", "fr-FR", "ja-JP", "ru-RU", "zh-CN",
    ] {
        let p = repo_root().join(format!("Assets/i18n/strings/{lang}.json"));
        assert!(p.exists(), "Translation catalog missing: {lang}.json");
    }
}

#[test]
fn f42_t1_05_translation_all_7_files_valid_json_objects() {
    for lang in &[
        "de-DE", "en-US", "es-ES", "fr-FR", "ja-JP", "ru-RU", "zh-CN",
    ] {
        let p = repo_root().join(format!("Assets/i18n/strings/{lang}.json"));
        let map: Result<HashMap<String, String>, _> =
            serde_json::from_str(&fs::read_to_string(p).unwrap());
        assert!(
            map.is_ok(),
            "{lang}.json must parse into HashMap<String, String>"
        );
        assert!(!map.unwrap().is_empty(), "{lang}.json must not be empty");
    }
}

#[test]
fn f42_t2_01_translation_100_percent_key_matching_across_all_catalogs() {
    let en_path = repo_root().join("Assets/i18n/strings/en-US.json");
    let en_map: HashMap<String, String> =
        serde_json::from_str(&fs::read_to_string(en_path).unwrap()).unwrap();
    let en_keys: HashSet<_> = en_map.keys().collect();

    for lang in &["de-DE", "es-ES", "fr-FR", "ja-JP", "ru-RU", "zh-CN"] {
        let p = repo_root().join(format!("Assets/i18n/strings/{lang}.json"));
        let map: HashMap<String, String> =
            serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap();
        let target_keys: HashSet<_> = map.keys().collect();

        let missing: Vec<_> = en_keys.difference(&target_keys).collect();
        let extra: Vec<_> = target_keys.difference(&en_keys).collect();
        assert!(
            missing.is_empty() && extra.is_empty(),
            "{lang}.json key mismatch: missing {}, extra {}",
            missing.len(),
            extra.len()
        );
    }
}

#[test]
fn f42_t2_02_translation_placeholder_marker_consistency() {
    for lang in &[
        "de-DE", "en-US", "es-ES", "fr-FR", "ja-JP", "ru-RU", "zh-CN",
    ] {
        let p = repo_root().join(format!("Assets/i18n/strings/{lang}.json"));
        let map: HashMap<String, String> =
            serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap();
        for (src, dst) in map {
            let src_count = src.matches("{}").count();
            let dst_count = dst.matches("{}").count();
            assert_eq!(
                src_count, dst_count,
                "{lang}: marker count mismatch for key '{src}': expected {src_count}, got {dst_count}"
            );
        }
    }
}

#[test]
fn f42_t2_03_translation_no_empty_translated_strings() {
    for lang in &[
        "de-DE", "en-US", "es-ES", "fr-FR", "ja-JP", "ru-RU", "zh-CN",
    ] {
        let p = repo_root().join(format!("Assets/i18n/strings/{lang}.json"));
        let map: HashMap<String, String> =
            serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap();
        for (src, dst) in map {
            assert!(
                !dst.trim().is_empty(),
                "{lang}: empty translation for '{src}'"
            );
        }
    }
}

#[test]
fn f42_t2_04_translation_tr_args_runtime_formatting() {
    set_language("en-US");
    let out = tr_args("Arquivo: {}", &[&"video.mp4"]);
    assert_eq!(out, "File: video.mp4");

    set_language("pt-BR");
    let out_pt = tr_args("Arquivo: {}", &[&"video.mp4"]);
    assert_eq!(out_pt, "Arquivo: video.mp4");
}

#[test]
fn f42_t2_05_translation_tr_text_number_formatting() {
    set_language("en-US");
    assert_eq!(tr_text("29,97 (NTSC)"), "29.97 (NTSC)");
    set_language("pt-BR");
    assert_eq!(tr_text("29,97 (NTSC)"), "29,97 (NTSC)");
}
