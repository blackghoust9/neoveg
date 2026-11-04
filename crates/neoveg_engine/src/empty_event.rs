//! *Inserir evento vazio* (manual v21, "Inserting an empty event"): a blank event with no media on
//! the selected track, as long as the time selection or a default time. The kind of event, audio or
//! video, follows the kind of the track. It holds the place for something that comes later and plays
//! nothing.

use crate::command::EditCommand;
use crate::timeline::{EventId, Timeline, TrackEvent, TrackId, TIME_EPSILON};

/// Shortest an empty event can be, in seconds.
pub const MIN_DURATION: f64 = 0.01;

/// The edit that puts an empty event of `duration` seconds at `start` on `track_id`, and the id of
/// the new event. `None` when the track does not exist, is not usable or the times are not numbers.
pub fn insert(
    timeline: &mut Timeline,
    track_id: TrackId,
    start: f64,
    duration: f64,
) -> Option<(EventId, EditCommand)> {
    if !start.is_finite() || !duration.is_finite() || timeline.track(track_id).is_none() {
        return None;
    }
    let duration = duration.max(MIN_DURATION);
    let id = timeline.alloc_id();
    let event = TrackEvent {
        id,
        name: "Evento vazio".into(),
        media_id: None,
        start: start.max(0.0),
        duration,
        ..TrackEvent::default()
    };
    debug_assert!(event.duration > TIME_EPSILON);
    Some((id, EditCommand::AddEvent { track_id, event }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::timeline::TrackKind;

    fn timeline_with(kind: TrackKind) -> (Timeline, TrackId) {
        let mut tl = Timeline::new();
        let track = tl.new_track(kind);
        let id = track.id;
        tl.tracks.push(track);
        (tl, id)
    }

    #[test]
    fn an_empty_event_is_put_on_the_track_with_no_media_for_audio_and_for_video() {
        for kind in [TrackKind::Audio, TrackKind::Video] {
            let (mut tl, track) = timeline_with(kind);
            let original = tl.clone();
            let (id, command) = insert(&mut tl, track, 2.0, 3.0).unwrap();
            let mut history = History::new();
            assert!(history.execute(&mut tl, command));
            let event = tl
                .track(track)
                .unwrap()
                .events
                .iter()
                .find(|e| e.id == id)
                .unwrap();
            assert!(event.media_id.is_none() && !event.adjustment);
            assert_eq!((event.start, event.duration), (2.0, 3.0));
            // One undo step takes it back
            assert!(history.undo(&mut tl));
            assert_eq!(
                tl.track(track).unwrap().events,
                original.track(track).unwrap().events
            );
        }
    }

    #[test]
    fn odd_times_and_missing_tracks_are_handled() {
        let (mut tl, track) = timeline_with(TrackKind::Video);
        assert!(
            insert(&mut tl, track + 99, 0.0, 1.0).is_none(),
            "no such track"
        );
        assert!(insert(&mut tl, track, f64::NAN, 1.0).is_none());
        assert!(insert(&mut tl, track, 0.0, f64::INFINITY).is_none());
        // A negative start is held at zero and a tiny length at the shortest one
        let (id, command) = insert(&mut tl, track, -3.0, 0.0).unwrap();
        let mut history = History::new();
        assert!(history.execute(&mut tl, command));
        let event = tl
            .track(track)
            .unwrap()
            .events
            .iter()
            .find(|e| e.id == id)
            .unwrap();
        assert_eq!(event.start, 0.0);
        assert_eq!(event.duration, MIN_DURATION);
    }
}
