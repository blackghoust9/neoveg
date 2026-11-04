//! Cursor navigation: project end and the edit points (event edges and fade edges) the cursor jumps
//! between. Pure functions over the timeline, so the jumps are testable without a window.

use crate::selection::{EventRef, TimeRange};
use crate::timeline::{Timeline, TIME_EPSILON};

/// End of the last event, seconds (`0` for an empty timeline).
pub fn project_end(timeline: &Timeline) -> f64 {
    timeline
        .tracks
        .iter()
        .flat_map(|t| t.events.iter().map(|e| e.end()))
        .fold(0.0, f64::max)
}

/// Every edit point of the timeline, sorted and without duplicates: the edges of the events and the
/// edges of their fades.
pub fn edit_points(timeline: &Timeline) -> Vec<f64> {
    let mut points: Vec<f64> = Vec::new();
    for event in timeline.tracks.iter().flat_map(|t| &t.events) {
        points.push(event.start);
        points.push(event.end());
        if event.fade_in > 0.0 {
            points.push(event.start + event.fade_in);
        }
        if event.fade_out > 0.0 {
            points.push(event.end() - event.fade_out);
        }
    }
    points.retain(|p| p.is_finite());
    points.sort_by(f64::total_cmp);
    points.dedup_by(|a, b| (*a - *b).abs() < TIME_EPSILON);
    points
}

/// The closest edit point after `from`.
pub fn next_edit_point(timeline: &Timeline, from: f64) -> Option<f64> {
    edit_points(timeline)
        .into_iter()
        .find(|p| *p > from + TIME_EPSILON)
}

/// The closest edit point before `from`.
pub fn previous_edit_point(timeline: &Timeline, from: f64) -> Option<f64> {
    edit_points(timeline)
        .into_iter()
        .rev()
        .find(|p| *p < from - TIME_EPSILON)
}

/// Reads a time typed in the "go to" box: a timecode (`00:01:02:10`, `;` for drop-frame), minutes
/// and seconds (`1:02.5`), seconds (`62,5` or `62.5s`) or frames (`1500f`). `None` when it is
/// none of these or is negative.
pub fn parse_time(text: &str, fps: f64) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() || fps <= 0.0 {
        return None;
    }
    if let Some((frame, _)) = neoveg_common::timecode::parse_timecode(text, fps) {
        return (frame >= 0).then(|| frame as f64 / fps);
    }
    if let Some(frames) = text.strip_suffix(['f', 'F']) {
        let n: f64 = frames.trim().parse().ok()?;
        return (n >= 0.0 && n.is_finite()).then(|| n / fps);
    }
    let plain = text
        .strip_suffix(['s', 'S'])
        .unwrap_or(text)
        .replace(',', ".");
    let value = match plain.split(':').collect::<Vec<_>>().as_slice() {
        [secs] => secs.trim().parse::<f64>().ok()?,
        [mins, secs] => {
            mins.trim().parse::<f64>().ok()? * 60.0 + secs.trim().parse::<f64>().ok()?
        }
        [hours, mins, secs] => {
            hours.trim().parse::<f64>().ok()? * 3600.0
                + mins.trim().parse::<f64>().ok()? * 60.0
                + secs.trim().parse::<f64>().ok()?
        }
        _ => return None,
    };
    (value >= 0.0 && value.is_finite()).then_some(value)
}

/// The closest region edge (start or end) strictly before `from`: `,` in the reference map.
pub fn previous_region_edge(timeline: &Timeline, from: f64) -> Option<f64> {
    region_edges(timeline)
        .into_iter()
        .rev()
        .find(|t| *t < from - TIME_EPSILON)
}

/// The closest region edge (start or end) strictly after `from`: `.` in the reference map.
pub fn next_region_edge(timeline: &Timeline, from: f64) -> Option<f64> {
    region_edges(timeline)
        .into_iter()
        .find(|t| *t > from + TIME_EPSILON)
}

fn region_edges(timeline: &Timeline) -> Vec<f64> {
    let mut edges: Vec<f64> = timeline
        .regions
        .iter()
        .flat_map(|r| [r.start, r.end])
        .filter(|t| t.is_finite())
        .collect();
    edges.sort_by(f64::total_cmp);
    edges.dedup_by(|a, b| (*a - *b).abs() < TIME_EPSILON);
    edges
}

/// The span of every selected event that still exists, ordered by start.
fn selected_spans(timeline: &Timeline, selected: &[EventRef]) -> Vec<TimeRange> {
    let mut spans: Vec<TimeRange> = selected
        .iter()
        .filter_map(|(track, event)| {
            let event = timeline
                .track(*track)?
                .events
                .iter()
                .find(|e| e.id == *event)?;
            Some(TimeRange::new(event.start, event.end()))
        })
        .collect();
    spans.sort_by(|a, b| a.start.total_cmp(&b.start));
    spans
}

/// The selected event the cursor is at or after: the last one that starts before `from`
/// (`Ctrl+[` goes to its start, `Ctrl+Shift+[` selects its time).
pub fn previous_selected_event(
    timeline: &Timeline,
    selected: &[EventRef],
    from: f64,
) -> Option<TimeRange> {
    selected_spans(timeline, selected)
        .into_iter()
        .rev()
        .find(|s| s.start < from - TIME_EPSILON)
}

/// The selected event the cursor is at or before: the first one that ends after `from`
/// (`Ctrl+]` goes to its end, `Ctrl+Shift+]` selects its time).
pub fn next_selected_event(
    timeline: &Timeline,
    selected: &[EventRef],
    from: f64,
) -> Option<TimeRange> {
    selected_spans(timeline, selected)
        .into_iter()
        .find(|s| s.end > from + TIME_EPSILON)
}

/// How many earlier time selections `Backspace` can bring back.
pub const SELECTION_HISTORY_LEN: usize = 5;

/// The last time selections, so `Backspace` can restore them one after the other.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TimeSelectionHistory {
    last: Option<TimeRange>,
    past: Vec<TimeRange>,
}

impl TimeSelectionHistory {
    /// Reports the current time selection. When it differs from the one seen before, that one goes
    /// into the history (only the last five are kept; empty ones are never kept).
    pub fn observe(&mut self, current: Option<TimeRange>) {
        if current == self.last {
            return;
        }
        if let Some(previous) = self.last.filter(|r| !r.is_empty()) {
            self.past.push(previous);
            if self.past.len() > SELECTION_HISTORY_LEN {
                self.past.remove(0);
            }
        }
        self.last = current;
    }

    /// Takes the most recent earlier selection out of the history; the caller makes it the
    /// current one. `None` when there is nothing to restore.
    pub fn restore(&mut self) -> Option<TimeRange> {
        let range = self.past.pop()?;
        self.last = Some(range);
        Some(range)
    }

    pub fn len(&self) -> usize {
        self.past.len()
    }

    pub fn is_empty(&self) -> bool {
        self.past.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::{TrackEvent, TrackKind};

    fn timeline() -> Timeline {
        let mut tl = Timeline::new();
        let mut a = tl.new_track(TrackKind::Audio);
        let mut b = tl.new_track(TrackKind::Audio);
        // Built from JSON so the test does not depend on every field the event has grown
        let add = |tl: &mut Timeline,
                   track: &mut crate::timeline::Track,
                   start: f64,
                   duration: f64,
                   fade_in: f64| {
            let event: TrackEvent = serde_json::from_value(serde_json::json!({
                "id": tl.alloc_id(),
                "name": "",
                "media_id": null,
                "start": start,
                "duration": duration,
                "trim_in": 0.0,
                "fade_in": fade_in,
                "fade_out": 0.0,
            }))
            .unwrap();
            track.events.push(event);
        };
        add(&mut tl, &mut a, 1.0, 2.0, 0.5);
        add(&mut tl, &mut b, 2.0, 4.0, 0.0);
        tl.tracks.push(a);
        tl.tracks.push(b);
        tl
    }

    #[test]
    fn edit_points_are_sorted_and_include_fade_edges() {
        assert_eq!(edit_points(&timeline()), vec![1.0, 1.5, 2.0, 3.0, 6.0]);
        assert_eq!(project_end(&timeline()), 6.0);
        assert_eq!(project_end(&Timeline::new()), 0.0);
        assert!(edit_points(&Timeline::new()).is_empty());
    }

    #[test]
    fn typed_times_are_read_in_every_form() {
        let close = |a: Option<f64>, b: f64| a.is_some_and(|v| (v - b).abs() < 1e-6);
        assert!(close(parse_time("00:00:02:12", 24.0), 2.5));
        assert!(close(parse_time("1:02.5", 25.0), 62.5));
        assert!(close(parse_time("62,5", 25.0), 62.5));
        assert!(close(parse_time("62.5s", 25.0), 62.5));
        assert!(close(parse_time("1500f", 25.0), 60.0));
        assert!(close(parse_time("0:1:2", 25.0), 62.0));
        assert!(parse_time("", 25.0).is_none());
        assert!(parse_time("abc", 25.0).is_none());
        assert!(parse_time("-3", 25.0).is_none());
        assert!(parse_time("1:2:3:4:5", 25.0).is_none());
        assert!(parse_time("10", 0.0).is_none());
    }

    #[test]
    fn the_cursor_jumps_to_the_next_and_previous_point() {
        let tl = timeline();
        assert_eq!(next_edit_point(&tl, 0.0), Some(1.0));
        assert_eq!(next_edit_point(&tl, 1.0), Some(1.5));
        assert_eq!(next_edit_point(&tl, 3.0), Some(6.0));
        assert_eq!(next_edit_point(&tl, 6.0), None);
        assert_eq!(previous_edit_point(&tl, 6.0), Some(3.0));
        assert_eq!(previous_edit_point(&tl, 1.2), Some(1.0));
        assert_eq!(previous_edit_point(&tl, 1.0), None);
    }

    #[test]
    fn the_cursor_jumps_between_region_edges() {
        let mut tl = timeline();
        assert_eq!(next_region_edge(&tl, 0.0), None);
        for (id, start, end) in [(901, 2.0, 4.0), (902, 4.0, 5.0)] {
            tl.regions.push(
                serde_json::from_value(
                    serde_json::json!({"id": id, "start": start, "end": end, "name": ""}),
                )
                .unwrap(),
            );
        }
        assert_eq!(next_region_edge(&tl, 0.0), Some(2.0));
        assert_eq!(next_region_edge(&tl, 2.0), Some(4.0));
        assert_eq!(next_region_edge(&tl, 4.0), Some(5.0));
        assert_eq!(next_region_edge(&tl, 5.0), None);
        assert_eq!(previous_region_edge(&tl, 5.0), Some(4.0));
        assert_eq!(previous_region_edge(&tl, 3.0), Some(2.0));
        assert_eq!(previous_region_edge(&tl, 2.0), None);
    }

    #[test]
    fn selected_events_are_found_before_and_after_the_cursor() {
        let tl = timeline();
        let a = (tl.tracks[0].id, tl.tracks[0].events[0].id); // 1 s to 3 s
        let b = (tl.tracks[1].id, tl.tracks[1].events[0].id); // 2 s to 6 s
        let both = [b, a];
        assert_eq!(
            previous_selected_event(&tl, &both, 2.5),
            Some(TimeRange::new(2.0, 6.0))
        );
        assert_eq!(
            previous_selected_event(&tl, &both, 2.0),
            Some(TimeRange::new(1.0, 3.0))
        );
        assert_eq!(previous_selected_event(&tl, &both, 1.0), None);
        assert_eq!(
            next_selected_event(&tl, &both, 0.0),
            Some(TimeRange::new(1.0, 3.0))
        );
        assert_eq!(
            next_selected_event(&tl, &both, 3.0),
            Some(TimeRange::new(2.0, 6.0))
        );
        assert_eq!(next_selected_event(&tl, &both, 6.0), None);
        // Nothing selected, nothing to find
        assert_eq!(next_selected_event(&tl, &[], 0.0), None);
        // An event that is not selected is ignored
        assert_eq!(next_selected_event(&tl, &[a], 3.0), None);
    }

    #[test]
    fn the_history_keeps_the_last_five_time_selections() {
        let range = |n: f64| Some(TimeRange::new(n, n + 1.0));
        let mut history = TimeSelectionHistory::default();
        history.observe(None);
        history.observe(range(1.0));
        assert!(history.is_empty());
        for n in 2..=8 {
            history.observe(range(f64::from(n)));
        }
        // Seven selections were replaced; only the last five were kept
        assert_eq!(history.len(), SELECTION_HISTORY_LEN);
        // Observing the same value again changes nothing
        history.observe(range(8.0));
        assert_eq!(history.len(), SELECTION_HISTORY_LEN);
        assert_eq!(history.restore(), range(7.0));
        assert_eq!(history.restore(), range(6.0));
        // The restored selection is the current one now and is not pushed back
        history.observe(range(6.0));
        assert_eq!(history.len(), 3);
        assert_eq!(history.restore(), range(5.0));
        assert_eq!(history.restore(), range(4.0));
        assert_eq!(history.restore(), range(3.0));
        assert_eq!(history.restore(), None);
    }

    #[test]
    fn clearing_the_selection_keeps_it_in_the_history() {
        let mut history = TimeSelectionHistory::default();
        history.observe(Some(TimeRange::new(1.0, 2.0)));
        history.observe(None);
        assert_eq!(history.restore(), Some(TimeRange::new(1.0, 2.0)));
    }
}
