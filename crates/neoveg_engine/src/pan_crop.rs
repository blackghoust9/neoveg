//! Pan/Crop of video events: the pure builders of its undoable commands.
//!
//! The model (`PanCrop`, its keys and interpolation) lives in `neoveg_common::pan_crop`; here the
//! timeline is read and one [`EditCommand`] is returned, so a released drag or a menu action is
//! exactly one undo step.

use neoveg_common::pan_crop::PanCrop;

use crate::command::EditCommand;
use crate::timeline::{EventId, Timeline, TrackId, TrackKind};

/// The Pan/Crop of an event of a video track, or `None` when the event does not exist or its track
/// is not a video track.
pub fn get(timeline: &Timeline, track_id: TrackId, event_id: EventId) -> Option<&PanCrop> {
    let track = timeline.track(track_id)?;
    if track.kind != TrackKind::Video {
        return None;
    }
    track
        .events
        .iter()
        .find(|e| e.id == event_id)
        .map(|e| &e.pan_crop)
}

/// A command that replaces the Pan/Crop of the event with `after`, or `None` when nothing changes
/// (or the event is not a video event).
pub fn set(
    timeline: &Timeline,
    track_id: TrackId,
    event_id: EventId,
    after: PanCrop,
) -> Option<EditCommand> {
    let before = get(timeline, track_id, event_id)?.clone();
    (before != after).then_some(EditCommand::SetPanCrop {
        track_id,
        event_id,
        before,
        after,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::timeline::TrackEvent;
    use neoveg_common::pan_crop::PanCropFrame;

    fn setup() -> (Timeline, History, TrackId, EventId) {
        let mut tl = Timeline::new();
        let mut h = History::new();
        let track = tl.new_track(TrackKind::Video);
        let tid = track.id;
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track }));
        let eid = tl.alloc_id();
        let event = TrackEvent {
            id: eid,
            media_id: Some(1),
            duration: 8.0,
            ..TrackEvent::default()
        };
        assert!(h.execute(
            &mut tl,
            EditCommand::AddEvent {
                track_id: tid,
                event
            }
        ));
        (tl, h, tid, eid)
    }

    fn zoomed() -> PanCrop {
        let mut p = PanCrop::default();
        p.upsert_key(
            0.0,
            PanCropFrame {
                width: 0.5,
                height: 0.5,
                ..PanCropFrame::FULL
            },
            1e-3,
        );
        p
    }

    #[test]
    fn a_pan_crop_change_is_one_undo_step_and_no_change_is_refused() {
        let (mut tl, mut h, tid, eid) = setup();
        assert!(get(&tl, tid, eid).unwrap().is_default());
        assert!(
            set(&tl, tid, eid, PanCrop::default()).is_none(),
            "nothing changes"
        );
        let cmd = set(&tl, tid, eid, zoomed()).unwrap();
        assert!(h.execute(&mut tl, cmd));
        assert_eq!(get(&tl, tid, eid).unwrap().keys.len(), 1);
        assert!(h.undo(&mut tl));
        assert!(get(&tl, tid, eid).unwrap().is_default());
        assert!(h.redo(&mut tl));
        assert_eq!(get(&tl, tid, eid).unwrap().frame_at(1.0).width, 0.5);
    }

    #[test]
    fn only_events_of_video_tracks_have_a_pan_crop() {
        let (mut tl, mut h, _tid, _eid) = setup();
        let audio = tl.new_track(TrackKind::Audio);
        let aid = audio.id;
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track: audio }));
        let eid = tl.alloc_id();
        let event = TrackEvent {
            id: eid,
            duration: 2.0,
            ..TrackEvent::default()
        };
        assert!(h.execute(
            &mut tl,
            EditCommand::AddEvent {
                track_id: aid,
                event
            }
        ));
        assert!(get(&tl, aid, eid).is_none());
        assert!(set(&tl, aid, eid, zoomed()).is_none());
        assert!(set(&tl, 999, eid, zoomed()).is_none());
    }

    #[test]
    fn a_split_gives_the_second_half_the_box_at_the_cut_and_undo_restores_the_first() {
        let (mut tl, mut h, tid, eid) = setup();
        let mut p = PanCrop::default();
        p.upsert_key(
            0.0,
            PanCropFrame {
                x_center: 0.0,
                ..PanCropFrame::FULL
            },
            1e-3,
        );
        p.upsert_key(
            8.0,
            PanCropFrame {
                x_center: 1.0,
                ..PanCropFrame::FULL
            },
            1e-3,
        );
        let cmd = set(&tl, tid, eid, p.clone()).unwrap();
        assert!(h.execute(&mut tl, cmd));
        let new_event_id = tl.alloc_id();
        assert!(h.execute(
            &mut tl,
            EditCommand::SplitEvent {
                track_id: tid,
                event_id: eid,
                new_event_id,
                at: 4.0,
                original_duration: 8.0,
                original_fade_out: 0.0,
            }
        ));
        let right = get(&tl, tid, new_event_id).unwrap();
        assert!(
            (right.keys[0].frame.x_center - 0.5).abs() < 1e-5,
            "starts at the cut"
        );
        assert!((right.keys.last().unwrap().time - 4.0).abs() < 1e-9);
        assert_eq!(
            get(&tl, tid, eid).unwrap(),
            &p,
            "the left half keeps its keys"
        );
        assert!(h.undo(&mut tl));
        assert_eq!(get(&tl, tid, eid).unwrap(), &p);
    }

    #[test]
    fn projects_saved_before_pan_crop_load_and_a_pan_crop_round_trips() {
        let event: TrackEvent = serde_json::from_str(
            r#"{"id":1,"name":"x","media_id":null,"start":0.0,"duration":1.0,"trim_in":0.0,"fade_in":0.0,"fade_out":0.0}"#,
        )
        .unwrap();
        assert!(event.pan_crop.is_default());
        assert!(!serde_json::to_string(&event).unwrap().contains("pan_crop"));
        let mut with = event.clone();
        with.pan_crop = zoomed();
        let back: TrackEvent =
            serde_json::from_str(&serde_json::to_string(&with).unwrap()).unwrap();
        assert_eq!(back.pan_crop, with.pan_crop);
    }
}
