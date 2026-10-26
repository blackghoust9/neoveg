//! Compound editing operations: delete, cut, copy, paste, trim and split over selections and time
//! ranges. Every function is pure (it only reads the timeline and allocates ids) and returns one
//! [`EditCommand`], so a menu action or shortcut is exactly one undo step.

use std::collections::BTreeMap;

use crate::command::{EditCommand, TrackChange};
use crate::selection::{EventRef, TimeRange};
use crate::timeline::{EventId, GroupId, Timeline, TrackEvent, TrackId, TrackKind, TIME_EPSILON};

/// A command plus the events the UI should select once it ran (pasted or created events).
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub command: EditCommand,
    pub touched: Vec<EventRef>,
}

/// Which events an operation may touch.
#[derive(Debug, Clone, Copy)]
pub enum Scope<'a> {
    AllTracks,
    Tracks(&'a [TrackId]),
    Events(&'a [EventRef]),
}

impl Scope<'_> {
    fn includes(&self, track: TrackId, event: EventId) -> bool {
        match self {
            Scope::AllTracks => true,
            Scope::Tracks(ids) => ids.contains(&track),
            Scope::Events(refs) => refs.contains(&(track, event)),
        }
    }
}

/// One event copied to the clipboard, positioned relative to the clip origin.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipItem {
    /// Track distance from the first track of the copied group.
    pub track_offset: usize,
    pub kind: TrackKind,
    /// `start` is relative to the clip origin; ids are reassigned on paste.
    pub event: TrackEvent,
}

/// Events held by cut/copy until they are pasted.
#[derive(Debug, Clone, PartialEq)]
pub struct Clipboard {
    pub items: Vec<ClipItem>,
    /// Time covered by the clip (what Paste Insert pushes later events by).
    pub length: f64,
}

/// How a paste treats the events already on the track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteMode {
    /// New events lie on top of the old ones (overlaps crossfade when enabled).
    Overlay,
    /// Events under the cursor are split and everything later moves by the clip length.
    Insert,
}

/// How repeated pastes are spaced.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RepeatSpacing {
    /// One copy right after the other.
    EndToEnd,
    /// The starts of the copies are this many seconds apart.
    Every(f64),
}

// ------------------------------------------------------------------------------------------
// Event-level helpers
// ------------------------------------------------------------------------------------------

/// The part of `ev` inside `range`, or `None` when nothing is left. A fade survives only on an edge
/// that was not cut.
fn clip_to(ev: &TrackEvent, range: TimeRange) -> Option<TrackEvent> {
    let start = ev.start.max(range.start);
    let end = ev.end().min(range.end);
    if end - start <= TIME_EPSILON {
        return None;
    }
    let mut out = ev.clone();
    out.start = start;
    out.duration = end - start;
    out.trim_in = ev.trim_in + (start - ev.start);
    if start > ev.start + TIME_EPSILON {
        out.fade_in = 0.0;
    }
    if end < ev.end() - TIME_EPSILON {
        out.fade_out = 0.0;
    }
    out.fade_in = out.fade_in.clamp(0.0, out.duration);
    out.fade_out = out
        .fade_out
        .clamp(0.0, (out.duration - out.fade_in).max(0.0));
    Some(out)
}

/// Splits every event of `list` that strictly contains `at` and passes `pick`; returns the new
/// (right-hand) events.
fn split_list(
    list: &mut Vec<TrackEvent>,
    at: f64,
    alloc: &mut impl FnMut() -> u64,
    pick: impl Fn(&TrackEvent) -> bool,
) -> Vec<EventId> {
    let mut created = Vec::new();
    let mut result = Vec::with_capacity(list.len() + 1);
    for ev in list.drain(..) {
        if pick(&ev) && at > ev.start + TIME_EPSILON && at < ev.end() - TIME_EPSILON {
            let left = clip_to(&ev, TimeRange::new(ev.start, at));
            let right = clip_to(&ev, TimeRange::new(at, ev.end()));
            if let (Some(left), Some(mut right)) = (left, right) {
                right.id = alloc();
                created.push(right.id);
                result.push(left);
                result.push(right);
                continue;
            }
        }
        result.push(ev);
    }
    *list = result;
    created
}

/// Per-track copies of the event lists, edited and then diffed into one command.
pub(crate) struct Draft {
    before: BTreeMap<TrackId, Vec<TrackEvent>>,
    after: BTreeMap<TrackId, Vec<TrackEvent>>,
}

impl Draft {
    pub(crate) fn new(timeline: &Timeline) -> Self {
        let before: BTreeMap<_, _> = timeline
            .tracks
            .iter()
            .map(|t| (t.id, t.events.clone()))
            .collect();
        Self {
            after: before.clone(),
            before,
        }
    }

    pub(crate) fn list(&mut self, track: TrackId) -> Option<&mut Vec<TrackEvent>> {
        self.after.get_mut(&track)
    }

    /// The command for every track whose events changed, or `None` when nothing changed.
    pub(crate) fn finish(self) -> Option<EditCommand> {
        let mut changes = Vec::new();
        for (track_id, mut after) in self.after {
            after.sort_by(|a, b| a.start.total_cmp(&b.start));
            let before = self.before.get(&track_id).cloned().unwrap_or_default();
            let mut sorted_before = before.clone();
            sorted_before.sort_by(|a, b| a.start.total_cmp(&b.start));
            if sorted_before != after {
                changes.push(TrackChange {
                    track_id,
                    before,
                    after,
                });
            }
        }
        (!changes.is_empty()).then_some(EditCommand::ReplaceEvents { changes })
    }
}

fn outcome(command: Option<EditCommand>, touched: Vec<EventRef>) -> Option<Outcome> {
    command.map(|command| Outcome { command, touched })
}

// ------------------------------------------------------------------------------------------
// Delete, cut and copy
// ------------------------------------------------------------------------------------------

/// Removes the given events.
pub fn delete_events(timeline: &Timeline, events: &[EventRef]) -> Option<Outcome> {
    let mut draft = Draft::new(timeline);
    for &(track_id, event_id) in events {
        if let Some(list) = draft.list(track_id) {
            list.retain(|e| e.id != event_id);
        }
    }
    outcome(draft.finish(), Vec::new())
}

/// Copies whole events. Their relative positions and tracks are kept.
pub fn copy_events(timeline: &Timeline, events: &[EventRef]) -> Option<Clipboard> {
    let mut found = Vec::new();
    for (index, track) in timeline.tracks.iter().enumerate() {
        for ev in &track.events {
            if events.contains(&(track.id, ev.id)) {
                found.push((index, track.kind, ev.clone()));
            }
        }
    }
    clipboard_from(found, None)
}

/// Copies the part of each event inside `range` (the parts are cut at the range edges).
pub fn copy_range(timeline: &Timeline, range: TimeRange, scope: Scope) -> Option<Clipboard> {
    if range.is_empty() {
        return None;
    }
    let mut found = Vec::new();
    for (index, track) in timeline.tracks.iter().enumerate() {
        for ev in &track.events {
            if scope.includes(track.id, ev.id) {
                if let Some(part) = clip_to(ev, range) {
                    found.push((index, track.kind, part));
                }
            }
        }
    }
    clipboard_from(found, Some(range))
}

fn clipboard_from(
    found: Vec<(usize, TrackKind, TrackEvent)>,
    range: Option<TimeRange>,
) -> Option<Clipboard> {
    let first_track = found.iter().map(|(i, _, _)| *i).min()?;
    let origin = range.map_or_else(
        || {
            found
                .iter()
                .map(|(_, _, e)| e.start)
                .fold(f64::INFINITY, f64::min)
        },
        |r| r.start,
    );
    let end = range.map_or_else(
        || {
            found
                .iter()
                .map(|(_, _, e)| e.end())
                .fold(f64::NEG_INFINITY, f64::max)
        },
        |r| r.end,
    );
    let items = found
        .into_iter()
        .map(|(index, kind, mut event)| {
            event.start -= origin;
            ClipItem {
                track_offset: index - first_track,
                kind,
                event,
            }
        })
        .collect();
    Some(Clipboard {
        items,
        length: end - origin,
    })
}

/// Removes the part of the events inside `range` (events crossing an edge are split). With `ripple`
/// everything after the range moves earlier by its length, closing the gap; ripple only applies
/// when the scope is not an explicit event list.
pub fn remove_range(
    timeline: &mut Timeline,
    range: TimeRange,
    scope: Scope,
    ripple: bool,
) -> Option<Outcome> {
    if range.is_empty() {
        return None;
    }
    let ripple = ripple && !matches!(scope, Scope::Events(_));
    let mut draft = Draft::new(timeline);
    let track_ids: Vec<TrackId> = timeline.tracks.iter().map(|t| t.id).collect();
    let mut alloc = || timeline.alloc_id();
    for track_id in track_ids {
        let Some(list) = draft.list(track_id) else {
            continue;
        };
        let mut result = Vec::with_capacity(list.len() + 1);
        for ev in list.drain(..) {
            if !scope.includes(track_id, ev.id) {
                result.push(ev);
                continue;
            }
            if ev.end() <= range.start + TIME_EPSILON {
                result.push(ev);
            } else if ev.start >= range.end - TIME_EPSILON {
                let mut moved = ev;
                if ripple {
                    moved.start -= range.len();
                }
                result.push(moved);
            } else {
                if let Some(left) = clip_to(&ev, TimeRange::new(ev.start, range.start)) {
                    result.push(left);
                }
                if let Some(mut right) = clip_to(&ev, TimeRange::new(range.end, ev.end())) {
                    if ripple {
                        right.start -= range.len();
                    }
                    if ev.start < range.start - TIME_EPSILON {
                        right.id = alloc();
                    }
                    result.push(right);
                }
            }
        }
        *list = result;
    }
    outcome(draft.finish(), Vec::new())
}

// ------------------------------------------------------------------------------------------
// Paste
// ------------------------------------------------------------------------------------------

/// Where each clipboard item lands: `(track id, event)` with a fresh id, plus the number of items
/// that had no suitable track.
fn place(
    timeline: &mut Timeline,
    clip: &Clipboard,
    at: f64,
    base_track: usize,
) -> (Vec<(TrackId, TrackEvent)>, usize) {
    let mut placed = Vec::new();
    let mut skipped = 0;
    // Every pasted copy forms its own groups, never joining the events it was copied from
    let mut groups: BTreeMap<GroupId, GroupId> = BTreeMap::new();
    for item in &clip.items {
        let target = timeline
            .tracks
            .get(base_track + item.track_offset)
            .filter(|t| t.kind == item.kind)
            .map(|t| t.id);
        match target {
            Some(track_id) => {
                let mut event = item.event.clone();
                event.id = timeline.alloc_id();
                if let Some(old) = event.group {
                    event.group = Some(*groups.entry(old).or_insert_with(|| timeline.alloc_id()));
                }
                event.start = (at + item.event.start).max(0.0);
                placed.push((track_id, event));
            }
            None => skipped += 1,
        }
    }
    (placed, skipped)
}

/// Pastes the clipboard with its first track on `base_track` (an index into `timeline.tracks`).
/// Items whose track is missing or of the other kind (video events only go on video tracks) are
/// skipped; the second value of the result is how many were skipped.
pub fn paste(
    timeline: &mut Timeline,
    clip: &Clipboard,
    at: f64,
    base_track: usize,
    mode: PasteMode,
) -> Option<(Outcome, usize)> {
    paste_copies(timeline, clip, &[at], base_track, mode)
}

/// Pastes `count` copies starting at `at`, spaced as requested, as one undo step.
pub fn paste_repeat(
    timeline: &mut Timeline,
    clip: &Clipboard,
    at: f64,
    base_track: usize,
    count: usize,
    spacing: RepeatSpacing,
) -> Option<(Outcome, usize)> {
    let step = match spacing {
        RepeatSpacing::EndToEnd => clip.length,
        RepeatSpacing::Every(seconds) => seconds,
    };
    if count == 0 || step <= TIME_EPSILON {
        return None;
    }
    let starts: Vec<f64> = (0..count).map(|k| at + step * k as f64).collect();
    paste_copies(timeline, clip, &starts, base_track, PasteMode::Overlay)
}

fn paste_copies(
    timeline: &mut Timeline,
    clip: &Clipboard,
    starts: &[f64],
    base_track: usize,
    mode: PasteMode,
) -> Option<(Outcome, usize)> {
    let mut placed = Vec::new();
    let mut skipped = 0;
    for &at in starts {
        let (mut p, s) = place(timeline, clip, at, base_track);
        placed.append(&mut p);
        skipped += s;
    }
    if placed.is_empty() {
        return None;
    }
    let mut draft = Draft::new(timeline);
    if mode == PasteMode::Insert {
        // Only meaningful for one paste position
        let at = starts[0];
        let mut tracks: Vec<TrackId> = placed.iter().map(|(t, _)| *t).collect();
        tracks.sort_unstable();
        tracks.dedup();
        let mut alloc = || timeline.alloc_id();
        for track_id in tracks {
            if let Some(list) = draft.list(track_id) {
                split_list(list, at, &mut alloc, |_| true);
                for ev in list.iter_mut().filter(|e| e.start >= at - TIME_EPSILON) {
                    ev.start += clip.length;
                }
            }
        }
    }
    let touched: Vec<EventRef> = placed.iter().map(|(t, e)| (*t, e.id)).collect();
    for (track_id, event) in placed {
        if let Some(list) = draft.list(track_id) {
            list.push(event);
        }
    }
    outcome(draft.finish(), touched).map(|o| (o, skipped))
}

// ------------------------------------------------------------------------------------------
// Trim and split
// ------------------------------------------------------------------------------------------

/// Keeps only the part of each target inside `range`; targets entirely outside it are removed.
/// Events that are not targets are not touched.
pub fn trim_to_range(
    timeline: &Timeline,
    targets: &[EventRef],
    range: TimeRange,
) -> Option<Outcome> {
    if range.is_empty() {
        return None;
    }
    let mut draft = Draft::new(timeline);
    for &(track_id, event_id) in targets {
        if let Some(list) = draft.list(track_id) {
            if let Some(i) = list.iter().position(|e| e.id == event_id) {
                match clip_to(&list[i], range) {
                    Some(part) => list[i] = part,
                    None => {
                        list.remove(i);
                    }
                }
            }
        }
    }
    outcome(draft.finish(), targets.to_vec())
}

/// Trims the start of each target that contains `time` to `time` (Trim Start).
pub fn trim_start_to(timeline: &Timeline, targets: &[EventRef], time: f64) -> Option<Outcome> {
    trim_edge(timeline, targets, time, true)
}

/// Trims the end of each target that contains `time` to `time` (Trim End).
pub fn trim_end_to(timeline: &Timeline, targets: &[EventRef], time: f64) -> Option<Outcome> {
    trim_edge(timeline, targets, time, false)
}

fn trim_edge(timeline: &Timeline, targets: &[EventRef], time: f64, start: bool) -> Option<Outcome> {
    let mut draft = Draft::new(timeline);
    let mut touched = Vec::new();
    for &(track_id, event_id) in targets {
        let Some(list) = draft.list(track_id) else {
            continue;
        };
        let Some(i) = list.iter().position(|e| e.id == event_id) else {
            continue;
        };
        let ev = &list[i];
        if time <= ev.start + TIME_EPSILON || time >= ev.end() - TIME_EPSILON {
            continue;
        }
        let range = if start {
            TimeRange::new(time, ev.end())
        } else {
            TimeRange::new(ev.start, time)
        };
        if let Some(part) = clip_to(ev, range) {
            list[i] = part;
            touched.push((track_id, event_id));
        }
    }
    outcome(draft.finish(), touched)
}

/// Splits every event inside `scope` that strictly contains `time`.
pub fn split_at(timeline: &mut Timeline, time: f64, scope: Scope) -> Option<Outcome> {
    split_at_times(timeline, &[time], scope)
}

/// Splits at both edges of a time selection.
pub fn split_range(timeline: &mut Timeline, range: TimeRange, scope: Scope) -> Option<Outcome> {
    split_at_times(timeline, &[range.start, range.end], scope)
}

fn split_at_times(timeline: &mut Timeline, times: &[f64], scope: Scope) -> Option<Outcome> {
    let mut draft = Draft::new(timeline);
    let track_ids: Vec<TrackId> = timeline.tracks.iter().map(|t| t.id).collect();
    let mut alloc = || timeline.alloc_id();
    let mut touched = Vec::new();
    // (group, split time) -> group of the right-hand halves, so a split cuts a group in two
    let mut halves: Vec<(GroupId, f64, GroupId)> = Vec::new();
    for track_id in track_ids {
        let Some(list) = draft.list(track_id) else {
            continue;
        };
        for &at in times {
            let created = split_list(list, at, &mut alloc, |e| scope.includes(track_id, e.id));
            for id in &created {
                if let Some(e) = list.iter_mut().find(|e| e.id == *id) {
                    if let Some(old) = e.group {
                        let known = halves
                            .iter()
                            .find(|(g, t, _)| *g == old && (*t - at).abs() <= TIME_EPSILON);
                        let new = match known {
                            Some((_, _, new)) => *new,
                            None => {
                                let new = alloc();
                                halves.push((old, at, new));
                                new
                            }
                        };
                        e.group = Some(new);
                    }
                }
            }
            touched.extend(created.into_iter().map(|id| (track_id, id)));
        }
    }
    outcome(draft.finish(), touched)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::selection::tests::sample;

    fn r(tl: &Timeline, track: usize, event: usize) -> EventRef {
        (tl.tracks[track].id, tl.tracks[track].events[event].id)
    }

    fn spans(tl: &Timeline, track: usize) -> Vec<(f64, f64)> {
        tl.tracks[track]
            .events
            .iter()
            .map(|e| (e.start, e.end()))
            .collect()
    }

    fn run(tl: &mut Timeline, history: &mut History, outcome: Option<Outcome>) -> Vec<EventRef> {
        let outcome = outcome.expect("operation should change the timeline");
        assert!(history.execute(tl, outcome.command));
        outcome.touched
    }

    #[test]
    fn delete_removes_and_undo_restores() {
        let mut tl = sample();
        let before = tl.clone();
        let mut history = History::new();
        let out = delete_events(&tl, &[r(&tl, 0, 0), r(&tl, 1, 2)]);
        run(&mut tl, &mut history, out);
        assert_eq!(spans(&tl, 0), vec![(3.0, 5.0), (6.0, 8.0)]);
        assert_eq!(spans(&tl, 1), vec![(0.0, 2.0), (3.0, 5.0)]);
        history.undo(&mut tl);
        assert_eq!(tl, before);
        history.redo(&mut tl);
        assert_eq!(spans(&tl, 0).len(), 2);
        assert!(delete_events(&tl, &[]).is_none());
    }

    #[test]
    fn range_removal_splits_at_the_edges_and_keeps_the_source_aligned() {
        let mut tl = sample();
        let mut history = History::new();
        // Events 3-5 and 6-8 on track 0; remove 4-7
        let first = [tl.tracks[0].id];
        let out = remove_range(
            &mut tl,
            TimeRange::new(4.0, 7.0),
            Scope::Tracks(&first),
            false,
        );
        run(&mut tl, &mut history, out);
        assert_eq!(
            spans(&tl, 0),
            vec![(0.0, 2.0), (3.0, 4.0), (7.0, 8.0)],
            "the 3-5 event keeps its head, the 6-8 event keeps its tail"
        );
        // The surviving tail still plays source time 1.0 at timeline 7.0
        assert_eq!(tl.tracks[0].events[2].trim_in, 1.0);
        assert_eq!(spans(&tl, 1).len(), 3, "other tracks untouched");
        history.undo(&mut tl);
        assert_eq!(spans(&tl, 0).len(), 3);
    }

    #[test]
    fn ripple_removal_closes_the_gap() {
        let mut tl = sample();
        let mut history = History::new();
        let out = remove_range(&mut tl, TimeRange::new(2.0, 3.0), Scope::AllTracks, true);
        run(&mut tl, &mut history, out);
        assert_eq!(spans(&tl, 0), vec![(0.0, 2.0), (2.0, 4.0), (5.0, 7.0)]);
        assert_eq!(spans(&tl, 1), vec![(0.0, 2.0), (2.0, 4.0), (5.0, 7.0)]);
    }

    #[test]
    fn explicit_event_scope_never_ripples_and_spares_other_events() {
        let mut tl = sample();
        let mut history = History::new();
        let keep = r(&tl, 1, 1);
        let target = [r(&tl, 0, 1)];
        let out = remove_range(
            &mut tl,
            TimeRange::new(3.5, 4.5),
            Scope::Events(&target),
            true,
        );
        run(&mut tl, &mut history, out);
        assert_eq!(
            spans(&tl, 0),
            vec![(0.0, 2.0), (3.0, 3.5), (4.5, 5.0), (6.0, 8.0)]
        );
        assert!(tl.tracks[1]
            .events
            .iter()
            .any(|e| e.id == keep.1 && e.start == 3.0));
    }

    #[test]
    fn copy_range_cuts_edges_and_paste_places_at_the_cursor() {
        let mut tl = sample();
        let mut history = History::new();
        let clip = copy_range(&tl, TimeRange::new(1.0, 4.0), Scope::AllTracks).unwrap();
        assert_eq!(clip.length, 3.0);
        assert_eq!(clip.items.len(), 4, "1-2 and 3-4 on both tracks");
        let (out, skipped) = paste(&mut tl, &clip, 10.0, 0, PasteMode::Overlay).unwrap();
        assert_eq!(skipped, 0);
        let touched = out.touched.clone();
        assert!(history.execute(&mut tl, out.command));
        assert_eq!(touched.len(), 4);
        assert_eq!(spans(&tl, 0).last(), Some(&(12.0, 13.0)));
        assert_eq!(spans(&tl, 0)[3], (10.0, 11.0));
        // The pasted 1-2 piece plays source 1.0..2.0
        assert_eq!(tl.tracks[0].events[3].trim_in, 1.0);
        history.undo(&mut tl);
        assert_eq!(spans(&tl, 0).len(), 3);
    }

    #[test]
    fn copy_events_keeps_relative_positions_and_tracks() {
        let tl = sample();
        let clip = copy_events(&tl, &[r(&tl, 0, 1), r(&tl, 1, 2)]).unwrap();
        assert_eq!(clip.length, 5.0, "3 s to 8 s");
        let mut offsets: Vec<_> = clip
            .items
            .iter()
            .map(|i| (i.track_offset, i.event.start))
            .collect();
        offsets.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(offsets, vec![(0, 0.0), (1, 3.0)]);
    }

    #[test]
    fn paste_skips_items_without_a_track_of_the_right_kind() {
        let mut tl = sample();
        let video = tl.new_track(TrackKind::Video);
        tl.tracks.push(video);
        let clip = copy_events(&tl, &[r(&tl, 0, 0), r(&tl, 1, 0)]).unwrap();
        // Base 1: the second item would land on the video track (wrong kind)
        let (out, skipped) = paste(&mut tl, &clip, 20.0, 1, PasteMode::Overlay).unwrap();
        assert_eq!(skipped, 1);
        assert_eq!(out.touched.len(), 1);
        // Base past the end: nothing to place
        assert!(paste(&mut tl, &clip, 20.0, 9, PasteMode::Overlay).is_none());
    }

    #[test]
    fn paste_insert_splits_and_pushes_later_events() {
        let mut tl = sample();
        let mut history = History::new();
        let clip = copy_range(
            &tl,
            TimeRange::new(0.0, 1.0),
            Scope::Tracks(&[tl.tracks[0].id]),
        )
        .unwrap();
        // Insert at 3.5 (inside the 3-5 event)
        let (out, _) = paste(&mut tl, &clip, 3.5, 0, PasteMode::Insert).unwrap();
        assert!(history.execute(&mut tl, out.command));
        assert_eq!(
            spans(&tl, 0),
            vec![(0.0, 2.0), (3.0, 3.5), (3.5, 4.5), (4.5, 6.0), (7.0, 9.0)],
            "head stays, pasted piece, tail and later events move by 1 s"
        );
        assert_eq!(spans(&tl, 1).len(), 3, "other tracks are not pushed");
        history.undo(&mut tl);
        assert_eq!(spans(&tl, 0), vec![(0.0, 2.0), (3.0, 5.0), (6.0, 8.0)]);
    }

    #[test]
    fn paste_repeat_end_to_end_and_every() {
        let mut tl = sample();
        let mut history = History::new();
        let clip = copy_events(&tl, &[r(&tl, 0, 0)]).unwrap();
        let (out, _) = paste_repeat(&mut tl, &clip, 20.0, 0, 3, RepeatSpacing::EndToEnd).unwrap();
        assert_eq!(out.touched.len(), 3);
        assert!(history.execute(&mut tl, out.command));
        let tail: Vec<_> = spans(&tl, 0).into_iter().skip(3).collect();
        assert_eq!(tail, vec![(20.0, 22.0), (22.0, 24.0), (24.0, 26.0)]);
        let (out, _) = paste_repeat(&mut tl, &clip, 40.0, 0, 2, RepeatSpacing::Every(5.0)).unwrap();
        assert!(history.execute(&mut tl, out.command));
        assert_eq!(spans(&tl, 0).last(), Some(&(45.0, 47.0)));
        assert!(paste_repeat(&mut tl, &clip, 0.0, 0, 0, RepeatSpacing::EndToEnd).is_none());
        assert!(paste_repeat(&mut tl, &clip, 0.0, 0, 2, RepeatSpacing::Every(0.0)).is_none());
    }

    #[test]
    fn trim_to_range_cuts_targets_and_removes_those_outside() {
        let mut tl = sample();
        let mut history = History::new();
        let targets = [r(&tl, 0, 0), r(&tl, 0, 1), r(&tl, 0, 2)];
        let out = trim_to_range(&tl, &targets, TimeRange::new(1.0, 4.0));
        run(&mut tl, &mut history, out);
        assert_eq!(
            spans(&tl, 0),
            vec![(1.0, 2.0), (3.0, 4.0)],
            "the 6-8 event is removed"
        );
        assert_eq!(tl.tracks[0].events[0].trim_in, 1.0);
        assert_eq!(spans(&tl, 1).len(), 3, "events that were not targets stay");
    }

    #[test]
    fn trim_start_and_end_to_the_cursor() {
        let mut tl = sample();
        let mut history = History::new();
        let target = [r(&tl, 0, 1)];
        let out = trim_start_to(&tl, &target, 4.0);
        run(&mut tl, &mut history, out);
        assert_eq!(spans(&tl, 0)[1], (4.0, 5.0));
        assert_eq!(tl.tracks[0].events[1].trim_in, 1.0);
        let out = trim_end_to(&tl, &target, 4.5);
        run(&mut tl, &mut history, out);
        assert_eq!(spans(&tl, 0)[1], (4.0, 4.5));
        // A cursor outside the event changes nothing
        assert!(trim_start_to(&tl, &target, 9.0).is_none());
        assert!(trim_end_to(&tl, &target, 0.0).is_none());
    }

    #[test]
    fn fades_survive_only_on_uncut_edges() {
        let mut tl = sample();
        tl.tracks[0].events[1].fade_in = 0.5;
        tl.tracks[0].events[1].fade_out = 0.5;
        let mut history = History::new();
        let target = [r(&tl, 0, 1)];
        let out = trim_start_to(&tl, &target, 4.0);
        run(&mut tl, &mut history, out);
        let e = &tl.tracks[0].events[1];
        assert_eq!((e.fade_in, e.fade_out), (0.0, 0.5));
    }

    #[test]
    fn split_scope_all_tracks_selected_events_and_range() {
        let mut tl = sample();
        let mut history = History::new();
        // All tracks at 3.5: both tracks split
        let out = split_at(&mut tl, 3.5, Scope::AllTracks);
        let touched = run(&mut tl, &mut history, out);
        assert_eq!(touched.len(), 2);
        assert_eq!(spans(&tl, 0).len(), 4);
        assert_eq!(spans(&tl, 1).len(), 4);
        history.undo(&mut tl);
        assert_eq!(spans(&tl, 1).len(), 3);
        // Only the selected event
        let only = [r(&tl, 1, 1)];
        let out = split_at(&mut tl, 3.5, Scope::Events(&only));
        run(&mut tl, &mut history, out);
        assert_eq!(spans(&tl, 0).len(), 3);
        assert_eq!(spans(&tl, 1).len(), 4);
        // Both edges of a range
        let first = [tl.tracks[0].id];
        let out = split_range(&mut tl, TimeRange::new(6.5, 7.5), Scope::Tracks(&first));
        run(&mut tl, &mut history, out);
        assert_eq!(
            spans(&tl, 0),
            vec![(0.0, 2.0), (3.0, 5.0), (6.0, 6.5), (6.5, 7.5), (7.5, 8.0)]
        );
        // A time with no event under it splits nothing
        assert!(split_at(&mut tl, 2.5, Scope::AllTracks).is_none());
    }

    #[test]
    fn new_ids_are_unique_across_a_complex_edit() {
        let mut tl = sample();
        let mut history = History::new();
        let out = split_at(&mut tl, 1.0, Scope::AllTracks);
        run(&mut tl, &mut history, out);
        let out = remove_range(&mut tl, TimeRange::new(0.5, 3.5), Scope::AllTracks, true);
        run(&mut tl, &mut history, out);
        let mut ids: Vec<u64> = tl
            .tracks
            .iter()
            .flat_map(|t| t.events.iter().map(|e| e.id))
            .collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total, "no duplicated event id");
    }
}
