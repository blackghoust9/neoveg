//! Adding several media at once: *one after another*, *each on its own track* or *as the takes of
//! one event*. Pure: builds one undoable command from the media list.

use crate::command::EditCommand;
use crate::timeline::{Take, Timeline, TrackEvent, TrackId, TrackKind};

/// How the media are laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddMode {
    /// One after another, on the first track of their kind (a new one when there is none).
    Sequence,
    /// Each on a new track of its own, all starting at the same time.
    Tracks,
    /// One event that holds every media as a take; the first one plays.
    Takes,
}

impl AddMode {
    pub const ALL: [AddMode; 3] = [AddMode::Sequence, AddMode::Tracks, AddMode::Takes];

    pub fn label(self) -> &'static str {
        match self {
            AddMode::Sequence => "Adicionar em sequência",
            AddMode::Tracks => "Adicionar em faixas",
            AddMode::Takes => "Adicionar como takes",
        }
    }
}

/// A media of the project that is being added.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaItem {
    pub media_id: u64,
    pub name: String,
    pub duration: f64,
    pub kind: TrackKind,
}

fn event_of(timeline: &mut Timeline, item: &MediaItem, start: f64) -> TrackEvent {
    TrackEvent {
        id: timeline.alloc_id(),
        name: item.name.clone(),
        media_id: Some(item.media_id),
        start,
        duration: item.duration.max(0.0),
        ..Default::default()
    }
}

/// The first track of `kind` that is not locked, or a new one added by the command.
fn track_for(
    timeline: &mut Timeline,
    kind: TrackKind,
    created: &mut Vec<(TrackKind, TrackId)>,
    commands: &mut Vec<EditCommand>,
) -> TrackId {
    if let Some(track) = timeline
        .tracks
        .iter()
        .find(|t| t.kind == kind && !t.locked && !t.adjustment)
    {
        return track.id;
    }
    if let Some((_, id)) = created.iter().find(|(k, _)| *k == kind) {
        return *id;
    }
    let track = timeline.new_track(kind);
    let id = track.id;
    commands.push(EditCommand::AddTrack { track });
    created.push((kind, id));
    id
}

/// The command that adds `items` from `start` as `mode` says; `None` when there is nothing to add.
///
/// The tracks and events the command adds are allocated from `timeline` (its id counter moves on),
/// but the timeline is not otherwise changed until the command runs.
pub fn add_media(
    timeline: &mut Timeline,
    mode: AddMode,
    items: &[MediaItem],
    start: f64,
) -> Option<EditCommand> {
    if items.is_empty() {
        return None;
    }
    let start = start.max(0.0);
    let mut commands = Vec::new();
    let mut created = Vec::new();
    match mode {
        AddMode::Sequence => {
            let mut at = start;
            for item in items {
                let track_id = track_for(timeline, item.kind, &mut created, &mut commands);
                let event = event_of(timeline, item, at);
                at += event.duration;
                commands.push(EditCommand::AddEvent { track_id, event });
            }
        }
        AddMode::Tracks => {
            for item in items {
                let track = timeline.new_track(item.kind);
                let track_id = track.id;
                commands.push(EditCommand::AddTrack { track });
                let event = event_of(timeline, item, start);
                commands.push(EditCommand::AddEvent { track_id, event });
            }
        }
        AddMode::Takes => {
            let first = &items[0];
            let track_id = track_for(timeline, first.kind, &mut created, &mut commands);
            let mut event = event_of(timeline, first, start);
            // Only media of the same kind can stand in for each other
            event.takes = items
                .iter()
                .filter(|i| i.kind == first.kind)
                .map(|i| Take {
                    media_id: Some(i.media_id),
                    trim_offset: 0.0,
                    name: i.name.clone(),
                })
                .collect();
            event.active_take = 0;
            commands.push(EditCommand::AddEvent { track_id, event });
        }
    }
    Some(if commands.len() == 1 {
        commands.remove(0)
    } else {
        EditCommand::Batch(commands)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;

    fn item(id: u64, duration: f64, kind: TrackKind) -> MediaItem {
        MediaItem {
            media_id: id,
            name: format!("m{id}"),
            duration,
            kind,
        }
    }

    fn run(mode: AddMode, items: &[MediaItem], start: f64) -> Timeline {
        let mut tl = Timeline::default();
        let command = add_media(&mut tl, mode, items, start).expect("a command");
        let mut history = History::new();
        history.execute(&mut tl, command);
        tl
    }

    #[test]
    fn nothing_to_add_gives_no_command() {
        assert!(add_media(&mut Timeline::default(), AddMode::Sequence, &[], 0.0).is_none());
    }

    #[test]
    fn sequence_puts_the_media_one_after_another_on_one_track() {
        let items = [
            item(1, 2.0, TrackKind::Audio),
            item(2, 3.0, TrackKind::Audio),
            item(3, 1.0, TrackKind::Audio),
        ];
        let tl = run(AddMode::Sequence, &items, 4.0);
        assert_eq!(tl.tracks.len(), 1);
        let starts: Vec<f64> = tl.tracks[0].events.iter().map(|e| e.start).collect();
        assert_eq!(starts, vec![4.0, 6.0, 9.0]);
    }

    #[test]
    fn tracks_gives_each_media_a_track_at_the_same_start() {
        let items = [
            item(1, 2.0, TrackKind::Audio),
            item(2, 3.0, TrackKind::Video),
        ];
        let tl = run(AddMode::Tracks, &items, 1.5);
        assert_eq!(tl.tracks.len(), 2);
        assert_eq!(tl.tracks[0].kind, TrackKind::Audio);
        assert_eq!(tl.tracks[1].kind, TrackKind::Video);
        assert!(tl
            .tracks
            .iter()
            .all(|t| t.events.len() == 1 && t.events[0].start == 1.5));
    }

    #[test]
    fn takes_make_one_event_with_every_media_as_a_take() {
        let items = [
            item(1, 2.0, TrackKind::Audio),
            item(2, 3.0, TrackKind::Audio),
            item(3, 1.0, TrackKind::Video),
        ];
        let tl = run(AddMode::Takes, &items, 0.0);
        assert_eq!(tl.tracks.len(), 1);
        let events = &tl.tracks[0].events;
        assert_eq!(events.len(), 1);
        let ids: Vec<_> = events[0].takes.iter().map(|t| t.media_id).collect();
        assert_eq!(
            ids,
            vec![Some(1), Some(2)],
            "the video does not stand in for audio"
        );
        assert_eq!((events[0].media_id, events[0].duration), (Some(1), 2.0));
    }

    #[test]
    fn the_sequence_reuses_a_track_that_is_there_and_undo_removes_everything() {
        let mut tl = Timeline::default();
        let track = tl.new_track(TrackKind::Audio);
        tl.tracks.push(track);
        let items = [
            item(1, 2.0, TrackKind::Audio),
            item(2, 2.0, TrackKind::Audio),
        ];
        let command = add_media(&mut tl, AddMode::Sequence, &items, 0.0).unwrap();
        let mut history = History::new();
        history.execute(&mut tl, command);
        assert_eq!((tl.tracks.len(), tl.tracks[0].events.len()), (1, 2));
        history.undo(&mut tl);
        assert_eq!((tl.tracks.len(), tl.tracks[0].events.len()), (1, 0));
    }
}
