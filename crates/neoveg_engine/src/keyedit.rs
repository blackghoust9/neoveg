//! Edits the reference map does with keys alone: moving events to the track above or below, and
//! turning a cut into a transition (a crossfade) and the other way round.
//!
//! The numeric-keypad keys of the reference map cannot be told from the number row here, so these
//! edits are commands with keys of their own.

use crate::command::EditCommand;
use crate::crossfade::find_crossfades;
use crate::edit;
use crate::selection::EventRef;
use crate::switches;
use crate::timeline::{Timeline, TrackEvent, TIME_EPSILON};

/// Moves the (unlocked) `events` to the track above (`up`) or below, as one undo step. Every event
/// has to find a track of its own kind there; otherwise nothing moves.
pub fn move_to_adjacent_track(
    timeline: &Timeline,
    events: &[EventRef],
    up: bool,
) -> Option<EditCommand> {
    let events = switches::unlocked(timeline, events);
    let mut commands = Vec::new();
    for &(track_id, event_id) in &events {
        let index = timeline.tracks.iter().position(|t| t.id == track_id)?;
        let target = if up { index.checked_sub(1)? } else { index + 1 };
        let to = timeline.tracks.get(target)?;
        if to.kind != timeline.tracks[index].kind {
            return None;
        }
        let geometry = timeline.tracks[index]
            .events
            .iter()
            .find(|e| e.id == event_id)?
            .geometry();
        commands.push(EditCommand::EditEvent {
            from_track: track_id,
            to_track: to.id,
            event_id,
            old: geometry,
            new: geometry,
        });
    }
    match commands.len() {
        0 => None,
        1 => commands.pop(),
        _ => Some(EditCommand::Batch(commands)),
    }
}

/// Turns the cut at the start of `event` into a crossfade of up to `length` seconds, by opening
/// the head of the event backwards over the end of the event before it. The overlap is as long as
/// the media in front of the event allows. `None` when the event has no neighbour touching it, or
/// no media left to open.
pub fn cut_to_transition(timeline: &Timeline, event: EventRef, length: f64) -> Option<EditCommand> {
    let track = timeline.track(event.0)?;
    let later = track.events.iter().find(|e| e.id == event.1)?;
    if later.locked {
        return None;
    }
    let earlier = touching_before(&track.events, later)?;
    let rate = edit::clamp_playback_rate(later.playback_rate);
    let room_in_media = later.trim_in.max(0.0) / rate;
    let room_in_earlier = earlier.duration * 0.9;
    let overlap = length
        .min(room_in_media)
        .min(room_in_earlier)
        .min(later.start);
    if !overlap.is_finite() || overlap <= TIME_EPSILON {
        return None;
    }
    let old = later.geometry();
    let mut new = old;
    new.start -= overlap;
    new.duration += overlap;
    new.trim_in -= overlap * rate;
    Some(EditCommand::EditEvent {
        from_track: event.0,
        to_track: event.0,
        event_id: event.1,
        old,
        new,
    })
}

/// Turns the crossfade at the start of `event` back into a cut, by taking the overlapped head off
/// the event. `None` when the event does not start a crossfade.
pub fn transition_to_cut(timeline: &Timeline, event: EventRef) -> Option<EditCommand> {
    let track = timeline.track(event.0)?;
    let later = track.events.iter().find(|e| e.id == event.1)?;
    if later.locked {
        return None;
    }
    let fade = find_crossfades(&track.events)
        .into_iter()
        .find(|c| c.later == later.id)?;
    let overlap = fade.length();
    if later.duration - overlap <= TIME_EPSILON {
        return None;
    }
    let rate = edit::clamp_playback_rate(later.playback_rate);
    let old = later.geometry();
    let mut new = old;
    new.start += overlap;
    new.duration -= overlap;
    new.trim_in += overlap * rate;
    new.fade_in = (old.fade_in - overlap).max(0.0);
    Some(EditCommand::EditEvent {
        from_track: event.0,
        to_track: event.0,
        event_id: event.1,
        old,
        new,
    })
}

/// The event that ends where `later` starts, on the same track.
fn touching_before<'a>(events: &'a [TrackEvent], later: &TrackEvent) -> Option<&'a TrackEvent> {
    events
        .iter()
        .filter(|e| e.id != later.id)
        .find(|e| (e.end() - later.start).abs() <= TIME_EPSILON)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::timeline::TrackKind;

    fn event(id: u64, start: f64, duration: f64, trim_in: f64) -> TrackEvent {
        TrackEvent {
            id,
            name: format!("e{id}"),
            media_id: Some(1),
            start,
            duration,
            trim_in,
            ..Default::default()
        }
    }

    fn timeline_with_a_cut(trim_in: f64) -> (Timeline, EventRef) {
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Video);
        track.events = vec![event(1, 0.0, 4.0, 0.0), event(2, 4.0, 4.0, trim_in)];
        let id = track.id;
        tl.tracks.push(track);
        (tl, (id, 2))
    }

    #[test]
    fn a_cut_becomes_a_crossfade_and_back_again() {
        let (mut tl, later) = timeline_with_a_cut(3.0);
        let mut h = History::new();
        assert!(find_crossfades(&tl.tracks[0].events).is_empty());

        let cmd = cut_to_transition(&tl, later, 1.0).expect("a cut with media in front");
        assert!(h.execute(&mut tl, cmd));
        let fades = find_crossfades(&tl.tracks[0].events);
        assert_eq!(fades.len(), 1);
        assert!((fades[0].length() - 1.0).abs() < 1e-9);
        // The picture does not shift: the event shows the same source at the same time
        let e = &tl.tracks[0].events[1];
        assert!((e.start - 3.0).abs() < 1e-9 && (e.trim_in - 2.0).abs() < 1e-9);
        assert!((e.end() - 8.0).abs() < 1e-9);

        let cmd = transition_to_cut(&tl, later).expect("a crossfade");
        assert!(h.execute(&mut tl, cmd));
        assert!(find_crossfades(&tl.tracks[0].events).is_empty());
        let e = &tl.tracks[0].events[1];
        assert!((e.start - 4.0).abs() < 1e-9 && (e.trim_in - 3.0).abs() < 1e-9);
    }

    #[test]
    fn the_overlap_is_limited_by_the_media_in_front_of_the_event() {
        let (tl, later) = timeline_with_a_cut(0.25);
        let Some(EditCommand::EditEvent { new, .. }) = cut_to_transition(&tl, later, 1.0) else {
            panic!("there are 0.25 s of media to open");
        };
        assert!((new.start - 3.75).abs() < 1e-9);

        // No media in front, no neighbour: nothing to turn
        let (tl, later) = timeline_with_a_cut(0.0);
        assert!(cut_to_transition(&tl, later, 1.0).is_none());
        let first = (later.0, 1);
        assert!(cut_to_transition(&tl, first, 1.0).is_none());
        assert!(transition_to_cut(&tl, later).is_none());
    }

    #[test]
    fn events_move_to_the_next_track_of_their_kind_only() {
        let mut tl = Timeline::new();
        let mut top = tl.new_track(TrackKind::Video);
        top.events = vec![event(1, 0.0, 2.0, 0.0)];
        let mut bottom = tl.new_track(TrackKind::Video);
        bottom.events = vec![];
        let mut audio = tl.new_track(TrackKind::Audio);
        audio.events = vec![];
        let (a, b, c) = (top.id, bottom.id, audio.id);
        tl.tracks = vec![top, bottom, audio];
        let mut h = History::new();

        assert!(move_to_adjacent_track(&tl, &[(a, 1)], true).is_none());
        let cmd = move_to_adjacent_track(&tl, &[(a, 1)], false).expect("down to the video track");
        assert!(h.execute(&mut tl, cmd));
        assert_eq!(tl.track(b).unwrap().events.len(), 1);
        // The track below is audio: a video event does not go there
        assert!(move_to_adjacent_track(&tl, &[(b, 1)], false).is_none());
        let _ = c;
        assert!(h.undo(&mut tl));
        assert_eq!(tl.track(a).unwrap().events.len(), 1);
    }
}
