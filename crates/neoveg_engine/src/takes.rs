//! Takes: several versions of the media of one event (manual v21, *Using takes as alternate versions
//! of events*). The event keeps its place and length; the *active* take decides what it plays.
//!
//! The event's own `media_id` and `trim_in` are always those of the active take, so the mixer, the
//! compositor, the waveform and every edit read the event as they always did and nothing has to
//! resolve a take. `TrackEvent::takes` lists all the versions (the active one included) with the
//! start of each in its source *relative to the active take's in point*, so a slip, a trim or a
//! split of the event moves every take together (the manual's *Slip all takes*, always on here).
//! Every function builds one [`EditCommand::ReplaceEvents`], so each is one undo step. Locked and
//! adjustment events are left alone.

use crate::command::{EditCommand, TrackChange};
use crate::selection::EventRef;
use crate::timeline::{Take, Timeline, TrackEvent, TrackId, TIME_EPSILON};

/// How many takes the event has (an event with no list has one).
pub fn count(event: &TrackEvent) -> usize {
    event.takes.len().max(1)
}

/// Position of the active take, from 0.
pub fn active_index(event: &TrackEvent) -> usize {
    if event.takes.is_empty() {
        0
    } else {
        event.active_take.min(event.takes.len() - 1)
    }
}

/// Name of the take at `index` (the active one is named by the event when it has no take name).
pub fn take_name(event: &TrackEvent, index: usize) -> String {
    match event.takes.get(index) {
        Some(take) if !take.name.is_empty() => take.name.clone(),
        _ if event.takes.is_empty() => event.name.clone(),
        _ => format!("Take {}", index + 1),
    }
}

/// Makes the list agree with the event: the active take holds the event's media and sits at offset 0.
fn sync_active(event: &mut TrackEvent) {
    if event.takes.is_empty() {
        event.active_take = 0;
        return;
    }
    let active = active_index(event);
    event.active_take = active;
    event.takes[active].media_id = event.media_id;
    event.takes[active].trim_offset = 0.0;
}

/// Makes take `index` the one the event plays: its media and its in point (kept at or after 0), and
/// the other offsets re-based on it.
fn load(event: &mut TrackEvent, index: usize) {
    let base = event.trim_in;
    let wanted = (base + event.takes[index].trim_offset).max(0.0);
    for take in &mut event.takes {
        take.trim_offset = base + take.trim_offset - wanted;
    }
    event.takes[index].trim_offset = 0.0;
    event.media_id = event.takes[index].media_id;
    event.trim_in = wanted;
    event.active_take = index;
}

fn usable(event: &TrackEvent) -> bool {
    !event.locked && !event.adjustment
}

/// Runs `edit` on each usable selected event and gathers the changes into one command; `None` when
/// no event changed.
fn change(
    timeline: &Timeline,
    events: &[EventRef],
    mut edit: impl FnMut(&mut TrackEvent) -> bool,
) -> Option<EditCommand> {
    let mut tracks: Vec<TrackId> = events.iter().map(|(t, _)| *t).collect();
    tracks.sort_unstable();
    tracks.dedup();
    let mut changes = Vec::new();
    for track_id in tracks {
        let Some(track) = timeline.track(track_id) else {
            continue;
        };
        let mut after = track.events.clone();
        let mut changed = false;
        for event in &mut after {
            if events.contains(&(track_id, event.id)) && usable(event) && edit(event) {
                changed = true;
            }
        }
        if changed {
            changes.push(TrackChange {
                track_id,
                before: track.events.clone(),
                after,
            });
        }
    }
    (!changes.is_empty()).then_some(EditCommand::ReplaceEvents { changes })
}

/// *Add as take*: gives each event in `events` a new take that plays `media_id` from `trim_in` (an
/// instant of the source, not before 0) and makes it the active one. The event's previous media stays
/// as the take before it. `name` is the name of the new take.
pub fn add(
    timeline: &Timeline,
    events: &[EventRef],
    media_id: u64,
    name: &str,
    trim_in: f64,
) -> Option<EditCommand> {
    if !trim_in.is_finite() {
        return None;
    }
    let trim_in = trim_in.max(0.0);
    change(timeline, events, |event| {
        if event.takes.is_empty() {
            event.takes.push(Take {
                media_id: event.media_id,
                trim_offset: 0.0,
                name: event.name.clone(),
            });
            event.active_take = 0;
        }
        sync_active(event);
        event.takes.push(Take {
            media_id: Some(media_id),
            trim_offset: trim_in - event.trim_in,
            name: name.to_string(),
        });
        let last = event.takes.len() - 1;
        load(event, last);
        true
    })
}

/// Moves the active take of each event by `delta` places (`1` next, `-1` previous), wrapping around
/// the ends; events with one take are left alone.
pub fn step(timeline: &Timeline, events: &[EventRef], delta: isize) -> Option<EditCommand> {
    change(timeline, events, |event| {
        let n = event.takes.len();
        if n < 2 {
            return false;
        }
        sync_active(event);
        let next = (active_index(event) as isize + delta).rem_euclid(n as isize) as usize;
        load(event, next);
        true
    })
}

/// Makes take `index` the active one of every event in `events` that has it.
pub fn set_active(timeline: &Timeline, events: &[EventRef], index: usize) -> Option<EditCommand> {
    change(timeline, events, |event| {
        if index >= event.takes.len() || index == active_index(event) {
            return false;
        }
        sync_active(event);
        load(event, index);
        true
    })
}

/// *Delete active take*: removes the active take of each event and plays the one that followed it
/// (the one before it for the last). An event never loses its only take: such events are skipped.
pub fn delete_active(timeline: &Timeline, events: &[EventRef]) -> Option<EditCommand> {
    change(timeline, events, |event| {
        if event.takes.len() < 2 {
            return false;
        }
        sync_active(event);
        let removed = active_index(event);
        event.takes.remove(removed);
        let next = removed.min(event.takes.len() - 1);
        // The offsets are still measured from the removed take's in point, which is `trim_in`
        load(event, next);
        if event.takes.len() == 1 {
            event.takes.clear();
            event.active_take = 0;
        }
        true
    })
}

/// A container struct wrapping alternative takes for an event.
///
/// Encapsulates cycling through available takes (`next_take`, `prev_take`) and slipping
/// all takes simultaneously (`slip_all_takes`) while preserving relative media offsets.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TakeContainer {
    pub takes: Vec<Take>,
    pub active_take: usize,
    pub trim_in: f64,
    pub media_id: Option<u64>,
}

impl TakeContainer {
    pub fn new(takes: Vec<Take>, active_take: usize, trim_in: f64, media_id: Option<u64>) -> Self {
        let active = if takes.is_empty() {
            0
        } else {
            active_take.min(takes.len() - 1)
        };
        Self {
            takes,
            active_take: active,
            trim_in: trim_in.max(0.0),
            media_id,
        }
    }

    pub fn from_event(ev: &TrackEvent) -> Self {
        Self {
            takes: ev.takes.clone(),
            active_take: active_index(ev),
            trim_in: ev.trim_in,
            media_id: ev.media_id,
        }
    }

    pub fn apply_to_event(&self, ev: &mut TrackEvent) {
        ev.takes = self.takes.clone();
        ev.active_take = self.active_take;
        ev.trim_in = self.trim_in;
        ev.media_id = self.media_id;
    }

    pub fn count(&self) -> usize {
        self.takes.len().max(1)
    }

    pub fn next_take(&mut self) {
        if self.takes.len() >= 2 {
            let next = (self.active_take + 1) % self.takes.len();
            self.load(next);
        }
    }

    pub fn prev_take(&mut self) {
        if self.takes.len() >= 2 {
            let prev = (self.active_take + self.takes.len() - 1) % self.takes.len();
            self.load(prev);
        }
    }

    /// Slips the source media offsets of all takes in the container simultaneously by `delta_sec`.
    pub fn slip_all_takes(&mut self, delta_sec: f64) {
        self.trim_in = (self.trim_in + delta_sec).max(0.0);
        for take in &mut self.takes {
            take.trim_offset += delta_sec;
        }
    }

    fn load(&mut self, index: usize) {
        let base = self.trim_in;
        let wanted = (base + self.takes[index].trim_offset).max(0.0);
        for take in &mut self.takes {
            take.trim_offset = base + take.trim_offset - wanted;
        }
        self.takes[index].trim_offset = 0.0;
        self.media_id = self.takes[index].media_id;
        self.trim_in = wanted;
        self.active_take = index;
    }
}

/// *Slip all takes*: slips the source media offsets of all takes for selected events
/// simultaneously by `delta_sec`.
pub fn slip_all_takes(
    timeline: &Timeline,
    events: &[EventRef],
    delta_sec: f64,
) -> Option<EditCommand> {
    if !delta_sec.is_finite() || delta_sec.abs() <= TIME_EPSILON {
        return None;
    }
    change(timeline, events, |event| {
        let mut container = TakeContainer::from_event(event);
        container.slip_all_takes(delta_sec);
        container.apply_to_event(event);
        true
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::timeline::TrackKind;

    fn timeline() -> (Timeline, EventRef) {
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Audio);
        let event = TrackEvent {
            id: tl.alloc_id(),
            name: "voz".into(),
            media_id: Some(1),
            start: 2.0,
            duration: 4.0,
            trim_in: 1.0,
            ..TrackEvent::default()
        };
        let reference = (track.id, event.id);
        track.events.push(event);
        tl.tracks.push(track);
        (tl, reference)
    }

    fn event(tl: &Timeline, (track, id): EventRef) -> TrackEvent {
        tl.track(track)
            .unwrap()
            .events
            .iter()
            .find(|e| e.id == id)
            .unwrap()
            .clone()
    }

    fn run(tl: &mut Timeline, history: &mut History, command: Option<EditCommand>) -> bool {
        command.is_some_and(|c| history.execute(tl, c))
    }

    #[test]
    fn adding_a_take_makes_it_active_and_keeps_the_earlier_media_as_the_first_take() {
        let (mut tl, r) = timeline();
        let original = tl.clone();
        let mut history = History::new();
        let c = add(&tl, &[r], 7, "b.wav", 0.5);
        assert!(run(&mut tl, &mut history, c));
        let e = event(&tl, r);
        assert_eq!((count(&e), active_index(&e)), (2, 1));
        assert_eq!((e.media_id, e.trim_in), (Some(7), 0.5));
        assert_eq!(
            (e.start, e.duration),
            (2.0, 4.0),
            "the event keeps its place"
        );
        assert_eq!(e.takes[0].media_id, Some(1));
        assert_eq!(take_name(&e, 0), "voz");
        assert_eq!(take_name(&e, 1), "b.wav");
        // One undo step takes it back, format included
        assert!(history.undo(&mut tl));
        assert_eq!(event(&tl, r), event(&original, r));
    }

    #[test]
    fn next_and_previous_wrap_around_and_restore_each_takes_in_point() {
        let (mut tl, r) = timeline();
        let mut history = History::new();
        for (media, trim) in [(2, 3.0), (3, 0.0)] {
            let c = add(&tl, &[r], media, "x", trim);
            assert!(run(&mut tl, &mut history, c));
        }
        let e = event(&tl, r);
        assert_eq!((active_index(&e), e.media_id, e.trim_in), (2, Some(3), 0.0));
        let c = step(&tl, &[r], 1);
        assert!(run(&mut tl, &mut history, c));
        let e = event(&tl, r);
        assert_eq!((active_index(&e), e.media_id, e.trim_in), (0, Some(1), 1.0));
        let c = step(&tl, &[r], -1);
        assert!(run(&mut tl, &mut history, c));
        let e = event(&tl, r);
        assert_eq!((active_index(&e), e.media_id), (2, Some(3)));
        let c = step(&tl, &[r], -1);
        assert!(run(&mut tl, &mut history, c));
        let e = event(&tl, r);
        assert_eq!((active_index(&e), e.media_id, e.trim_in), (1, Some(2), 3.0));
    }

    #[test]
    fn slipping_the_active_take_moves_every_take() {
        let (mut tl, r) = timeline();
        let mut history = History::new();
        let c = add(&tl, &[r], 2, "b", 3.0);
        assert!(run(&mut tl, &mut history, c));
        // A slip of 0.5 s on the event (its in point moves)
        tl.tracks[0].events[0].trim_in += 0.5;
        let c = set_active(&tl, &[r], 0);
        assert!(run(&mut tl, &mut history, c));
        let e = event(&tl, r);
        assert_eq!((e.media_id, e.trim_in), (Some(1), 1.5));
    }

    #[test]
    fn a_take_cannot_start_before_its_source() {
        let (mut tl, r) = timeline();
        let mut history = History::new();
        // The first take starts at 1.0 and the new one at 5.0; slip back so the first would be < 0
        let c = add(&tl, &[r], 2, "b", 5.0);
        assert!(run(&mut tl, &mut history, c));
        tl.tracks[0].events[0].trim_in = 0.0;
        let c = step(&tl, &[r], 1);
        assert!(run(&mut tl, &mut history, c));
        assert_eq!(event(&tl, r).trim_in, 0.0);
    }

    #[test]
    fn deleting_the_active_take_plays_the_next_and_never_empties_the_event() {
        let (mut tl, r) = timeline();
        let mut history = History::new();
        assert!(delete_active(&tl, &[r]).is_none(), "one take stays");
        for (media, trim) in [(2, 3.0), (3, 0.0)] {
            let c = add(&tl, &[r], media, "x", trim);
            assert!(run(&mut tl, &mut history, c));
        }
        let before_delete = tl.clone();
        let c = set_active(&tl, &[r], 1);
        assert!(run(&mut tl, &mut history, c));
        let c = delete_active(&tl, &[r]);
        assert!(run(&mut tl, &mut history, c));
        let e = event(&tl, r);
        assert_eq!((count(&e), active_index(&e)), (2, 1));
        assert_eq!((e.media_id, e.trim_in), (Some(3), 0.0));
        let c = delete_active(&tl, &[r]);
        assert!(run(&mut tl, &mut history, c));
        let e = event(&tl, r);
        assert_eq!((count(&e), e.media_id), (1, Some(1)));
        assert!(
            e.takes.is_empty() && e.active_take == 0,
            "back to a plain event"
        );
        assert!(delete_active(&tl, &[r]).is_none());
        // Undo twice, then the delete before it, restore the three takes
        assert!(history.undo(&mut tl) && history.undo(&mut tl));
        assert_eq!(count(&event(&tl, r)), 3);
        assert!(history.undo(&mut tl));
        assert_eq!(event(&tl, r), event(&before_delete, r));
    }

    #[test]
    fn locked_adjustment_and_single_take_events_are_left_alone() {
        let (mut tl, r) = timeline();
        assert!(step(&tl, &[r], 1).is_none());
        assert!(set_active(&tl, &[r], 1).is_none());
        assert!(add(&tl, &[r], 2, "b", f64::NAN).is_none());
        tl.tracks[0].events[0].locked = true;
        assert!(add(&tl, &[r], 2, "b", 0.0).is_none());
        tl.tracks[0].events[0].locked = false;
        tl.tracks[0].events[0].adjustment = true;
        assert!(add(&tl, &[r], 2, "b", 0.0).is_none());
        assert!(add(&tl, &[(99, 99)], 2, "b", 0.0).is_none());
    }

    #[test]
    fn projects_without_takes_keep_their_format_and_old_files_open() {
        let (tl, r) = timeline();
        let json = serde_json::to_string(&event(&tl, r)).unwrap();
        assert!(!json.contains("takes") && !json.contains("active_take"));
        let back: TrackEvent = serde_json::from_str(&json).unwrap();
        assert!(back.takes.is_empty() && back.active_take == 0);
        // With takes the lists round-trip
        let (mut tl, r) = timeline();
        let mut history = History::new();
        let c = add(&tl, &[r], 2, "b", 3.0);
        assert!(run(&mut tl, &mut history, c));
        let json = serde_json::to_string(&event(&tl, r)).unwrap();
        let back: TrackEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back, event(&tl, r));
    }
}
