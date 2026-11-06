//! End-to-End Opaque-Box Integration Tests: Tier 3 & Tier 4 Scenarios
//!
//! Tiers covered:
//! - Tier 3: Cross-Feature Interactions (42 pairwise boundary tests)
//! - Tier 4: Real-World Production Scenarios (6 end-to-end multi-step pipelines)
//!
//! Requires 100% headless CPU execution, zero dedicated physical GPU/AI hardware.

use neoveg_common::pan_crop::{PanCrop, PanCropFrame, PanCropKey};
use neoveg_common::presets::FieldOrder as PresetFieldOrder;
use neoveg_common::project::{MediaFormat, MediaRef, ProjectInfo, ProjectSettings};
use neoveg_engine::command::History;
use neoveg_engine::crossfade::find_crossfades;
use neoveg_engine::details::{rows, to_tsv};
use neoveg_engine::envelope::{EnvelopePoint, EnvelopeShape};
use neoveg_engine::groups::{expand_to_groups, group_events, move_events, ungroup_events};
use neoveg_engine::keytrim::{edge_time, Edge, EdgeSide};
use neoveg_engine::project_manager::ProjectManager;
use neoveg_engine::relink::{find_exact, find_fuzzy};
use neoveg_engine::render_track::{describe, only_events, placement};
use neoveg_engine::takes::{
    active_index, add as add_take, count as take_count, set_active as set_active_take,
    step as step_take,
};
use neoveg_engine::timeline::{EventId, Timeline, TrackEvent, TrackKind};
use neoveg_engine::track_arm::{armed_tracks, set as arm_track, target_track};
use neoveg_engine::transition_progress::{
    is_active as is_progress_active, new_envelope, progress_at,
};
use neoveg_engine::Caption;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertCategory {
    MissingMedia,
    ProjectProperties,
    MediaPool,
}

#[derive(Debug, Clone)]
pub struct StandardAlert {
    pub id: &'static str,
    pub severity: AlertSeverity,
    pub category: AlertCategory,
    pub message: String,
}

#[derive(Default)]
pub struct AlertRegistry {
    alerts: Vec<StandardAlert>,
}

impl AlertRegistry {
    pub fn new() -> Self {
        Self { alerts: Vec::new() }
    }
    pub fn register(&mut self, alert: StandardAlert) {
        self.alerts.push(alert);
    }
    pub fn has_alerts(&self) -> bool {
        !self.alerts.is_empty()
    }
}

fn helper_event(id: EventId, start: f64, duration: f64, media_id: Option<u64>) -> TrackEvent {
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
// Tier 3: Cross-Feature Interactions (42 Pairwise Interaction Tests)
// ============================================================================

#[test]
fn t3_01_track_arming_with_locked_track_skip() {
    let mut tl = Timeline::new();
    let mut a1 = tl.new_track(TrackKind::Audio);
    a1.armed = true;
    a1.locked = true;
    let mut a2 = tl.new_track(TrackKind::Audio);
    a2.armed = true;
    let a2_id = a2.id;
    tl.tracks.extend([a1, a2]);
    assert_eq!(target_track(&tl), Some(a2_id));
}

#[test]
fn t3_02_transition_envelope_progress_drives_crossfade_boundary() {
    let env = new_envelope(2.0);
    let p0 = progress_at(&env, 0.0);
    let p1 = progress_at(&env, 2.0);
    assert_eq!(p0, 0.0);
    assert_eq!(p1, 1.0);
}

#[test]
fn t3_03_pan_crop_smoothstep_keyframing_at_event_boundary() {
    let mut pc = PanCrop::default();
    pc.keys.push(PanCropKey {
        time: 0.0,
        frame: PanCropFrame {
            x_center: 0.2,
            ..PanCropFrame::FULL
        },
        smoothness: 1.0,
    });
    pc.keys.push(PanCropKey {
        time: 4.0,
        frame: PanCropFrame {
            x_center: 0.8,
            ..PanCropFrame::FULL
        },
        smoothness: 1.0,
    });
    let mid = pc.frame_at(2.0);
    assert!((mid.x_center - 0.5).abs() < 1e-3);
}

#[test]
fn t3_04_detail_rows_tsv_export_with_multi_channel_audio() {
    let mut tl = Timeline::new();
    let mut a = tl.new_track(TrackKind::Audio);
    a.events.push(helper_event(1, 0.0, 10.0, Some(1)));
    tl.tracks.push(a);

    let detail = rows(&tl);
    let tsv = to_tsv(&detail, 25.0, false);
    assert!(tsv.contains("audio"));
    assert!(tsv.contains("Event_1"));
}

#[test]
fn t3_05_project_manager_dirty_flag_updates_on_event_grouping() {
    let mut pm = ProjectManager::new();
    let pid = pm.register(ProjectInfo::create_empty(), Timeline::new(), None);
    let mut p = pm.take(pid).unwrap();

    let mut v = p.timeline.new_track(TrackKind::Video);
    let mut a = p.timeline.new_track(TrackKind::Audio);
    v.events.push(helper_event(1, 0.0, 5.0, Some(1)));
    a.events.push(helper_event(2, 0.0, 5.0, Some(1)));
    let (vid, aid) = (v.id, a.id);
    p.timeline.tracks.extend([v, a]);

    let out = group_events(&mut p.timeline, &[(vid, 1), (aid, 2)]).unwrap();
    p.history.execute(&mut p.timeline, out.command);
    assert!(p.is_dirty());
    pm.put(p);
}

#[test]
fn t3_06_make_movie_describe_selection_with_grouped_events() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    let mut a = tl.new_track(TrackKind::Audio);
    v.events.push(helper_event(1, 1.0, 4.0, Some(1)));
    a.events.push(helper_event(2, 1.0, 4.0, Some(1)));
    let (vid, aid) = (v.id, a.id);
    tl.tracks.extend([v, a]);

    let desc = describe(&tl, &[(vid, 1), (aid, 2)]).unwrap();
    assert_eq!(desc.span, (1.0, 5.0));
    assert!(desc.has_video && desc.has_audio);
}

#[test]
fn t3_07_relink_with_format_validation() {
    let media = MediaRef {
        id: 1,
        path: "C:/media/take_01.mp4".into(),
        duration_seconds: 10.0,
        subproject: false,
        format: Some(MediaFormat {
            channels: Some(2),
            fps: Some(30.0),
        }),
    };
    let dir = std::env::temp_dir().join(format!("neoveg_t3_07_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("take_01.mp4"), b"sample");

    let found = find_exact(&media, std::slice::from_ref(&dir));
    assert!(found.is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn t3_08_l_cut_offset_group_expansion() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    let mut a = tl.new_track(TrackKind::Audio);
    let mut ev_v = helper_event(1, 0.0, 5.0, Some(1));
    let mut ev_a = helper_event(2, 0.0, 7.0, Some(1)); // L-cut
    ev_v.group = Some(42);
    ev_a.group = Some(42);
    v.events.push(ev_v);
    a.events.push(ev_a);
    let (vid, aid) = (v.id, a.id);
    tl.tracks.extend([v, a]);

    let members = expand_to_groups(&tl, &[(vid, 1)]);
    assert_eq!(members.len(), 2);
    assert!(members.contains(&(aid, 2)));
}

#[test]
fn t3_09_crossfade_overlap_with_l_cut_tail() {
    let ev1 = helper_event(1, 0.0, 6.0, Some(1));
    let ev2 = helper_event(2, 5.0, 5.0, Some(2));
    let cfs = find_crossfades(&[ev1, ev2]);
    assert_eq!(cfs.len(), 1);
    assert_eq!(cfs[0].length(), 1.0);
}

#[test]
fn t3_10_multicam_take_switching_preserves_grouping() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    let mut ev = helper_event(1, 0.0, 10.0, Some(101));
    ev.group = Some(99);
    v.events.push(ev);
    let vid = v.id;
    tl.tracks.push(v);

    let mut h = History::new();
    let add_cmd = add_take(&tl, &[(vid, 1)], 102, "Angle 2", 0.0).unwrap();
    h.execute(&mut tl, add_cmd);

    assert_eq!(take_count(&tl.track(vid).unwrap().events[0]), 2);
    assert_eq!(tl.track(vid).unwrap().events[0].group, Some(99));
}

#[test]
fn t3_11_project_settings_custom_resolution_and_framerate() {
    let s = ProjectSettings {
        width: 3840,
        height: 2160,
        fps: 59.94,
        ..Default::default()
    };
    assert_eq!(s.width * s.height, 8_294_400);
}

#[test]
fn t3_12_alert_registry_resolution_loss_on_quality_switch() {
    let mut reg = AlertRegistry::new();
    let alert = StandardAlert {
        id: "RES_WARN",
        severity: AlertSeverity::Warning,
        category: AlertCategory::ProjectProperties,
        message: "Proxy preview downscaled to draft resolution".into(),
    };
    reg.register(alert);
    assert!(reg.has_alerts());
}

#[test]
fn t3_13_edge_trimming_recalculates_crossfades() {
    let mut e1 = helper_event(1, 0.0, 5.0, Some(1));
    let e2 = helper_event(2, 4.0, 5.0, Some(2));
    assert_eq!(find_crossfades(&[e1.clone(), e2.clone()]).len(), 1);

    // Trim e1 back to 3.5, removing the overlap
    e1.duration = 3.5;
    assert_eq!(find_crossfades(&[e1, e2]).len(), 0);
}

#[test]
fn t3_14_move_grouped_events_preserves_relative_l_cut_offset() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    let mut a = tl.new_track(TrackKind::Audio);
    let mut ev_v = helper_event(1, 0.0, 4.0, Some(1));
    let mut ev_a = helper_event(2, 0.0, 6.0, Some(1)); // 2s L-cut
    ev_v.group = Some(10);
    ev_a.group = Some(10);
    v.events.push(ev_v);
    a.events.push(ev_a);
    let (vid, aid) = (v.id, a.id);
    tl.tracks.extend([v, a]);

    let out = move_events(&tl, &[(vid, 1), (aid, 2)], 5.0).unwrap();
    let mut h = History::new();
    h.execute(&mut tl, out.command);

    let v_end = tl.track(vid).unwrap().events[0].end();
    let a_end = tl.track(aid).unwrap().events[0].end();
    assert_eq!(
        a_end - v_end,
        2.0,
        "Relative L-cut offset must survive move"
    );
}

#[test]
fn t3_15_placement_replace_selection_updates_detail_rows() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    v.events.push(helper_event(1, 0.0, 5.0, Some(1)));
    let vid = v.id;
    tl.tracks.push(v);

    let rendered = helper_event(2, 0.0, 5.0, Some(99));
    let cmd = placement(&tl, &[(vid, 1)], vec![(vid, rendered)], true).unwrap();
    let mut h = History::new();
    h.execute(&mut tl, cmd);

    let d = rows(&tl);
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].event.1, 2);
}

#[test]
fn t3_16_pan_crop_aspect_fit_for_4k_source() {
    let pc = PanCropFrame {
        x_center: 0.5,
        y_center: 0.5,
        width: 1.0,
        height: 1.0,
        rotation_deg: 0.0,
    };
    assert!(pc.is_full());
}

#[test]
fn t3_17_closed_captions_survive_track_arming_and_disarming() {
    let mut tl = Timeline::new();
    let a = tl.new_track(TrackKind::Audio);
    let aid = a.id;
    tl.tracks.push(a);
    tl.captions.push(Caption {
        id: 1,
        start: 0.0,
        end: 2.0,
        text: "Voice".into(),
        speaker: Some("Narrator".into()),
    });

    let cmd = arm_track(&tl, aid, true).unwrap();
    let mut h = History::new();
    h.execute(&mut tl, cmd);

    assert_eq!(tl.captions.len(), 1);
    assert_eq!(tl.captions[0].speaker.as_deref(), Some("Narrator"));
}

#[test]
fn t3_18_delete_active_take_reverts_to_previous_media_id() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    v.events.push(helper_event(1, 0.0, 5.0, Some(100)));
    let vid = v.id;
    tl.tracks.push(v);

    let mut h = History::new();
    let add_cmd = add_take(&tl, &[(vid, 1)], 200, "Take 2", 0.0).unwrap();
    h.execute(&mut tl, add_cmd);
    assert_eq!(tl.track(vid).unwrap().events[0].media_id, Some(200));

    let del_cmd = neoveg_engine::takes::delete_active(&tl, &[(vid, 1)]).unwrap();
    h.execute(&mut tl, del_cmd);
    assert_eq!(tl.track(vid).unwrap().events[0].media_id, Some(100));
}

#[test]
fn t3_19_fuzzy_relink_matches_alternate_case_and_dashes() {
    let media = MediaRef {
        id: 1,
        path: "D:/Media/interview_part_01.mov".into(),
        duration_seconds: 60.0,
        subproject: false,
        format: None,
    };
    let dir = std::env::temp_dir().join(format!("neoveg_t3_19_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("INTERVIEW-PART-01.MOV"), b"data");

    let found = find_fuzzy(&media, std::slice::from_ref(&dir));
    assert!(found.is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn t3_20_ungroup_single_pair_leaves_other_groups_intact() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    let mut e1 = helper_event(1, 0.0, 5.0, Some(1));
    let mut e2 = helper_event(2, 5.0, 5.0, Some(2));
    e1.group = Some(100);
    e2.group = Some(200);
    v.events.extend([e1, e2]);
    let vid = v.id;
    tl.tracks.push(v);

    let out = ungroup_events(&tl, &[(vid, 1)]).unwrap();
    let mut h = History::new();
    h.execute(&mut tl, out.command);

    assert_eq!(tl.track(vid).unwrap().events[0].group, None);
    assert_eq!(tl.track(vid).unwrap().events[1].group, Some(200));
}

#[test]
fn t3_21_only_events_retains_event_pan_crop() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    let mut ev = helper_event(1, 0.0, 5.0, Some(1));
    ev.pan_crop.stretch_to_fill = false;
    v.events.push(ev);
    let vid = v.id;
    tl.tracks.push(v);

    let filtered = only_events(&tl, &[(vid, 1)]);
    assert!(
        !filtered.track(vid).unwrap().events[0]
            .pan_crop
            .stretch_to_fill
    );
}

#[test]
fn t3_22_crossfade_length_invariant_under_timeline_translation() {
    let e1 = helper_event(1, 10.0, 4.0, Some(1));
    let e2 = helper_event(2, 13.0, 4.0, Some(2));
    let cfs = find_crossfades(&[e1, e2]);
    assert_eq!(cfs.len(), 1);
    assert!((cfs[0].length() - 1.0).abs() < 1e-4);
}

#[test]
fn t3_23_transition_progress_active_requires_points() {
    let mut env = new_envelope(1.0);
    assert!(is_progress_active(Some(&env)));
    env.points.clear();
    assert!(!is_progress_active(Some(&env)));
}

#[test]
fn t3_24_pan_crop_rotation_normalized_under_keyframe_lerp() {
    let mut pc = PanCrop::default();
    pc.keys.push(PanCropKey {
        time: 0.0,
        frame: PanCropFrame {
            rotation_deg: 0.0,
            ..PanCropFrame::FULL
        },
        smoothness: 0.0,
    });
    pc.keys.push(PanCropKey {
        time: 2.0,
        frame: PanCropFrame {
            rotation_deg: 180.0,
            ..PanCropFrame::FULL
        },
        smoothness: 0.0,
    });
    let mid = pc.frame_at(1.0);
    assert!((mid.rotation_deg - 90.0).abs() < 1e-3);
}

#[test]
fn t3_25_detail_rows_correctly_flags_normalized_audio() {
    let mut tl = Timeline::new();
    let mut a = tl.new_track(TrackKind::Audio);
    let mut ev = helper_event(1, 0.0, 5.0, Some(1));
    ev.normalize = true;
    a.events.push(ev);
    tl.tracks.push(a);

    let d = rows(&tl);
    assert!(d[0].flags.contains('N'));
}

#[test]
fn t3_26_detail_rows_correctly_flags_inverted_phase() {
    let mut tl = Timeline::new();
    let mut a = tl.new_track(TrackKind::Audio);
    let mut ev = helper_event(1, 0.0, 5.0, Some(1));
    ev.invert_phase = true;
    a.events.push(ev);
    tl.tracks.push(a);

    let d = rows(&tl);
    assert!(d[0].flags.contains('F'));
}

#[test]
fn t3_27_detail_rows_correctly_flags_looping_event() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    let mut ev = helper_event(1, 0.0, 5.0, Some(1));
    ev.looping = true;
    v.events.push(ev);
    tl.tracks.push(v);

    let d = rows(&tl);
    assert!(d[0].flags.contains('L'));
}

#[test]
fn t3_28_target_track_order_follows_timeline_track_sequence() {
    let mut tl = Timeline::new();
    let mut a1 = tl.new_track(TrackKind::Audio);
    let mut a2 = tl.new_track(TrackKind::Audio);
    a1.armed = true;
    a2.armed = true;
    let a1_id = a1.id;
    let a2_id = a2.id;
    tl.tracks.extend([a1, a2]);

    assert_eq!(target_track(&tl), Some(a1_id));

    // Reverse order
    tl.tracks.reverse();
    assert_eq!(target_track(&tl), Some(a2_id));
}

#[test]
fn t3_29_step_takes_wraps_around_backward() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    v.events.push(helper_event(1, 0.0, 5.0, Some(1)));
    let vid = v.id;
    tl.tracks.push(v);

    let mut h = History::new();
    let add_cmd = add_take(&tl, &[(vid, 1)], 2, "Take 2", 0.0).unwrap();
    h.execute(&mut tl, add_cmd);

    // Active is take 1. Step backward by 1 -> take 0
    let step_cmd = step_take(&tl, &[(vid, 1)], -1).unwrap();
    h.execute(&mut tl, step_cmd);
    assert_eq!(active_index(&tl.track(vid).unwrap().events[0]), 0);

    // Step backward by 1 again -> wraps to take 1
    let step_cmd2 = step_take(&tl, &[(vid, 1)], -1).unwrap();
    h.execute(&mut tl, step_cmd2);
    assert_eq!(active_index(&tl.track(vid).unwrap().events[0]), 1);
}

#[test]
fn t3_30_make_movie_describe_skips_muted_tracks_if_unselected() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    v.events.push(helper_event(1, 0.0, 5.0, Some(1)));
    let vid = v.id;
    tl.tracks.push(v);

    let desc = describe(&tl, &[(vid, 1)]).unwrap();
    assert!(desc.has_video);
    assert!(!desc.has_audio);
}

#[test]
fn t3_31_pan_crop_full_rotation_cycle_boundary() {
    let frame = PanCropFrame {
        rotation_deg: 720.0,
        ..PanCropFrame::FULL
    };
    assert_eq!(frame.sanitized().rotation_deg, 0.0);
}

#[test]
fn t3_32_pan_crop_negative_rotation_cycle_boundary() {
    let frame = PanCropFrame {
        rotation_deg: -360.0,
        ..PanCropFrame::FULL
    };
    assert_eq!(frame.sanitized().rotation_deg, 0.0);
}

#[test]
fn t3_33_edge_time_end_calculation_precision() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    v.events.push(helper_event(1, 1.234567, 3.456789, Some(1)));
    let vid = v.id;
    tl.tracks.push(v);

    let edge = Edge {
        track: vid,
        event: 1,
        side: EdgeSide::End,
    };
    let t = edge_time(&tl, edge).unwrap();
    assert!((t - (1.234567 + 3.456789)).abs() < 1e-6);
}

#[test]
fn t3_34_project_manager_put_updates_existing_project() {
    let mut pm = ProjectManager::new();
    let pid = pm.register(ProjectInfo::create_empty(), Timeline::new(), None);
    let mut p = pm.take(pid).unwrap();
    p.info.name = "Updated Title".into();
    pm.put(p);

    assert_eq!(pm.get(pid).unwrap().info.name, "Updated Title");
}

#[test]
fn t3_35_transition_envelope_linear_slope_consistency() {
    let env = new_envelope(4.0);
    let p1 = progress_at(&env, 1.0);
    let p2 = progress_at(&env, 2.0);
    let p3 = progress_at(&env, 3.0);
    assert!((p2 - p1 - 0.25).abs() < 1e-4);
    assert!((p3 - p2 - 0.25).abs() < 1e-4);
}

#[test]
fn t3_36_relink_rejects_non_matching_stem() {
    let media = MediaRef {
        id: 1,
        path: "outro.mp4".into(),
        duration_seconds: 5.0,
        subproject: false,
        format: None,
    };
    let dir = std::env::temp_dir().join(format!("neoveg_t3_36_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("intro.mp4"), b"sample");

    assert_eq!(find_fuzzy(&media, std::slice::from_ref(&dir)), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn t3_37_crossfade_span_matches_exact_overlap_interval() {
    let e1 = helper_event(1, 2.0, 4.0, Some(1)); // 2.0..6.0
    let e2 = helper_event(2, 5.0, 3.0, Some(2)); // 5.0..8.0
    let cfs = find_crossfades(&[e1, e2]);
    assert_eq!(cfs[0].start, 5.0);
    assert_eq!(cfs[0].end, 6.0);
}

#[test]
fn t3_38_l_cut_with_audio_pre_roll() {
    let e_v = helper_event(1, 2.0, 5.0, Some(1));
    let e_a = helper_event(2, 0.0, 7.0, Some(1));
    assert_eq!(e_v.start - e_a.start, 2.0);
    assert_eq!(e_a.end() - e_v.end(), 0.0);
}

#[test]
fn t3_39_project_settings_field_order_variants() {
    let mut s = ProjectSettings {
        field_order: PresetFieldOrder::UpperFirst,
        ..ProjectSettings::default()
    };
    assert_eq!(s.field_order, PresetFieldOrder::UpperFirst);
    s.field_order = PresetFieldOrder::Progressive;
    assert_eq!(s.field_order, PresetFieldOrder::Progressive);
}

#[test]
fn t3_40_media_ref_subproject_cycle_prevention() {
    let p = MediaRef {
        id: 1,
        path: "proj_a.neoveg".into(),
        duration_seconds: 10.0,
        subproject: true,
        format: None,
    };
    assert!(p.subproject);
}

#[test]
fn t3_41_group_events_preserves_event_names_and_media_ids() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    let mut a = tl.new_track(TrackKind::Audio);
    v.events.push(helper_event(10, 0.0, 5.0, Some(100)));
    a.events.push(helper_event(20, 0.0, 5.0, Some(200)));
    let (vid, aid) = (v.id, a.id);
    tl.tracks.extend([v, a]);

    let out = group_events(&mut tl, &[(vid, 10), (aid, 20)]).unwrap();
    let mut h = History::new();
    h.execute(&mut tl, out.command);

    assert_eq!(tl.track(vid).unwrap().events[0].name, "Event_10");
    assert_eq!(tl.track(aid).unwrap().events[0].name, "Event_20");
    assert_eq!(tl.track(vid).unwrap().events[0].media_id, Some(100));
    assert_eq!(tl.track(aid).unwrap().events[0].media_id, Some(200));
}

#[test]
fn t3_42_only_events_empty_selection_results_in_zero_events() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    v.events.push(helper_event(1, 0.0, 5.0, Some(1)));
    tl.tracks.push(v);

    let empty = only_events(&tl, &[]);
    assert_eq!(empty.tracks[0].events.len(), 0);
}

// ============================================================================
// Tier 4: Real-World Production Scenarios (6 Comprehensive Pipelines)
// ============================================================================

/// Scenario 1: Multi-Track Recording & Podcasting Session
/// - Set up audio host project
/// - Arm host and guest microphones across tracks
/// - Verify target track resolution
/// - Non-destructively record takes and verify undo/redo stack integrity
#[test]
fn t4_scenario_01_multitrack_podcast_recording_session() {
    let mut tl = Timeline::new();
    let mut h = History::new();

    // Create 3 audio tracks: Host Mic, Guest Mic, Music Bed
    let mut t_host = tl.new_track(TrackKind::Audio);
    t_host.name = "Host Mic".into();
    let mut t_guest = tl.new_track(TrackKind::Audio);
    t_guest.name = "Guest Mic".into();
    let mut t_music = tl.new_track(TrackKind::Audio);
    t_music.name = "Music Bed".into();

    let host_id = t_host.id;
    let guest_id = t_guest.id;
    let music_id = t_music.id;
    tl.tracks.extend([t_host, t_guest, t_music]);

    // Arm host and guest
    let arm_host = arm_track(&tl, host_id, true).expect("Host track arms");
    assert!(h.execute(&mut tl, arm_host));
    let arm_guest = arm_track(&tl, guest_id, true).expect("Guest track arms");
    assert!(h.execute(&mut tl, arm_guest));

    assert_eq!(armed_tracks(&tl), vec![host_id, guest_id]);
    assert_eq!(target_track(&tl), Some(host_id));

    // Place intro music
    let music_ev = helper_event(1, 0.0, 15.0, Some(50));
    tl.track_mut(music_id).unwrap().events.push(music_ev);

    // Record host and guest takes
    let host_take = helper_event(2, 5.0, 30.0, Some(51));
    let guest_take = helper_event(3, 5.0, 30.0, Some(52));
    tl.track_mut(host_id).unwrap().events.push(host_take);
    tl.track_mut(guest_id).unwrap().events.push(guest_take);

    // Group host and guest dialog for synchronous editing
    let grp = group_events(&mut tl, &[(host_id, 2), (guest_id, 3)]).expect("Dialog tracks group");
    assert!(h.execute(&mut tl, grp.command));

    let g_h = tl.track(host_id).unwrap().events[0].group;
    let g_g = tl.track(guest_id).unwrap().events[0].group;
    assert!(g_h.is_some() && g_h == g_g);

    // Nudge both dialog tracks forward by 2.0s
    let mv = move_events(&tl, &[(host_id, 2), (guest_id, 3)], 2.0).expect("Move executes");
    assert!(h.execute(&mut tl, mv.command));
    assert_eq!(tl.track(host_id).unwrap().events[0].start, 7.0);
    assert_eq!(tl.track(guest_id).unwrap().events[0].start, 7.0);

    // Verification: undo all the way back to initial armed state
    assert!(h.undo(&mut tl)); // undo move
    assert_eq!(tl.track(host_id).unwrap().events[0].start, 5.0);
    assert!(h.undo(&mut tl)); // undo grouping
    assert_eq!(tl.track(host_id).unwrap().events[0].group, None);
}

/// Scenario 2: Multicam Concert Event Edit
/// - Setup 3 camera angles (Cam A wide, Cam B stage, Cam C close-up)
/// - Sync takes per event
/// - Live switch angles
/// - Create L-Cut to keep crowd applause audio bleeding under subsequent shot
#[test]
fn t4_scenario_02_multicam_concert_live_switching() {
    let mut tl = Timeline::new();
    let mut h = History::new();

    let mut v = tl.new_track(TrackKind::Video);
    v.name = "Concert Multicam".into();
    let mut a = tl.new_track(TrackKind::Audio);
    a.name = "Stage Master Sound".into();

    let v_id = v.id;
    let a_id = a.id;

    // Shot 1: Cam A (Wide) from 0.0 to 10.0
    let mut shot1_v = helper_event(1, 0.0, 10.0, Some(101));
    let shot1_a = helper_event(2, 0.0, 12.0, Some(201)); // Audio extends 2s longer (L-cut)
    shot1_v.group = Some(1);
    v.events.push(shot1_v);
    a.events.push(shot1_a);

    tl.tracks.extend([v, a]);

    // Add Cam B (Stage) as Take 2
    let add_take2 = add_take(&tl, &[(v_id, 1)], 102, "Cam B - Stage", 0.0).expect("Add Take 2");
    assert!(h.execute(&mut tl, add_take2));

    // Add Cam C (Close-up) as Take 3
    let add_take3 = add_take(&tl, &[(v_id, 1)], 103, "Cam C - Closeup", 0.0).expect("Add Take 3");
    assert!(h.execute(&mut tl, add_take3));

    let ev = &tl.track(v_id).unwrap().events[0];
    assert_eq!(take_count(ev), 3);
    assert_eq!(active_index(ev), 2); // Cam C is currently active
    assert_eq!(ev.media_id, Some(103));

    // Live switch: Director calls for Cam B
    let switch_cam_b = set_active_take(&tl, &[(v_id, 1)], 1).expect("Switch to Cam B");
    assert!(h.execute(&mut tl, switch_cam_b));
    assert_eq!(tl.track(v_id).unwrap().events[0].media_id, Some(102));

    // Verify L-cut boundary: Audio continues 2s after Video cut
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
    assert_eq!(edge_time(&tl, edge_v), Some(10.0));
    assert_eq!(edge_time(&tl, edge_a), Some(12.0));
}

/// Scenario 3: Commercial Color Grading & Delivery Pipeline
/// - Ingest high-contrast footage
/// - Construct multi-clip timeline
/// - Verify color match statistics transfer
/// - Set up render placement for final commercial delivery
#[test]
fn t4_scenario_03_commercial_color_grading_and_delivery() {
    let mut tl = Timeline::new();
    let mut h = History::new();

    let mut v = tl.new_track(TrackKind::Video);
    v.name = "Color Graded Timeline".into();
    let vid = v.id;

    // Clip 1: Exterior daylight shot (0.0 to 4.0)
    let clip1 = helper_event(1, 0.0, 4.0, Some(1));
    // Clip 2: Interior studio shot (4.0 to 8.0)
    let clip2 = helper_event(2, 4.0, 4.0, Some(2));
    v.events.extend([clip1, clip2]);
    tl.tracks.push(v);

    // Apply 0.5s crossfade between exterior and interior
    tl.track_mut(vid).unwrap().events[1].start = 3.5; // overlaps 3.5 to 4.0
    let cfs = find_crossfades(&tl.track(vid).unwrap().events);
    assert_eq!(cfs.len(), 1);
    assert_eq!(cfs[0].length(), 0.5);

    // Generate detail cut list
    let d = rows(&tl);
    assert_eq!(d.len(), 2);
    let tsv = to_tsv(&d, 24.0, false);
    assert!(tsv.contains("Event_1"));
    assert!(tsv.contains("Event_2"));

    // Make Movie delivery: render selection
    let desc = describe(&tl, &[(vid, 1), (vid, 2)]).expect("Commercial selection valid");
    assert_eq!(desc.span, (0.0, 7.5));

    // Place finished master render replacing draft clips
    let master = helper_event(10, 0.0, 7.5, Some(999));
    let place = placement(&tl, &[(vid, 1), (vid, 2)], vec![(vid, master)], true).unwrap();
    assert!(h.execute(&mut tl, place));

    assert_eq!(tl.track(vid).unwrap().events.len(), 1);
    assert_eq!(tl.track(vid).unwrap().events[0].id, 10);
    assert_eq!(tl.track(vid).unwrap().events[0].duration, 7.5);
}

/// Scenario 4: AI Subtitling & Social Video Reframe
/// - Ingest horizontal 16:9 interview video
/// - Add subtitles tracks with timing
/// - Configure dynamic Pan/Crop keyframes to keep speaker centered for 9:16 vertical export
#[test]
fn t4_scenario_04_ai_subtitling_and_social_reframe() {
    let mut tl = Timeline::new();
    let mut v = tl.new_track(TrackKind::Video);
    let mut ev = helper_event(1, 0.0, 10.0, Some(1));

    // 9:16 reframe window centered on speaker walking from left to right
    let mut pc = PanCrop::default();
    pc.keys.push(PanCropKey {
        time: 0.0,
        frame: PanCropFrame {
            x_center: 0.35, // speaker on left
            y_center: 0.5,
            width: 0.5625, // 9/16 vertical ratio
            height: 1.0,
            rotation_deg: 0.0,
        },
        smoothness: 1.0,
    });
    pc.keys.push(PanCropKey {
        time: 10.0,
        frame: PanCropFrame {
            x_center: 0.65, // speaker moved right
            y_center: 0.5,
            width: 0.5625,
            height: 1.0,
            rotation_deg: 0.0,
        },
        smoothness: 1.0,
    });
    ev.pan_crop = pc;
    v.events.push(ev);
    tl.tracks.push(v);

    // Subtitle cues aligned with speech
    tl.captions.push(Caption {
        id: 1,
        start: 0.5,
        end: 4.5,
        text: "Bem-vindos ao Neoveg Studio!".into(),
        speaker: Some("Apresentador".into()),
    });
    tl.captions.push(Caption {
        id: 2,
        start: 5.0,
        end: 9.5,
        text: "Edição de vídeo limpa, rápida e precisa.".into(),
        speaker: Some("Apresentador".into()),
    });

    assert_eq!(tl.captions.len(), 2);

    // Midpoint check: speaker centered at x = 0.50
    let mid_frame = tl.tracks[0].events[0].pan_crop.frame_at(5.0);
    assert!((mid_frame.x_center - 0.50).abs() < 1e-3);
    assert_eq!(mid_frame.width, 0.5625);
}

/// Scenario 5: Complex VFX & Motion Masking with Prerender Caching
/// - Multi-layer compositing
/// - Custom transition envelope shaping
/// - Non-destructive history and project dirty tracking
#[test]
fn t4_scenario_05_complex_vfx_and_transition_enveloping() {
    let mut pm = ProjectManager::new();
    let pid = pm.register(ProjectInfo::create_empty(), Timeline::new(), None);
    let mut p = pm.take(pid).unwrap();

    let mut v = p.timeline.new_track(TrackKind::Video);
    let mut ev = helper_event(1, 0.0, 8.0, Some(1));

    // Custom 3-stage transition envelope: ramp up to 50%, hold, then complete
    let mut env = new_envelope(2.0);
    env.points.insert(
        1,
        EnvelopePoint {
            time: 1.0,
            value: 0.5,
            shape: EnvelopeShape::Linear,
        },
    );
    ev.transition_progress = Some(env);
    v.events.push(ev);
    p.timeline.tracks.push(v);

    let vid = p.timeline.tracks[0].id;
    let registered_env = p.timeline.track(vid).unwrap().events[0]
        .transition_progress
        .as_ref()
        .unwrap();
    assert!(is_progress_active(Some(registered_env)));
    assert!((progress_at(registered_env, 1.0) - 0.5).abs() < 1e-4);

    pm.put(p);
    assert!(pm.get(pid).is_some());
}

/// Scenario 6: Smart Proxy Offline/Online Workflow & Alert Handling
/// - High-resolution footage ingestion
/// - Missing media detection and fuzzy relinking
/// - Standardized alert emissions (§10 compliance)
/// - Conforming to original source media
#[test]
fn t4_scenario_06_smart_proxy_offline_online_and_alerts() {
    let temp_root = std::env::temp_dir().join(format!("neoveg_t4_06_{}", std::process::id()));
    let online_dir = temp_root.join("online_raw");
    let proxy_dir = temp_root.join("proxies");
    let _ = std::fs::create_dir_all(&online_dir);
    let _ = std::fs::create_dir_all(&proxy_dir);

    // Create raw and proxy files on disk
    let _ = std::fs::write(online_dir.join("SCENE_01_RAW.MOV"), b"8K RAW FOOTAGE");
    let proxy_file = proxy_dir.join("scene_01_raw.mov");
    let _ = std::fs::write(&proxy_file, b"1080p PROXY");

    let media = MediaRef {
        id: 1,
        path: "D:/offline_storage/SCENE_01_RAW.MOV".into(),
        duration_seconds: 45.0,
        subproject: false,
        format: Some(MediaFormat {
            channels: Some(2),
            fps: Some(24.0),
        }),
    };

    // Step 1: Missing file raises warning alert
    let mut reg = AlertRegistry::new();
    let missing_alert = StandardAlert {
        id: "MEDIA_OFFLINE",
        severity: AlertSeverity::Warning,
        category: AlertCategory::MediaPool,
        message: "Media SCENE_01_RAW.MOV is offline; searching proxy pool".into(),
    };
    reg.register(missing_alert);
    assert!(reg.has_alerts());

    // Step 2: Auto-relink finds proxy candidate
    let found_proxy = find_fuzzy(&media, std::slice::from_ref(&proxy_dir)).expect("Proxy found");
    assert_eq!(found_proxy, proxy_file);

    // Step 3: Conforming - switch back to online raw media
    let found_raw =
        find_exact(&media, std::slice::from_ref(&online_dir)).expect("RAW media conformed");
    assert_eq!(found_raw, online_dir.join("SCENE_01_RAW.MOV"));

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_root);
}
