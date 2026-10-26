//! Markers and regions: named points and spans on the project ruler.
//!
//! They live in the [`Timeline`] so every change goes through `History::execute` and is one undo
//! step. The functions here are pure: they build an [`EditCommand`] (or answer a question) and
//! never touch the UI.

use crate::command::EditCommand;
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
