//! Markers that belong to a media (a cue in the source file) and where they show on the timeline:
//! every event that plays the part of the media holding a marker shows it at the matching time, and
//! the snapping can take those times as targets when the user turns that on.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::timeline::{Timeline, TrackEvent};

/// A named point inside a media, in seconds from its start.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaMarker {
    pub time: f64,
    pub name: String,
}

/// The markers of every media of the project, by media pool id.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MediaMarkerStore {
    #[serde(default)]
    by_media: BTreeMap<u64, Vec<MediaMarker>>,
}

/// A media marker as one event shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct EventMediaMarker {
    pub media_time: f64,
    /// Where it falls on the timeline.
    pub timeline_time: f64,
    pub name: String,
}

impl MediaMarkerStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// `true` when no media has a marker.
    pub fn is_empty(&self) -> bool {
        self.by_media.values().all(Vec::is_empty)
    }

    /// Adds a marker, keeping the list in time order. `false` when one already sits at the time.
    pub fn add(&mut self, media_id: u64, time: f64, name: &str) -> bool {
        let list = self.by_media.entry(media_id).or_default();
        let time = time.max(0.0);
        if list.iter().any(|m| (m.time - time).abs() < 1e-9) {
            return false;
        }
        list.push(MediaMarker {
            time,
            name: name.to_string(),
        });
        list.sort_by(|a, b| a.time.total_cmp(&b.time));
        true
    }

    /// Removes the marker at `time` (within a millisecond). `false` when there is none.
    pub fn remove(&mut self, media_id: u64, time: f64) -> bool {
        let Some(list) = self.by_media.get_mut(&media_id) else {
            return false;
        };
        let before = list.len();
        list.retain(|m| (m.time - time).abs() > 1e-3);
        let removed = list.len() != before;
        if list.is_empty() {
            self.by_media.remove(&media_id);
        }
        removed
    }

    pub fn markers_of(&self, media_id: u64) -> &[MediaMarker] {
        self.by_media.get(&media_id).map(Vec::as_slice).unwrap_or(&[])
    }
}

/// The media markers inside the part of the media that `event` plays, with their times on the
/// timeline (the event's speed taken into account).
pub fn markers_on_event(store: &MediaMarkerStore, event: &TrackEvent) -> Vec<EventMediaMarker> {
    let Some(media_id) = event.media_id else {
        return Vec::new();
    };
    let rate = if event.playback_rate > 0.0 {
        event.playback_rate
    } else {
        1.0
    };
    let from = event.trim_in;
    let to = event.trim_in + event.duration * rate;
    store
        .markers_of(media_id)
        .iter()
        .filter(|m| m.time >= from && m.time <= to)
        .map(|m| EventMediaMarker {
            media_time: m.time,
            timeline_time: event.start + (m.time - from) / rate,
            name: m.name.clone(),
        })
        .collect()
}

/// The timeline times of every media marker of every event, ascending: extra snap targets for
/// when the option is on.
pub fn snap_targets(timeline: &Timeline, store: &MediaMarkerStore) -> Vec<f64> {
    let mut times: Vec<f64> = timeline
        .tracks
        .iter()
        .flat_map(|t| t.events.iter())
        .flat_map(|e| markers_on_event(store, e))
        .map(|m| m.timeline_time)
        .collect();
    times.sort_by(f64::total_cmp);
    times.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    times
}

/// `candidates` with the media-marker times added when `enabled` (the optional snap).
pub fn with_media_markers(
    mut candidates: Vec<f64>,
    timeline: &Timeline,
    store: &MediaMarkerStore,
    enabled: bool,
) -> Vec<f64> {
    if enabled {
        candidates.extend(snap_targets(timeline, store));
    }
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::TrackKind;

    fn event(media: u64, start: f64, duration: f64, trim_in: f64, rate: f64) -> TrackEvent {
        TrackEvent {
            media_id: Some(media),
            start,
            duration,
            trim_in,
            playback_rate: rate,
            ..TrackEvent::default()
        }
    }

    fn store() -> MediaMarkerStore {
        let mut s = MediaMarkerStore::new();
        assert!(s.add(1, 5.0, "cue"));
        assert!(s.add(1, 2.0, "intro"));
        assert!(!s.add(1, 5.0, "again"));
        assert!(s.add(2, 1.0, "other"));
        s
    }

    #[test]
    fn markers_are_kept_in_time_order_and_removed_by_time() {
        let mut s = store();
        let names: Vec<&str> = s.markers_of(1).iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, vec!["intro", "cue"]);
        assert!(s.remove(1, 2.0));
        assert!(!s.remove(1, 2.0));
        assert!(s.remove(2, 1.0));
        assert!(s.markers_of(2).is_empty());
        assert!(s.markers_of(99).is_empty());
    }

    #[test]
    fn an_event_shows_the_markers_of_the_part_of_the_media_it_plays() {
        let s = store();
        // Plays media 1 from 1.0 for 3.5 s at normal speed, placed at 10 s: shows "intro" at 11
        let e = event(1, 10.0, 3.5, 1.0, 1.0);
        let shown = markers_on_event(&s, &e);
        assert_eq!(shown.len(), 1);
        assert_eq!(shown[0].name, "intro");
        assert_eq!(shown[0].timeline_time, 11.0);
        // Double speed: 4 s of timeline cover 8 s of media (1..9): both markers show
        let fast = event(1, 10.0, 4.0, 1.0, 2.0);
        let times: Vec<f64> = markers_on_event(&s, &fast)
            .iter()
            .map(|m| m.timeline_time)
            .collect();
        assert_eq!(times, vec![10.5, 12.0]);
        // A cut that leaves the markers out, and generated media
        assert!(markers_on_event(&s, &event(1, 0.0, 1.0, 2.5, 1.0)).is_empty());
        let mut generated = e.clone();
        generated.media_id = None;
        assert!(markers_on_event(&s, &generated).is_empty());
    }

    #[test]
    fn the_snap_takes_the_media_markers_only_when_it_is_on() {
        let s = store();
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Audio);
        track.events.push(event(1, 10.0, 3.5, 1.0, 1.0));
        track.events.push(event(1, 20.0, 1.0, 5.0, 1.0));
        tl.tracks.push(track);
        assert_eq!(snap_targets(&tl, &s), vec![11.0, 20.0]);
        let base = vec![0.0, 3.0];
        assert_eq!(with_media_markers(base.clone(), &tl, &s, false), base);
        assert_eq!(
            with_media_markers(base, &tl, &s, true),
            vec![0.0, 3.0, 11.0, 20.0]
        );
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<MediaMarkerStore>(&json).unwrap(), s);
    }
}
