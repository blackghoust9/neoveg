//! Adjustment events and adjustment tracks.
//!
//! - An **adjustment event** (`TrackEvent::adjustment`) is an empty video event whose effect chain
//!   runs on the composite of the video tracks below it for as long as it lasts. It behaves like a
//!   video event otherwise (fades, opacity, trim, split, copy and paste, Mute and Lock), except
//!   that it has no media, so a velocity envelope, a Pan/Crop or a reverse mean nothing for it.
//! - An **adjustment track** (`Track::adjustment`) is a video track with no media of its own whose
//!   track-level `video_effects` run on all video tracks below it in the track list, weighted over
//!   time by its `VideoLevel` composite envelope (which starts enabled at `1.0`).
//!
//! Everything here is a pure function; `neoveg_ui` turns adjustment events and adjustment tracks
//! into composite nodes.

use crate::command::EditCommand;
use crate::envelope::{AutomationTarget, TrackEnvelope};
use crate::timeline::{
    EventGeometry, EventId, Timeline, Track, TrackEvent, TrackId, TrackKind, TIME_EPSILON,
};

/// Length of a new adjustment event, in seconds (the manual gives none).
pub const DEFAULT_LENGTH: f64 = 5.0;

/// Name a new adjustment event starts with.
pub const DEFAULT_NAME: &str = "Evento de ajuste";

/// Prefix a new adjustment track starts with (`"Ajuste 1"`, `"Ajuste 2"`, …).
pub const DEFAULT_TRACK_PREFIX: &str = "Ajuste";

/// Builds (without inserting) a new adjustment video track with an enabled `VideoLevel` composite
/// envelope at `1.0` and the preferred fade-to-color endpoints.
pub fn new_track(timeline: &mut Timeline, fade_top: [u8; 3], fade_bottom: [u8; 3]) -> Track {
    let number = timeline.tracks.iter().filter(|t| t.adjustment).count() + 1;
    let mut track = timeline.new_track(TrackKind::Video);
    track.name = format!("{DEFAULT_TRACK_PREFIX} {number}");
    track.adjustment = true;
    track.fade_color = fade_top;
    track.fade_color_bottom = fade_bottom;
    track.envelopes = vec![TrackEnvelope::new(AutomationTarget::VideoLevel, 1.0)];
    track
}

/// Inserts a new adjustment track right above `selected_track` when `selected_track` is a video
/// track, or at index `0` (the top layer of `Timeline::tracks`) otherwise, returning its `TrackId`
/// and a single undoable [`EditCommand`].
pub fn insert_track_command(
    timeline: &mut Timeline,
    selected_track: Option<TrackId>,
    fade_top: [u8; 3],
    fade_bottom: [u8; 3],
) -> (TrackId, EditCommand) {
    let index = selected_track
        .and_then(|id| {
            timeline
                .tracks
                .iter()
                .position(|t| t.id == id && t.kind == TrackKind::Video)
        })
        .unwrap_or(0);
    let track = new_track(timeline, fade_top, fade_bottom);
    let id = track.id;
    (id, EditCommand::InsertTrack { track, index })
}

/// The command that adds an adjustment event with id `id` at `start` on video track `track_id`, or
/// `None` when the track does not exist or is not a video track.
pub fn insert_command(
    timeline: &Timeline,
    track_id: TrackId,
    start: f64,
    id: EventId,
) -> Option<EditCommand> {
    let track = timeline.track(track_id)?;
    if track.kind != TrackKind::Video {
        return None;
    }
    let start = if start.is_finite() {
        start.max(0.0)
    } else {
        0.0
    };
    Some(EditCommand::AddEvent {
        track_id,
        event: TrackEvent {
            id,
            name: DEFAULT_NAME.into(),
            media_id: None,
            start,
            duration: DEFAULT_LENGTH,
            adjustment: true,
            ..TrackEvent::default()
        },
    })
}

/// End time of the project content for resizing adjustment events: the end of the last
/// non-adjustment event across all tracks, falling back to `timeline.duration()` when the project
/// holds only adjustment events.
pub fn project_span_end(timeline: &Timeline) -> f64 {
    let content_end = timeline
        .tracks
        .iter()
        .flat_map(|t| t.events.iter())
        .filter(|e| !e.adjustment)
        .map(TrackEvent::end)
        .fold(0.0, f64::max);
    if content_end > TIME_EPSILON {
        content_end
    } else {
        timeline.duration()
    }
}

/// `true` when the project has non-zero duration and `targets` includes at least one unlocked
/// adjustment event on a video track.
pub fn can_resize_to_project(timeline: &Timeline, targets: &[(TrackId, EventId)]) -> bool {
    let end = project_span_end(timeline);
    if !end.is_finite() || end <= TIME_EPSILON {
        return false;
    }
    targets.iter().any(|&(track_id, event_id)| {
        timeline
            .track(track_id)
            .filter(|t| t.kind == TrackKind::Video)
            .and_then(|t| t.events.iter().find(|e| e.id == event_id))
            .is_some_and(|e| e.adjustment && !e.locked)
    })
}

/// Builds a single undo step (`EditEvent` or `Batch`) resizing all unlocked adjustment events in
/// `targets` to span `0.0..project_span_end(timeline)`, clamping fades and `snap_offset` to the
/// new length. Returns `None` when no target event needs changing (along with the count of resized
/// events when `Some`).
pub fn resize_to_project_command(
    timeline: &Timeline,
    targets: &[(TrackId, EventId)],
) -> Option<(EditCommand, usize)> {
    let end = project_span_end(timeline);
    if !end.is_finite() || end <= TIME_EPSILON {
        return None;
    }
    let mut seen = Vec::with_capacity(targets.len());
    let mut commands = Vec::new();
    for &(track_id, event_id) in targets {
        if seen.contains(&(track_id, event_id)) {
            continue;
        }
        seen.push((track_id, event_id));
        let Some(track) = timeline
            .track(track_id)
            .filter(|t| t.kind == TrackKind::Video)
        else {
            continue;
        };
        let Some(event) = track
            .events
            .iter()
            .find(|e| e.id == event_id && e.adjustment && !e.locked)
        else {
            continue;
        };
        let old = event.geometry();
        let fade_in = old.fade_in.clamp(0.0, end);
        let fade_out = old.fade_out.clamp(0.0, (end - fade_in).max(0.0));
        let snap_offset = if old.snap_offset.is_finite() {
            old.snap_offset.clamp(0.0, end)
        } else {
            0.0
        };
        let new = EventGeometry {
            start: 0.0,
            duration: end,
            fade_in,
            fade_out,
            snap_offset,
            ..old
        };
        if (old.start - new.start).abs() > TIME_EPSILON
            || (old.duration - new.duration).abs() > TIME_EPSILON
            || (old.fade_in - new.fade_in).abs() > TIME_EPSILON
            || (old.fade_out - new.fade_out).abs() > TIME_EPSILON
            || (old.snap_offset - new.snap_offset).abs() > TIME_EPSILON
        {
            commands.push(EditCommand::EditEvent {
                from_track: track_id,
                to_track: track_id,
                event_id,
                old,
                new,
            });
        }
    }
    let count = commands.len();
    match count {
        0 => None,
        1 => commands.pop().map(|c| (c, 1)),
        _ => Some((EditCommand::Batch(commands), count)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::timeline::TrackEffect;

    fn setup() -> (Timeline, History, TrackId, TrackId) {
        let mut tl = Timeline::new();
        let mut h = History::new();
        let video = tl.new_track(TrackKind::Video);
        let audio = tl.new_track(TrackKind::Audio);
        let (v, a) = (video.id, audio.id);
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track: video }));
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track: audio }));
        (tl, h, v, a)
    }

    #[test]
    fn an_adjustment_event_goes_on_a_video_track_only_and_undoes_in_one_step() {
        let (mut tl, mut h, v, a) = setup();
        let id = tl.alloc_id();
        assert!(
            insert_command(&tl, a, 1.0, id).is_none(),
            "not on an audio track"
        );
        assert!(insert_command(&tl, 999, 1.0, id).is_none());
        let command = insert_command(&tl, v, 2.5, id).unwrap();
        assert!(h.execute(&mut tl, command));
        let event = tl.track(v).unwrap().events[0].clone();
        assert!(event.adjustment && event.media_id.is_none());
        assert_eq!((event.start, event.duration), (2.5, DEFAULT_LENGTH));
        assert!(h.undo(&mut tl));
        assert!(tl.track(v).unwrap().events.is_empty());
        // A bad start is brought back to zero
        let again = insert_command(&tl, v, f64::NAN, id).unwrap();
        assert!(matches!(&again, EditCommand::AddEvent { event, .. } if event.start == 0.0));
    }

    #[test]
    fn velocity_and_pan_crop_do_not_apply_to_an_adjustment_event() {
        let (mut tl, mut h, v, _) = setup();
        let id = tl.alloc_id();
        let command = insert_command(&tl, v, 0.0, id).unwrap();
        assert!(h.execute(&mut tl, command));
        assert!(crate::velocity::get(&tl, v, id).is_none());
        assert!(
            crate::velocity::set(&tl, v, id, Some(crate::velocity::new_envelope(2.0))).is_none()
        );
        assert!(crate::pan_crop::get(&tl, v, id).is_none());
    }

    #[test]
    fn the_effect_chain_and_the_flag_survive_saving_and_old_events_are_not_adjustments() {
        let mut event = TrackEvent {
            id: 7,
            adjustment: true,
            duration: 3.0,
            video_effects: vec![TrackEffect::new("vignette")],
            ..TrackEvent::default()
        };
        let back: TrackEvent =
            serde_json::from_str(&serde_json::to_string(&event).unwrap()).unwrap();
        assert_eq!(back, event);
        event.adjustment = false;
        let json = serde_json::to_string(&event).unwrap();
        assert!(
            !json.contains("adjustment"),
            "a plain event does not write the flag"
        );
        let old: TrackEvent = serde_json::from_str(
            r#"{"id":1,"name":"x","media_id":null,"start":0.0,"duration":1.0,"trim_in":0.0,"fade_in":0.0,"fade_out":0.0}"#,
        )
        .unwrap();
        assert!(!old.adjustment);
    }

    #[test]
    fn adjustment_track_inserts_above_selected_video_track_with_composite_envelope_and_serde() {
        let (mut tl, mut h, v1, audio) = setup();
        let v2_track = tl.new_track(TrackKind::Video);
        let v2 = v2_track.id;
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track: v2_track }));
        // Tracks right now: [v1, audio, v2]

        // Selecting an audio track (or None) inserts at the very top (index 0)
        let (adj1, cmd1) = insert_track_command(&mut tl, Some(audio), [1, 2, 3], [250, 251, 252]);
        assert!(h.execute(&mut tl, cmd1));
        assert_eq!(tl.tracks[0].id, adj1);
        assert_eq!(tl.tracks[0].name, "Ajuste 1");
        assert!(tl.tracks[0].adjustment);
        assert_eq!(tl.tracks[0].kind, TrackKind::Video);
        assert_eq!(tl.tracks[0].fade_color, [1, 2, 3]);
        assert_eq!(tl.tracks[0].fade_color_bottom, [250, 251, 252]);
        assert_eq!(tl.tracks[0].envelopes.len(), 1);
        assert_eq!(
            tl.tracks[0].envelopes[0].target,
            AutomationTarget::VideoLevel
        );
        assert!(tl.tracks[0].envelopes[0].enabled && tl.tracks[0].envelopes[0].visible);
        assert_eq!(tl.tracks[0].envelopes[0].value_at(0.0, 0.0), 1.0);

        // Selecting v2 inserts right above v2 (leaving v1 and audio above it)
        let (adj2, cmd2) = insert_track_command(&mut tl, Some(v2), [0, 0, 0], [255, 255, 255]);
        assert!(h.execute(&mut tl, cmd2));
        let ids: Vec<TrackId> = tl.tracks.iter().map(|t| t.id).collect();
        assert_eq!(ids, vec![adj1, v1, audio, adj2, v2]);
        assert_eq!(tl.track(adj2).unwrap().name, "Ajuste 2");

        // Serde round-trip & backward compatibility on Track
        let adj_json = serde_json::to_string(tl.track(adj1).unwrap()).unwrap();
        assert!(adj_json.contains("\"adjustment\":true"));
        let round: Track = serde_json::from_str(&adj_json).unwrap();
        assert_eq!(&round, tl.track(adj1).unwrap());

        let plain_json = serde_json::to_string(tl.track(v1).unwrap()).unwrap();
        assert!(
            !plain_json.contains("adjustment"),
            "plain video tracks omit the adjustment flag"
        );
        let legacy: Track = serde_json::from_str(
            r#"{"id":42,"kind":"Video","name":"Video 1","muted":false,"volume_db":0.0,"pan":0.0,"invert_phase":false,"events":[],"effects":[]}"#,
        )
        .unwrap();
        assert!(!legacy.adjustment);

        // Undo & redo restore exact track order
        assert!(h.undo(&mut tl));
        assert_eq!(
            tl.tracks.iter().map(|t| t.id).collect::<Vec<_>>(),
            vec![adj1, v1, audio, v2]
        );
        assert!(h.redo(&mut tl));
        assert_eq!(
            tl.tracks.iter().map(|t| t.id).collect::<Vec<_>>(),
            vec![adj1, v1, audio, adj2, v2]
        );
    }

    #[test]
    fn resize_to_project_matches_media_end_skips_locked_and_clamps_fades_and_snap_offset() {
        let (mut tl, mut h, v_top, audio) = setup();
        let v_bot_track = tl.new_track(TrackKind::Video);
        let v_bot = v_bot_track.id;
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track: v_bot_track }));

        // Empty timeline: nothing to resize to
        assert!(!can_resize_to_project(&tl, &[(v_top, 1)]));
        assert!(resize_to_project_command(&tl, &[(v_top, 1)]).is_none());

        // Media clips: video ends at 8 s, audio ends at 14 s -> project content ends at 14 s
        let clip_v = tl.alloc_id();
        let clip_a = tl.alloc_id();
        assert!(h.execute(
            &mut tl,
            EditCommand::AddEvent {
                track_id: v_bot,
                event: TrackEvent {
                    id: clip_v,
                    media_id: Some(1),
                    start: 1.0,
                    duration: 7.0,
                    ..TrackEvent::default()
                },
            }
        ));
        assert!(h.execute(
            &mut tl,
            EditCommand::AddEvent {
                track_id: audio,
                event: TrackEvent {
                    id: clip_a,
                    media_id: Some(2),
                    start: 2.0,
                    duration: 12.0,
                    ..TrackEvent::default()
                },
            }
        ));

        // Add three adjustment events:
        // 1) short one (3..8)
        // 2) oversized one (4..25 -> extends beyond 14 s, with large fades and snap_offset = 19 s)
        // 3) locked one (5..9 -> must NOT move)
        let adj_short = tl.alloc_id();
        let adj_long = tl.alloc_id();
        let adj_locked = tl.alloc_id();
        assert!(h.execute(
            &mut tl,
            EditCommand::AddEvent {
                track_id: v_top,
                event: TrackEvent {
                    id: adj_short,
                    adjustment: true,
                    start: 3.0,
                    duration: 5.0,
                    ..TrackEvent::default()
                },
            }
        ));
        assert!(h.execute(
            &mut tl,
            EditCommand::AddEvent {
                track_id: v_bot,
                event: TrackEvent {
                    id: adj_long,
                    adjustment: true,
                    start: 4.0,
                    duration: 21.0,
                    fade_in: 10.0,
                    fade_out: 10.0,
                    snap_offset: 19.0,
                    ..TrackEvent::default()
                },
            }
        ));
        assert!(h.execute(
            &mut tl,
            EditCommand::AddEvent {
                track_id: v_top,
                event: TrackEvent {
                    id: adj_locked,
                    adjustment: true,
                    locked: true,
                    start: 5.0,
                    duration: 4.0,
                    ..TrackEvent::default()
                },
            }
        ));

        // Even though adj_long ends at 25 s, project_span_end uses the media content end (14 s)!
        assert_eq!(project_span_end(&tl), 14.0);
        // Locked or plain media targets alone are not resizable
        assert!(!can_resize_to_project(
            &tl,
            &[(v_top, adj_locked), (v_bot, clip_v)]
        ));

        let targets = vec![
            (v_top, adj_short),
            (v_top, adj_short), // duplicate ignored
            (v_bot, adj_long),
            (v_top, adj_locked),
            (v_bot, clip_v),
        ];
        assert!(can_resize_to_project(&tl, &targets));
        let (cmd, count) = resize_to_project_command(&tl, &targets).unwrap();
        assert_eq!(count, 2);
        assert!(h.execute(&mut tl, cmd));

        let e_short = tl
            .track(v_top)
            .unwrap()
            .events
            .iter()
            .find(|e| e.id == adj_short)
            .unwrap();
        assert_eq!((e_short.start, e_short.duration), (0.0, 14.0));

        let e_long = tl
            .track(v_bot)
            .unwrap()
            .events
            .iter()
            .find(|e| e.id == adj_long)
            .unwrap();
        assert_eq!((e_long.start, e_long.duration), (0.0, 14.0));
        assert_eq!((e_long.fade_in, e_long.fade_out), (10.0, 4.0));
        assert_eq!(e_long.snap_offset, 14.0);

        let e_locked = tl
            .track(v_top)
            .unwrap()
            .events
            .iter()
            .find(|e| e.id == adj_locked)
            .unwrap();
        assert_eq!((e_locked.start, e_locked.duration), (5.0, 4.0));

        // Running again is a no-op (returns None)
        assert!(resize_to_project_command(&tl, &targets).is_none());

        // Single undo restores both adjustment events cleanly
        assert!(h.undo(&mut tl));
        let e_short = tl
            .track(v_top)
            .unwrap()
            .events
            .iter()
            .find(|e| e.id == adj_short)
            .unwrap();
        assert_eq!((e_short.start, e_short.duration), (3.0, 5.0));
        let e_long = tl
            .track(v_bot)
            .unwrap()
            .events
            .iter()
            .find(|e| e.id == adj_long)
            .unwrap();
        assert_eq!(
            (
                e_long.start,
                e_long.duration,
                e_long.fade_out,
                e_long.snap_offset
            ),
            (4.0, 21.0, 10.0, 19.0)
        );
    }
}
