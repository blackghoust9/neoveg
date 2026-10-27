//! Event switches: Mute, Lock and Invert Phase.
//!
//! A switch is a per-event on/off flag. Turning one on or off for a selection is a single
//! [`EditCommand::ReplaceEvents`], so it is one undo step. Lock is also the rule other edits
//! consult: a locked event cannot be moved, trimmed, split, grouped, cut or deleted
//! ([`unlocked`], [`is_locked`]). Neoveg's choice for mixed selections: an operation on several
//! events skips the locked ones and still applies to the others; the switches other than Lock
//! also skip locked events, so a locked event is frozen completely until it is unlocked.

use crate::command::{EditCommand, TrackChange};
use crate::selection::EventRef;
use crate::timeline::{Timeline, TrackEvent, TrackKind};

/// The per-event switches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Switch {
    Mute,
    Lock,
    /// Audio events only.
    InvertPhase,
}

impl Switch {
    pub const ALL: [Switch; 3] = [Switch::Mute, Switch::Lock, Switch::InvertPhase];

    /// Whether the switch is on for `event`.
    pub fn is_on(self, event: &TrackEvent) -> bool {
        match self {
            Switch::Mute => event.muted,
            Switch::Lock => event.locked,
            Switch::InvertPhase => event.invert_phase,
        }
    }

    fn set(self, event: &mut TrackEvent, on: bool) {
        match self {
            Switch::Mute => event.muted = on,
            Switch::Lock => event.locked = on,
            Switch::InvertPhase => event.invert_phase = on,
        }
    }
}

/// `true` when the event exists and is locked.
pub fn is_locked(timeline: &Timeline, event: EventRef) -> bool {
    timeline
        .track(event.0)
        .and_then(|t| t.events.iter().find(|e| e.id == event.1))
        .is_some_and(|e| e.locked)
}

/// The events of `events` that are not locked, in the same order.
pub fn unlocked(timeline: &Timeline, events: &[EventRef]) -> Vec<EventRef> {
    events
        .iter()
        .copied()
        .filter(|&e| !is_locked(timeline, e))
        .collect()
}

/// State of `switch` over a selection, for a checked menu item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchState {
    Off,
    On,
    /// Some events have it on and some off.
    Mixed,
}

/// Whether `switch` is on for all, none or some of the events that can take it.
pub fn state(timeline: &Timeline, events: &[EventRef], switch: Switch) -> SwitchState {
    let flags: Vec<bool> = eligible(timeline, events, switch)
        .iter()
        .filter_map(|&(t, id)| {
            timeline
                .track(t)?
                .events
                .iter()
                .find(|e| e.id == id)
                .map(|e| switch.is_on(e))
        })
        .collect();
    match (flags.iter().all(|&f| f), flags.iter().any(|&f| f)) {
        (true, true) => SwitchState::On,
        (false, false) | (true, false) => SwitchState::Off,
        (false, true) => SwitchState::Mixed,
    }
}

/// The events a switch can change: Invert Phase needs an audio event, and every switch other than
/// Lock skips locked events.
fn eligible(timeline: &Timeline, events: &[EventRef], switch: Switch) -> Vec<EventRef> {
    events
        .iter()
        .copied()
        .filter(|&(track_id, event_id)| {
            let Some(track) = timeline.track(track_id) else {
                return false;
            };
            let Some(event) = track.events.iter().find(|e| e.id == event_id) else {
                return false;
            };
            match switch {
                Switch::Lock => true,
                Switch::Mute => !event.locked,
                Switch::InvertPhase => !event.locked && track.kind == TrackKind::Audio,
            }
        })
        .collect()
}

/// Turns `switch` on for the events, or off when it is already on for all of them (a menu
/// toggle). One undo step; `None` when no event can take the switch or nothing changes.
pub fn toggle(timeline: &Timeline, events: &[EventRef], switch: Switch) -> Option<EditCommand> {
    let target = eligible(timeline, events, switch);
    let on = state(timeline, events, switch) != SwitchState::On;
    set(timeline, &target, switch, on)
}

/// Sets `switch` to `on` for the events that can take it. One undo step; `None` when nothing
/// changes.
pub fn set(
    timeline: &Timeline,
    events: &[EventRef],
    switch: Switch,
    on: bool,
) -> Option<EditCommand> {
    let target = eligible(timeline, events, switch);
    let mut changes = Vec::new();
    for track in &timeline.tracks {
        let mut after = track.events.clone();
        let mut changed = false;
        for event in &mut after {
            if target.contains(&(track.id, event.id)) && switch.is_on(event) != on {
                switch.set(event, on);
                changed = true;
            }
        }
        if changed {
            changes.push(TrackChange {
                track_id: track.id,
                before: track.events.clone(),
                after,
            });
        }
    }
    (!changes.is_empty()).then_some(EditCommand::ReplaceEvents { changes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;

    fn timeline() -> (Timeline, EventRef, EventRef, EventRef) {
        let mut tl = Timeline::new();
        let mut audio = tl.new_track(TrackKind::Audio);
        let mut video = tl.new_track(TrackKind::Video);
        let mut refs = Vec::new();
        for (list, start) in [(0, 0.0), (0, 5.0), (1, 0.0)] {
            let id = tl.alloc_id();
            let track = if list == 0 { &mut audio } else { &mut video };
            track.events.push(TrackEvent {
                id,
                start,
                duration: 4.0,
                ..TrackEvent::default()
            });
            refs.push((track.id, id));
        }
        tl.tracks.push(audio);
        tl.tracks.push(video);
        (tl, refs[0], refs[1], refs[2])
    }

    fn run(tl: &mut Timeline, command: EditCommand) -> History {
        let mut history = History::default();
        history.execute(tl, command);
        history
    }

    #[test]
    fn toggling_is_one_undo_step_over_several_events() {
        let (mut tl, a, b, _) = timeline();
        let command = toggle(&tl, &[a, b], Switch::Mute).unwrap();
        let mut history = run(&mut tl, command);
        assert!(tl.track(a.0).unwrap().events.iter().all(|e| e.muted));
        history.undo(&mut tl);
        assert!(tl.track(a.0).unwrap().events.iter().all(|e| !e.muted));
    }

    #[test]
    fn toggling_twice_turns_it_off() {
        let (mut tl, a, b, _) = timeline();
        let mut history = History::default();
        for _ in 0..2 {
            let command = toggle(&tl, &[a, b], Switch::Mute).unwrap();
            history.execute(&mut tl, command);
        }
        assert!(tl.track(a.0).unwrap().events.iter().all(|e| !e.muted));
    }

    #[test]
    fn mixed_selection_turns_everything_on_first() {
        let (mut tl, a, b, _) = timeline();
        let command = set(&tl, &[a], Switch::Mute, true).unwrap();
        run(&mut tl, command);
        assert_eq!(state(&tl, &[a, b], Switch::Mute), SwitchState::Mixed);
        let command = toggle(&tl, &[a, b], Switch::Mute).unwrap();
        run(&mut tl, command);
        assert_eq!(state(&tl, &[a, b], Switch::Mute), SwitchState::On);
    }

    #[test]
    fn invert_phase_skips_video_events() {
        let (tl, _, _, video) = timeline();
        assert!(toggle(&tl, &[video], Switch::InvertPhase).is_none());
        assert!(toggle(&tl, &[video], Switch::Mute).is_some());
    }

    #[test]
    fn a_locked_event_only_accepts_unlock() {
        let (mut tl, a, b, _) = timeline();
        let command = set(&tl, &[a], Switch::Lock, true).unwrap();
        run(&mut tl, command);
        assert!(is_locked(&tl, a));
        assert_eq!(unlocked(&tl, &[a, b]), vec![b]);
        assert!(toggle(&tl, &[a], Switch::Mute).is_none());
        assert!(toggle(&tl, &[a], Switch::InvertPhase).is_none());
        let command = toggle(&tl, &[a], Switch::Lock).unwrap();
        run(&mut tl, command);
        assert!(!is_locked(&tl, a));
    }

    #[test]
    fn setting_the_current_value_changes_nothing() {
        let (tl, a, _, _) = timeline();
        assert!(set(&tl, &[a], Switch::Mute, false).is_none());
    }

    fn locked_pair() -> (Timeline, EventRef, EventRef) {
        let (mut tl, a, b, _) = timeline();
        let command = set(&tl, &[a], Switch::Lock, true).unwrap();
        run(&mut tl, command);
        (tl, a, b)
    }

    fn count(tl: &Timeline) -> usize {
        tl.tracks.iter().map(|t| t.events.len()).sum()
    }

    #[test]
    fn a_locked_event_is_not_deleted_but_the_others_are() {
        let (tl, a, b) = locked_pair();
        assert!(crate::ops::delete_events(&tl, &[a]).is_none());
        let outcome = crate::ops::delete_events(&tl, &[a, b]).unwrap();
        let mut tl = tl;
        History::default().execute(&mut tl, outcome.command);
        assert!(is_locked(&tl, a));
        assert_eq!(count(&tl), 2);
        assert!(crate::ripple::ripple_delete_events(&tl, &[a]).is_none());
    }

    #[test]
    fn a_locked_event_is_not_moved_trimmed_split_or_grouped() {
        let (mut tl, a, b) = locked_pair();
        assert!(crate::groups::move_events(&tl, &[a], 1.0).is_none());
        assert!(crate::ops::trim_start_to(&tl, &[a], 2.0).is_none());
        assert!(crate::ops::trim_end_to(&tl, &[a], 2.0).is_none());
        let range = crate::selection::TimeRange::new(1.0, 2.0);
        assert!(crate::ops::trim_to_range(&tl, &[a], range).is_none());
        assert!(crate::ops::split_at(&mut tl, 2.0, crate::ops::Scope::Events(&[a])).is_none());
        assert!(crate::groups::group_events(&mut tl, &[a, b]).is_none());
        // An unlocked neighbour still splits, so the filter is per event
        assert!(crate::ops::split_at(&mut tl, 7.0, crate::ops::Scope::Events(&[b])).is_some());
    }

    #[test]
    fn removing_a_time_range_leaves_a_locked_event_whole() {
        let (mut tl, a, _) = locked_pair();
        let range = crate::selection::TimeRange::new(1.0, 2.0);
        let audio = [a.0];
        let scope = crate::ops::Scope::Tracks(&audio);
        let outcome = crate::ops::remove_range(&mut tl, range, scope, false);
        assert!(outcome.is_none(), "the only event in range is locked");
        assert!(is_locked(&tl, a));
    }

    #[test]
    fn a_muted_event_is_not_the_playing_one() {
        let (mut tl, a, _, _) = timeline();
        assert_eq!(tl.playing_event_at(a.0, 1.0).map(|e| e.id), Some(a.1));
        let command = set(&tl, &[a], Switch::Mute, true).unwrap();
        run(&mut tl, command);
        assert!(tl.playing_event_at(a.0, 1.0).is_none());
        assert!(
            tl.event_at(a.0, 1.0).is_some(),
            "it is still on the timeline"
        );
    }

    #[test]
    fn old_projects_without_the_flags_load() {
        let json = r#"{"id":1,"name":"x","media_id":null,"start":0.0,"duration":1.0,
            "trim_in":0.0,"fade_in":0.0,"fade_out":0.0}"#;
        let event: TrackEvent = serde_json::from_str(json).unwrap();
        assert!(!event.muted && !event.locked && !event.invert_phase);
        let saved = serde_json::to_string(&event).unwrap();
        assert!(!saved.contains("muted") && !saved.contains("locked"));
    }

    #[test]
    fn flags_survive_a_round_trip() {
        let event = TrackEvent {
            muted: true,
            locked: true,
            invert_phase: true,
            ..TrackEvent::default()
        };
        let back: TrackEvent =
            serde_json::from_str(&serde_json::to_string(&event).unwrap()).unwrap();
        assert_eq!(back, event);
    }
}
