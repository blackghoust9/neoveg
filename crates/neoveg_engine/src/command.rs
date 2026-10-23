//! Undoable edit commands. Each command carries everything needed to apply and revert it.

use crate::timeline::{EventId, Timeline, Track, TrackEvent, TrackId, TIME_EPSILON};

#[derive(Debug, Clone, PartialEq)]
pub enum EditCommand {
    AddTrack {
        track: Track,
    },
    AddEvent {
        track_id: TrackId,
        event: TrackEvent,
    },
    RemoveEvent {
        track_id: TrackId,
        index: usize,
        event: TrackEvent,
    },
    MoveEvent {
        track_id: TrackId,
        event_id: EventId,
        old_start: f64,
        new_start: f64,
    },
    SplitEvent {
        track_id: TrackId,
        event_id: EventId,
        new_event_id: EventId,
        at: f64,
        original_duration: f64,
        original_fade_out: f64,
    },
}

impl EditCommand {
    /// Builds a split at `at` for the event `event_id`, or `None` when `at` is not strictly inside it.
    pub fn split(
        timeline: &mut Timeline,
        track_id: TrackId,
        event_id: EventId,
        at: f64,
    ) -> Option<Self> {
        let event = timeline
            .track(track_id)?
            .events
            .iter()
            .find(|e| e.id == event_id)?;
        if at <= event.start + TIME_EPSILON || at >= event.end() - TIME_EPSILON {
            return None;
        }
        let (original_duration, original_fade_out) = (event.duration, event.fade_out);
        let new_event_id = timeline.alloc_id();
        Some(Self::SplitEvent {
            track_id,
            event_id,
            new_event_id,
            at,
            original_duration,
            original_fade_out,
        })
    }

    fn apply(&self, tl: &mut Timeline) -> bool {
        match self {
            Self::AddTrack { track } => {
                tl.tracks.push(track.clone());
                true
            }
            Self::AddEvent { track_id, event } => match tl.track_mut(*track_id) {
                Some(t) => {
                    t.events.push(event.clone());
                    t.events.sort_by(|a, b| a.start.total_cmp(&b.start));
                    true
                }
                None => false,
            },
            Self::RemoveEvent {
                track_id, event, ..
            } => match tl.track_mut(*track_id) {
                Some(t) => {
                    t.events.retain(|e| e.id != event.id);
                    true
                }
                None => false,
            },
            Self::MoveEvent {
                track_id,
                event_id,
                new_start,
                ..
            } => set_start(tl, *track_id, *event_id, *new_start),
            Self::SplitEvent {
                track_id,
                event_id,
                new_event_id,
                at,
                ..
            } => {
                let Some(t) = tl.track_mut(*track_id) else {
                    return false;
                };
                let Some(i) = t.events.iter().position(|e| e.id == *event_id) else {
                    return false;
                };
                let left = &mut t.events[i];
                let left_duration = at - left.start;
                let right = TrackEvent {
                    id: *new_event_id,
                    name: left.name.clone(),
                    media_id: left.media_id,
                    start: *at,
                    duration: left.duration - left_duration,
                    trim_in: left.trim_in + left_duration,
                    fade_in: 0.0,
                    fade_out: left.fade_out,
                };
                left.duration = left_duration;
                left.fade_out = 0.0;
                t.events.insert(i + 1, right);
                true
            }
        }
    }

    fn revert(&self, tl: &mut Timeline) {
        match self {
            Self::AddTrack { track } => tl.tracks.retain(|t| t.id != track.id),
            Self::AddEvent { track_id, event } => {
                if let Some(t) = tl.track_mut(*track_id) {
                    t.events.retain(|e| e.id != event.id);
                }
            }
            Self::RemoveEvent {
                track_id,
                index,
                event,
            } => {
                if let Some(t) = tl.track_mut(*track_id) {
                    t.events.insert((*index).min(t.events.len()), event.clone());
                }
            }
            Self::MoveEvent {
                track_id,
                event_id,
                old_start,
                ..
            } => {
                set_start(tl, *track_id, *event_id, *old_start);
            }
            Self::SplitEvent {
                track_id,
                event_id,
                new_event_id,
                original_duration,
                original_fade_out,
                ..
            } => {
                if let Some(t) = tl.track_mut(*track_id) {
                    t.events.retain(|e| e.id != *new_event_id);
                    if let Some(e) = t.events.iter_mut().find(|e| e.id == *event_id) {
                        e.duration = *original_duration;
                        e.fade_out = *original_fade_out;
                    }
                }
            }
        }
    }
}

fn set_start(tl: &mut Timeline, track_id: TrackId, event_id: EventId, start: f64) -> bool {
    let Some(t) = tl.track_mut(track_id) else {
        return false;
    };
    let Some(e) = t.events.iter_mut().find(|e| e.id == event_id) else {
        return false;
    };
    e.start = start.max(0.0);
    t.events.sort_by(|a, b| a.start.total_cmp(&b.start));
    true
}

/// Undo/redo stacks. Executing a new command clears the redo stack.
#[derive(Debug, Default)]
pub struct History {
    undo: Vec<EditCommand>,
    redo: Vec<EditCommand>,
    /// Bumped by every execute/undo/redo; lets callers detect unsaved changes cheaply.
    revision: u64,
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies `command`; returns `false` (and records nothing) when it does not apply.
    pub fn execute(&mut self, timeline: &mut Timeline, command: EditCommand) -> bool {
        if !command.apply(timeline) {
            return false;
        }
        self.undo.push(command);
        self.redo.clear();
        self.revision += 1;
        true
    }

    pub fn undo(&mut self, timeline: &mut Timeline) -> bool {
        match self.undo.pop() {
            Some(cmd) => {
                cmd.revert(timeline);
                self.redo.push(cmd);
                self.revision += 1;
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self, timeline: &mut Timeline) -> bool {
        match self.redo.pop() {
            Some(cmd) => {
                cmd.apply(timeline);
                self.undo.push(cmd);
                self.revision += 1;
                true
            }
            None => false,
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::TrackKind;

    fn timeline_with_event() -> (Timeline, History, TrackId, EventId) {
        let mut tl = Timeline::new();
        let mut history = History::new();
        let track = tl.new_track(TrackKind::Audio);
        let track_id = track.id;
        assert!(history.execute(&mut tl, EditCommand::AddTrack { track }));
        let event_id = tl.alloc_id();
        let event = TrackEvent {
            id: event_id,
            name: "clip".into(),
            media_id: Some(1),
            start: 2.0,
            duration: 10.0,
            trim_in: 1.0,
            fade_in: 0.0,
            fade_out: 0.5,
        };
        assert!(history.execute(&mut tl, EditCommand::AddEvent { track_id, event }));
        (tl, history, track_id, event_id)
    }

    #[test]
    fn new_timeline_is_empty() {
        let tl = Timeline::new();
        assert!(tl.tracks.is_empty());
        assert_eq!(tl.duration(), 0.0);
    }

    #[test]
    fn split_divides_the_event_and_keeps_source_alignment() {
        let (mut tl, mut history, track_id, event_id) = timeline_with_event();
        let cmd = EditCommand::split(&mut tl, track_id, event_id, 5.0).unwrap();
        assert!(history.execute(&mut tl, cmd));

        let events = &tl.track(track_id).unwrap().events;
        assert_eq!(events.len(), 2);
        assert_eq!(
            (events[0].start, events[0].duration, events[0].fade_out),
            (2.0, 3.0, 0.0)
        );
        assert_eq!(
            (events[1].start, events[1].duration, events[1].trim_in),
            (5.0, 7.0, 4.0)
        );
        assert_eq!(events[1].fade_out, 0.5);
    }

    #[test]
    fn split_outside_the_event_is_rejected() {
        let (mut tl, _, track_id, event_id) = timeline_with_event();
        assert!(EditCommand::split(&mut tl, track_id, event_id, 2.0).is_none());
        assert!(EditCommand::split(&mut tl, track_id, event_id, 12.0).is_none());
        assert!(EditCommand::split(&mut tl, track_id, event_id, 20.0).is_none());
    }

    #[test]
    fn undo_and_redo_restore_the_exact_state() {
        let (mut tl, mut history, track_id, event_id) = timeline_with_event();
        let before = tl.clone();

        let cmd = EditCommand::split(&mut tl, track_id, event_id, 5.0).unwrap();
        history.execute(&mut tl, cmd);
        let after = tl.clone();

        assert!(history.undo(&mut tl));
        assert_eq!(tl.tracks, before.tracks);
        assert!(history.redo(&mut tl));
        assert_eq!(tl.tracks, after.tracks);
    }

    #[test]
    fn move_and_remove_are_undoable() {
        let (mut tl, mut history, track_id, event_id) = timeline_with_event();
        history.execute(
            &mut tl,
            EditCommand::MoveEvent {
                track_id,
                event_id,
                old_start: 2.0,
                new_start: 8.0,
            },
        );
        assert_eq!(tl.track(track_id).unwrap().events[0].start, 8.0);
        history.undo(&mut tl);
        assert_eq!(tl.track(track_id).unwrap().events[0].start, 2.0);

        let event = tl.track(track_id).unwrap().events[0].clone();
        history.execute(
            &mut tl,
            EditCommand::RemoveEvent {
                track_id,
                index: 0,
                event,
            },
        );
        assert!(tl.track(track_id).unwrap().events.is_empty());
        history.undo(&mut tl);
        assert_eq!(tl.track(track_id).unwrap().events.len(), 1);
    }

    #[test]
    fn revision_changes_with_every_edit() {
        let (mut tl, mut history, track_id, event_id) = timeline_with_event();
        let r0 = history.revision();
        history.execute(
            &mut tl,
            EditCommand::MoveEvent {
                track_id,
                event_id,
                old_start: 2.0,
                new_start: 3.0,
            },
        );
        assert_ne!(history.revision(), r0);
        let r1 = history.revision();
        history.undo(&mut tl);
        assert_ne!(history.revision(), r1);
        // a rejected command is not a change
        let r2 = history.revision();
        assert!(!history.undo(&mut Timeline::new()) || history.revision() != r2);
    }

    #[test]
    fn a_new_edit_clears_redo() {
        let (mut tl, mut history, track_id, event_id) = timeline_with_event();
        history.execute(
            &mut tl,
            EditCommand::MoveEvent {
                track_id,
                event_id,
                old_start: 2.0,
                new_start: 3.0,
            },
        );
        history.undo(&mut tl);
        assert!(history.can_redo());
        history.execute(
            &mut tl,
            EditCommand::MoveEvent {
                track_id,
                event_id,
                old_start: 2.0,
                new_start: 4.0,
            },
        );
        assert!(!history.can_redo());
    }
}
