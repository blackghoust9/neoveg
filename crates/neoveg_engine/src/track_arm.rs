//! *Armar faixa para gravação* (manual v18, *Arm for Record*): the audio tracks that wait for a
//! recording. When the user records, the take goes on the first armed track that is not locked
//! (the track list order is the one of the timeline); with none armed it goes where the selection
//! says, as before.

use crate::command::EditCommand;
use crate::timeline::{Timeline, TrackId, TrackKind};

/// The edit that arms (`true`) or disarms `track_id`; `None` when it has that state already, does
/// not exist, or is not an audio track (only audio is recorded).
pub fn set(timeline: &Timeline, track_id: TrackId, armed: bool) -> Option<EditCommand> {
    let track = timeline.track(track_id)?;
    (track.kind == TrackKind::Audio && track.armed != armed)
        .then_some(EditCommand::SetTrackArmed { track_id, armed })
}

/// The armed audio tracks, in timeline order.
pub fn armed_tracks(timeline: &Timeline) -> Vec<TrackId> {
    timeline
        .tracks
        .iter()
        .filter(|t| t.kind == TrackKind::Audio && t.armed)
        .map(|t| t.id)
        .collect()
}

/// The track a recording goes on: the first armed track that is not locked.
pub fn target_track(timeline: &Timeline) -> Option<TrackId> {
    timeline
        .tracks
        .iter()
        .find(|t| t.kind == TrackKind::Audio && t.armed && !t.locked)
        .map(|t| t.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;

    fn timeline() -> (Timeline, TrackId, TrackId, TrackId) {
        let mut tl = Timeline::new();
        let a = tl.new_track(TrackKind::Audio);
        let b = tl.new_track(TrackKind::Audio);
        let v = tl.new_track(TrackKind::Video);
        let ids = (a.id, b.id, v.id);
        tl.tracks.extend([a, b, v]);
        (tl, ids.0, ids.1, ids.2)
    }

    #[test]
    fn only_audio_tracks_arm_and_each_change_is_one_undo_step() {
        let (mut tl, a, _, v) = timeline();
        let mut h = History::new();
        assert!(set(&tl, v, true).is_none(), "video is not recorded");
        assert!(set(&tl, a, false).is_none(), "already disarmed");
        assert!(set(&tl, 999, true).is_none());
        let arm = set(&tl, a, true).unwrap();
        assert!(h.execute(&mut tl, arm));
        assert_eq!(armed_tracks(&tl), [a]);
        assert!(set(&tl, a, true).is_none(), "already armed");
        assert!(h.undo(&mut tl));
        assert!(armed_tracks(&tl).is_empty());
        assert!(h.redo(&mut tl));
        assert_eq!(armed_tracks(&tl), [a]);
    }

    #[test]
    fn the_take_goes_on_the_first_armed_track_that_is_not_locked() {
        let (mut tl, a, b, _) = timeline();
        assert_eq!(target_track(&tl), None);
        tl.track_mut(b).unwrap().armed = true;
        assert_eq!(target_track(&tl), Some(b));
        tl.track_mut(a).unwrap().armed = true;
        assert_eq!(target_track(&tl), Some(a), "timeline order");
        tl.track_mut(a).unwrap().locked = true;
        assert_eq!(target_track(&tl), Some(b), "a locked track takes no take");
        assert_eq!(armed_tracks(&tl), [a, b]);
    }

    #[test]
    fn the_armed_state_is_kept_in_the_project_and_left_out_when_off() {
        let (mut tl, a, b, _) = timeline();
        tl.track_mut(a).unwrap().armed = true;
        let json = serde_json::to_string(&tl).unwrap();
        assert_eq!(json.matches("\"armed\"").count(), 1);
        let back: Timeline = serde_json::from_str(&json).unwrap();
        assert!(back.track(a).unwrap().armed && !back.track(b).unwrap().armed);
    }
}
