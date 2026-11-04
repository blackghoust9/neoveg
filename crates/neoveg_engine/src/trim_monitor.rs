//! The edits of the trim monitor: the two events of a cut, seen side by side (the last frame of the
//! first and the first frame of the second), and the cut moved a frame or a few at a time.
//!
//! The monitor is an addition to the VEGAS ways of trimming, which stay as they are (drag an edge,
//! `Ctrl`+`Alt`+drag for the rolling trim, the numeric keypad); its three modes do what those do:
//! *Roll* moves the cut for both events at once (the rolling trim of [`edit::roll`]), *Out* trims
//! the end of the first event and *In* the start of the second (the edge trims of
//! [`edit::trim_right`] and [`edit::trim_left`]). The functions are pure: they build an
//! [`EditCommand`].

use crate::command::EditCommand;
use crate::edit;
use crate::timeline::{EventGeometry, EventId, Timeline, TrackId, TIME_EPSILON};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrimMode {
    /// The cut moves for both events: what one gains the other loses.
    #[default]
    Roll,
    /// The end of the first event.
    Out,
    /// The start of the second event.
    In,
}

impl TrimMode {
    pub const ALL: [TrimMode; 3] = [Self::Roll, Self::Out, Self::In];

    /// Name in the window (a key of the translation files).
    pub fn label(self) -> &'static str {
        match self {
            Self::Roll => "Rolar o corte",
            Self::Out => "Aparar a saída",
            Self::In => "Aparar a entrada",
        }
    }
}

/// Where the two events meet: the end of the first and the start of the second. `None` when either
/// does not exist on the track.
pub fn edges(
    timeline: &Timeline,
    track_id: TrackId,
    left: EventId,
    right: EventId,
) -> Option<(f64, f64)> {
    let track = timeline.track(track_id)?;
    let l = track.events.iter().find(|e| e.id == left)?;
    let r = track.events.iter().find(|e| e.id == right)?;
    Some((l.end(), r.start))
}

fn edit_command(
    track_id: TrackId,
    event_id: EventId,
    old: EventGeometry,
    new: EventGeometry,
) -> Option<EditCommand> {
    (old != new).then_some(EditCommand::EditEvent {
        from_track: track_id,
        to_track: track_id,
        event_id,
        old,
        new,
    })
}

/// Moves the edge(s) of the cut by `frames` frames of `fps` (negative: earlier). The first event
/// can grow only into the media it has left (`left_source_len`, when known), the second only back
/// into its own, and each keeps at least one frame. `None` when nothing moves.
#[allow(clippy::too_many_arguments)]
pub fn nudge(
    timeline: &Timeline,
    track_id: TrackId,
    left_id: EventId,
    right_id: EventId,
    mode: TrimMode,
    frames: i32,
    fps: f64,
    left_source_len: Option<f64>,
) -> Option<EditCommand> {
    if frames == 0 || !(fps.is_finite() && fps > 0.0) {
        return None;
    }
    let track = timeline.track(track_id)?;
    let left = track.events.iter().find(|e| e.id == left_id)?.geometry();
    let right = track.events.iter().find(|e| e.id == right_id)?.geometry();
    let delta = f64::from(frames) / fps;
    let min_len = 1.0 / fps;
    let (new_left, new_right) = match mode {
        TrimMode::Out => (
            edit::trim_right(left, left.end() + delta, min_len, left_source_len),
            right,
        ),
        TrimMode::In => (left, edit::trim_left(right, right.start + delta, min_len)),
        TrimMode::Roll if (left.end() - right.start).abs() <= TIME_EPSILON => {
            edit::roll(left, right, left.end() + delta, min_len, left_source_len)
        }
        TrimMode::Roll => {
            // The edges are apart: both move by what the one with less room can move
            let l = edit::trim_right(left, left.end() + delta, min_len, left_source_len);
            let r = edit::trim_left(right, right.start + delta, min_len);
            let moved = (l.end() - left.end())
                .abs()
                .min((r.start - right.start).abs());
            let by = moved.copysign(delta);
            (
                edit::trim_right(left, left.end() + by, min_len, left_source_len),
                edit::trim_left(right, right.start + by, min_len),
            )
        }
    };
    let commands: Vec<EditCommand> = [
        edit_command(track_id, left_id, left, new_left),
        edit_command(track_id, right_id, right, new_right),
    ]
    .into_iter()
    .flatten()
    .collect();
    match commands.len() {
        0 => None,
        1 => commands.into_iter().next(),
        _ => Some(EditCommand::Batch(commands)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::timeline::{TrackEvent, TrackKind};

    const FPS: f64 = 25.0;

    fn event(id: u64, start: f64, duration: f64, trim_in: f64) -> TrackEvent {
        TrackEvent {
            id,
            start,
            duration,
            trim_in,
            media_id: Some(1),
            ..Default::default()
        }
    }

    fn cut() -> (Timeline, TrackId) {
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Video);
        track.events = vec![event(10, 0.0, 4.0, 0.0), event(11, 4.0, 4.0, 10.0)];
        let id = track.id;
        tl.tracks = vec![track];
        (tl, id)
    }

    fn run(tl: &mut Timeline, command: Option<EditCommand>) -> bool {
        History::new().execute(tl, command.expect("an edit"))
    }

    #[test]
    fn rolling_moves_the_cut_for_both_events_and_keeps_the_total_length() {
        let (mut tl, track) = cut();
        let command = nudge(&tl, track, 10, 11, TrimMode::Roll, 5, FPS, Some(100.0));
        assert!(run(&mut tl, command));
        let (l, r) = edges(&tl, track, 10, 11).unwrap();
        assert!((l - 4.2).abs() < 1e-9 && (r - 4.2).abs() < 1e-9, "{l} {r}");
        let events = &tl.track(track).unwrap().events;
        assert!(
            (events[1].trim_in - 10.2).abs() < 1e-9,
            "the second starts later in its media"
        );
        assert!((events[1].end() - 8.0).abs() < 1e-9);
        // And back
        let command = nudge(&tl, track, 10, 11, TrimMode::Roll, -10, FPS, Some(100.0));
        assert!(run(&mut tl, command));
        assert!((edges(&tl, track, 10, 11).unwrap().0 - 3.8).abs() < 1e-9);
    }

    #[test]
    fn the_out_and_in_modes_trim_one_side_and_leave_a_gap_or_an_overlap() {
        let (mut tl, track) = cut();
        let command = nudge(&tl, track, 10, 11, TrimMode::Out, -2, FPS, None);
        assert!(run(&mut tl, command));
        let (end, start) = edges(&tl, track, 10, 11).unwrap();
        assert!(
            (end - 3.92).abs() < 1e-9 && start == 4.0,
            "a gap of two frames"
        );
        // Rolling with the edges apart moves both by the same amount
        let command = nudge(&tl, track, 10, 11, TrimMode::Roll, 1, FPS, None);
        assert!(run(&mut tl, command));
        let (l, r) = edges(&tl, track, 10, 11).unwrap();
        assert!(
            (l - 3.96).abs() < 1e-9 && (r - 4.04).abs() < 1e-9,
            "{l} {r}"
        );
        let (mut tl, track) = cut();
        let command = nudge(&tl, track, 10, 11, TrimMode::In, 3, FPS, None);
        assert!(run(&mut tl, command));
        let (l, r) = edges(&tl, track, 10, 11).unwrap();
        assert!((l - 4.0).abs() < 1e-9 && (r - 4.12).abs() < 1e-9);
    }

    #[test]
    fn the_media_left_limits_the_move_and_a_move_with_no_room_is_no_edit() {
        let (tl, track) = cut();
        // The first event has 0.04 s of media left: only one frame fits
        let command = nudge(&tl, track, 10, 11, TrimMode::Out, 5, FPS, Some(4.04));
        let mut copy = tl.clone();
        assert!(run(&mut copy, command));
        assert!((edges(&copy, track, 10, 11).unwrap().0 - 4.04).abs() < 1e-9);
        assert!(nudge(&tl, track, 10, 11, TrimMode::Out, 5, FPS, Some(4.0)).is_none());
        assert!(nudge(&tl, track, 10, 11, TrimMode::Roll, 0, FPS, None).is_none());
        // The second event cannot go back past the start of its media
        let mut at_start = tl.clone();
        at_start.tracks[0].events[1].trim_in = 0.0;
        assert!(nudge(&at_start, track, 10, 11, TrimMode::In, -3, FPS, None).is_none());
        assert!(nudge(&tl, track, 10, 99, TrimMode::Roll, 1, FPS, None).is_none());
    }

    #[test]
    fn a_move_is_one_undo_step() {
        let (mut tl, track) = cut();
        let original = tl.clone();
        let mut history = History::new();
        let command = nudge(&tl, track, 10, 11, TrimMode::Roll, 3, FPS, None).unwrap();
        assert!(history.execute(&mut tl, command));
        assert!(history.undo(&mut tl));
        assert_eq!(tl, original);
    }
}
