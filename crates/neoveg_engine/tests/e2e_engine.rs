//! End-to-End Opaque-Box Integration Tests: neoveg_engine
//!
//! Tiers covered:
//! - Tier 1: Feature Coverage (Primary behavior)
//! - Tier 2: Boundary & Corner Cases (Limits, clamping, edge conditions)
//!
//! Features covered:
//! - Feature 3: Track Arming & Disarming (R2.1)
//! - Feature 8: Transition Envelope & Progress Curve (R2.1)
//! - Feature 22 & 23: Pan/Crop & Auto Reframe Keyframing (R2.7)
//! - Feature 27: 23 Media Detail Columns (R2.5)
//! - Feature 28 & 29: Media Bins & Explorer Navigation (R2.5)
//! - Feature 30: Make Movie Wizard & Render Track (R2.6)
//! - Feature 34: Smart Proxies & Media Relinking (R2.9)
//! - Feature 35: L-Cut & J-Cut Editing & Keyboard Trimming (R2.11)
//! - Feature 36: Simultaneous Audio/Video Fades (Crossfades) (R2.11)
//! - Feature 37: Persistent Sync Links & Event Grouping (R2.11)
//! - Feature 39 & 40: Multicamera Track & Alternative Event Takes (R2.12)

use neoveg_common::pan_crop::{PanCrop, PanCropFrame, PanCropKey, MAX_SIZE, MIN_SIZE};
use neoveg_common::project::{MediaFormat, MediaRef, ProjectInfo};
use neoveg_engine::command::{EditCommand, History};
use neoveg_engine::crossfade::find_crossfades;
use neoveg_engine::details::{rows, to_tsv};
use neoveg_engine::envelope::{AutomationTarget, EnvelopePoint, EnvelopeShape};
use neoveg_engine::groups::{
    expand_to_groups, group_events, group_members, move_events, remove_from_groups, ungroup_events,
};
use neoveg_engine::keytrim::{edge_time, Edge, EdgeSide, TrimMode};
use neoveg_engine::pan_crop;
use neoveg_engine::project_manager::ProjectManager;
use neoveg_engine::relink::{find_exact, find_fuzzy};
use neoveg_engine::render_track::{describe, only_events, placement};
use neoveg_engine::takes::{active_index, add, count, delete_active, set_active, step, take_name};
use neoveg_engine::timeline::{EventId, Timeline, TrackEvent, TrackKind, TIME_EPSILON};
use neoveg_engine::track_arm::{armed_tracks, set as arm_track, target_track};
use neoveg_engine::transition_progress::{is_active, new_envelope, progress_at};

fn create_test_event(id: EventId, start: f64, duration: f64, media_id: Option<u64>) -> TrackEvent {
    TrackEvent {
        id,
        name: format!("Event_{}", id),
        start,
        duration,
        media_id,
        ..Default::default()
    }
}

// ============================================================================
// Feature 3: Track Arming & Disarming (R2.1)
// ============================================================================

#[test]
fn f03_t1_01_arm_audio_track() {
    let mut tl = Timeline::new();
    let a1 = tl.new_track(TrackKind::Audio);
    let a1_id = a1.id;
    tl.tracks.push(a1);

    let cmd = arm_track(&tl, a1_id, true).expect("Audio track must arm");
    let mut h = History::new();
    assert!(h.execute(&mut tl, cmd));
    assert_eq!(armed_tracks(&tl), vec![a1_id]);
    assert_eq!(target_track(&tl), Some(a1_id));
}

#[test]
fn f03_t1_02_disarm_audio_track() {
    let mut tl = Timeline::new();
    let mut a1 = tl.new_track(TrackKind::Audio);
    a1.armed = true;
    let a1_id = a1.id;
    tl.tracks.push(a1);

    let cmd = arm_track(&tl, a1_id, false).expect("Armed audio track must disarm");
    let mut h = History::new();
    assert!(h.execute(&mut tl, cmd));
    assert!(armed_tracks(&tl).is_empty());
    assert_eq!(target_track(&tl), None);
}

#[test]
fn f03_t1_03_video_track_cannot_be_armed() {
    let mut tl = Timeline::new();
    let v1 = tl.new_track(TrackKind::Video);
    let v1_id = v1.id;
    tl.tracks.push(v1);

    assert!(arm_track(&tl, v1_id, true).is_none(), "Video tracks cannot be armed for recording");
}

#[test]
fn f03_t1_04_target_track_selects_first_unlocked_armed_track() {
    let mut tl = Timeline::new();
    let mut a1 = tl.new_track(TrackKind::Audio);
    a1.armed = true;
    a1.locked = true; // locked!
    let mut a2 = tl.new_track(TrackKind::Audio);
    a2.armed = true;
    a2.locked = false; // unlocked
    let a1_id = a1.id;
    let a2_id = a2.id;
    tl.tracks.extend([a1, a2]);

    assert_eq!(target_track(&tl), Some(a2_id), "Target track must skip locked armed tracks");
    assert_eq!(armed_tracks(&tl), vec![a1_id, a2_id]);
}

#[test]
fn f03_t1_05_undo_redo_track_arming() {
    let mut tl = Timeline::new();
    let a1 = tl.new_track(TrackKind::Audio);
    let a1_id = a1.id;
    tl.tracks.push(a1);

    let cmd = arm_track(&tl, a1_id, true).unwrap();
    let mut h = History::new();
    h.execute(&mut tl, cmd);
    assert!(tl.track(a1_id).unwrap().armed);

    h.undo(&mut tl);
    assert!(!tl.track(a1_id).unwrap().armed);

    h.redo(&mut tl);
    assert!(tl.track(a1_id).unwrap().armed);
}

#[test]
fn f03_t2_01_arm_already_armed_track_is_noop() {
    let mut tl = Timeline::new();
    let mut a1 = tl.new_track(TrackKind::Audio);
    a1.armed = true;
    let a1_id = a1.id;
    tl.tracks.push(a1);

    assert!(arm_track(&tl, a1_id, true).is_none());
}

#[test]
fn f03_t2_02_disarm_already_disarmed_track_is_noop() {
    let mut tl = Timeline::new();
    let a1 = tl.new_track(TrackKind::Audio);
    let a1_id = a1.id;
    tl.tracks.push(a1);

    assert!(arm_track(&tl, a1_id, false).is_none());
}

#[test]
fn f03_t2_03_arm_nonexistent_track_returns_none() {
    let tl = Timeline::new();
    assert!(arm_track(&tl, 9999, true).is_none());
}

#[test]
fn f03_t2_04_target_track_when_all_armed_are_locked_is_none() {
    let mut tl = Timeline::new();
    let mut a1 = tl.new_track(TrackKind::Audio);
    a1.armed = true;
    a1.locked = true;
    tl.tracks.push(a1);

    assert_eq!(target_track(&tl), None);
}

#[test]
fn f03_t2_05_target_track_with_zero_armed_tracks_is_none() {
    let mut tl = Timeline::new();
    let a1 = tl.new_track(TrackKind::Audio);
    tl.tracks.push(a1);

    assert_eq!(target_track(&tl), None);
}

// ============================================================================
// Feature 8: Transition Envelope & Progress Curve (R2.1)
// ============================================================================

#[test]
fn f08_t1_01_create_transition_envelope_default_ramp() {
    let env = new_envelope(1.5);
    assert_eq!(env.target, AutomationTarget::EventTransitionProgress);
    assert_eq!(env.points.len(), 1);
    assert_eq!(env.points[0].time, 1.5);
    assert_eq!(env.points[0].value, 1.0);
}

#[test]
fn f08_t1_02_transition_envelope_is_active_check() {
    let mut env = new_envelope(1.0);
    assert!(is_active(Some(&env)));

    env.enabled = false;
    assert!(!is_active(Some(&env)));

    assert!(!is_active(None));
}

#[test]
fn f08_t1_03_progress_at_interpolates_curve() {
    let env = new_envelope(2.0);
    let p_start = progress_at(&env, 0.0);
    let p_mid = progress_at(&env, 1.0);
    let p_end = progress_at(&env, 2.0);

    assert!((p_start - 0.0).abs() < 1e-4);
    assert!((p_mid - 0.5).abs() < 1e-4);
    assert!((p_end - 1.0).abs() < 1e-4);
}

#[test]
fn f08_t1_04_progress_clamped_beyond_duration() {
    let env = new_envelope(1.0);
    let p_past = progress_at(&env, 5.0);
    assert_eq!(p_past, 1.0);
}

#[test]
fn f08_t1_05_multi_point_transition_envelope_shaping() {
    let mut env = new_envelope(2.0);
    env.points.insert(
        0,
        EnvelopePoint {
            time: 1.0,
            value: 0.2, // holds at 0.2
            shape: EnvelopeShape::Linear,
        },
    );
    let mid = progress_at(&env, 1.0);
    assert!((mid - 0.2).abs() < 1e-4);
}

#[test]
fn f08_t2_01_transition_envelope_zero_or_negative_ramp_defaults_to_one() {
    let env_zero = new_envelope(0.0);
    assert_eq!(env_zero.points[0].time, 1.0);

    let env_neg = new_envelope(-5.0);
    assert_eq!(env_neg.points[0].time, 1.0);
}

#[test]
fn f08_t2_02_transition_envelope_nan_time_handled() {
    let env = new_envelope(1.0);
    let res = progress_at(&env, f64::NAN);
    assert!(res.is_finite());
}

#[test]
fn f08_t2_03_progress_before_event_start_clamps_to_zero() {
    let env = new_envelope(1.0);
    let res = progress_at(&env, -2.0);
    assert_eq!(res, 0.0);
}

#[test]
fn f08_t2_04_empty_points_envelope_is_inactive() {
    let mut env = new_envelope(1.0);
    env.points.clear();
    assert!(!is_active(Some(&env)));
}

#[test]
fn f08_t2_05_envelope_extreme_progress_values_clamped() {
    let mut env = new_envelope(1.0);
    env.points[0].value = 50.0; // out of [0, 1] range
    let res = progress_at(&env, 1.0);
    assert_eq!(res, 1.0);
}

// ============================================================================
// Feature 22 & 23: Pan/Crop & Auto Reframe Keyframing (R2.7)
// ============================================================================

#[test]
fn f22_t1_01_pan_crop_full_frame_identity() {
    let frame = PanCropFrame::FULL;
    assert!(frame.is_full());
    assert_eq!(frame.x_center, 0.5);
    assert_eq!(frame.y_center, 0.5);
    assert_eq!(frame.width, 1.0);
    assert_eq!(frame.height, 1.0);
}

#[test]
fn f22_t1_02_pan_crop_vertical_reframe_aspect_ratio_9_16() {
    // 9:16 reframe inside 16:9 source has width = 9/16 * 9/16 or centered box
    let v_frame = PanCropFrame {
        x_center: 0.5,
        y_center: 0.5,
        width: 0.5625, // (9 / 16)
        height: 1.0,
        rotation_deg: 0.0,
    };
    let clean = v_frame.sanitized();
    assert_eq!(clean.width, 0.5625);
    assert_eq!(clean.height, 1.0);
    assert_eq!(clean.x_center, 0.5);
}

#[test]
fn f22_t1_03_pan_crop_keyframing_and_interpolation() {
    let mut pc = PanCrop::default();
    pc.keys.push(PanCropKey {
        time: 0.0,
        frame: PanCropFrame {
            x_center: 0.2,
            ..PanCropFrame::FULL
        },
        smoothness: 0.0,
    });
    pc.keys.push(PanCropKey {
        time: 2.0,
        frame: PanCropFrame {
            x_center: 0.8,
            ..PanCropFrame::FULL
        },
        smoothness: 0.0,
    });

    let mid = pc.frame_at(1.0);
    assert!((mid.x_center - 0.5).abs() < 1e-4);
}

#[test]
fn f22_t1_04_pan_crop_timeline_get_and_set() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    let ev = create_test_event(10, 0.0, 5.0, Some(1));
    v.events.push(ev);
    let v_id = v.id;
    tl.tracks.push(v);

    let current = pan_crop::get(&tl, v_id, 10).unwrap();
    assert!(current.is_default());

    let mut new_pc = PanCrop::default();
    new_pc.stretch_to_fill = false;
    let cmd = pan_crop::set(&tl, v_id, 10, new_pc).unwrap();
    let mut h = History::new();
    assert!(h.execute(&mut tl, cmd));

    assert!(!pan_crop::get(&tl, v_id, 10).unwrap().stretch_to_fill);
}

#[test]
fn f22_t1_05_pan_crop_audio_track_returns_none() {
    let mut tl = Timeline::new();
    let mut a = tl.new_track(TrackKind::Audio);
    let ev = create_test_event(10, 0.0, 5.0, Some(1));
    a.events.push(ev);
    let a_id = a.id;
    tl.tracks.push(a);

    assert!(pan_crop::get(&tl, a_id, 10).is_none());
}

#[test]
fn f22_t2_01_pan_crop_size_clamping_to_min_and_max() {
    let frame_too_small = PanCropFrame {
        width: 0.0001,
        height: 0.0001,
        ..PanCropFrame::FULL
    };
    assert_eq!(frame_too_small.sanitized().width, MIN_SIZE);

    let frame_too_large = PanCropFrame {
        width: 100.0,
        height: 100.0,
        ..PanCropFrame::FULL
    };
    assert_eq!(frame_too_large.sanitized().width, MAX_SIZE);
}

#[test]
fn f22_t2_02_pan_crop_rotation_normalization() {
    let frame = PanCropFrame {
        rotation_deg: 450.0, // 450 % 360 = 90
        ..PanCropFrame::FULL
    };
    assert_eq!(frame.sanitized().rotation_deg, 90.0);
}

#[test]
fn f22_t2_03_pan_crop_nan_coordinates_fallback_to_center() {
    let frame = PanCropFrame {
        x_center: f32::NAN,
        y_center: f32::NAN,
        ..PanCropFrame::FULL
    };
    let clean = frame.sanitized();
    assert_eq!(clean.x_center, 0.5);
    assert_eq!(clean.y_center, 0.5);
}

#[test]
fn f22_t2_04_pan_crop_frame_at_before_and_after_keys() {
    let mut pc = PanCrop::default();
    pc.keys.push(PanCropKey {
        time: 1.0,
        frame: PanCropFrame {
            x_center: 0.3,
            ..PanCropFrame::FULL
        },
        smoothness: 0.0,
    });
    pc.keys.push(PanCropKey {
        time: 3.0,
        frame: PanCropFrame {
            x_center: 0.7,
            ..PanCropFrame::FULL
        },
        smoothness: 0.0,
    });

    assert_eq!(pc.frame_at(0.0).x_center, 0.3);
    assert_eq!(pc.frame_at(5.0).x_center, 0.7);
}

#[test]
fn f22_t2_05_pan_crop_set_identical_is_noop() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    let ev = create_test_event(10, 0.0, 5.0, Some(1));
    v.events.push(ev);
    let v_id = v.id;
    tl.tracks.push(v);

    assert!(pan_crop::set(&tl, v_id, 10, PanCrop::default()).is_none());
}

// ============================================================================
// Feature 27: 23 Media Detail Columns (R2.5)
// ============================================================================

#[test]
fn f27_t1_01_detail_rows_ordering_by_start_time() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    v.name = "Video Track".into();
    v.events.push(create_test_event(1, 10.0, 2.0, Some(1)));
    v.events.push(create_test_event(2, 2.0, 3.0, Some(1)));
    tl.tracks.push(v);

    let detail = rows(&tl);
    assert_eq!(detail.len(), 2);
    assert_eq!(detail[0].event.1, 2, "Earlier event must come first");
    assert_eq!(detail[1].event.1, 1);
}

#[test]
fn f27_t1_02_detail_row_geometry_and_times() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    v.name = "Video".into();
    let mut ev = create_test_event(1, 5.0, 4.0, Some(1));
    ev.trim_in = 1.5;
    v.events.push(ev);
    tl.tracks.push(v);

    let detail = rows(&tl);
    assert_eq!(detail[0].start, 5.0);
    assert_eq!(detail[0].duration, 4.0);
    assert_eq!(detail[0].end, 9.0);
    assert_eq!(detail[0].trim_in, 1.5);
}

#[test]
fn f27_t1_03_detail_row_switches_flags() {
    let mut tl = Timeline::new();
    let mut a = tl.new_track(TrackKind::Audio);
    let mut ev = create_test_event(1, 0.0, 1.0, Some(1));
    ev.muted = true;
    ev.locked = true;
    a.events.push(ev);
    tl.tracks.push(a);

    let detail = rows(&tl);
    assert!(detail[0].flags.contains('M'), "Must flag Mute");
    assert!(detail[0].flags.contains('B'), "Must flag Lock (Bloqueio)");
}

#[test]
fn f27_t1_04_detail_rows_tsv_export() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    v.name = "Main".into();
    v.events.push(create_test_event(1, 0.0, 5.0, Some(1)));
    tl.tracks.push(v);

    let detail = rows(&tl);
    let tsv = to_tsv(&detail, 30.0, false);
    assert!(tsv.starts_with("N\tFaixa\tTipo\tNome\tInicio"));
    assert!(tsv.contains("Main"));
    assert!(tsv.contains("video"));
}

#[test]
fn f27_t1_05_detail_rows_multi_track_representation() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    v.events.push(create_test_event(1, 0.0, 2.0, Some(1)));
    let mut a = tl.new_track(TrackKind::Audio);
    a.events.push(create_test_event(2, 0.0, 2.0, Some(2)));
    tl.tracks.extend([v, a]);

    let detail = rows(&tl);
    assert_eq!(detail.len(), 2);
    assert_eq!(detail[0].kind, TrackKind::Video);
    assert_eq!(detail[1].kind, TrackKind::Audio);
}

#[test]
fn f27_t2_01_detail_rows_empty_timeline_returns_empty_vec() {
    let tl = Timeline::new();
    assert!(rows(&tl).is_empty());
}

#[test]
fn f27_t2_02_detail_rows_track_without_events() {
    let mut tl = Timeline::new();
    let trk = tl.new_track(TrackKind::Video);
    tl.tracks.push(trk);
    assert!(rows(&tl).is_empty());
}

#[test]
fn f27_t2_03_detail_rows_sanitizes_newlines_in_name() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    let mut ev = create_test_event(1, 0.0, 1.0, Some(1));
    ev.name = "Line1\nLine2\tTabbed".into();
    v.events.push(ev);
    tl.tracks.push(v);

    let detail = rows(&tl);
    let tsv = to_tsv(&detail, 30.0, false);
    assert!(!tsv.contains("Line1\nLine2"));
    assert!(tsv.contains("Line1 Line2 Tabbed"));
}

#[test]
fn f27_t2_04_detail_rows_equal_start_times_secondary_sort() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    v.name = "Track 1".into();
    v.events.push(create_test_event(2, 1.0, 2.0, Some(1)));
    v.events.push(create_test_event(1, 1.0, 2.0, Some(1)));
    tl.tracks.push(v);

    let detail = rows(&tl);
    assert_eq!(detail.len(), 2);
}

#[test]
fn f27_t2_05_detail_rows_zero_playback_rate_handled() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    let mut ev = create_test_event(1, 0.0, 1.0, Some(1));
    ev.playback_rate = 0.0;
    v.events.push(ev);
    tl.tracks.push(v);

    let detail = rows(&tl);
    assert_eq!(detail[0].playback_rate, 0.0);
}

// ============================================================================
// Feature 28 & 29: Media Bins & Explorer Navigation (R2.5)
// ============================================================================

#[test]
fn f28_t1_01_project_manager_new_project_clean_state() {
    let mut pm = ProjectManager::new();
    let pid = pm.register(ProjectInfo::create_empty(), Timeline::new(), None);
    let p = pm.get(pid).expect("New project exists");
    assert_eq!(p.id, pid);
    assert!(!p.is_dirty());
    assert!(p.timeline.tracks.is_empty());
}

#[test]
fn f28_t1_02_project_manager_dirty_state_tracking() {
    let mut pm = ProjectManager::new();
    let pid = pm.register(ProjectInfo::create_empty(), Timeline::new(), None);
    let mut p = pm.take(pid).unwrap();
    let t = p.timeline.new_track(TrackKind::Video);
    p.history.execute(&mut p.timeline, EditCommand::AddTrack { track: t });
    assert!(p.is_dirty());
    pm.put(p);
    assert!(pm.get(pid).unwrap().is_dirty());
}

#[test]
fn f28_t1_03_project_manager_close_project() {
    let mut pm = ProjectManager::new();
    let pid = pm.register(ProjectInfo::create_empty(), Timeline::new(), None);
    assert!(pm.close_project(pid).is_some());
    assert!(pm.get(pid).is_none());
}

#[test]
fn f28_t1_04_project_title_indicates_dirty_state() {
    let mut pm = ProjectManager::new();
    let pid = pm.register(ProjectInfo::create_empty(), Timeline::new(), None);
    let mut p = pm.take(pid).unwrap();
    p.info.name = "MyProject".into();
    assert_eq!(p.title(), "MyProject");

    let t = p.timeline.new_track(TrackKind::Video);
    p.history.execute(&mut p.timeline, EditCommand::AddTrack { track: t });
    assert_eq!(p.title(), "MyProject *");
}

#[test]
fn f28_t1_05_project_manager_active_project_switching() {
    let mut pm = ProjectManager::new();
    let p1 = pm.register(ProjectInfo::create_empty(), Timeline::new(), None);
    let p2 = pm.register(ProjectInfo::create_empty(), Timeline::new(), None);
    assert_eq!(pm.active_project_id(), Some(p1));

    assert!(pm.switch_active_project(p2));
    assert_eq!(pm.active_project_id(), Some(p2));
}

#[test]
fn f28_t2_01_close_nonexistent_project_returns_false() {
    let mut pm = ProjectManager::new();
    assert!(pm.close_project(9999).is_none());
}

#[test]
fn f28_t2_02_take_nonexistent_project_returns_none() {
    let mut pm = ProjectManager::new();
    assert!(pm.take(9999).is_none());
}

#[test]
fn f28_t2_03_set_active_nonexistent_project_returns_false() {
    let mut pm = ProjectManager::new();
    assert!(!pm.switch_active_project(9999));
}

#[test]
fn f28_t2_04_project_manager_active_after_closing_active() {
    let mut pm = ProjectManager::new();
    let p1 = pm.register(ProjectInfo::create_empty(), Timeline::new(), None);
    let p2 = pm.register(ProjectInfo::create_empty(), Timeline::new(), None);
    pm.switch_active_project(p2);
    pm.close_project(p2);
    assert_eq!(pm.active_project_id(), Some(p1));
}

#[test]
fn f28_t2_05_project_manager_close_all_leaves_no_active() {
    let mut pm = ProjectManager::new();
    let p1 = pm.register(ProjectInfo::create_empty(), Timeline::new(), None);
    pm.close_project(p1);
    assert_eq!(pm.active_project_id(), None);
}

// ============================================================================
// Feature 30: Make Movie Wizard & Render Track (R2.6)
// ============================================================================

#[test]
fn f30_t1_01_describe_selection_span_and_tracks() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    let mut a = tl.new_track(TrackKind::Audio);
    v.events.push(create_test_event(1, 2.0, 3.0, Some(1)));
    a.events.push(create_test_event(2, 1.0, 6.0, Some(2)));
    let v_id = v.id;
    let a_id = a.id;
    tl.tracks.extend([v, a]);

    let desc = describe(&tl, &[(v_id, 1), (a_id, 2)]).expect("Selection must describe");
    assert_eq!(desc.span, (1.0, 7.0));
    assert!(desc.has_video);
    assert!(desc.has_audio);
    assert_eq!(desc.video_track, Some(v_id));
    assert_eq!(desc.audio_track, Some(a_id));
}

#[test]
fn f30_t1_02_only_events_filters_timeline_to_selection() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    v.events.push(create_test_event(1, 0.0, 2.0, Some(1)));
    v.events.push(create_test_event(2, 5.0, 2.0, Some(1)));
    let v_id = v.id;
    tl.tracks.push(v);

    let filtered = only_events(&tl, &[(v_id, 1)]);
    let track = filtered.track(v_id).unwrap();
    assert_eq!(track.events.len(), 1);
    assert_eq!(track.events[0].id, 1);
}

#[test]
fn f30_t1_03_placement_add_rendered_event() {
    let mut tl = Timeline::new();
    let v = tl.new_track(TrackKind::Video);
    let v_id = v.id;
    tl.tracks.push(v);

    let rendered = create_test_event(100, 0.0, 10.0, Some(99));
    let cmd = placement(&tl, &[], vec![(v_id, rendered)], false).unwrap();
    let mut h = History::new();
    assert!(h.execute(&mut tl, cmd));

    assert_eq!(tl.track(v_id).unwrap().events.len(), 1);
    assert_eq!(tl.track(v_id).unwrap().events[0].id, 100);
}

#[test]
fn f30_t1_04_placement_replace_selection_with_render() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    v.events.push(create_test_event(1, 0.0, 5.0, Some(1)));
    let v_id = v.id;
    tl.tracks.push(v);

    let rendered = create_test_event(2, 0.0, 5.0, Some(99));
    let cmd = placement(&tl, &[(v_id, 1)], vec![(v_id, rendered)], true).unwrap();
    let mut h = History::new();
    assert!(h.execute(&mut tl, cmd));

    let track = tl.track(v_id).unwrap();
    assert_eq!(track.events.len(), 1);
    assert_eq!(track.events[0].id, 2);
}

#[test]
fn f30_t1_05_describe_empty_selection_returns_none() {
    let tl = Timeline::new();
    assert!(describe(&tl, &[]).is_none());
}

#[test]
fn f30_t2_01_describe_missing_event_returns_none() {
    let mut tl = Timeline::new();
    let v = tl.new_track(TrackKind::Video);
    let v_id = v.id;
    tl.tracks.push(v);

    assert!(describe(&tl, &[(v_id, 999)]).is_none());
}

#[test]
fn f30_t2_02_placement_empty_added_and_no_replace_is_none() {
    let tl = Timeline::new();
    assert!(placement(&tl, &[], vec![], false).is_none());
}

#[test]
fn f30_t2_03_describe_single_point_span_filtered_out() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    v.events.push(create_test_event(1, 2.0, 0.0001, Some(1)));
    let v_id = v.id;
    tl.tracks.push(v);

    assert!(describe(&tl, &[(v_id, 1)]).is_none(), "Span under 1ms is ignored");
}

#[test]
fn f30_t2_04_only_events_clears_captions() {
    let mut tl = Timeline::new();
    tl.captions.push(neoveg_engine::Caption {
        id: 1,
        start: 0.0,
        end: 2.0,
        text: "Subtitle".into(),
    });
    let filtered = only_events(&tl, &[]);
    assert!(filtered.captions.is_empty());
}

#[test]
fn f30_t2_05_placement_undo_redo_replaces_events_cleanly() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    v.events.push(create_test_event(1, 0.0, 5.0, Some(1)));
    let v_id = v.id;
    tl.tracks.push(v);

    let rendered = create_test_event(2, 0.0, 5.0, Some(99));
    let cmd = placement(&tl, &[(v_id, 1)], vec![(v_id, rendered)], true).unwrap();
    let mut h = History::new();
    h.execute(&mut tl, cmd);
    assert_eq!(tl.track(v_id).unwrap().events[0].id, 2);

    h.undo(&mut tl);
    assert_eq!(tl.track(v_id).unwrap().events[0].id, 1);

    h.redo(&mut tl);
    assert_eq!(tl.track(v_id).unwrap().events[0].id, 2);
}

// ============================================================================
// Feature 34: Smart Proxies & Media Relinking (R2.9)
// ============================================================================

#[test]
fn f34_t1_01_relink_find_exact_file_match() {
    let dir = std::env::temp_dir().join(format!("neoveg_test_relink_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let target = dir.join("clip_01.mp4");
    let _ = std::fs::write(&target, b"dummy content");

    let media = MediaRef {
        id: 1,
        path: "D:/lost_footage/clip_01.mp4".into(),
        duration_seconds: 10.0,
        subproject: false,
        format: None,
    };

    let found = find_exact(&media, &[dir.clone()]);
    assert_eq!(found, Some(target));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn f34_t1_02_relink_find_fuzzy_case_insensitive_match() {
    let dir = std::env::temp_dir().join(format!("neoveg_test_fuzzy_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let target = dir.join("CLIP-01_final.mp4");
    let _ = std::fs::write(&target, b"dummy");

    let media = MediaRef {
        id: 1,
        path: "clip01.mp4".into(),
        duration_seconds: 5.0,
        subproject: false,
        format: None,
    };

    let found = find_fuzzy(&media, &[dir.clone()]);
    assert!(found.is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn f34_t1_03_relink_empty_search_directories_returns_none() {
    let media = MediaRef {
        id: 1,
        path: "clip_01.mp4".into(),
        duration_seconds: 10.0,
        subproject: false,
        format: None,
    };
    assert_eq!(find_exact(&media, &[]), None);
    assert_eq!(find_fuzzy(&media, &[]), None);
}

#[test]
fn f34_t1_04_relink_requires_same_extension_for_fuzzy() {
    let dir = std::env::temp_dir().join(format!("neoveg_test_ext_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("clip01.wav"), b"audio"); // .wav instead of .mp4

    let media = MediaRef {
        id: 1,
        path: "clip01.mp4".into(),
        duration_seconds: 5.0,
        subproject: false,
        format: None,
    };

    let found = find_fuzzy(&media, &[dir.clone()]);
    assert_eq!(found, None, "Fuzzy match must reject mismatched file extension");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn f34_t1_05_media_ref_format_metadata_preservation() {
    let format = MediaFormat {
        audio_channels: 2,
        fps: 24.0,
    };
    let media = MediaRef {
        id: 10,
        path: "interview.mov".into(),
        duration_seconds: 120.0,
        subproject: false,
        format: Some(format),
    };
    assert_eq!(media.format.as_ref().unwrap().audio_channels, 2);
    assert_eq!(media.format.as_ref().unwrap().fps, 24.0);
}

#[test]
fn f34_t2_01_relink_nonexistent_search_dir_skipped_safely() {
    let media = MediaRef {
        id: 1,
        path: "clip.mp4".into(),
        duration_seconds: 10.0,
        subproject: false,
        format: None,
    };
    let non_existent = std::path::PathBuf::from("Z:/non_existent_folder_abc123");
    assert_eq!(find_exact(&media, &[non_existent]), None);
}

#[test]
fn f34_t2_02_relink_empty_path_returns_none() {
    let media = MediaRef {
        id: 1,
        path: "".into(),
        duration_seconds: 0.0,
        subproject: false,
        format: None,
    };
    assert_eq!(find_fuzzy(&media, &[]), None);
}

#[test]
fn f34_t2_03_relink_duplicate_search_directories_deduplicated() {
    let dir = std::env::temp_dir().join(format!("neoveg_test_dedup_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let target = dir.join("test.mp4");
    let _ = std::fs::write(&target, b"video");

    let media = MediaRef {
        id: 1,
        path: "test.mp4".into(),
        duration_seconds: 1.0,
        subproject: false,
        format: None,
    };

    let found = find_exact(&media, &[dir.clone(), dir.clone()]);
    assert_eq!(found, Some(target));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn f34_t2_04_relink_subproject_media_ref() {
    let media = MediaRef {
        id: 99,
        path: "subproject.neoveg".into(),
        duration_seconds: 60.0,
        subproject: true,
        format: None,
    };
    assert!(media.subproject);
}

#[test]
fn f34_t2_05_relink_case_insensitive_exact_match() {
    let dir = std::env::temp_dir().join(format!("neoveg_test_case_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let target = dir.join("CLIP.MP4");
    let _ = std::fs::write(&target, b"data");

    let media = MediaRef {
        id: 1,
        path: "clip.mp4".into(),
        duration_seconds: 1.0,
        subproject: false,
        format: None,
    };

    let found = find_exact(&media, &[dir.clone()]);
    assert_eq!(found, Some(target));
    let _ = std::fs::remove_dir_all(&dir);
}

// ============================================================================
// Feature 35: L-Cut & J-Cut Editing & Keyboard Trimming (R2.11)
// ============================================================================

#[test]
fn f35_t1_01_l_cut_independent_audio_video_split() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    let mut a = tl.new_track(TrackKind::Audio);

    // L-Cut: Video cuts at 5.0, but Audio continues until 8.0
    let v_ev = create_test_event(1, 0.0, 5.0, Some(1));
    let a_ev = create_test_event(2, 0.0, 8.0, Some(1));
    v.events.push(v_ev);
    a.events.push(a_ev);
    let v_id = v.id;
    let a_id = a.id;
    tl.tracks.extend([v, a]);

    let edge_v = Edge {
        track: v_id,
        event: 1,
        side: EdgeSide::End,
    };
    let edge_a = Edge {
        track: a_id,
        event: 2,
        side: EdgeSide::End,
    };

    assert_eq!(edge_time(&tl, edge_v), Some(5.0));
    assert_eq!(edge_time(&tl, edge_a), Some(8.0));
    assert_eq!(edge_time(&tl, edge_a).unwrap() - edge_time(&tl, edge_v).unwrap(), 3.0, "Audio extends 3s past video");
}

#[test]
fn f35_t1_02_j_cut_audio_leads_video_in() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    let mut a = tl.new_track(TrackKind::Audio);

    // J-Cut: Audio begins at 2.0, Video arrives later at 4.0
    let v_ev = create_test_event(1, 4.0, 6.0, Some(1));
    let a_ev = create_test_event(2, 2.0, 8.0, Some(1));
    v.events.push(v_ev);
    a.events.push(a_ev);
    let v_id = v.id;
    let a_id = a.id;
    tl.tracks.extend([v, a]);

    let edge_v = Edge {
        track: v_id,
        event: 1,
        side: EdgeSide::Start,
    };
    let edge_a = Edge {
        track: a_id,
        event: 2,
        side: EdgeSide::Start,
    };

    assert_eq!(edge_time(&tl, edge_a), Some(2.0));
    assert_eq!(edge_time(&tl, edge_v), Some(4.0));
    assert_eq!(edge_time(&tl, edge_v).unwrap() - edge_time(&tl, edge_a).unwrap(), 2.0, "Audio leads video by 2s");
}

#[test]
fn f35_t1_03_edge_time_start_and_end() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    v.events.push(create_test_event(1, 10.0, 4.0, Some(1)));
    let v_id = v.id;
    tl.tracks.push(v);

    let start_edge = Edge {
        track: v_id,
        event: 1,
        side: EdgeSide::Start,
    };
    let end_edge = Edge {
        track: v_id,
        event: 1,
        side: EdgeSide::End,
    };

    assert_eq!(edge_time(&tl, start_edge), Some(10.0));
    assert_eq!(edge_time(&tl, end_edge), Some(14.0));
}

#[test]
fn f35_t1_04_trim_modes_default() {
    assert_eq!(TrimMode::default(), TrimMode::Normal);
}

#[test]
fn f35_t1_05_missing_edge_event_returns_none() {
    let tl = Timeline::new();
    let edge = Edge {
        track: 1,
        event: 999,
        side: EdgeSide::Start,
    };
    assert_eq!(edge_time(&tl, edge), None);
}

#[test]
fn f35_t2_01_edge_time_zero_duration_event() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    v.events.push(create_test_event(1, 5.0, 0.0, Some(1)));
    let v_id = v.id;
    tl.tracks.push(v);

    let end_edge = Edge {
        track: v_id,
        event: 1,
        side: EdgeSide::End,
    };
    assert_eq!(edge_time(&tl, end_edge), Some(5.0));
}

#[test]
fn f35_t2_02_edge_time_missing_track_returns_none() {
    let tl = Timeline::new();
    let edge = Edge {
        track: 999,
        event: 1,
        side: EdgeSide::Start,
    };
    assert_eq!(edge_time(&tl, edge), None);
}

#[test]
fn f35_t2_03_trim_modes_distinct_variants() {
    let modes = [
        TrimMode::Normal,
        TrimMode::Slip,
        TrimMode::Slide,
        TrimMode::Stretch,
        TrimMode::Roll,
    ];
    for i in 0..modes.len() {
        for j in i + 1..modes.len() {
            assert_ne!(modes[i], modes[j]);
        }
    }
}

#[test]
fn f35_t2_04_l_cut_offset_trim_in_consistency() {
    let mut ev = create_test_event(1, 0.0, 10.0, Some(1));
    ev.trim_in = 5.0;
    assert_eq!(ev.trim_in, 5.0);
    assert_eq!(ev.end(), 10.0);
}

#[test]
fn f35_t2_05_j_cut_zero_start_boundary() {
    let ev = create_test_event(1, 0.0, 5.0, Some(1));
    assert_eq!(ev.start, 0.0);
}

// ============================================================================
// Feature 36: Simultaneous Audio/Video Fades (Crossfades) (R2.11)
// ============================================================================

#[test]
fn f36_t1_01_find_crossfades_detects_overlapping_events() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    // Event 1: 0.0 to 5.0
    // Event 2: 4.0 to 9.0 (overlaps by 1.0s from 4.0 to 5.0)
    v.events.push(create_test_event(1, 0.0, 5.0, Some(1)));
    v.events.push(create_test_event(2, 4.0, 5.0, Some(2)));

    let crossfades = find_crossfades(&v.events);
    assert_eq!(crossfades.len(), 1);
    assert_eq!(crossfades[0].earlier, 1);
    assert_eq!(crossfades[0].later, 2);
    assert_eq!(crossfades[0].start, 4.0);
    assert_eq!(crossfades[0].end, 5.0);
    assert!((crossfades[0].length() - 1.0).abs() < 1e-4);
}

#[test]
fn f36_t1_02_no_crossfade_for_abutting_events() {
    let ev1 = create_test_event(1, 0.0, 4.0, Some(1));
    let ev2 = create_test_event(2, 4.0, 4.0, Some(2));
    assert!(find_crossfades(&[ev1, ev2]).is_empty());
}

#[test]
fn f36_t1_03_no_crossfade_when_event_is_fully_contained() {
    let outer = create_test_event(1, 0.0, 10.0, Some(1));
    let inner = create_test_event(2, 2.0, 3.0, Some(2));
    assert!(find_crossfades(&[outer, inner]).is_empty());
}

#[test]
fn f36_t1_04_multiple_consecutive_crossfades() {
    let e1 = create_test_event(1, 0.0, 5.0, Some(1));
    let e2 = create_test_event(2, 4.0, 5.0, Some(2));
    let e3 = create_test_event(3, 8.0, 4.0, Some(3));

    let cfs = find_crossfades(&[e1, e2, e3]);
    assert_eq!(cfs.len(), 2);
    assert_eq!(cfs[0].earlier, 1);
    assert_eq!(cfs[0].later, 2);
    assert_eq!(cfs[1].earlier, 2);
    assert_eq!(cfs[1].later, 3);
}

#[test]
fn f36_t1_05_simultaneous_audio_and_video_crossfades() {
    let v1 = create_test_event(1, 0.0, 6.0, Some(1));
    let v2 = create_test_event(2, 5.0, 5.0, Some(2));
    let a1 = create_test_event(3, 0.0, 6.0, Some(1));
    let a2 = create_test_event(4, 5.0, 5.0, Some(2));

    let v_cf = find_crossfades(&[v1, v2]);
    let a_cf = find_crossfades(&[a1, a2]);

    assert_eq!(v_cf.len(), 1);
    assert_eq!(a_cf.len(), 1);
    assert_eq!(v_cf[0].start, a_cf[0].start);
    assert_eq!(v_cf[0].end, a_cf[0].end);
}

#[test]
fn f36_t2_01_empty_events_list_has_zero_crossfades() {
    assert!(find_crossfades(&[]).is_empty());
}

#[test]
fn f36_t2_02_single_event_has_zero_crossfades() {
    let e = create_test_event(1, 0.0, 5.0, Some(1));
    assert!(find_crossfades(&[e]).is_empty());
}

#[test]
fn f36_t2_03_zero_overlap_touching_events_boundary() {
    let e1 = create_test_event(1, 0.0, 2.0, Some(1));
    let e2 = create_test_event(2, 2.0 + TIME_EPSILON / 2.0, 2.0, Some(2));
    assert!(find_crossfades(&[e1, e2]).is_empty());
}

#[test]
fn f36_t2_04_identical_start_and_end_is_not_crossfade() {
    let e1 = create_test_event(1, 1.0, 4.0, Some(1));
    let e2 = create_test_event(2, 1.0, 4.0, Some(2));
    assert!(find_crossfades(&[e1, e2]).is_empty());
}

#[test]
fn f36_t2_05_crossfade_sort_order_chronological() {
    let e3 = create_test_event(3, 8.0, 4.0, Some(3));
    let e2 = create_test_event(2, 4.0, 5.0, Some(2));
    let e1 = create_test_event(1, 0.0, 5.0, Some(1));

    // Input scrambled
    let cfs = find_crossfades(&[e3, e1, e2]);
    assert_eq!(cfs.len(), 2);
    assert!(cfs[0].start < cfs[1].start);
}

// ============================================================================
// Feature 37: Persistent Sync Links & Event Grouping (R2.11)
// ============================================================================

#[test]
fn f37_t1_01_group_events_assigns_shared_group_id() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    let mut a = tl.new_track(TrackKind::Audio);
    v.events.push(create_test_event(1, 0.0, 5.0, Some(1)));
    a.events.push(create_test_event(2, 0.0, 5.0, Some(1)));
    let v_id = v.id;
    let a_id = a.id;
    tl.tracks.extend([v, a]);

    let outcome = group_events(&mut tl, &[(v_id, 1), (a_id, 2)]).unwrap();
    let mut h = History::new();
    assert!(h.execute(&mut tl, outcome.command));

    let g1 = tl.track(v_id).unwrap().events[0].group;
    let g2 = tl.track(a_id).unwrap().events[0].group;
    assert!(g1.is_some());
    assert_eq!(g1, g2);
}

#[test]
fn f37_t1_02_expand_to_groups_selects_all_members() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    let mut a = tl.new_track(TrackKind::Audio);
    let mut e1 = create_test_event(1, 0.0, 5.0, Some(1));
    let mut e2 = create_test_event(2, 0.0, 5.0, Some(1));
    e1.group = Some(100);
    e2.group = Some(100);
    v.events.push(e1);
    a.events.push(e2);
    let v_id = v.id;
    let a_id = a.id;
    tl.tracks.extend([v, a]);

    let expanded = expand_to_groups(&tl, &[(v_id, 1)]);
    assert_eq!(expanded.len(), 2);
    assert!(expanded.contains(&(v_id, 1)));
    assert!(expanded.contains(&(a_id, 2)));
}

#[test]
fn f37_t1_03_move_events_shifts_all_members_together() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    let mut a = tl.new_track(TrackKind::Audio);
    let mut e1 = create_test_event(1, 0.0, 5.0, Some(1));
    let mut e2 = create_test_event(2, 0.0, 5.0, Some(1));
    e1.group = Some(100);
    e2.group = Some(100);
    v.events.push(e1);
    a.events.push(e2);
    let v_id = v.id;
    let a_id = a.id;
    tl.tracks.extend([v, a]);

    let outcome = move_events(&tl, &[(v_id, 1), (a_id, 2)], 3.0).unwrap();
    let mut h = History::new();
    assert!(h.execute(&mut tl, outcome.command));

    assert_eq!(tl.track(v_id).unwrap().events[0].start, 3.0);
    assert_eq!(tl.track(a_id).unwrap().events[0].start, 3.0);
}

#[test]
fn f37_t1_04_ungroup_events_clears_group_ids() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    let mut e1 = create_test_event(1, 0.0, 5.0, Some(1));
    e1.group = Some(100);
    v.events.push(e1);
    let v_id = v.id;
    tl.tracks.push(v);

    let outcome = ungroup_events(&tl, &[(v_id, 1)]).unwrap();
    let mut h = History::new();
    assert!(h.execute(&mut tl, outcome.command));

    assert_eq!(tl.track(v_id).unwrap().events[0].group, None);
}

#[test]
fn f37_t1_05_group_members_query() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    let mut a = tl.new_track(TrackKind::Audio);
    let mut e1 = create_test_event(1, 0.0, 5.0, Some(1));
    let mut e2 = create_test_event(2, 0.0, 5.0, Some(1));
    e1.group = Some(77);
    e2.group = Some(77);
    v.events.push(e1);
    a.events.push(e2);
    let v_id = v.id;
    let a_id = a.id;
    tl.tracks.extend([v, a]);

    let members = group_members(&tl, (v_id, 1));
    assert_eq!(members.len(), 2);
    assert!(members.contains(&(v_id, 1)));
    assert!(members.contains(&(a_id, 2)));
}

#[test]
fn f37_t2_01_group_single_event_is_none() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    v.events.push(create_test_event(1, 0.0, 5.0, Some(1)));
    let v_id = v.id;
    tl.tracks.push(v);

    assert!(group_events(&mut tl, &[(v_id, 1)]).is_none(), "Need >= 2 events to group");
}

#[test]
fn f37_t2_02_ungroup_already_ungrouped_is_none() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    v.events.push(create_test_event(1, 0.0, 5.0, Some(1)));
    let v_id = v.id;
    tl.tracks.push(v);

    assert!(ungroup_events(&tl, &[(v_id, 1)]).is_none());
}

#[test]
fn f37_t2_03_move_events_zero_delta_is_none() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    v.events.push(create_test_event(1, 0.0, 5.0, Some(1)));
    let v_id = v.id;
    tl.tracks.push(v);

    assert!(move_events(&tl, &[(v_id, 1)], 0.0).is_none());
}

#[test]
fn f37_t2_04_remove_from_groups_partial_member() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    let mut a = tl.new_track(TrackKind::Audio);
    let mut e1 = create_test_event(1, 0.0, 5.0, Some(1));
    let mut e2 = create_test_event(2, 0.0, 5.0, Some(1));
    e1.group = Some(50);
    e2.group = Some(50);
    v.events.push(e1);
    a.events.push(e2);
    let v_id = v.id;
    let a_id = a.id;
    tl.tracks.extend([v, a]);

    let outcome = remove_from_groups(&tl, &[(v_id, 1)]).unwrap();
    let mut h = History::new();
    assert!(h.execute(&mut tl, outcome.command));

    assert_eq!(tl.track(v_id).unwrap().events[0].group, None);
    assert_eq!(tl.track(a_id).unwrap().events[0].group, Some(50));
}

#[test]
fn f37_t2_05_move_events_locked_events_prevent_move() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    let mut e1 = create_test_event(1, 0.0, 5.0, Some(1));
    e1.locked = true;
    v.events.push(e1);
    let v_id = v.id;
    tl.tracks.push(v);

    assert!(move_events(&tl, &[(v_id, 1)], 2.0).is_none(), "Locked events cannot be moved");
}

// ============================================================================
// Feature 39 & 40: Multicamera Track & Alternative Event Takes (R2.12)
// ============================================================================

#[test]
fn f40_t1_01_event_starts_with_one_take() {
    let ev = create_test_event(1, 0.0, 5.0, Some(1));
    assert_eq!(count(&ev), 1);
    assert_eq!(active_index(&ev), 0);
}

#[test]
fn f40_t1_02_add_take_to_event() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    v.events.push(create_test_event(1, 0.0, 5.0, Some(1)));
    let v_id = v.id;
    tl.tracks.push(v);

    let cmd = add(&tl, &[(v_id, 1)], 200, "Take 2", 1.0).unwrap();
    let mut h = History::new();
    assert!(h.execute(&mut tl, cmd));

    let ev = &tl.track(v_id).unwrap().events[0];
    assert_eq!(count(ev), 2);
    assert_eq!(active_index(ev), 1);
    assert_eq!(ev.media_id, Some(200));
}

#[test]
fn f40_t1_03_step_takes_cycles_active_take() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    v.events.push(create_test_event(1, 0.0, 5.0, Some(1)));
    let v_id = v.id;
    tl.tracks.push(v);

    let mut h = History::new();
    h.execute(&mut tl, add(&tl, &[(v_id, 1)], 200, "Take 2", 0.0).unwrap());

    // Step forward (wrap around back to take 0)
    let step_cmd = step(&tl, &[(v_id, 1)], 1).unwrap();
    assert!(h.execute(&mut tl, step_cmd));

    let ev = &tl.track(v_id).unwrap().events[0];
    assert_eq!(active_index(ev), 0);
    assert_eq!(ev.media_id, Some(1));
}

#[test]
fn f40_t1_04_set_active_take_by_index() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    v.events.push(create_test_event(1, 0.0, 5.0, Some(1)));
    let v_id = v.id;
    tl.tracks.push(v);

    let mut h = History::new();
    h.execute(&mut tl, add(&tl, &[(v_id, 1)], 200, "Take 2", 0.0).unwrap());
    h.execute(&mut tl, add(&tl, &[(v_id, 1)], 300, "Take 3", 0.0).unwrap());

    // Switch directly to take 0
    let cmd = set_active(&tl, &[(v_id, 1)], 0).unwrap();
    assert!(h.execute(&mut tl, cmd));
    assert_eq!(active_index(&tl.track(v_id).unwrap().events[0]), 0);
}

#[test]
fn f40_t1_05_delete_active_take() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    v.events.push(create_test_event(1, 0.0, 5.0, Some(1)));
    let v_id = v.id;
    tl.tracks.push(v);

    let mut h = History::new();
    h.execute(&mut tl, add(&tl, &[(v_id, 1)], 200, "Take 2", 0.0).unwrap());

    let del_cmd = delete_active(&tl, &[(v_id, 1)]).unwrap();
    assert!(h.execute(&mut tl, del_cmd));

    let ev = &tl.track(v_id).unwrap().events[0];
    assert_eq!(count(ev), 1);
    assert_eq!(ev.media_id, Some(1));
}

#[test]
fn f40_t2_01_step_takes_with_single_take_is_noop() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    v.events.push(create_test_event(1, 0.0, 5.0, Some(1)));
    let v_id = v.id;
    tl.tracks.push(v);

    assert!(step(&tl, &[(v_id, 1)], 1).is_none());
}

#[test]
fn f40_t2_02_delete_active_take_on_single_take_event_is_noop() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    v.events.push(create_test_event(1, 0.0, 5.0, Some(1)));
    let v_id = v.id;
    tl.tracks.push(v);

    assert!(delete_active(&tl, &[(v_id, 1)]).is_none(), "Never delete the only take");
}

#[test]
fn f40_t2_03_set_active_take_out_of_bounds_is_noop() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    v.events.push(create_test_event(1, 0.0, 5.0, Some(1)));
    let v_id = v.id;
    tl.tracks.push(v);

    assert!(set_active(&tl, &[(v_id, 1)], 99).is_none());
}

#[test]
fn f40_t2_04_add_take_nan_trim_in_returns_none() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    v.events.push(create_test_event(1, 0.0, 5.0, Some(1)));
    let v_id = v.id;
    tl.tracks.push(v);

    assert!(add(&tl, &[(v_id, 1)], 200, "Nan Take", f64::NAN).is_none());
}

#[test]
fn f40_t2_05_take_name_defaults_to_formatted_index() {
    let mut ev = create_test_event(1, 0.0, 5.0, Some(1));
    assert_eq!(take_name(&ev, 0), "Event_1");
}
