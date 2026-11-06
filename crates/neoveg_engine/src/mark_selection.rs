//! Several markers and regions at once: selecting them by time, moving them together, resizing the
//! regions among them and stretching the whole selection. Each call gives one [`EditCommand`], so a
//! change of many marks is one undo step.

use crate::command::EditCommand;
use crate::edit::quantize;
use crate::markers::{MarkId, Marks};
use crate::selection::TimeRange;
use crate::timeline::{Timeline, TIME_EPSILON};

/// The marks that sit inside `range` (a marker by its time, a region when it lies entirely inside
/// it, or only touches it when `touching` is set), regions after markers.
pub fn select_in_range(timeline: &Timeline, range: TimeRange, touching: bool) -> Vec<MarkId> {
    let mut ids: Vec<MarkId> = timeline
        .markers
        .iter()
        .filter(|m| m.time >= range.start - TIME_EPSILON && m.time <= range.end + TIME_EPSILON)
        .map(|m| m.id)
        .collect();
    ids.extend(timeline.regions.iter().filter_map(|r| {
        let inside = r.start >= range.start - TIME_EPSILON && r.end <= range.end + TIME_EPSILON;
        let overlaps = r.end > range.start && r.start < range.end;
        (inside || (touching && overlaps)).then_some(r.id)
    }));
    ids
}

/// Which end of the selected regions a resize moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionEdge {
    Start,
    End,
}

fn finish(timeline: &Timeline, mut after: Marks) -> Option<EditCommand> {
    after.markers.sort_by(|a, b| a.time.total_cmp(&b.time));
    after.regions.sort_by(|a, b| a.start.total_cmp(&b.start));
    let before = timeline.marks();
    (before != after).then_some(EditCommand::ReplaceMarks { before, after })
}

/// The span (earliest start, latest end) of the selected marks; `None` when none is selected.
pub fn selection_span(timeline: &Timeline, ids: &[MarkId]) -> Option<(f64, f64)> {
    let mut span: Option<(f64, f64)> = None;
    let mut add = |a: f64, b: f64| {
        span = Some(match span {
            Some((s, e)) => (s.min(a), e.max(b)),
            None => (a, b),
        });
    };
    for m in timeline.markers.iter().filter(|m| ids.contains(&m.id)) {
        add(m.time, m.time);
    }
    for r in timeline.regions.iter().filter(|r| ids.contains(&r.id)) {
        add(r.start, r.end);
    }
    span
}

/// Moves the selected marks together by `delta` seconds (frame-snapped when `fps` is positive). The
/// move stops where the earliest one reaches zero, so they keep their distances. `None` for no
/// selection or no change.
pub fn move_selected(
    timeline: &Timeline,
    ids: &[MarkId],
    delta: f64,
    fps: f64,
) -> Option<EditCommand> {
    let (first, _) = selection_span(timeline, ids)?;
    let target = if fps > 0.0 {
        quantize((first + delta).max(0.0), fps)
    } else {
        (first + delta).max(0.0)
    };
    let delta = target - first;
    let mut after = timeline.marks();
    for m in after.markers.iter_mut().filter(|m| ids.contains(&m.id)) {
        m.time += delta;
    }
    for r in after.regions.iter_mut().filter(|r| ids.contains(&r.id)) {
        r.start += delta;
        r.end += delta;
    }
    finish(timeline, after)
}

/// Moves one end of every selected region by `delta`; a region keeps at least one frame
/// (`min_len`) and its start does not go below zero. Markers in the selection stay. `None` when
/// nothing changes.
pub fn resize_selected(
    timeline: &Timeline,
    ids: &[MarkId],
    edge: RegionEdge,
    delta: f64,
    min_len: f64,
) -> Option<EditCommand> {
    let min_len = min_len.max(TIME_EPSILON);
    let mut after = timeline.marks();
    for r in after.regions.iter_mut().filter(|r| ids.contains(&r.id)) {
        match edge {
            RegionEdge::Start => {
                r.start = (r.start + delta).clamp(0.0, r.end - min_len);
            }
            RegionEdge::End => {
                r.end = (r.end + delta).max(r.start + min_len);
            }
        }
    }
    finish(timeline, after)
}

/// Stretches the selection about `anchor` by `factor` (`2.0` doubles every distance from it):
/// markers move, regions move and change length. Nothing goes below zero and a region keeps
/// `min_len`. `None` for no selection, a factor that is not positive, or no change.
pub fn stretch_selected(
    timeline: &Timeline,
    ids: &[MarkId],
    anchor: f64,
    factor: f64,
    min_len: f64,
) -> Option<EditCommand> {
    if !factor.is_finite() || factor <= 0.0 {
        return None;
    }
    selection_span(timeline, ids)?;
    let min_len = min_len.max(TIME_EPSILON);
    let map = |t: f64| (anchor + (t - anchor) * factor).max(0.0);
    let mut after = timeline.marks();
    for m in after.markers.iter_mut().filter(|m| ids.contains(&m.id)) {
        m.time = map(m.time);
    }
    for r in after.regions.iter_mut().filter(|r| ids.contains(&r.id)) {
        r.start = map(r.start);
        r.end = map(r.end).max(r.start + min_len);
    }
    finish(timeline, after)
}

/// Removes the selected marks.
pub fn remove_selected(timeline: &Timeline, ids: &[MarkId]) -> Option<EditCommand> {
    let mut after = timeline.marks();
    after.markers.retain(|m| !ids.contains(&m.id));
    after.regions.retain(|r| !ids.contains(&r.id));
    finish(timeline, after)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::markers::{add_marker, add_region};

    fn setup() -> (Timeline, History, Vec<MarkId>) {
        let mut tl = Timeline::new();
        let mut h = History::new();
        let cmd = add_marker(&mut tl, 1.0, Some("a")).unwrap();
        assert!(h.execute(&mut tl, cmd));
        let cmd = add_marker(&mut tl, 3.0, Some("b")).unwrap();
        assert!(h.execute(&mut tl, cmd));
        let cmd = add_region(&mut tl, TimeRange::new(4.0, 6.0), Some("r")).unwrap();
        assert!(h.execute(&mut tl, cmd));
        let cmd = add_marker(&mut tl, 9.0, Some("far")).unwrap();
        assert!(h.execute(&mut tl, cmd));
        let ids = vec![tl.markers[0].id, tl.markers[1].id, tl.regions[0].id];
        (tl, h, ids)
    }

    #[test]
    fn a_time_range_selects_the_marks_inside_it() {
        let (tl, _, ids) = setup();
        let inside = select_in_range(&tl, TimeRange::new(0.5, 7.0), false);
        assert_eq!(inside, ids);
        // A region only half inside is left out, unless touching counts
        let half = TimeRange::new(0.5, 5.0);
        assert_eq!(select_in_range(&tl, half, false).len(), 2);
        assert_eq!(select_in_range(&tl, half, true).len(), 3);
        assert_eq!(selection_span(&tl, &ids), Some((1.0, 6.0)));
        assert_eq!(selection_span(&tl, &[]), None);
    }

    #[test]
    fn the_selection_moves_together_in_one_undo_step() {
        let (mut tl, mut h, ids) = setup();
        let cmd = move_selected(&tl, &ids, 2.0, 0.0).unwrap();
        assert!(h.execute(&mut tl, cmd));
        let times: Vec<f64> = tl.markers.iter().map(|m| m.time).collect();
        assert_eq!(times, vec![3.0, 5.0, 9.0], "the far marker stays");
        assert_eq!((tl.regions[0].start, tl.regions[0].end), (6.0, 8.0));
        assert!(h.undo(&mut tl));
        assert_eq!(tl.markers[0].time, 1.0);
        assert_eq!(tl.regions[0].start, 4.0);
        // Moving before zero stops where the earliest mark reaches zero
        let cmd = move_selected(&tl, &ids, -10.0, 0.0).unwrap();
        assert!(h.execute(&mut tl, cmd));
        assert_eq!(tl.markers[0].time, 0.0);
        assert_eq!(tl.markers[1].time, 2.0, "distances are kept");
        assert!(move_selected(&tl, &[], 1.0, 0.0).is_none());
        assert!(move_selected(&tl, &ids, 0.0, 0.0).is_none());
    }

    #[test]
    fn regions_resize_by_an_edge_and_keep_a_minimum_length() {
        let (mut tl, mut h, ids) = setup();
        let cmd = resize_selected(&tl, &ids, RegionEdge::End, 1.5, 0.04).unwrap();
        assert!(h.execute(&mut tl, cmd));
        assert_eq!((tl.regions[0].start, tl.regions[0].end), (4.0, 7.5));
        assert_eq!(tl.markers[0].time, 1.0, "markers do not resize");
        let cmd = resize_selected(&tl, &ids, RegionEdge::Start, 100.0, 0.04).unwrap();
        assert!(h.execute(&mut tl, cmd));
        assert!((tl.regions[0].end - tl.regions[0].start - 0.04).abs() < 1e-9);
        // Already at the minimum: shrinking more changes nothing
        assert!(resize_selected(&tl, &ids, RegionEdge::End, -100.0, 0.04).is_none());
        assert!(tl.regions[0].end > tl.regions[0].start);
    }

    #[test]
    fn stretching_scales_the_distances_from_the_anchor() {
        let (mut tl, mut h, ids) = setup();
        let cmd = stretch_selected(&tl, &ids, 1.0, 2.0, 0.04).unwrap();
        assert!(h.execute(&mut tl, cmd));
        // marker at 3 -> 5; region 4..6 -> 7..11; the marker on the anchor stays
        assert_eq!(tl.markers[0].time, 1.0);
        assert_eq!(tl.markers[1].time, 5.0);
        assert_eq!((tl.regions[0].start, tl.regions[0].end), (7.0, 11.0));
        assert!(stretch_selected(&tl, &ids, 1.0, 0.0, 0.04).is_none());
        assert!(stretch_selected(&tl, &ids, 1.0, 1.0, 0.04).is_none());
    }

    #[test]
    fn the_selected_marks_are_removed_together() {
        let (mut tl, mut h, ids) = setup();
        let cmd = remove_selected(&tl, &ids).unwrap();
        assert!(h.execute(&mut tl, cmd));
        assert_eq!(tl.markers.len(), 1);
        assert!(tl.regions.is_empty());
        assert!(h.undo(&mut tl));
        assert_eq!(tl.markers.len(), 3);
    }
}
