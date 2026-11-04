//! Generic keyframed envelope curves, point-thinning/inversion helpers, and track automation modes.
//!
//! Shared between [`neoveg_engine`] (editing & serialization), `neoveg_audio` (zero-allocation
//! real-time mixer evaluation), and `neoveg_ui` (video compositor and curve editors).

use crate::fade::FadeCurve;
use crate::i18n::tr;
use serde::{Deserialize, Serialize};

/// Smallest distinct gap between two envelope timestamps (1 microsecond).
pub const ENVELOPE_TIME_EPSILON: f64 = 1e-6;

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
    #[inline]
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

    /// Human-readable curve name.
    pub fn label(self) -> &'static str {
        match self {
            EnvelopeShape::Linear => tr("Linear"),
            EnvelopeShape::Fast => tr("Rápida"),
            EnvelopeShape::Slow => tr("Lenta"),
            EnvelopeShape::Smooth => tr("Suave"),
            EnvelopeShape::Sharp => tr("Acentuada"),
            EnvelopeShape::Hold => tr("Degrau (manter)"),
        }
    }
}

/// One automation or parameter keyframe on an envelope.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EnvelopePoint {
    pub time: f64,
    pub value: f32,
    #[serde(default)]
    pub shape: EnvelopeShape,
}

/// Generic keyframed curve evaluated by both the audio mixer and the video compositor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Envelope {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub points: Vec<EnvelopePoint>,
}

impl Default for Envelope {
    fn default() -> Self {
        Self {
            enabled: true,
            points: Vec::new(),
        }
    }
}

impl Envelope {
    pub fn new(initial_value: f32, shape: EnvelopeShape) -> Self {
        Self {
            enabled: true,
            points: vec![EnvelopePoint {
                time: 0.0,
                value: initial_value,
                shape,
            }],
        }
    }

    pub fn from_points(mut points: Vec<EnvelopePoint>) -> Self {
        sort_points(&mut points);
        Self {
            enabled: true,
            points,
        }
    }

    #[inline]
    pub fn is_active(&self) -> bool {
        self.enabled && !self.points.is_empty()
    }

    #[inline]
    pub fn sample(&self, time: f64) -> Option<f32> {
        if !self.enabled {
            return None;
        }
        eval_points_opt(&self.points, time)
    }

    #[inline]
    pub fn value_at(&self, time: f64, fallback: f32) -> f32 {
        self.sample(time).unwrap_or(fallback)
    }
}

/// Per-track automation mode controlling whether automation envelopes are read or recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AutomationMode {
    /// Automation is bypassed on this track; static controls apply and curves are dimmed.
    Off,
    /// Envelopes are evaluated during playback and scrubbing.
    #[default]
    Read,
    /// Records points while a control is actively dragged during playback; stops writing on release.
    Touch,
    /// Starts recording when a control is touched during playback and continues writing until stop.
    Latch,
}

impl AutomationMode {
    pub const ALL: [AutomationMode; 4] = [
        AutomationMode::Off,
        AutomationMode::Read,
        AutomationMode::Touch,
        AutomationMode::Latch,
    ];

    pub fn is_default(&self) -> bool {
        *self == AutomationMode::Read
    }

    pub fn reads_envelopes(self) -> bool {
        self != AutomationMode::Off
    }

    pub fn is_writing(self) -> bool {
        matches!(self, AutomationMode::Touch | AutomationMode::Latch)
    }

    pub fn records(self) -> bool {
        self.is_writing()
    }

    pub fn label(self) -> &'static str {
        match self {
            AutomationMode::Off => tr("Automação desligada"),
            AutomationMode::Read => tr("Automação: Leitura"),
            AutomationMode::Touch => tr("Automação: Gravar (Toque)"),
            AutomationMode::Latch => tr("Automação: Gravar (Trava)"),
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            AutomationMode::Off => tr("Desl."),
            AutomationMode::Read => tr("Ler"),
            AutomationMode::Touch => tr("Toque"),
            AutomationMode::Latch => tr("Trava"),
        }
    }
}

/// Sorts `points` in non-decreasing timestamp order.
pub fn sort_points(points: &mut [EnvelopePoint]) {
    points.sort_by(|a, b| {
        a.time
            .partial_cmp(&b.time)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

/// Evaluates sorted `points` at `time`, returning `None` when `points` is empty.
#[inline]
pub fn eval_points_opt(points: &[EnvelopePoint], time: f64) -> Option<f32> {
    let (first, last) = (points.first()?, points.last()?);
    if time <= first.time {
        return Some(first.value);
    }
    if time >= last.time {
        return Some(last.value);
    }
    let idx = points.partition_point(|p| p.time <= time);
    if idx == 0 {
        return Some(first.value);
    }
    if idx >= points.len() {
        return Some(last.value);
    }
    let a = points[idx - 1];
    let b = points[idx];
    let span = b.time - a.time;
    if span <= ENVELOPE_TIME_EPSILON {
        return Some(b.value);
    }
    let u = a.shape.eval((time - a.time) / span) as f32;
    Some(a.value + (b.value - a.value) * u)
}

/// Evaluates sorted `points` at `time`, returning `fallback` when `points` is empty.
#[inline]
pub fn eval_points(points: &[EnvelopePoint], time: f64, fallback: f32) -> f32 {
    eval_points_opt(points, time).unwrap_or(fallback)
}

/// Reflects `points` around `center`, clamped to `min..=max`. When `range` is `Some((lo, hi))`,
/// only points inside `[lo, hi]` are reflected.
pub fn flip_points(
    points: &[EnvelopePoint],
    range: Option<(f64, f64)>,
    center: f32,
    min: f32,
    max: f32,
) -> Vec<EnvelopePoint> {
    let norm_range = range.map(|(a, b)| if a <= b { (a, b) } else { (b, a) });
    points
        .iter()
        .map(|p| {
            let in_range = match norm_range {
                None => true,
                Some((lo, hi)) => {
                    p.time >= lo - ENVELOPE_TIME_EPSILON && p.time <= hi + ENVELOPE_TIME_EPSILON
                }
            };
            if in_range {
                EnvelopePoint {
                    value: (2.0 * center - p.value).clamp(min, max),
                    ..*p
                }
            } else {
                *p
            }
        })
        .collect()
}

/// Simplifies redundant interior points whose omission changes the evaluated curve by at most
/// `value_tol`. When `range` is `Some((lo, hi))`, points outside `[lo, hi]` are preserved verbatim.
pub fn thin_points(
    points: &[EnvelopePoint],
    range: Option<(f64, f64)>,
    value_tol: f32,
) -> Vec<EnvelopePoint> {
    if points.len() <= 2 {
        return points.to_vec();
    }
    let tol = value_tol.max(0.0);
    match range {
        None => thin_slice(points, tol),
        Some((a, b)) => {
            let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
            let start = points.partition_point(|p| p.time < lo - ENVELOPE_TIME_EPSILON);
            let end = points.partition_point(|p| p.time <= hi + ENVELOPE_TIME_EPSILON);
            if end <= start + 2 {
                return points.to_vec();
            }
            let mut out = Vec::with_capacity(points.len());
            out.extend_from_slice(&points[..start]);
            out.extend(thin_slice(&points[start..end], tol));
            out.extend_from_slice(&points[end..]);
            out
        }
    }
}

fn thin_slice(sub: &[EnvelopePoint], tol: f32) -> Vec<EnvelopePoint> {
    if sub.len() <= 2 {
        return sub.to_vec();
    }
    let first = sub[0];
    let last = sub[sub.len() - 1];
    let chord = [first, last];

    let mut max_err = 0.0f32;
    let mut split_idx = 1usize;

    for (i, pt) in sub.iter().enumerate().take(sub.len() - 1).skip(1) {
        let approx = eval_points(&chord, pt.time, first.value);
        let err = (pt.value - approx).abs();
        if err > max_err {
            max_err = err;
            split_idx = i;
        }
    }
    // Also check segment midpoints so non-linear or Hold segments are never wrongly collapsed.
    for i in 0..(sub.len() - 1) {
        let mid_t = 0.5 * (sub[i].time + sub[i + 1].time);
        let orig = eval_points(sub, mid_t, first.value);
        let approx = eval_points(&chord, mid_t, first.value);
        let err = (orig - approx).abs();
        if err > max_err {
            max_err = err;
            split_idx = if i == 0 { 1 } else { i };
        }
    }

    if max_err <= tol {
        vec![first, last]
    } else {
        let mut left = thin_slice(&sub[..=split_idx], tol);
        let right = thin_slice(&sub[split_idx..], tol);
        left.pop();
        left.extend(right);
        left
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_six_shapes_respect_endpoints_and_monotonicity() {
        for shape in EnvelopeShape::ALL {
            assert!((shape.eval(0.0) - 0.0).abs() < 1e-6);
            assert!((shape.eval(1.0) - 1.0).abs() < 1e-6);
            assert!(!shape.label().is_empty());
            let a = shape.eval(0.25);
            let b = shape.eval(0.75);
            assert!(b >= a, "{shape:?}: {b} < {a}");
        }
        assert_eq!(EnvelopeShape::Hold.eval(0.0), 0.0);
        assert_eq!(EnvelopeShape::Hold.eval(0.999), 0.0);
        assert_eq!(EnvelopeShape::Hold.eval(1.0), 1.0);
    }

    #[test]
    fn generic_envelope_evaluates_and_respects_enabled_flag() {
        let mut env = Envelope::from_points(vec![
            EnvelopePoint {
                time: 2.0,
                value: 1.0,
                shape: EnvelopeShape::Linear,
            },
            EnvelopePoint {
                time: 0.0,
                value: 0.0,
                shape: EnvelopeShape::Linear,
            },
        ]);
        assert!(env.is_active());
        assert!((env.value_at(1.0, -5.0) - 0.5).abs() < 1e-5);
        env.enabled = false;
        assert!(!env.is_active());
        assert_eq!(env.sample(1.0), None);
        assert_eq!(env.value_at(1.0, -5.0), -5.0);
    }

    #[test]
    fn automation_mode_defaults_to_read_and_round_trips() {
        assert_eq!(AutomationMode::default(), AutomationMode::Read);
        assert!(AutomationMode::Read.is_default());
        assert!(!AutomationMode::Off.reads_envelopes());
        assert!(AutomationMode::Read.reads_envelopes());
        assert!(!AutomationMode::Read.is_writing());
        assert!(AutomationMode::Touch.is_writing());
        assert!(AutomationMode::Latch.is_writing());
        for m in AutomationMode::ALL {
            let json = serde_json::to_string(&m).unwrap();
            let back: AutomationMode = serde_json::from_str(&json).unwrap();
            assert_eq!(back, m);
            assert!(!m.label().is_empty());
            assert!(!m.short_label().is_empty());
        }
    }

    #[test]
    fn thin_points_drops_collinear_samples_and_keeps_corners_and_hold_steps() {
        let dense: Vec<EnvelopePoint> = (0..=20)
            .map(|i| {
                let t = i as f64 * 0.1;
                let v = if t <= 1.0 { t as f32 } else { (2.0 - t) as f32 };
                EnvelopePoint {
                    time: t,
                    value: v,
                    shape: EnvelopeShape::Linear,
                }
            })
            .collect();
        let thinned = thin_points(&dense, None, 0.01);
        assert_eq!(thinned.len(), 3);
        assert!((thinned[0].time - 0.0).abs() < 1e-6);
        assert!((thinned[1].time - 1.0).abs() < 1e-6);
        assert!((thinned[2].time - 2.0).abs() < 1e-6);

        // Range-restricted thinning leaves points outside the range untouched.
        let partial = thin_points(&dense, Some((0.0, 1.0)), 0.01);
        assert!(partial.len() < dense.len());
        assert_eq!(partial.iter().filter(|p| p.time > 1.05).count(), 10);

        // Hold steps with distinct values are preserved.
        let steps = vec![
            EnvelopePoint {
                time: 0.0,
                value: 0.0,
                shape: EnvelopeShape::Hold,
            },
            EnvelopePoint {
                time: 1.0,
                value: 1.0,
                shape: EnvelopeShape::Hold,
            },
            EnvelopePoint {
                time: 2.0,
                value: 0.0,
                shape: EnvelopeShape::Hold,
            },
        ];
        assert_eq!(thin_points(&steps, None, 0.05).len(), 3);
    }

    #[test]
    fn flip_points_reflects_around_center_and_clamps_to_bounds() {
        let pts = vec![
            EnvelopePoint {
                time: 0.0,
                value: 6.0,
                shape: EnvelopeShape::Linear,
            },
            EnvelopePoint {
                time: 1.0,
                value: -20.0,
                shape: EnvelopeShape::Smooth,
            },
            EnvelopePoint {
                time: 2.0,
                value: -3.0,
                shape: EnvelopeShape::Linear,
            },
        ];
        // Volume bounds [-60, +12], center 0 dB: -20 -> +20 clamped to +12
        let all = flip_points(&pts, None, 0.0, -60.0, 12.0);
        assert_eq!(all[0].value, -6.0);
        assert_eq!(all[1].value, 12.0);
        assert_eq!(all[2].value, 3.0);

        let sub = flip_points(&pts, Some((0.5, 1.5)), 0.0, -60.0, 12.0);
        assert_eq!(sub[0].value, 6.0);
        assert_eq!(sub[1].value, 12.0);
        assert_eq!(sub[2].value, -3.0);
    }
}
