//! Geometric & Bézier Event Masks.
//!
//! Event masks restrict visibility of an event to a specified geometric shape (rectangle,
//! ellipse, diamond, or smooth Bézier spline) with keyframed animation, opacity, inversion,
//! and directional feathering (In, Out, Both).

use crate::fx_mask::FxMask;
use serde::{Deserialize, Serialize};

/// Feathering style for event mask edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MaskFeatherType {
    #[default]
    None,
    In,
    Out,
    Both,
}

/// A keyframe in an event mask's animation track.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventMaskKey {
    /// Timestamp in seconds relative to event start.
    pub time: f64,
    /// Mask shape, geometry, and base feather.
    pub mask: FxMask,
    /// Overall mask opacity (`0.0..=1.0`).
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    /// Edge feathering behavior.
    #[serde(default)]
    pub feather_type: MaskFeatherType,
}

fn default_opacity() -> f32 {
    1.0
}

/// Keyframed geometric and Bézier event mask applied to a timeline event.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct EventMask {
    /// Whether the event mask is active.
    #[serde(default)]
    pub enabled: bool,
    /// Invert mask coverage (negative mask: reveal outside, hide inside).
    #[serde(default)]
    pub invert: bool,
    /// Keyframes defining mask shape and properties over time.
    #[serde(default)]
    pub keys: Vec<EventMaskKey>,
}

impl EventMask {
    pub fn is_default(&self) -> bool {
        !self.enabled && self.keys.is_empty() && !self.invert
    }

    /// Evaluates the interpolated mask shape, opacity, and feather type at `time_s` from event start.
    pub fn mask_at(&self, time_s: f64) -> Option<(FxMask, f32, MaskFeatherType)> {
        if !self.enabled || self.keys.is_empty() {
            return None;
        }

        if self.keys.len() == 1 || time_s <= self.keys[0].time {
            let k = &self.keys[0];
            return Some((k.mask.clone(), k.opacity, k.feather_type));
        }

        if time_s >= self.keys.last().unwrap().time {
            let k = self.keys.last().unwrap();
            return Some((k.mask.clone(), k.opacity, k.feather_type));
        }

        for i in 0..self.keys.len() - 1 {
            let k0 = &self.keys[i];
            let k1 = &self.keys[i + 1];
            if time_s >= k0.time && time_s <= k1.time {
                let span = (k1.time - k0.time).max(1e-6);
                let t = ((time_s - k0.time) / span) as f32;
                let opacity = k0.opacity + (k1.opacity - k0.opacity) * t;

                let mut mask = k0.mask.clone();
                mask.center[0] = k0.mask.center[0] + (k1.mask.center[0] - k0.mask.center[0]) * t;
                mask.center[1] = k0.mask.center[1] + (k1.mask.center[1] - k0.mask.center[1]) * t;
                mask.size[0] = k0.mask.size[0] + (k1.mask.size[0] - k0.mask.size[0]) * t;
                mask.size[1] = k0.mask.size[1] + (k1.mask.size[1] - k0.mask.size[1]) * t;
                mask.rotation_deg =
                    k0.mask.rotation_deg + (k1.mask.rotation_deg - k0.mask.rotation_deg) * t;
                mask.feather = k0.mask.feather + (k1.mask.feather - k0.mask.feather) * t;

                if k0.mask.points.len() == k1.mask.points.len() && !k0.mask.points.is_empty() {
                    for (p_out, (p0, p1)) in mask
                        .points
                        .iter_mut()
                        .zip(k0.mask.points.iter().zip(k1.mask.points.iter()))
                    {
                        p_out[0] = p0[0] + (p1[0] - p0[0]) * t;
                        p_out[1] = p0[1] + (p1[1] - p0[1]) * t;
                    }
                }

                return Some((mask, opacity, k0.feather_type));
            }
        }

        let k = &self.keys[0];
        Some((k.mask.clone(), k.opacity, k.feather_type))
    }
}
