//! Drag-and-copy: `Ctrl`+drag drops a copy of the dragged events where they land, and a drag with the
//! right button asks "Mover aqui / Copiar aqui / Embaralhar" ([`DragChoice`]). Pure rules; the
//! mouse handling is in `neoveg_ui`.

use std::collections::BTreeMap;

use crate::groups;
use crate::ops::{Draft, Outcome};
use crate::ripple;
use crate::selection::EventRef;
use crate::timeline::{EventId, GroupId, Timeline, TrackId, TIME_EPSILON};

/// What the right-button drag menu offers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DragChoice {
    /// Move the events to where they were dropped.
    Move,
    /// Leave the originals and drop copies there.
    Copy,
    /// Put the dragged event in the slot where it was dropped among the others of its track.
    Shuffle,
}

impl DragChoice {
    pub const ALL: [DragChoice; 3] = [DragChoice::Move, DragChoice::Copy, DragChoice::Shuffle];

    pub fn label(self) -> &'static str {
        match self {
            DragChoice::Move => "Mover aqui",
            DragChoice::Copy => "Copiar aqui",
            DragChoice::Shuffle => "Embaralhar",
        }
    }

    /// The choices that make sense: shuffling reorders one event of a plain sequence.
    pub fn available(dragged: usize) -> Vec<DragChoice> {
        Self::ALL
            .into_iter()
            .filter(|c| *c != DragChoice::Shuffle || dragged == 1)
            .collect()
    }
}

/// Copies `events` shifted by `delta` seconds (none starts before zero). A single event may land on
/// another track of its own kind (`to_track`). The copies get fresh ids and form groups of their
/// own; locked events are copied unlocked. `None` when nothing is copied. One undo step; the
/// outcome's `touched` are the new events.
pub fn copy_moved(
    timeline: &mut Timeline,
    events: &[EventRef],
    delta: f64,
    to_track: Option<TrackId>,
) -> Option<Outcome> {
    let mut sources = Vec::new();
    for track in &timeline.tracks {
        for ev in &track.events {
            if events.contains(&(track.id, ev.id)) {
                sources.push((track.id, ev.clone()));
            }
        }
    }
    if sources.is_empty() {
        return None;
    }
    let earliest = sources
        .iter()
        .map(|(_, e)| e.start)
        .fold(f64::INFINITY, f64::min);
    let delta = delta.max(-earliest);
    let target = match (to_track, sources.len()) {
        (Some(id), 1) => timeline
            .track(id)
            .zip(timeline.track(sources[0].0))
            .filter(|(to, from)| to.kind == from.kind)
            .map(|(to, _)| to.id),
        _ => None,
    };
    // The original group members that are copied together stay together as a new group
    let mut groups_map: BTreeMap<GroupId, GroupId> = BTreeMap::new();
    let mut draft = Draft::new(timeline);
    let mut touched = Vec::new();
    for (track_id, mut ev) in sources {
        let id = timeline.alloc_id();
        ev.id = id;
        ev.start = (ev.start + delta).max(0.0);
        ev.locked = false;
        if let Some(old) = ev.group {
            ev.group = Some(*groups_map.entry(old).or_insert_with(|| timeline.alloc_id()));
        }
        let dest = target.unwrap_or(track_id);
        draft.list(dest)?.push(ev);
        touched.push((dest, id));
    }
    draft.finish().map(|command| Outcome { command, touched })
}

/// Applies a menu choice to a drag that ended with the dragged event (`event`) at `new_start` on
/// `to_track`. `moved` is the whole selection that was dragged (or just the event). `None` when
/// the choice changes nothing.
pub fn apply_choice(
    timeline: &mut Timeline,
    choice: DragChoice,
    track: TrackId,
    event: EventId,
    moved: &[EventRef],
    new_start: f64,
    to_track: TrackId,
) -> Option<Outcome> {
    let origin = timeline
        .track(track)?
        .events
        .iter()
        .find(|e| e.id == event)?
        .clone();
    let delta = new_start - origin.start;
    let all: Vec<EventRef> = if moved.is_empty() {
        vec![(track, event)]
    } else {
        moved.to_vec()
    };
    match choice {
        DragChoice::Copy => copy_moved(timeline, &all, delta, Some(to_track)),
        DragChoice::Shuffle => ripple::shuffle_events(
            timeline,
            track,
            event,
            new_start + origin.duration / 2.0,
        ),
        DragChoice::Move => {
            if all.len() > 1 {
                groups::move_events(timeline, &all, delta)
            } else if to_track == track && delta.abs() <= TIME_EPSILON {
                None
            } else {
                let mut geometry = origin.geometry();
                geometry.start = new_start.max(0.0);
                Some(Outcome {
                    command: crate::command::EditCommand::EditEvent {
                        from_track: track,
                        to_track,
                        event_id: event,
                        old: origin.geometry(),
                        new: geometry,
                    },
                    touched: vec![(to_track, event)],
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::timeline::{TrackEvent, TrackKind};

    fn timeline() -> (Timeline, Vec<TrackId>, Vec<EventId>) {
        let mut tl = Timeline::new();
        let mut tracks = Vec::new();
        let mut ids = Vec::new();
        for lane in 0..2 {
            let mut track = tl.new_track(TrackKind::Audio);
            if lane == 0 {
                for (start, duration) in [(0.0, 2.0), (2.0, 3.0), (6.0, 1.0)] {
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
            }
            tracks.push(track.id);
            tl.tracks.push(track);
        }
        (tl, tracks, ids)
    }

    fn run(tl: &mut Timeline, outcome: Outcome) -> History {
        let mut h = History::new();
        assert!(h.execute(tl, outcome.command));
        h
    }

    #[test]
    fn a_copy_dropped_elsewhere_keeps_the_original() {
        let (mut tl, tracks, ids) = timeline();
        let out = copy_moved(&mut tl, &[(tracks[0], ids[0])], 10.0, Some(tracks[1])).unwrap();
        assert_eq!(out.touched.len(), 1);
        let mut h = run(&mut tl, out);
        assert_eq!(tl.tracks[0].events.len(), 3);
        assert_eq!(tl.tracks[1].events.len(), 1);
        let copy = &tl.tracks[1].events[0];
        assert_eq!((copy.start, copy.duration), (10.0, 2.0));
        assert_ne!(copy.id, ids[0]);
        assert!(h.undo(&mut tl));
        assert!(tl.tracks[1].events.is_empty());
    }

    #[test]
    fn copies_never_start_before_zero_and_unlock() {
        let (mut tl, tracks, ids) = timeline();
        tl.tracks[0].events[1].locked = true;
        let out = copy_moved(&mut tl, &[(tracks[0], ids[1])], -50.0, None).unwrap();
        run(&mut tl, out);
        let starts: Vec<f64> = tl.tracks[0].events.iter().map(|e| e.start).collect();
        assert_eq!(starts, vec![0.0, 0.0, 2.0, 6.0]);
        assert!(tl.tracks[0].events.iter().filter(|e| e.locked).count() == 1);
    }

    #[test]
    fn a_block_is_copied_together_with_new_groups() {
        let (mut tl, tracks, ids) = timeline();
        let group = tl.alloc_id();
        tl.tracks[0].events[0].group = Some(group);
        tl.tracks[0].events[1].group = Some(group);
        let sel = [(tracks[0], ids[0]), (tracks[0], ids[1])];
        let out = copy_moved(&mut tl, &sel, 20.0, Some(tracks[1])).unwrap();
        run(&mut tl, out);
        // A block stays on its own tracks and gets one new shared group
        assert_eq!(tl.tracks[0].events.len(), 5);
        let new: Vec<_> = tl.tracks[0].events.iter().filter(|e| e.start >= 20.0).collect();
        assert_eq!(new.len(), 2);
        assert_eq!(new[0].group, new[1].group);
        assert_ne!(new[0].group, Some(group));
    }

    #[test]
    fn menu_choices_move_copy_and_shuffle() {
        let (mut tl, tracks, ids) = timeline();
        assert_eq!(DragChoice::available(1).len(), 3);
        assert_eq!(DragChoice::available(2).len(), 2);
        // Move
        let out = apply_choice(&mut tl, DragChoice::Move, tracks[0], ids[2], &[], 8.0, tracks[0])
            .unwrap();
        run(&mut tl, out);
        assert_eq!(tl.tracks[0].events.last().unwrap().start, 8.0);
        // Copy
        let out =
            apply_choice(&mut tl, DragChoice::Copy, tracks[0], ids[0], &[], 30.0, tracks[0])
                .unwrap();
        run(&mut tl, out);
        assert_eq!(tl.tracks[0].events.len(), 4);
        // Shuffle: the first event goes after the second (drop mid-point past its midpoint)
        let (mut tl2, tracks2, ids2) = timeline();
        let out = apply_choice(&mut tl2, DragChoice::Shuffle, tracks2[0], ids2[0], &[], 4.0, tracks2[0])
            .unwrap();
        run(&mut tl2, out);
        assert_eq!(tl2.tracks[0].events[1].id, ids2[0]);
        // Dropping where it started does nothing
        assert!(apply_choice(&mut tl2, DragChoice::Move, tracks2[0], ids2[1], &[], 0.0, tracks2[0]).is_none());
    }
}
