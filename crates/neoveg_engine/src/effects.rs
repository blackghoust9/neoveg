//! Effect chains of tracks and of the master bus.
//!
//! Every function is pure: it reads the timeline and returns one [`EditCommand`], so a menu action
//! or a released slider is exactly one undo step. The engine does not know what an effect does; it
//! stores the registry id and the parameter values the user chose (see [`TrackEffect`]).

use crate::command::{EditCommand, FxTarget};
use crate::timeline::{Timeline, TrackEffect};

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
}
