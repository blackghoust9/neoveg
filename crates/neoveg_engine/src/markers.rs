//! Markers and regions: named points and spans on the project ruler.
//!
//! They live in the [`Timeline`] so every change goes through `History::execute` and is one undo
//! step. The functions here are pure: they build an [`EditCommand`] (or answer a question) and
//! never touch the UI.

use crate::command::EditCommand;
use crate::edit::quantize;
use crate::selection::TimeRange;
use crate::timeline::{Timeline, TIME_EPSILON};
use serde::{Deserialize, Serialize};

pub type MarkId = u64;

/// A named point on the ruler.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Marker {
    pub id: MarkId,
    pub time: f64,
    pub name: String,
}

/// A named span on the ruler.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Region {
    pub id: MarkId,
    pub start: f64,
    pub end: f64,
    pub name: String,
}

impl Region {
    pub fn range(&self) -> TimeRange {
        TimeRange::new(self.start, self.end)
    }
}

/// Every marker and region of a timeline, kept sorted by time. Stored whole in the undo command.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Marks {
    #[serde(default)]
    pub markers: Vec<Marker>,
    #[serde(default)]
    pub regions: Vec<Region>,
}

impl Marks {
    pub fn is_empty(&self) -> bool {
        self.markers.is_empty() && self.regions.is_empty()
    }

    fn sort(&mut self) {
        self.markers.sort_by(|a, b| a.time.total_cmp(&b.time));
        self.regions.sort_by(|a, b| a.start.total_cmp(&b.start));
    }
}

impl Timeline {
    pub fn marks(&self) -> Marks {
        Marks {
            markers: self.markers.clone(),
            regions: self.regions.clone(),
        }
    }

    pub(crate) fn set_marks(&mut self, marks: &Marks) {
        self.markers = marks.markers.clone();
        self.regions = marks.regions.clone();
    }
}

fn replace(timeline: &Timeline, mut after: Marks) -> Option<EditCommand> {
    after.sort();
    let before = timeline.marks();
    (before != after).then_some(EditCommand::ReplaceMarks { before, after })
}

fn default_name(prefix: &str, count: usize) -> String {
    format!("{prefix} {}", count + 1)
}

/// Adds a marker at `time`. `None` when a marker already sits there (within a tolerance).
pub fn add_marker(timeline: &mut Timeline, time: f64, name: Option<&str>) -> Option<EditCommand> {
    let time = time.max(0.0);
    if timeline
        .markers
        .iter()
        .any(|m| (m.time - time).abs() <= TIME_EPSILON)
    {
        return None;
    }
    let mut after = timeline.marks();
    let name = name
        .map(str::to_owned)
        .unwrap_or_else(|| default_name("Marcador", after.markers.len()));
    let id = timeline.alloc_id();
    after.markers.push(Marker { id, time, name });
    replace(timeline, after)
}

/// Adds a region over `range`. `None` for an empty range.
pub fn add_region(
    timeline: &mut Timeline,
    range: TimeRange,
    name: Option<&str>,
) -> Option<EditCommand> {
    if range.is_empty() {
        return None;
    }
    let mut after = timeline.marks();
    let name = name
        .map(str::to_owned)
        .unwrap_or_else(|| default_name("Região", after.regions.len()));
    let id = timeline.alloc_id();
    after.regions.push(Region {
        id,
        start: range.start,
        end: range.end,
        name,
    });
    replace(timeline, after)
}

/// Moves a marker, or both ends of a region, so it starts at `to`. `None` for an unknown id.
pub fn move_mark(timeline: &Timeline, id: MarkId, to: f64) -> Option<EditCommand> {
    let to = to.max(0.0);
    let mut after = timeline.marks();
    if let Some(m) = after.markers.iter_mut().find(|m| m.id == id) {
        m.time = to;
    } else {
        let r = after.regions.iter_mut().find(|r| r.id == id)?;
        let length = r.end - r.start;
        r.start = to;
        r.end = to + length;
    }
    replace(timeline, after)
}

/// Renames a marker or region. `None` for an unknown id.
pub fn rename_mark(timeline: &Timeline, id: MarkId, name: &str) -> Option<EditCommand> {
    let mut after = timeline.marks();
    if let Some(m) = after.markers.iter_mut().find(|m| m.id == id) {
        m.name = name.to_owned();
    } else {
        let r = after.regions.iter_mut().find(|r| r.id == id)?;
        r.name = name.to_owned();
    }
    replace(timeline, after)
}

/// Removes the marker or region `id`. `None` for an unknown id.
pub fn remove_mark(timeline: &Timeline, id: MarkId) -> Option<EditCommand> {
    let mut after = timeline.marks();
    after.markers.retain(|m| m.id != id);
    after.regions.retain(|r| r.id != id);
    replace(timeline, after)
}

/// Removes every marker inside `range` and every region that lies entirely inside it
/// (everything, when `range` is `None`).
pub fn clear_marks(timeline: &Timeline, range: Option<TimeRange>) -> Option<EditCommand> {
    let mut after = timeline.marks();
    match range {
        None => after = Marks::default(),
        Some(range) => {
            after.markers.retain(|m| !range.contains(m.time));
            after
                .regions
                .retain(|r| !(range.contains(r.start) && range.contains(r.end)));
        }
    }
    replace(timeline, after)
}

/// The part of a marker or region a pointer grabbed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkPart {
    Marker,
    RegionBody,
    RegionStart,
    RegionEnd,
}

/// What lies under `time`: a marker within `tolerance` seconds, else a region edge within it, else
/// a region body. On a region narrower than three tolerances the edges shrink so the body can
/// still be grabbed.
pub fn hit_test(timeline: &Timeline, time: f64, tolerance: f64) -> Option<(MarkId, MarkPart)> {
    if let Some(m) = timeline
        .markers
        .iter()
        .find(|m| (m.time - time).abs() <= tolerance)
    {
        return Some((m.id, MarkPart::Marker));
    }
    timeline.regions.iter().find_map(|r| {
        if time < r.start - tolerance || time > r.end + tolerance {
            return None;
        }
        let edge = tolerance.min((r.end - r.start) / 3.0);
        if (time - r.start).abs() <= edge {
            Some((r.id, MarkPart::RegionStart))
        } else if (time - r.end).abs() <= edge {
            Some((r.id, MarkPart::RegionEnd))
        } else if time > r.start && time < r.end {
            Some((r.id, MarkPart::RegionBody))
        } else {
            None
        }
    })
}

/// The span a drag shows: where the grabbed `part` of a mark that spans `start..end` lands when
/// the pointer is at `pointer` and was `grab_offset` seconds from the part's anchor when grabbed.
/// Times snap to frames, never go below zero, and a region keeps at least one frame.
pub fn drag_span(
    part: MarkPart,
    (start, end): (f64, f64),
    grab_offset: f64,
    pointer: f64,
    fps: f64,
) -> (f64, f64) {
    let frame = if fps > 0.0 { 1.0 / fps } else { TIME_EPSILON };
    let at = quantize((pointer - grab_offset).max(0.0), fps);
    match part {
        MarkPart::Marker => (at, at),
        MarkPart::RegionBody => (at, at + (end - start)),
        MarkPart::RegionStart => (at.min(end - frame).max(0.0), end),
        MarkPart::RegionEnd => (start, at.max(start + frame)),
    }
}

/// The time a grab is measured from: the region's end for its end handle, else the start.
pub fn part_anchor(part: MarkPart, (start, end): (f64, f64)) -> f64 {
    match part {
        MarkPart::RegionEnd => end,
        _ => start,
    }
}

/// Sets a marker's time (`start`) or a region's `start..end`. `None` for an unknown id, an empty
/// region or no change.
pub fn set_mark_span(timeline: &Timeline, id: MarkId, start: f64, end: f64) -> Option<EditCommand> {
    let mut after = timeline.marks();
    if let Some(m) = after.markers.iter_mut().find(|m| m.id == id) {
        m.time = start.max(0.0);
    } else {
        let r = after.regions.iter_mut().find(|r| r.id == id)?;
        if end - start <= TIME_EPSILON {
            return None;
        }
        r.start = start.max(0.0);
        r.end = end;
    }
    replace(timeline, after)
}

/// The `start..end` span of a marker (both its time) or a region.
pub fn span_of(timeline: &Timeline, id: MarkId) -> Option<(f64, f64)> {
    if let Some(m) = timeline.markers.iter().find(|m| m.id == id) {
        return Some((m.time, m.time));
    }
    let r = timeline.regions.iter().find(|r| r.id == id)?;
    Some((r.start, r.end))
}

/// Times that the cursor can jump to: markers and the edges of regions, ascending.
fn stops(timeline: &Timeline) -> Vec<f64> {
    let mut times: Vec<f64> = timeline
        .markers
        .iter()
        .map(|m| m.time)
        .chain(timeline.regions.iter().flat_map(|r| [r.start, r.end]))
        .collect();
    times.sort_by(f64::total_cmp);
    times
}

/// The closest marker or region edge strictly before `time`.
pub fn previous_stop(timeline: &Timeline, time: f64) -> Option<f64> {
    stops(timeline)
        .into_iter()
        .rev()
        .find(|t| *t < time - TIME_EPSILON)
}

/// The closest marker or region edge strictly after `time`.
pub fn next_stop(timeline: &Timeline, time: f64) -> Option<f64> {
    stops(timeline)
        .into_iter()
        .find(|t| *t > time + TIME_EPSILON)
}

/// The `n`-th marker (1-based, by time) for the `1`–`9` jump keys, `0` being the tenth.
pub fn marker_by_number(timeline: &Timeline, n: usize) -> Option<&Marker> {
    let index = if n == 0 { 9 } else { n - 1 };
    let mut sorted: Vec<&Marker> = timeline.markers.iter().collect();
    sorted.sort_by(|a, b| a.time.total_cmp(&b.time));
    sorted.get(index).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;

    fn run(tl: &mut Timeline, h: &mut History, cmd: Option<EditCommand>) {
        assert!(h.execute(tl, cmd.expect("command")));
    }

    /// Builds a command from the timeline and executes it (the builder borrows `tl` first).
    macro_rules! exec {
        ($tl:ident, $h:ident, $build:expr) => {{
            let cmd = $build;
            run(&mut $tl, &mut $h, cmd);
        }};
    }

    #[test]
    fn markers_are_sorted_named_and_not_duplicated() {
        let mut tl = Timeline::new();
        let mut h = History::new();
        let c = add_marker(&mut tl, 5.0, None);
        run(&mut tl, &mut h, c);
        let c = add_marker(&mut tl, 2.0, Some("Intro"));
        run(&mut tl, &mut h, c);
        assert!(add_marker(&mut tl, 5.0, None).is_none());

        let times: Vec<f64> = tl.markers.iter().map(|m| m.time).collect();
        assert_eq!(times, vec![2.0, 5.0]);
        assert_eq!(tl.markers[0].name, "Intro");
        assert_eq!(tl.markers[1].name, "Marcador 1");
    }

    #[test]
    fn every_change_is_one_exact_undo_step() {
        let mut tl = Timeline::new();
        let mut h = History::new();
        let c = add_marker(&mut tl, 1.0, None);
        run(&mut tl, &mut h, c);
        // Ids are never reused, so `next_id` is not part of the comparison
        let before = tl.marks();
        let c = add_region(&mut tl, TimeRange::new(2.0, 4.0), None);
        run(&mut tl, &mut h, c);
        let after = tl.marks();
        assert_eq!(tl.regions.len(), 1);

        assert!(h.undo(&mut tl));
        assert_eq!(tl.marks(), before);
        assert!(h.redo(&mut tl));
        assert_eq!(tl.marks(), after);
    }

    #[test]
    fn empty_region_and_unknown_ids_are_rejected() {
        let mut tl = Timeline::new();
        assert!(add_region(&mut tl, TimeRange::new(3.0, 3.0), None).is_none());
        assert!(move_mark(&tl, 99, 1.0).is_none());
        assert!(rename_mark(&tl, 99, "x").is_none());
        assert!(remove_mark(&tl, 99).is_none());
    }

    #[test]
    fn move_rename_and_remove() {
        let mut tl = Timeline::new();
        let mut h = History::new();
        let c = add_marker(&mut tl, 1.0, None);
        run(&mut tl, &mut h, c);
        let c = add_region(&mut tl, TimeRange::new(2.0, 4.0), None);
        run(&mut tl, &mut h, c);
        let (marker, region) = (tl.markers[0].id, tl.regions[0].id);

        exec!(tl, h, move_mark(&tl, marker, 9.0));
        exec!(tl, h, move_mark(&tl, region, 10.0));
        exec!(tl, h, rename_mark(&tl, region, "Cena"));
        assert_eq!(tl.markers[0].time, 9.0);
        assert_eq!(
            (
                tl.regions[0].start,
                tl.regions[0].end,
                tl.regions[0].name.as_str()
            ),
            (10.0, 12.0, "Cena")
        );

        exec!(tl, h, remove_mark(&tl, marker));
        assert!(tl.markers.is_empty() && tl.regions.len() == 1);
    }

    #[test]
    fn clear_in_range_keeps_what_sticks_out() {
        let mut tl = Timeline::new();
        let mut h = History::new();
        for t in [1.0, 3.0, 8.0] {
            let c = add_marker(&mut tl, t, None);
            run(&mut tl, &mut h, c);
        }
        let c = add_region(&mut tl, TimeRange::new(2.0, 4.0), None);
        run(&mut tl, &mut h, c);
        let c = add_region(&mut tl, TimeRange::new(6.0, 12.0), None);
        run(&mut tl, &mut h, c);

        exec!(tl, h, clear_marks(&tl, Some(TimeRange::new(0.0, 5.0))));
        assert_eq!(tl.markers.len(), 1);
        assert_eq!(tl.regions.len(), 1);
        assert!(clear_marks(&tl, Some(TimeRange::new(0.0, 1.0))).is_none());
        exec!(tl, h, clear_marks(&tl, None));
        assert!(tl.marks().is_empty());
    }

    #[test]
    fn navigation_visits_markers_and_region_edges() {
        let mut tl = Timeline::new();
        let mut h = History::new();
        let c = add_marker(&mut tl, 1.0, None);
        run(&mut tl, &mut h, c);
        let c = add_region(&mut tl, TimeRange::new(4.0, 6.0), None);
        run(&mut tl, &mut h, c);

        assert_eq!(next_stop(&tl, 0.0), Some(1.0));
        assert_eq!(next_stop(&tl, 1.0), Some(4.0));
        assert_eq!(next_stop(&tl, 6.0), None);
        assert_eq!(previous_stop(&tl, 5.0), Some(4.0));
        assert_eq!(previous_stop(&tl, 1.0), None);
    }

    #[test]
    fn hit_test_prefers_markers_then_region_edges_then_the_body() {
        let mut tl = Timeline::new();
        let mut h = History::new();
        let c = add_marker(&mut tl, 5.0, None);
        run(&mut tl, &mut h, c);
        let c = add_region(&mut tl, TimeRange::new(4.0, 10.0), None);
        run(&mut tl, &mut h, c);
        let (marker, region) = (tl.markers[0].id, tl.regions[0].id);

        assert_eq!(hit_test(&tl, 5.02, 0.05), Some((marker, MarkPart::Marker)));
        assert_eq!(
            hit_test(&tl, 4.03, 0.05),
            Some((region, MarkPart::RegionStart))
        );
        assert_eq!(
            hit_test(&tl, 10.04, 0.05),
            Some((region, MarkPart::RegionEnd))
        );
        assert_eq!(
            hit_test(&tl, 7.0, 0.05),
            Some((region, MarkPart::RegionBody))
        );
        assert_eq!(hit_test(&tl, 12.0, 0.05), None);
    }

    #[test]
    fn a_narrow_region_keeps_a_body_to_grab() {
        let mut tl = Timeline::new();
        let mut h = History::new();
        let c = add_region(&mut tl, TimeRange::new(4.0, 4.06), None);
        run(&mut tl, &mut h, c);
        let id = tl.regions[0].id;
        assert_eq!(hit_test(&tl, 4.03, 0.05), Some((id, MarkPart::RegionBody)));
    }

    #[test]
    fn dragging_snaps_to_frames_and_keeps_a_region_one_frame_long() {
        let fps = 25.0;
        let frame = 1.0 / fps;
        let region = (2.0, 4.0);
        // The body moves as a block, the grab offset keeps it under the pointer
        let (s, e) = drag_span(MarkPart::RegionBody, region, 0.5, 6.51, fps);
        assert!((s - 6.0).abs() < 1e-9 && (e - 8.0).abs() < 1e-9, "{s} {e}");
        // Edges cannot cross
        let (s, e) = drag_span(MarkPart::RegionStart, region, 0.0, 9.0, fps);
        assert!((s - (4.0 - frame)).abs() < 1e-9 && e == 4.0);
        let (s, e) = drag_span(MarkPart::RegionEnd, region, 0.0, 0.5, fps);
        assert!(s == 2.0 && (e - (2.0 + frame)).abs() < 1e-9);
        // A marker never goes below zero
        assert_eq!(
            drag_span(MarkPart::Marker, (3.0, 3.0), 0.0, -2.0, fps),
            (0.0, 0.0)
        );
    }

    #[test]
    fn set_mark_span_moves_a_marker_and_resizes_a_region_in_one_step() {
        let mut tl = Timeline::new();
        let mut h = History::new();
        let c = add_marker(&mut tl, 1.0, None);
        run(&mut tl, &mut h, c);
        let c = add_region(&mut tl, TimeRange::new(2.0, 4.0), None);
        run(&mut tl, &mut h, c);
        let (marker, region) = (tl.markers[0].id, tl.regions[0].id);

        exec!(tl, h, set_mark_span(&tl, marker, 3.0, 3.0));
        exec!(tl, h, set_mark_span(&tl, region, 2.0, 9.0));
        assert_eq!(tl.markers[0].time, 3.0);
        assert_eq!((tl.regions[0].start, tl.regions[0].end), (2.0, 9.0));
        assert!(h.undo(&mut tl));
        assert_eq!(tl.regions[0].end, 4.0);

        assert!(
            set_mark_span(&tl, region, 5.0, 5.0).is_none(),
            "empty region"
        );
        assert!(set_mark_span(&tl, region, 2.0, 4.0).is_none(), "no change");
        assert!(set_mark_span(&tl, 99, 1.0, 2.0).is_none());
        assert_eq!(span_of(&tl, marker), Some((3.0, 3.0)));
        assert_eq!(span_of(&tl, region), Some((2.0, 4.0)));
    }

    #[test]
    fn number_keys_pick_markers_by_time() {
        let mut tl = Timeline::new();
        let mut h = History::new();
        for t in [7.0, 3.0] {
            let c = add_marker(&mut tl, t, None);
            run(&mut tl, &mut h, c);
        }
        assert_eq!(marker_by_number(&tl, 1).unwrap().time, 3.0);
        assert_eq!(marker_by_number(&tl, 2).unwrap().time, 7.0);
        assert!(marker_by_number(&tl, 3).is_none());
        assert!(marker_by_number(&tl, 0).is_none());
    }
}
