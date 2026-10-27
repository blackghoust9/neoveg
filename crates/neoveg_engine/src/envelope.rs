//! Parameter automation envelopes: keyframed curves over timeline time for track volume, track pan
//! and audio effect parameters.
//!
//! Envelopes are pure functions in the engine; [`BlockMixer`](../../neoveg_audio/fxmixer/index.html)
//! evaluates them per audio block/sample with zero allocations inside the real-time callback.

use neoveg_common::fade::FadeCurve;
use serde::{Deserialize, Serialize};

use crate::command::{EditCommand, FxTarget};
use crate::timeline::{Timeline, TrackKind, TrackMix, TIME_EPSILON};

fn default_true() -> bool {
    true
}

/// Interpolation curve from one keyframe to the next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EnvelopeShape {
    #[default]
    Linear,
    Fast,
    Slow,
    Smooth,
    Sharp,
    /// Holds the keyframe's value constant until the next keyframe's timestamp.
    Hold,
}

impl EnvelopeShape {
    pub const ALL: [EnvelopeShape; 6] = [
        EnvelopeShape::Linear,
        EnvelopeShape::Fast,
        EnvelopeShape::Slow,
        EnvelopeShape::Smooth,
        EnvelopeShape::Sharp,
        EnvelopeShape::Hold,
    ];

    /// Evaluates progress `u` in `[0, 1]` into an interpolation factor in `[0, 1]`.
    pub fn eval(self, u: f64) -> f64 {
        let u = u.clamp(0.0, 1.0);
        match self {
            EnvelopeShape::Linear => FadeCurve::Linear.gain(u),
            EnvelopeShape::Fast => FadeCurve::Fast.gain(u),
            EnvelopeShape::Slow => FadeCurve::Slow.gain(u),
            EnvelopeShape::Smooth => FadeCurve::Smooth.gain(u),
            EnvelopeShape::Sharp => FadeCurve::Sharp.gain(u),
            EnvelopeShape::Hold => {
                if u >= 1.0 {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }

    /// Label in pt-BR.
    pub fn label(self) -> &'static str {
        match self {
            EnvelopeShape::Linear => "Linear",
            EnvelopeShape::Fast => "Rápida",
            EnvelopeShape::Slow => "Lenta",
            EnvelopeShape::Smooth => "Suave",
            EnvelopeShape::Sharp => "Acentuada",
            EnvelopeShape::Hold => "Degrau (manter)",
        }
    }
}

/// One automation keyframe on an envelope.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EnvelopePoint {
    pub time: f64,
    pub value: f32,
    #[serde(default)]
    pub shape: EnvelopeShape,
}

/// What parameter an automation envelope drives on its track (or master bus).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AutomationTarget {
    /// Track volume in dB (`-60.0..=12.0`).
    Volume,
    /// Track pan (`-1.0..=1.0`).
    Pan,
    /// Parameter `param_id` of the `effect_index`-th effect in the chain.
    EffectParam {
        effect_index: usize,
        param_id: String,
    },
}

impl AutomationTarget {
    /// Human-readable short name in pt-BR.
    pub fn label(&self) -> String {
        match self {
            AutomationTarget::Volume => "Volume".into(),
            AutomationTarget::Pan => "Pan".into(),
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
        Self {
            target,
            visible: true,
            enabled: true,
            points: vec![EnvelopePoint {
                time: 0.0,
                value: initial_value,
                shape: EnvelopeShape::Linear,
            }],
        }
    }

    /// Evaluates the envelope at timeline `time` seconds, returning `fallback` when disabled or
    /// empty. Outside the first and last keyframe the nearest keyframe value holds.
    pub fn value_at(&self, time: f64, fallback: f32) -> f32 {
        if !self.enabled {
            return fallback;
        }
        let (Some(first), Some(last)) = (self.points.first(), self.points.last()) else {
            return fallback;
        };
        if time <= first.time {
            return first.value;
        }
        if time >= last.time {
            return last.value;
        }
        let idx = self.points.partition_point(|p| p.time <= time);
        if idx == 0 {
            return first.value;
        }
        if idx >= self.points.len() {
            return last.value;
        }
        let a = self.points[idx - 1];
        let b = self.points[idx];
        let span = b.time - a.time;
        if span <= TIME_EPSILON {
            return b.value;
        }
        let u = a.shape.eval((time - a.time) / span) as f32;
        a.value + (b.value - a.value) * u
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
    }
}

fn edit(
    timeline: &Timeline,
    target: FxTarget,
    change: impl FnOnce(&mut Vec<TrackEnvelope>) -> bool,
) -> Option<EditCommand> {
    if let FxTarget::Track(id) = target {
        if timeline.track(id)?.kind != TrackKind::Audio {
            return None;
        }
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
            AutomationTarget::Volume | AutomationTarget::Pan
        )
    {
        return None;
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
        FxTarget::Master => 0.0,
    };
    edit(timeline, target, |list| {
        if list.is_empty() {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
