//! *Corte suave* (the idea of Morph Cut of the Premiere Pro reference, built on the VEGAS parts that
//! are already here): at a cut between two video events that touch, the earlier event is extended
//! into its unused media by a few frames so the two overlap, and the later one gets the
//! `warp_flow` transition (the optical-flow morph that *Smart Split* uses), which carries the
//! picture from one side of the cut to the other instead of jumping.
//!
//! Nothing moves on the timeline: the cut stays where it is and only the overlap is new, so the
//! rest of the project keeps its time. A cut whose earlier event has no media left past its end
//! gets a shorter overlap, or none (then the command says so and changes nothing).

use crate::command::EditCommand;
use crate::edit;
use crate::ops::WARP_FLOW_TRANSITION_ID;
use crate::selection::EventRef;
use crate::timeline::{EventId, Timeline, TrackEffect, TrackId, TrackKind};
use crate::transitions;

/// Frames of overlap asked for, as in *Smart Split*.
pub const OVERLAP_FRAMES: f64 = 5.0;
/// How far from `time` an edge may be to count as the cut, seconds.
pub const CUT_TOLERANCE: f64 = 1e-3;

/// The pair of events of the video track `track_id` that meet at `time` (each edge within
/// `tolerance` seconds): the one that ends there and the one that starts there.
pub fn find_cut(
    timeline: &Timeline,
    track_id: TrackId,
    time: f64,
    tolerance: f64,
) -> Option<(EventRef, EventRef)> {
    let track = timeline.track(track_id)?;
    if track.kind != TrackKind::Video {
        return None;
    }
    let left = track
        .events
        .iter()
        .filter(|e| (e.end() - time).abs() <= tolerance)
        .max_by(|a, b| a.start.total_cmp(&b.start))?;
    let right = track
        .events
        .iter()
        .filter(|e| e.id != left.id && (e.start - time).abs() <= tolerance)
        .min_by(|a, b| a.start.total_cmp(&b.start))?;
    Some(((track_id, left.id), (track_id, right.id)))
}

/// The edit that smooths the cut between `left` and `right` (both on the video track `track_id`,
/// `left` first): `left` grows by up to `overlap` seconds (never past its media, `left_source_len`
/// when known, nor more than half of `right`) and `right` gets the morph transition.
/// `None` when the events are not a cut, or there is nothing to change.
pub fn smooth_cut(
    timeline: &Timeline,
    track_id: TrackId,
    left_id: EventId,
    right_id: EventId,
    overlap: f64,
    left_source_len: Option<f64>,
) -> Option<EditCommand> {
    let track = timeline.track(track_id)?;
    if track.kind != TrackKind::Video || left_id == right_id {
        return None;
    }
    let left = track.events.iter().find(|e| e.id == left_id)?;
    let right = track.events.iter().find(|e| e.id == right_id)?;
    if left.adjustment || right.adjustment || left.media_id.is_none() || right.media_id.is_none() {
        return None;
    }
    // They have to touch or already overlap: the later one starts at or before the end of the first
    let gap = right.start - left.end();
    if gap > CUT_TOLERANCE || right.start < left.start {
        return None;
    }
    let mut commands = Vec::new();
    // Already overlapping: only the transition is missing; touching: make the overlap first
    if gap.abs() <= CUT_TOLERANCE {
        let wanted = overlap.min(right.duration * 0.5).max(0.0);
        let old = left.geometry();
        let new = edit::trim_right(old, left.end() + wanted, 0.0, left_source_len);
        if new.duration - old.duration > CUT_TOLERANCE {
            commands.push(EditCommand::EditEvent {
                from_track: track_id,
                to_track: track_id,
                event_id: left_id,
                old,
                new,
            });
        } else {
            return None;
        }
    }
    let warp = TrackEffect::new(WARP_FLOW_TRANSITION_ID);
    if let Some(command) = transitions::set(timeline, track_id, right_id, false, Some(warp)) {
        commands.push(command);
    }
    match commands.len() {
        0 => None,
        1 => commands.pop(),
        _ => Some(EditCommand::Batch(commands)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::timeline::TrackEvent;

    fn event(id: u64, start: f64, duration: f64, trim_in: f64) -> TrackEvent {
        TrackEvent {
            id,
            start,
            duration,
            trim_in,
            media_id: Some(1),
            ..Default::default()
        }
    }

    fn cut() -> (Timeline, TrackId) {
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Video);
        track.events = vec![event(10, 0.0, 4.0, 0.0), event(11, 4.0, 4.0, 10.0)];
        let id = track.id;
        tl.tracks = vec![track];
        (tl, id)
    }

    #[test]
    fn the_cut_is_found_where_one_event_ends_and_the_next_starts() {
        let (tl, track) = cut();
        let near = find_cut(&tl, track, 4.0, CUT_TOLERANCE);
        assert_eq!(near, Some(((track, 10), (track, 11))));
        assert_eq!(
            find_cut(&tl, track, 4.02, 0.02),
            near,
            "within the tolerance"
        );
        assert_eq!(find_cut(&tl, track, 4.02, CUT_TOLERANCE), None);
        assert_eq!(find_cut(&tl, track, 2.0, CUT_TOLERANCE), None);
        assert_eq!(
            find_cut(&tl, track, 8.0, CUT_TOLERANCE),
            None,
            "the end of the last one"
        );
    }

    #[test]
    fn a_cut_gets_an_overlap_into_the_media_left_and_the_morph_in_one_undo_step() {
        let (mut tl, track) = cut();
        let original = tl.clone();
        let mut history = History::new();
        let command = smooth_cut(&tl, track, 10, 11, 0.2, Some(100.0)).unwrap();
        assert!(history.execute(&mut tl, command));
        let events = &tl.track(track).unwrap().events;
        assert!(
            (events[0].end() - 4.2).abs() < 1e-9,
            "the first grew by the overlap"
        );
        assert_eq!(events[1].start, 4.0, "the cut did not move");
        assert_eq!(events[1].transition_in.as_ref().unwrap().kind, "warp_flow");
        assert!(history.undo(&mut tl));
        assert_eq!(tl, original);
    }

    #[test]
    fn the_overlap_is_limited_by_the_media_left_and_by_half_of_the_next_event() {
        let (tl, track) = cut();
        // The media of the first event ends 0.05 s after the cut
        let command = smooth_cut(&tl, track, 10, 11, 0.2, Some(4.05)).unwrap();
        let mut copy = tl.clone();
        assert!(History::new().execute(&mut copy, command));
        assert!((copy.track(track).unwrap().events[0].end() - 4.05).abs() < 1e-9);
        // No media left: nothing to do
        assert!(smooth_cut(&tl, track, 10, 11, 0.2, Some(4.0)).is_none());
        // A short next event: half of it at most
        let mut short = tl.clone();
        short.tracks[0].events[1].duration = 0.1;
        let command = smooth_cut(&short, track, 10, 11, 0.2, None).unwrap();
        assert!(History::new().execute(&mut short, command));
        assert!((short.track(track).unwrap().events[0].end() - 4.05).abs() < 1e-9);
    }

    #[test]
    fn events_that_do_not_meet_or_have_no_media_are_left_alone_and_an_overlap_only_gets_the_morph()
    {
        let (mut tl, track) = cut();
        tl.tracks[0].events[1].start = 5.0;
        assert!(smooth_cut(&tl, track, 10, 11, 0.2, None).is_none(), "a gap");
        assert!(
            smooth_cut(&tl, track, 11, 10, 0.2, None).is_none(),
            "wrong order"
        );
        let (mut tl, track) = cut();
        tl.tracks[0].events[0].media_id = None;
        assert!(smooth_cut(&tl, track, 10, 11, 0.2, None).is_none());
        // Already overlapping: the transition alone is the edit
        let (mut tl, track) = cut();
        tl.tracks[0].events[1].start = 3.9;
        let command = smooth_cut(&tl, track, 10, 11, 0.2, None).unwrap();
        assert!(!matches!(command, EditCommand::Batch(_)));
        assert!(History::new().execute(&mut tl, command));
        assert_eq!(tl.tracks[0].events[0].end(), 4.0, "the first did not grow");
        assert!(
            smooth_cut(&tl, track, 10, 11, 0.2, None).is_none(),
            "done already"
        );
    }
}
