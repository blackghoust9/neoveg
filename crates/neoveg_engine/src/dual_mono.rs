//! *Mono duplo*: a stereo audio event becomes two mono events, one with the left channel and one with
//! the right one, so each side can be edited, mixed and given effects on its own. The left channel stays
//! where the event was; the right channel goes to a new audio track right below it, at the same time,
//! sharing the same media (no file is written). Undoing it puts the stereo event back and removes the
//! track.
//!
//! The channel selection is the event's `ChannelMode`; nothing here touches the media. A locked event
//! or track, an event with no media and an event that is not plain stereo are left alone.

use std::collections::BTreeMap;

use neoveg_common::channels::ChannelMode;

use crate::command::EditCommand;
use crate::selection::EventRef;
use crate::switches;
use crate::timeline::{Timeline, TrackId, TrackKind};

/// Whether the event can be split into two mono events.
pub fn can_split(timeline: &Timeline, (track_id, event_id): EventRef) -> bool {
    timeline
        .track(track_id)
        .filter(|t| t.kind == TrackKind::Audio && !t.locked)
        .and_then(|t| t.events.iter().find(|e| e.id == event_id))
        .is_some_and(|e| e.media_id.is_some() && !e.locked && e.channels == ChannelMode::Both)
}

/// The edit that splits the stereo audio events among `refs` into dual mono, as one undo step; `None`
/// when none of them can be. The events of one track share the one new track below it. `timeline` is
/// only used to hand out the ids of the new tracks and events.
pub fn split(timeline: &mut Timeline, refs: &[EventRef]) -> Option<EditCommand> {
    let mut by_track: BTreeMap<TrackId, Vec<EventRef>> = BTreeMap::new();
    for &r in refs {
        if can_split(timeline, r) {
            by_track.entry(r.0).or_default().push(r);
        }
    }
    if by_track.is_empty() {
        return None;
    }
    // Later tracks first, so the indexes of the earlier ones still hold when each new track is put in
    let mut order: Vec<(usize, TrackId)> = by_track
        .keys()
        .filter_map(|id| {
            timeline
                .tracks
                .iter()
                .position(|t| t.id == *id)
                .map(|i| (i, *id))
        })
        .collect();
    order.sort_by_key(|(index, _)| std::cmp::Reverse(*index));
    let mut commands = Vec::new();
    let mut lefts = Vec::new();
    for (index, track_id) in order {
        let source = timeline.track(track_id)?.clone();
        let mut right = timeline.new_track(TrackKind::Audio);
        right.name = format!("{} (D)", source.name);
        right.output = source.output;
        for &(_, event_id) in &by_track[&track_id] {
            let Some(original) = source.events.iter().find(|e| e.id == event_id) else {
                continue;
            };
            let mut copy = original.clone();
            copy.id = timeline.alloc_id();
            copy.group = None;
            copy.channels = ChannelMode::Right;
            right.events.push(copy);
            lefts.push((track_id, event_id));
        }
        commands.push(EditCommand::InsertTrack {
            track: right,
            index: index + 1,
        });
    }
    // The original events keep the left channel
    if let Some(command) = switches::set_channels(timeline, &lefts, ChannelMode::Left) {
        commands.push(command);
    }
    Some(EditCommand::Batch(commands))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::timeline::TrackEvent;

    fn event(id: u64, start: f64) -> TrackEvent {
        TrackEvent {
            id,
            media_id: Some(7),
            start,
            duration: 2.0,
            ..Default::default()
        }
    }

    fn timeline() -> (Timeline, TrackId, TrackId) {
        let mut tl = Timeline::new();
        let mut a = tl.new_track(TrackKind::Audio);
        a.events = vec![event(100, 0.0), event(101, 3.0)];
        let b = tl.new_track(TrackKind::Audio);
        let (ia, ib) = (a.id, b.id);
        tl.tracks.extend([a, b]);
        (tl, ia, ib)
    }

    #[test]
    fn a_stereo_event_becomes_a_left_event_and_a_right_copy_on_a_new_track_below() {
        let (mut tl, a, b) = timeline();
        let mut h = History::new();
        let command = split(&mut tl, &[(a, 100)]).unwrap();
        assert!(h.execute(&mut tl, command));
        assert_eq!(tl.tracks.len(), 3);
        assert_eq!(tl.tracks[0].id, a);
        assert_ne!(tl.tracks[1].id, b, "the new track sits right below");
        assert_eq!(tl.tracks[2].id, b);
        let events = &tl.track(a).unwrap().events;
        let left = events.iter().find(|e| e.id == 100).unwrap();
        assert_eq!(left.channels, ChannelMode::Left);
        let other = events.iter().find(|e| e.id == 101).unwrap();
        assert_eq!(
            other.channels,
            ChannelMode::Both,
            "the other one was not asked"
        );
        let right = &tl.tracks[1];
        assert_eq!(right.events.len(), 1);
        let copy = &right.events[0];
        assert_eq!(
            (copy.channels, copy.media_id, copy.start, copy.duration),
            (ChannelMode::Right, Some(7), 0.0, 2.0)
        );
        assert_ne!(copy.id, 100, "a copy has an id of its own");
        assert!(right.name.ends_with("(D)"));
        // One undo step takes the new track away and puts the stereo event back
        assert!(h.undo(&mut tl));
        assert_eq!(tl.tracks.len(), 2);
        assert_eq!(tl.track(a).unwrap().events[0].channels, ChannelMode::Both);
        assert!(h.redo(&mut tl));
        assert_eq!(tl.tracks.len(), 3);
    }

    #[test]
    fn events_of_one_track_share_the_new_track_and_each_track_gets_its_own() {
        let (mut tl, a, b) = timeline();
        tl.track_mut(b).unwrap().events.push(event(200, 1.0));
        let mut h = History::new();
        let command = split(&mut tl, &[(a, 100), (a, 101), (b, 200)]).unwrap();
        assert!(h.execute(&mut tl, command));
        let ids: Vec<_> = tl.tracks.iter().map(|t| t.id).collect();
        assert_eq!(tl.tracks.len(), 4);
        assert_eq!(ids[0], a);
        assert_eq!(ids[2], b, "a, its right side, b, its right side");
        assert_eq!(tl.tracks[1].events.len(), 2);
        assert_eq!(tl.tracks[3].events.len(), 1);
        assert!(tl.tracks[1]
            .events
            .iter()
            .chain(&tl.tracks[3].events)
            .all(|e| e.channels == ChannelMode::Right));
    }

    #[test]
    fn locked_mono_medialess_and_video_events_are_left_alone() {
        let (mut tl, a, b) = timeline();
        let mut v = tl.new_track(TrackKind::Video);
        v.events = vec![event(300, 0.0)];
        let video = v.id;
        tl.tracks.push(v);
        tl.track_mut(a).unwrap().events[0].locked = true;
        tl.track_mut(a).unwrap().events[1].channels = ChannelMode::Left;
        tl.track_mut(b).unwrap().events.push(TrackEvent {
            id: 201,
            media_id: None,
            duration: 1.0,
            ..Default::default()
        });
        for r in [(a, 100), (a, 101), (b, 201), (video, 300), (a, 999)] {
            assert!(!can_split(&tl, r), "{r:?}");
        }
        assert!(split(&mut tl, &[(a, 100), (a, 101), (b, 201), (video, 300)]).is_none());
        assert!(split(&mut tl, &[]).is_none());
        // A locked track is not split either
        tl.track_mut(b).unwrap().events.push(event(202, 5.0));
        tl.track_mut(b).unwrap().locked = true;
        assert!(split(&mut tl, &[(b, 202)]).is_none());
    }
}
