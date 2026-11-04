//! *Travar faixa*: a locked track keeps its events as they are. They cannot be moved, trimmed,
//! split, deleted, replaced or given effects, nothing can be put on the track, and no event can
//! leave it. The track itself can still be muted, soloed and mixed, and the lock is taken off the
//! same way it was put on.
//!
//! The rule lives in one place: [`strip_locked`] takes the parts of a command that touch the events
//! of a locked track out of it, and `History::execute` runs every command through it. An operation on
//! a selection that spans a locked and an unlocked track then does its work on the unlocked one
//! only, as an operation does with a locked event.

use crate::command::{EditCommand, TrackChange, VideoFxTarget};
use crate::timeline::{Timeline, TrackId};

/// `true` when the track exists and is locked.
pub fn is_track_locked(timeline: &Timeline, track_id: TrackId) -> bool {
    timeline.track(track_id).is_some_and(|t| t.locked)
}

/// The edit that locks (`true`) or unlocks `track_id`; `None` when it has that state already or
/// does not exist.
pub fn set(timeline: &Timeline, track_id: TrackId, locked: bool) -> Option<EditCommand> {
    let track = timeline.track(track_id)?;
    (track.locked != locked).then_some(EditCommand::SetTrackLocked { track_id, locked })
}

/// `command` without the parts that change the events of a locked track, or `None` when nothing
/// is left of it.
pub fn strip_locked(timeline: &Timeline, command: EditCommand) -> Option<EditCommand> {
    let locked = |id: &TrackId| is_track_locked(timeline, *id);
    match command {
        EditCommand::AddEvent { track_id, .. }
        | EditCommand::RemoveEvent { track_id, .. }
        | EditCommand::MoveEvent { track_id, .. }
        | EditCommand::SplitEvent { track_id, .. }
        | EditCommand::SetPanCrop { track_id, .. }
        | EditCommand::SetTransition { track_id, .. }
        | EditCommand::SetVelocity { track_id, .. }
        | EditCommand::SetEventVolume { track_id, .. }
        | EditCommand::SetTransitionProgress { track_id, .. }
            if locked(&track_id) =>
        {
            None
        }
        EditCommand::EditEvent {
            from_track,
            to_track,
            ..
        } if locked(&from_track) || locked(&to_track) => None,
        EditCommand::SetVideoEffects {
            target: VideoFxTarget::Event { track_id, .. },
            ..
        } if locked(&track_id) => None,
        EditCommand::ReplaceEvents { changes } => {
            let kept: Vec<TrackChange> = changes
                .into_iter()
                .filter(|c| !locked(&c.track_id))
                .collect();
            (!kept.is_empty()).then_some(EditCommand::ReplaceEvents { changes: kept })
        }
        EditCommand::Batch(commands) => {
            let kept: Vec<EditCommand> = commands
                .into_iter()
                .filter_map(|c| strip_locked(timeline, c))
                .collect();
            (!kept.is_empty()).then_some(EditCommand::Batch(kept))
        }
        other => Some(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::timeline::{TrackEvent, TrackKind};

    /// Two audio tracks with one event each; the ids of the tracks and of the events.
    fn timeline() -> (Timeline, [TrackId; 2], [u64; 2]) {
        let mut tl = Timeline::new();
        let mut tracks = [0; 2];
        let mut events = [0; 2];
        for i in 0..2 {
            let mut track = tl.new_track(TrackKind::Audio);
            let event = TrackEvent {
                id: tl.alloc_id(),
                start: 1.0,
                duration: 2.0,
                media_id: Some(1),
                ..TrackEvent::default()
            };
            tracks[i] = track.id;
            events[i] = event.id;
            track.events.push(event);
            tl.tracks.push(track);
        }
        (tl, tracks, events)
    }

    fn lock(tl: &mut Timeline, history: &mut History, track: TrackId) {
        let command = set(tl, track, true).expect("a lock");
        assert!(history.execute(tl, command));
    }

    #[test]
    fn locking_is_an_undoable_edit_and_the_same_state_is_no_edit() {
        let (mut tl, tracks, _) = timeline();
        let original = tl.clone();
        let mut history = History::new();
        assert!(set(&tl, tracks[0], false).is_none(), "already unlocked");
        assert!(set(&tl, 999, true).is_none(), "no such track");
        lock(&mut tl, &mut history, tracks[0]);
        assert!(is_track_locked(&tl, tracks[0]) && !is_track_locked(&tl, tracks[1]));
        assert!(set(&tl, tracks[0], true).is_none());
        let unlock = set(&tl, tracks[0], false).unwrap();
        assert!(history.execute(&mut tl, unlock));
        assert!(!is_track_locked(&tl, tracks[0]));
        assert!(history.undo(&mut tl) && history.undo(&mut tl));
        assert_eq!(tl, original);
    }

    #[test]
    fn nothing_changes_the_events_of_a_locked_track_and_the_other_track_is_not_affected() {
        let (mut tl, tracks, events) = timeline();
        let mut history = History::new();
        lock(&mut tl, &mut history, tracks[0]);
        let before = tl.clone();
        // Remove, add and edit on the locked track: refused, nothing recorded
        let revision = history.revision();
        let event = tl.track(tracks[0]).unwrap().events[0].clone();
        let mut moved = event.geometry();
        moved.start = 5.0;
        for command in [
            EditCommand::RemoveEvent {
                track_id: tracks[0],
                index: 0,
                event: event.clone(),
            },
            EditCommand::AddEvent {
                track_id: tracks[0],
                event: TrackEvent {
                    id: 4242,
                    ..event.clone()
                },
            },
            EditCommand::EditEvent {
                from_track: tracks[0],
                to_track: tracks[0],
                event_id: events[0],
                old: event.geometry(),
                new: moved,
            },
            // Moving an event of the other track onto the locked one
            EditCommand::EditEvent {
                from_track: tracks[1],
                to_track: tracks[0],
                event_id: events[1],
                old: event.geometry(),
                new: moved,
            },
        ] {
            assert!(!history.execute(&mut tl, command));
        }
        assert_eq!(history.revision(), revision);
        assert_eq!(tl, before);
        // The unlocked track still takes edits
        let other = tl.track(tracks[1]).unwrap().events[0].clone();
        let mut later = other.geometry();
        later.start = 3.0;
        assert!(history.execute(
            &mut tl,
            EditCommand::EditEvent {
                from_track: tracks[1],
                to_track: tracks[1],
                event_id: events[1],
                old: other.geometry(),
                new: later,
            }
        ));
        assert_eq!(tl.track(tracks[1]).unwrap().events[0].start, 3.0);
    }

    #[test]
    fn a_change_that_spans_both_tracks_keeps_only_the_unlocked_part() {
        let (mut tl, tracks, _) = timeline();
        let mut history = History::new();
        lock(&mut tl, &mut history, tracks[0]);
        let change = |track: TrackId, start: f64| {
            let t = tl.track(track).unwrap();
            let mut after = t.events.clone();
            after[0].start = start;
            TrackChange {
                track_id: track,
                before: t.events.clone(),
                after,
            }
        };
        let both = EditCommand::ReplaceEvents {
            changes: vec![change(tracks[0], 9.0), change(tracks[1], 9.0)],
        };
        match strip_locked(&tl, both.clone()) {
            Some(EditCommand::ReplaceEvents { changes }) => {
                assert_eq!(changes.len(), 1);
                assert_eq!(changes[0].track_id, tracks[1]);
            }
            other => panic!("{other:?}"),
        }
        let only = EditCommand::Batch(vec![EditCommand::ReplaceEvents {
            changes: vec![change(tracks[0], 7.0)],
        }]);
        assert!(history.execute(&mut tl, both));
        assert_eq!(tl.track(tracks[0]).unwrap().events[0].start, 1.0);
        assert_eq!(tl.track(tracks[1]).unwrap().events[0].start, 9.0);
        // Only locked parts in a batch: nothing is left
        assert!(strip_locked(&tl, only).is_none());
        // A command that is not about events passes through as it is
        let mute = EditCommand::SetCompositeMode {
            track_id: tracks[0],
            before: Default::default(),
            after: Default::default(),
        };
        assert_eq!(strip_locked(&tl, mute.clone()), Some(mute));
    }

    #[test]
    fn the_lock_is_saved_with_the_project_and_an_old_project_reads_as_unlocked() {
        let (mut tl, tracks, _) = timeline();
        assert!(!serde_json::to_string(&tl)
            .unwrap()
            .contains("\"locked\":true"));
        let mut history = History::new();
        lock(&mut tl, &mut history, tracks[1]);
        let json = serde_json::to_string(&tl).unwrap();
        let back: Timeline = serde_json::from_str(&json).unwrap();
        assert!(is_track_locked(&back, tracks[1]) && !is_track_locked(&back, tracks[0]));
    }
}
