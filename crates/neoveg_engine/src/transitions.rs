//! Video transitions of events: the pure builders of their undoable commands.
//!
//! A transition replaces the plain dissolve of a fade: it runs over the fade-in of the event, or over
//! the overlap with the event before it (`transition_in`), and over the fade-out (`transition_out`),
//! and follows the fade curve of the event. Like the effect chains, the engine does not know what a
//! transition draws; it stores the registry id and the values the user chose
//! (see [`TrackEffect`]).

use crate::command::EditCommand;
use crate::timeline::{EventId, Timeline, TrackEffect, TrackEvent, TrackId, TrackKind};

fn event(timeline: &Timeline, track_id: TrackId, event_id: EventId) -> Option<&TrackEvent> {
    let track = timeline.track(track_id)?;
    if track.kind != TrackKind::Video {
        return None;
    }
    track.events.iter().find(|e| e.id == event_id)
}

/// The transition of an event of a video track (`Some(None)` is a plain dissolve), or `None` when the
/// event does not exist or is not on a video track.
pub fn get(
    timeline: &Timeline,
    track_id: TrackId,
    event_id: EventId,
    outgoing: bool,
) -> Option<Option<&TrackEffect>> {
    let e = event(timeline, track_id, event_id)?;
    Some(if outgoing {
        e.transition_out.as_ref()
    } else {
        e.transition_in.as_ref()
    })
}

/// A command that puts `after` as the transition, or `None` when nothing changes.
pub fn set(
    timeline: &Timeline,
    track_id: TrackId,
    event_id: EventId,
    outgoing: bool,
    after: Option<TrackEffect>,
) -> Option<EditCommand> {
    let before = get(timeline, track_id, event_id, outgoing)?.cloned();
    (before != after).then_some(EditCommand::SetTransition {
        track_id,
        event_id,
        outgoing,
        before,
        after,
    })
}

/// Chooses the transition `kind` (its values start at the defaults; choosing the one already there
/// changes nothing).
pub fn choose(
    timeline: &Timeline,
    track_id: TrackId,
    event_id: EventId,
    outgoing: bool,
    kind: &str,
) -> Option<EditCommand> {
    let current = get(timeline, track_id, event_id, outgoing)?;
    if current.is_some_and(|t| t.kind == kind) {
        return None;
    }
    set(
        timeline,
        track_id,
        event_id,
        outgoing,
        Some(TrackEffect::new(kind)),
    )
}

/// Back to the plain dissolve.
pub fn clear(
    timeline: &Timeline,
    track_id: TrackId,
    event_id: EventId,
    outgoing: bool,
) -> Option<EditCommand> {
    set(timeline, track_id, event_id, outgoing, None)
}

/// Sets one value of the transition. Values are stored as given; the video layer clamps them.
pub fn set_param(
    timeline: &Timeline,
    track_id: TrackId,
    event_id: EventId,
    outgoing: bool,
    param: &str,
    value: f32,
) -> Option<EditCommand> {
    if !value.is_finite() {
        return None;
    }
    let mut next = get(timeline, track_id, event_id, outgoing)??.clone();
    next.params.insert(param.to_owned(), value);
    set(timeline, track_id, event_id, outgoing, Some(next))
}

/// Puts one value back to its default.
pub fn reset_param(
    timeline: &Timeline,
    track_id: TrackId,
    event_id: EventId,
    outgoing: bool,
    param: &str,
) -> Option<EditCommand> {
    let mut next = get(timeline, track_id, event_id, outgoing)??.clone();
    next.params.remove(param)?;
    set(timeline, track_id, event_id, outgoing, Some(next))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;

    fn setup() -> (Timeline, History, TrackId, EventId) {
        let mut tl = Timeline::new();
        let mut h = History::new();
        let track = tl.new_track(TrackKind::Video);
        let tid = track.id;
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track }));
        let eid = tl.alloc_id();
        let event = TrackEvent {
            id: eid,
            media_id: Some(1),
            duration: 8.0,
            fade_in: 1.0,
            ..TrackEvent::default()
        };
        assert!(h.execute(
            &mut tl,
            EditCommand::AddEvent {
                track_id: tid,
                event
            }
        ));
        (tl, h, tid, eid)
    }

    macro_rules! exec {
        ($tl:ident, $h:ident, $build:expr) => {{
            let cmd = $build;
            assert!($h.execute(&mut $tl, cmd.expect("command")));
        }};
    }

    #[test]
    fn a_transition_is_chosen_changed_cleared_and_each_change_is_one_undo_step() {
        let (mut tl, mut h, t, e) = setup();
        assert_eq!(get(&tl, t, e, false), Some(None), "a plain dissolve");
        exec!(tl, h, choose(&tl, t, e, false, "linear_wipe"));
        assert!(
            choose(&tl, t, e, false, "linear_wipe").is_none(),
            "already chosen"
        );
        exec!(tl, h, set_param(&tl, t, e, false, "angle", 90.0));
        assert_eq!(
            get(&tl, t, e, false).unwrap().unwrap().params["angle"],
            90.0
        );
        exec!(tl, h, reset_param(&tl, t, e, false, "angle"));
        assert!(
            reset_param(&tl, t, e, false, "angle").is_none(),
            "nothing to reset"
        );
        // Another transition starts from its own defaults
        exec!(tl, h, set_param(&tl, t, e, false, "softness", 0.4));
        exec!(tl, h, choose(&tl, t, e, false, "iris"));
        assert!(get(&tl, t, e, false).unwrap().unwrap().params.is_empty());
        exec!(tl, h, clear(&tl, t, e, false));
        assert_eq!(get(&tl, t, e, false), Some(None));

        assert!(h.undo(&mut tl));
        assert_eq!(get(&tl, t, e, false).unwrap().unwrap().kind, "iris");
        assert!(h.undo(&mut tl));
        assert_eq!(get(&tl, t, e, false).unwrap().unwrap().kind, "linear_wipe");
        assert!(h.redo(&mut tl));
        assert_eq!(get(&tl, t, e, false).unwrap().unwrap().kind, "iris");
    }

    #[test]
    fn the_in_and_the_out_transition_are_separate() {
        let (mut tl, mut h, t, e) = setup();
        exec!(tl, h, choose(&tl, t, e, false, "barn_door"));
        exec!(tl, h, choose(&tl, t, e, true, "clock_wipe"));
        assert_eq!(get(&tl, t, e, false).unwrap().unwrap().kind, "barn_door");
        assert_eq!(get(&tl, t, e, true).unwrap().unwrap().kind, "clock_wipe");
        exec!(tl, h, clear(&tl, t, e, true));
        assert_eq!(get(&tl, t, e, false).unwrap().unwrap().kind, "barn_door");
    }

    #[test]
    fn bad_requests_are_refused() {
        let (mut tl, mut h, t, e) = setup();
        assert!(
            set_param(&tl, t, e, false, "angle", 10.0).is_none(),
            "no transition yet"
        );
        exec!(tl, h, choose(&tl, t, e, false, "linear_wipe"));
        assert!(set_param(&tl, t, e, false, "angle", f32::NAN).is_none());
        assert!(choose(&tl, t, 999, false, "iris").is_none());
        assert!(choose(&tl, 999, e, false, "iris").is_none());
        let audio = tl.new_track(TrackKind::Audio);
        let aid = audio.id;
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track: audio }));
        let eid = tl.alloc_id();
        assert!(h.execute(
            &mut tl,
            EditCommand::AddEvent {
                track_id: aid,
                event: TrackEvent {
                    id: eid,
                    duration: 2.0,
                    ..TrackEvent::default()
                }
            }
        ));
        assert!(
            choose(&tl, aid, eid, false, "iris").is_none(),
            "audio events have none"
        );
    }

    #[test]
    fn a_split_leaves_the_new_event_without_a_transition_in_and_with_the_old_one_out() {
        let (mut tl, mut h, t, e) = setup();
        exec!(tl, h, choose(&tl, t, e, false, "iris"));
        exec!(tl, h, choose(&tl, t, e, true, "clock_wipe"));
        let new_event_id = tl.alloc_id();
        assert!(h.execute(
            &mut tl,
            EditCommand::SplitEvent {
                track_id: t,
                event_id: e,
                new_event_id,
                at: 4.0,
                original_duration: 8.0,
                original_fade_out: 0.0,
            }
        ));
        assert_eq!(get(&tl, t, e, false).unwrap().unwrap().kind, "iris");
        assert_eq!(get(&tl, t, new_event_id, false), Some(None));
        assert_eq!(
            get(&tl, t, new_event_id, true).unwrap().unwrap().kind,
            "clock_wipe"
        );
        assert!(h.undo(&mut tl));
        assert_eq!(get(&tl, t, e, true).unwrap().unwrap().kind, "clock_wipe");
    }

    #[test]
    fn projects_saved_before_transitions_load_and_a_transition_round_trips() {
        let event: TrackEvent = serde_json::from_str(
            r#"{"id":1,"name":"x","media_id":null,"start":0.0,"duration":1.0,"trim_in":0.0,"fade_in":0.0,"fade_out":0.0}"#,
        )
        .unwrap();
        assert!(event.transition_in.is_none() && event.transition_out.is_none());
        let json = serde_json::to_string(&event).unwrap();
        assert!(!json.contains("transition"));
        let mut with = event.clone();
        with.transition_in = Some(TrackEffect::new("iris"));
        let back: TrackEvent =
            serde_json::from_str(&serde_json::to_string(&with).unwrap()).unwrap();
        assert_eq!(back.transition_in, with.transition_in);
    }
}
