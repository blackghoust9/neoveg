//! Ripple edits that do not come from a time selection: deleting events and closing the gap.

use crate::ops::{Draft, Outcome};
use crate::selection::EventRef;
use crate::timeline::{Timeline, TrackEvent, TrackId, TIME_EPSILON};

/// Removes the given events and moves every later event of the same track earlier by the length of
/// the gaps that were opened before it (overlapping removals count once). One undo step.
pub fn ripple_delete_events(timeline: &Timeline, events: &[EventRef]) -> Option<Outcome> {
    let events = &crate::switches::unlocked(timeline, events);
    let mut draft = Draft::new(timeline);
    let track_ids: Vec<TrackId> = events.iter().map(|(t, _)| *t).collect();
    let mut seen: Vec<TrackId> = Vec::new();
    for track_id in track_ids {
        if seen.contains(&track_id) {
            continue;
        }
        seen.push(track_id);
        let Some(list) = draft.list(track_id) else {
            continue;
        };
        // The spans being removed, merged where they overlap or touch
        let mut gaps: Vec<(f64, f64)> = list
            .iter()
            .filter(|e| events.contains(&(track_id, e.id)))
            .map(|e| (e.start, e.end()))
            .collect();
        gaps.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut merged: Vec<(f64, f64)> = Vec::new();
        for gap in gaps {
            match merged.last_mut() {
                Some(last) if gap.0 <= last.1 + TIME_EPSILON => last.1 = last.1.max(gap.1),
                _ => merged.push(gap),
            }
        }
        list.retain(|e| !events.contains(&(track_id, e.id)));
        for event in list.iter_mut() {
            event.start -= shift_for(event, &merged);
        }
    }
    draft.finish().map(|command| Outcome {
        command,
        touched: Vec::new(),
    })
}

/// How far `event` moves left: the total length of the removed spans that end at or before its start.
fn shift_for(event: &TrackEvent, gaps: &[(f64, f64)]) -> f64 {
    gaps.iter()
        .filter(|(_, end)| *end <= event.start + TIME_EPSILON)
        .map(|(start, end)| end - start)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::timeline::TrackKind;

    fn timeline() -> (Timeline, TrackId, Vec<u64>) {
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Audio);
        let mut ids = Vec::new();
        for (start, duration) in [(0.0, 2.0), (2.0, 3.0), (6.0, 1.0), (10.0, 2.0)] {
            let event = TrackEvent {
                id: tl.alloc_id(),
                start,
                duration,
                media_id: Some(1),
                ..TrackEvent::default()
            };
            ids.push(event.id);
            track.events.push(event);
        }
        let id = track.id;
        tl.tracks.push(track);
        (tl, id, ids)
    }

    fn starts(tl: &Timeline) -> Vec<f64> {
        tl.tracks[0].events.iter().map(|e| e.start).collect()
    }

    #[test]
    fn later_events_close_the_gap_of_a_deleted_one() {
        let (mut tl, track, ids) = timeline();
        let outcome = ripple_delete_events(&tl, &[(track, ids[1])]).unwrap();
        let mut history = History::new();
        assert!(history.execute(&mut tl, outcome.command));
        // The 3 s event is gone; the two after it move 3 s earlier
        assert_eq!(starts(&tl), vec![0.0, 3.0, 7.0]);
        assert!(history.undo(&mut tl));
        assert_eq!(starts(&tl), vec![0.0, 2.0, 6.0, 10.0]);
    }

    #[test]
    fn several_deletions_add_up_and_only_count_gaps_before_each_event() {
        let (tl, track, ids) = timeline();
        let outcome = ripple_delete_events(&tl, &[(track, ids[0]), (track, ids[2])]).unwrap();
        let mut copy = tl.clone();
        let mut history = History::new();
        assert!(history.execute(&mut copy, outcome.command));
        // Removed: 0-2 (2 s) and 6-7 (1 s). Event at 2.0 moves 2 s, the one at 10.0 moves 3 s
        assert_eq!(starts(&copy), vec![0.0, 7.0]);
    }

    #[test]
    fn other_tracks_and_unknown_events_are_left_alone() {
        let (mut tl, track, ids) = timeline();
        let mut other = tl.new_track(TrackKind::Video);
        other.events.push(TrackEvent {
            id: tl.alloc_id(),
            start: 8.0,
            duration: 1.0,
            ..TrackEvent::default()
        });
        tl.tracks.push(other);
        let outcome = ripple_delete_events(&tl, &[(track, ids[1])]).unwrap();
        let mut history = History::new();
        assert!(history.execute(&mut tl, outcome.command));
        assert_eq!(
            tl.tracks[1].events[0].start, 8.0,
            "the video track did not ripple"
        );
        assert!(ripple_delete_events(&tl, &[(track, 9999)]).is_none());
        assert!(ripple_delete_events(&tl, &[]).is_none());
    }
}
