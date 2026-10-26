//! Timeline, tracks and events. Times are seconds (`f64`, sub-frame precision).

use crate::captions::Caption;
use crate::markers::{Marker, Region};
use std::collections::BTreeMap;

use neoveg_common::fade::FadeCurve;
use neoveg_common::pan::PanLaw;
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

fn default_playback_rate() -> f64 {
    1.0
}

fn is_unity_rate(rate: &f64) -> bool {
    (*rate - 1.0).abs() < 1e-6
}

fn default_preserve_pitch() -> bool {
    true
}

fn is_true(v: &bool) -> bool {
    *v
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
    /// Playback speed factor (`1.0` = normal, `0.5` = half speed, `2.0` = double speed).
    #[serde(
        default = "default_playback_rate",
        skip_serializing_if = "is_unity_rate"
    )]
    pub playback_rate: f64,
    /// When `true`, time-stretching keeps the original pitch via WSOLA; when `false`, varispeed.
    #[serde(default = "default_preserve_pitch", skip_serializing_if = "is_true")]
    pub preserve_pitch: bool,
    /// Events that share a group are selected and moved together (older projects have none).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<GroupId>,
}

impl Default for TrackEvent {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::new(),
            media_id: None,
            start: 0.0,
            duration: 0.0,
            trim_in: 0.0,
            fade_in: 0.0,
            fade_out: 0.0,
            fade_in_curve: FadeCurve::default(),
            fade_out_curve: FadeCurve::default(),
            playback_rate: 1.0,
            preserve_pitch: true,
            group: None,
        }
    }
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
    pub playback_rate: f64,
    pub preserve_pitch: bool,
}

impl Default for EventGeometry {
    fn default() -> Self {
        Self {
            start: 0.0,
            duration: 0.0,
            trim_in: 0.0,
            fade_in: 0.0,
            fade_out: 0.0,
            fade_in_curve: FadeCurve::default(),
            fade_out_curve: FadeCurve::default(),
            playback_rate: 1.0,
            preserve_pitch: true,
        }
    }
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
            playback_rate: self.playback_rate,
            preserve_pitch: self.preserve_pitch,
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
        self.playback_rate = g.playback_rate;
        self.preserve_pitch = g.preserve_pitch;
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

fn default_sensitivity() -> f32 {
    6.0
}
fn default_reduce_db() -> f32 {
    15.0
}
fn default_fade_s() -> f32 {
    0.8
}

/// The mixer controls of a track: mute, solo, fader, pan and phase invert.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TrackMix {
    pub muted: bool,
    pub solo: bool,
    /// Fader after the track's effects, in dB (`0` = unity).
    pub volume_db: f32,
    /// `-1.0` (left) to `1.0` (right).
    pub pan: f32,
    /// Flips the polarity of the track.
    pub invert_phase: bool,
}

impl Default for TrackMix {
    fn default() -> Self {
        Self {
            muted: false,
            solo: false,
            volume_db: 0.0,
            pan: 0.0,
            invert_phase: false,
        }
    }
}

impl TrackMix {
    /// Lowest and highest fader value, dB.
    pub const VOLUME_RANGE_DB: (f32, f32) = (-60.0, 12.0);

    /// `true` when the fader, pan and phase leave the audio untouched.
    pub fn is_neutral(&self) -> bool {
        self.volume_db == 0.0 && self.pan == 0.0 && !self.invert_phase
    }

    /// Linear gain of the fader.
    pub fn gain(&self) -> f32 {
        if self.volume_db <= Self::VOLUME_RANGE_DB.0 {
            0.0
        } else {
            10f32.powf(self.volume_db / 20.0)
        }
    }

    /// The same controls with every value clamped to its range (and NaN made neutral).
    pub fn sanitized(mut self) -> Self {
        let (lo, hi) = Self::VOLUME_RANGE_DB;
        self.volume_db = if self.volume_db.is_finite() {
            self.volume_db.clamp(lo, hi)
        } else {
            0.0
        };
        self.pan = if self.pan.is_finite() {
            self.pan.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        self
    }
}

/// Auto-ducking of a track (typically music): whenever the `controllers` tracks (dialogue, effects)
/// have sound, this track is turned down by `reduce_db`, with a smooth ramp that starts before the
/// sound does. The analysis is done when playback starts, on the controllers' audio.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackDucking {
    pub controllers: Vec<TrackId>,
    /// 0 to 10: how quiet a controller may be and still count (higher is more sensitive).
    #[serde(default = "default_sensitivity")]
    pub sensitivity: f32,
    /// How much the track is turned down, dB (positive).
    #[serde(default = "default_reduce_db")]
    pub reduce_db: f32,
    /// Length of the down and up ramps, seconds.
    #[serde(default = "default_fade_s")]
    pub fade_s: f32,
}

impl Default for TrackDucking {
    fn default() -> Self {
        Self {
            controllers: Vec::new(),
            sensitivity: default_sensitivity(),
            reduce_db: default_reduce_db(),
            fade_s: default_fade_s(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: TrackId,
    pub kind: TrackKind,
    pub name: String,
    pub muted: bool,
    /// Solo: while any audio track is soloed, only the soloed ones are heard.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub solo: bool,
    /// Fader after the track's effects, dB (older projects have unity).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub volume_db: f32,
    /// Pan, `-1.0` to `1.0` (older projects have the center).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub pan: f32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub invert_phase: bool,
    pub events: Vec<TrackEvent>,
    /// Effect chain, applied in order to the sum of the track's events (older projects have none).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<TrackEffect>,
    /// Auto-ducking against other tracks (older projects have none).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ducking: Option<TrackDucking>,
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

fn is_default_law(law: &PanLaw) -> bool {
    *law == PanLaw::default()
}

impl Track {
    /// The mixer controls of the track.
    pub fn mix(&self) -> TrackMix {
        TrackMix {
            muted: self.muted,
            solo: self.solo,
            volume_db: self.volume_db,
            pan: self.pan,
            invert_phase: self.invert_phase,
        }
    }

    pub fn set_mix(&mut self, mix: TrackMix) {
        let mix = mix.sanitized();
        self.muted = mix.muted;
        self.solo = mix.solo;
        self.volume_db = mix.volume_db;
        self.pan = mix.pan;
        self.invert_phase = mix.invert_phase;
    }
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
    /// Subtitles (older projects have none).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub captions: Vec<Caption>,
    /// Effect chain of the master bus, applied after every track is summed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub master_effects: Vec<TrackEffect>,
    /// How the pan controls of the tracks turn into left / right gains.
    #[serde(default, skip_serializing_if = "is_default_law")]
    pub pan_law: PanLaw,
    /// Path of the `.cube` color table applied to the video (older projects have none).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_lut: Option<String>,
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
            solo: false,
            volume_db: 0.0,
            pan: 0.0,
            invert_phase: false,
            events: Vec::new(),
            effects: Vec::new(),
            ducking: None,
        }
    }

    /// `true` when the track is heard (or shown, for video): not muted and, for audio tracks
    /// while any audio track is soloed, soloed.
    pub fn audible(&self, track: &Track) -> bool {
        if track.muted {
            return false;
        }
        if track.kind != TrackKind::Audio {
            return true;
        }
        let any_solo = self
            .tracks
            .iter()
            .any(|t| t.kind == TrackKind::Audio && t.solo);
        !any_solo || track.solo
    }

    /// The event under `time` on `track_id`, if any.
    pub fn event_at(&self, track_id: TrackId, time: f64) -> Option<&TrackEvent> {
        self.track(track_id)?
            .events
            .iter()
            .find(|e| time >= e.start - TIME_EPSILON && time < e.end() - TIME_EPSILON)
    }
}
