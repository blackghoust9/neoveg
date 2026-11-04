//! Pan laws: how a stereo track's pan control turns into left / right gains.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PanLaw {
    /// Balance: the center is untouched and panning only turns the far channel down.
    #[default]
    Balance,
    /// Constant power: moving a source across keeps its loudness (the center is unity, the near
    /// channel gains up to 3 dB before it is capped at unity).
    ConstantPower,
    /// Linear crossfade between the channels; the center is 6 dB down on each side.
    Linear,
}

impl PanLaw {
    pub const ALL: [PanLaw; 3] = [PanLaw::Balance, PanLaw::ConstantPower, PanLaw::Linear];

    pub fn label(self) -> &'static str {
        match self {
            PanLaw::Balance => "Balanço",
            PanLaw::ConstantPower => "Potência constante",
            PanLaw::Linear => "Linear (−6 dB no centro)",
        }
    }

    /// Left and right gains for `pan` in `-1.0` (hard left) to `1.0` (hard right); values outside
    /// the range are clamped.
    pub fn gains(self, pan: f32) -> (f32, f32) {
        let p = if pan.is_finite() {
            pan.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        match self {
            PanLaw::Balance => ((1.0 - p).min(1.0), (1.0 + p).min(1.0)),
            PanLaw::ConstantPower => {
                let angle = (p + 1.0) * std::f32::consts::FRAC_PI_4;
                (
                    (std::f32::consts::SQRT_2 * angle.cos()).clamp(0.0, 1.0),
                    (std::f32::consts::SQRT_2 * angle.sin()).clamp(0.0, 1.0),
                )
            }
            PanLaw::Linear => ((1.0 - p) / 2.0, (1.0 + p) / 2.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: (f32, f32), b: (f32, f32)) -> bool {
        (a.0 - b.0).abs() < 1e-5 && (a.1 - b.1).abs() < 1e-5
    }

    #[test]
    fn the_center_is_unity_except_for_the_linear_law() {
        assert!(close(PanLaw::Balance.gains(0.0), (1.0, 1.0)));
        assert!(close(PanLaw::ConstantPower.gains(0.0), (1.0, 1.0)));
        assert!(close(PanLaw::Linear.gains(0.0), (0.5, 0.5)));
    }

    #[test]
    fn hard_pan_silences_the_far_channel_in_every_law() {
        for law in PanLaw::ALL {
            let (l, r) = law.gains(1.0);
            assert!(l.abs() < 1e-5 && (r - 1.0).abs() < 1e-5, "{law:?}");
            let (l, r) = law.gains(-1.0);
            assert!(r.abs() < 1e-5 && (l - 1.0).abs() < 1e-5, "{law:?}");
        }
    }

    #[test]
    fn balance_only_turns_the_far_channel_down() {
        assert!(close(PanLaw::Balance.gains(0.5), (0.5, 1.0)));
        assert!(close(PanLaw::Balance.gains(-0.25), (1.0, 0.75)));
    }

    #[test]
    fn gains_are_mirrored_and_never_exceed_unity() {
        for law in PanLaw::ALL {
            for step in -20..=20 {
                let p = step as f32 / 20.0;
                let (l, r) = law.gains(p);
                let (ml, mr) = law.gains(-p);
                assert!(
                    (l - mr).abs() < 1e-5 && (r - ml).abs() < 1e-5,
                    "{law:?} {p}"
                );
                assert!((0.0..=1.0 + 1e-6).contains(&l) && (0.0..=1.0 + 1e-6).contains(&r));
            }
        }
    }

    #[test]
    fn bad_values_do_not_panic() {
        assert!(close(PanLaw::Balance.gains(f32::NAN), (1.0, 1.0)));
        assert!(close(
            PanLaw::Balance.gains(7.0),
            PanLaw::Balance.gains(1.0)
        ));
    }
}
