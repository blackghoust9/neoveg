//! The timeline of the project a sub-project was opened from, as a read-only picture: the tracks
//! and their events in order, with the events that play the sub-project highlighted. Opening the
//! sub-project does not change the parent; this only says where it is used.

use crate::timeline::{Timeline, TrackKind};

/// One track of the parent, in the order of the timeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParentTrack {
    pub name: String,
    pub video: bool,
}

/// One event of the parent, in seconds of the parent's timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct ParentEvent {
    /// Index into [`ParentView::tracks`].
    pub track: usize,
    pub start: f64,
    pub end: f64,
    pub name: String,
    /// `true` when the event plays the sub-project.
    pub is_child: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ParentView {
    pub tracks: Vec<ParentTrack>,
    pub events: Vec<ParentEvent>,
    /// The end of the last event (`0.0` for an empty timeline).
    pub length: f64,
}

/// The read-only picture of `timeline`. `is_child` says whether a media id is the sub-project.
pub fn parent_view(timeline: &Timeline, is_child: impl Fn(Option<u64>) -> bool) -> ParentView {
    let mut view = ParentView::default();
    for (index, track) in timeline.tracks.iter().enumerate() {
        view.tracks.push(ParentTrack {
            name: track.name.clone(),
            video: track.kind == TrackKind::Video,
        });
        for event in &track.events {
            view.length = view.length.max(event.end());
            view.events.push(ParentEvent {
                track: index,
                start: event.start,
                end: event.end(),
                name: event.name.clone(),
                is_child: is_child(event.media_id),
            });
        }
    }
    view
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::{TrackEvent, TrackKind};

    fn event(id: u64, start: f64, duration: f64, media: Option<u64>) -> TrackEvent {
        TrackEvent {
            id,
            name: format!("e{id}"),
            media_id: media,
            start,
            duration,
            ..Default::default()
        }
    }

    #[test]
    fn events_keep_their_track_and_span_and_the_child_is_marked() {
        let mut tl = Timeline::new();
        let mut video = tl.new_track(TrackKind::Video);
        video.name = "Vídeo 1".into();
        video.events = vec![event(1, 0.0, 4.0, Some(10)), event(2, 4.0, 3.0, Some(20))];
        let mut audio = tl.new_track(TrackKind::Audio);
        audio.events = vec![event(3, 1.0, 9.0, Some(10))];
        tl.tracks = vec![video, audio];

        let view = parent_view(&tl, |m| m == Some(20));
        assert_eq!(view.tracks.len(), 2);
        assert!(view.tracks[0].video && !view.tracks[1].video);
        assert_eq!(view.tracks[0].name, "Vídeo 1");
        assert_eq!(view.events.len(), 3);
        assert!(view.events[1].is_child && !view.events[0].is_child);
        assert_eq!(view.events[1].track, 0);
        assert_eq!((view.events[1].start, view.events[1].end), (4.0, 7.0));
        assert!(
            (view.length - 10.0).abs() < 1e-9,
            "the longest event ends at 10 s"
        );
    }

    #[test]
    fn an_empty_timeline_is_an_empty_view() {
        let view = parent_view(&Timeline::new(), |_| true);
        assert!(view.tracks.is_empty() && view.events.is_empty());
        assert_eq!(view.length, 0.0);
    }
}
