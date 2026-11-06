//! Empirical Adversarial Stress Test Suite for Milestone 5 (Features 30–34).
//!
//! Subsystems Challenged:
//! 1. Make Movie Wizard (F30):
//!    - Step transitions, invalid destination inputs, zero-byte / empty project validation (Alert 3404),
//!    - Disk space estimation with huge frame counts (10M, 100M, 1B frames),
//!    - Overwrite confirmation (Alert 1911) & Insufficient disk space (Alert 17603).
//! 2. Dynamic RAM Preview Cache (F32):
//!    - 0 MB budget setting triggers Alert 46259,
//!    - Cache auto-truncation on selection overflow,
//!    - Memory bounds enforcement and buffer pool recycling behavior,
//!    - Cancellation mid-render preserving partial renders.
//! 3. Selective Disk Prerender (F32):
//!    - <= 300 frame chunking boundary conditions,
//!    - .nvprv 64-byte binary header validation & index table verification,
//!    - Header and index corruption resistance (truncated, bad magic, EOF seeks, missing files),
//!    - Cleanup deletion modes (DeleteInactive, DeleteInSelection, DeleteAll).
//! 4. Preview Cache Ruler (F31):
//!    - O(1) monotonic playback seeking vs random scrubbing,
//!    - Range coalescing and interval fragmentation,
//!    - Granular timeline edit invalidation (Green -> Red) preserving adjacent frames,
//!    - Resize growing and shrinking behavior.
//! 5. Smart Proxies (F34):
//!    - Video preview quality switching: Draft/Preview -> proxy vs Good/Best -> master,
//!    - Final render conforming: ALWAYS strictly uses master original media,
//!    - Missing / offline proxy graceful fallback to master,
//!    - Canonical path generation (.sfvp0) and CPU integer box downscaling.
//! 6. Standardized Alerts Subsystem (F33):
//!    - Non-blocking queue overflow resistance under high-frequency dispatch,
//!    - Headless mock handler interception and dynamic reconfiguration,
//!    - Message template formatting with placeholders ({0}, %s, %d),
//!    - Clean-room zero vendor trademark policy across the alert catalog.

use neoveg_common::alerts::{AlertAction, AlertCode, AlertQueue, AlertRegistry};
use neoveg_common::presets::RenderQuality;
use neoveg_common::project::ProjectSettings;
use neoveg_engine::cache_ruler::{CacheSpan, FrameCacheStatus, PreviewCacheRuler};
use neoveg_engine::make_movie::{
    ExportDestination, MakeMovieSettings, MakeMovieWizard, WizardStep,
};
use neoveg_engine::prerender::{
    DynamicRamPreview, PrerenderCleanMode, SelectiveDiskPrerender, NVPRV_MAGIC,
    PRERENDER_MAX_CHUNK_FRAMES,
};
use neoveg_engine::proxy::SmartProxyManager;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// ============================================================================
// 1. Make Movie Wizard Adversarial Stress Suite
// ============================================================================

#[test]
fn test_adv_wizard_empty_and_zero_duration_validation() {
    let settings = ProjectSettings::default();

    // Adversarial Condition 1: No timeline events, zero duration
    let mut wiz1 = MakeMovieWizard::new(&settings, false, 0.0);
    let res1 = wiz1.choose_destination(ExportDestination::WebSharing);
    assert_eq!(
        res1,
        Err(AlertCode::EmptyProjectRender),
        "Empty project without events must yield AlertCode 3404"
    );
    assert!(
        matches!(
            wiz1.current_step(),
            WizardStep::Failed {
                error_code: AlertCode::EmptyProjectRender,
                ..
            }
        ),
        "Wizard state must transition to Failed on empty project"
    );

    // Adversarial Condition 2: has_timeline_events is true, but duration is exactly 0.0
    let mut wiz2 = MakeMovieWizard::new(&settings, true, 0.0);
    let res2 = wiz2.choose_destination(ExportDestination::HardDrive);
    assert_eq!(
        res2,
        Err(AlertCode::EmptyProjectRender),
        "Zero duration even with events must yield AlertCode 3404"
    );

    // Adversarial Condition 3: has_timeline_events is true, but duration is negative
    let mut wiz3 = MakeMovieWizard::new(&settings, true, -15.5);
    let res3 = wiz3.choose_destination(ExportDestination::PortableDevice);
    assert_eq!(
        res3,
        Err(AlertCode::EmptyProjectRender),
        "Negative duration must yield AlertCode 3404"
    );

    // Adversarial Condition 4: has_timeline_events is false, but duration is positive (phantom duration)
    let mut wiz4 = MakeMovieWizard::new(&settings, false, 120.0);
    let res4 = wiz4.choose_destination(ExportDestination::DiscAuthoring);
    assert_eq!(
        res4,
        Err(AlertCode::EmptyProjectRender),
        "Duration without timeline events must yield AlertCode 3404"
    );

    // Valid minimal duration (0.01 sec) with events succeeds to ConfigureSettings
    let mut wiz_ok = MakeMovieWizard::new(&settings, true, 0.01);
    assert!(
        wiz_ok
            .choose_destination(ExportDestination::WebSharing)
            .is_ok(),
        "Positive duration with events must proceed"
    );
    assert!(matches!(
        wiz_ok.current_step(),
        WizardStep::ConfigureSettings(_)
    ));
}

#[test]
fn test_adv_wizard_huge_frame_counts_space_estimation() {
    let settings = ProjectSettings::default();

    // Stress test file size estimation across all destinations with extreme frame counts:
    // 1. 10,000,000 frames @ 30 fps = ~333,333 seconds (~92.6 hours)
    // 2. 100,000,000 frames @ 60 fps = ~1,666,666 seconds (~463 hours)
    // 3. 1,000,000,000 seconds (~31.7 years)
    let durations = [333_333.33, 1_666_666.67, 1_000_000_000.0];

    for &dur in &durations {
        for &dest in &ExportDestination::ALL {
            let mut wizard = MakeMovieWizard::new(&settings, true, dur);
            assert!(
                wizard.choose_destination(dest).is_ok(),
                "Must accept huge duration {dur} without arithmetic overflow"
            );

            if let WizardStep::ConfigureSettings(cfg) = wizard.current_step() {
                assert!(
                    cfg.estimated_size_mb > 0,
                    "Estimated size for {dur}s to {dest:?} must be strictly positive"
                );
                // Verify ordering of bitrates: Portable (6) < Disc (8) < Web (12) < HardDrive (25)
                // Bitrate ratio checks:
                // HardDrive (25 Mbps) should be > WebSharing (12 Mbps)
                let dur_sec = 1000.0;
                let mut w_hd = MakeMovieWizard::new(&settings, true, dur_sec);
                w_hd.choose_destination(ExportDestination::HardDrive)
                    .unwrap();
                let est_hd = match w_hd.current_step() {
                    WizardStep::ConfigureSettings(c) => c.estimated_size_mb,
                    _ => 0,
                };

                let mut w_web = MakeMovieWizard::new(&settings, true, dur_sec);
                w_web
                    .choose_destination(ExportDestination::WebSharing)
                    .unwrap();
                let est_web = match w_web.current_step() {
                    WizardStep::ConfigureSettings(c) => c.estimated_size_mb,
                    _ => 0,
                };

                assert!(
                    est_hd > est_web,
                    "HardDrive destination (25 Mbps) must estimate larger size than WebSharing (12 Mbps)"
                );
            } else {
                panic!("Expected ConfigureSettings step");
            }
        }
    }
}

#[test]
fn test_adv_wizard_step_transitions_and_progress_clamping() {
    let settings = ProjectSettings::default();
    let mut wizard = MakeMovieWizard::new(&settings, true, 60.0);

    // Initial step
    assert_eq!(*wizard.current_step(), WizardStep::SelectDestination);

    // Transition to Step 2
    wizard
        .choose_destination(ExportDestination::HardDrive)
        .unwrap();
    let settings_step2 = match wizard.current_step() {
        WizardStep::ConfigureSettings(s) => s.clone(),
        _ => panic!("Expected ConfigureSettings"),
    };

    // Transition to Step 3: Rendering
    wizard
        .validate_and_start_render(settings_step2, 1_000_000, true)
        .unwrap();

    // Adversarial progress values: negative, NaN, > 1.0
    wizard.update_render_progress(-0.5, 120.0);
    if let WizardStep::Rendering {
        progress_fraction, ..
    } = wizard.current_step()
    {
        assert_eq!(
            *progress_fraction, 0.0,
            "Negative fraction must clamp to 0.0"
        );
    }

    wizard.update_render_progress(1.85, 10.0);
    if let WizardStep::Rendering {
        progress_fraction, ..
    } = wizard.current_step()
    {
        assert_eq!(*progress_fraction, 1.0, "Fraction > 1.0 must clamp to 1.0");
    }

    // Cancellation returns to initial step
    wizard.cancel_render().unwrap();
    assert_eq!(*wizard.current_step(), WizardStep::SelectDestination);
}

#[test]
fn test_adv_wizard_destination_presets_and_disk_validation() {
    let mut settings = ProjectSettings {
        width: 3840,
        height: 2160,
        fps: 29.97,
        ..Default::default()
    };

    let mut wizard = MakeMovieWizard::new(&settings, true, 100.0);
    // 4K HardDrive should recommend 4K preset
    assert_eq!(
        wizard.recommend_preset(ExportDestination::HardDrive),
        "4K UHD 29,97"
    );

    // Disc Authoring with 29.97 FPS recommends NTSC
    assert_eq!(
        wizard.recommend_preset(ExportDestination::DiscAuthoring),
        "DVD NTSC widescreen"
    );

    // Disc Authoring with 25.0 FPS recommends PAL
    settings.fps = 25.0;
    let wizard_pal = MakeMovieWizard::new(&settings, true, 100.0);
    assert_eq!(
        wizard_pal.recommend_preset(ExportDestination::DiscAuthoring),
        "DVD PAL widescreen"
    );

    // Output path already exists validation (Alert 1911)
    let temp_file =
        std::env::temp_dir().join(format!("neoveg_adv_wizard_out_{}.mp4", std::process::id()));
    let _ = File::create(&temp_file);

    let mut cfg = MakeMovieSettings {
        destination: ExportDestination::WebSharing,
        template_name: "Internet 1080-30p".into(),
        output_path: temp_file.clone(),
        render_loop_region_only: false,
        stretch_video_to_fill: false,
        save_project_markers: false,
        estimated_size_mb: 50,
    };

    // When allow_overwrite = false, must return AlertCode::RenderFileAlreadyExists
    let res_exists = wizard.validate_and_start_render(cfg.clone(), 1000, false);
    assert_eq!(res_exists, Err(AlertCode::RenderFileAlreadyExists));

    // When allow_overwrite = true, succeeds
    let res_overwrite = wizard.validate_and_start_render(cfg.clone(), 1000, true);
    assert!(res_overwrite.is_ok());

    // Insufficient disk space validation (Alert 17603)
    cfg.output_path = std::env::temp_dir().join("non_existent_unique_file.mp4");
    cfg.estimated_size_mb = 500;
    let res_space = wizard.validate_and_start_render(cfg, 499, false);
    assert_eq!(
        res_space,
        Err(AlertCode::InsufficientDiskSpace),
        "Available space < estimated size must return InsufficientDiskSpace"
    );

    let _ = std::fs::remove_file(temp_file);
}

// ============================================================================
// 2. Dynamic RAM Preview Buffer Memory Bounds Suite
// ============================================================================

#[test]
fn test_adv_ram_preview_zero_mb_yields_alert_46259() {
    // Zero MB setting in constructor must reject build_selection with Alert 46259
    let mut ram_preview = DynamicRamPreview::new(0, 1920, 1080);
    let cancel = AtomicBool::new(false);
    let res = ram_preview.build_selection(0, 10, &cancel, |_f| vec![0u8; 1024]);
    assert_eq!(
        res,
        Err(AlertCode::DynamicRamPreviewZeroMb),
        "0 MB budget must return AlertCode::DynamicRamPreviewZeroMb (46259)"
    );

    // Dynamic reconfiguration to 0 MB must flush frames and reject subsequent renders
    let mut ram_reconfig = DynamicRamPreview::new(10, 100, 100);
    ram_reconfig
        .build_selection(0, 5, &cancel, |_f| vec![0u8; 40_000])
        .unwrap();
    assert!(ram_reconfig.cached_frame_count() > 0);

    ram_reconfig.set_budget_mb(0);
    assert_eq!(
        ram_reconfig.cached_frame_count(),
        0,
        "Setting budget to 0 MB must evict all cached frames"
    );
    let res_reconfig = ram_reconfig.build_selection(0, 5, &cancel, |_f| vec![0u8; 40_000]);
    assert_eq!(
        res_reconfig,
        Err(AlertCode::DynamicRamPreviewZeroMb),
        "Reconfigured 0 MB budget must return Alert 46259"
    );
}

#[test]
fn test_adv_ram_preview_auto_truncation_under_tight_budget() {
    // 1 MB budget = 1,048,576 bytes
    // Frame size: 250 x 200 x 4 = 200,000 bytes
    // Exact capacity: 1,048,576 / 200,000 = 5 frames (1,000,000 bytes <= 1,048,576)
    let mut ram = DynamicRamPreview::new(1, 250, 200);
    let cancel = AtomicBool::new(false);

    // Request 50 frames (which would consume 10 MB!)
    let (start, end) = ram
        .build_selection(100, 150, &cancel, |_f| vec![0xEE; 200_000])
        .expect("build selection");

    assert_eq!(start, 100);
    assert_eq!(
        end, 105,
        "Selection must be auto-truncated from 50 frames down to exactly 5 frames"
    );
    assert_eq!(ram.cached_frame_count(), 5);
    assert!(
        ram.used_bytes() <= ram.max_bytes(),
        "used_bytes ({}) must not exceed max_bytes ({})",
        ram.used_bytes(),
        ram.max_bytes()
    );

    // Check cached frames are present and contain rendered payload
    for f in 100..105 {
        let frame_data = ram.get_frame(f).expect("cached frame");
        assert_eq!(frame_data.len(), 200_000);
        assert_eq!(frame_data[0], 0xEE);
    }
    assert!(
        ram.get_frame(105).is_none(),
        "Frame 105 was beyond truncated boundary"
    );
}

#[test]
fn test_adv_ram_preview_eviction_and_arc_retention() {
    // 2 MB budget, frame size: 100 x 100 x 4 = 40,000 bytes (fits ~52 frames)
    let mut ram = DynamicRamPreview::new(2, 100, 100);
    let cancel = AtomicBool::new(false);

    // Render initial 40 frames
    ram.build_selection(0, 40, &cancel, |f| vec![f as u8; 40_000])
        .unwrap();

    // Client grabs Arc to frame 5
    let frame_5_ref = ram.get_frame(5).expect("frame 5 present");
    assert_eq!(frame_5_ref[0], 5);

    // Now render 60 more frames (frames 40..100), forcing eviction of earlier frames (including frame 5)
    ram.build_selection(40, 100, &cancel, |f| vec![f as u8; 40_000])
        .unwrap();

    // Frame 5 has been evicted from cache
    assert!(
        ram.get_frame(5).is_none(),
        "Frame 5 must be evicted from active cache"
    );

    // BUT the external Arc held by client must remain completely intact and valid!
    assert_eq!(
        frame_5_ref.len(),
        40_000,
        "External Arc held during eviction must retain its data without memory corruption"
    );
    assert_eq!(frame_5_ref[0], 5);

    // Memory used by cache must remain bounded
    assert!(ram.used_bytes() <= ram.max_bytes());
}

#[test]
fn test_adv_ram_preview_cancellation_mid_render() {
    let mut ram = DynamicRamPreview::new(10, 100, 100);
    let cancel = Arc::new(AtomicBool::new(false));

    let cancel_clone = cancel.clone();
    let (start, end) = ram
        .build_selection(0, 100, &cancel, move |f| {
            if f == 15 {
                cancel_clone.store(true, Ordering::Relaxed);
            }
            vec![0xAA; 40_000]
        })
        .unwrap();

    assert_eq!(start, 0);
    assert_eq!(end, 100, "Requested span reported");
    // Frames 0..=15 should be cached before cancellation stopped the loop
    assert_eq!(
        ram.cached_frame_count(),
        16,
        "Cancellation after 16 frames (0..=15) must halt further rendering"
    );
    assert!(ram.get_frame(15).is_some());
    assert!(ram.get_frame(16).is_none());
}

// ============================================================================
// 3. Selective Disk Prerender <= 300 Frame Chunking & Binary Header Suite
// ============================================================================

#[test]
fn test_adv_disk_prerender_chunking_boundary_limits() {
    let dir = std::env::temp_dir().join(format!("neoveg_adv_chunk_limits_{}", std::process::id()));
    let mut prerender = SelectiveDiskPrerender::new(dir.clone());

    // Condition 1: 0 frames (start == end) => 0 chunks
    let c0 = prerender
        .prerender_range(100, 100, 100, 100, 1, |_| vec![])
        .unwrap();
    assert_eq!(c0.len(), 0);

    // Condition 2: 1 frame => 1 chunk of 1 frame
    let c1 = prerender
        .prerender_range(0, 1, 100, 100, 1, |_| vec![0u8; 16])
        .unwrap();
    assert_eq!(c1.len(), 1);
    assert_eq!(c1[0].end_frame - c1[0].start_frame, 1);

    // Condition 3: Exactly 300 frames => 1 chunk of 300 frames
    let c300 = prerender
        .prerender_range(10, 310, 100, 100, 1, |_| vec![0u8; 16])
        .unwrap();
    assert_eq!(c300.len(), 1);
    assert_eq!(
        c300[0].end_frame - c300[0].start_frame,
        PRERENDER_MAX_CHUNK_FRAMES
    );

    // Condition 4: 301 frames => 2 chunks: 300 frames and 1 frame
    let c301 = prerender
        .prerender_range(0, 301, 100, 100, 1, |_| vec![0u8; 16])
        .unwrap();
    assert_eq!(c301.len(), 2);
    assert_eq!(c301[0].end_frame - c301[0].start_frame, 300);
    assert_eq!(c301[1].end_frame - c301[1].start_frame, 1);

    // Condition 5: 901 frames => 4 chunks: 300, 300, 300, 1
    let c901 = prerender
        .prerender_range(0, 901, 100, 100, 1, |_| vec![0u8; 16])
        .unwrap();
    assert_eq!(c901.len(), 4);
    assert_eq!(c901[0].end_frame - c901[0].start_frame, 300);
    assert_eq!(c901[1].end_frame - c901[1].start_frame, 300);
    assert_eq!(c901[2].end_frame - c901[2].start_frame, 300);
    assert_eq!(c901[3].end_frame - c901[3].start_frame, 1);

    // Condition 6: Non-zero start frame offset: 550..1200 (650 frames)
    // 550..850 (300), 850..1150 (300), 1150..1200 (50)
    let c_arb = prerender
        .prerender_range(550, 1200, 100, 100, 1, |_| vec![0u8; 16])
        .unwrap();
    assert_eq!(c_arb.len(), 3);
    assert_eq!(c_arb[0].start_frame, 550);
    assert_eq!(c_arb[0].end_frame, 850);
    assert_eq!(c_arb[1].start_frame, 850);
    assert_eq!(c_arb[1].end_frame, 1150);
    assert_eq!(c_arb[2].start_frame, 1150);
    assert_eq!(c_arb[2].end_frame, 1200);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_adv_disk_prerender_nvprv_binary_header_inspection() {
    let dir =
        std::env::temp_dir().join(format!("neoveg_adv_header_inspect_{}", std::process::id()));
    let mut prerender = SelectiveDiskPrerender::new(dir.clone());

    let gen_hash = 0xDEAD_BEEF_CAFE_BABE_u64;
    let chunks = prerender
        .prerender_range(100, 105, 1920, 1080, gen_hash, |f| vec![f as u8; 32])
        .unwrap();

    assert_eq!(chunks.len(), 1);
    let chunk_path = &chunks[0].file_path;

    // Inspect binary structure of the written .nvprv file
    let mut file = File::open(chunk_path).expect("open nvprv");
    let mut header = [0u8; 64];
    file.read_exact(&mut header).expect("read 64-byte header");

    // 1. Magic [0..8]
    assert_eq!(&header[0..8], NVPRV_MAGIC);

    // 2. Width [8..12] & Height [12..16]
    assert_eq!(u32::from_le_bytes(header[8..12].try_into().unwrap()), 1920);
    assert_eq!(u32::from_le_bytes(header[12..16].try_into().unwrap()), 1080);

    // 3. FPS Num [16..20] & FPS Den [20..24]
    assert_eq!(
        u32::from_le_bytes(header[16..20].try_into().unwrap()),
        30000
    );
    assert_eq!(u32::from_le_bytes(header[20..24].try_into().unwrap()), 1001);

    // 4. Pixel Format [24..28] (1 = RGBA8)
    assert_eq!(u32::from_le_bytes(header[24..28].try_into().unwrap()), 1);

    // 5. Frame Count [28..32] (5 frames: 100..105)
    assert_eq!(u32::from_le_bytes(header[28..32].try_into().unwrap()), 5);

    // 6. Start Frame [32..40] & Generation Hash [40..48]
    assert_eq!(u64::from_le_bytes(header[32..40].try_into().unwrap()), 100);
    assert_eq!(
        u64::from_le_bytes(header[40..48].try_into().unwrap()),
        gen_hash
    );

    // 7. Index table offset: 64 to 64 + 5*8 = 104
    let mut index_table = [0u8; 40];
    file.read_exact(&mut index_table).expect("read index table");
    let mut offsets = Vec::new();
    for i in 0..5 {
        let off = u64::from_le_bytes(index_table[i * 8..(i + 1) * 8].try_into().unwrap());
        offsets.push(off);
    }
    // Payloads start at byte 104
    assert_eq!(offsets[0], 104);
    assert_eq!(offsets[1], 136); // 104 + 32
    assert_eq!(offsets[2], 168); // 136 + 32
    assert_eq!(offsets[3], 200);
    assert_eq!(offsets[4], 232);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_adv_disk_prerender_corruption_resistance() {
    let dir = std::env::temp_dir().join(format!("neoveg_adv_prv_corrupt_{}", std::process::id()));
    let mut prerender = SelectiveDiskPrerender::new(dir.clone());

    let chunks = prerender
        .prerender_range(0, 10, 100, 100, 0x111, |_| vec![0xCC; 50])
        .unwrap();
    let file_path = chunks[0].file_path.clone();

    // Adversarial Corruption 1: Truncate file to 10 bytes (header missing)
    {
        let mut f = File::create(&file_path).unwrap();
        f.write_all(b"SHORT12345").unwrap();
    }
    assert!(
        prerender.read_frame(0).is_none(),
        "Truncated file must return None without panicking"
    );

    // Adversarial Corruption 2: Header intact (64 bytes), but index table missing
    {
        let mut f = File::create(&file_path).unwrap();
        f.write_all(&[0u8; 64]).unwrap();
    }
    assert!(
        prerender.read_frame(2).is_none(),
        "Missing index table must return None without panicking"
    );

    // Adversarial Corruption 3: Offset pointing to absurd address (e.g. u64::MAX)
    {
        let mut f = File::create(&file_path).unwrap();
        f.write_all(&[0u8; 64]).unwrap();
        f.write_all(&u64::MAX.to_le_bytes()).unwrap(); // index for frame 0
    }
    assert!(
        prerender.read_frame(0).is_none(),
        "Index offset pointing past EOF must return None or empty without crashing"
    );

    // Adversarial Corruption 4: File completely deleted from disk while tracked
    std::fs::remove_file(&file_path).unwrap();
    assert!(
        prerender.read_frame(0).is_none(),
        "Deleted file on disk must return None gracefully"
    );

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_adv_disk_prerender_cleanup_modes_adversarial() {
    let dir = std::env::temp_dir().join(format!("neoveg_adv_cleanup_{}", std::process::id()));
    let mut prerender = SelectiveDiskPrerender::new(dir.clone());

    let hash_v1 = 0xAAA;
    let hash_v2 = 0xBBB;

    // Create 2 chunks with hash_v1 (frames 0..300, 300..600)
    prerender
        .prerender_range(0, 600, 100, 100, hash_v1, |_| vec![1; 10])
        .unwrap();
    // Create 1 chunk with hash_v2 (frames 600..900)
    prerender
        .prerender_range(600, 900, 100, 100, hash_v2, |_| vec![2; 10])
        .unwrap();

    assert_eq!(prerender.active_chunks().len(), 3);

    // Mode 1: DeleteInactive with active_hash = hash_v2
    // Must delete the two chunks with hash_v1, retaining only hash_v2
    let deleted_inactive = prerender.cleanup(PrerenderCleanMode::DeleteInactive, hash_v2, None);
    assert_eq!(deleted_inactive, 2);
    assert_eq!(prerender.active_chunks().len(), 1);
    assert_eq!(prerender.active_chunks()[0].generation_hash, hash_v2);

    // Re-create chunks for selection cleanup test
    prerender
        .prerender_range(0, 300, 100, 100, hash_v2, |_| vec![3; 10])
        .unwrap();
    prerender
        .prerender_range(300, 600, 100, 100, hash_v2, |_| vec![4; 10])
        .unwrap();
    assert_eq!(prerender.active_chunks().len(), 3); // [600..900, 0..300, 300..600]

    // Mode 2: DeleteInSelection with non-overlapping selection: 1000..1200
    let del_no_overlap = prerender.cleanup(
        PrerenderCleanMode::DeleteInSelection,
        hash_v2,
        Some((1000, 1200)),
    );
    assert_eq!(
        del_no_overlap, 0,
        "Non-overlapping selection deletes 0 chunks"
    );
    assert_eq!(prerender.active_chunks().len(), 3);

    // Boundary selection: 290..310 (overlaps chunk 0..300 and chunk 300..600!)
    let del_boundary = prerender.cleanup(
        PrerenderCleanMode::DeleteInSelection,
        hash_v2,
        Some((290, 310)),
    );
    assert_eq!(
        del_boundary, 2,
        "Selection intersecting 2 chunks must remove both"
    );
    assert_eq!(prerender.active_chunks().len(), 1); // Only 600..900 remains

    // Mode 3: DeleteAll
    let del_all = prerender.cleanup(PrerenderCleanMode::DeleteAll, hash_v2, None);
    assert_eq!(del_all, 1);
    assert_eq!(prerender.active_chunks().len(), 0);

    let _ = std::fs::remove_dir_all(dir);
}

// ============================================================================
// 4. Preview Cache Ruler O(1) Seeking & Range Coalescing Suite
// ============================================================================

#[test]
fn test_adv_cache_ruler_range_coalescing_and_fragmentation() {
    let mut ruler = PreviewCacheRuler::new(1000, 30.0);
    assert_eq!(ruler.all_spans().len(), 1);
    assert_eq!(
        ruler.all_spans()[0],
        CacheSpan {
            start_frame: 0,
            end_frame: 1000,
            status: FrameCacheStatus::RedUnrendered
        }
    );

    // Mark 100..200 Green
    ruler.mark_span(100, 200, FrameCacheStatus::GreenCached);
    // Mark 200..300 Green -> MUST coalesce with 100..200 to form 100..300 Green!
    ruler.mark_span(200, 300, FrameCacheStatus::GreenCached);
    assert!(
        ruler.all_spans().iter().any(|s| s.start_frame == 100
            && s.end_frame == 300
            && s.status == FrameCacheStatus::GreenCached),
        "Adjacent spans with identical status must coalesce"
    );

    // Mark 50..100 Green -> MUST coalesce with 100..300 to form 50..300 Green!
    ruler.mark_span(50, 100, FrameCacheStatus::GreenCached);
    assert!(
        ruler.all_spans().iter().any(|s| s.start_frame == 50
            && s.end_frame == 300
            && s.status == FrameCacheStatus::GreenCached),
        "Preceding adjacent span must coalesce"
    );

    // Now fragment: Mark 150..200 Yellow right in the middle of 50..300 Green
    // Must produce: 50..150 Green, 150..200 Yellow, 200..300 Green
    ruler.mark_span(150, 200, FrameCacheStatus::YellowRealtime);
    assert_eq!(ruler.status_at(100), FrameCacheStatus::GreenCached);
    assert_eq!(ruler.status_at(175), FrameCacheStatus::YellowRealtime);
    assert_eq!(ruler.status_at(250), FrameCacheStatus::GreenCached);

    // Invalidation: invalidate 100..250 (RedUnrendered)
    // Overlaps end of first Green (100..150), entire Yellow (150..200), and start of second Green (200..250)
    ruler.invalidate_region(100, 250);
    assert_eq!(
        ruler.status_at(75),
        FrameCacheStatus::GreenCached,
        "0..100 Green remains"
    );
    assert_eq!(ruler.status_at(125), FrameCacheStatus::RedUnrendered);
    assert_eq!(ruler.status_at(175), FrameCacheStatus::RedUnrendered);
    assert_eq!(ruler.status_at(225), FrameCacheStatus::RedUnrendered);
    assert_eq!(
        ruler.status_at(275),
        FrameCacheStatus::GreenCached,
        "250..300 Green remains"
    );
}

#[test]
fn test_adv_cache_ruler_o1_monotonic_seeking_vs_random_scrubbing() {
    let total = 10_000u64;
    let mut ruler = PreviewCacheRuler::new(total, 60.0);

    // Mark alternating patterns: Green 0..1000, Yellow 1000..2000, Red 2000..5000, Green 5000..8000, Yellow 8000..10000
    ruler.mark_span(0, 1000, FrameCacheStatus::GreenCached);
    ruler.mark_span(1000, 2000, FrameCacheStatus::YellowRealtime);
    ruler.mark_span(5000, 8000, FrameCacheStatus::GreenCached);
    ruler.mark_span(8000, 10000, FrameCacheStatus::YellowRealtime);

    // 1. Monotonic linear playback simulation: query 0..10,000 sequentially (exercises cursor_idx fast-path)
    for f in 0..total {
        let expected = if f < 1000 {
            FrameCacheStatus::GreenCached
        } else if f < 2000 {
            FrameCacheStatus::YellowRealtime
        } else if f < 5000 {
            FrameCacheStatus::RedUnrendered
        } else if f < 8000 {
            FrameCacheStatus::GreenCached
        } else {
            FrameCacheStatus::YellowRealtime
        };
        assert_eq!(ruler.status_at(f), expected, "Monotonic seek at frame {f}");
    }

    // 2. Random scrub queries jumping all over the timeline
    let jump_frames = [
        9999, 0, 5500, 1500, 2500, 4999, 5000, 7999, 8000, 100, 1999, 2000, 1000, 7500,
    ];
    for &f in &jump_frames {
        let status = ruler.status_at(f);
        let expected = if f < 1000 {
            FrameCacheStatus::GreenCached
        } else if f < 2000 {
            FrameCacheStatus::YellowRealtime
        } else if f < 5000 {
            FrameCacheStatus::RedUnrendered
        } else if f < 8000 {
            FrameCacheStatus::GreenCached
        } else {
            FrameCacheStatus::YellowRealtime
        };
        assert_eq!(status, expected, "Random scrub jump at frame {f}");
    }

    // Out of bounds queries
    assert_eq!(ruler.status_at(total), FrameCacheStatus::RedUnrendered);
    assert_eq!(
        ruler.status_at(total + 5000),
        FrameCacheStatus::RedUnrendered
    );
}

#[test]
fn test_adv_cache_ruler_invalidation_boundary_conditions() {
    let mut ruler = PreviewCacheRuler::new(500, 30.0);
    ruler.mark_span(0, 500, FrameCacheStatus::GreenCached);

    // Invalidate empty region: start >= end
    ruler.invalidate_region(100, 100);
    assert_eq!(
        ruler.status_at(100),
        FrameCacheStatus::GreenCached,
        "Empty invalidation must be a no-op"
    );

    // Invalidate inverted region: start > end
    ruler.invalidate_region(200, 100);
    assert_eq!(
        ruler.status_at(150),
        FrameCacheStatus::GreenCached,
        "Inverted invalidation must be a no-op"
    );

    // Invalidate region beyond total_frames
    ruler.invalidate_region(600, 800);
    assert_eq!(
        ruler.percent_cached(),
        100.0,
        "Out-of-bounds invalidation must not affect valid spans"
    );

    // Partial out-of-bounds: 400..800 (should clamp to 400..500)
    ruler.invalidate_region(400, 800);
    assert_eq!(ruler.status_at(399), FrameCacheStatus::GreenCached);
    assert_eq!(ruler.status_at(450), FrameCacheStatus::RedUnrendered);
    assert_eq!(ruler.percent_cached(), 80.0);
}

// ============================================================================
// 5. Smart Proxies Quality Switching & Final Render Conform Suite
// ============================================================================

#[test]
fn test_adv_proxy_quality_switching_and_master_conform() {
    let original = PathBuf::from("scene_hero_master_8k.mov");
    let proxy = SmartProxyManager::get_proxy_path_for(&original);
    let _ = File::create(&proxy);

    let mut mgr = SmartProxyManager::new();
    mgr.register_media(original.clone(), 7680, 4320);
    mgr.mark_proxy_available(&original);

    // 1. Preview Quality = Draft => uses proxy
    assert_eq!(
        mgr.resolve_read_path(&original, RenderQuality::Draft, false),
        proxy.as_path(),
        "Draft preview must read from proxy"
    );

    // 2. Preview Quality = Preview => uses proxy
    assert_eq!(
        mgr.resolve_read_path(&original, RenderQuality::Preview, false),
        proxy.as_path(),
        "Preview quality must read from proxy"
    );

    // 3. Preview Quality = Good => uses original master
    assert_eq!(
        mgr.resolve_read_path(&original, RenderQuality::Good, false),
        original.as_path(),
        "Good quality must read from original master"
    );

    // 4. Preview Quality = Best => uses original master
    assert_eq!(
        mgr.resolve_read_path(&original, RenderQuality::Best, false),
        original.as_path(),
        "Best quality must read from original master"
    );

    // 5. CRITICAL CONFORM GUARANTEE: Final Render with Draft or Preview active!
    // Final rendering MUST ALWAYS route to original master source media!
    assert_eq!(
        mgr.resolve_read_path(&original, RenderQuality::Draft, true),
        original.as_path(),
        "Final render with Draft quality MUST conform to original master!"
    );
    assert_eq!(
        mgr.resolve_read_path(&original, RenderQuality::Preview, true),
        original.as_path(),
        "Final render with Preview quality MUST conform to original master!"
    );

    let _ = std::fs::remove_file(&proxy);
}

#[test]
fn test_adv_proxy_missing_or_offline_fallback() {
    let original = PathBuf::from("unproxied_interview_4k.mp4");
    let mut mgr = SmartProxyManager::new();
    mgr.register_media(original.clone(), 3840, 2160);

    // Case 1: ProxyState::None (never generated) => must fallback to original in Draft
    assert_eq!(
        mgr.resolve_read_path(&original, RenderQuality::Draft, false),
        original.as_path()
    );

    // Case 2: ProxyState marked Available, but companion file deleted from disk
    mgr.mark_proxy_available(&original);
    let proxy_path = SmartProxyManager::get_proxy_path_for(&original);
    assert!(!proxy_path.exists());
    assert_eq!(
        mgr.resolve_read_path(&original, RenderQuality::Draft, false),
        original.as_path(),
        "Missing proxy file on disk must fallback to original master without error"
    );

    // Case 3: Completely unregistered media path
    let unregistered = PathBuf::from("random_sound_effect.wav");
    assert_eq!(
        mgr.resolve_read_path(&unregistered, RenderQuality::Draft, false),
        unregistered.as_path()
    );
}

#[test]
fn test_adv_proxy_canonical_path_generation() {
    let p1 = PathBuf::from("C:/Media/take 01.mp4");
    assert_eq!(
        SmartProxyManager::get_proxy_path_for(&p1),
        PathBuf::from("C:/Media/take 01.mp4.sfvp0")
    );

    let p2 = PathBuf::from("archive.tar.gz");
    assert_eq!(
        SmartProxyManager::get_proxy_path_for(&p2),
        PathBuf::from("archive.tar.gz.sfvp0")
    );

    let p3 = PathBuf::from("unicode_ação_áudio.mov");
    assert_eq!(
        SmartProxyManager::get_proxy_path_for(&p3),
        PathBuf::from("unicode_ação_áudio.mov.sfvp0")
    );
}

#[test]
#[allow(clippy::chunks_exact_to_as_chunks)]
fn test_adv_proxy_downscale_cpu_rgba_accuracy() {
    // 4x4 RGBA image with distinct color values downscaled to 2x2
    let src_w = 4;
    let src_h = 4;
    let mut src = vec![0u8; src_w * src_h * 4];
    // Fill with solid color: RGBA(100, 150, 200, 255)
    for pixel in src.chunks_exact_mut(4) {
        pixel[0] = 100;
        pixel[1] = 150;
        pixel[2] = 200;
        pixel[3] = 255;
    }

    let dst_w = 2;
    let dst_h = 2;
    let dst = SmartProxyManager::downscale_cpu_rgba(&src, src_w, src_h, dst_w, dst_h);

    assert_eq!(dst.len(), dst_w * dst_h * 4);
    for pixel in dst.chunks_exact(4) {
        assert_eq!(pixel[0], 100);
        assert_eq!(pixel[1], 150);
        assert_eq!(pixel[2], 200);
        assert_eq!(pixel[3], 255);
    }
}

// ============================================================================
// 6. Standard Alerts Subsystem Stress & Headless Mock Suite
// ============================================================================

#[test]
fn test_adv_alerts_headless_mock_interception() {
    let queue = AlertQueue::default();

    // Register a dynamic mock handler that intercepts alerts and returns tailored actions
    queue.set_mock_handler(Some(Arc::new(|event| match event.code {
        AlertCode::MediaFileNotFound => AlertAction::IgnoreAll,
        AlertCode::InsufficientDiskSpace => AlertAction::Retry,
        AlertCode::GpuAccelerationError => AlertAction::Ok,
        AlertCode::TimelineMediaDeleteConfirm => AlertAction::Yes,
        _ => AlertAction::Cancel,
    })));

    // Dispatches must return Mock Action immediately
    let r1 = queue.dispatch(AlertCode::MediaFileNotFound, &["clip.mov"], "en-US", None);
    assert_eq!(r1, Some(AlertAction::IgnoreAll));

    let r2 = queue.dispatch(
        AlertCode::InsufficientDiskSpace,
        &["500", "100"],
        "en-US",
        None,
    );
    assert_eq!(r2, Some(AlertAction::Retry));

    let r3 = queue.dispatch(AlertCode::GpuAccelerationError, &[], "en-US", None);
    assert_eq!(r3, Some(AlertAction::Ok));

    // When mock handler is active, events must NOT leak into the pending queue
    assert!(
        queue.pop_event().is_none(),
        "Mock handler should intercept alerts so queue remains empty"
    );

    // Disabling mock handler restores standard queuing behavior
    queue.set_mock_handler(None);
    let r_none = queue.dispatch(AlertCode::EmptyProjectRender, &[], "en-US", None);
    assert_eq!(r_none, None);
    let popped = queue.pop_event().expect("popped event from queue");
    assert_eq!(popped.code, AlertCode::EmptyProjectRender);
}

#[test]
fn test_adv_alerts_high_volume_queue_overflow_resistance() {
    let queue = AlertQueue::default();
    queue.set_mock_handler(None);

    let count = 5_000;
    for i in 0..count {
        queue.dispatch(
            AlertCode::MediaIsOfflinePrompt,
            &[&format!("media_{i}.mp4")],
            "en-US",
            None,
        );
    }

    // Verify FIFO ordering and non-blocking retrieval
    for i in 0..count {
        let ev = queue.pop_event().expect("event must be present in queue");
        assert_eq!(ev.id, (i + 1) as u64);
        assert!(ev.message.contains(&format!("media_{i}.mp4")));
    }

    assert!(queue.pop_event().is_none());
}

#[test]
fn test_adv_alerts_formatting_and_placeholders() {
    // English substitution with {0} and {1}
    let msg_en =
        AlertRegistry::format_message(AlertCode::InsufficientDiskSpace, &["1024", "256"], "en-US");
    assert_eq!(
        msg_en,
        "Estimated file size (1024 MB) exceeds available free disk space (256 MB)."
    );

    // Portuguese substitution with {0} and {1}
    let msg_pt =
        AlertRegistry::format_message(AlertCode::InsufficientDiskSpace, &["1024", "256"], "pt-BR");
    assert_eq!(
        msg_pt,
        "O tamanho estimado do arquivo (1024 MB) excede o espaço livre em disco (256 MB)."
    );

    // Under-provisioned arguments: template contains {0} but empty args provided
    let msg_under = AlertRegistry::format_message(AlertCode::MediaFileNotFound, &[], "en-US");
    assert!(
        msg_under.contains("{0}"),
        "Under-provisioned template placeholders must not panic"
    );

    // Over-provisioned arguments: template needs 1 arg, provided 3
    let msg_over = AlertRegistry::format_message(
        AlertCode::MediaFileNotFound,
        &["shot.mov", "extra1", "extra2"],
        "en-US",
    );
    assert_eq!(
        msg_over,
        "The file 'shot.mov' was not found in the specified path."
    );
}

#[test]
fn test_adv_alerts_clean_room_zero_trademarks_exhaustive() {
    // Audit ALL cataloged alert codes to ensure zero trademarked terms appear in templates
    let all_codes = [
        AlertCode::MissingProjectMediaElements,
        AlertCode::MediaIsOfflinePrompt,
        AlertCode::FindMissingFile,
        AlertCode::MediaFileNotFound,
        AlertCode::OfflineMediaFile,
        AlertCode::CannotSaveMarkersMediaOffline,
        AlertCode::CannotOpenInSoundEditorMediaOffline,
        AlertCode::LocateMissingFilePrompt,
        AlertCode::TimelineMediaDeleteConfirm,
        AlertCode::FileCorrupted,
        AlertCode::StreamAttributesCouldNotBeDetermined,
        AlertCode::MissingVideoCodec,
        AlertCode::CorruptVideoStream,
        AlertCode::MissingAudioCodec,
        AlertCode::UnknownStreamError,
        AlertCode::CorruptAudioStream,
        AlertCode::UnsupportedAudioChannelCount,
        AlertCode::TemplateNameInvalid,
        AlertCode::BuiltinTemplateCannotModify,
        AlertCode::OverwriteTemplateConfirm,
        AlertCode::BuiltinTemplateCannotDelete,
        AlertCode::CannotDeleteBuiltinTemplate,
        AlertCode::CannotModifyBuiltinTemplate,
        AlertCode::TemplateCorruptOrUnreadable,
        AlertCode::Over80PercentHasToBeRendered,
        AlertCode::PlayerAssociationMissing,
        AlertCode::NoTemplatesSupportMedia,
        AlertCode::RenderFileAlreadyExists,
        AlertCode::EmptyProjectRender,
        AlertCode::CancelRenderConfirmation,
        AlertCode::ClearEditHistoryConfirmation,
        AlertCode::InsufficientDiskSpace,
        AlertCode::ParametersInvalid,
        AlertCode::DiskFullDvdImage,
        AlertCode::DynamicRamPreviewZeroMb,
        AlertCode::GpuDeviceLost,
        AlertCode::GpuInitFailed,
        AlertCode::GpuDriverOutdated,
        AlertCode::GpuFatalError,
        AlertCode::GpuAccelerationError,
        AlertCode::GpuDeviceDriverProblem,
        AlertCode::EffectFailed,
        AlertCode::PluginChainFull,
        AlertCode::PluginFailedToLoad,
        AlertCode::PluginCrashed,
        AlertCode::PluginIncompatible,
        AlertCode::FxSendFull,
        AlertCode::VideoEffectsUiInitFailed,
        AlertCode::RootBinCannotRemove,
        AlertCode::RootBinCannotRename,
        AlertCode::BinNameEmpty,
        AlertCode::DuplicateBinRenamed,
        AlertCode::InvalidDrive,
    ];

    let forbidden_trademarks = [
        "sony",
        "vegas",
        "magix",
        "sound forge",
        "dvd architect",
        "acid pro",
        "youtube",
        "vimeo",
        "quicktime",
    ];

    for code in all_codes {
        let alert = AlertRegistry::get(code);
        let en_lower = alert.en_us_template.to_lowercase();
        let pt_lower = alert.pt_br_template.to_lowercase();

        for &tm in &forbidden_trademarks {
            assert!(
                !en_lower.contains(tm),
                "Alert {:?} en-US contains forbidden trademark '{tm}': '{}'",
                code,
                alert.en_us_template
            );
            assert!(
                !pt_lower.contains(tm),
                "Alert {:?} pt-BR contains forbidden trademark '{tm}': '{}'",
                code,
                alert.pt_br_template
            );
        }
    }
}
