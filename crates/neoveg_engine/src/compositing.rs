//! How a video track blends over the tracks below it: the track's compositing mode (Source Alpha,
//! Add, Multiply, ... see `neoveg_common::composite::CompositeMode`), changed as one undo step.
//!
//! Only video tracks that hold media take a mode. An adjustment track runs its effects on what is
//! below it and has no layer of its own to blend.

use neoveg_common::composite::CompositeMode;

use crate::command::EditCommand;
use crate::timeline::{Timeline, TrackId, TrackKind};

/// `true` when `track_id` is a video track with a layer of its own (not an adjustment track).
pub fn takes_mode(timeline: &Timeline, track_id: TrackId) -> bool {
    timeline
        .track(track_id)
        .is_some_and(|t| t.kind == TrackKind::Video && !t.adjustment)
}

/// The edit that gives `track_id` the compositing `mode`; `None` when it already has it or the track
/// does not take a mode.
pub fn set_mode(
    timeline: &Timeline,
    track_id: TrackId,
    mode: CompositeMode,
) -> Option<EditCommand> {
    let track = timeline.track(track_id)?;
    (takes_mode(timeline, track_id) && track.composite_mode != mode).then_some(
        EditCommand::SetCompositeMode {
            track_id,
            before: track.composite_mode,
            after: mode,
        },
    )
}

/// The edit that gives every video track that takes a mode the compositing `mode`, as one undo step.
pub fn set_mode_all(timeline: &Timeline, mode: CompositeMode) -> Option<EditCommand> {
    let mut commands: Vec<EditCommand> = timeline
        .tracks
        .iter()
        .filter_map(|t| set_mode(timeline, t.id, mode))
        .collect();
    match commands.len() {
        0 => None,
        1 => commands.pop(),
        _ => Some(EditCommand::Batch(commands)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;

    fn timeline() -> (Timeline, [TrackId; 4]) {
        let mut tl = Timeline::new();
        let mut ids = [0; 4];
        for (i, (kind, adjustment)) in [
            (TrackKind::Video, false),
            (TrackKind::Video, false),
            (TrackKind::Video, true),
            (TrackKind::Audio, false),
        ]
        .into_iter()
        .enumerate()
        {
            let mut track = tl.new_track(kind);
            track.adjustment = adjustment;
            ids[i] = track.id;
            tl.tracks.push(track);
        }
        (tl, ids)
    }

    #[test]
    fn a_video_track_takes_a_mode_in_one_undo_step() {
        let (mut tl, ids) = timeline();
        assert_eq!(tl.tracks[0].composite_mode, CompositeMode::SourceAlpha);
        // The mode it already has is not an edit
        assert!(set_mode(&tl, ids[0], CompositeMode::SourceAlpha).is_none());
        let original = tl.clone();
        let mut history = History::new();
        let command = set_mode(&tl, ids[0], CompositeMode::Multiply).unwrap();
        assert!(history.execute(&mut tl, command));
        assert_eq!(tl.tracks[0].composite_mode, CompositeMode::Multiply);
        assert_eq!(tl.tracks[1].composite_mode, CompositeMode::SourceAlpha);
        assert!(history.undo(&mut tl));
        assert_eq!(tl, original);
        assert!(history.redo(&mut tl));
        assert_eq!(tl.tracks[0].composite_mode, CompositeMode::Multiply);
    }

    #[test]
    fn audio_adjustment_and_missing_tracks_take_no_mode() {
        let (tl, ids) = timeline();
        assert!(
            set_mode(&tl, ids[2], CompositeMode::Add).is_none(),
            "adjustment"
        );
        assert!(set_mode(&tl, ids[3], CompositeMode::Add).is_none(), "audio");
        assert!(
            set_mode(&tl, 9999, CompositeMode::Add).is_none(),
            "no such track"
        );
        assert!(takes_mode(&tl, ids[0]) && !takes_mode(&tl, ids[2]));
    }

    #[test]
    fn all_video_tracks_change_together_and_undo_together() {
        let (mut tl, ids) = timeline();
        let original = tl.clone();
        let mut history = History::new();
        let command = set_mode_all(&tl, CompositeMode::Screen).unwrap();
        assert!(history.execute(&mut tl, command));
        let mode_of = |tl: &Timeline, i: usize| tl.track(ids[i]).unwrap().composite_mode;
        assert_eq!(mode_of(&tl, 0), CompositeMode::Screen);
        assert_eq!(mode_of(&tl, 1), CompositeMode::Screen);
        // The adjustment and audio tracks are left alone
        assert_eq!(mode_of(&tl, 2), CompositeMode::SourceAlpha);
        assert_eq!(mode_of(&tl, 3), CompositeMode::SourceAlpha);
        assert!(history.undo(&mut tl));
        assert_eq!(tl, original);
        // Nothing left to change once they all have it
        assert!(history.redo(&mut tl));
        assert!(set_mode_all(&tl, CompositeMode::Screen).is_none());
    }

    #[test]
    fn a_mode_is_saved_with_the_project_and_the_default_is_left_out() {
        let (mut tl, ids) = timeline();
        let plain = serde_json::to_string(&tl).unwrap();
        assert!(
            !plain.contains("composite_mode"),
            "the default is not written"
        );
        tl.track_mut(ids[0]).unwrap().composite_mode = CompositeMode::DifferenceSquared;
        let json = serde_json::to_string(&tl).unwrap();
        assert!(json.contains("\"composite_mode\":\"difference_squared\""));
        let back: Timeline = serde_json::from_str(&json).unwrap();
        assert_eq!(
            back.track(ids[0]).unwrap().composite_mode,
            CompositeMode::DifferenceSquared
        );
        // A project from before the mode existed reads as Source Alpha
        let old: Timeline = serde_json::from_str(&plain).unwrap();
        assert_eq!(
            old.track(ids[0]).unwrap().composite_mode,
            CompositeMode::SourceAlpha
        );
    }
}
