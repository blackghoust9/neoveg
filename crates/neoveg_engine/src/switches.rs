//! Event switches: Mute, Lock and Invert Phase.
//!
//! A switch is a per-event on/off flag. Turning one on or off for a selection is a single
//! [`EditCommand::ReplaceEvents`], so it is one undo step. Lock is also the rule other edits
//! consult: a locked event cannot be moved, trimmed, split, grouped, cut or deleted
//! ([`unlocked`], [`is_locked`]). Neoveg's choice for mixed selections: an operation on several
//! events skips the locked ones and still applies to the others; the switches other than Lock
//! also skip locked events, so a locked event is frozen completely until it is unlocked.

use neoveg_common::channels::ChannelMode;

use crate::command::{EditCommand, TrackChange};
use crate::selection::EventRef;
use crate::timeline::{Timeline, TrackEvent, TrackId, TrackKind};

/// The per-event switches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Switch {
    Mute,
    Lock,
    /// Audio events only.
    InvertPhase,
    /// Events played at normal speed only (a stretched event cannot loop).
    Loop,
    /// Audio events only; the gain itself comes from [`set_normalize`].
    Normalize,
}

impl Switch {
    pub const ALL: [Switch; 5] = [
        Switch::Mute,
        Switch::Lock,
        Switch::InvertPhase,
        Switch::Loop,
        Switch::Normalize,
    ];

    /// Whether the switch is on for `event`.
    pub fn is_on(self, event: &TrackEvent) -> bool {
        match self {
            Switch::Mute => event.muted,
            Switch::Lock => event.locked,
            Switch::InvertPhase => event.invert_phase,
            Switch::Loop => event.looping,
            Switch::Normalize => event.normalize,
        }
    }

    fn set(self, event: &mut TrackEvent, on: bool) {
        match self {
            Switch::Mute => event.muted = on,
            Switch::Lock => event.locked = on,
            Switch::InvertPhase => event.invert_phase = on,
            Switch::Loop => event.looping = on,
            Switch::Normalize => {
                event.normalize = on;
                if !on {
                    event.normalize_gain = 1.0;
                }
            }
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
                Switch::InvertPhase | Switch::Normalize => {
                    !event.locked && track.kind == TrackKind::Audio
                }
                Switch::Loop => !event.locked && (event.playback_rate - 1.0).abs() < 1e-6,
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

/// Sets how the audio events among `events` use the channels of their media (Edit > Channels).
/// Video events and locked events are skipped. One undo step; `None` when nothing changes.
pub fn set_channels(
    timeline: &Timeline,
    events: &[EventRef],
    mode: ChannelMode,
) -> Option<EditCommand> {
    let mut changes = Vec::new();
    for track in timeline
        .tracks
        .iter()
        .filter(|t| t.kind == TrackKind::Audio)
    {
        let mut after = track.events.clone();
        let mut changed = false;
        for event in &mut after {
            if events.contains(&(track.id, event.id)) && !event.locked && event.channels != mode {
                event.channels = mode;
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

/// Turns Normalize on with the given gain for each listed audio event, or recalculates the gain of
/// events that already have it. One undo step; `None` when nothing changes.
pub fn set_normalize(timeline: &Timeline, gains: &[(EventRef, f32)]) -> Option<EditCommand> {
    let mut changes = Vec::new();
    for track in &timeline.tracks {
        let mut after = track.events.clone();
        let mut changed = false;
        for event in &mut after {
            let Some(&(_, gain)) = gains.iter().find(|(r, _)| *r == (track.id, event.id)) else {
                continue;
            };
            let allowed = !event.locked && track.kind == TrackKind::Audio;
            if allowed && (!event.normalize || (event.normalize_gain - gain).abs() > 1e-9) {
                event.normalize = true;
                event.normalize_gain = gain;
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

/// Longest name an event can be given.
pub const MAX_NAME_CHARS: usize = 120;

/// Renames `event` (one undo step). The name is trimmed and cut at [`MAX_NAME_CHARS`]; an empty name,
/// the same name or a locked event give `None`.
pub fn rename_event(timeline: &Timeline, event: EventRef, name: &str) -> Option<EditCommand> {
    let name: String = name.trim().chars().take(MAX_NAME_CHARS).collect();
    if name.is_empty() {
        return None;
    }
    let (track_id, event_id) = event;
    let track = timeline.track(track_id)?;
    let current = track.events.iter().find(|e| e.id == event_id)?;
    if current.locked || current.name == name {
        return None;
    }
    let mut after = track.events.clone();
    after.iter_mut().find(|e| e.id == event_id)?.name = name;
    Some(EditCommand::ReplaceEvents {
        changes: vec![TrackChange {
            track_id,
            before: track.events.clone(),
            after,
        }],
    })
}

/// The Resample Mode the selected video events share, or `None` when there is none or they differ.
pub fn resample_of(
    timeline: &Timeline,
    events: &[EventRef],
) -> Option<crate::timeline::ResampleMode> {
    let mut modes = events.iter().filter_map(|&(track_id, event_id)| {
        let track = timeline.track(track_id)?;
        if track.kind != TrackKind::Video {
            return None;
        }
        track
            .events
            .iter()
            .find(|e| e.id == event_id && !e.adjustment)
            .map(|e| e.resample)
    });
    let first = modes.next()?;
    modes.all(|m| m == first).then_some(first)
}

/// Sets the Resample Mode of the video events among `events` (one undo step). A locked event, an
/// adjustment event and an event of an audio track are left alone; `None` when nothing changes.
pub fn set_resample(
    timeline: &Timeline,
    events: &[EventRef],
    mode: crate::timeline::ResampleMode,
) -> Option<EditCommand> {
    let mut changes: Vec<crate::command::TrackChange> = Vec::new();
    let mut tracks: Vec<TrackId> = events.iter().map(|(t, _)| *t).collect();
    tracks.sort_unstable();
    tracks.dedup();
    for track_id in tracks {
        let Some(track) = timeline.track(track_id) else {
            continue;
        };
        if track.kind != TrackKind::Video {
            continue;
        }
        let mut after = track.events.clone();
        let mut changed = false;
        for event in &mut after {
            if events.contains(&(track_id, event.id))
                && !event.locked
                && !event.adjustment
                && event.resample != mode
            {
                event.resample = mode;
                changed = true;
            }
        }
        if changed {
            changes.push(crate::command::TrackChange {
                track_id,
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
    fn loop_needs_normal_speed_and_normalize_needs_audio() {
        let (mut tl, a, _, video) = timeline();
        assert!(toggle(&tl, &[a], Switch::Loop).is_some());
        assert!(toggle(&tl, &[video], Switch::Normalize).is_none());
        tl.tracks[0].events[0].playback_rate = 2.0;
        assert!(
            toggle(&tl, &[a], Switch::Loop).is_none(),
            "a stretched event cannot loop"
        );
    }

    #[test]
    fn normalize_stores_and_recalculates_the_gain_and_off_resets_it() {
        let (mut tl, a, b, _) = timeline();
        let command = set_normalize(&tl, &[(a, 2.0), (b, 0.5)]).unwrap();
        let mut history = run(&mut tl, command);
        let gain = |tl: &Timeline, r: EventRef| {
            tl.track(r.0)
                .unwrap()
                .events
                .iter()
                .find(|e| e.id == r.1)
                .unwrap()
                .normalize_gain
        };
        assert_eq!((gain(&tl, a), gain(&tl, b)), (2.0, 0.5));
        assert!(
            set_normalize(&tl, &[(a, 2.0)]).is_none(),
            "same gain: nothing to do"
        );
        let command = set_normalize(&tl, &[(a, 3.0)]).unwrap();
        history.execute(&mut tl, command);
        assert_eq!(gain(&tl, a), 3.0);
        let command = set(&tl, &[a], Switch::Normalize, false).unwrap();
        history.execute(&mut tl, command);
        assert_eq!(gain(&tl, a), 1.0);
        history.undo(&mut tl);
        assert_eq!(gain(&tl, a), 3.0);
    }

    #[test]
    fn a_locked_event_keeps_its_normalize_gain() {
        let (mut tl, a, _, _) = timeline();
        let command = set(&tl, &[a], Switch::Lock, true).unwrap();
        run(&mut tl, command);
        assert!(set_normalize(&tl, &[(a, 2.0)]).is_none());
    }

    #[test]
    fn channel_modes_apply_to_unlocked_audio_events_only_and_undo_in_one_step() {
        let (mut tl, a, b, video) = timeline();
        let command = set(&tl, &[b], Switch::Lock, true).unwrap();
        run(&mut tl, command);
        let mut history = History::default();
        let command = set_channels(&tl, &[a, b, video], ChannelMode::Swap).unwrap();
        history.execute(&mut tl, command);
        let mode = |tl: &Timeline, r: EventRef| {
            tl.track(r.0)
                .unwrap()
                .events
                .iter()
                .find(|e| e.id == r.1)
                .unwrap()
                .channels
        };
        assert_eq!(mode(&tl, a), ChannelMode::Swap);
        assert_eq!(mode(&tl, b), ChannelMode::Both, "locked");
        assert_eq!(mode(&tl, video), ChannelMode::Both, "video has no channels");
        assert!(
            set_channels(&tl, &[a], ChannelMode::Swap).is_none(),
            "already set"
        );
        history.undo(&mut tl);
        assert_eq!(mode(&tl, a), ChannelMode::Both);
    }

    #[test]
    fn old_projects_without_the_flags_load() {
        let json = r#"{"id":1,"name":"x","media_id":null,"start":0.0,"duration":1.0,
            "trim_in":0.0,"fade_in":0.0,"fade_out":0.0}"#;
        let event: TrackEvent = serde_json::from_str(json).unwrap();
        assert!(!event.muted && !event.locked && !event.invert_phase);
        assert!(!event.looping && !event.normalize && event.normalize_gain == 1.0);
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

    #[test]
    fn the_resample_mode_is_set_on_unlocked_video_events_only_in_one_undo_step() {
        use crate::command::History;
        use crate::timeline::ResampleMode;
        let mut tl = Timeline::new();
        let mut h = History::new();
        let mut refs = Vec::new();
        for kind in [TrackKind::Video, TrackKind::Video, TrackKind::Audio] {
            let track = tl.new_track(kind);
            let tid = track.id;
            assert!(h.execute(&mut tl, EditCommand::AddTrack { track }));
            let id = tl.alloc_id();
            let event = TrackEvent {
                id,
                media_id: Some(1),
                duration: 2.0,
                ..TrackEvent::default()
            };
            assert!(h.execute(
                &mut tl,
                EditCommand::AddEvent {
                    track_id: tid,
                    event
                }
            ));
            refs.push((tid, id));
        }
        // The second video event is locked
        tl.track_mut(refs[1].0).unwrap().events[0].locked = true;
        assert_eq!(resample_of(&tl, &refs[..1]), Some(ResampleMode::Project));
        let command = set_resample(&tl, &refs, ResampleMode::FrameBlend).unwrap();
        assert!(h.execute(&mut tl, command));
        assert_eq!(resample_of(&tl, &refs[..1]), Some(ResampleMode::FrameBlend));
        // Locked and audio events stay as they were
        assert_eq!(resample_of(&tl, &refs[1..2]), Some(ResampleMode::Project));
        assert!(
            resample_of(&tl, &refs[2..]).is_none(),
            "an audio event has none"
        );
        // Mixed selections have no common mode, and the same mode again changes nothing
        assert!(resample_of(&tl, &refs[..2]).is_none());
        assert!(set_resample(&tl, &refs[..1], ResampleMode::FrameBlend).is_none());
        assert!(h.undo(&mut tl));
        assert_eq!(resample_of(&tl, &refs[..1]), Some(ResampleMode::Project));
        // Old projects load with the default, and the default is not written
        let event = &tl.track(refs[0].0).unwrap().events[0];
        assert!(!serde_json::to_string(event).unwrap().contains("resample"));
        let mut blended = event.clone();
        blended.resample = ResampleMode::Disabled;
        let back: TrackEvent =
            serde_json::from_str(&serde_json::to_string(&blended).unwrap()).unwrap();
        assert_eq!(back.resample, ResampleMode::Disabled);
    }

    #[test]
    fn an_event_is_renamed_in_one_undo_step_unless_locked_or_the_name_is_empty() {
        use crate::command::History;
        let mut tl = Timeline::new();
        let mut h = History::new();
        let track = tl.new_track(TrackKind::Video);
        let tid = track.id;
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track }));
        let id = tl.alloc_id();
        let event = TrackEvent {
            id,
            name: "Plano 1".into(),
            duration: 2.0,
            ..TrackEvent::default()
        };
        assert!(h.execute(
            &mut tl,
            EditCommand::AddEvent {
                track_id: tid,
                event
            }
        ));
        let cmd = rename_event(&tl, (tid, id), "  Abertura  ").unwrap();
        assert!(h.execute(&mut tl, cmd));
        assert_eq!(tl.track(tid).unwrap().events[0].name, "Abertura");
        assert!(h.undo(&mut tl));
        assert_eq!(tl.track(tid).unwrap().events[0].name, "Plano 1");
        assert!(
            rename_event(&tl, (tid, id), "   ").is_none(),
            "no empty name"
        );
        assert!(
            rename_event(&tl, (tid, id), "Plano 1").is_none(),
            "nothing changes"
        );
        let long = "x".repeat(MAX_NAME_CHARS + 30);
        let Some(EditCommand::ReplaceEvents { changes }) = rename_event(&tl, (tid, id), &long)
        else {
            panic!("a rename");
        };
        assert_eq!(changes[0].after[0].name.chars().count(), MAX_NAME_CHARS);
        tl.track_mut(tid).unwrap().events[0].locked = true;
        assert!(
            rename_event(&tl, (tid, id), "Outro").is_none(),
            "a locked event is frozen"
        );
        assert!(rename_event(&tl, (tid, 999), "Outro").is_none());
    }
}
