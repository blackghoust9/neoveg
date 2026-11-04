//! The Trimmer: choosing the part of a media file to use, and turning that choice into an event.
//!
//! Plain state and functions, no window: the Trimmer panel only translates pointer and key input
//! into calls here. A media file is opened in a [`TrimmerState`] (cursor, in point, out point, a
//! small undo history of the marks). The selection can then be added to the timeline:
//!
//! * **Three-point edit** ([`TrimmerState::add_from_cursor`] / [`TrimmerState::add_up_to_cursor`]):
//!   the selection is placed after / before the timeline cursor.
//! * **Four-point edit** ([`TrimmerState::fit_to_fill`]): the selection is time-compressed or
//!   stretched to fill a range of the timeline. Two timeline points and one Trimmer point
//!   ([`TrimmerState::from_timeline_range`]) place the selection into that range without changing
//!   its speed.
//! * By default the new event is laid over what is on the track (punch-in); with *overwrite* the
//!   part of the track it covers is replaced ([`add_command`]).

use crate::command::EditCommand;
use crate::edit::{clamp_playback_rate, quantize};
use crate::ops::{remove_range, Scope};
use crate::selection::TimeRange;
use crate::timeline::{Timeline, TrackEvent, TrackId};

/// The shortest selection, in frames.
const MIN_FRAMES: f64 = 1.0;
/// Number of mark changes the history keeps.
const HISTORY_LIMIT: usize = 100;

/// The in and out points (`None` = not marked).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct Marks {
    mark_in: Option<f64>,
    mark_out: Option<f64>,
}

/// A media file open in the Trimmer.
#[derive(Debug, Clone, PartialEq)]
pub struct TrimmerState {
    pub media_id: u64,
    pub name: String,
    /// Length of the media in seconds.
    pub duration: f64,
    /// Project frame rate (the cursor and the marks sit on frames).
    pub fps: f64,
    cursor: f64,
    marks: Marks,
    undo: Vec<Marks>,
    redo: Vec<Marks>,
}

/// Where and how a selection becomes an event.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    /// Start on the timeline.
    pub start: f64,
    /// Length on the timeline.
    pub duration: f64,
    /// Where in the media the event begins.
    pub trim_in: f64,
    /// `1.0` unless the selection was fitted to a longer or shorter range.
    pub playback_rate: f64,
}

impl Placement {
    pub fn end(&self) -> f64 {
        self.start + self.duration
    }

    /// The event for this placement (`id` comes from `Timeline::alloc_id`).
    pub fn into_event(self, id: u64, name: &str, media_id: u64) -> TrackEvent {
        TrackEvent {
            id,
            name: name.to_owned(),
            media_id: Some(media_id),
            start: self.start,
            duration: self.duration,
            trim_in: self.trim_in,
            playback_rate: self.playback_rate,
            ..Default::default()
        }
    }
}

impl TrimmerState {
    pub fn new(media_id: u64, name: &str, duration: f64, fps: f64) -> Self {
        Self {
            media_id,
            name: name.to_owned(),
            duration: duration.max(0.0),
            fps: if fps > 0.0 { fps } else { 30.0 },
            cursor: 0.0,
            marks: Marks::default(),
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    fn frame(&self) -> f64 {
        1.0 / self.fps
    }

    fn snap(&self, seconds: f64) -> f64 {
        quantize(seconds.clamp(0.0, self.duration), self.fps).clamp(0.0, self.duration)
    }

    pub fn cursor(&self) -> f64 {
        self.cursor
    }

    pub fn set_cursor(&mut self, seconds: f64) {
        self.cursor = self.snap(seconds);
    }

    /// Moves the cursor by whole frames (`←` / `→`, the previous / next frame buttons).
    pub fn step_frames(&mut self, frames: i64) {
        self.set_cursor(self.cursor + frames as f64 * self.frame());
    }

    pub fn mark_in_point(&self) -> Option<f64> {
        self.marks.mark_in
    }

    pub fn mark_out_point(&self) -> Option<f64> {
        self.marks.mark_out
    }

    fn remember(&mut self) {
        self.undo.push(self.marks);
        if self.undo.len() > HISTORY_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// `I` / `[`: marks the in point at the cursor. An out point at or before it is dropped.
    pub fn mark_in(&mut self) {
        self.set_in(self.cursor);
    }

    /// `O` / `]`: marks the out point at the cursor. An in point at or after it is dropped.
    pub fn mark_out(&mut self) {
        self.set_out(self.cursor);
    }

    /// Sets the in point (typed timecode, or a drag on the Trimmer timeline).
    pub fn set_in(&mut self, seconds: f64) {
        let t = self.snap(seconds);
        self.remember();
        self.marks.mark_in = Some(t);
        if self
            .marks
            .mark_out
            .is_some_and(|o| o < t + MIN_FRAMES * self.frame() - 1e-9)
        {
            self.marks.mark_out = None;
        }
    }

    pub fn set_out(&mut self, seconds: f64) {
        let t = self.snap(seconds);
        self.remember();
        self.marks.mark_out = Some(t);
        if self
            .marks
            .mark_in
            .is_some_and(|i| i > t - MIN_FRAMES * self.frame() + 1e-9)
        {
            self.marks.mark_in = None;
        }
    }

    /// Dragging over the Trimmer timeline: left to right sets the in point first, right to left the
    /// out point first; either way the result is an ordered selection of at least one frame.
    pub fn select(&mut self, from: f64, to: f64) {
        let (a, b) = (self.snap(from.min(to)), self.snap(from.max(to)));
        if b - a < MIN_FRAMES * self.frame() - 1e-9 {
            return;
        }
        self.remember();
        self.marks = Marks {
            mark_in: Some(a),
            mark_out: Some(b),
        };
    }

    /// A double click: the in and out points go back to the start and end of the media.
    pub fn reset_marks(&mut self) {
        if self.marks != Marks::default() {
            self.remember();
            self.marks = Marks::default();
        }
    }

    /// The selection in the media: the marks, with the start / end of the media where a point is
    /// not marked.
    pub fn selection(&self) -> (f64, f64) {
        (
            self.marks.mark_in.unwrap_or(0.0),
            self.marks.mark_out.unwrap_or(self.duration),
        )
    }

    pub fn selection_len(&self) -> f64 {
        let (a, b) = self.selection();
        (b - a).max(0.0)
    }

    pub fn undo_marks(&mut self) -> bool {
        match self.undo.pop() {
            Some(previous) => {
                self.redo.push(self.marks);
                self.marks = previous;
                true
            }
            None => false,
        }
    }

    pub fn redo_marks(&mut self) -> bool {
        match self.redo.pop() {
            Some(next) => {
                self.undo.push(self.marks);
                self.marks = next;
                true
            }
            None => false,
        }
    }

    /// *Add to Timeline from Cursor*: the selection starts at the timeline cursor.
    pub fn add_from_cursor(&self, cursor: f64) -> Option<Placement> {
        let (a, _) = self.selection();
        let len = self.selection_len();
        (len > 0.0).then(|| Placement {
            start: cursor.max(0.0),
            duration: len,
            trim_in: a,
            playback_rate: 1.0,
        })
    }

    /// *Add to Timeline up to Cursor*: the selection ends at the timeline cursor. When there is not
    /// enough room before time zero the start of the selection is cut off instead.
    pub fn add_up_to_cursor(&self, cursor: f64) -> Option<Placement> {
        let (a, _) = self.selection();
        let len = self.selection_len();
        if len <= 0.0 || cursor <= 0.0 {
            return None;
        }
        let start = cursor - len;
        Some(if start >= 0.0 {
            Placement {
                start,
                duration: len,
                trim_in: a,
                playback_rate: 1.0,
            }
        } else {
            Placement {
                start: 0.0,
                duration: cursor,
                trim_in: a - start,
                playback_rate: 1.0,
            }
        })
    }

    /// *Fit to Fill* (four-point edit): the selection is compressed or stretched to fill `range` of
    /// the timeline. The speed is limited like any playback rate; when the needed speed is outside
    /// the limit the event gets the limit and falls short of (or runs past) the range, which the
    /// second value reports (`true` = the range is filled exactly).
    pub fn fit_to_fill(&self, range: TimeRange) -> Option<(Placement, bool)> {
        let len = self.selection_len();
        let target = range.end - range.start;
        if len <= 0.0 || target <= 0.0 {
            return None;
        }
        let wanted = len / target;
        let rate = clamp_playback_rate(wanted);
        let exact = (rate - wanted).abs() < 1e-9;
        Some((
            Placement {
                start: range.start,
                duration: len / rate,
                trim_in: self.selection().0,
                playback_rate: rate,
            },
            exact,
        ))
    }

    /// Three-point edit with two timeline points and one Trimmer point: the part of the media that
    /// begins at the in point (or ends at the out point when only that is marked) is placed in
    /// `range` at normal speed. Ripple editing does not apply to it.
    pub fn from_timeline_range(&self, range: TimeRange) -> Option<Placement> {
        let target = range.end - range.start;
        if target <= 0.0 {
            return None;
        }
        let trim_in = match (self.marks.mark_in, self.marks.mark_out) {
            (Some(i), _) => i,
            (None, Some(o)) => (o - target).max(0.0),
            (None, None) => 0.0,
        };
        let duration = target.min((self.duration - trim_in).max(0.0));
        (duration > 0.0).then_some(Placement {
            start: range.start,
            duration,
            trim_in,
            playback_rate: 1.0,
        })
    }
}

/// The undoable command that adds `event` to `track_id`. Without `overwrite` the event is laid over
/// what is on the track (punch-in, the events underneath are kept); with it the part of the track
/// the event covers is removed first, as one undo step. `None` when the track does not exist.
pub fn add_command(
    timeline: &mut Timeline,
    track_id: TrackId,
    event: TrackEvent,
    overwrite: bool,
) -> Option<EditCommand> {
    timeline.track(track_id)?;
    let add = EditCommand::AddEvent {
        track_id,
        event: event.clone(),
    };
    if !overwrite {
        return Some(add);
    }
    let covered = TimeRange::new(event.start, event.start + event.duration);
    match remove_range(timeline, covered, Scope::Tracks(&[track_id]), false) {
        Some(outcome) => Some(EditCommand::Batch(vec![outcome.command, add])),
        None => Some(add),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::timeline::TrackKind;

    fn state() -> TrimmerState {
        TrimmerState::new(7, "clip", 10.0, 25.0)
    }

    #[test]
    fn marks_follow_the_cursor_and_stay_ordered() {
        let mut t = state();
        t.set_cursor(2.0);
        t.mark_in();
        t.set_cursor(6.0);
        t.mark_out();
        assert_eq!(t.selection(), (2.0, 6.0));
        // An out point before the in point drops the in point
        t.set_cursor(1.0);
        t.mark_out();
        assert_eq!(t.mark_in_point(), None);
        assert_eq!(t.selection(), (0.0, 1.0));
        // Without marks the whole media is selected
        t.reset_marks();
        assert_eq!(t.selection(), (0.0, 10.0));
    }

    #[test]
    fn the_cursor_stays_on_frames_inside_the_media() {
        let mut t = state();
        t.set_cursor(1.03);
        assert!((t.cursor() - 1.04).abs() < 1e-9);
        t.set_cursor(99.0);
        assert_eq!(t.cursor(), 10.0);
        t.step_frames(-1);
        assert!((t.cursor() - 9.96).abs() < 1e-9);
        t.set_cursor(-3.0);
        assert_eq!(t.cursor(), 0.0);
    }

    #[test]
    fn dragging_either_way_makes_the_same_selection() {
        let mut t = state();
        t.select(5.0, 3.0);
        assert_eq!(t.selection(), (3.0, 5.0));
        t.select(4.0, 4.0); // less than a frame: ignored
        assert_eq!(t.selection(), (3.0, 5.0));
    }

    #[test]
    fn mark_changes_can_be_undone_and_redone() {
        let mut t = state();
        t.set_cursor(2.0);
        t.mark_in();
        t.set_cursor(4.0);
        t.mark_out();
        assert!(t.undo_marks());
        assert_eq!(t.selection(), (2.0, 10.0));
        assert!(t.redo_marks());
        assert_eq!(t.selection(), (2.0, 4.0));
        assert!(t.undo_marks() && t.undo_marks());
        assert!(!t.undo_marks());
        assert_eq!(t.selection(), (0.0, 10.0));
    }

    #[test]
    fn add_from_and_up_to_the_cursor() {
        let mut t = state();
        t.select(2.0, 5.0);
        let after = t.add_from_cursor(10.0).unwrap();
        assert_eq!(
            (after.start, after.duration, after.trim_in),
            (10.0, 3.0, 2.0)
        );
        let before = t.add_up_to_cursor(10.0).unwrap();
        assert_eq!(
            (before.start, before.end(), before.trim_in),
            (7.0, 10.0, 2.0)
        );
        // Not enough room before zero: the start of the selection is cut off
        let short = t.add_up_to_cursor(1.0).unwrap();
        assert_eq!((short.start, short.duration), (0.0, 1.0));
        assert!((short.trim_in - 4.0).abs() < 1e-9);
        assert!(t.add_up_to_cursor(0.0).is_none());
    }

    #[test]
    fn fit_to_fill_changes_the_speed() {
        let mut t = state();
        t.select(0.0, 4.0);
        let (p, exact) = t.fit_to_fill(TimeRange::new(10.0, 18.0)).unwrap();
        assert!(exact);
        assert!((p.playback_rate - 0.5).abs() < 1e-9);
        assert!((p.duration - 8.0).abs() < 1e-9 && p.start == 10.0);
        // Faster
        let (p, _) = t.fit_to_fill(TimeRange::new(0.0, 2.0)).unwrap();
        assert!((p.playback_rate - 2.0).abs() < 1e-9);
        // Beyond the speed limit: the limit is used and the range is not filled exactly
        let (p, exact) = t.fit_to_fill(TimeRange::new(0.0, 4000.0)).unwrap();
        assert!(!exact && p.duration < 4000.0);
        assert!(t.fit_to_fill(TimeRange::new(3.0, 3.0)).is_none());
    }

    #[test]
    fn two_timeline_points_and_one_trimmer_point() {
        let mut t = state();
        t.set_cursor(3.0);
        t.mark_in();
        let p = t.from_timeline_range(TimeRange::new(20.0, 24.0)).unwrap();
        assert_eq!(
            (p.start, p.duration, p.trim_in, p.playback_rate),
            (20.0, 4.0, 3.0, 1.0)
        );
        // Only an out point: the part that ends there
        let mut t = state();
        t.set_cursor(6.0);
        t.mark_out();
        let p = t.from_timeline_range(TimeRange::new(0.0, 4.0)).unwrap();
        assert_eq!(p.trim_in, 2.0);
        // The media is shorter than the range
        let mut t = state();
        t.set_cursor(8.0);
        t.mark_in();
        let p = t.from_timeline_range(TimeRange::new(0.0, 5.0)).unwrap();
        assert_eq!(p.duration, 2.0);
    }

    fn timeline_with_event() -> (Timeline, TrackId) {
        let mut tl = Timeline::new();
        let track = tl.new_track(TrackKind::Audio);
        let track_id = track.id;
        let mut history = History::new();
        history.execute(&mut tl, EditCommand::AddTrack { track });
        let event = TrackEvent {
            id: tl.alloc_id(),
            name: "base".into(),
            media_id: Some(1),
            start: 0.0,
            duration: 10.0,
            ..Default::default()
        };
        history.execute(&mut tl, EditCommand::AddEvent { track_id, event });
        (tl, track_id)
    }

    #[test]
    fn punch_in_keeps_the_events_underneath_and_overwrite_replaces_them() {
        let mut t = state();
        t.select(1.0, 3.0);
        let placement = t.add_from_cursor(4.0).unwrap();

        let (mut tl, track_id) = timeline_with_event();
        let event = placement.into_event(tl.alloc_id(), "clip", 7);
        let command = add_command(&mut tl, track_id, event, false).unwrap();
        let mut history = History::new();
        history.execute(&mut tl, command);
        assert_eq!(tl.track(track_id).unwrap().events.len(), 2);

        let (mut tl, track_id) = timeline_with_event();
        let event = placement.into_event(tl.alloc_id(), "clip", 7);
        let command = add_command(&mut tl, track_id, event, true).unwrap();
        let mut history = History::new();
        history.execute(&mut tl, command);
        let events = &tl.track(track_id).unwrap().events;
        // The base event is cut into the part before and the part after the new one
        assert_eq!(events.len(), 3);
        assert!(events
            .iter()
            .any(|e| e.media_id == Some(7) && e.start == 4.0));
        assert!(events
            .iter()
            .any(|e| e.name == "base" && e.end() <= 4.0 + 1e-9));
        assert!(events
            .iter()
            .any(|e| e.name == "base" && e.start >= 6.0 - 1e-9));
        // One undo step brings the track back
        assert!(history.undo(&mut tl));
        assert_eq!(tl.track(track_id).unwrap().events.len(), 1);
    }

    #[test]
    fn a_missing_track_is_refused() {
        let (mut tl, _) = timeline_with_event();
        let event = TrackEvent::default();
        assert!(add_command(&mut tl, 999_999, event, false).is_none());
    }
}
