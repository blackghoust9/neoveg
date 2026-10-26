//! Timeline, tracks and events. Times are seconds (`f64`, sub-frame precision).

use crate::markers::{Marker, Region};
use std::collections::BTreeMap;

use neoveg_common::fade::FadeCurve;
use serde::{Deserialize, Serialize};

pub type TrackId = u64;
pub type EventId = u64;
pub type GroupId = u64;

/// Tolerance used when comparing event boundaries.
pub const TIME_EPSILON: f64 = 1e-6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackKind {
    Video,
    Audio,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackEvent {
    pub id: EventId,
    pub name: String,
    /// Media pool id (`None` for generated media).
    pub media_id: Option<u64>,
    pub start: f64,
    pub duration: f64,
    /// Offset into the source media where this event begins.
    pub trim_in: f64,
    pub fade_in: f64,
    pub fade_out: f64,
    #[serde(default)]
    pub fade_in_curve: FadeCurve,
    #[serde(default)]
    pub fade_out_curve: FadeCurve,
    /// Events that share a group are selected and moved together (older projects have none).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<GroupId>,
}

impl TrackEvent {
    pub fn end(&self) -> f64 {
        self.start + self.duration
    }
}

/// The editable placement of an event: where it sits and which part of the source it shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EventGeometry {
    pub start: f64,
    pub duration: f64,
    pub trim_in: f64,
    pub fade_in: f64,
    pub fade_out: f64,
    pub fade_in_curve: FadeCurve,
    pub fade_out_curve: FadeCurve,
}

impl EventGeometry {
    pub fn end(&self) -> f64 {
        self.start + self.duration
    }
}

impl TrackEvent {
    pub fn geometry(&self) -> EventGeometry {
        EventGeometry {
            start: self.start,
            duration: self.duration,
            trim_in: self.trim_in,
            fade_in: self.fade_in,
            fade_out: self.fade_out,
            fade_in_curve: self.fade_in_curve,
            fade_out_curve: self.fade_out_curve,
        }
    }

    pub fn set_geometry(&mut self, g: EventGeometry) {
        self.start = g.start;
        self.duration = g.duration;
        self.trim_in = g.trim_in;
        self.fade_in = g.fade_in;
        self.fade_out = g.fade_out;
        self.fade_in_curve = g.fade_in_curve;
        self.fade_out_curve = g.fade_out_curve;
    }
}

/// An audio effect on a track or on the master bus. The engine only stores what the user chose:
/// the registry id of the effect (`neoveg_audio::dsp::effects`), whether it is bypassed and the
/// parameters that differ from their defaults. What the id means is up to the audio layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackEffect {
    pub kind: String,
    #[serde(default)]
    pub bypass: bool,
    #[serde(default)]
    pub params: BTreeMap<String, f32>,
}

impl TrackEffect {
    pub fn new(kind: &str) -> Self {
        Self {
            kind: kind.to_owned(),
            bypass: false,
            params: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: TrackId,
    pub kind: TrackKind,
    pub name: String,
    pub muted: bool,
    pub events: Vec<TrackEvent>,
    /// Effect chain, applied in order to the sum of the track's events (older projects have none).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<TrackEffect>,
}

/// A project timeline. A new timeline has no tracks.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Timeline {
    pub tracks: Vec<Track>,
    /// Named points on the ruler (older projects have none).
    #[serde(default)]
    pub markers: Vec<Marker>,
    /// Named spans on the ruler (older projects have none).
    #[serde(default)]
    pub regions: Vec<Region>,
    /// Effect chain of the master bus, applied after every track is summed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub master_effects: Vec<TrackEffect>,
    next_id: u64,
}

impl Timeline {
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocates a unique id for a track or event.
    pub fn alloc_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    pub fn track(&self, id: TrackId) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == id)
    }

    pub fn track_mut(&mut self, id: TrackId) -> Option<&mut Track> {
        self.tracks.iter_mut().find(|t| t.id == id)
    }

    /// End of the last event on any track.
    pub fn duration(&self) -> f64 {
        self.tracks
            .iter()
            .flat_map(|t| t.events.iter())
            .map(TrackEvent::end)
            .fold(0.0, f64::max)
    }

    /// Builds (without inserting) a new empty track with a fresh id.
    pub fn new_track(&mut self, kind: TrackKind) -> Track {
        let id = self.alloc_id();
        let number = self.tracks.iter().filter(|t| t.kind == kind).count() + 1;
        let label = match kind {
            TrackKind::Video => "Video",
            TrackKind::Audio => "Audio",
        };
        Track {
            id,
            kind,
            name: format!("{label} {number}"),
            muted: false,
            events: Vec::new(),
            effects: Vec::new(),
        }
    }

    /// The event under `time` on `track_id`, if any.
    pub fn event_at(&self, track_id: TrackId, time: f64) -> Option<&TrackEvent> {
        self.track(track_id)?
            .events
            .iter()
            .find(|e| time >= e.start - TIME_EPSILON && time < e.end() - TIME_EPSILON)
    }
}
