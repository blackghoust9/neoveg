//! *Renderizar para nova faixa* and *Renderizar e substituir*: the selected events are rendered to
//! a file of their own, and the file is laid on a new track or in place of the events (the idea of
//! rendering and replacing comes from the Premiere Pro reference; the name and the behavior of
//! *Render to New Track* are those of the VEGAS).
//!
//! What is rendered is the picture and the sound of the selected events alone, with the effects of
//! their tracks, over black and silence: everything not selected is taken out of the copy of the
//! project that the render reads. The functions here are pure: they describe the selection, make
//! that copy and build the one edit that puts the result in the project.

use crate::command::EditCommand;
use crate::ops;
use crate::selection::EventRef;
use crate::timeline::{Timeline, TrackEvent, TrackId, TrackKind};

/// What a selection holds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Selected {
    /// From the start of the first event to the end of the last one, seconds.
    pub span: (f64, f64),
    pub has_video: bool,
    pub has_audio: bool,
    /// The track of the first video event and of the first audio event (in the order of the
    /// selection): where a replacement goes back to.
    pub video_track: Option<TrackId>,
    pub audio_track: Option<TrackId>,
}

/// Describes the events in `refs` that exist. `None` when there are none, or they last no time.
pub fn describe(timeline: &Timeline, refs: &[EventRef]) -> Option<Selected> {
    let mut found = Selected {
        span: (f64::INFINITY, f64::NEG_INFINITY),
        has_video: false,
        has_audio: false,
        video_track: None,
        audio_track: None,
    };
    let mut any = false;
    for &(track_id, event_id) in refs {
        let Some(track) = timeline.track(track_id) else {
            continue;
        };
        let Some(event) = track.events.iter().find(|e| e.id == event_id) else {
            continue;
        };
        any = true;
        found.span.0 = found.span.0.min(event.start);
        found.span.1 = found.span.1.max(event.start + event.duration);
        match track.kind {
            TrackKind::Video => {
                found.has_video = true;
                found.video_track.get_or_insert(track_id);
            }
            TrackKind::Audio => {
                found.has_audio = true;
                found.audio_track.get_or_insert(track_id);
            }
        }
    }
    (any && found.span.1 - found.span.0 > 1e-3).then_some(found)
}

/// A copy of the project with only the events in `refs`: what the render of the selection reads.
pub fn only_events(timeline: &Timeline, refs: &[EventRef]) -> Timeline {
    let mut copy = timeline.clone();
    for track in &mut copy.tracks {
        let id = track.id;
        track
            .events
            .retain(|e| refs.iter().any(|&(t, ev)| t == id && ev == e.id));
    }
    copy.captions.clear();
    copy
}

/// The edit that puts the rendered `added` events (each with its track) in the project; with
/// `replace` the events in `refs` go out in the same step. `None` when it changes nothing.
pub fn placement(
    timeline: &Timeline,
    refs: &[EventRef],
    added: Vec<(TrackId, TrackEvent)>,
    replace: bool,
) -> Option<EditCommand> {
    let mut commands = Vec::new();
    if replace {
        if let Some(outcome) = ops::delete_events(timeline, refs) {
            commands.push(outcome.command);
        }
    }
    commands.extend(
        added
            .into_iter()
            .map(|(track_id, event)| EditCommand::AddEvent { track_id, event }),
    );
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

    fn event(id: u64, start: f64, duration: f64) -> TrackEvent {
        TrackEvent {
            id,
            start,
            duration,
            media_id: Some(1),
            ..Default::default()
        }
    }

    fn project() -> (Timeline, TrackId, TrackId) {
        let mut tl = Timeline::new();
        let mut video = tl.new_track(TrackKind::Video);
        video.events = vec![event(10, 2.0, 3.0), event(11, 8.0, 1.0)];
        let mut audio = tl.new_track(TrackKind::Audio);
        audio.events = vec![event(20, 1.0, 6.0)];
        let (v, a) = (video.id, audio.id);
        tl.tracks = vec![video, audio];
        (tl, v, a)
    }

    #[test]
    fn a_selection_is_described_by_its_span_its_kinds_and_its_first_tracks() {
        let (tl, v, a) = project();
        let found = describe(&tl, &[(v, 10), (a, 20), (v, 11)]).unwrap();
        assert_eq!(found.span, (1.0, 9.0));
        assert!(found.has_video && found.has_audio);
        assert_eq!((found.video_track, found.audio_track), (Some(v), Some(a)));
        let audio_only = describe(&tl, &[(a, 20)]).unwrap();
        assert!(!audio_only.has_video && audio_only.video_track.is_none());
        assert!(describe(&tl, &[]).is_none());
        assert!(describe(&tl, &[(v, 999)]).is_none(), "no such event");
    }

    #[test]
    fn the_copy_for_the_render_holds_only_the_selected_events() {
        let (tl, v, a) = project();
        let copy = only_events(&tl, &[(v, 11), (a, 20)]);
        assert_eq!(copy.track(v).unwrap().events.len(), 1);
        assert_eq!(copy.track(v).unwrap().events[0].id, 11);
        assert_eq!(copy.track(a).unwrap().events.len(), 1);
        assert_eq!(
            tl.track(v).unwrap().events.len(),
            2,
            "the project is untouched"
        );
    }

    #[test]
    fn placing_the_result_adds_it_and_replacing_also_takes_the_originals_out_in_one_step() {
        let (mut tl, v, _) = project();
        let original = tl.clone();
        let mut history = History::new();
        let rendered = event(30, 2.0, 3.0);
        // New track: the originals stay
        let add = placement(&tl, &[(v, 10)], vec![(v, rendered.clone())], false).unwrap();
        assert!(history.execute(&mut tl, add));
        assert_eq!(tl.track(v).unwrap().events.len(), 3);
        assert!(history.undo(&mut tl));
        assert_eq!(tl, original);
        // Replace: the original goes, the render takes its place; one undo brings it back
        let swap = placement(&tl, &[(v, 10)], vec![(v, rendered)], true).unwrap();
        assert!(history.execute(&mut tl, swap));
        let ids: Vec<u64> = tl.track(v).unwrap().events.iter().map(|e| e.id).collect();
        assert!(ids.contains(&30) && !ids.contains(&10) && ids.contains(&11));
        assert!(history.undo(&mut tl));
        assert_eq!(tl, original);
        assert!(placement(&tl, &[], Vec::new(), true).is_none());
    }
}
