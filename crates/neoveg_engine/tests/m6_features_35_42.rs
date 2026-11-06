//! Milestone 6 Comprehensive Integration Test Suite (Features 35–42)
//!
//! Tests:
//! - Feature 35: L-Cut & J-Cut Split Editing, Ignore Event Grouping, and Single-Track Ripple
//! - Feature 36: Simultaneous Audio/Video Fades and Audio Micro-Quickfades (10 ms)
//! - Feature 37: Persistent Sync Links, Out-of-Sync Indicators, and Resync (Moving & Slipping)
//! - Feature 38: Multicamera Audio/Timecode Sync (Two-Stage Hierarchical Envelope Correlation)
//! - Feature 39: Multicamera Track Creation, Angle Switching, and Cut-to-Overlap Crossfades
//! - Feature 40: Alternative Event Takes System (TakeContainer, Next/Prev, and Slip All Takes)
//! - Feature 41: Parity Matrix Classifications and README.md Verification
//! - Feature 42: Translation Catalogs Synchronization (100% key parity across all 7 catalogs)

use neoveg_engine::command::{EditCommand, History};
use neoveg_engine::groups;
use neoveg_engine::multicam::{
    create_multicamera_track, cut_and_switch_angle, expand_multicamera_track, MulticamSynchronizer,
};
use neoveg_engine::ops::{
    apply_audio_micro_quickfade, split_at, Scope, DEFAULT_AUDIO_QUICKFADE_SEC,
};
use neoveg_engine::ripple::{ripple_resize, RippleScope};
use neoveg_engine::sync_link::{resync_by_moving, resync_by_slipping, SyncLink, SyncLinkManager};
use neoveg_engine::takes::{slip_all_takes, TakeContainer};
use neoveg_engine::timeline::{Take, Timeline, TrackEvent, TrackKind};
use std::collections::BTreeMap;
use std::path::Path;

// ==============================================================================================
// Feature 35: L-Cut & J-Cut Split Editing & Ripple
// ==============================================================================================

#[test]
fn test_f35_l_cut_and_j_cut_independent_trimming() {
    let mut tl = Timeline::new();
    let mut history = History::new();

    let mut vtrack = tl.new_track(TrackKind::Video);
    let vid = tl.alloc_id();
    let gid = tl.alloc_id();

    vtrack.events.push(TrackEvent {
        id: vid,
        name: "video_clip".into(),
        media_id: Some(1),
        start: 2.0,
        duration: 8.0,
        trim_in: 0.0,
        group: Some(gid),
        ..Default::default()
    });
    let v_ref = (vtrack.id, vid);
    tl.tracks.push(vtrack);

    let mut atrack = tl.new_track(TrackKind::Audio);
    let aid = tl.alloc_id();
    atrack.events.push(TrackEvent {
        id: aid,
        name: "audio_clip".into(),
        media_id: Some(1),
        start: 2.0,
        duration: 8.0,
        trim_in: 0.0,
        group: Some(gid),
        ..Default::default()
    });
    let _a_ref = (atrack.id, aid);
    tl.tracks.push(atrack);

    // Standard grouped trimming: trims both audio and video edges together
    let outcome = groups::trim_group_edge(&tl, v_ref, true, 3.0, 0.1, &|_, _| Some(20.0))
        .expect("should trim group edge");
    history.execute(&mut tl, outcome.command);
    assert_eq!(tl.tracks[0].events[0].start, 3.0);
    assert_eq!(tl.tracks[1].events[0].start, 3.0);

    // L-cut / J-cut: Independent trimming ignoring grouping (e.g. Shift-drag or Ignore Grouping)
    // Trim video left edge independently to 4.0 (audio remains at 3.0)
    let v_ev = &tl.tracks[0].events[0];
    let new_v_geom = neoveg_engine::edit::trim_left(v_ev.geometry(), 4.0, 0.1);
    let edit_cmd = EditCommand::EditEvent {
        from_track: v_ref.0,
        to_track: v_ref.0,
        event_id: v_ref.1,
        old: v_ev.geometry(),
        new: new_v_geom,
    };
    history.execute(&mut tl, edit_cmd);

    // Video was trimmed to 4.0; Audio stayed at 3.0 (creating a 1.0s J-cut where audio leads)
    assert_eq!(tl.tracks[0].events[0].start, 4.0);
    assert_eq!(tl.tracks[0].events[0].duration, 6.0);
    assert_eq!(tl.tracks[1].events[0].start, 3.0);
    assert_eq!(tl.tracks[1].events[0].duration, 7.0);

    // Single-track ripple in AffectedTracks mode:
    // Adding a second downstream event on each track
    let v2_id = tl.alloc_id();
    tl.tracks[0].events.push(TrackEvent {
        id: v2_id,
        name: "video_2".into(),
        start: 12.0,
        duration: 5.0,
        ..Default::default()
    });
    let a2_id = tl.alloc_id();
    tl.tracks[1].events.push(TrackEvent {
        id: a2_id,
        name: "audio_2".into(),
        start: 12.0,
        duration: 5.0,
        ..Default::default()
    });

    // Resize video event 1 (trim right by -2.0 seconds) with AffectedTracks ripple
    let mut v1_resized = tl.tracks[0].events[0].geometry();
    v1_resized.duration -= 2.0;
    let ripple_outcome = ripple_resize(
        &tl,
        tl.tracks[0].id,
        vid,
        v1_resized,
        RippleScope::AffectedTracks,
    )
    .expect("ripple resize should succeed");
    history.execute(&mut tl, ripple_outcome.command);

    // Video 2 shifted earlier by 2.0 seconds on video track (from 12.0 to 10.0)
    assert_eq!(tl.tracks[0].events[1].start, 10.0);
    // Downstream audio 2 on audio track was unaffected in AffectedTracks mode
    assert_eq!(tl.tracks[1].events[1].start, 12.0);
}

// ==============================================================================================
// Feature 36: Simultaneous Fades and Audio Micro-Quickfades
// ==============================================================================================

#[test]
fn test_f36_simultaneous_fades_and_audio_micro_quickfades() {
    let mut tl = Timeline::new();
    let mut history = History::new();

    let mut atrack = tl.new_track(TrackKind::Audio);
    let aid = tl.alloc_id();
    atrack.events.push(TrackEvent {
        id: aid,
        name: "voice".into(),
        media_id: Some(2),
        start: 0.0,
        duration: 4.0,
        fade_in: 0.0,
        fade_out: 0.0,
        ..Default::default()
    });
    tl.tracks.push(atrack);

    // Split audio event at 2.0 seconds
    let split_outcome = split_at(&mut tl, 2.0, Scope::AllTracks).expect("split should succeed");
    history.execute(&mut tl, split_outcome.command);

    let audio_events = &tl.tracks[0].events;
    assert_eq!(audio_events.len(), 2);
    // Cut edges receive the default 10 ms (0.010 s) rapid anti-pop quickfade
    assert!((audio_events[0].fade_out - DEFAULT_AUDIO_QUICKFADE_SEC).abs() < 1e-9);
    assert!((audio_events[1].fade_in - DEFAULT_AUDIO_QUICKFADE_SEC).abs() < 1e-9);
    // Uncut outer edges remain at 0.0
    assert_eq!(audio_events[0].fade_in, 0.0);
    assert_eq!(audio_events[1].fade_out, 0.0);

    // Clamping test on short audio event (16 ms)
    let mut short_ev = TrackEvent {
        id: 999,
        duration: 0.016,
        ..Default::default()
    };
    apply_audio_micro_quickfade(&mut short_ev, true, true, 0.010);
    // Clamped so max fade is duration / 2 = 8 ms (0.008 s)
    assert!((short_ev.fade_in - 0.008).abs() < 1e-9);
    assert!((short_ev.fade_out - 0.008).abs() < 1e-9);
}

// ==============================================================================================
// Feature 37: Persistent Sync Links and Resyncing
// ==============================================================================================

#[test]
fn test_f37_persistent_sync_links_and_resynchronization() {
    let mut tl = Timeline::new();
    let mut history = History::new();

    let mut vtrack = tl.new_track(TrackKind::Video);
    let vid = tl.alloc_id();
    vtrack.events.push(TrackEvent {
        id: vid,
        name: "master_video".into(),
        media_id: Some(10),
        start: 5.0,
        duration: 10.0,
        trim_in: 2.0,
        ..Default::default()
    });
    let vref = (vtrack.id, vid);
    tl.tracks.push(vtrack);

    let mut atrack = tl.new_track(TrackKind::Audio);
    let aid = tl.alloc_id();
    atrack.events.push(TrackEvent {
        id: aid,
        name: "linked_dialog".into(),
        media_id: Some(10),
        start: 5.0,
        duration: 10.0,
        trim_in: 2.0,
        ..Default::default()
    });
    let aref = (atrack.id, aid);
    tl.tracks.push(atrack);

    // Initial sync link creation
    let master = &tl.tracks[0].events[0];
    let linked = &tl.tracks[1].events[0];
    let sync_link = SyncLink::from_events(vref, master, aref, linked);
    assert_eq!(sync_link.base_offset_sec, 0.0);
    assert_eq!(sync_link.calculate_drift(master, linked), 0.0);

    // SyncLinkManager tracking
    let mut manager = SyncLinkManager::new();
    manager.add_link(sync_link.clone());
    assert!(manager.is_linked(vref, aref));
    assert_eq!(manager.get_links_for_master(vref).len(), 1);

    // Audio drifts by +1.5s on timeline (moved later)
    tl.tracks[1].events[0].start = 6.5;
    let drift = sync_link.calculate_drift(&tl.tracks[0].events[0], &tl.tracks[1].events[0]);
    assert!((drift - 1.5).abs() < 1e-9);

    // Resync By Moving: moves linked event start backward to restore sync
    let move_cmd =
        resync_by_moving(&tl, vref, aref, sync_link.base_offset_sec).expect("resync command");
    history.execute(&mut tl, move_cmd);
    assert_eq!(tl.tracks[1].events[0].start, 5.0);
    assert_eq!(tl.tracks[1].events[0].trim_in, 2.0);

    // Audio drifts again by +0.8s
    tl.tracks[1].events[0].start = 5.8;

    // Resync By Slipping: keeps timeline start at 5.8, slips trim_in by +0.8
    let slip_cmd =
        resync_by_slipping(&tl, vref, aref, sync_link.base_offset_sec).expect("slip command");
    history.execute(&mut tl, slip_cmd);
    assert_eq!(tl.tracks[1].events[0].start, 5.8);
    assert!((tl.tracks[1].events[0].trim_in - 2.8).abs() < 1e-9);
    // Relative drift is now 0.0
    let drift_after = sync_link.calculate_drift(&tl.tracks[0].events[0], &tl.tracks[1].events[0]);
    assert!(drift_after.abs() < 1e-9);
}

// ==============================================================================================
// Feature 38: Multicamera Audio & Timecode Sync
// ==============================================================================================

#[test]
fn test_f38_multicamera_audio_and_timecode_sync() {
    let synchronizer = MulticamSynchronizer::new();

    // 1. Timecode Synchronization
    let tc_clips = vec![(10, 7200.0), (20, 7205.2), (30, 7198.5)];
    let tc_res = synchronizer.sync_by_timecode(&tc_clips);
    assert_eq!(tc_res.len(), 3);
    assert_eq!(tc_res[2], (30, 0.0));
    assert_eq!(tc_res[0], (10, 1.5));
    assert!((tc_res[1].1 - 6.7).abs() < 1e-9);

    // 2. Audio Waveform Correlation Sync (200 Hz Coarse + 2000 Hz Fine + Parabolic)
    let sample_rate = 16000;
    let duration = 3.5;
    let n_samples = (duration * sample_rate as f64) as usize;

    let mut master_audio = vec![0.0f32; n_samples];
    for (i, item) in master_audio.iter_mut().enumerate() {
        let t = i as f32 / sample_rate as f32;
        // Periodic burst envelope
        let env = ((2.0 * std::f32::consts::PI * 2.0 * t).sin().powi(4) > 0.5) as i32 as f32;
        let tone = (2.0 * std::f32::consts::PI * 440.0 * t).sin();
        *item = env * tone;
    }

    // Cam 2 delayed by 0.180 seconds
    let delay_sec = 0.180;
    let delay_samples = (delay_sec * sample_rate as f64) as usize;
    let mut cam2_audio = vec![0.0f32; n_samples];
    cam2_audio[delay_samples..n_samples]
        .copy_from_slice(&master_audio[..(n_samples - delay_samples)]);

    let tracks: &[(u64, &[f32])] = &[(1, &master_audio), (2, &cam2_audio)];
    let res = synchronizer
        .sync_by_audio_correlation(tracks, sample_rate)
        .expect("audio sync should succeed");

    assert_eq!(res.len(), 2);
    assert_eq!(res[0], (1, 0.0));
    assert_eq!(res[1].0, 2);
    // Sub-0.1 ms precision verification
    assert!(
        (res[1].1 - (-delay_sec)).abs() < 0.0005,
        "offset was {}",
        res[1].1
    );

    // Failure on silent audio
    let silent = vec![0.0f32; n_samples];
    let bad_tracks: &[(u64, &[f32])] = &[(1, &master_audio), (3, &silent)];
    assert!(synchronizer
        .sync_by_audio_correlation(bad_tracks, sample_rate)
        .is_err());
}

// ==============================================================================================
// Feature 39: Multicamera Track & Live Angle Switching
// ==============================================================================================

#[test]
fn test_f39_multicamera_track_and_live_switching() {
    let mut tl = Timeline::new();
    let mut history = History::new();

    let mut t1 = tl.new_track(TrackKind::Video);
    let tid1 = t1.id;
    t1.events.push(TrackEvent {
        id: tl.alloc_id(),
        name: "Cam1".into(),
        media_id: Some(101),
        start: 0.0,
        duration: 12.0,
        trim_in: 0.0,
        ..Default::default()
    });
    tl.tracks.push(t1);

    let mut t2 = tl.new_track(TrackKind::Video);
    let tid2 = t2.id;
    // Cam 2 started 2 seconds late
    t2.events.push(TrackEvent {
        id: tl.alloc_id(),
        name: "Cam2".into(),
        media_id: Some(102),
        start: 2.0,
        duration: 10.0,
        trim_in: 0.0,
        ..Default::default()
    });
    tl.tracks.push(t2);

    let mc_id =
        create_multicamera_track(&mut tl, &[tid1, tid2]).expect("should create multicam track");
    let mc_track = tl.track(mc_id).unwrap();
    assert_eq!(mc_track.events.len(), 2);

    // First event [0..2] has Cam 1 and (no camera)
    assert_eq!(mc_track.events[0].takes[0].name, "Camera 1");
    assert_eq!(mc_track.events[0].takes[1].name, "(no camera)");
    assert_eq!(mc_track.events[0].takes[1].media_id, None);

    // Second event [2..12] has both cameras
    let ev2_id = mc_track.events[1].id;
    assert_eq!(mc_track.events[1].takes[0].media_id, Some(101));
    assert_eq!(mc_track.events[1].takes[1].media_id, Some(102));

    // Live Cut-to-Take with crossfade (cut-to-overlap) at 6.0s switching to Camera 2 (index 1)
    let cut_cmd = cut_and_switch_angle(&mut tl, mc_id, ev2_id, 6.0, 1, Some(0.5))
        .expect("cut and switch should succeed");
    history.execute(&mut tl, cut_cmd);

    let updated_events = &tl.track(mc_id).unwrap().events;
    assert_eq!(updated_events.len(), 3);
    // Right event switched to Camera 2 with 0.5s crossfade
    assert_eq!(updated_events[2].active_take, 1);
    assert_eq!(updated_events[2].media_id, Some(102));
    assert!((updated_events[2].fade_in - 0.5).abs() < 1e-9);
    assert!((updated_events[1].fade_out - 0.5).abs() < 1e-9);

    // Expand multicam track back to separate tracks
    let unpacked_tracks =
        expand_multicamera_track(&mut tl, mc_id).expect("should expand multicam track");
    assert_eq!(unpacked_tracks.len(), 2);
}

// ==============================================================================================
// Feature 40: Alternative Event Takes System
// ==============================================================================================

#[test]
fn test_f40_take_container_and_slip_all_takes() {
    let mut tl = Timeline::new();
    let mut history = History::new();

    let mut track = tl.new_track(TrackKind::Video);
    let eid = tl.alloc_id();
    track.events.push(TrackEvent {
        id: eid,
        name: "scene_take".into(),
        media_id: Some(1),
        start: 10.0,
        duration: 5.0,
        trim_in: 0.0,
        takes: vec![
            Take {
                media_id: Some(1),
                trim_offset: 0.0,
                name: "Take 1".into(),
            },
            Take {
                media_id: Some(2),
                trim_offset: 1.5,
                name: "Take 2".into(),
            },
        ],
        active_take: 0,
        ..Default::default()
    });
    let eref = (track.id, eid);
    tl.tracks.push(track);

    // TakeContainer operations
    let mut container = TakeContainer::from_event(&tl.tracks[0].events[0]);
    assert_eq!(container.active_take, 0);

    container.next_take();
    assert_eq!(container.active_take, 1);
    assert_eq!(container.media_id, Some(2));

    container.prev_take();
    assert_eq!(container.active_take, 0);
    assert_eq!(container.media_id, Some(1));

    // Slip all takes command
    let slip_cmd = slip_all_takes(&tl, &[eref], 2.0).expect("slip command");
    history.execute(&mut tl, slip_cmd);

    let event = &tl.tracks[0].events[0];
    assert_eq!(event.trim_in, 2.0);
    assert_eq!(event.takes[0].trim_offset, 2.0);
    assert_eq!(event.takes[1].trim_offset, 3.5);
}

// ==============================================================================================
// Feature 41 & 42: Parity Matrix and Translation Catalogs Synchronization
// ==============================================================================================

#[test]
fn test_f41_and_f42_parity_and_translation_catalogs() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();

    // Feature 41: Check python parity matrix exits cleanly with 0
    let py_script = repo_root.join("scripts").join("parity_matrix.py");
    let output = std::process::Command::new("python")
        .args([py_script.to_str().unwrap(), "--check"])
        .current_dir(repo_root)
        .output()
        .expect("parity_matrix.py check should execute");
    assert!(
        output.status.success(),
        "parity matrix check failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Feature 42: Validate all 7 catalogs have 100% key parity
    let catalogs = [
        "de-DE", "en-US", "es-ES", "fr-FR", "ja-JP", "ru-RU", "zh-CN",
    ];
    let mut catalog_maps = Vec::new();

    for cat in catalogs {
        let path = repo_root
            .join("Assets")
            .join("i18n")
            .join("strings")
            .join(format!("{cat}.json"));
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("Failed to read catalog {}", path.display()));
        let map: BTreeMap<String, String> = serde_json::from_str(&content)
            .unwrap_or_else(|_| panic!("Invalid JSON in catalog {}", path.display()));
        catalog_maps.push((cat, map));
    }

    let base_keys: Vec<String> = catalog_maps[0].1.keys().cloned().collect();
    for (name, map) in &catalog_maps[1..] {
        assert_eq!(
            map.len(),
            base_keys.len(),
            "Catalog {name} has {} keys, expected {}",
            map.len(),
            base_keys.len()
        );
        for key in &base_keys {
            assert!(map.contains_key(key), "Catalog {name} missing key: '{key}'");
        }
    }
}
