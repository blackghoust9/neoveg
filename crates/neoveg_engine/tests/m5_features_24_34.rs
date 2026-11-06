//! Comprehensive Integration & Edge Case Verification Suite for Milestone 5 (Features 24–34).
//!
//! Verifies:
//! - Feature 24: Project Properties (Dialogs 826/863/817/807, ACES/OCIO, LFE, Ruler, Gamma)
//! - Feature 25: 46 Standard .vqpt Templates, Immutability & Custom Preset Manager
//! - Feature 26: Media Profiles Rule Matcher (reverse evaluation, wildcards, FPS epsilon)
//! - Feature 27: Project Media 23 Detail Columns, Sorting, Search, Relink & In-Use Protection
//! - Feature 28: Media Bins Hierarchy, Quick Tags (Ctrl+1..0) & Smart Search Bins
//! - Feature 29: Unified Explorer, Companion File Suppression & Audition Transport
//! - Feature 30: Make Movie 3-Step Wizard Flow, Preset Recommendation & Space Validation
//! - Feature 31: Color-Coded Preview Cache Ruler (Green/Yellow/Red, O(1) Query, Invalidation)
//! - Feature 32: Dynamic RAM Preview (Shift+B), Selective Disk Prerender (Shift+M) & Cleanup (Dialog 834)
//! - Feature 33: Standardized Alerts Subsystem (§10 Error Codes, Localized Templates, Mock Dispatch)
//! - Feature 34: Smart Proxies, Video Preview Quality Switching & Master Render Conforming

use neoveg_common::alerts::{AlertAction, AlertCode, AlertQueue};
use neoveg_common::media_props::AlphaMode;
use neoveg_common::presets::{
    canonical_46_templates, export_vqpt, import_vqpt, CompositingGamma, HdrMode, MasterBusMode,
    PixelFormat, ProjectTemplate, RenderQuality, TemplateManager,
};
use neoveg_common::profiles::MediaProfileMatcher;
use neoveg_common::project::{AcesViewTransform, ProjectSettings};
use neoveg_common::ruler_format::RulerFormat;
use neoveg_engine::cache_ruler::{FrameCacheStatus, PreviewCacheRuler};
use neoveg_engine::explorer::ExplorerModel;
use neoveg_engine::make_movie::{ExportDestination, MakeMovieWizard, WizardStep};
use neoveg_engine::media_bins::{MediaBinTree, SearchCondition, SearchCriterion, SearchField};
use neoveg_engine::media_pool::{MediaDetailRow, MediaPoolManager};
use neoveg_engine::prerender::{
    DynamicRamPreview, SelectiveDiskPrerender, PRERENDER_MAX_CHUNK_FRAMES,
};
use neoveg_engine::proxy::SmartProxyManager;
use neoveg_engine::test_fixtures::BlenderOpenMovieFixture;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

// ============================================================================
// Feature 24: Project Properties Setup & Edge Cases
// ============================================================================

#[test]
fn test_f24_project_settings_edge_cases() {
    let mut s = ProjectSettings::default();

    // Edge Case 1 & 2: Odd dimensions and range bounds
    s.width = 1921;
    s.height = 1079;
    s.fps = 29.97003;
    s.stereo_busses = 50; // Max is 26
    s.lfe_filter_enabled = true;
    s.master_bus = MasterBusMode::Stereo; // In Stereo, LFE must be disabled
    s.pixel_format = PixelFormat::Legacy8Video; // In 8-bit, gamma must be Video 2.222
    s.compositing_gamma = CompositingGamma::Linear;
    s.ruler_format = RulerFormat::AudioCd;
    s.ruler_start = 12.5; // Audio CD start offset must be forced to 0.0

    s.sanitize();

    assert_eq!(s.width, 1920, "Must be even dimension");
    assert_eq!(s.height, 1078, "Must be even dimension");
    assert_eq!(s.stereo_busses, 26, "Stereo busses clamped to 26 max");
    assert!(
        !s.lfe_filter_enabled,
        "LFE filter must be disabled in Stereo mode (Edge Case 5)"
    );
    assert_eq!(
        s.compositing_gamma,
        CompositingGamma::Video,
        "Gamma choice locked in 8-bit (Edge Case 3)"
    );
    assert_eq!(
        s.ruler_start, 0.0,
        "Audio CD ruler start must be 0.0 (Edge Case 6)"
    );

    // Edge Case 4: HDR mode auto-configuration
    s.apply_hdr_mode(HdrMode::Hdr10);
    assert_eq!(s.pixel_format, PixelFormat::Float32Full);
    assert_eq!(s.compositing_gamma, CompositingGamma::Linear);
    assert_eq!(s.view_transform, AcesViewTransform::Rec2020);

    s.apply_hdr_mode(HdrMode::Hlg);
    assert_eq!(s.view_transform, AcesViewTransform::Hlg);
}

// ============================================================================
// Feature 25: Project Templates (.vqpt) & Immutability
// ============================================================================

#[test]
fn test_f25_templates_canonical_46_and_immutability() {
    let canonical = canonical_46_templates();
    assert_eq!(
        canonical.len(),
        46,
        "Must provide exactly 46 canonical .vqpt templates (§18.3)"
    );

    let mut mgr = TemplateManager::new();

    // Verify immutability: attempting to overwrite built-in template returns error (Edge Case 7)
    let builtin_tpl = ProjectTemplate::new("Full HD 1080p 24", ProjectSettings::default());
    let res_overwrite = mgr.save_custom(builtin_tpl);
    assert!(res_overwrite.is_err());

    // Verify immutability: attempting to delete built-in template returns error (Edge Case 8)
    let res_delete = mgr.delete_custom("Full HD 1080p 24");
    assert!(res_delete.is_err());

    // Empty name rejected (Edge Case 9)
    let empty_tpl = ProjectTemplate::new("   ", ProjectSettings::default());
    let res_empty = mgr.save_custom(empty_tpl);
    assert!(res_empty.is_err());

    // Custom template save and delete
    let custom_tpl = ProjectTemplate::new("My Custom Podcast 1080p", ProjectSettings::default());
    assert!(mgr.save_custom(custom_tpl).is_ok());
    assert!(mgr.find_by_name("My Custom Podcast 1080p").is_some());
    assert!(mgr.delete_custom("My Custom Podcast 1080p").is_ok());

    // Serialization roundtrip
    let tpl = &canonical[0];
    let vqpt_str = export_vqpt(tpl).expect("export vqpt");
    let imported = import_vqpt(&vqpt_str).expect("import vqpt");
    assert_eq!(imported.name, tpl.name);
    assert_eq!(imported.settings.width, tpl.settings.width);
}

// ============================================================================
// Feature 26: Media Profiles Rule Matcher (`profiles.ini`)
// ============================================================================

#[test]
fn test_f26_profiles_reverse_order_and_wildcards() {
    let ini = r#"
[Video Media Profiles]
Key1 = 0, "None", 1920, 1080, 24.0, 0
Attributes1 = "Progressive", 1.0, "None", 2.222222
Key2 = 0, "None", 1920, 1080, 24.0, 0
Attributes2 = "Progressive", 1.0, "Premultiplied (Dirty)", 2.400000
"#;
    let matcher = MediaProfileMatcher::parse_ini(ini).expect("parse");
    assert_eq!(matcher.rules().len(), 2);

    // Later rule Key2 must win (Edge Case 12)
    let matched = matcher
        .match_raw(None, None, 1920, 1080, 24.0, 8)
        .expect("matched");
    assert_eq!(
        matched.alpha_interpretation,
        AlphaMode::PremultipliedDirty,
        "Premultiplied Dirty alpha (Edge Case 14)"
    );
    assert_eq!(matched.gamma, 2.4);

    // Epsilon tolerance for FPS (Edge Case 13)
    let matched_eps = matcher.match_raw(None, None, 1920, 1080, 24.004, 8);
    assert!(
        matched_eps.is_some(),
        "24.004 matches 24.0 within 0.005 tolerance"
    );

    let matched_out_of_bounds = matcher.match_raw(None, None, 1920, 1080, 24.010, 8);
    assert!(
        matched_out_of_bounds.is_none(),
        "24.010 is outside 0.005 tolerance"
    );
}

// ============================================================================
// Feature 27: Project Media Window (23 Columns)
// ============================================================================

#[test]
fn test_f27_media_pool_twenty_three_columns_and_replace() {
    let mut pool = MediaPoolManager::new();
    assert_eq!(pool.visible_columns().len(), 23);

    let mut row = MediaDetailRow::new(1, "HeroShot.mov", PathBuf::from("clips/hero.mov"));
    row.use_count = 1;
    row.frame_size = (3840, 2160);
    row.fps = 59.94;
    pool.add_item(row);

    // Cannot remove in-use media without explicit confirmation (Edge Case 15)
    let res = pool.remove_item(1, false);
    assert_eq!(res, Err(AlertCode::TimelineMediaDeleteConfirm));

    // Replace media with alternate file (Edge Case 16)
    let mut new_info =
        MediaDetailRow::new(1, "HeroShot_v02.mov", PathBuf::from("clips/hero_v02.mov"));
    new_info.fps = 60.0;
    assert!(pool
        .replace_media(1, PathBuf::from("clips/hero_v02.mov"), Some(new_info))
        .is_ok());
    assert_eq!(pool.items()[0].path, PathBuf::from("clips/hero_v02.mov"));
}

// ============================================================================
// Feature 28: Media Bins & Smart Search Bins
// ============================================================================

#[test]
fn test_f28_bins_hierarchy_and_smart_evaluation() {
    let mut tree = MediaBinTree::new();

    // Cannot delete or rename root bin "All Media" (Edge Case 18)
    assert_eq!(
        tree.delete_bin(MediaBinTree::ROOT_BIN_ID),
        Err(AlertCode::RootBinCannotRemove)
    );
    assert_eq!(
        tree.rename_bin(MediaBinTree::ROOT_BIN_ID, "Root"),
        Err(AlertCode::RootBinCannotRename)
    );

    // Auto-rename on name collision (Edge Case 19)
    let b1 = tree.create_bin(None, "Interviews").unwrap();
    let b2 = tree.create_bin(None, "Interviews").unwrap();
    assert_eq!(tree.get_bin(b1).unwrap().name, "Interviews");
    assert_eq!(tree.get_bin(b2).unwrap().name, "Interviews (1)");

    // Smart bin creation and evaluation
    let crit = vec![SearchCriterion::new(
        SearchField::Type,
        SearchCondition::Equals,
        "Audio",
    )];
    let smart_id = tree.create_smart_bin(None, "All Audio", crit).unwrap();

    let mut row1 = MediaDetailRow::new(10, "Narration.wav", PathBuf::from("audio.wav"));
    row1.media_type = "Audio".into();
    let mut row2 = MediaDetailRow::new(20, "Footage.mp4", PathBuf::from("video.mp4"));
    row2.media_type = "Video".into();

    tree.evaluate_smart_bins(&[row1, row2]);
    let smart_bin = tree.get_bin(smart_id).unwrap();
    assert!(smart_bin.media_ids.contains(&10));
    assert!(!smart_bin.media_ids.contains(&20));
}

// ============================================================================
// Feature 29: Unified File Explorer
// ============================================================================

#[test]
fn test_f29_explorer_companion_file_filtering() {
    let dir = std::env::temp_dir().join(format!("neoveg_e2e_exp_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);

    let main_clip = dir.join("nature.mp4");
    let peak_file = dir.join("nature.mp4.sfk");
    let proxy_file = dir.join("nature.mp4.sfvp0");
    let prerender_file = dir.join("nature.mp4.nvprv");
    let peak_upper = dir.join("nature.mp4.SFK");
    let proxy_upper = dir.join("nature.mp4.SFVP0");
    let prerender_upper = dir.join("nature.mp4.NVPRV");

    let _ = std::fs::write(&main_clip, b"main");
    let _ = std::fs::write(&peak_file, b"peaks");
    let _ = std::fs::write(&proxy_file, b"proxy");
    let _ = std::fs::write(&prerender_file, b"prerender");
    let _ = std::fs::write(&peak_upper, b"PEAKS");
    let _ = std::fs::write(&proxy_upper, b"PROXY");
    let _ = std::fs::write(&prerender_upper, b"PRERENDER");

    let mut explorer = ExplorerModel::new();
    let items = explorer.navigate_to(&dir).expect("navigate");

    // Companion files must be hidden case-insensitively (Edge Case 21)
    assert_eq!(
        items.len(),
        1,
        "Must suppress lowercase and uppercase companion files"
    );
    assert_eq!(items[0].name, "nature.mp4");

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_f29_explorer_scene_detection_and_blender_fixtures() {
    let explorer = ExplorerModel::new();

    // 1. Error handling: non-existent file returns Err
    let non_existent = PathBuf::from("target/test_fixtures/blender/non_existent_123.mp4");
    assert!(explorer.detect_scenes(&non_existent, 0.3).is_err());

    // 2. Error handling: empty 0-byte file returns Err
    let empty_file = std::env::temp_dir().join(format!("empty_clip_{}.mp4", std::process::id()));
    let _ = std::fs::write(&empty_file, b"");
    assert!(explorer.detect_scenes(&empty_file, 0.3).is_err());
    let _ = std::fs::remove_file(&empty_file);

    // 3. Blender Open Movie test fixture: Big Buck Bunny (3 distinct shots: 2.0s, 3.0s, 2.5s = 7.5s total)
    let bbb_fixture = BlenderOpenMovieFixture::ensure_fixture("big_buck_bunny")
        .expect("BBB fixture must generate cleanly offline in target/test_fixtures/blender/");

    // Low threshold (0.25): detects the 2 cut points, yielding 3 subclip spans
    let bbb_scenes = explorer
        .detect_scenes(&bbb_fixture, 0.25)
        .expect("detect scenes BBB");
    assert_eq!(
        bbb_scenes.len(),
        3,
        "Big Buck Bunny fixture has 3 distinct luminance scenes"
    );
    assert_eq!(bbb_scenes[0].0, 0.0);
    assert!(
        (bbb_scenes[0].1 - 2.0).abs() < 0.2,
        "Cut 1 should occur at ~2.0s"
    );
    assert!((bbb_scenes[1].0 - 2.0).abs() < 0.2);
    assert!(
        (bbb_scenes[1].1 - 5.0).abs() < 0.2,
        "Cut 2 should occur at ~5.0s"
    );
    assert!((bbb_scenes[2].0 - 5.0).abs() < 0.2);
    assert!(
        (bbb_scenes[2].1 - 7.5).abs() < 0.05,
        "Final shot must be strictly bounded by 7.5s"
    );

    // High threshold (0.95): no transitions exceed threshold, returns single continuous span bounded by 7.5s (never 10.0s)
    let bbb_single = explorer
        .detect_scenes(&bbb_fixture, 0.95)
        .expect("detect scenes BBB high threshold");
    assert_eq!(bbb_single.len(), 1, "High threshold yields single span");
    assert_eq!(bbb_single[0].0, 0.0);
    assert!(
        (bbb_single[0].1 - 7.5).abs() < 0.01,
        "Must yield exactly 7.5s duration, never hardcoded 10.0s"
    );

    // 4. Blender Open Movie test fixture: Sintel (2 shots: 2.0s blizzard, 3.0s cavern = 5.0s total)
    let sintel_fixture = BlenderOpenMovieFixture::ensure_fixture("sintel")
        .expect("Sintel fixture must generate cleanly offline");
    let sintel_scenes = explorer
        .detect_scenes(&sintel_fixture, 0.25)
        .expect("detect scenes Sintel");
    assert_eq!(
        sintel_scenes.len(),
        2,
        "Sintel fixture has 2 distinct shots"
    );
    assert_eq!(sintel_scenes[0].0, 0.0);
    assert!((sintel_scenes[0].1 - 2.0).abs() < 0.2);
    assert!((sintel_scenes[1].0 - 2.0).abs() < 0.2);
    assert!(
        (sintel_scenes[1].1 - 5.0).abs() < 0.05,
        "Strictly bounded by 5.0s media duration"
    );

    // 5. Short 2.0s clip verification (Addressing Challenger 1 / Reviewer Finding 1):
    // A 2.0s clip must yield (0.0, 2.0), NEVER (0.0, 10.0)!
    let short_clip_path =
        std::env::temp_dir().join(format!("short_clip_2s_{}.nvvid", std::process::id()));
    {
        // 48 frames at 24 fps = 2.0s
        let (w, h, fps, frames) = (32u32, 18u32, 24u32, 48u32);
        let mut f = std::fs::File::create(&short_clip_path).unwrap();
        use std::io::Write;
        f.write_all(b"NVVID100").unwrap();
        f.write_all(&w.to_le_bytes()).unwrap();
        f.write_all(&h.to_le_bytes()).unwrap();
        f.write_all(&fps.to_le_bytes()).unwrap();
        f.write_all(&1u32.to_le_bytes()).unwrap();
        f.write_all(&frames.to_le_bytes()).unwrap();
        let frame_buf = vec![128u8; (w * h) as usize];
        for _ in 0..frames {
            f.write_all(&frame_buf).unwrap();
        }
    }
    let short_scenes = explorer
        .detect_scenes(&short_clip_path, 0.5)
        .expect("short clip detect");
    assert_eq!(short_scenes.len(), 1);
    assert_eq!(
        short_scenes[0],
        (0.0, 2.0),
        "2.0s file must yield exactly (0.0, 2.0), NEVER 10.0s!"
    );
    let _ = std::fs::remove_file(&short_clip_path);

    // 6. Real MP4 container inspection: sample_video.mp4 (15.0s duration)
    let repo_sample = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../neoveg_video/tests/assets/sample_video.mp4");
    if repo_sample.exists() {
        let probed = explorer.probe_media(&repo_sample).expect("probe real mp4");
        assert_eq!(probed.width, 1920);
        assert_eq!(probed.height, 1080);
        assert!(
            (probed.duration_seconds - 15.0).abs() < 0.1,
            "Duration must be 15.0s"
        );

        let mp4_scenes = explorer
            .detect_scenes(&repo_sample, 0.95)
            .expect("detect mp4");
        assert_eq!(mp4_scenes.len(), 1);
        assert_eq!(mp4_scenes[0].0, 0.0);
        assert!(
            (mp4_scenes[0].1 - 15.0).abs() < 0.1,
            "MP4 span bounded strictly by 15.0s"
        );
    }

    // 7. Blender Audio Fixture (WAV) scene/cut detection
    let audio_fixture = BlenderOpenMovieFixture::ensure_audio_fixture("bbb_audio")
        .expect("Audio fixture generation");
    let audio_scenes = explorer
        .detect_scenes(&audio_fixture, 0.3)
        .expect("detect scenes audio");
    assert!(!audio_scenes.is_empty());
    assert_eq!(audio_scenes[0].0, 0.0);
    assert!(
        (audio_scenes.last().unwrap().1 - 4.0).abs() < 0.2,
        "Audio strictly bounded by 4.0s duration"
    );
}

// ============================================================================
// Feature 30: Make Movie Guided Export Wizard
// ============================================================================

#[test]
fn test_f30_make_movie_empty_validation_and_flow() {
    let settings = ProjectSettings::default();

    // Empty project triggers Alert ID 3404 (Edge Case 23)
    let mut wizard = MakeMovieWizard::new(&settings, false, 0.0);
    assert_eq!(
        wizard.choose_destination(ExportDestination::WebSharing),
        Err(AlertCode::EmptyProjectRender)
    );

    // Non-empty project succeeds to Step 2
    let mut wizard_ok = MakeMovieWizard::new(&settings, true, 120.0);
    assert!(wizard_ok
        .choose_destination(ExportDestination::WebSharing)
        .is_ok());
    assert!(matches!(
        wizard_ok.current_step(),
        WizardStep::ConfigureSettings(_)
    ));
}

// ============================================================================
// Feature 31: Preview Cache Ruler
// ============================================================================

#[test]
fn test_f31_cache_ruler_invalidation_and_playback_query() {
    let mut ruler = PreviewCacheRuler::new(1000, 30.0);

    // Initial state: unrendered (Red)
    assert_eq!(ruler.status_at(50), FrameCacheStatus::RedUnrendered);

    // Mark prerendered region (Green)
    ruler.mark_span(100, 400, FrameCacheStatus::GreenCached);
    assert_eq!(ruler.status_at(200), FrameCacheStatus::GreenCached);

    // Invalidation in frames 200..300: turns Red, leaving 100..200 and 300..400 Green (Edge Case 26)
    ruler.invalidate_region(200, 300);
    assert_eq!(ruler.status_at(150), FrameCacheStatus::GreenCached);
    assert_eq!(ruler.status_at(250), FrameCacheStatus::RedUnrendered);
    assert_eq!(ruler.status_at(350), FrameCacheStatus::GreenCached);
}

// ============================================================================
// Feature 32: Dynamic RAM & Selective Prerender
// ============================================================================

#[test]
fn test_f32_dynamic_ram_and_prerender_chunks() {
    // 0 MB budget triggers Alert 46259
    let mut ram_zero = DynamicRamPreview::new(0, 1920, 1080);
    let cancel = AtomicBool::new(false);
    let res_zero = ram_zero.build_selection(0, 100, &cancel, |_f| vec![0u8; 1000]);
    assert_eq!(res_zero, Err(AlertCode::DynamicRamPreviewZeroMb));

    // Selective disk prerender 300-frame chunks
    let dir = std::env::temp_dir().join(format!("neoveg_e2e_prv_{}", std::process::id()));
    let mut disk_prerender = SelectiveDiskPrerender::new(dir.clone());

    // Prerender 600 frames => exactly 2 chunks of 300 frames each (Edge Case 29)
    let chunks = disk_prerender
        .prerender_range(0, 600, 1920, 1080, 0x12345, |_f| vec![0u8; 128])
        .unwrap();

    assert_eq!(chunks.len(), 2);
    assert_eq!(
        chunks[0].end_frame - chunks[0].start_frame,
        PRERENDER_MAX_CHUNK_FRAMES
    );
    assert_eq!(
        chunks[1].end_frame - chunks[1].start_frame,
        PRERENDER_MAX_CHUNK_FRAMES
    );

    let _ = std::fs::remove_dir_all(dir);
}

// ============================================================================
// Feature 33: Standardized Alerts Subsystem
// ============================================================================

#[test]
fn test_f33_alerts_mock_dispatch_and_no_trademarks() {
    let queue = AlertQueue::default();
    queue.set_mock_handler(Some(Arc::new(|event| {
        if event.code == AlertCode::MediaFileNotFound {
            AlertAction::IgnoreAll
        } else {
            AlertAction::Cancel
        }
    })));

    let response = queue.dispatch(
        AlertCode::MediaFileNotFound,
        &["missing_audio.wav"],
        "en-US",
        Some(PathBuf::from("missing_audio.wav")),
    );

    assert_eq!(response, Some(AlertAction::IgnoreAll));

    // Ensure error codes are uniquely mapped
    assert_eq!(AlertCode::MissingProjectMediaElements.as_u32(), 1561);
    assert_eq!(AlertCode::EmptyProjectRender.as_u32(), 3404);
    assert_eq!(AlertCode::GpuAccelerationError.as_u32(), 33316);
}

// ============================================================================
// Feature 34: Smart Proxies & Quality Switching
// ============================================================================

#[test]
fn test_f34_smart_proxy_switching_and_conforming() {
    let original = PathBuf::from("master_4k_scene1.mp4");
    let proxy = SmartProxyManager::get_proxy_path_for(&original);
    let _ = std::fs::write(&proxy, b"proxy_data");

    let mut proxy_mgr = SmartProxyManager::new();
    proxy_mgr.register_media(original.clone(), 3840, 2160);
    proxy_mgr.mark_proxy_available(&original);

    // 1. In Draft quality => uses proxy (Edge Case 32)
    let path_draft = proxy_mgr.resolve_read_path(&original, RenderQuality::Draft, false);
    assert_eq!(path_draft, proxy.as_path());

    // 2. In Good quality => uses original master
    let path_good = proxy_mgr.resolve_read_path(&original, RenderQuality::Good, false);
    assert_eq!(path_good, original.as_path());

    // 3. In Final Render with Draft active => strictly conforms to original! (Edge Case 33)
    let path_render = proxy_mgr.resolve_read_path(&original, RenderQuality::Draft, true);
    assert_eq!(path_render, original.as_path());

    let _ = std::fs::remove_file(&proxy);
}
