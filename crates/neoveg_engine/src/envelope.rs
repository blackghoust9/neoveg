//! Parameter automation envelopes: keyframed curves over timeline time for track volume, track pan,
//! audio effect parameters and the video track controls (composite level, fade to color, mute).
//!
//! Envelopes are pure functions in the engine; [`BlockMixer`](../../neoveg_audio/fxmixer/index.html)
//! evaluates them per audio block/sample with zero allocations inside the real-time callback.

use neoveg_common::envelope::{eval_points_opt, flip_points, sort_points as sort_pts, thin_points};
pub use neoveg_common::envelope::{AutomationMode, Envelope, EnvelopePoint, EnvelopeShape};
use neoveg_common::i18n::tr;
use serde::{Deserialize, Serialize};

use crate::command::{EditCommand, FxTarget};
use crate::selection::TimeRange;
use crate::timeline::{Timeline, TrackId, TrackKind, TrackMix, TIME_EPSILON};

fn default_true() -> bool {
    true
}

/// What parameter an automation envelope drives on its track (or master bus).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AutomationTarget {
    /// Track volume in dB (`-60.0..=12.0`).
    Volume,
    /// Track pan (`-1.0..=1.0`).
    Pan,
    /// Audio track mute (`0.0..=1.0`, `0.5` and above silence the track; `Hold` step curve).
    AudioMute,
    /// Parameter `param_id` of the `effect_index`-th effect in the chain.
    EffectParam {
        effect_index: usize,
        param_id: String,
    },
    /// Video track composite level (`0.0..=1.0`, `1.0` is fully opaque).
    VideoLevel,
    /// Video track fade to color (`0.0..=1.0`, `0.0` leaves the picture alone).
    VideoFade,
    /// Video track mute (`0.0..=1.0`, `0.5` and above hide the track).
    VideoMute,
    /// Speed of a video event (`-1.0..=3.0`: `-100 %` reverse to `300 %`, `1.0` normal). Lives on
    /// the event (`TrackEvent::velocity`), in seconds from its start, not on a track.
    EventVelocity,
    /// Level of an audio event in dB (`-60.0..=12.0`, `0.0` leaves it alone). Lives on the event
    /// (`TrackEvent::volume_envelope`), in seconds from its start, not on a track.
    EventVolume,
    /// How far the transition of a video event has gone, `0.0..=1.0` (`1.0`: whole picture, no
    /// transition). Lives on the event (`TrackEvent::transition_progress`), in seconds from its start.
    EventTransitionProgress,
}

impl AutomationTarget {
    /// `true` for the targets that live on video tracks.
    pub fn is_video(&self) -> bool {
        matches!(
            self,
            AutomationTarget::VideoLevel
                | AutomationTarget::VideoFade
                | AutomationTarget::VideoMute
        )
    }

    /// `true` for on/off mute targets whose keyframes step without fading (`Hold`).
    pub fn is_mute(&self) -> bool {
        matches!(
            self,
            AutomationTarget::AudioMute | AutomationTarget::VideoMute
        )
    }

    /// Human-readable short name in pt-BR.
    pub fn label(&self) -> String {
        match self {
            AutomationTarget::Volume => tr("Volume").into(),
            AutomationTarget::Pan => tr("Pan").into(),
            AutomationTarget::AudioMute => tr("Mudo da faixa").into(),
            AutomationTarget::VideoLevel => tr("Nível da faixa").into(),
            AutomationTarget::VideoFade => tr("Esmaecer para cor").into(),
            AutomationTarget::VideoMute => tr("Mudo da faixa").into(),
            AutomationTarget::EventVelocity => tr("Velocidade do evento").into(),
            AutomationTarget::EventVolume => tr("Volume do evento").into(),
            AutomationTarget::EventTransitionProgress => tr("Progresso da transição").into(),
            AutomationTarget::EffectParam {
                effect_index,
                param_id,
            } => format!("FX #{} · {param_id}", effect_index + 1),
        }
    }

    /// Default `(min, max, neutral)` bounds for built-in track targets.
    pub fn default_bounds(&self) -> (f32, f32, f32) {
        match self {
            AutomationTarget::Volume => {
                let (lo, hi) = TrackMix::VOLUME_RANGE_DB;
                (lo, hi, 0.0)
            }
            AutomationTarget::Pan => (-1.0, 1.0, 0.0),
            AutomationTarget::AudioMute => (0.0, 1.0, 0.0),
            AutomationTarget::VideoLevel => (0.0, 1.0, 1.0),
            AutomationTarget::EventVelocity => (-1.0, 3.0, 1.0),
            AutomationTarget::EventTransitionProgress => (0.0, 1.0, 1.0),
            AutomationTarget::EventVolume => {
                let (lo, hi) = TrackMix::VOLUME_RANGE_DB;
                (lo, hi, 0.0)
            }
            AutomationTarget::VideoFade => (-1.0, 1.0, 0.0),
            AutomationTarget::VideoMute => (0.0, 1.0, 0.0),
            AutomationTarget::EffectParam { .. } => (0.0, 1.0, 0.0),
        }
    }
}

/// Keyframed automation curve attached to a track or to the master bus.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackEnvelope {
    pub target: AutomationTarget,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub points: Vec<EnvelopePoint>,
}

impl TrackEnvelope {
    pub fn new(target: AutomationTarget, initial_value: f32) -> Self {
        let shape = if target.is_mute() {
            EnvelopeShape::Hold
        } else {
            EnvelopeShape::Linear
        };
        Self {
            target,
            visible: true,
            enabled: true,
            points: vec![EnvelopePoint {
                time: 0.0,
                value: initial_value,
                shape,
            }],
        }
    }

    /// Projects this track envelope into the generic [`Envelope`] curve representation.
    pub fn to_generic(&self) -> Envelope {
        Envelope {
            enabled: self.enabled,
            points: self.points.clone(),
        }
    }

    /// Evaluates the envelope at timeline `time` seconds, returning `fallback` when disabled or
    /// empty. Outside the first and last keyframe the nearest keyframe value holds.
    pub fn value_at(&self, time: f64, fallback: f32) -> f32 {
        if !self.enabled {
            return fallback;
        }
        eval_points_opt(&self.points, time).unwrap_or(fallback)
    }

    /// Inserts a keyframe or updates an existing one within `tolerance` seconds, keeping `points`
    /// sorted by `time` and `value` clamped to `min..=max`. Returns the point's index.
    pub fn upsert_point(
        &mut self,
        time: f64,
        value: f32,
        shape: EnvelopeShape,
        min: f32,
        max: f32,
        tolerance: f64,
    ) -> usize {
        let t = if time.is_finite() { time.max(0.0) } else { 0.0 };
        let v = if value.is_finite() {
            value.clamp(min, max)
        } else {
            min.clamp(min, max)
        };
        let tol = tolerance.max(TIME_EPSILON);
        if let Some(pos) = self.points.iter().position(|p| (p.time - t).abs() <= tol) {
            self.points[pos].time = t;
            self.points[pos].value = v;
            self.points[pos].shape = shape;
            self.sort_points();
            return self
                .points
                .iter()
                .position(|p| (p.time - t).abs() <= TIME_EPSILON)
                .unwrap_or(pos);
        }
        self.points.push(EnvelopePoint {
            time: t,
            value: v,
            shape,
        });
        self.sort_points();
        self.points
            .iter()
            .position(|p| (p.time - t).abs() <= TIME_EPSILON)
            .unwrap_or(0)
    }

    /// Moves `index` to `(new_time, new_value)`, clamping `new_time` between neighbouring points so
    /// keyframes never cross over one another during a mouse drag.
    pub fn move_point(
        &mut self,
        index: usize,
        new_time: f64,
        new_value: f32,
        min: f32,
        max: f32,
    ) -> Option<usize> {
        if index >= self.points.len() {
            return None;
        }
        let lo_t = if index > 0 {
            self.points[index - 1].time + 1e-4
        } else {
            0.0
        };
        let hi_t = if index + 1 < self.points.len() {
            (self.points[index + 1].time - 1e-4).max(lo_t)
        } else {
            f64::INFINITY
        };
        let t = if new_time.is_finite() {
            new_time.clamp(lo_t, hi_t)
        } else {
            self.points[index].time
        };
        let v = if new_value.is_finite() {
            new_value.clamp(min, max)
        } else {
            self.points[index].value
        };
        self.points[index].time = t;
        self.points[index].value = v;
        Some(index)
    }

    /// Removes the keyframe at `index` (leaving at least one point if desired by the caller, or
    /// allowing an empty curve).
    pub fn remove_point(&mut self, index: usize) -> bool {
        if index < self.points.len() {
            self.points.remove(index);
            true
        } else {
            false
        }
    }

    pub fn sort_points(&mut self) {
        self.points.sort_by(|a, b| a.time.total_cmp(&b.time));
    }
}

// ---- pure EditCommand builders ------------------------------------------------------------------

/// Automation envelopes stored at `target` (track or master bus).
pub fn envelopes(timeline: &Timeline, target: FxTarget) -> Option<&[TrackEnvelope]> {
    match target {
        FxTarget::Master => Some(&timeline.master_envelopes),
        FxTarget::Track(id) => timeline.track(id).map(|t| t.envelopes.as_slice()),
        // A bus has no automation envelopes
        FxTarget::Bus(_) => None,
    }
}

fn edit(
    timeline: &Timeline,
    target: FxTarget,
    change: impl FnOnce(&mut Vec<TrackEnvelope>) -> bool,
) -> Option<EditCommand> {
    if let FxTarget::Track(id) = target {
        // A video track has its own targets and an audio track its own: nothing else is edited
        timeline.track(id)?;
    }
    let before = envelopes(timeline, target)?.to_vec();
    let mut after = before.clone();
    if !change(&mut after) || before == after {
        return None;
    }
    Some(EditCommand::SetEnvelopes {
        target,
        before,
        after,
    })
}

/// Shows or hides the automation envelope for `auto_target`, creating it with `initial_value` at
/// `0.0 s` if it does not exist yet.
pub fn toggle_envelope(
    timeline: &Timeline,
    target: FxTarget,
    auto_target: AutomationTarget,
    initial_value: f32,
) -> Option<EditCommand> {
    if target == FxTarget::Master
        && matches!(
            auto_target,
            AutomationTarget::Volume | AutomationTarget::Pan | AutomationTarget::AudioMute
        )
    {
        return None;
    }
    if let FxTarget::Track(id) = target {
        // Video targets only on video tracks, audio targets only on audio tracks
        let is_video_track = timeline.track(id)?.kind == TrackKind::Video;
        if auto_target.is_video() != is_video_track {
            return None;
        }
    }
    edit(timeline, target, |list| {
        if let Some(env) = list.iter_mut().find(|e| e.target == auto_target) {
            env.visible = !env.visible;
        } else {
            list.push(TrackEnvelope::new(auto_target, initial_value));
        }
        true
    })
}

/// `Ctrl+Shift+E`: hides all visible envelopes on `target`, or reveals all hidden ones (creating a
/// volume envelope when an audio track has no envelopes yet).
pub fn toggle_all_envelopes(timeline: &Timeline, target: FxTarget) -> Option<EditCommand> {
    let default_vol = match target {
        FxTarget::Track(id) => timeline.track(id)?.volume_db,
        FxTarget::Master | FxTarget::Bus(_) => 0.0,
    };
    let video_track = matches!(target, FxTarget::Track(id) if timeline.track(id).is_some_and(|t| t.kind == TrackKind::Video));
    edit(timeline, target, |list| {
        if list.is_empty() {
            if video_track {
                list.push(TrackEnvelope::new(AutomationTarget::VideoLevel, 1.0));
                return true;
            }
            if matches!(target, FxTarget::Track(_)) {
                list.push(TrackEnvelope::new(AutomationTarget::Volume, default_vol));
                return true;
            }
            return false;
        }
        let any_visible = list.iter().any(|e| e.visible);
        for env in list.iter_mut() {
            env.visible = !any_visible;
        }
        true
    })
}

pub fn set_envelope_enabled(
    timeline: &Timeline,
    target: FxTarget,
    auto_target: &AutomationTarget,
    enabled: bool,
) -> Option<EditCommand> {
    edit(timeline, target, |list| {
        let Some(env) = list.iter_mut().find(|e| &e.target == auto_target) else {
            return false;
        };
        env.enabled = enabled;
        true
    })
}

pub fn remove_envelope(
    timeline: &Timeline,
    target: FxTarget,
    auto_target: &AutomationTarget,
) -> Option<EditCommand> {
    edit(timeline, target, |list| {
        let before_len = list.len();
        list.retain(|e| &e.target != auto_target);
        list.len() != before_len
    })
}

pub fn upsert_keyframe(
    timeline: &Timeline,
    target: FxTarget,
    auto_target: &AutomationTarget,
    point: EnvelopePoint,
    bounds: (f32, f32),
    tolerance: f64,
) -> Option<EditCommand> {
    edit(timeline, target, |list| {
        let Some(env) = list.iter_mut().find(|e| &e.target == auto_target) else {
            return false;
        };
        env.upsert_point(
            point.time,
            point.value,
            point.shape,
            bounds.0,
            bounds.1,
            tolerance,
        );
        true
    })
}

pub fn move_keyframe(
    timeline: &Timeline,
    target: FxTarget,
    auto_target: &AutomationTarget,
    index: usize,
    pos: (f64, f32),
    bounds: (f32, f32),
) -> Option<EditCommand> {
    edit(timeline, target, |list| {
        let Some(env) = list.iter_mut().find(|e| &e.target == auto_target) else {
            return false;
        };
        env.move_point(index, pos.0, pos.1, bounds.0, bounds.1)
            .is_some()
    })
}

pub fn set_keyframe_shape(
    timeline: &Timeline,
    target: FxTarget,
    auto_target: &AutomationTarget,
    index: usize,
    shape: EnvelopeShape,
) -> Option<EditCommand> {
    edit(timeline, target, |list| {
        let Some(env) = list.iter_mut().find(|e| &e.target == auto_target) else {
            return false;
        };
        let Some(pt) = env.points.get_mut(index) else {
            return false;
        };
        pt.shape = shape;
        true
    })
}

pub fn remove_keyframe(
    timeline: &Timeline,
    target: FxTarget,
    auto_target: &AutomationTarget,
    index: usize,
) -> Option<EditCommand> {
    edit(timeline, target, |list| {
        let Some(env) = list.iter_mut().find(|e| &e.target == auto_target) else {
            return false;
        };
        if env.points.len() <= 1 {
            return false;
        }
        env.remove_point(index)
    })
}

/// Sets the exact numeric `value` of keyframe `index` (*Definir valor…*), clamped to `bounds`.
pub fn set_keyframe_value(
    timeline: &Timeline,
    target: FxTarget,
    auto_target: &AutomationTarget,
    index: usize,
    value: f32,
    bounds: (f32, f32),
) -> Option<EditCommand> {
    if !value.is_finite() {
        return None;
    }
    let (lo, hi) = if bounds.0 <= bounds.1 {
        bounds
    } else {
        (bounds.1, bounds.0)
    };
    edit(timeline, target, |list| {
        let Some(env) = list.iter_mut().find(|e| &e.target == auto_target) else {
            return false;
        };
        let Some(pt) = env.points.get_mut(index) else {
            return false;
        };
        pt.value = value.clamp(lo, hi);
        true
    })
}

/// Reflects all points (or points inside `range`) around `center`, clamped to `bounds`.
pub fn flip_envelope(
    timeline: &Timeline,
    target: FxTarget,
    auto_target: &AutomationTarget,
    range: Option<TimeRange>,
    bounds: (f32, f32),
    center: f32,
) -> Option<EditCommand> {
    let (lo, hi) = if bounds.0 <= bounds.1 {
        bounds
    } else {
        (bounds.1, bounds.0)
    };
    let span = range.filter(|r| !r.is_empty()).map(|r| (r.start, r.end));
    edit(timeline, target, |list| {
        let Some(env) = list.iter_mut().find(|e| &e.target == auto_target) else {
            return false;
        };
        env.points = flip_points(&env.points, span, center, lo, hi);
        true
    })
}

/// Simplifies redundant interior keyframes across the whole envelope or inside `range`.
pub fn thin_envelope(
    timeline: &Timeline,
    target: FxTarget,
    auto_target: &AutomationTarget,
    range: Option<TimeRange>,
    tolerance: f32,
) -> Option<EditCommand> {
    let span = range.filter(|r| !r.is_empty()).map(|r| (r.start, r.end));
    edit(timeline, target, |list| {
        let Some(env) = list.iter_mut().find(|e| &e.target == auto_target) else {
            return false;
        };
        env.points = thin_points(&env.points, span, tolerance);
        true
    })
}

/// Commits a freehand brush stroke (`Shift`+drag) over `auto_target`, replacing existing points in
/// the stroke's time span and thinning redundant intermediate points by `tolerance`.
pub fn brush_envelope(
    timeline: &Timeline,
    target: FxTarget,
    auto_target: &AutomationTarget,
    stroke: &[(f64, f32)],
    bounds: (f32, f32),
    tolerance: f32,
) -> Option<EditCommand> {
    if stroke.is_empty() {
        return None;
    }
    let (lo, hi) = if bounds.0 <= bounds.1 {
        bounds
    } else {
        (bounds.1, bounds.0)
    };
    let shape = if auto_target.is_mute() {
        EnvelopeShape::Hold
    } else {
        EnvelopeShape::Linear
    };
    let mut drawn: Vec<EnvelopePoint> = stroke
        .iter()
        .filter(|(t, v)| t.is_finite() && v.is_finite())
        .map(|&(t, v)| EnvelopePoint {
            time: t.max(0.0),
            value: v.clamp(lo, hi),
            shape,
        })
        .collect();
    if drawn.is_empty() {
        return None;
    }
    sort_pts(&mut drawn);
    drawn.dedup_by(|b, a| (b.time - a.time).abs() <= TIME_EPSILON);
    let drawn = thin_points(&drawn, None, tolerance);
    let (t_min, t_max) = (drawn[0].time, drawn[drawn.len() - 1].time);

    edit(timeline, target, |list| {
        let Some(env) = list.iter_mut().find(|e| &e.target == auto_target) else {
            return false;
        };
        env.points
            .retain(|p| p.time < t_min - TIME_EPSILON || p.time > t_max + TIME_EPSILON);
        env.points.extend(drawn);
        env.sort_points();
        true
    })
}

/// Copies envelope keyframes (either all points or those within `range`) normalized so the first
/// copied timestamp starts at `0.0 s` (or relative to `range.start` when a range is active).
pub fn copy_points(env: &TrackEnvelope, range: Option<TimeRange>) -> Vec<EnvelopePoint> {
    let span = range.filter(|r| !r.is_empty());
    let selected: Vec<EnvelopePoint> = env
        .points
        .iter()
        .copied()
        .filter(|p| match span {
            None => true,
            Some(r) => p.time >= r.start - TIME_EPSILON && p.time <= r.end + TIME_EPSILON,
        })
        .collect();
    let Some(origin) = span
        .map(|r| r.start)
        .or_else(|| selected.first().map(|p| p.time))
    else {
        return Vec::new();
    };
    selected
        .into_iter()
        .map(|p| EnvelopePoint {
            time: (p.time - origin).max(0.0),
            ..p
        })
        .collect()
}

/// Pastes `clip` keyframes starting at `at_time` on `auto_target`, replacing existing keyframes in
/// the pasted span and clamping values to `bounds`.
pub fn paste_points(
    timeline: &Timeline,
    target: FxTarget,
    auto_target: &AutomationTarget,
    clip: &[EnvelopePoint],
    at_time: f64,
    bounds: (f32, f32),
) -> Option<EditCommand> {
    if clip.is_empty() || !at_time.is_finite() {
        return None;
    }
    let base = at_time.max(0.0);
    let (lo, hi) = if bounds.0 <= bounds.1 {
        bounds
    } else {
        (bounds.1, bounds.0)
    };
    let mut shifted: Vec<EnvelopePoint> = clip
        .iter()
        .map(|p| EnvelopePoint {
            time: (base + p.time.max(0.0)).max(0.0),
            value: p.value.clamp(lo, hi),
            shape: if auto_target.is_mute() {
                EnvelopeShape::Hold
            } else {
                p.shape
            },
        })
        .collect();
    sort_pts(&mut shifted);
    let (t_min, t_max) = (shifted[0].time, shifted[shifted.len() - 1].time);
    edit(timeline, target, |list| {
        let Some(env) = list.iter_mut().find(|e| &e.target == auto_target) else {
            return false;
        };
        env.points
            .retain(|p| p.time < t_min - TIME_EPSILON || p.time > t_max + TIME_EPSILON);
        env.points.extend(shifted);
        env.sort_points();
        true
    })
}

/// Commits a recorded `Touch` (`hold_until == None`) or `Latch` (`hold_until == Some(stop_time)`)
/// automation pass onto `auto_target` as one undo step, creating the envelope if needed and
/// anchoring the pre-existing curve immediately outside the recorded time window.
#[allow(clippy::too_many_arguments)]
pub fn record_automation_pass(
    timeline: &Timeline,
    target: FxTarget,
    auto_target: AutomationTarget,
    initial_value: f32,
    samples: &[(f64, f32)],
    hold_until: Option<f64>,
    bounds: (f32, f32),
    tolerance: f32,
) -> Option<EditCommand> {
    if samples.is_empty() {
        return None;
    }
    if target == FxTarget::Master
        && matches!(
            auto_target,
            AutomationTarget::Volume | AutomationTarget::Pan | AutomationTarget::AudioMute
        )
    {
        return None;
    }
    if let FxTarget::Track(id) = target {
        let is_video_track = timeline.track(id)?.kind == TrackKind::Video;
        if auto_target.is_video() != is_video_track {
            return None;
        }
    }
    let (lo, hi) = if bounds.0 <= bounds.1 {
        bounds
    } else {
        (bounds.1, bounds.0)
    };
    let shape = if auto_target.is_mute() {
        EnvelopeShape::Hold
    } else {
        EnvelopeShape::Linear
    };
    let mut pts: Vec<EnvelopePoint> = samples
        .iter()
        .filter(|(t, v)| t.is_finite() && v.is_finite())
        .map(|&(t, v)| EnvelopePoint {
            time: t.max(0.0),
            value: v.clamp(lo, hi),
            shape,
        })
        .collect();
    if pts.is_empty() {
        return None;
    }
    sort_pts(&mut pts);
    pts.dedup_by(|b, a| (b.time - a.time).abs() <= TIME_EPSILON);
    if let Some(end_t) = hold_until.filter(|t| t.is_finite()) {
        let last = *pts.last().unwrap();
        if end_t > last.time + 1e-4 {
            pts.push(EnvelopePoint {
                time: end_t,
                value: last.value,
                shape,
            });
        }
    }
    let pts = thin_points(&pts, None, tolerance);
    let (t_min, t_max) = (pts[0].time, pts[pts.len() - 1].time);

    edit(timeline, target, |list| {
        let env = match list.iter_mut().find(|e| e.target == auto_target) {
            Some(existing) => {
                existing.visible = true;
                existing.enabled = true;
                existing
            }
            None => {
                list.push(TrackEnvelope::new(
                    auto_target.clone(),
                    initial_value.clamp(lo, hi),
                ));
                list.last_mut().unwrap()
            }
        };
        let has_before = env.points.iter().any(|p| p.time < t_min - 1e-3);
        let has_after = env.points.iter().any(|p| p.time > t_max + 1e-3);
        let pre_anchor = has_before.then(|| {
            let t = (t_min - 1e-3).max(0.0);
            EnvelopePoint {
                time: t,
                value: env.value_at(t, initial_value).clamp(lo, hi),
                shape,
            }
        });
        let post_anchor = has_after.then(|| {
            let t = t_max + 1e-3;
            EnvelopePoint {
                time: t,
                value: env.value_at(t, initial_value).clamp(lo, hi),
                shape,
            }
        });
        env.points
            .retain(|p| p.time < t_min - 1e-3 || p.time > t_max + 1e-3);
        if let Some(a) = pre_anchor {
            env.points.push(a);
        }
        env.points.extend(pts);
        if let Some(b) = post_anchor {
            env.points.push(b);
        }
        env.sort_points();
        true
    })
}

/// *Lock Envelopes to Events*: augments an event-moving command so track-level envelope keyframes
/// and track motion keyframes falling inside any moved event's `[old_start, old_end]` travel by
/// the same delta on the same track, wrapped into a single undoable [`EditCommand`].
pub fn with_locked_envelopes(timeline: &Timeline, command: EditCommand) -> EditCommand {
    let mut moves: Vec<(TrackId, f64, f64, f64)> = Vec::new();
    collect_same_track_moves(timeline, &command, &mut moves);
    if moves.is_empty() {
        return command;
    }
    let mut by_track: std::collections::BTreeMap<TrackId, Vec<(f64, f64, f64)>> =
        std::collections::BTreeMap::new();
    for (tid, s, e, d) in moves {
        by_track.entry(tid).or_default().push((s, e, d));
    }

    let mut extra = Vec::new();
    for (track_id, spans) in by_track {
        let Some(track) = timeline.track(track_id) else {
            continue;
        };
        if !track.envelopes.is_empty() {
            let mut after = track.envelopes.clone();
            for env in &mut after {
                for pt in &mut env.points {
                    if let Some(&(_, _, delta)) = spans.iter().find(|&&(s, e, _)| {
                        pt.time >= s - TIME_EPSILON && pt.time <= e + TIME_EPSILON
                    }) {
                        pt.time = (pt.time + delta).max(0.0);
                    }
                }
                env.sort_points();
            }
            if after != track.envelopes {
                extra.push(EditCommand::SetEnvelopes {
                    target: FxTarget::Track(track_id),
                    before: track.envelopes.clone(),
                    after,
                });
            }
        }
        if !track.motion.keys.is_empty() {
            let mut after = track.motion.clone();
            for key in &mut after.keys {
                if let Some(&(_, _, delta)) = spans.iter().find(|&&(s, e, _)| {
                    key.time >= s - TIME_EPSILON && key.time <= e + TIME_EPSILON
                }) {
                    key.time = (key.time + delta).max(0.0);
                }
            }
            after.keys.sort_by(|a, b| {
                a.time
                    .partial_cmp(&b.time)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            if after != track.motion {
                extra.push(EditCommand::SetTrackMotion {
                    track_id,
                    before: track.motion.clone(),
                    after,
                });
            }
        }
    }

    if extra.is_empty() {
        command
    } else {
        let mut all = match command {
            EditCommand::Batch(list) => list,
            single => vec![single],
        };
        all.extend(extra);
        EditCommand::Batch(all)
    }
}

fn collect_same_track_moves(
    timeline: &Timeline,
    command: &EditCommand,
    out: &mut Vec<(TrackId, f64, f64, f64)>,
) {
    match command {
        EditCommand::MoveEvent {
            track_id,
            event_id,
            old_start,
            new_start,
        } => {
            let delta = *new_start - *old_start;
            if delta.abs() <= TIME_EPSILON {
                return;
            }
            if let Some(ev) = timeline
                .track(*track_id)
                .and_then(|t| t.events.iter().find(|e| e.id == *event_id))
            {
                out.push((*track_id, *old_start, *old_start + ev.duration, delta));
            }
        }
        EditCommand::EditEvent {
            from_track,
            to_track,
            old,
            new,
            ..
        } if from_track == to_track => {
            let delta = new.start - old.start;
            let pure_move = delta.abs() > TIME_EPSILON
                && (new.duration - old.duration).abs() <= 1e-5
                && (new.trim_in - old.trim_in).abs() <= 1e-5;
            if pure_move {
                out.push((*from_track, old.start, old.end(), delta));
            }
        }
        EditCommand::ReplaceEvents { changes } => {
            for change in changes {
                for before in &change.before {
                    if let Some(after) = change.after.iter().find(|e| e.id == before.id) {
                        let delta = after.start - before.start;
                        let pure_move = delta.abs() > TIME_EPSILON
                            && (after.duration - before.duration).abs() <= 1e-5
                            && (after.trim_in - before.trim_in).abs() <= 1e-5;
                        if pure_move {
                            out.push((change.track_id, before.start, before.end(), delta));
                        }
                    }
                }
            }
        }
        EditCommand::Batch(list) => {
            for sub in list {
                collect_same_track_moves(timeline, sub, out);
            }
        }
        _ => {}
    }
}

// ---- audio & video track state ------------------------------------------------------------------

/// `true` when `track` is silenced at timeline `time` by an enabled [`AutomationTarget::AudioMute`]
/// envelope while the track's automation mode reads envelopes.
pub fn audio_track_muted(track: &crate::timeline::Track, time: f64) -> bool {
    if !track.automation_mode.reads_envelopes() {
        return false;
    }
    track
        .envelopes
        .iter()
        .filter(|e| e.enabled && e.target == AutomationTarget::AudioMute)
        .any(|e| {
            let v = e.value_at(time, 0.0);
            v.is_finite() && v >= 0.5
        })
}

/// What the video envelopes of a track say at one instant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VideoTrackState {
    /// Composite level `0.0..=1.0` (`1.0` without an envelope).
    pub level: f32,
    /// Fade to a color of the track, `-1.0..=1.0` (`0.0` without an envelope): above zero towards the
    /// top color, below zero towards the bottom one (see [`fade_mix`]).
    pub fade: f32,
    /// The mute envelope hides the track.
    pub muted: bool,
}

impl Default for VideoTrackState {
    fn default() -> Self {
        Self {
            level: 1.0,
            fade: 0.0,
            muted: false,
        }
    }
}

/// The level, fade and mute of `track` at timeline `time`, from its enabled envelopes. Values are
/// clamped to their range, so a damaged file cannot produce a level above 1 or a NaN.
pub fn video_track_state(track: &crate::timeline::Track, time: f64) -> VideoTrackState {
    if !track.automation_mode.reads_envelopes() {
        return VideoTrackState::default();
    }
    let mut state = VideoTrackState::default();
    for env in track.envelopes.iter().filter(|e| e.enabled) {
        let (min, max, default) = env.target.default_bounds();
        let value = env.value_at(time, default);
        let value = if value.is_finite() {
            value.clamp(min, max)
        } else {
            default
        };
        match env.target {
            AutomationTarget::VideoLevel => state.level = value,
            AutomationTarget::VideoFade => state.fade = value,
            AutomationTarget::VideoMute => state.muted = value >= 0.5,
            _ => {}
        }
    }
    state
}

/// The color and the amount (`0.0..=1.0`) of a fade of `fade` (`-1.0..=1.0`): the top color of the track
/// for a positive value, the bottom one for a negative value, and the size of the value as the amount.
pub fn fade_mix(track: &crate::timeline::Track, fade: f32) -> ([u8; 3], f32) {
    let fade = if fade.is_finite() {
        fade.clamp(-1.0, 1.0)
    } else {
        0.0
    };
    let color = if fade < 0.0 {
        track.fade_color_bottom
    } else {
        track.fade_color
    };
    (color, fade.abs())
}

/// A command that sets the top color a video track fades to, or `None` when it does not change or the
/// track is not a video track.
pub fn set_fade_color(
    timeline: &Timeline,
    track_id: crate::timeline::TrackId,
    color: [u8; 3],
) -> Option<EditCommand> {
    fade_color_command(timeline, track_id, color, false)
}

/// Like [`set_fade_color`], for the bottom color (the one the envelope reaches below zero).
pub fn set_fade_color_bottom(
    timeline: &Timeline,
    track_id: crate::timeline::TrackId,
    color: [u8; 3],
) -> Option<EditCommand> {
    fade_color_command(timeline, track_id, color, true)
}

fn fade_color_command(
    timeline: &Timeline,
    track_id: crate::timeline::TrackId,
    color: [u8; 3],
    bottom: bool,
) -> Option<EditCommand> {
    let track = timeline.track(track_id)?;
    let current = if bottom {
        track.fade_color_bottom
    } else {
        track.fade_color
    };
    (track.kind == TrackKind::Video && current != color).then_some(EditCommand::SetFadeColor {
        track_id,
        bottom,
        before: current,
        after: color,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fade_below_zero_goes_to_the_bottom_color_and_above_zero_to_the_top_one() {
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Video);
        track.fade_color = [10, 20, 30];
        track.fade_color_bottom = [200, 100, 50];
        assert_eq!(fade_mix(&track, 0.5), ([10, 20, 30], 0.5));
        assert_eq!(fade_mix(&track, -0.25), ([200, 100, 50], 0.25));
        assert_eq!(fade_mix(&track, 0.0).1, 0.0);
        assert_eq!(fade_mix(&track, 9.0), ([10, 20, 30], 1.0), "clamped");
        assert_eq!(fade_mix(&track, f32::NAN).1, 0.0);
        // The envelope may go from -1 to 1, and an envelope that stays at or above zero (every older
        // project) keeps fading to the top color alone
        assert_eq!(
            AutomationTarget::VideoFade.default_bounds(),
            (-1.0, 1.0, 0.0)
        );
        let mut fade = TrackEnvelope::new(AutomationTarget::VideoFade, -0.5);
        fade.points.push(EnvelopePoint {
            time: 2.0,
            value: 0.5,
            shape: EnvelopeShape::Linear,
        });
        track.envelopes.push(fade);
        assert_eq!(video_track_state(&track, 0.0).fade, -0.5);
        assert!((video_track_state(&track, 1.0).fade).abs() < 1e-6);
        assert_eq!(video_track_state(&track, 2.0).fade, 0.5);
    }

    #[test]
    fn the_bottom_color_has_its_own_undo_step_and_old_files_get_white_without_writing_it() {
        let mut tl = Timeline::new();
        let video = tl.new_track(TrackKind::Video);
        let id = video.id;
        tl.tracks.push(video);
        assert!(
            set_fade_color_bottom(&tl, id, [255, 255, 255]).is_none(),
            "already white"
        );
        let cmd = set_fade_color_bottom(&tl, id, [1, 2, 3]).unwrap();
        let mut history = crate::command::History::new();
        assert!(history.execute(&mut tl, cmd));
        assert_eq!(tl.track(id).unwrap().fade_color_bottom, [1, 2, 3]);
        assert_eq!(
            tl.track(id).unwrap().fade_color,
            [0, 0, 0],
            "the top color is untouched"
        );
        assert!(history.undo(&mut tl));
        assert_eq!(tl.track(id).unwrap().fade_color_bottom, [255, 255, 255]);
        let mut old = serde_json::to_value(tl.track(id).unwrap()).unwrap();
        assert!(old.get("fade_color_bottom").is_none());
        tl.track_mut(id).unwrap().fade_color_bottom = [9, 8, 7];
        let saved = serde_json::to_string(tl.track(id).unwrap()).unwrap();
        let back: crate::timeline::Track = serde_json::from_str(&saved).unwrap();
        assert_eq!(back.fade_color_bottom, [9, 8, 7]);
        old.as_object_mut().unwrap().remove("fade_color_bottom");
        let loaded: crate::timeline::Track = serde_json::from_value(old).unwrap();
        assert_eq!(loaded.fade_color_bottom, [255, 255, 255]);
    }

    #[test]
    fn linear_smooth_and_hold_shapes_interpolate_accurately() {
        let mut env = TrackEnvelope::new(AutomationTarget::Volume, 0.0);
        env.upsert_point(2.0, -12.0, EnvelopeShape::Hold, -60.0, 12.0, 1e-4);
        env.upsert_point(4.0, 6.0, EnvelopeShape::Linear, -60.0, 12.0, 1e-4);

        assert_eq!(env.value_at(-1.0, 0.0), 0.0);
        assert!((env.value_at(1.0, 0.0) - -6.0).abs() < 1e-5);
        // Hold segment [2.0 .. 4.0) stays at -12.0 until 4.0
        assert!((env.value_at(3.5, 0.0) - -12.0).abs() < 1e-5);
        assert!((env.value_at(4.0, 0.0) - 6.0).abs() < 1e-5);
        assert!((env.value_at(9.0, 0.0) - 6.0).abs() < 1e-5);

        env.enabled = false;
        assert_eq!(env.value_at(1.0, -3.0), -3.0);
    }

    #[test]
    fn move_point_clamps_between_neighbours_and_value_bounds() {
        let mut env = TrackEnvelope::new(AutomationTarget::Pan, -0.5);
        env.upsert_point(1.0, 0.0, EnvelopeShape::Linear, -1.0, 1.0, 1e-4);
        env.upsert_point(3.0, 0.5, EnvelopeShape::Linear, -1.0, 1.0, 1e-4);

        // Middle point (index 1) cannot cross 0.0 or 3.0, and pan clamps to [-1, 1]
        env.move_point(1, 99.0, 4.0, -1.0, 1.0);
        assert!(env.points[1].time < 3.0 && env.points[1].time > 2.99);
        assert_eq!(env.points[1].value, 1.0);

        assert!(env.remove_point(1));
        assert_eq!(env.points.len(), 2);
    }

    #[test]
    fn toggle_upsert_and_undo_round_trip_with_backward_compatible_json() {
        use crate::command::History;

        let mut tl = Timeline::new();
        let mut h = History::new();
        let track = tl.new_track(TrackKind::Audio);
        let tid = track.id;
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track }));

        // Older JSON without `envelopes` or `realtime` loads cleanly
        let legacy_json = serde_json::to_string(&tl).unwrap();
        assert!(!legacy_json.contains("envelopes"));
        let decoded: Timeline = serde_json::from_str(&legacy_json).unwrap();
        assert!(decoded.tracks[0].envelopes.is_empty());

        let cmd =
            toggle_envelope(&tl, FxTarget::Track(tid), AutomationTarget::Volume, -3.0).unwrap();
        assert!(h.execute(&mut tl, cmd));
        assert_eq!(tl.tracks[0].envelopes.len(), 1);
        assert!(tl.tracks[0].envelopes[0].visible);

        let cmd = upsert_keyframe(
            &tl,
            FxTarget::Track(tid),
            &AutomationTarget::Volume,
            EnvelopePoint {
                time: 2.0,
                value: -9.0,
                shape: EnvelopeShape::Smooth,
            },
            (-60.0, 12.0),
            1e-4,
        )
        .unwrap();
        assert!(h.execute(&mut tl, cmd));
        assert_eq!(tl.tracks[0].envelopes[0].points.len(), 2);

        let saved = serde_json::to_string(&tl).unwrap();
        let round: Timeline = serde_json::from_str(&saved).unwrap();
        assert_eq!(round, tl);

        assert!(h.undo(&mut tl));
        assert_eq!(tl.tracks[0].envelopes[0].points.len(), 1);
        assert!(h.undo(&mut tl));
        assert!(tl.tracks[0].envelopes.is_empty());
        assert!(h.redo(&mut tl));
        assert_eq!(tl.tracks[0].envelopes.len(), 1);
    }

    fn video_track(tl: &mut Timeline) -> (crate::command::History, crate::timeline::TrackId) {
        let mut h = crate::command::History::new();
        let track = tl.new_track(TrackKind::Video);
        let id = track.id;
        assert!(h.execute(tl, EditCommand::AddTrack { track }));
        (h, id)
    }

    #[test]
    fn video_targets_live_on_video_tracks_and_audio_targets_on_audio_tracks() {
        let mut tl = Timeline::new();
        let (mut h, vid) = video_track(&mut tl);
        let audio = tl.new_track(TrackKind::Audio);
        let aid = audio.id;
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track: audio }));

        assert!(
            toggle_envelope(&tl, FxTarget::Track(vid), AutomationTarget::Volume, 0.0).is_none()
        );
        assert!(
            toggle_envelope(&tl, FxTarget::Track(aid), AutomationTarget::VideoLevel, 1.0).is_none()
        );
        let cmd =
            toggle_envelope(&tl, FxTarget::Track(vid), AutomationTarget::VideoLevel, 1.0).unwrap();
        assert!(h.execute(&mut tl, cmd));
        assert_eq!(tl.track(vid).unwrap().envelopes.len(), 1);
        // `Ctrl+Shift+E` on a video track starts with the level envelope
        let mut empty = Timeline::new();
        let (_, other) = video_track(&mut empty);
        let cmd = toggle_all_envelopes(&empty, FxTarget::Track(other)).unwrap();
        let mut h2 = crate::command::History::new();
        assert!(h2.execute(&mut empty, cmd));
        assert_eq!(
            empty.track(other).unwrap().envelopes[0].target,
            AutomationTarget::VideoLevel
        );
    }

    #[test]
    fn the_video_state_follows_the_envelopes_and_stays_in_range() {
        let mut tl = Timeline::new();
        let (mut h, id) = video_track(&mut tl);
        assert_eq!(
            video_track_state(tl.track(id).unwrap(), 3.0),
            VideoTrackState::default()
        );

        let mut level = TrackEnvelope::new(AutomationTarget::VideoLevel, 1.0);
        level.upsert_point(4.0, 0.0, EnvelopeShape::Linear, 0.0, 1.0, 1e-4);
        let mut fade = TrackEnvelope::new(AutomationTarget::VideoFade, 0.0);
        fade.upsert_point(2.0, 1.0, EnvelopeShape::Linear, 0.0, 1.0, 1e-4);
        let mut mute = TrackEnvelope::new(AutomationTarget::VideoMute, 0.0);
        // A step: the first keyframe holds until the second one
        mute.points[0].shape = EnvelopeShape::Hold;
        mute.upsert_point(6.0, 1.0, EnvelopeShape::Hold, 0.0, 1.0, 1e-4);
        let before = tl.track(id).unwrap().envelopes.clone();
        assert!(h.execute(
            &mut tl,
            EditCommand::SetEnvelopes {
                target: FxTarget::Track(id),
                before,
                after: vec![level, fade, mute],
            }
        ));
        let at = |t: f64| video_track_state(tl.track(id).unwrap(), t);
        assert!((at(2.0).level - 0.5).abs() < 1e-6);
        assert!((at(1.0).fade - 0.5).abs() < 1e-6);
        assert!(!at(5.0).muted && at(6.0).muted);

        // A disabled envelope is ignored; an out-of-range stored value is clamped
        let mut bad = TrackEnvelope::new(AutomationTarget::VideoLevel, 7.0);
        bad.points[0].value = 7.0;
        tl.track_mut(id).unwrap().envelopes = vec![bad.clone()];
        assert_eq!(video_track_state(tl.track(id).unwrap(), 0.0).level, 1.0);
        bad.enabled = false;
        tl.track_mut(id).unwrap().envelopes = vec![bad];
        assert_eq!(
            video_track_state(tl.track(id).unwrap(), 0.0),
            VideoTrackState::default()
        );
    }

    #[test]
    fn the_fade_color_is_a_video_track_property_with_undo_and_old_files_fade_to_black() {
        let mut tl = Timeline::new();
        let (mut h, id) = video_track(&mut tl);
        assert!(
            set_fade_color(&tl, id, [0, 0, 0]).is_none(),
            "already black"
        );
        let cmd = set_fade_color(&tl, id, [255, 255, 255]).unwrap();
        assert!(h.execute(&mut tl, cmd));
        assert_eq!(tl.track(id).unwrap().fade_color, [255, 255, 255]);
        assert!(h.undo(&mut tl));
        assert_eq!(tl.track(id).unwrap().fade_color, [0, 0, 0]);

        let old: crate::timeline::Track =
            serde_json::from_str(r#"{"id":1,"kind":"Video","name":"V","muted":false,"events":[]}"#)
                .unwrap();
        assert_eq!(old.fade_color, [0, 0, 0]);
        assert!(!serde_json::to_string(&old).unwrap().contains("fade_color"));
        let mut white = old.clone();
        white.fade_color = [9, 8, 7];
        let back: crate::timeline::Track =
            serde_json::from_str(&serde_json::to_string(&white).unwrap()).unwrap();
        assert_eq!(back.fade_color, [9, 8, 7]);

        let audio = tl.new_track(TrackKind::Audio);
        let aid = audio.id;
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track: audio }));
        assert!(
            set_fade_color(&tl, aid, [1, 2, 3]).is_none(),
            "audio tracks have no fade color"
        );
    }

    #[test]
    fn audio_mute_envelope_defaults_to_hold_and_respects_automation_mode_off() {
        let mut tl = Timeline::new();
        let mut h = crate::command::History::new();
        let audio = tl.new_track(TrackKind::Audio);
        let aid = audio.id;
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track: audio }));

        let cmd = toggle_envelope(&tl, FxTarget::Track(aid), AutomationTarget::AudioMute, 0.0)
            .expect("audio mute envelope");
        assert!(h.execute(&mut tl, cmd));
        assert_eq!(
            tl.track(aid).unwrap().envelopes[0].points[0].shape,
            EnvelopeShape::Hold
        );
        let cmd = upsert_keyframe(
            &tl,
            FxTarget::Track(aid),
            &AutomationTarget::AudioMute,
            EnvelopePoint {
                time: 2.0,
                value: 1.0,
                shape: EnvelopeShape::Hold,
            },
            (0.0, 1.0),
            1e-4,
        )
        .unwrap();
        assert!(h.execute(&mut tl, cmd));
        assert!(!audio_track_muted(tl.track(aid).unwrap(), 1.9));
        assert!(audio_track_muted(tl.track(aid).unwrap(), 2.0));

        // AutomationMode::Off bypasses envelope reading
        let off = crate::mix::set_automation_mode(&tl, aid, AutomationMode::Off).unwrap();
        assert!(h.execute(&mut tl, off));
        assert!(!audio_track_muted(tl.track(aid).unwrap(), 2.5));
    }

    #[test]
    fn flip_thin_brush_copy_paste_and_touch_latch_recording_are_pure_undoable_steps() {
        let mut tl = Timeline::new();
        let mut h = crate::command::History::new();
        let audio = tl.new_track(TrackKind::Audio);
        let aid = audio.id;
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track: audio }));
        let target = FxTarget::Track(aid);

        // Touch pass creates Volume envelope and thins collinear points
        let pass = record_automation_pass(
            &tl,
            target,
            AutomationTarget::Volume,
            0.0,
            &[(1.0, -3.0), (1.5, -4.5), (2.0, -6.0)],
            Some(4.0), // Latch hold until 4.0 s
            (-60.0, 12.0),
            0.05,
        )
        .unwrap();
        assert!(h.execute(&mut tl, pass));
        let env = &tl.track(aid).unwrap().envelopes[0];
        assert!((env.value_at(1.5, 0.0) - (-4.5)).abs() < 0.05);
        assert!((env.value_at(3.5, 0.0) - (-6.0)).abs() < 0.05);

        // Flip around 0 dB -> +6 dB at 3.5 s
        let flip = flip_envelope(
            &tl,
            target,
            &AutomationTarget::Volume,
            None,
            (-60.0, 12.0),
            0.0,
        )
        .unwrap();
        assert!(h.execute(&mut tl, flip));
        assert!((tl.track(aid).unwrap().envelopes[0].value_at(3.5, 0.0) - 6.0).abs() < 0.05);

        // Copy & paste points at 6.0 s
        let clip = copy_points(
            &tl.track(aid).unwrap().envelopes[0],
            Some(TimeRange::new(1.0, 4.0)),
        );
        assert!(!clip.is_empty());
        let paste = paste_points(
            &tl,
            target,
            &AutomationTarget::Volume,
            &clip,
            6.0,
            (-60.0, 12.0),
        )
        .unwrap();
        assert!(h.execute(&mut tl, paste));
        assert!(
            tl.track(aid).unwrap().envelopes[0]
                .points
                .last()
                .unwrap()
                .time
                >= 8.9
        );

        // Brush stroke replaces interior span
        let brush = brush_envelope(
            &tl,
            target,
            &AutomationTarget::Volume,
            &[(2.0, -12.0), (2.5, -12.0), (3.0, -12.0)],
            (-60.0, 12.0),
            0.1,
        )
        .unwrap();
        assert!(h.execute(&mut tl, brush));
        assert!((tl.track(aid).unwrap().envelopes[0].value_at(2.5, 0.0) - (-12.0)).abs() < 0.05);
    }

    #[test]
    fn lock_envelopes_to_events_shifts_track_envelopes_and_track_motion_with_moved_event() {
        use neoveg_common::pan_crop::PanCropFrame;
        let mut tl = Timeline::new();
        let mut h = crate::command::History::new();
        let mut video = tl.new_track(TrackKind::Video);
        let vid = video.id;
        let eid = tl.alloc_id();
        video.events.push(crate::timeline::TrackEvent {
            id: eid,
            start: 2.0,
            duration: 3.0,
            ..Default::default()
        });
        let mut lvl = TrackEnvelope::new(AutomationTarget::VideoLevel, 1.0);
        lvl.upsert_point(3.0, 0.4, EnvelopeShape::Linear, 0.0, 1.0, 1e-4);
        lvl.upsert_point(8.0, 0.9, EnvelopeShape::Linear, 0.0, 1.0, 1e-4);
        video.envelopes.push(lvl);
        video.motion.upsert_key(
            3.5,
            PanCropFrame {
                x_center: 0.25,
                ..PanCropFrame::FULL
            },
            1e-3,
        );
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track: video }));

        let raw_move = EditCommand::MoveEvent {
            track_id: vid,
            event_id: eid,
            old_start: 2.0,
            new_start: 4.0,
        };
        let locked = with_locked_envelopes(&tl, raw_move);
        assert!(h.execute(&mut tl, locked));
        let track = tl.track(vid).unwrap();
        // Point at 3.0 s (inside [2, 5]) moved +2.0 s to 5.0 s; point at 8.0 s stayed at 8.0 s!
        assert!(track.envelopes[0]
            .points
            .iter()
            .any(|p| (p.time - 5.0).abs() < 1e-5 && (p.value - 0.4).abs() < 1e-5));
        assert!(track.envelopes[0]
            .points
            .iter()
            .any(|p| (p.time - 8.0).abs() < 1e-5));
        // Track motion key at 3.5 s moved +2.0 s to 5.5 s!
        assert!((track.motion.keys[0].time - 5.5).abs() < 1e-5);

        // One undo reverts event, envelope, and track motion together!
        assert!(h.undo(&mut tl));
        assert!((tl.track(vid).unwrap().events[0].start - 2.0).abs() < 1e-6);
        assert!((tl.track(vid).unwrap().motion.keys[0].time - 3.5).abs() < 1e-5);
    }
}
