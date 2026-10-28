//! Motion of video tracks: the pure builder of its undoable command.
//!
//! The model (`TrackMotion`, its keys and interpolation) lives in `neoveg_common::pan_crop`, where
//! it is the same keyframed box as the Pan/Crop of an event; here the timeline is read and one
//! [`EditCommand`] is returned, so a released drag is exactly one undo step.

use neoveg_common::pan_crop::TrackMotion;

use crate::command::EditCommand;
use crate::timeline::{Timeline, TrackId, TrackKind};

/// The motion of a video track, or `None` when the track does not exist or is not a video track.
pub fn get(timeline: &Timeline, track_id: TrackId) -> Option<&TrackMotion> {
    let track = timeline.track(track_id)?;
    (track.kind == TrackKind::Video).then_some(&track.motion)
}

/// A command that replaces the motion of the track with `after`, or `None` when nothing changes (or
/// the track is not a video track).
pub fn set(timeline: &Timeline, track_id: TrackId, after: TrackMotion) -> Option<EditCommand> {
    let before = get(timeline, track_id)?.clone();
    (before != after).then_some(EditCommand::SetTrackMotion {
        track_id,
        before,
        after,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use neoveg_common::pan_crop::PanCropFrame;

    fn track(kind: TrackKind) -> (Timeline, History, TrackId) {
        let mut tl = Timeline::new();
        let mut h = History::new();
        let track = tl.new_track(kind);
        let id = track.id;
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track }));
        (tl, h, id)
    }

    fn corner() -> TrackMotion {
        let mut m = TrackMotion::default();
        m.upsert_key(
            0.0,
            PanCropFrame {
                x_center: 0.8,
                y_center: 0.8,
                width: 0.3,
                height: 0.3,
                rotation_deg: 0.0,
            },
            1e-3,
        );
        m
    }

    #[test]
    fn a_motion_change_is_one_undo_step_and_no_change_is_refused() {
        let (mut tl, mut h, id) = track(TrackKind::Video);
        assert!(get(&tl, id).unwrap().is_default());
        assert!(set(&tl, id, TrackMotion::default()).is_none());
        let cmd = set(&tl, id, corner()).unwrap();
        assert!(h.execute(&mut tl, cmd));
        assert_eq!(get(&tl, id).unwrap().frame_at(5.0).width, 0.3);
        assert!(h.undo(&mut tl));
        assert!(get(&tl, id).unwrap().is_default());
        assert!(h.redo(&mut tl));
        assert_eq!(get(&tl, id).unwrap().keys.len(), 1);
    }

    #[test]
    fn only_video_tracks_have_motion() {
        let (tl, _, aid) = track(TrackKind::Audio);
        assert!(get(&tl, aid).is_none());
        assert!(set(&tl, aid, corner()).is_none());
        assert!(set(&tl, 999, corner()).is_none());
    }

    #[test]
    fn projects_saved_before_track_motion_load_and_a_motion_round_trips() {
        let old: crate::timeline::Track =
            serde_json::from_str(r#"{"id":1,"kind":"Video","name":"V","muted":false,"events":[]}"#)
                .unwrap();
        assert!(old.motion.is_default());
        assert!(!serde_json::to_string(&old).unwrap().contains("motion"));
        let mut with = old.clone();
        with.motion = corner();
        let back: crate::timeline::Track =
            serde_json::from_str(&serde_json::to_string(&with).unwrap()).unwrap();
        assert_eq!(back.motion, with.motion);
    }
}
