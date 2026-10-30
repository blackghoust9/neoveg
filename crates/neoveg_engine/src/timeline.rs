//! Timeline, tracks and events. Times are seconds (`f64`, sub-frame precision).

use crate::captions::Caption;
use crate::envelope::TrackEnvelope;
use crate::markers::{Marker, Region};
use std::collections::BTreeMap;

use neoveg_common::channels::ChannelMode;
use neoveg_common::chroma::ChromaKey;
use neoveg_common::fade::FadeCurve;
use neoveg_common::grade::Grade;
use neoveg_common::pan::PanLaw;
use neoveg_common::pan_crop::PanCrop;
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

fn is_false(v: &bool) -> bool {
    !*v
}

fn default_gain() -> f32 {
    1.0
}

fn is_unity_gain(v: &f32) -> bool {
    (*v - 1.0).abs() < 1e-9
}

/// How a video event treats footage whose frames do not line up with the project's (slow motion,
/// a different frame rate): the nearest frame is shown as it is, or the two around the moment are
/// blended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResampleMode {
    /// Follow the project's setting, which today is no resampling.
    #[default]
    Project,
    /// Blend the frames before and after the moment, in proportion to how close each one is.
    FrameBlend,
    /// No resampling: the nearest frame is shown.
    Disabled,
}

impl ResampleMode {
    pub const ALL: [ResampleMode; 3] = [
        ResampleMode::Project,
        ResampleMode::FrameBlend,
        ResampleMode::Disabled,
    ];

    /// Name for the menu, in pt-BR.
    pub fn label(self) -> &'static str {
        match self {
            ResampleMode::Project => "Usar configuração do projeto",
            ResampleMode::FrameBlend => "Mistura de quadros",
            ResampleMode::Disabled => "Desativar reamostragem",
        }
    }

    pub fn is_default(&self) -> bool {
        *self == ResampleMode::Project
    }

    /// `true` when frames are blended.
    pub fn blends(self) -> bool {
        self == ResampleMode::FrameBlend
    }
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
    /// Event switch Mute: the event is left out of playback (audio and video).
    #[serde(default, skip_serializing_if = "is_false")]
    pub muted: bool,
    /// Event switch Lock: the event cannot be moved, trimmed, split, grouped or deleted.
    #[serde(default, skip_serializing_if = "is_false")]
    pub locked: bool,
    /// Event switch Invert Phase (audio only): the event's samples are negated.
    #[serde(default, skip_serializing_if = "is_false")]
    pub invert_phase: bool,
    /// Event switch Loop: extending the event past the end of its source repeats the source
    /// (from `trim_in` to the end of the media) instead of leaving silence.
    #[serde(default, skip_serializing_if = "is_false")]
    pub looping: bool,
    /// Event switch Normalize (audio only): the event is amplified by `normalize_gain`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub normalize: bool,
    /// Linear gain that brings the event's peak to the project's normalize level; computed when
    /// Normalize is turned on or recalculated, `1.0` otherwise.
    #[serde(default = "default_gain", skip_serializing_if = "is_unity_gain")]
    pub normalize_gain: f32,
    /// Edit > Channels (audio only): how the two channels of the media reach the mix.
    #[serde(default, skip_serializing_if = "ChannelMode::is_default")]
    pub channels: ChannelMode,
    /// Video effect chain of the event, applied in order to the decoded picture before the event's
    /// opacity and the track's effects (older projects have none). The engine only stores the
    /// registry id and the parameters; `neoveg_video::vfx` knows what each effect does.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub video_effects: Vec<TrackEffect>,
    /// Pan/Crop of a video event: the part of the picture it shows, with keyframes measured from
    /// the start of the event (older projects show the whole picture).
    #[serde(default, skip_serializing_if = "PanCrop::is_default")]
    pub pan_crop: PanCrop,
    /// Video transition that reveals the event: it runs over the fade-in (or the overlap with the
    /// event before it) and follows the fade-in curve. `None` is the plain dissolve. The engine stores
    /// the registry id and the values; `neoveg_video::transitions` knows what each one draws.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_in: Option<TrackEffect>,
    /// Video transition that hides the event over its fade-out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_out: Option<TrackEffect>,
    /// Speed of a video event over time, keyframes in seconds from the start of the event: how much
    /// of the source plays per second (negative plays backwards). The length of the event does not
    /// change. Older projects play at the event's `playback_rate`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub velocity: Option<TrackEnvelope>,
    /// Level of an audio event over time, in dB, keyframes in seconds from the start of the event
    /// (`0` leaves the sound alone). It comes on top of the event's gain and fades. Older projects
    /// have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume_envelope: Option<TrackEnvelope>,
    /// An adjustment event: it has no media, and its effect chain runs on the picture of the video
    /// tracks below it for as long as it lasts (see `neoveg_engine::adjustment`).
    #[serde(default, skip_serializing_if = "is_false")]
    pub adjustment: bool,
    /// Event switch Resample Mode (video only).
    #[serde(default, skip_serializing_if = "ResampleMode::is_default")]
    pub resample: ResampleMode,
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
            muted: false,
            locked: false,
            invert_phase: false,
            looping: false,
            normalize: false,
            normalize_gain: 1.0,
            channels: ChannelMode::Both,
            video_effects: Vec::new(),
            pan_crop: PanCrop::default(),
            transition_in: None,
            transition_out: None,
            velocity: None,
            volume_envelope: None,
            adjustment: false,
            resample: ResampleMode::Project,
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
/// Id of an audio bus (from the same counter as track and event ids).
pub type BusId = u64;

/// Where a track or a bus sends its sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BusOutput {
    /// The master bus.
    #[default]
    Master,
    /// Another audio bus.
    Bus(BusId),
}

impl BusOutput {
    pub fn is_master(&self) -> bool {
        *self == BusOutput::Master
    }
}

/// A send from a track to a bus: a copy of the track's sound, at `level_db`, added to the bus.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TrackSend {
    pub bus: BusId,
    /// Level of the send in dB (`-60` and below is silence, `0` unity).
    pub level_db: f32,
    /// Taken after the track's fader (follows it) instead of before it (independent of it, which is
    /// what a cue mix needs; the default).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub post_volume: bool,
    /// The send keeps the pan of the track instead of a centred stereo signal.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub link_pan: bool,
}

/// An audio bus: a channel that tracks (and other buses) send their sound to, with its own effects
/// and fader, before it goes on to the master bus or another bus (manual v21 p.869-873).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioBus {
    pub id: BusId,
    /// A name chosen by the user; empty keeps the default one, the letter of its place (A, B, ...).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Fader, pan, mute and phase of the bus, after its effects.
    #[serde(default)]
    pub mix: TrackMix,
    /// Where the bus goes.
    #[serde(default, skip_serializing_if = "BusOutput::is_master")]
    pub output: BusOutput,
    /// Effect chain of the bus (up to the same limit as a track's).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<TrackEffect>,
}

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
    /// Live sidechain follower driven post-FX and post-fader instead of pre-playback lookahead.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub realtime: bool,
}

impl Default for TrackDucking {
    fn default() -> Self {
        Self {
            controllers: Vec::new(),
            sensitivity: default_sensitivity(),
            reduce_db: default_reduce_db(),
            fade_s: default_fade_s(),
            realtime: false,
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
    /// Keyframed automation envelopes (volume, pan, effect parameters; older projects have none).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub envelopes: Vec<TrackEnvelope>,
    /// Video effect chain of a video track, applied to the track's composited layer (older
    /// projects have none).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub video_effects: Vec<TrackEffect>,
    /// Color a video track fades to with its fade-to-color envelope (older projects fade to black).
    #[serde(default, skip_serializing_if = "is_black")]
    pub fade_color: [u8; 3],
    /// The color the fade envelope goes to below zero (the track's *bottom* color; `fade_color` is the
    /// top one). Older projects have white, which they never reach: their envelopes stay at or above zero.
    #[serde(default = "white", skip_serializing_if = "is_white")]
    pub fade_color_bottom: [u8; 3],
    /// Where the layer of a video track sits in the project's frame, with keyframes in timeline
    /// seconds (older projects leave the layer where it is).
    #[serde(default, skip_serializing_if = "PanCrop::is_default")]
    pub motion: PanCrop,
    /// Where an audio track sends its sound: the master bus, or an audio bus (older projects: master).
    #[serde(default, skip_serializing_if = "BusOutput::is_master")]
    pub output: BusOutput,
    /// Sends of an audio track to buses, besides its output (older projects have none).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sends: Vec<TrackSend>,
}

fn is_black(color: &[u8; 3]) -> bool {
    *color == [0, 0, 0]
}

fn white() -> [u8; 3] {
    [255, 255, 255]
}

fn is_white(color: &[u8; 3]) -> bool {
    *color == [255, 255, 255]
}

fn is_neutral_grade(grade: &Grade) -> bool {
    grade.is_neutral()
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
    /// Automation envelopes for master bus effect parameters (older projects have none).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub master_envelopes: Vec<TrackEnvelope>,
    /// Audio buses, in the order of their letters (older projects have none).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub buses: Vec<AudioBus>,
    /// How the pan controls of the tracks turn into left / right gains.
    #[serde(default, skip_serializing_if = "is_default_law")]
    pub pan_law: PanLaw,
    /// Path of the `.cube` color table applied to the video (older projects have none).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_lut: Option<String>,
    /// Primary color correction of the video (older projects have none).
    #[serde(default, skip_serializing_if = "is_neutral_grade")]
    pub video_grade: Grade,
    /// Chroma key (green/blue screen) settings of the video (older projects have none).
    #[serde(default, skip_serializing_if = "ChromaKey::is_disabled")]
    pub chroma_key: ChromaKey,
    /// Video effect chain of the whole project, applied to the composited picture before the
    /// color correction and the LUT (older projects have none).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub video_effects: Vec<TrackEffect>,
    /// Video effect chain of a media pool entry, by media id: applied to every event that plays
    /// the media, before the event's own chain (older projects have none).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub media_video_effects: BTreeMap<u64, Vec<TrackEffect>>,
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
            envelopes: Vec::new(),
            video_effects: Vec::new(),
            fade_color: [0, 0, 0],
            fade_color_bottom: [255, 255, 255],
            motion: PanCrop::default(),
            output: BusOutput::Master,
            sends: Vec::new(),
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

    /// Like [`Self::event_at`], but muted events are skipped: the event that is actually played.
    pub fn playing_event_at(&self, track_id: TrackId, time: f64) -> Option<&TrackEvent> {
        self.track(track_id)?
            .events
            .iter()
            .find(|e| !e.muted && time >= e.start - TIME_EPSILON && time < e.end() - TIME_EPSILON)
    }
}
