//! Event groups and multi-event moves.
//!
//! A group is a set of events that share a [`GroupId`]: selecting one selects them all and a drag
//! moves them together. Like every compound edit, each function returns one [`Outcome`], so one
//! gesture is one undo step.

use crate::ops::{Draft, Outcome};
use crate::selection::EventRef;
use crate::timeline::{GroupId, Timeline, TIME_EPSILON};

/// `events` that exist in the timeline, in track and event order, without repeats.
fn existing(timeline: &Timeline, events: &[EventRef]) -> Vec<EventRef> {
    let mut found = Vec::new();
    for track in &timeline.tracks {
        for e in &track.events {
            if events.contains(&(track.id, e.id)) {
                found.push((track.id, e.id));
            }
        }
    }
    found
}

fn group_of(timeline: &Timeline, event: EventRef) -> Option<GroupId> {
    timeline
        .track(event.0)?
        .events
        .iter()
        .find(|e| e.id == event.1)?
        .group
}

fn members_of(timeline: &Timeline, group: GroupId) -> Vec<EventRef> {
    timeline
        .tracks
        .iter()
        .flat_map(|t| {
            t.events
                .iter()
                .filter(move |e| e.group == Some(group))
                .map(move |e| (t.id, e.id))
        })
        .collect()
}

/// `event` plus every other event of its group (just `event` when it has none).
pub fn group_members(timeline: &Timeline, event: EventRef) -> Vec<EventRef> {
    match group_of(timeline, event) {
        Some(group) => members_of(timeline, group),
        None => vec![event],
    }
}

/// `events` plus every member of their groups, without repeats.
pub fn expand_to_groups(timeline: &Timeline, events: &[EventRef]) -> Vec<EventRef> {
    let groups: Vec<GroupId> = events
        .iter()
        .filter_map(|&e| group_of(timeline, e))
        .collect();
    let mut all = existing(timeline, events);
    for group in groups {
        for member in members_of(timeline, group) {
            if !all.contains(&member) {
                all.push(member);
            }
        }
    }
    all
}

/// Groups the given events (at least two). Events that were in other groups leave them.
/// `None` when fewer than two exist or they already form exactly one group.
pub fn group_events(timeline: &mut Timeline, events: &[EventRef]) -> Option<Outcome> {
    let events = existing(timeline, events);
    if events.len() < 2 {
        return None;
    }
    if let Some(first) = group_of(timeline, events[0]) {
        let same = events.iter().all(|&e| group_of(timeline, e) == Some(first));
        if same && members_of(timeline, first).len() == events.len() {
            return None;
        }
    }
    let group = timeline.alloc_id();
    let mut draft = Draft::new(timeline);
    for &(track_id, event_id) in &events {
        if let Some(e) = draft
            .list(track_id)
            .and_then(|l| l.iter_mut().find(|e| e.id == event_id))
        {
            e.group = Some(group);
        }
    }
    draft.finish().map(|command| Outcome {
        command,
        touched: events,
    })
}

/// Dissolves every group that contains one of `events`. `None` when none of them is grouped.
pub fn ungroup_events(timeline: &Timeline, events: &[EventRef]) -> Option<Outcome> {
    let groups: Vec<GroupId> = existing(timeline, events)
        .into_iter()
        .filter_map(|e| group_of(timeline, e))
        .collect();
    if groups.is_empty() {
        return None;
    }
    let mut draft = Draft::new(timeline);
    let mut touched = Vec::new();
    for track in &timeline.tracks {
        let Some(list) = draft.list(track.id) else {
            continue;
        };
        for e in list.iter_mut() {
            if e.group.is_some_and(|g| groups.contains(&g)) {
                e.group = None;
                touched.push((track.id, e.id));
            }
        }
    }
    draft.finish().map(|command| Outcome { command, touched })
}

/// Moves every event of `events` by `delta` seconds, together, as one edit. The shift is limited
/// so that none of them starts before zero. `None` when nothing moves.
pub fn move_events(timeline: &Timeline, events: &[EventRef], delta: f64) -> Option<Outcome> {
    let events = existing(timeline, events);
    let earliest = events
        .iter()
        .filter_map(|&(t, id)| {
            timeline
                .track(t)?
                .events
                .iter()
                .find(|e| e.id == id)
                .map(|e| e.start)
        })
        .fold(f64::INFINITY, f64::min);
    if !earliest.is_finite() {
        return None;
    }
    let delta = delta.max(-earliest);
    if delta.abs() <= TIME_EPSILON {
        return None;
    }
    let mut draft = Draft::new(timeline);
    for &(track_id, event_id) in &events {
        if let Some(e) = draft
            .list(track_id)
            .and_then(|l| l.iter_mut().find(|e| e.id == event_id))
        {
            e.start += delta;
        }
    }
    draft.finish().map(|command| Outcome {
        command,
        touched: events,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::selection::tests::sample;

    fn r(tl: &Timeline, track: usize, event: usize) -> EventRef {
        (tl.tracks[track].id, tl.tracks[track].events[event].id)
    }

    fn run(tl: &mut Timeline, h: &mut History, outcome: Option<Outcome>) {
        assert!(h.execute(tl, outcome.expect("should change").command));
    }

    #[test]
    fn grouping_needs_two_events_and_one_undo_step_covers_all_tracks() {
        let mut tl = sample();
        let mut h = History::new();
        let (a, b) = (r(&tl, 0, 0), r(&tl, 1, 0));
        assert!(group_events(&mut tl, &[a]).is_none());
        let before = tl.clone();
        let out = group_events(&mut tl, &[a, b]);
        run(&mut tl, &mut h, out);
        assert_eq!(group_members(&tl, a), vec![a, b]);
        assert_eq!(group_members(&tl, b), vec![a, b]);

        assert!(h.undo(&mut tl));
        // Ids are never reused, so only the events are compared
        assert_eq!(tl.tracks, before.tracks);
    }

    #[test]
    fn grouping_an_existing_group_again_does_nothing() {
        let mut tl = sample();
        let mut h = History::new();
        let (a, b) = (r(&tl, 0, 0), r(&tl, 1, 0));
        let out = group_events(&mut tl, &[a, b]);
        run(&mut tl, &mut h, out);
        assert!(group_events(&mut tl, &[a, b]).is_none());
    }

    #[test]
    fn regrouping_takes_events_out_of_their_old_group() {
        let mut tl = sample();
        let mut h = History::new();
        let (a, b, c) = (r(&tl, 0, 0), r(&tl, 1, 0), r(&tl, 0, 1));
        let out = group_events(&mut tl, &[a, b]);
        run(&mut tl, &mut h, out);
        let out = group_events(&mut tl, &[b, c]);
        run(&mut tl, &mut h, out);
        let mut members = group_members(&tl, b);
        members.sort();
        let mut want = vec![b, c];
        want.sort();
        assert_eq!(members, want);
        assert_eq!(group_members(&tl, a), vec![a], "a is alone again");
    }

    #[test]
    fn expand_adds_the_members_of_every_touched_group() {
        let mut tl = sample();
        let mut h = History::new();
        let (a, b, c) = (r(&tl, 0, 0), r(&tl, 1, 0), r(&tl, 0, 1));
        let out = group_events(&mut tl, &[a, b]);
        run(&mut tl, &mut h, out);
        let mut expanded = expand_to_groups(&tl, &[a, c]);
        expanded.sort();
        let mut want = vec![a, b, c];
        want.sort();
        assert_eq!(expanded, want);
        assert_eq!(expand_to_groups(&tl, &[c]), vec![c]);
    }

    #[test]
    fn ungroup_dissolves_the_whole_group_and_ignores_loose_events() {
        let mut tl = sample();
        let mut h = History::new();
        let (a, b, c) = (r(&tl, 0, 0), r(&tl, 1, 0), r(&tl, 0, 1));
        assert!(ungroup_events(&tl, &[a]).is_none());
        let out = group_events(&mut tl, &[a, b]);
        run(&mut tl, &mut h, out);
        let out = ungroup_events(&tl, &[a, c]);
        run(&mut tl, &mut h, out);
        assert_eq!(group_members(&tl, a), vec![a]);
        assert_eq!(group_members(&tl, b), vec![b]);
        assert!(h.undo(&mut tl));
        assert_eq!(group_members(&tl, a).len(), 2);
    }

    #[test]
    fn moving_events_shifts_them_together_and_never_before_zero() {
        let mut tl = sample();
        let mut h = History::new();
        let (a, b) = (r(&tl, 0, 0), r(&tl, 1, 0));
        let starts = |tl: &Timeline| {
            [a, b].map(|(t, id)| {
                tl.track(t)
                    .unwrap()
                    .events
                    .iter()
                    .find(|e| e.id == id)
                    .unwrap()
                    .start
            })
        };
        let before = starts(&tl);
        let out = move_events(&tl, &[a, b], 2.5);
        run(&mut tl, &mut h, out);
        assert_eq!(starts(&tl), [before[0] + 2.5, before[1] + 2.5]);

        // Pulled far to the left, the earliest event stops at zero and the other keeps its distance
        let out = move_events(&tl, &[a, b], -100.0);
        run(&mut tl, &mut h, out);
        let after = starts(&tl);
        assert_eq!(after[0].min(after[1]), 0.0);
        assert!(((after[1] - after[0]) - (before[1] - before[0])).abs() < 1e-9);

        assert!(h.undo(&mut tl) && h.undo(&mut tl));
        assert_eq!(starts(&tl), before);
        assert!(move_events(&tl, &[a, b], 0.0).is_none());
        assert!(move_events(&tl, &[], 1.0).is_none());
    }

    #[test]
    fn splitting_a_group_cuts_it_in_two_groups() {
        use crate::ops::{split_at, Scope};
        let mut tl = sample();
        let mut h = History::new();
        let (a, b) = (r(&tl, 0, 1), r(&tl, 1, 1));
        let out = group_events(&mut tl, &[a, b]);
        run(&mut tl, &mut h, out);
        let original = group_of(&tl, a).unwrap();

        let out = split_at(&mut tl, 4.0, Scope::AllTracks);
        let touched = out.as_ref().unwrap().touched.clone();
        run(&mut tl, &mut h, out);

        // The left halves keep the group, the right halves form a new one
        assert_eq!(group_of(&tl, a), Some(original));
        assert_eq!(group_of(&tl, b), Some(original));
        let rights: Vec<Option<GroupId>> = touched.iter().map(|&e| group_of(&tl, e)).collect();
        assert_eq!(rights.len(), 2);
        assert!(rights[0].is_some() && rights[0] == rights[1]);
        assert_ne!(rights[0], Some(original));
    }

    #[test]
    fn a_pasted_copy_forms_its_own_group() {
        use crate::ops::{copy_events, paste, PasteMode};
        let mut tl = sample();
        let mut h = History::new();
        let (a, b) = (r(&tl, 0, 0), r(&tl, 1, 0));
        let out = group_events(&mut tl, &[a, b]);
        run(&mut tl, &mut h, out);
        let original = group_of(&tl, a).unwrap();

        let clip = copy_events(&tl, &[a, b]).unwrap();
        let (out, skipped) = paste(&mut tl, &clip, 20.0, 0, PasteMode::Overlay).unwrap();
        assert_eq!(skipped, 0);
        let pasted = out.touched.clone();
        assert!(h.execute(&mut tl, out.command));

        let groups: Vec<Option<GroupId>> = pasted.iter().map(|&e| group_of(&tl, e)).collect();
        assert!(groups[0].is_some() && groups[0] == groups[1]);
        assert_ne!(groups[0], Some(original));
        assert_eq!(
            members_of(&tl, original).len(),
            2,
            "the original group is untouched"
        );
    }
}
