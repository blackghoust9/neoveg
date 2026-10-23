//! The five fade curves shared by the engine (stored on events) and the mixer (applied to audio).
//!
//! `gain(x)` maps fade progress `x` in `[0, 1]` to a gain in `[0, 1]`: a fade-in uses it directly, a
//! fade-out evaluates it at the remaining fraction, so `gain(0) = 0` and `gain(1) = 1` for every curve.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FadeCurve {
    /// Straight line.
    #[default]
    Linear,
    /// Rises quickly and flattens out (`1 - (1 - x)^2`).
    Fast,
    /// Starts slowly and speeds up (`x^2`).
    Slow,
    /// S-curve, gentle at both ends (smoothstep).
    Smooth,
    /// Inverse S-curve, steep at both ends and flat in the middle.
    Sharp,
}

impl FadeCurve {
    pub const ALL: [FadeCurve; 5] = [
        FadeCurve::Linear,
        FadeCurve::Fast,
        FadeCurve::Slow,
        FadeCurve::Smooth,
        FadeCurve::Sharp,
    ];

    pub fn gain(self, x: f64) -> f64 {
        let x = x.clamp(0.0, 1.0);
        match self {
            FadeCurve::Linear => x,
            FadeCurve::Fast => 1.0 - (1.0 - x) * (1.0 - x),
            FadeCurve::Slow => x * x,
            FadeCurve::Smooth => x * x * (3.0 - 2.0 * x),
            FadeCurve::Sharp => 0.5 - ((1.0 - 2.0 * x).asin() / 3.0).sin(),
        }
    }

    /// Menu label (pt-BR).
    pub fn label(self) -> &'static str {
        match self {
            FadeCurve::Linear => "Linear",
            FadeCurve::Fast => "Rápida",
            FadeCurve::Slow => "Lenta",
            FadeCurve::Smooth => "Suave",
            FadeCurve::Sharp => "Acentuada",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_curve_goes_from_silence_to_full_gain() {
        for curve in FadeCurve::ALL {
            assert!(curve.gain(0.0).abs() < 1e-12, "{curve:?}");
            assert!((curve.gain(1.0) - 1.0).abs() < 1e-12, "{curve:?}");
            // out-of-range progress is clamped
            assert_eq!(curve.gain(-3.0), curve.gain(0.0));
            assert_eq!(curve.gain(7.0), curve.gain(1.0));
        }
    }

    #[test]
    fn every_curve_is_monotonic() {
        for curve in FadeCurve::ALL {
            let mut last = -1.0;
            for i in 0..=1000 {
                let g = curve.gain(f64::from(i) / 1000.0);
                assert!(g >= last - 1e-12, "{curve:?} at {i}");
                last = g;
            }
        }
    }

    #[test]
    fn midpoint_values_distinguish_the_shapes() {
        assert!((FadeCurve::Linear.gain(0.5) - 0.5).abs() < 1e-12);
        assert!((FadeCurve::Fast.gain(0.5) - 0.75).abs() < 1e-12);
        assert!((FadeCurve::Slow.gain(0.5) - 0.25).abs() < 1e-12);
        assert!((FadeCurve::Smooth.gain(0.5) - 0.5).abs() < 1e-12);
        assert!((FadeCurve::Sharp.gain(0.5) - 0.5).abs() < 1e-12);
        // smooth is gentle near the ends, sharp is steep there
        assert!(FadeCurve::Smooth.gain(0.1) < 0.1);
        assert!(FadeCurve::Sharp.gain(0.1) > 0.1);
    }

    #[test]
    fn symmetric_curves_crossfade_to_constant_amplitude() {
        for curve in [FadeCurve::Linear, FadeCurve::Smooth, FadeCurve::Sharp] {
            for i in 0..=20 {
                let x = f64::from(i) / 20.0;
                assert!(
                    (curve.gain(x) + curve.gain(1.0 - x) - 1.0).abs() < 1e-12,
                    "{curve:?}"
                );
            }
        }
    }

    #[test]
    fn default_is_linear_and_survives_serde() {
        assert_eq!(FadeCurve::default(), FadeCurve::Linear);
        let json = serde_json::to_string(&FadeCurve::Sharp).unwrap();
        assert_eq!(
            serde_json::from_str::<FadeCurve>(&json).unwrap(),
            FadeCurve::Sharp
        );
    }
}
