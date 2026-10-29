//! Adjustment events: an empty video event whose effect chain runs on the picture of the video
//! tracks below it for as long as it lasts. It behaves like a video event otherwise (fades,
//! opacity, trim, split, copy and paste, Mute and Lock), except that it has no media, so a velocity
//! envelope, a Pan/Crop or a reverse mean nothing for it.
//!
//! Everything here is a pure function; `neoveg_ui` turns the event into a composite node.

use crate::command::EditCommand;
use crate::timeline::{EventId, Timeline, TrackEvent, TrackId, TrackKind};

/// Length of a new adjustment event, in seconds (the manual gives none).
pub const DEFAULT_LENGTH: f64 = 5.0;

/// Name a new adjustment event starts with.
pub const DEFAULT_NAME: &str = "Evento de ajuste";

/// The command that adds an adjustment event with id `id` at `start` on video track `track_id`, or
/// `None` when the track does not exist or is not a video track.
pub fn insert_command(
    timeline: &Timeline,
    track_id: TrackId,
    start: f64,
    id: EventId,
) -> Option<EditCommand> {
    let track = timeline.track(track_id)?;
    if track.kind != TrackKind::Video {
        return None;
    }
    let start = if start.is_finite() {
        start.max(0.0)
    } else {
        0.0
    };
    Some(EditCommand::AddEvent {
        track_id,
        event: TrackEvent {
            id,
            name: DEFAULT_NAME.into(),
            media_id: None,
            start,
            duration: DEFAULT_LENGTH,
            adjustment: true,
            ..TrackEvent::default()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::timeline::TrackEffect;

    fn setup() -> (Timeline, History, TrackId, TrackId) {
        let mut tl = Timeline::new();
        let mut h = History::new();
        let video = tl.new_track(TrackKind::Video);
        let audio = tl.new_track(TrackKind::Audio);
        let (v, a) = (video.id, audio.id);
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track: video }));
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track: audio }));
        (tl, h, v, a)
    }

    #[test]
    fn an_adjustment_event_goes_on_a_video_track_only_and_undoes_in_one_step() {
        let (mut tl, mut h, v, a) = setup();
        let id = tl.alloc_id();
        assert!(
            insert_command(&tl, a, 1.0, id).is_none(),
            "not on an audio track"
        );
        assert!(insert_command(&tl, 999, 1.0, id).is_none());
        let command = insert_command(&tl, v, 2.5, id).unwrap();
        assert!(h.execute(&mut tl, command));
        let event = tl.track(v).unwrap().events[0].clone();
        assert!(event.adjustment && event.media_id.is_none());
        assert_eq!((event.start, event.duration), (2.5, DEFAULT_LENGTH));
        assert!(h.undo(&mut tl));
        assert!(tl.track(v).unwrap().events.is_empty());
        // A bad start is brought back to zero
        let again = insert_command(&tl, v, f64::NAN, id).unwrap();
        assert!(matches!(&again, EditCommand::AddEvent { event, .. } if event.start == 0.0));
    }

    #[test]
    fn velocity_and_pan_crop_do_not_apply_to_an_adjustment_event() {
        let (mut tl, mut h, v, _) = setup();
        let id = tl.alloc_id();
        let command = insert_command(&tl, v, 0.0, id).unwrap();
        assert!(h.execute(&mut tl, command));
        assert!(crate::velocity::get(&tl, v, id).is_none());
        assert!(
            crate::velocity::set(&tl, v, id, Some(crate::velocity::new_envelope(2.0))).is_none()
        );
        assert!(crate::pan_crop::get(&tl, v, id).is_none());
    }

    #[test]
    fn the_effect_chain_and_the_flag_survive_saving_and_old_events_are_not_adjustments() {
        let mut event = TrackEvent {
            id: 7,
            adjustment: true,
            duration: 3.0,
            video_effects: vec![TrackEffect::new("vignette")],
            ..TrackEvent::default()
        };
        let back: TrackEvent =
            serde_json::from_str(&serde_json::to_string(&event).unwrap()).unwrap();
        assert_eq!(back, event);
        event.adjustment = false;
        let json = serde_json::to_string(&event).unwrap();
        assert!(
            !json.contains("adjustment"),
            "a plain event does not write the flag"
        );
        let old: TrackEvent = serde_json::from_str(
            r#"{"id":1,"name":"x","media_id":null,"start":0.0,"duration":1.0,"trim_in":0.0,"fade_in":0.0,"fade_out":0.0}"#,
        )
        .unwrap();
        assert!(!old.adjustment);
    }
}
