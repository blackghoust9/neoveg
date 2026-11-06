//! *Storyboard*: the events of a video track laid out as cards on a grid, in the order they play.
//! The user drags cards to a new place and sends the new order back to the timeline, where the
//! events are packed one after the other from where the first one started. Pure and tested without
//! a window.

use crate::command::EditCommand;
use crate::timeline::{EventId, Timeline, TrackId, TIME_EPSILON};

/// One card of the storyboard.
#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    pub event_id: EventId,
    pub name: String,
    pub media_id: Option<u64>,
    pub duration: f64,
}

/// The cards of a track, in the order the events start. An empty list for an unknown track.
pub fn cards_of_track(timeline: &Timeline, track_id: TrackId) -> Vec<Card> {
    let Some(track) = timeline.track(track_id) else {
        return Vec::new();
    };
    let mut events: Vec<_> = track.events.iter().collect();
    events.sort_by(|a, b| a.start.total_cmp(&b.start));
    events
        .into_iter()
        .map(|e| Card {
            event_id: e.id,
            name: e.name.clone(),
            media_id: e.media_id,
            duration: e.duration,
        })
        .collect()
}

/// `(row, column)` of card `index` on a grid `columns` wide.
pub fn grid_cell(index: usize, columns: usize) -> (usize, usize) {
    let columns = columns.max(1);
    (index / columns, index % columns)
}

/// Index of the card at `(row, column)`, `None` outside the grid or past the last card.
pub fn index_at(row: usize, column: usize, columns: usize, len: usize) -> Option<usize> {
    let columns = columns.max(1);
    (column < columns).then(|| row * columns + column).filter(|i| *i < len)
}

/// How many columns of `card_width` (plus `gap`) fit in `available` width; at least one.
pub fn columns_for(available: f32, card_width: f32, gap: f32) -> usize {
    (((available + gap) / (card_width + gap).max(1.0)).floor() as usize).max(1)
}

/// Moves the card at `from` so that it ends up at index `to` (both clamped); returns where it is.
pub fn move_card<T>(cards: &mut Vec<T>, from: usize, to: usize) -> usize {
    if cards.is_empty() || from >= cards.len() {
        return from;
    }
    let to = to.min(cards.len() - 1);
    let card = cards.remove(from);
    cards.insert(to, card);
    to
}

/// The edit that plays the events of `track_id` in `order`, packed back to back from the start of
/// the earliest one. `None` when `order` does not name exactly the events of the track, or nothing
/// would move.
pub fn send_to_timeline(
    timeline: &Timeline,
    track_id: TrackId,
    order: &[EventId],
) -> Option<EditCommand> {
    let track = timeline.track(track_id)?;
    if order.len() != track.events.len() {
        return None;
    }
    let first = track.events.iter().map(|e| e.start).fold(f64::INFINITY, f64::min);
    let mut at = first;
    let mut moves = Vec::new();
    let mut seen = Vec::new();
    for id in order {
        let event = track.events.iter().find(|e| e.id == *id)?;
        if seen.contains(id) {
            return None;
        }
        seen.push(*id);
        if (event.start - at).abs() > TIME_EPSILON {
            moves.push(EditCommand::MoveEvent {
                track_id,
                event_id: *id,
                old_start: event.start,
                new_start: at,
            });
        }
        at += event.duration;
    }
    (!moves.is_empty()).then_some(EditCommand::Batch(moves))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::timeline::{TrackEvent, TrackKind};

    fn timeline() -> (Timeline, TrackId, Vec<EventId>) {
        let mut tl = Timeline::new();
        let track = tl.new_track(TrackKind::Video);
        let track_id = track.id;
        tl.tracks.push(track);
        let mut ids = Vec::new();
        // Three cards of 2, 3 and 1 s, with gaps between them, starting at 5 s
        for (name, start, dur) in [("A", 5.0, 2.0), ("B", 9.0, 3.0), ("C", 14.0, 1.0)] {
            let id = tl.alloc_id();
            ids.push(id);
            tl.track_mut(track_id).unwrap().events.push(TrackEvent {
                id,
                name: name.into(),
                start,
                duration: dur,
                ..TrackEvent::default()
            });
        }
        (tl, track_id, ids)
    }

    #[test]
    fn cards_follow_the_start_order_and_sit_on_a_grid() {
        let (mut tl, track, ids) = timeline();
        tl.track_mut(track).unwrap().events.reverse();
        let cards = cards_of_track(&tl, track);
        assert_eq!(cards.iter().map(|c| c.event_id).collect::<Vec<_>>(), ids);
        assert_eq!(grid_cell(0, 4), (0, 0));
        assert_eq!(grid_cell(5, 4), (1, 1));
        assert_eq!(grid_cell(3, 0), (3, 0), "no columns still gives one");
        assert_eq!(index_at(1, 1, 4, 6), Some(5));
        assert_eq!(index_at(1, 2, 4, 6), None, "past the last card");
        assert_eq!(index_at(0, 4, 4, 6), None, "past the last column");
        assert_eq!(columns_for(1000.0, 150.0, 10.0), 6);
        assert_eq!(columns_for(10.0, 150.0, 10.0), 1);
        assert!(cards_of_track(&tl, 9999).is_empty());
    }

    #[test]
    fn a_card_moves_to_a_new_place_in_the_order() {
        let mut v = vec!['a', 'b', 'c', 'd'];
        assert_eq!(move_card(&mut v, 0, 2), 2);
        assert_eq!(v, ['b', 'c', 'a', 'd']);
        assert_eq!(move_card(&mut v, 3, 0), 0);
        assert_eq!(v, ['d', 'b', 'c', 'a']);
        assert_eq!(move_card(&mut v, 1, 99), 3);
        assert_eq!(v, ['d', 'c', 'a', 'b']);
        assert_eq!(move_card(&mut v, 9, 0), 9, "no such card");
    }

    #[test]
    fn sending_a_new_order_packs_the_events_from_the_first_start_and_undoes_in_one_step() {
        let (mut tl, track, ids) = timeline();
        let order = [ids[2], ids[0], ids[1]];
        let command = send_to_timeline(&tl, track, &order).expect("order changes");
        let mut history = History::new();
        assert!(history.execute(&mut tl, command));
        let start_of = |tl: &Timeline, id| tl.track(track).unwrap().events.iter().find(|e| e.id == id).unwrap().start;
        assert_eq!(start_of(&tl, ids[2]), 5.0);
        assert_eq!(start_of(&tl, ids[0]), 6.0);
        assert_eq!(start_of(&tl, ids[1]), 8.0);
        assert!(history.undo(&mut tl));
        assert_eq!(start_of(&tl, ids[0]), 5.0);
        assert_eq!(start_of(&tl, ids[2]), 14.0);
    }

    #[test]
    fn a_bad_order_or_no_change_sends_nothing() {
        let (tl, track, ids) = timeline();
        assert!(send_to_timeline(&tl, track, &ids[..2]).is_none(), "missing a card");
        assert!(send_to_timeline(&tl, track, &[ids[0], ids[0], ids[1]]).is_none(), "repeated card");
        assert!(send_to_timeline(&tl, track, &[ids[0], ids[1], 777]).is_none(), "unknown card");
        // Already packed in this order? Pack once, then the same order changes nothing
        let mut tl = tl;
        let mut history = History::new();
        let packed = send_to_timeline(&tl, track, &ids).unwrap();
        history.execute(&mut tl, packed);
        assert!(send_to_timeline(&tl, track, &ids).is_none());
    }
}
