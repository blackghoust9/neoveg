//! Where events go on the timeline relative to its cursor and to the end of a track: *append to the
//! end of the track*, *end at the cursor* (backtiming), *replace edit* (the clip of the source takes
//! the place of an event) and *match frame* / *reverse match* (from an event to its source frame and
//! back). Everything here is pure and returns one undoable command.

use crate::command::EditCommand;
use crate::edit::clamp_playback_rate;
use crate::ops::{Draft, Outcome};
use crate::selection::EventRef;
use crate::timeline::{Timeline, TrackId, TIME_EPSILON};

/// The time just after the last event of the track (`0.0` for an empty or unknown track).
pub fn track_end(timeline: &Timeline, track_id: TrackId) -> f64 {
    timeline
        .track(track_id)
        .map(|t| t.events.iter().map(|e| e.end()).fold(0.0, f64::max))
        .unwrap_or(0.0)
}

/// Where media appended to the end of `tracks` starts: after the last event of all of them, so a
/// picture and its sound begin together.
pub fn append_start(timeline: &Timeline, tracks: &[TrackId]) -> f64 {
    tracks
        .iter()
        .map(|&t| track_end(timeline, t))
        .fold(0.0, f64::max)
}

/// *End at cursor*: moves the unlocked events so that the latest end among them is at `cursor`,
/// keeping their distances. They never go before time zero (then the group is moved as far back as
/// it can go). `None` when nothing moves.
pub fn end_at_cursor(timeline: &Timeline, events: &[EventRef], cursor: f64) -> Option<Outcome> {
    let movable = crate::switches::unlocked(timeline, events);
    let mut ends = Vec::new();
    let mut earliest = f64::INFINITY;
    for track in &timeline.tracks {
        for e in &track.events {
            if movable.contains(&(track.id, e.id)) {
                ends.push(e.end());
                earliest = earliest.min(e.start);
            }
        }
    }
    let latest = ends.into_iter().fold(f64::NEG_INFINITY, f64::max);
    if !latest.is_finite() || !cursor.is_finite() {
        return None;
    }
    let delta = (cursor - latest).max(-earliest);
    if delta.abs() <= TIME_EPSILON {
        return None;
    }
    let mut draft = Draft::new(timeline);
    for &(track_id, event_id) in &movable {
        if let Some(e) = draft
            .list(track_id)
            .and_then(|l| l.iter_mut().find(|e| e.id == event_id))
        {
            e.start += delta;
        }
    }
    draft.finish().map(|command| Outcome {
        command,
        touched: movable,
    })
}

/// Why a replace edit changed less than asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplaceNote {
    /// The cursor frame of the source lines up with the cursor of the timeline.
    Aligned,
    /// The source starts too late to line the frames up: the event begins at the start of the source.
    ClampedToSourceStart,
}

/// *Replace edit*: the event keeps its place and its length and plays `media_id` instead, so that the
/// frame at `source_time` of the source is under `cursor` of the timeline. `cursor` has to be inside
/// the event. The event's speed stays; the old takes are dropped. `None` for a locked event, a cursor
/// outside it or a source time that is not a number.
pub fn replace_with_source(
    timeline: &Timeline,
    target: EventRef,
    media_id: u64,
    name: &str,
    source_time: f64,
    cursor: f64,
) -> Option<(Outcome, ReplaceNote)> {
    if crate::switches::is_locked(timeline, target) || !source_time.is_finite() {
        return None;
    }
    let event = timeline
        .track(target.0)?
        .events
        .iter()
        .find(|e| e.id == target.1)?;
    if cursor < event.start - TIME_EPSILON || cursor > event.end() + TIME_EPSILON {
        return None;
    }
    let rate = clamp_playback_rate(event.playback_rate);
    let wanted = source_time - (cursor - event.start) * rate;
    let (trim_in, note) = if wanted < 0.0 {
        (0.0, ReplaceNote::ClampedToSourceStart)
    } else {
        (wanted, ReplaceNote::Aligned)
    };
    let mut draft = Draft::new(timeline);
    let e = draft
        .list(target.0)?
        .iter_mut()
        .find(|e| e.id == target.1)?;
    e.media_id = Some(media_id);
    e.name = name.to_owned();
    e.trim_in = trim_in;
    e.takes.clear();
    draft.finish().map(|command| {
        (
            Outcome {
                command,
                touched: vec![target],
            },
            note,
        )
    })
}

/// *Match frame*: the media under `cursor` in the event, and the frame of it that is shown there
/// (seconds in the source). `None` for an event with no media or a cursor outside the event.
pub fn match_frame(timeline: &Timeline, event: EventRef, cursor: f64) -> Option<(u64, f64)> {
    let e = timeline
        .track(event.0)?
        .events
        .iter()
        .find(|e| e.id == event.1)?;
    let media = e.media_id?;
    if e.adjustment || cursor < e.start - TIME_EPSILON || cursor > e.end() + TIME_EPSILON {
        return None;
    }
    let rate = clamp_playback_rate(e.playback_rate);
    Some((media, e.trim_in + (cursor - e.start).max(0.0) * rate))
}

/// *Reverse match*: every event that shows `source_time` of `media_id`, with the time of the
/// timeline where it shows it, in time order.
pub fn reverse_match(timeline: &Timeline, media_id: u64, source_time: f64) -> Vec<(EventRef, f64)> {
    let mut found = Vec::new();
    for track in &timeline.tracks {
        for e in &track.events {
            if e.media_id != Some(media_id) || e.adjustment || e.duration <= 0.0 {
                continue;
            }
            let rate = clamp_playback_rate(e.playback_rate);
            let offset = (source_time - e.trim_in) / rate;
            if offset >= -TIME_EPSILON && offset <= e.duration + TIME_EPSILON {
                found.push(((track.id, e.id), e.start + offset.max(0.0)));
            }
        }
    }
    found.sort_by(|a, b| a.1.total_cmp(&b.1));
    found
}

/// *Move to the track above / below* (the numeric keypad's `8` and `2`): moves every unlocked
/// selected event `delta` tracks (`-1` is one track towards the top of the timeline) when the
/// destination exists, has the same kind and is not locked. One undo step; `None` when nothing can
/// move.
pub fn move_to_track(timeline: &Timeline, events: &[EventRef], delta: i32) -> Option<Outcome> {
    if delta == 0 {
        return None;
    }
    let movable = crate::switches::unlocked(timeline, events);
    let mut commands = Vec::new();
    let mut touched = Vec::new();
    for &(track_id, event_id) in &movable {
        let Some(index) = timeline.tracks.iter().position(|t| t.id == track_id) else {
            continue;
        };
        let Some(event) = timeline.tracks[index]
            .events
            .iter()
            .find(|e| e.id == event_id)
        else {
            continue;
        };
        let Some(target_index) = (index as i32 + delta)
            .try_into()
            .ok()
            .filter(|i: &usize| *i < timeline.tracks.len())
        else {
            continue;
        };
        let target = &timeline.tracks[target_index];
        if target.kind != timeline.tracks[index].kind || target.locked {
            continue;
        }
        commands.push(EditCommand::EditEvent {
            from_track: track_id,
            to_track: target.id,
            event_id,
            old: event.geometry(),
            new: event.geometry(),
        });
        touched.push((target.id, event_id));
    }
    (!commands.is_empty()).then(|| Outcome {
        command: EditCommand::Batch(commands),
        touched,
    })
}

/// Applies the command of an outcome to a timeline (for tests).
#[cfg(test)]
fn run(timeline: &mut Timeline, outcome: &Outcome) {
    let mut history = crate::command::History::default();
    history.execute(timeline, outcome.command.clone());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::{TrackEvent, TrackKind};

    fn timeline() -> (Timeline, TrackId, TrackId) {
        let mut tl = Timeline::new();
        let mut a = tl.new_track(TrackKind::Video);
        let mut b = tl.new_track(TrackKind::Audio);
        for (on_b, start, dur, media, trim) in [
            (false, 0.0, 4.0, 7u64, 10.0),
            (false, 6.0, 2.0, 7, 0.0),
            (true, 1.0, 3.0, 8, 0.0),
        ] {
            let id = tl.alloc_id();
            let track = if on_b { &mut b } else { &mut a };
            track.events.push(TrackEvent {
                id,
                name: format!("e{id}"),
                media_id: Some(media),
                start,
                duration: dur,
                trim_in: trim,
                ..TrackEvent::default()
            });
        }
        let (ia, ib) = (a.id, b.id);
        tl.tracks.extend([a, b]);
        (tl, ia, ib)
    }

    #[test]
    fn moving_to_the_track_above_or_below_keeps_the_time_and_respects_the_kind() {
        let mut tl = Timeline::new();
        let mut top = tl.new_track(TrackKind::Video);
        let mut mid = tl.new_track(TrackKind::Video);
        let mut low = tl.new_track(TrackKind::Audio);
        let event = TrackEvent {
            id: tl.alloc_id(),
            start: 1.0,
            duration: 2.0,
            media_id: Some(1),
            ..TrackEvent::default()
        };
        let id = event.id;
        mid.events.push(event);
        let (top_id, mid_id) = (top.id, mid.id);
        tl.tracks.extend([top, mid, low]);
        let up = move_to_track(&tl, &[(mid_id, id)], -1).expect("a track above");
        run(&mut tl, &up);
        assert!(tl.track(top_id).unwrap().events.iter().any(|e| e.id == id));
        let down = move_to_track(&tl, &[(top_id, id)], 1).expect("a track below");
        run(&mut tl, &down);
        assert!(tl.track(mid_id).unwrap().events.iter().any(|e| e.id == id));
        // A video event never lands on the audio track below
        assert!(move_to_track(&tl, &[(mid_id, id)], 1).is_none());
        let up = move_to_track(&tl, &[(mid_id, id)], -1).expect("a track above");
        run(&mut tl, &up);
        // Nor leaves the top of the timeline
        assert!(move_to_track(&tl, &[(top_id, id)], -1).is_none());
    }

    #[test]
    fn append_goes_after_the_last_event_of_every_track_asked() {
        let (tl, a, b) = timeline();
        assert_eq!(track_end(&tl, a), 8.0);
        assert_eq!(track_end(&tl, b), 4.0);
        assert_eq!(track_end(&tl, 999), 0.0);
        assert_eq!(append_start(&tl, &[b]), 4.0);
        assert_eq!(append_start(&tl, &[a, b]), 8.0);
        assert_eq!(append_start(&tl, &[]), 0.0);
    }

    #[test]
    fn events_end_at_the_cursor_keeping_their_distances() {
        let (mut tl, a, b) = timeline();
        let ea = tl.track(a).unwrap().events[0].id;
        let eb = tl.track(b).unwrap().events[0].id;
        // Ends are 4.0 and 4.0; the cursor is at 10 so both move by 6
        let out = end_at_cursor(&tl, &[(a, ea), (b, eb)], 10.0).unwrap();
        run(&mut tl, &out);
        assert_eq!(tl.track(a).unwrap().events[0].end(), 10.0);
        assert_eq!(tl.track(b).unwrap().events[0].end(), 10.0);
        // Going back is limited by time zero
        let out = end_at_cursor(&tl, &[(a, ea)], 1.0).unwrap();
        run(&mut tl, &out);
        assert_eq!(tl.track(a).unwrap().events[0].start, 0.0);
        // Already there: nothing to do
        let again = end_at_cursor(&tl, &[(a, ea)], tl.track(a).unwrap().events[0].end());
        assert!(again.is_none());
    }

    #[test]
    fn a_locked_event_does_not_end_at_the_cursor() {
        let (mut tl, a, _) = timeline();
        tl.track_mut(a).unwrap().events[0].locked = true;
        let id = tl.track(a).unwrap().events[0].id;
        assert!(end_at_cursor(&tl, &[(a, id)], 20.0).is_none());
    }

    #[test]
    fn replace_keeps_place_and_length_and_lines_up_the_cursor_frames() {
        let (mut tl, a, _) = timeline();
        let id = tl.track(a).unwrap().events[0].id;
        // Cursor 1 s into the event, source frame at 30 s: the event shows the source from 29 s
        let (out, note) = replace_with_source(&tl, (a, id), 99, "novo", 30.0, 1.0).unwrap();
        assert_eq!(note, ReplaceNote::Aligned);
        run(&mut tl, &out);
        let e = &tl.track(a).unwrap().events[0];
        assert_eq!((e.start, e.duration, e.media_id), (0.0, 4.0, Some(99)));
        assert_eq!((e.trim_in, e.name.as_str()), (29.0, "novo"));
        // A source frame earlier than the room needs: starts at the source start
        let (_, note) = replace_with_source(&tl, (a, id), 99, "novo", 0.5, 3.0).unwrap();
        assert_eq!(note, ReplaceNote::ClampedToSourceStart);
        // Cursor outside the event: refused
        assert!(replace_with_source(&tl, (a, id), 99, "novo", 5.0, 5.0).is_none());
    }

    #[test]
    fn replace_follows_the_speed_of_the_event() {
        let (mut tl, a, _) = timeline();
        let id = tl.track(a).unwrap().events[0].id;
        tl.track_mut(a).unwrap().events[0].playback_rate = 2.0;
        let (out, _) = replace_with_source(&tl, (a, id), 5, "x", 20.0, 1.0).unwrap();
        run(&mut tl, &out);
        assert_eq!(tl.track(a).unwrap().events[0].trim_in, 18.0);
    }

    #[test]
    fn match_frame_finds_the_source_frame_under_the_cursor() {
        let (tl, a, _) = timeline();
        let id = tl.track(a).unwrap().events[0].id;
        assert_eq!(match_frame(&tl, (a, id), 2.5), Some((7, 12.5)));
        assert_eq!(match_frame(&tl, (a, id), 5.0), None);
    }

    #[test]
    fn reverse_match_lists_the_events_that_show_a_source_frame() {
        let (tl, _, _) = timeline();
        // Source 7 at 11 s: the first event (10..14 at 0..4) shows it at 1 s
        let hits = reverse_match(&tl, 7, 11.0);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].1, 1.0);
        // Source 7 at 1 s: only the second event (0..2 of the source at 6..8)
        let hits = reverse_match(&tl, 7, 1.0);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].1, 7.0);
        assert!(reverse_match(&tl, 7, 50.0).is_empty());
        assert!(reverse_match(&tl, 42, 1.0).is_empty());
    }
}
