//! Operations on a whole track: duplicating it and closing the gaps between its events. Each one
//! returns a single [`EditCommand`], so it is one undo step.

use crate::command::EditCommand;
use crate::timeline::{Timeline, TrackId, TIME_EPSILON};

/// A copy of the track: the same controls, effects, envelopes and events (with new ids, out of any
/// group), named "<name> (cópia)", not soloed, added after the last track. Returns the id of the copy
/// and the command that adds it. `None` when the track does not exist.
pub fn duplicate(timeline: &mut Timeline, track_id: TrackId) -> Option<(TrackId, EditCommand)> {
    let mut track = timeline.track(track_id)?.clone();
    track.id = timeline.alloc_id();
    track.name = format!("{} (cópia)", track.name);
    track.solo = false;
    for event in &mut track.events {
        event.id = timeline.alloc_id();
        event.group = None;
    }
    let id = track.id;
    Some((id, EditCommand::AddTrack { track }))
}

/// *Close gaps*: every event of the track moves left to meet the end of the one before it, so no
/// empty space is left between them. The first event stays where it is, an overlap (a crossfade)
/// is left as it is, and a locked event does not move (the ones after it close up to its end).
/// `None` when there is nothing to close.
pub fn close_gaps(timeline: &Timeline, track_id: TrackId) -> Option<EditCommand> {
    let track = timeline.track(track_id)?;
    let mut order: Vec<usize> = (0..track.events.len()).collect();
    order.sort_by(|&a, &b| track.events[a].start.total_cmp(&track.events[b].start));
    let mut commands = Vec::new();
    // `orig_end` and `new_end`: where the events seen so far end before and after the move;
    // `shift`: how far the one before moved (an event that overlaps it moves along with it)
    let mut ends: Option<(f64, f64)> = None;
    let mut shift = 0.0;
    for index in order {
        let event = &track.events[index];
        let start = match ends {
            Some(_) if event.locked => event.start,
            Some((orig_end, new_end)) if event.start > orig_end + TIME_EPSILON => {
                new_end.min(event.start)
            }
            Some(_) => event.start + shift,
            None => event.start,
        };
        shift = start - event.start;
        if shift.abs() > TIME_EPSILON {
            let old = event.geometry();
            commands.push(EditCommand::EditEvent {
                from_track: track_id,
                to_track: track_id,
                event_id: event.id,
                old,
                new: crate::timeline::EventGeometry { start, ..old },
            });
        }
        let (orig_end, new_end) = ends.unwrap_or((f64::MIN, f64::MIN));
        ends = Some((
            orig_end.max(event.start + event.duration),
            new_end.max(start + event.duration),
        ));
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
    use crate::timeline::{TrackEvent, TrackKind};

    fn track_with(events: &[(f64, f64)]) -> (Timeline, History, TrackId, Vec<u64>) {
        let mut tl = Timeline::new();
        let mut h = History::new();
        let track = tl.new_track(TrackKind::Audio);
        let tid = track.id;
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track }));
        let mut ids = Vec::new();
        for &(start, duration) in events {
            let id = tl.alloc_id();
            ids.push(id);
            assert!(h.execute(
                &mut tl,
                EditCommand::AddEvent {
                    track_id: tid,
                    event: TrackEvent {
                        id,
                        media_id: Some(1),
                        start,
                        duration,
                        ..TrackEvent::default()
                    },
                }
            ));
        }
        (tl, h, tid, ids)
    }

    fn starts(tl: &Timeline, tid: TrackId) -> Vec<f64> {
        let mut v: Vec<f64> = tl
            .track(tid)
            .unwrap()
            .events
            .iter()
            .map(|e| e.start)
            .collect();
        v.sort_by(f64::total_cmp);
        v
    }

    #[test]
    fn a_duplicate_has_the_same_content_with_new_ids_and_undoes_in_one_step() {
        let (mut tl, mut h, tid, ids) = track_with(&[(0.0, 2.0), (3.0, 1.0)]);
        tl.track_mut(tid).unwrap().solo = true;
        tl.track_mut(tid).unwrap().events[0].group = Some(7);
        let (new_id, command) = duplicate(&mut tl, tid).unwrap();
        assert!(h.execute(&mut tl, command));
        assert_eq!(tl.tracks.len(), 2);
        let copy = tl.track(new_id).unwrap();
        let original = tl.track(tid).unwrap();
        assert_eq!(copy.name, format!("{} (cópia)", original.name));
        assert!(
            !copy.solo && original.solo,
            "only the original keeps the solo"
        );
        assert_eq!(copy.events.len(), 2);
        assert!(copy
            .events
            .iter()
            .zip(&original.events)
            .all(|(c, o)| c.start == o.start && c.duration == o.duration && c.id != o.id));
        assert!(copy.events.iter().all(|e| e.group.is_none()));
        assert!(copy.events.iter().all(|e| !ids.contains(&e.id)));
        assert!(h.undo(&mut tl));
        assert_eq!(tl.tracks.len(), 1);
        assert!(duplicate(&mut tl, 999).is_none());
    }

    #[test]
    fn closing_gaps_moves_events_left_and_keeps_the_first_and_the_overlaps() {
        // 1..3, a gap, 5..6, a gap, 9..10 and an event that overlaps the previous one: 9.5..11
        let (mut tl, mut h, tid, _) = track_with(&[(1.0, 2.0), (5.0, 1.0), (9.0, 1.0), (9.5, 1.5)]);
        let command = close_gaps(&tl, tid).unwrap();
        assert!(h.execute(&mut tl, command));
        // 1..3, 3..4, 4..5; the overlapping event keeps its 0.5 s overlap with the one before it
        assert_eq!(starts(&tl, tid), vec![1.0, 3.0, 4.0, 4.5]);
        assert!(close_gaps(&tl, tid).is_none(), "nothing left to close");
        assert!(h.undo(&mut tl));
        assert_eq!(starts(&tl, tid), vec![1.0, 5.0, 9.0, 9.5]);
    }

    #[test]
    fn a_locked_event_stays_and_the_ones_after_it_close_up_to_its_end() {
        let (mut tl, mut h, tid, ids) = track_with(&[(0.0, 1.0), (4.0, 1.0), (8.0, 1.0)]);
        tl.track_mut(tid)
            .unwrap()
            .events
            .iter_mut()
            .find(|e| e.id == ids[1])
            .unwrap()
            .locked = true;
        let command = close_gaps(&tl, tid).unwrap();
        assert!(h.execute(&mut tl, command));
        // The first has nothing before it; the locked one stays at 4; the last closes to 5
        assert_eq!(starts(&tl, tid), vec![0.0, 4.0, 5.0]);
        assert!(close_gaps(&tl, 999).is_none());
    }
}
