//! Video effect chains of events and of video tracks.
//!
//! Every function is pure: it reads the timeline and returns one [`EditCommand`], so a menu action
//! or a released slider is exactly one undo step. Like the audio chains (`effects`), the engine does
//! not know what an effect does; it stores the registry id and the parameter values the user chose
//! (see [`TrackEffect`]). `neoveg_video::vfx` clamps the values and runs the chain.

use crate::command::{EditCommand, VideoFxTarget};
use crate::timeline::{Timeline, TrackEffect, TrackKind, TIME_EPSILON};
use neoveg_common::envelope::{eval_points, sort_points, EnvelopePoint, EnvelopeShape};

/// Longest video effect chain of an event or a track.
pub const MAX_VIDEO_EFFECTS: usize = 32;

/// The chain at `target`, or `None` when the track or the event does not exist (or the track is
/// not a video track). A media or the project always has a chain, possibly empty.
pub fn chain(timeline: &Timeline, target: VideoFxTarget) -> Option<&[TrackEffect]> {
    match target {
        VideoFxTarget::Project => Some(timeline.video_effects.as_slice()),
        VideoFxTarget::Media(id) => Some(
            timeline
                .media_video_effects
                .get(&id)
                .map_or(&[][..], Vec::as_slice),
        ),
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

/// Sets several parameters of the effect at `index` as one edit (one undo step): the two numbers of
/// a dragged curve point, or the ten of a whole curve.
pub fn set_params(
    timeline: &Timeline,
    target: VideoFxTarget,
    index: usize,
    values: &[(String, f32)],
) -> Option<EditCommand> {
    if values.iter().any(|(_, v)| !v.is_finite()) {
        return None;
    }
    edit(timeline, target, |list| match list.get_mut(index) {
        Some(effect) => {
            let before = effect.params.clone();
            for (param, value) in values {
                effect.params.insert(param.clone(), *value);
            }
            effect.params != before
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
        Some(effect) => {
            let removed_val = effect.params.remove(param).is_some();
            let removed_kf = effect.keyframes.remove(param).is_some();
            removed_val || removed_kf
        }
        None => false,
    })
}

/// Toggles keyframe animation on `param` of the `index`-th video effect: turning it on seeds a key
/// at `0.0 s` with the current static value (or `default_value`); turning it off collapses the
/// curve back to its value at `0.0 s`.
pub fn toggle_param_animation(
    timeline: &Timeline,
    target: VideoFxTarget,
    index: usize,
    param: &str,
    default_value: f32,
) -> Option<EditCommand> {
    edit(timeline, target, |list| {
        let Some(effect) = list.get_mut(index) else {
            return false;
        };
        if effect.param_is_animated(param) {
            let val = effect.param_at(param, default_value, 0.0);
            effect.keyframes.remove(param);
            effect.params.insert(param.to_owned(), val);
        } else {
            let val = effect.params.get(param).copied().unwrap_or(default_value);
            effect.keyframes.insert(
                param.to_owned(),
                vec![EnvelopePoint {
                    time: 0.0,
                    value: val,
                    shape: EnvelopeShape::Linear,
                }],
            );
        }
        true
    })
}

/// Inserts or updates a keyframe on `param` of the `index`-th video effect within `tolerance` s.
pub fn upsert_param_keyframe(
    timeline: &Timeline,
    target: VideoFxTarget,
    index: usize,
    param: &str,
    point: EnvelopePoint,
    bounds: (f32, f32),
    tolerance: f64,
) -> Option<EditCommand> {
    if !point.time.is_finite() || !point.value.is_finite() {
        return None;
    }
    let (lo, hi) = if bounds.0 <= bounds.1 {
        bounds
    } else {
        (bounds.1, bounds.0)
    };
    let t = point.time.max(0.0);
    let v = point.value.clamp(lo, hi);
    let tol = tolerance.max(TIME_EPSILON);
    edit(timeline, target, |list| {
        let Some(effect) = list.get_mut(index) else {
            return false;
        };
        let pts = effect.keyframes.entry(param.to_owned()).or_default();
        if let Some(existing) = pts.iter_mut().find(|p| (p.time - t).abs() <= tol) {
            existing.time = t;
            existing.value = v;
            existing.shape = point.shape;
        } else {
            pts.push(EnvelopePoint {
                time: t,
                value: v,
                shape: point.shape,
            });
        }
        sort_points(pts);
        effect.params.insert(param.to_owned(), v);
        true
    })
}

/// Moves keyframe `key_index` of `param`, clamping time between neighbouring keys and value to `bounds`.
pub fn move_param_keyframe(
    timeline: &Timeline,
    target: VideoFxTarget,
    index: usize,
    param: &str,
    key_index: usize,
    pos: (f64, f32),
    bounds: (f32, f32),
) -> Option<EditCommand> {
    if !pos.0.is_finite() || !pos.1.is_finite() {
        return None;
    }
    let (lo, hi) = if bounds.0 <= bounds.1 {
        bounds
    } else {
        (bounds.1, bounds.0)
    };
    edit(timeline, target, |list| {
        let Some(effect) = list.get_mut(index) else {
            return false;
        };
        let Some(pts) = effect.keyframes.get_mut(param) else {
            return false;
        };
        if key_index >= pts.len() {
            return false;
        }
        let min_t = if key_index > 0 {
            pts[key_index - 1].time + 1e-4
        } else {
            0.0
        };
        let max_t = if key_index + 1 < pts.len() {
            (pts[key_index + 1].time - 1e-4).max(min_t)
        } else {
            f64::INFINITY
        };
        pts[key_index].time = pos.0.clamp(min_t, max_t);
        pts[key_index].value = pos.1.clamp(lo, hi);
        true
    })
}

/// Removes keyframe `key_index` of `param` (when more than one keyframe exists).
pub fn remove_param_keyframe(
    timeline: &Timeline,
    target: VideoFxTarget,
    index: usize,
    param: &str,
    key_index: usize,
) -> Option<EditCommand> {
    edit(timeline, target, |list| {
        let Some(effect) = list.get_mut(index) else {
            return false;
        };
        let Some(pts) = effect.keyframes.get_mut(param) else {
            return false;
        };
        if pts.len() <= 1 || key_index >= pts.len() {
            return false;
        }
        pts.remove(key_index);
        true
    })
}

/// Changes the interpolation curve shape of keyframe `key_index` on `param`.
pub fn set_param_keyframe_shape(
    timeline: &Timeline,
    target: VideoFxTarget,
    index: usize,
    param: &str,
    key_index: usize,
    shape: EnvelopeShape,
) -> Option<EditCommand> {
    edit(timeline, target, |list| {
        let Some(effect) = list.get_mut(index) else {
            return false;
        };
        let Some(pt) = effect.keyframes.get_mut(param).and_then(|pts| pts.get_mut(key_index))
        else {
            return false;
        };
        pt.shape = shape;
        true
    })
}

/// Shifts animated parameter keyframes in `chain` when the front of an event is cut off by `offset`
/// seconds (during split or left-edge trim): the interpolated value at `offset` becomes the new
/// keyframe at `0.0 s` and later keyframes shift back by `offset`.
pub fn shift_chain_keyframes(chain: &[TrackEffect], offset: f64) -> Vec<TrackEffect> {
    if offset <= TIME_EPSILON {
        return chain.to_vec();
    }
    chain
        .iter()
        .map(|fx| {
            let mut out = fx.clone();
            for (param, pts) in &mut out.keyframes {
                if pts.is_empty() {
                    continue;
                }
                let fallback = out.params.get(param).copied().unwrap_or(0.0);
                let first_val = eval_points(pts, offset, fallback);
                let shape = pts
                    .iter()
                    .rfind(|p| p.time <= offset)
                    .map_or(EnvelopeShape::Linear, |p| p.shape);
                let mut shifted = vec![EnvelopePoint {
                    time: 0.0,
                    value: first_val,
                    shape,
                }];
                shifted.extend(
                    pts.iter()
                        .filter(|p| p.time > offset + TIME_EPSILON)
                        .map(|p| EnvelopePoint {
                            time: p.time - offset,
                            ..*p
                        }),
                );
                *pts = shifted;
                out.params.insert(param.clone(), first_val);
            }
            out
        })
        .collect()
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
    fn several_parameters_change_as_one_undo_step() {
        let (mut tl, mut h, track_id, event_id) = setup();
        let target = VideoFxTarget::Event { track_id, event_id };
        exec!(tl, h, add_effect(&tl, target, "color_curves"));
        let point = [("m_x2".to_string(), 0.3), ("m_y2".to_string(), 0.1)];
        exec!(tl, h, set_params(&tl, target, 0, &point));
        let fx = &chain(&tl, target).unwrap()[0];
        assert_eq!((fx.params["m_x2"], fx.params["m_y2"]), (0.3, 0.1));
        assert!(
            set_params(&tl, target, 0, &point).is_none(),
            "the same values change nothing"
        );
        assert!(set_params(&tl, target, 0, &[("m_x2".into(), f32::NAN)]).is_none());
        assert!(set_params(&tl, target, 5, &point).is_none());
        assert!(h.undo(&mut tl));
        assert!(chain(&tl, target).unwrap()[0].params.is_empty());
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

    #[test]
    fn media_and_project_chains_are_edited_undone_and_saved() {
        let (mut tl, mut h, _track_id, _event_id) = setup();
        let media = VideoFxTarget::Media(7);
        let project = VideoFxTarget::Project;
        assert!(chain(&tl, media).unwrap().is_empty());
        exec!(tl, h, add_effect(&tl, media, "gaussian_blur"));
        exec!(tl, h, set_param(&tl, media, 0, "h_range", 0.03));
        exec!(tl, h, add_effect(&tl, project, "vignette"));
        assert_eq!(kinds(&tl, media), ["gaussian_blur"]);
        assert_eq!(kinds(&tl, project), ["vignette"]);
        assert!(
            chain(&tl, VideoFxTarget::Media(8)).unwrap().is_empty(),
            "another media is untouched"
        );

        let json = serde_json::to_string(&tl).unwrap();
        let back: Timeline = serde_json::from_str(&json).unwrap();
        assert_eq!(chain(&back, media).unwrap(), chain(&tl, media).unwrap());
        assert_eq!(chain(&back, project).unwrap(), chain(&tl, project).unwrap());

        assert!(h.undo(&mut tl) && h.undo(&mut tl) && h.undo(&mut tl));
        assert!(chain(&tl, media).unwrap().is_empty());
        assert!(
            tl.media_video_effects.is_empty(),
            "an emptied chain leaves no entry in the file"
        );
        assert!(chain(&tl, project).unwrap().is_empty());
        assert!(h.redo(&mut tl));
        assert_eq!(kinds(&tl, media), ["gaussian_blur"]);
    }

    #[test]
    fn projects_saved_before_media_and_project_chains_still_load() {
        let tl: Timeline = serde_json::from_str(r#"{"tracks":[],"next_id":3}"#).unwrap();
        assert!(tl.video_effects.is_empty() && tl.media_video_effects.is_empty());
        let json = serde_json::to_string(&tl).unwrap();
        assert!(!json.contains("video_effects") && !json.contains("media_video_effects"));
    }

    #[test]
    fn parameter_keyframes_animate_split_at_offset_and_round_trip() {
        let (mut tl, mut h, track_id, event_id) = setup();
        let target = VideoFxTarget::Event { track_id, event_id };
        exec!(tl, h, add_effect(&tl, target, "gaussian_blur"));
        exec!(tl, h, toggle_param_animation(&tl, target, 0, "h_range", 0.01));
        assert!(chain(&tl, target).unwrap()[0].param_is_animated("h_range"));

        exec!(
            tl,
            h,
            upsert_param_keyframe(
                &tl,
                target,
                0,
                "h_range",
                EnvelopePoint {
                    time: 4.0,
                    value: 0.09,
                    shape: EnvelopeShape::Linear,
                },
                (0.0, 0.2),
                1e-3,
            )
        );
        let fx = &chain(&tl, target).unwrap()[0];
        assert!((fx.param_at("h_range", 0.01, 2.0) - 0.05).abs() < 1e-5);

        // Splitting at 2.0 s gives the second half a curve starting at 0.05 -> 0.09 at 2.0 s!
        let split = EditCommand::split(&mut tl, track_id, event_id, 2.0).unwrap();
        assert!(h.execute(&mut tl, split));
        let right = &tl.track(track_id).unwrap().events[1];
        assert!((right.video_effects[0].param_at("h_range", 0.01, 0.0) - 0.05).abs() < 1e-5);
        assert!((right.video_effects[0].param_at("h_range", 0.01, 2.0) - 0.09).abs() < 1e-5);
        assert!(h.undo(&mut tl));
    }
}
