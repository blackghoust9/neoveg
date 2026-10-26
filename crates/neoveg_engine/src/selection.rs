//! Event selection and time ranges. Pure data and rules, no UI types.

use std::collections::BTreeSet;

use crate::timeline::{EventId, Timeline, TrackId, TIME_EPSILON};

/// An event addressed by its track and id.
pub type EventRef = (TrackId, EventId);

/// A span of time in seconds with `start <= end`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeRange {
    pub start: f64,
    pub end: f64,
}

impl TimeRange {
    /// Builds a range from two times in any order (negative times are clamped to zero).
    pub fn new(a: f64, b: f64) -> Self {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        Self {
            start: lo.max(0.0),
            end: hi.max(0.0),
        }
    }

    pub fn len(&self) -> f64 {
        self.end - self.start
    }

    /// `true` when the range is shorter than the time tolerance.
    pub fn is_empty(&self) -> bool {
        self.len() <= TIME_EPSILON
    }

    pub fn contains(&self, t: f64) -> bool {
        t >= self.start - TIME_EPSILON && t <= self.end + TIME_EPSILON
    }

    /// `true` when the open interval `(start, end)` overlaps this range by more than the tolerance.
    pub fn overlaps(&self, start: f64, end: f64) -> bool {
        start < self.end - TIME_EPSILON && end > self.start + TIME_EPSILON
    }
}

/// The set of selected events. `primary` is the last one clicked (shown in the inspector) and
/// `anchor` is where a Shift-click range starts.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Selection {
    events: BTreeSet<EventRef>,
    primary: Option<EventRef>,
    anchor: Option<EventRef>,
}

impl Selection {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn contains(&self, event: EventRef) -> bool {
        self.events.contains(&event)
    }

    pub fn primary(&self) -> Option<EventRef> {
        self.primary
    }

    /// Selected events ordered by track id and event id.
    pub fn refs(&self) -> Vec<EventRef> {
        self.events.iter().copied().collect()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Replaces the selection with `events` (the last one becomes the primary event).
    pub fn set(&mut self, events: impl IntoIterator<Item = EventRef>) {
        self.events = events.into_iter().collect();
        self.primary = self.events.iter().next_back().copied();
        self.anchor = self.primary;
    }

    /// Plain click on a grouped event: all of `events` are selected and `primary` is the one
    /// that was clicked.
    pub fn select_many(&mut self, events: impl IntoIterator<Item = EventRef>, primary: EventRef) {
        self.events = events.into_iter().collect();
        self.events.insert(primary);
        self.primary = Some(primary);
        self.anchor = Some(primary);
    }

    /// Ctrl-click on a grouped event: all of `events` leave the selection when `primary` was
    /// selected, otherwise they all join it.
    pub fn toggle_many(&mut self, events: &[EventRef], primary: EventRef) {
        if self.events.contains(&primary) {
            for e in events {
                self.events.remove(e);
            }
            self.events.remove(&primary);
            if self.primary == Some(primary) {
                self.primary = self.events.iter().next_back().copied();
            }
        } else {
            self.events.extend(events.iter().copied());
            self.events.insert(primary);
            self.primary = Some(primary);
        }
        self.anchor = Some(primary);
    }

    /// Adds events without changing the primary event or the Shift-click anchor.
    pub fn add_all(&mut self, events: impl IntoIterator<Item = EventRef>) {
        self.events.extend(events);
        if self.primary.is_none() {
            self.primary = self.events.iter().next_back().copied();
            self.anchor = self.primary;
        }
    }

    /// Plain click: only this event is selected.
    pub fn select_only(&mut self, event: EventRef) {
        self.events.clear();
        self.events.insert(event);
        self.primary = Some(event);
        self.anchor = Some(event);
    }

    /// Ctrl-click: adds the event, or removes it when it was already selected.
    pub fn toggle(&mut self, event: EventRef) {
        if self.events.remove(&event) {
            if self.primary == Some(event) {
                self.primary = self.events.iter().next_back().copied();
            }
        } else {
            self.events.insert(event);
            self.primary = Some(event);
        }
        self.anchor = Some(event);
    }

    /// Shift-click: selects every event between the anchor and `event`, that is, the events on the
    /// tracks from one to the other whose start lies between the two clicked events' starts.
    /// Without an anchor it behaves like a plain click.
    pub fn extend_to(&mut self, timeline: &Timeline, event: EventRef) {
        let Some(anchor) = self.anchor.filter(|a| locate(timeline, *a).is_some()) else {
            self.select_only(event);
            return;
        };
        let (Some((ta, ea)), Some((tb, eb))) = (locate(timeline, anchor), locate(timeline, event))
        else {
            self.select_only(event);
            return;
        };
        let (track_lo, track_hi) = (ta.min(tb), ta.max(tb));
        let (time_lo, time_hi) = (ea.min(eb), ea.max(eb));
        let mut picked = BTreeSet::new();
        for (index, track) in timeline.tracks.iter().enumerate() {
            if index < track_lo || index > track_hi {
                continue;
            }
            for e in &track.events {
                if e.start >= time_lo - TIME_EPSILON && e.start <= time_hi + TIME_EPSILON {
                    picked.insert((track.id, e.id));
                }
            }
        }
        picked.insert(event);
        self.events = picked;
        self.primary = Some(event);
        // The anchor stays so repeated Shift-clicks pivot around the same event
    }

    /// Ctrl+A: every event of every track.
    pub fn select_all(&mut self, timeline: &Timeline) {
        self.events = timeline
            .tracks
            .iter()
            .flat_map(|t| t.events.iter().map(move |e| (t.id, e.id)))
            .collect();
        self.primary = self.events.iter().next_back().copied();
        self.anchor = self.primary;
    }

    /// Selects `event` and every later event on the same track ("Select Events to End").
    pub fn select_to_end(&mut self, timeline: &Timeline, event: EventRef) {
        self.events.clear();
        if let Some(track) = timeline.track(event.0) {
            if let Some(from) = track.events.iter().find(|e| e.id == event.1) {
                let start = from.start;
                for e in track
                    .events
                    .iter()
                    .filter(|e| e.start >= start - TIME_EPSILON)
                {
                    self.events.insert((track.id, e.id));
                }
            }
        }
        self.primary = Some(event);
        self.anchor = Some(event);
    }

    /// Selects every event that overlaps `range` on the given tracks (`None` = all tracks); this is
    /// the rectangular selection of the Selection tool.
    pub fn select_in_rect(
        &mut self,
        timeline: &Timeline,
        tracks: Option<&[TrackId]>,
        range: TimeRange,
    ) {
        self.events.clear();
        for track in &timeline.tracks {
            if tracks.is_some_and(|ids| !ids.contains(&track.id)) {
                continue;
            }
            for e in &track.events {
                if range.overlaps(e.start, e.end()) {
                    self.events.insert((track.id, e.id));
                }
            }
        }
        self.primary = self.events.iter().next_back().copied();
        self.anchor = self.primary;
    }

    /// Drops references to events that no longer exist (after a delete, an undo or a file load).
    pub fn retain_existing(&mut self, timeline: &Timeline) {
        self.events.retain(|r| locate(timeline, *r).is_some());
        if self.primary.is_some_and(|p| !self.events.contains(&p)) {
            self.primary = self.events.iter().next_back().copied();
        }
        if self.anchor.is_some_and(|a| !self.events.contains(&a)) {
            self.anchor = self.primary;
        }
    }

    /// The smallest range covering every selected event.
    pub fn bounds(&self, timeline: &Timeline) -> Option<TimeRange> {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for &(track_id, event_id) in &self.events {
            let e = timeline
                .track(track_id)?
                .events
                .iter()
                .find(|e| e.id == event_id)?;
            lo = lo.min(e.start);
            hi = hi.max(e.end());
        }
        (lo.is_finite() && hi.is_finite()).then(|| TimeRange::new(lo, hi))
    }
}

/// Track index and start time of an event, if it exists.
fn locate(timeline: &Timeline, (track_id, event_id): EventRef) -> Option<(usize, f64)> {
    let index = timeline.tracks.iter().position(|t| t.id == track_id)?;
    let event = timeline.tracks[index]
        .events
        .iter()
        .find(|e| e.id == event_id)?;
    Some((index, event.start))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::timeline::{TrackEvent, TrackKind};

    /// Two audio tracks, three events each at 0-2, 3-5, 6-8 seconds.
    pub(crate) fn sample() -> Timeline {
        let mut tl = Timeline::new();
        for _ in 0..2 {
            let mut track = tl.new_track(TrackKind::Audio);
            for k in 0..3 {
                let id = tl.alloc_id();
                track.events.push(TrackEvent {
                    group: None,
                    id,
                    name: format!("e{k}"),
                    media_id: Some(1),
                    start: f64::from(k) * 3.0,
                    duration: 2.0,
                    trim_in: 0.0,
                    fade_in: 0.0,
                    fade_out: 0.0,
                    fade_in_curve: Default::default(),
                    fade_out_curve: Default::default(),
                });
            }
            tl.tracks.push(track);
        }
        tl
    }

    fn r(tl: &Timeline, track: usize, event: usize) -> EventRef {
        (tl.tracks[track].id, tl.tracks[track].events[event].id)
    }

    #[test]
    fn range_orders_and_clamps() {
        let range = TimeRange::new(5.0, -2.0);
        assert_eq!((range.start, range.end), (0.0, 5.0));
        assert!(TimeRange::new(1.0, 1.0).is_empty());
        assert!(range.overlaps(4.0, 9.0));
        assert!(!range.overlaps(5.0, 9.0), "touching is not overlapping");
    }

    #[test]
    fn group_clicks_select_and_toggle_every_member() {
        let tl = sample();
        let (a, b, c) = (r(&tl, 0, 0), r(&tl, 1, 0), r(&tl, 0, 1));
        let mut sel = Selection::new();
        sel.select_many([a, b], a);
        assert_eq!((sel.len(), sel.primary()), (2, Some(a)));
        sel.toggle_many(&[c], c);
        assert_eq!(sel.len(), 3);
        sel.toggle_many(&[a, b], a);
        assert_eq!((sel.refs(), sel.primary()), (vec![c], Some(c)));
        sel.add_all([a, b]);
        assert_eq!(
            (sel.len(), sel.primary()),
            (3, Some(c)),
            "primary is unchanged"
        );
    }

    #[test]
    fn click_toggle_and_primary() {
        let tl = sample();
        let mut sel = Selection::new();
        sel.select_only(r(&tl, 0, 0));
        sel.toggle(r(&tl, 0, 2));
        assert_eq!(sel.len(), 2);
        assert_eq!(sel.primary(), Some(r(&tl, 0, 2)));
        sel.toggle(r(&tl, 0, 2));
        assert_eq!(sel.len(), 1);
        assert_eq!(sel.primary(), Some(r(&tl, 0, 0)));
        sel.toggle(r(&tl, 0, 0));
        assert!(sel.is_empty());
        assert_eq!(sel.primary(), None);
    }

    #[test]
    fn shift_click_selects_a_rectangle_between_two_events() {
        let tl = sample();
        let mut sel = Selection::new();
        sel.select_only(r(&tl, 0, 0));
        sel.extend_to(&tl, r(&tl, 1, 1));
        // tracks 0..=1, starts between 0 and 3 s: events 0 and 1 of both tracks
        assert_eq!(sel.len(), 4);
        assert!(sel.contains(r(&tl, 0, 1)));
        assert!(!sel.contains(r(&tl, 0, 2)));
        // Pivots around the same anchor
        sel.extend_to(&tl, r(&tl, 0, 2));
        assert_eq!(sel.len(), 3);
    }

    #[test]
    fn shift_click_without_anchor_is_a_plain_click() {
        let tl = sample();
        let mut sel = Selection::new();
        sel.extend_to(&tl, r(&tl, 1, 2));
        assert_eq!(sel.refs(), vec![r(&tl, 1, 2)]);
    }

    #[test]
    fn select_all_to_end_and_rect() {
        let tl = sample();
        let mut sel = Selection::new();
        sel.select_all(&tl);
        assert_eq!(sel.len(), 6);
        sel.select_to_end(&tl, r(&tl, 0, 1));
        assert_eq!(sel.refs(), vec![r(&tl, 0, 1), r(&tl, 0, 2)]);
        sel.select_in_rect(&tl, Some(&[tl.tracks[1].id]), TimeRange::new(1.0, 3.5));
        assert_eq!(sel.refs(), vec![r(&tl, 1, 0), r(&tl, 1, 1)]);
        sel.select_in_rect(&tl, None, TimeRange::new(2.5, 2.9));
        assert!(sel.is_empty(), "a gap selects nothing");
    }

    #[test]
    fn retain_existing_drops_deleted_events_and_fixes_primary() {
        let mut tl = sample();
        let mut sel = Selection::new();
        sel.select_only(r(&tl, 0, 0));
        sel.toggle(r(&tl, 0, 1));
        let gone = r(&tl, 0, 1);
        tl.tracks[0].events.retain(|e| e.id != gone.1);
        sel.retain_existing(&tl);
        assert_eq!(sel.refs(), vec![r(&tl, 0, 0)]);
        assert_eq!(sel.primary(), Some(r(&tl, 0, 0)));
    }

    #[test]
    fn bounds_cover_all_selected_events() {
        let tl = sample();
        let mut sel = Selection::new();
        sel.select_only(r(&tl, 0, 0));
        sel.toggle(r(&tl, 1, 2));
        let b = sel.bounds(&tl).unwrap();
        assert_eq!((b.start, b.end), (0.0, 8.0));
        assert_eq!(Selection::new().bounds(&tl), None);
    }
}
