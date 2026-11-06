//! Default effects for imported audio: the preference "efeitos padrão ao importar áudio" puts a
//! Noise Gate, an equalizer and/or a compressor on the track the sound lands on. Pure and tested
//! without a window.

use crate::command::{EditCommand, FxTarget};
use crate::effects::add_effect;
use crate::timeline::{Timeline, TrackId};

/// Registry id of the gate.
pub const GATE: &str = "gate";
/// Registry id of the equalizer.
pub const EQ: &str = "parametric_eq";
/// Registry id of the compressor.
pub const COMPRESSOR: &str = "compressor";

/// The effects the preference asks for, in chain order (gate, then EQ, then compressor).
pub fn wanted_kinds(gate: bool, eq: bool, compressor: bool) -> Vec<&'static str> {
    [(gate, GATE), (eq, EQ), (compressor, COMPRESSOR)]
        .into_iter()
        .filter_map(|(on, kind)| on.then_some(kind))
        .collect()
}

/// One undoable edit that adds to the chain of `track_id` the effects of `kinds` it does not have
/// yet, so importing a second sound onto the same track adds nothing. `None` when there is nothing
/// to add.
pub fn default_chain_edit(timeline: &Timeline, track_id: TrackId, kinds: &[&str]) -> Option<EditCommand> {
    let mut staged = timeline.clone();
    let mut commands = Vec::new();
    for kind in kinds {
        let present = timeline
            .track(track_id)?
            .effects
            .iter()
            .any(|e| e.kind == *kind);
        if present {
            continue;
        }
        let command = add_effect(&staged, FxTarget::Track(track_id), kind)?;
        // Keep the next `add_effect` seeing the chain with this one in it
        if let EditCommand::SetEffects { after, .. } = &command {
            staged.track_mut(track_id)?.effects = after.clone();
        }
        commands.push(command);
    }
    match commands.len() {
        0 => None,
        _ => Some(EditCommand::Batch(commands)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::timeline::TrackKind;

    fn timeline() -> (Timeline, TrackId) {
        let mut tl = Timeline::new();
        let track = tl.new_track(TrackKind::Audio);
        let id = track.id;
        tl.tracks.push(track);
        (tl, id)
    }

    #[test]
    fn the_preference_flags_pick_the_effects_in_chain_order() {
        assert_eq!(wanted_kinds(true, true, true), [GATE, EQ, COMPRESSOR]);
        assert_eq!(wanted_kinds(false, true, true), [EQ, COMPRESSOR]);
        assert!(wanted_kinds(false, false, false).is_empty());
    }

    #[test]
    fn the_chosen_effects_land_on_the_track_once_and_undo_together() {
        let (mut tl, track) = timeline();
        let kinds = wanted_kinds(true, false, true);
        let edit = default_chain_edit(&tl, track, &kinds).expect("something to add");
        let mut history = History::new();
        assert!(history.execute(&mut tl, edit));
        let chain: Vec<_> = tl.track(track).unwrap().effects.iter().map(|e| e.kind.as_str()).collect();
        assert_eq!(chain, [GATE, COMPRESSOR]);
        // A second import onto the same track adds nothing
        assert!(default_chain_edit(&tl, track, &kinds).is_none());
        // ...but a newly wanted effect is added after the existing ones
        let more = default_chain_edit(&tl, track, &wanted_kinds(true, true, true)).unwrap();
        assert!(history.execute(&mut tl, more));
        assert_eq!(tl.track(track).unwrap().effects.len(), 3);
        assert!(history.undo(&mut tl));
        assert_eq!(tl.track(track).unwrap().effects.len(), 2);
        assert!(history.undo(&mut tl));
        assert!(tl.track(track).unwrap().effects.is_empty());
        assert!(default_chain_edit(&tl, 9999, &kinds).is_none());
    }
}
