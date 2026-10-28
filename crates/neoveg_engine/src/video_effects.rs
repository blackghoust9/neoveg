//! Video effect chains of events and of video tracks.
//!
//! Every function is pure: it reads the timeline and returns one [`EditCommand`], so a menu action
//! or a released slider is exactly one undo step. Like the audio chains (`effects`), the engine does
//! not know what an effect does; it stores the registry id and the parameter values the user chose
//! (see [`TrackEffect`]). `neoveg_video::vfx` clamps the values and runs the chain.

use crate::command::{EditCommand, VideoFxTarget};
use crate::timeline::{Timeline, TrackEffect, TrackKind};

/// Longest video effect chain of an event or a track.
pub const MAX_VIDEO_EFFECTS: usize = 32;

/// The chain at `target`, or `None` when the track or the event does not exist (or the track is
/// not a video track).
pub fn chain(timeline: &Timeline, target: VideoFxTarget) -> Option<&[TrackEffect]> {
    match target {
        VideoFxTarget::Track(id) => {
            let track = timeline.track(id)?;
            (track.kind == TrackKind::Video).then_some(track.video_effects.as_slice())
        }
        VideoFxTarget::Event { track_id, event_id } => {
            let track = timeline.track(track_id)?;
            if track.kind != TrackKind::Video {
                return None;
            }
            track
                .events
                .iter()
                .find(|e| e.id == event_id)
                .map(|e| e.video_effects.as_slice())
        }
    }
}

/// A command that replaces the chain with `after`, or `None` when nothing changes.
fn replace(
    timeline: &Timeline,
    target: VideoFxTarget,
    after: Vec<TrackEffect>,
) -> Option<EditCommand> {
    let before = chain(timeline, target)?.to_vec();
    (before != after).then_some(EditCommand::SetVideoEffects {
        target,
        before,
        after,
    })
}

fn edit(
    timeline: &Timeline,
    target: VideoFxTarget,
    change: impl FnOnce(&mut Vec<TrackEffect>) -> bool,
) -> Option<EditCommand> {
    let mut after = chain(timeline, target)?.to_vec();
    change(&mut after).then(|| replace(timeline, target, after))?
}

/// Appends an effect with default parameters. `None` for a full chain.
pub fn add_effect(timeline: &Timeline, target: VideoFxTarget, kind: &str) -> Option<EditCommand> {
    edit(timeline, target, |list| {
        if list.len() >= MAX_VIDEO_EFFECTS {
            return false;
        }
        list.push(TrackEffect::new(kind));
        true
    })
}

/// Removes the effect at `index`.
pub fn remove_effect(
    timeline: &Timeline,
    target: VideoFxTarget,
    index: usize,
) -> Option<EditCommand> {
    edit(timeline, target, |list| {
        if index >= list.len() {
            return false;
        }
        list.remove(index);
        true
    })
}

/// Moves the effect at `from` so it ends up at `to` (the chain order is the processing order).
pub fn move_effect(
    timeline: &Timeline,
    target: VideoFxTarget,
    from: usize,
    to: usize,
) -> Option<EditCommand> {
    edit(timeline, target, |list| {
        if from >= list.len() || to >= list.len() || from == to {
            return false;
        }
        let effect = list.remove(from);
        list.insert(to, effect);
        true
    })
}

pub fn set_bypass(
    timeline: &Timeline,
    target: VideoFxTarget,
    index: usize,
    bypass: bool,
) -> Option<EditCommand> {
    edit(timeline, target, |list| match list.get_mut(index) {
        Some(effect) => {
            effect.bypass = bypass;
            true
        }
        None => false,
    })
}

/// Sets one parameter. Values are stored as given; the video layer clamps them to the range of the
/// parameter when it builds the effect.
pub fn set_param(
    timeline: &Timeline,
    target: VideoFxTarget,
    index: usize,
    param: &str,
    value: f32,
) -> Option<EditCommand> {
    if !value.is_finite() {
        return None;
    }
    edit(timeline, target, |list| match list.get_mut(index) {
        Some(effect) => {
            effect.params.insert(param.to_owned(), value);
            true
        }
        None => false,
    })
}

/// Puts one parameter back to its default (drops the stored value).
pub fn reset_param(
    timeline: &Timeline,
    target: VideoFxTarget,
    index: usize,
    param: &str,
) -> Option<EditCommand> {
    edit(timeline, target, |list| match list.get_mut(index) {
        Some(effect) => effect.params.remove(param).is_some(),
        None => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::timeline::{TrackEvent, TrackId};

    fn setup() -> (Timeline, History, TrackId, u64) {
        let mut tl = Timeline::new();
        let mut h = History::new();
        let track = tl.new_track(TrackKind::Video);
        let tid = track.id;
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track }));
        let eid = tl.alloc_id();
        let event = TrackEvent {
            id: eid,
            name: "clip".into(),
            media_id: Some(1),
            duration: 5.0,
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

    /// Builds a command from the timeline and executes it (the builder borrows `tl` first).
    macro_rules! exec {
        ($tl:ident, $h:ident, $build:expr) => {{
            let cmd = $build;
            assert!($h.execute(&mut $tl, cmd.expect("command")));
        }};
    }

    fn kinds(tl: &Timeline, target: VideoFxTarget) -> Vec<String> {
        chain(tl, target)
            .unwrap()
            .iter()
            .map(|e| e.kind.clone())
            .collect()
    }

    #[test]
    fn event_chains_are_edited_in_order_and_each_change_is_one_undo_step() {
        let (mut tl, mut h, track_id, event_id) = setup();
        let target = VideoFxTarget::Event { track_id, event_id };
        for kind in ["gaussian_blur", "sharpen", "vignette"] {
            exec!(tl, h, add_effect(&tl, target, kind));
        }
        assert_eq!(kinds(&tl, target), ["gaussian_blur", "sharpen", "vignette"]);
        exec!(tl, h, move_effect(&tl, target, 2, 0));
        assert_eq!(kinds(&tl, target), ["vignette", "gaussian_blur", "sharpen"]);
        exec!(tl, h, remove_effect(&tl, target, 1));
        assert_eq!(kinds(&tl, target), ["vignette", "sharpen"]);

        assert!(h.undo(&mut tl) && h.undo(&mut tl));
        assert_eq!(kinds(&tl, target), ["gaussian_blur", "sharpen", "vignette"]);
        assert!(h.redo(&mut tl));
        assert_eq!(kinds(&tl, target), ["vignette", "gaussian_blur", "sharpen"]);
    }

    #[test]
    fn track_chains_are_separate_from_event_chains() {
        let (mut tl, mut h, track_id, event_id) = setup();
        let on_track = VideoFxTarget::Track(track_id);
        let on_event = VideoFxTarget::Event { track_id, event_id };
        exec!(tl, h, add_effect(&tl, on_track, "vignette"));
        assert_eq!(kinds(&tl, on_track), ["vignette"]);
        assert!(kinds(&tl, on_event).is_empty());
        assert!(h.undo(&mut tl));
        assert!(kinds(&tl, on_track).is_empty());
    }

    #[test]
    fn parameters_and_bypass_are_stored_and_reset() {
        let (mut tl, mut h, track_id, event_id) = setup();
        let target = VideoFxTarget::Event { track_id, event_id };
        exec!(tl, h, add_effect(&tl, target, "gaussian_blur"));
        exec!(tl, h, set_param(&tl, target, 0, "h_range", 0.04));
        exec!(tl, h, set_bypass(&tl, target, 0, true));
        let fx = &chain(&tl, target).unwrap()[0];
        assert_eq!(fx.params["h_range"], 0.04);
        assert!(fx.bypass);

        exec!(tl, h, reset_param(&tl, target, 0, "h_range"));
        assert!(chain(&tl, target).unwrap()[0].params.is_empty());
        assert!(
            reset_param(&tl, target, 0, "h_range").is_none(),
            "no change"
        );
        assert!(h.undo(&mut tl));
        assert_eq!(chain(&tl, target).unwrap()[0].params["h_range"], 0.04);
    }

    #[test]
    fn bad_requests_are_refused_and_the_chain_is_capped() {
        let (mut tl, mut h, track_id, event_id) = setup();
        let target = VideoFxTarget::Event { track_id, event_id };
        assert!(remove_effect(&tl, target, 0).is_none());
        assert!(move_effect(&tl, target, 0, 1).is_none());
        assert!(set_param(&tl, target, 0, "x", 1.0).is_none());
        let missing = VideoFxTarget::Event {
            track_id,
            event_id: 999,
        };
        assert!(add_effect(&tl, missing, "vignette").is_none());
        assert!(add_effect(&tl, VideoFxTarget::Track(999), "vignette").is_none());

        exec!(tl, h, add_effect(&tl, target, "vignette"));
        assert!(set_param(&tl, target, 0, "outer_radius", f32::NAN).is_none());
        assert!(set_bypass(&tl, target, 0, false).is_none(), "no change");
        for _ in 1..MAX_VIDEO_EFFECTS {
            exec!(tl, h, add_effect(&tl, target, "vignette"));
        }
        assert!(add_effect(&tl, target, "vignette").is_none(), "chain full");
    }

    #[test]
    fn audio_tracks_have_no_video_chain() {
        let mut tl = Timeline::new();
        let mut h = History::new();
        let track = tl.new_track(TrackKind::Audio);
        let id = track.id;
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track }));
        assert!(add_effect(&tl, VideoFxTarget::Track(id), "vignette").is_none());
    }

    #[test]
    fn projects_saved_before_video_effects_still_load_and_empty_chains_stay_out_of_the_file() {
        let track: crate::timeline::Track =
            serde_json::from_str(r#"{"id":1,"kind":"Video","name":"V","muted":false,"events":[]}"#)
                .unwrap();
        assert!(track.video_effects.is_empty());
        assert!(!serde_json::to_string(&track)
            .unwrap()
            .contains("video_effects"));

        let (mut tl, mut h, track_id, event_id) = setup();
        let target = VideoFxTarget::Event { track_id, event_id };
        exec!(tl, h, add_effect(&tl, target, "vignette"));
        let json = serde_json::to_string(&tl).unwrap();
        let back: Timeline = serde_json::from_str(&json).unwrap();
        assert_eq!(chain(&back, target).unwrap(), chain(&tl, target).unwrap());
    }

    #[test]
    fn a_split_keeps_the_chain_on_both_halves() {
        let (mut tl, mut h, track_id, event_id) = setup();
        let target = VideoFxTarget::Event { track_id, event_id };
        exec!(tl, h, add_effect(&tl, target, "sharpen"));
        let new_event_id = tl.alloc_id();
        assert!(h.execute(
            &mut tl,
            EditCommand::SplitEvent {
                track_id,
                event_id,
                new_event_id,
                at: 2.0,
                original_duration: 5.0,
                original_fade_out: 0.0,
            }
        ));
        let events = &tl.track(track_id).unwrap().events;
        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|e| e.video_effects.len() == 1));
        assert!(h.undo(&mut tl));
        let events = &tl.track(track_id).unwrap().events;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].video_effects.len(), 1);
    }
}
