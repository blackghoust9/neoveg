//! Keyboard edge trimming: pick an edit edge, then nudge it a step at a time.
//!
//! Pure rules only; the UI chooses the step (one frame, one pixel) and the [`TrimMode`].

use crate::command::EditCommand;
use crate::edit;
use crate::ops::Outcome;
use crate::timeline::{EventGeometry, EventId, Timeline, TrackId, TIME_EPSILON};

/// Which end of an event an edge is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeSide {
    Start,
    End,
}

/// One edge of one event: what the keyboard trim mode has selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edge {
    pub track: TrackId,
    pub event: EventId,
    pub side: EdgeSide,
}

/// What nudging does to the selected edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrimMode {
    /// The edge moves; the source keeps its alignment.
    #[default]
    Normal,
    /// The media moves under the event, which stays where it is.
    Slip,
    /// The event moves on the timeline, its media keeps its place.
    Slide,
    /// The edge moves and the media is time-stretched or compressed to fit.
    Stretch,
    /// The cut shared with the neighbouring event moves (adjacent trim).
    Roll,
}

/// Time of `edge` on the timeline, or `None` when the event no longer exists.
pub fn edge_time(timeline: &Timeline, edge: Edge) -> Option<f64> {
    let event = timeline
        .track(edge.track)?
        .events
        .iter()
        .find(|e| e.id == edge.event)?;
    Some(match edge.side {
        EdgeSide::Start => event.start,
        EdgeSide::End => event.end(),
    })
}

/// Every edge of the unlocked events on `tracks`, ordered by time. At the same time an end comes
/// before a start, so a cut between two events lists the outgoing edge first.
pub fn edges(timeline: &Timeline, tracks: &[TrackId]) -> Vec<(f64, Edge)> {
    let mut list = Vec::new();
    for (row, track) in timeline.tracks.iter().enumerate() {
        if !tracks.is_empty() && !tracks.contains(&track.id) {
            continue;
        }
        for event in track.events.iter().filter(|e| !e.locked) {
            let edge = |side| Edge {
                track: track.id,
                event: event.id,
                side,
            };
            list.push((event.start, row, 1u8, edge(EdgeSide::Start)));
            list.push((event.end(), row, 0u8, edge(EdgeSide::End)));
        }
    }
    list.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.2.cmp(&b.2)).then(a.1.cmp(&b.1)));
    list.into_iter().map(|(t, _, _, e)| (t, e)).collect()
}

/// The edge after (or before) `current`; with no current edge, the first one after (or before) the
/// edit cursor. Stays on `current` at the ends of the list. `tracks` empty means every track.
pub fn step_edge(
    timeline: &Timeline,
    tracks: &[TrackId],
    current: Option<Edge>,
    forward: bool,
    cursor: f64,
) -> Option<Edge> {
    let list = edges(timeline, tracks);
    if list.is_empty() {
        return None;
    }
    if let Some(index) = current.and_then(|c| list.iter().position(|(_, e)| *e == c)) {
        let next = if forward {
            (index + 1).min(list.len() - 1)
        } else {
            index.saturating_sub(1)
        };
        return Some(list[next].1);
    }
    let found = if forward {
        list.iter().find(|(t, _)| *t > cursor + TIME_EPSILON)
    } else {
        list.iter().rev().find(|(t, _)| *t < cursor - TIME_EPSILON)
    };
    found
        .or_else(|| if forward { list.last() } else { list.first() })
        .map(|(_, e)| *e)
}

/// Moves `edge` by `delta` seconds (negative = earlier) in `mode`. `source_len` gives the length of
/// an event's media when it is known (it limits growing and slipping). One undo step; `None` when
/// nothing changes, the event is locked or missing, or there is no neighbour to roll with.
pub fn nudge(
    timeline: &Timeline,
    edge: Edge,
    delta: f64,
    mode: TrimMode,
    min_len: f64,
    source_len: &dyn Fn(TrackId, EventId) -> Option<f64>,
) -> Option<Outcome> {
    let track = timeline.track(edge.track)?;
    let event = track.events.iter().find(|e| e.id == edge.event)?;
    if event.locked || !delta.is_finite() || delta == 0.0 {
        return None;
    }
    let old = event.geometry();
    let time = match edge.side {
        EdgeSide::Start => old.start,
        EdgeSide::End => old.end(),
    };
    let len = if event.looping {
        None
    } else {
        source_len(edge.track, edge.event)
    };
    let change = |event_id, old: EventGeometry, new: EventGeometry| EditCommand::EditEvent {
        from_track: edge.track,
        to_track: edge.track,
        event_id,
        old,
        new,
    };
    let command = match mode {
        TrimMode::Roll => {
            let touching = |other: &crate::timeline::TrackEvent| match edge.side {
                EdgeSide::End => (other.start - old.end()).abs() < TIME_EPSILON,
                EdgeSide::Start => (other.end() - old.start).abs() < TIME_EPSILON,
            };
            let other = track
                .events
                .iter()
                .find(|e| e.id != edge.event && touching(e))?;
            if other.locked {
                return None;
            }
            let other_old = other.geometry();
            let (left, right, left_len) = match edge.side {
                EdgeSide::End => (old, other_old, len),
                EdgeSide::Start => (
                    other_old,
                    old,
                    if other.looping {
                        None
                    } else {
                        source_len(edge.track, other.id)
                    },
                ),
            };
            let (new_left, new_right) = edit::roll(left, right, time + delta, min_len, left_len);
            let (this_new, other_new) = match edge.side {
                EdgeSide::End => (new_left, new_right),
                EdgeSide::Start => (new_right, new_left),
            };
            let mut commands = Vec::new();
            if this_new != old {
                commands.push(change(edge.event, old, this_new));
            }
            if other_new != other_old {
                commands.push(change(other.id, other_old, other_new));
            }
            match commands.len() {
                0 => return None,
                1 => commands.remove(0),
                _ => EditCommand::Batch(commands),
            }
        }
        _ => {
            let new = match (mode, edge.side) {
                (TrimMode::Normal, EdgeSide::Start) => edit::trim_left(old, time + delta, min_len),
                (TrimMode::Normal, EdgeSide::End) => {
                    edit::trim_right(old, time + delta, min_len, len)
                }
                (TrimMode::Stretch, EdgeSide::Start) => {
                    edit::stretch_left(old, time + delta, min_len)
                }
                (TrimMode::Stretch, EdgeSide::End) => {
                    edit::stretch_right(old, time + delta, min_len)
                }
                (TrimMode::Slip, _) => edit::slip(old, delta, len),
                (TrimMode::Slide, _) => edit::slide(old, delta, len),
                (TrimMode::Roll, _) => return None,
            };
            if new == old {
                return None;
            }
            change(edge.event, old, new)
        }
    };
    Some(Outcome {
        command,
        touched: vec![(edge.track, edge.event)],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::timeline::{TrackEvent, TrackKind};

    const FRAME: f64 = 0.04;

    /// Audio track: A = 0-4 (trim_in 0), B = 4-8 (trim_in 2), C = 10-12; B's media is 12 s long.
    fn timeline() -> (Timeline, TrackId, [EventId; 3]) {
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Audio);
        let mut ids = [0; 3];
        for (i, (start, duration, trim_in)) in [(0.0, 4.0, 0.0), (4.0, 4.0, 2.0), (10.0, 2.0, 0.0)]
            .into_iter()
            .enumerate()
        {
            let event = TrackEvent {
                id: tl.alloc_id(),
                start,
                duration,
                trim_in,
                media_id: Some(1),
                ..TrackEvent::default()
            };
            ids[i] = event.id;
            track.events.push(event);
        }
        let id = track.id;
        tl.tracks.push(track);
        (tl, id, ids)
    }

    fn no_limit(_: TrackId, _: EventId) -> Option<f64> {
        None
    }

    /// Executes an edit built from another timeline.
    fn exec(tl: &mut Timeline, outcome: Option<Outcome>) -> History {
        let mut history = History::new();
        assert!(history.execute(tl, outcome.expect("an edit").command));
        history
    }

    /// Builds the edit from the timeline as it is, then executes it.
    fn run(tl: &mut Timeline, build: impl FnOnce(&Timeline) -> Option<Outcome>) -> History {
        let outcome = build(tl);
        let mut history = History::new();
        assert!(history.execute(tl, outcome.expect("an edit").command));
        history
    }

    #[test]
    fn edges_are_ordered_and_a_cut_lists_the_outgoing_edge_first() {
        let (tl, track, ids) = timeline();
        let list = edges(&tl, &[track]);
        assert_eq!(list.len(), 6);
        let at_four: Vec<_> = list
            .iter()
            .filter(|(t, _)| *t == 4.0)
            .map(|(_, e)| *e)
            .collect();
        assert_eq!(at_four[0].side, EdgeSide::End);
        assert_eq!((at_four[0].event, at_four[1].event), (ids[0], ids[1]));
        assert!(list.windows(2).all(|w| w[0].0 <= w[1].0));
    }

    #[test]
    fn stepping_starts_at_the_cursor_and_stays_at_the_ends() {
        let (tl, track, ids) = timeline();
        let first = step_edge(&tl, &[track], None, true, 4.5).unwrap();
        assert_eq!(
            (first.event, first.side),
            (ids[1], EdgeSide::End),
            "next edge after 4.5 is B's end"
        );
        let back = step_edge(&tl, &[track], None, false, 4.5).unwrap();
        assert_eq!((back.event, back.side), (ids[1], EdgeSide::Start));
        let next = step_edge(&tl, &[track], Some(first), true, 0.0).unwrap();
        assert_eq!((next.event, next.side), (ids[2], EdgeSide::Start));
        let last = step_edge(&tl, &[track], None, true, 99.0).unwrap();
        assert_eq!(
            (last.event, last.side),
            (ids[2], EdgeSide::End),
            "past the last edge: the last one"
        );
        assert_eq!(step_edge(&tl, &[track], Some(last), true, 0.0), Some(last));
        assert!(step_edge(&Timeline::new(), &[], None, true, 0.0).is_none());
    }

    #[test]
    fn normal_nudge_trims_an_edge_and_keeps_the_source_aligned() {
        let (tl, track, ids) = timeline();
        let end = Edge {
            track,
            event: ids[2],
            side: EdgeSide::End,
        };
        let mut shorter = tl.clone();
        let mut history = exec(
            &mut shorter,
            nudge(&tl, end, -FRAME, TrimMode::Normal, FRAME, &no_limit),
        );
        let c = &shorter.tracks[0].events[2];
        assert!((c.duration - (2.0 - FRAME)).abs() < 1e-9 && c.start == 10.0);
        assert!(history.undo(&mut shorter));
        assert_eq!(shorter, tl, "one undo step restores the event");

        let start = Edge {
            track,
            event: ids[2],
            side: EdgeSide::Start,
        };
        let mut later = tl.clone();
        exec(
            &mut later,
            nudge(&tl, start, 0.5, TrimMode::Normal, FRAME, &no_limit),
        );
        let c = &later.tracks[0].events[2];
        assert!((c.start - 10.5).abs() < 1e-9 && (c.trim_in - 0.5).abs() < 1e-9);
        assert!((c.end() - 12.0).abs() < 1e-9, "the other edge stays put");
    }

    #[test]
    fn growing_stops_at_the_end_of_the_source() {
        let (tl, track, ids) = timeline();
        let end = Edge {
            track,
            event: ids[1],
            side: EdgeSide::End,
        };
        let limit = |_: TrackId, id: EventId| (id == ids[1]).then_some(6.5);
        // B shows source 2..6; the media ends at 6.5, so the edge can grow by 0.5 s at most
        let out = nudge(&tl, end, 3.0, TrimMode::Normal, FRAME, &limit).unwrap();
        let mut copy = tl.clone();
        exec(&mut copy, Some(out));
        assert!((copy.tracks[0].events[1].duration - 4.5).abs() < 1e-9);
    }

    #[test]
    fn stretch_changes_the_rate_and_not_the_source_span() {
        let (mut tl, track, ids) = timeline();
        let end = Edge {
            track,
            event: ids[0],
            side: EdgeSide::End,
        };
        run(&mut tl, |t| {
            nudge(t, end, 1.0, TrimMode::Stretch, FRAME, &no_limit)
        });
        let a = &tl.tracks[0].events[0];
        assert!((a.duration - 5.0).abs() < 1e-9);
        assert!((a.playback_rate - 0.8).abs() < 1e-9);
        assert_eq!(a.trim_in, 0.0);
    }

    #[test]
    fn roll_moves_the_cut_for_both_events_in_one_undo_step() {
        let (mut tl, track, ids) = timeline();
        let end_of_a = Edge {
            track,
            event: ids[0],
            side: EdgeSide::End,
        };
        let before = tl.clone();
        let mut h = run(&mut tl, |t| {
            nudge(t, end_of_a, 1.0, TrimMode::Roll, FRAME, &no_limit)
        });
        let (a, b) = (&tl.tracks[0].events[0], &tl.tracks[0].events[1]);
        assert!((a.end() - 5.0).abs() < 1e-9 && (b.start - 5.0).abs() < 1e-9);
        assert!((b.trim_in - 3.0).abs() < 1e-9);
        assert!(h.undo(&mut tl));
        assert_eq!(tl, before, "both events come back in one undo step");
        // From the other side of the same cut
        let start_of_b = Edge {
            track,
            event: ids[1],
            side: EdgeSide::Start,
        };
        run(&mut tl, |t| {
            nudge(t, start_of_b, -2.0, TrimMode::Roll, FRAME, &no_limit)
        });
        assert!((tl.tracks[0].events[1].start - 2.0).abs() < 1e-9);
        // C touches nothing: nothing to roll with
        let start_of_c = Edge {
            track,
            event: ids[2],
            side: EdgeSide::Start,
        };
        assert!(nudge(&tl, start_of_c, 1.0, TrimMode::Roll, FRAME, &no_limit).is_none());
    }

    #[test]
    fn slip_and_slide_nudge_the_whole_event() {
        let (mut tl, track, ids) = timeline();
        let edge = Edge {
            track,
            event: ids[1],
            side: EdgeSide::Start,
        };
        run(&mut tl, |t| {
            nudge(t, edge, 0.5, TrimMode::Slip, FRAME, &no_limit)
        });
        let b = &tl.tracks[0].events[1];
        assert_eq!((b.start, b.duration), (4.0, 4.0));
        assert!((b.trim_in - 2.5).abs() < 1e-9);
        run(&mut tl, |t| {
            nudge(t, edge, 0.5, TrimMode::Slide, FRAME, &no_limit)
        });
        let b = &tl.tracks[0].events[1];
        assert!((b.start - 4.5).abs() < 1e-9 && (b.trim_in - 3.0).abs() < 1e-9);
    }

    #[test]
    fn locked_missing_or_zero_nudges_are_not_edits() {
        let (mut tl, track, ids) = timeline();
        let edge = Edge {
            track,
            event: ids[0],
            side: EdgeSide::End,
        };
        assert!(nudge(&tl, edge, 0.0, TrimMode::Normal, FRAME, &no_limit).is_none());
        assert!(nudge(&tl, edge, f64::NAN, TrimMode::Normal, FRAME, &no_limit).is_none());
        let ghost = Edge {
            track,
            event: 9999,
            side: EdgeSide::End,
        };
        assert!(nudge(&tl, ghost, 1.0, TrimMode::Normal, FRAME, &no_limit).is_none());
        assert!(edge_time(&tl, ghost).is_none());
        assert_eq!(edge_time(&tl, edge), Some(4.0));
        tl.tracks[0].events[0].locked = true;
        assert!(nudge(&tl, edge, 1.0, TrimMode::Normal, FRAME, &no_limit).is_none());
        assert!(
            edges(&tl, &[track]).iter().all(|(_, e)| e.event != ids[0]),
            "locked events have no edges"
        );
    }
}
