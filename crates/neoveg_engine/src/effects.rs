//! Effect chains of tracks and of the master bus.
//!
//! Every function is pure: it reads the timeline and returns one [`EditCommand`], so a menu action
//! or a released slider is exactly one undo step. The engine does not know what an effect does; it
//! stores the registry id and the parameter values the user chose (see [`TrackEffect`]).

use crate::command::{EditCommand, FxTarget};
use crate::timeline::{Timeline, TrackDucking, TrackEffect, TrackId, TrackKind};

/// Longest effect chain of a track or of the master bus.
pub const MAX_EFFECTS: usize = 8;

/// The chain at `target`, or `None` when the track does not exist.
pub fn chain(timeline: &Timeline, target: FxTarget) -> Option<&[TrackEffect]> {
    match target {
        FxTarget::Master => Some(&timeline.master_effects),
        FxTarget::Track(id) => timeline.track(id).map(|t| t.effects.as_slice()),
    }
}

/// A command that replaces the chain with `after`, or `None` when nothing changes.
fn replace(timeline: &Timeline, target: FxTarget, after: Vec<TrackEffect>) -> Option<EditCommand> {
    let before = chain(timeline, target)?.to_vec();
    (before != after).then_some(EditCommand::SetEffects {
        target,
        before,
        after,
    })
}

fn edit(
    timeline: &Timeline,
    target: FxTarget,
    change: impl FnOnce(&mut Vec<TrackEffect>) -> bool,
) -> Option<EditCommand> {
    let mut after = chain(timeline, target)?.to_vec();
    change(&mut after).then(|| replace(timeline, target, after))?
}

/// Appends an effect with default parameters. `None` for a full chain.
pub fn add_effect(timeline: &Timeline, target: FxTarget, kind: &str) -> Option<EditCommand> {
    edit(timeline, target, |list| {
        if list.len() >= MAX_EFFECTS {
            return false;
        }
        list.push(TrackEffect::new(kind));
        true
    })
}

/// Removes the effect at `index`.
pub fn remove_effect(timeline: &Timeline, target: FxTarget, index: usize) -> Option<EditCommand> {
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
    target: FxTarget,
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
    target: FxTarget,
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

/// Sets one parameter. Values are stored as given; the audio layer clamps them to the range of the
/// parameter when it builds the effect.
pub fn set_param(
    timeline: &Timeline,
    target: FxTarget,
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
    target: FxTarget,
    index: usize,
    param: &str,
) -> Option<EditCommand> {
    edit(timeline, target, |list| match list.get_mut(index) {
        Some(effect) => effect.params.remove(param).is_some(),
        None => false,
    })
}

// ---- auto-ducking ----------------------------------------------------------------------------------

/// A number of a track's auto-ducking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuckParam {
    Sensitivity,
    ReduceDb,
    FadeSeconds,
}

impl DuckParam {
    /// Lowest and highest value.
    pub fn range(self) -> (f32, f32) {
        match self {
            DuckParam::Sensitivity => (0.0, 10.0),
            DuckParam::ReduceDb => (3.0, 40.0),
            DuckParam::FadeSeconds => (0.05, 5.0),
        }
    }
}

fn edit_ducking(
    timeline: &Timeline,
    track_id: TrackId,
    change: impl FnOnce(&mut Option<TrackDucking>) -> bool,
) -> Option<EditCommand> {
    let before = timeline.track(track_id)?.ducking.clone();
    let mut after = before.clone();
    if !change(&mut after) || before == after {
        return None;
    }
    Some(EditCommand::SetDucking {
        track_id,
        before,
        after,
    })
}

/// Turns auto-ducking on (with the default amounts and no controllers yet) or off.
pub fn enable_ducking(timeline: &Timeline, track_id: TrackId, on: bool) -> Option<EditCommand> {
    edit_ducking(timeline, track_id, |d| {
        match (on, d.is_some()) {
            (true, false) => *d = Some(TrackDucking::default()),
            (false, true) => *d = None,
            _ => return false,
        }
        true
    })
}

/// Adds or removes `controller` from the tracks this track ducks for. Only other audio tracks can
/// be controllers. `None` when ducking is off or nothing changes.
pub fn set_controller(
    timeline: &Timeline,
    track_id: TrackId,
    controller: TrackId,
    on: bool,
) -> Option<EditCommand> {
    let valid = controller != track_id
        && timeline
            .track(controller)
            .is_some_and(|t| t.kind == TrackKind::Audio);
    if !valid {
        return None;
    }
    edit_ducking(timeline, track_id, |d| {
        let Some(duck) = d else {
            return false;
        };
        let present = duck.controllers.contains(&controller);
        match (on, present) {
            (true, false) => duck.controllers.push(controller),
            (false, true) => duck.controllers.retain(|c| *c != controller),
            _ => return false,
        }
        true
    })
}

/// Sets one amount (kept inside its range). `None` when ducking is off or nothing changes.
pub fn set_duck_param(
    timeline: &Timeline,
    track_id: TrackId,
    param: DuckParam,
    value: f32,
) -> Option<EditCommand> {
    if !value.is_finite() {
        return None;
    }
    let (lo, hi) = param.range();
    let value = value.clamp(lo, hi);
    edit_ducking(timeline, track_id, |d| {
        let Some(duck) = d else {
            return false;
        };
        match param {
            DuckParam::Sensitivity => duck.sensitivity = value,
            DuckParam::ReduceDb => duck.reduce_db = value,
            DuckParam::FadeSeconds => duck.fade_s = value,
        }
        true
    })
}

/// Switches auto-ducking between pre-playback lookahead (`false`) and live sidechain (`true`).
pub fn set_duck_realtime(
    timeline: &Timeline,
    track_id: TrackId,
    realtime: bool,
) -> Option<EditCommand> {
    edit_ducking(timeline, track_id, |d| {
        let Some(duck) = d else {
            return false;
        };
        duck.realtime = realtime;
        true
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::timeline::TrackKind;

    fn setup() -> (Timeline, History, FxTarget) {
        let mut tl = Timeline::new();
        let mut h = History::new();
        let track = tl.new_track(TrackKind::Audio);
        let id = track.id;
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track }));
        (tl, h, FxTarget::Track(id))
    }

    /// Builds a command from the timeline and executes it (the builder borrows `tl` first).
    macro_rules! exec {
        ($tl:ident, $h:ident, $build:expr) => {{
            let cmd = $build;
            assert!($h.execute(&mut $tl, cmd.expect("command")));
        }};
    }

    fn kinds(tl: &Timeline, target: FxTarget) -> Vec<String> {
        chain(tl, target)
            .unwrap()
            .iter()
            .map(|e| e.kind.clone())
            .collect()
    }

    #[test]
    fn chains_are_edited_in_order_and_each_change_is_one_undo_step() {
        let (mut tl, mut h, target) = setup();
        for kind in ["compressor", "simple_eq", "reverb"] {
            exec!(tl, h, add_effect(&tl, target, kind));
        }
        assert_eq!(kinds(&tl, target), ["compressor", "simple_eq", "reverb"]);

        exec!(tl, h, move_effect(&tl, target, 2, 0));
        assert_eq!(kinds(&tl, target), ["reverb", "compressor", "simple_eq"]);
        exec!(tl, h, remove_effect(&tl, target, 1));
        assert_eq!(kinds(&tl, target), ["reverb", "simple_eq"]);

        assert!(h.undo(&mut tl) && h.undo(&mut tl));
        assert_eq!(kinds(&tl, target), ["compressor", "simple_eq", "reverb"]);
        assert!(h.redo(&mut tl));
        assert_eq!(kinds(&tl, target), ["reverb", "compressor", "simple_eq"]);
    }

    #[test]
    fn parameters_and_bypass_are_stored_and_reset() {
        let (mut tl, mut h, target) = setup();
        exec!(tl, h, add_effect(&tl, target, "compressor"));
        exec!(tl, h, set_param(&tl, target, 0, "ratio", 8.0));
        exec!(tl, h, set_bypass(&tl, target, 0, true));
        let effect = &chain(&tl, target).unwrap()[0];
        assert_eq!(effect.params.get("ratio"), Some(&8.0));
        assert!(effect.bypass);

        exec!(tl, h, reset_param(&tl, target, 0, "ratio"));
        assert!(chain(&tl, target).unwrap()[0].params.is_empty());
        assert!(
            reset_param(&tl, target, 0, "ratio").is_none(),
            "nothing to reset"
        );
        assert!(h.undo(&mut tl));
        assert_eq!(
            chain(&tl, target).unwrap()[0].params.get("ratio"),
            Some(&8.0)
        );
    }

    #[test]
    fn the_master_bus_has_its_own_chain() {
        let (mut tl, mut h, track) = setup();
        exec!(tl, h, add_effect(&tl, FxTarget::Master, "limiter"));
        assert_eq!(kinds(&tl, FxTarget::Master), ["limiter"]);
        assert!(kinds(&tl, track).is_empty());
    }

    #[test]
    fn invalid_requests_are_rejected_without_a_command() {
        let (mut tl, mut h, target) = setup();
        assert!(remove_effect(&tl, target, 0).is_none());
        assert!(move_effect(&tl, target, 0, 1).is_none());
        assert!(set_param(&tl, target, 0, "x", 1.0).is_none());
        assert!(add_effect(&tl, FxTarget::Track(999), "reverb").is_none());

        exec!(tl, h, add_effect(&tl, target, "delay"));
        assert!(set_param(&tl, target, 0, "mix", f32::NAN).is_none());
        assert!(set_bypass(&tl, target, 0, false).is_none(), "no change");
        for _ in 1..MAX_EFFECTS {
            exec!(tl, h, add_effect(&tl, target, "delay"));
        }
        assert!(add_effect(&tl, target, "delay").is_none(), "chain is full");
    }

    #[test]
    fn projects_saved_before_effects_still_load_and_empty_chains_stay_out_of_the_file() {
        let tl: Timeline = serde_json::from_str(r#"{"tracks":[],"next_id":3}"#).unwrap();
        assert!(tl.master_effects.is_empty());
        let track: crate::timeline::Track =
            serde_json::from_str(r#"{"id":1,"kind":"Audio","name":"A","muted":false,"events":[]}"#)
                .unwrap();
        assert!(track.effects.is_empty());
        assert!(!serde_json::to_string(&track).unwrap().contains("effects"));

        let mut with = track.clone();
        with.effects.push(TrackEffect::new("reverb"));
        let back: crate::timeline::Track =
            serde_json::from_str(&serde_json::to_string(&with).unwrap()).unwrap();
        assert_eq!(back.effects, with.effects);
    }

    #[test]
    fn ducking_is_switched_on_with_controllers_and_amounts_and_each_change_is_one_step() {
        let (mut tl, mut h, target) = setup();
        let FxTarget::Track(music) = target else {
            unreachable!()
        };
        let voice = tl.new_track(TrackKind::Audio);
        let voice_id = voice.id;
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track: voice }));
        let video = tl.new_track(TrackKind::Video);
        let video_id = video.id;
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track: video }));

        assert!(
            set_controller(&tl, music, voice_id, true).is_none(),
            "ducking is off"
        );
        exec!(tl, h, enable_ducking(&tl, music, true));
        assert!(enable_ducking(&tl, music, true).is_none(), "already on");
        exec!(tl, h, set_controller(&tl, music, voice_id, true));
        assert!(set_controller(&tl, music, voice_id, true).is_none());
        assert!(
            set_controller(&tl, music, music, true).is_none(),
            "not itself"
        );
        assert!(
            set_controller(&tl, music, video_id, true).is_none(),
            "video has no audio"
        );
        exec!(
            tl,
            h,
            set_duck_param(&tl, music, DuckParam::ReduceDb, 100.0)
        );
        let duck = tl.track(music).unwrap().ducking.clone().unwrap();
        assert_eq!(duck.controllers, [voice_id]);
        assert_eq!(duck.reduce_db, 40.0, "clamped to its range");

        assert!(h.undo(&mut tl));
        assert_eq!(
            tl.track(music).unwrap().ducking.as_ref().unwrap().reduce_db,
            15.0
        );
        exec!(tl, h, set_controller(&tl, music, voice_id, false));
        exec!(tl, h, enable_ducking(&tl, music, false));
        assert!(tl.track(music).unwrap().ducking.is_none());
        assert!(set_duck_param(&tl, music, DuckParam::Sensitivity, 5.0).is_none());
    }

    #[test]
    fn projects_saved_before_ducking_still_load_and_the_defaults_fill_missing_numbers() {
        let track: crate::timeline::Track =
            serde_json::from_str(r#"{"id":1,"kind":"Audio","name":"A","muted":false,"events":[]}"#)
                .unwrap();
        assert!(track.ducking.is_none());
        assert!(!serde_json::to_string(&track).unwrap().contains("ducking"));
        let duck: TrackDucking = serde_json::from_str(r#"{"controllers":[2]}"#).unwrap();
        assert_eq!(
            (duck.sensitivity, duck.reduce_db, duck.fade_s),
            (6.0, 15.0, 0.8)
        );
    }
}
