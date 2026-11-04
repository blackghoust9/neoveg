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

/// Trims one edge of `event` to `new_time` and trims the same edge of every group member whose edge
/// sits at the same time, so a linked picture and sound stay in step. The shared edge stops where the
/// most restrictive member does (source limits, `min_len`). Members with a different edge time, or
/// locked ones, are left alone; an event without a group is a plain trim. One undo step; `None` when
/// nothing changes. `source_len` gives the length of an event's media when it is known.
pub fn trim_group_edge(
    timeline: &Timeline,
    event: EventRef,
    left_edge: bool,
    new_time: f64,
    min_len: f64,
    source_len: &dyn Fn(crate::timeline::TrackId, crate::timeline::EventId) -> Option<f64>,
) -> Option<Outcome> {
    let edge_of = |e: EventRef| {
        let ev = timeline.track(e.0)?.events.iter().find(|x| x.id == e.1)?;
        Some((ev.clone(), if left_edge { ev.start } else { ev.end() }))
    };
    let (this, this_edge) = edge_of(event)?;
    if this.locked {
        return None;
    }
    let members: Vec<(EventRef, crate::timeline::EventGeometry, bool)> =
        group_members(timeline, event)
            .into_iter()
            .filter_map(|m| {
                let (ev, edge) = edge_of(m)?;
                (m == event || (!ev.locked && (edge - this_edge).abs() < TIME_EPSILON))
                    .then(|| (m, ev.geometry(), ev.looping))
            })
            .collect();
    let trimmed =
        |geometry: crate::timeline::EventGeometry, looping: bool, m: EventRef, to: f64| {
            if left_edge {
                crate::edit::trim_left(geometry, to, min_len)
            } else {
                let len = if looping { None } else { source_len(m.0, m.1) };
                crate::edit::trim_right(geometry, to, min_len, len)
            }
        };
    // Each member clamps on its own; the shared edge is the most restrictive of them
    let reached: Vec<f64> = members
        .iter()
        .map(|&(m, g, looping)| {
            let t = trimmed(g, looping, m, new_time);
            if left_edge {
                t.start
            } else {
                t.end()
            }
        })
        .collect();
    // All reached edges lie on the same side of the original one; the tightest moved the least
    let edge = reached
        .iter()
        .copied()
        .min_by(|a, b| (a - this_edge).abs().total_cmp(&(b - this_edge).abs()))?;
    let mut draft = Draft::new(timeline);
    let mut touched = Vec::new();
    for &(m, g, looping) in &members {
        let new = trimmed(g, looping, m, edge);
        if new == g {
            continue;
        }
        if let Some(list) = draft.list(m.0) {
            if let Some(target) = list.iter_mut().find(|e| e.id == m.1) {
                target.set_geometry(new);
                touched.push(m);
            }
        }
    }
    draft.finish().map(|command| Outcome { command, touched })
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
    let events = crate::switches::unlocked(timeline, &existing(timeline, events));
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
    let groups: Vec<GroupId> = crate::switches::unlocked(timeline, &existing(timeline, events))
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

/// Takes the given events out of their groups and leaves the other members grouped (`U`; the
/// whole-group version is [`ungroup_events`]). A group left with a single member is dissolved, as
/// a group of one means nothing. Locked events keep their group. `None` when none of the events
/// is grouped.
pub fn remove_from_groups(timeline: &Timeline, events: &[EventRef]) -> Option<Outcome> {
    let leaving: Vec<EventRef> = crate::switches::unlocked(timeline, &existing(timeline, events))
        .into_iter()
        .filter(|&e| group_of(timeline, e).is_some())
        .collect();
    if leaving.is_empty() {
        return None;
    }
    // Groups that end up with fewer than two members are dissolved altogether
    let mut dissolved: Vec<GroupId> = Vec::new();
    for &e in &leaving {
        let Some(group) = group_of(timeline, e) else {
            continue;
        };
        let staying = members_of(timeline, group)
            .into_iter()
            .filter(|m| !leaving.contains(m))
            .count();
        if staying < 2 && !dissolved.contains(&group) {
            dissolved.push(group);
        }
    }
    let mut draft = Draft::new(timeline);
    let mut touched = Vec::new();
    for track in &timeline.tracks {
        let Some(list) = draft.list(track.id) else {
            continue;
        };
        for e in list.iter_mut() {
            let leaves = leaving.contains(&(track.id, e.id));
            let in_dissolved = e.group.is_some_and(|g| dissolved.contains(&g)) && !e.locked;
            if leaves || in_dissolved {
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
    let events = crate::switches::unlocked(timeline, &existing(timeline, events));
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
    fn removing_one_event_leaves_the_rest_of_the_group_together() {
        let mut tl = sample();
        let mut h = History::new();
        let (a, b, c) = (r(&tl, 0, 0), r(&tl, 1, 0), r(&tl, 0, 1));
        let out = group_events(&mut tl, &[a, b, c]);
        run(&mut tl, &mut h, out);
        assert!(remove_from_groups(&tl, &[]).is_none());
        let out = remove_from_groups(&tl, &[a]);
        run(&mut tl, &mut h, out);
        assert_eq!(group_members(&tl, a), vec![a], "a is loose");
        let mut rest = group_members(&tl, b);
        rest.sort();
        let mut want = vec![b, c];
        want.sort();
        assert_eq!(rest, want, "b and c are still a group");
        assert!(h.undo(&mut tl));
        assert_eq!(group_members(&tl, a).len(), 3, "one undo step restores it");
    }

    #[test]
    fn a_group_left_with_one_member_is_dissolved() {
        let mut tl = sample();
        let mut h = History::new();
        let (a, b) = (r(&tl, 0, 0), r(&tl, 1, 0));
        let out = group_events(&mut tl, &[a, b]);
        run(&mut tl, &mut h, out);
        let out = remove_from_groups(&tl, &[a]);
        run(&mut tl, &mut h, out);
        assert_eq!(group_members(&tl, a), vec![a]);
        assert_eq!(group_members(&tl, b), vec![b], "a group of one is no group");
    }

    #[test]
    fn a_locked_event_keeps_its_group() {
        let mut tl = sample();
        let mut h = History::new();
        let (a, b, c) = (r(&tl, 0, 0), r(&tl, 1, 0), r(&tl, 0, 1));
        let out = group_events(&mut tl, &[a, b, c]);
        run(&mut tl, &mut h, out);
        tl.tracks[0].events[0].locked = true;
        assert!(remove_from_groups(&tl, &[a]).is_none());
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

    #[test]
    fn a_grouped_pair_trims_together_and_a_lone_edge_trims_alone() {
        let mut tl = sample(); // both tracks: events at 0-2, 3-5, 6-8
        let mut h = History::new();
        let (a, b, c) = (r(&tl, 0, 0), r(&tl, 1, 0), r(&tl, 1, 1));
        let out = group_events(&mut tl, &[a, b, c]);
        run(&mut tl, &mut h, out);
        let no_limit = |_, _| None;

        let before = tl.clone();
        let out = trim_group_edge(&tl, a, false, 1.5, 0.04, &no_limit);
        run(&mut tl, &mut h, out);
        assert_eq!(tl.tracks[0].events[0].duration, 1.5);
        assert_eq!(
            tl.tracks[1].events[0].duration, 1.5,
            "same edge time: trimmed too"
        );
        assert_eq!(
            tl.tracks[1].events[1].duration, 2.0,
            "different edge: left alone"
        );
        assert!(h.undo(&mut tl));
        assert_eq!(tl.tracks, before.tracks, "one undo step");

        // Left edge: both move and keep their source aligned
        let out = trim_group_edge(&tl, b, true, 0.5, 0.04, &no_limit);
        run(&mut tl, &mut h, out);
        for track in 0..2 {
            let e = &tl.tracks[track].events[0];
            assert_eq!((e.start, e.duration, e.trim_in), (0.5, 1.5, 0.5));
        }
    }

    #[test]
    fn the_shared_edge_stops_at_the_shortest_source() {
        let mut tl = sample();
        let mut h = History::new();
        let (a, b) = (r(&tl, 0, 0), r(&tl, 1, 0));
        let out = group_events(&mut tl, &[a, b]);
        run(&mut tl, &mut h, out);
        // Track 0's media is only 3 s long, track 1's is unknown: both stop at 3.0
        let limit = |track, _| (track == tl.tracks[0].id).then_some(3.0);
        let out = trim_group_edge(&tl, b, false, 6.0, 0.04, &limit);
        run(&mut tl, &mut h, out);
        assert_eq!(tl.tracks[0].events[0].end(), 3.0);
        assert_eq!(tl.tracks[1].events[0].end(), 3.0);
    }

    #[test]
    fn locked_events_and_unchanged_edges_are_not_trimmed() {
        let mut tl = sample();
        let (a, b) = (r(&tl, 0, 0), r(&tl, 1, 0));
        let mut h = History::new();
        let out = group_events(&mut tl, &[a, b]);
        run(&mut tl, &mut h, out);
        let no_limit = |_, _| None;
        assert!(
            trim_group_edge(&tl, a, false, 2.0, 0.04, &no_limit).is_none(),
            "same edge"
        );
        tl.tracks[1].events[0].locked = true;
        let out = trim_group_edge(&tl, a, false, 1.0, 0.04, &no_limit);
        run(&mut tl, &mut h, out);
        assert_eq!(tl.tracks[0].events[0].duration, 1.0);
        assert_eq!(
            tl.tracks[1].events[0].duration, 2.0,
            "the locked member stays"
        );
        assert!(
            trim_group_edge(&tl, b, false, 1.0, 0.04, &no_limit).is_none(),
            "locked itself"
        );
    }
}
