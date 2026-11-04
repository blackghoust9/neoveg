//! Primary color correction of the video: exposure, color temperature and tint, lift, contrast,
//! gamma and saturation. Pure per-color math shared by the project file, the UI and the video
//! worker.

use serde::{Deserialize, Serialize};

/// The controls of the correction; every field is neutral at its default.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Grade {
    /// Exposure in stops (`2^x`), `-4.0..=4.0`.
    pub exposure: f32,
    /// Color temperature, cool (`-1`) to warm (`1`).
    pub temperature: f32,
    /// Tint, green (`-1`) to magenta (`1`).
    pub tint: f32,
    /// Raises (or lowers) the blacks, `-0.5..=0.5`.
    pub lift: f32,
    /// Contrast around mid grey, `-1.0..=1.0`.
    pub contrast: f32,
    /// Mid-tone gamma, `0.2..=3.0` (1 = none; above 1 brightens the mids).
    pub gamma: f32,
    /// Saturation, `-1.0` (grey) to `1.0` (twice as colorful).
    pub saturation: f32,
}

impl Default for Grade {
    fn default() -> Self {
        Self {
            exposure: 0.0,
            temperature: 0.0,
            tint: 0.0,
            lift: 0.0,
            contrast: 0.0,
            gamma: 1.0,
            saturation: 0.0,
        }
    }
}

/// Rec. 709 luma weights.
fn luma([r, g, b]: [f32; 3]) -> f32 {
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

impl Grade {
    pub const EXPOSURE_RANGE: (f32, f32) = (-4.0, 4.0);
    pub const LIFT_RANGE: (f32, f32) = (-0.5, 0.5);
    pub const GAMMA_RANGE: (f32, f32) = (0.2, 3.0);

    pub fn is_neutral(&self) -> bool {
        *self == Self::default()
    }

    /// Every control clamped to its range; NaN becomes neutral.
    pub fn sanitized(self) -> Self {
        let clamp = |v: f32, lo: f32, hi: f32, neutral: f32| {
            if v.is_finite() {
                v.clamp(lo, hi)
            } else {
                neutral
            }
        };
        Self {
            exposure: clamp(
                self.exposure,
                Self::EXPOSURE_RANGE.0,
                Self::EXPOSURE_RANGE.1,
                0.0,
            ),
            temperature: clamp(self.temperature, -1.0, 1.0, 0.0),
            tint: clamp(self.tint, -1.0, 1.0, 0.0),
            lift: clamp(self.lift, Self::LIFT_RANGE.0, Self::LIFT_RANGE.1, 0.0),
            contrast: clamp(self.contrast, -1.0, 1.0, 0.0),
            gamma: clamp(self.gamma, Self::GAMMA_RANGE.0, Self::GAMMA_RANGE.1, 1.0),
            saturation: clamp(self.saturation, -1.0, 1.0, 0.0),
        }
    }

    /// Corrects one color (components `0.0..=1.0`); the result is clamped to the same range.
    pub fn apply(&self, rgb: [f32; 3]) -> [f32; 3] {
        let gain = 2f32.powf(self.exposure);
        let warm = [
            1.0 + 0.25 * self.temperature,
            1.0 - 0.25 * self.tint,
            1.0 - 0.25 * self.temperature,
        ];
        let inv_gamma = 1.0 / self.gamma.max(0.01);
        let mut out = [0.0f32; 3];
        for c in 0..3 {
            let mut v = rgb[c] * gain * warm[c];
            v += self.lift * (1.0 - v.clamp(0.0, 1.0));
            v = (v - 0.5) * (1.0 + self.contrast) + 0.5;
            out[c] = v.clamp(0.0, 1.0).powf(inv_gamma);
        }
        let y = luma(out);
        let k = 1.0 + self.saturation;
        for v in &mut out {
            *v = (y + (*v - y) * k).clamp(0.0, 1.0);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-4)
    }

    #[test]
    fn the_default_grade_changes_nothing() {
        let g = Grade::default();
        assert!(g.is_neutral());
        for rgb in [
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.2, 0.5, 0.8],
            [0.9, 0.1, 0.4],
        ] {
            assert!(close(g.apply(rgb), rgb), "{rgb:?}");
        }
    }

    #[test]
    fn exposure_doubles_per_stop_and_clips_at_white() {
        let g = Grade {
            exposure: 1.0,
            ..Grade::default()
        };
        assert!(close(g.apply([0.2, 0.25, 0.4]), [0.4, 0.5, 0.8]));
        assert!(close(g.apply([0.8, 0.8, 0.8]), [1.0, 1.0, 1.0]));
        let dark = Grade {
            exposure: -1.0,
            ..Grade::default()
        };
        assert!(close(dark.apply([0.8, 0.4, 0.2]), [0.4, 0.2, 0.1]));
    }

    #[test]
    fn saturation_minus_one_is_grey_and_plus_one_doubles_the_chroma() {
        let grey = Grade {
            saturation: -1.0,
            ..Grade::default()
        }
        .apply([0.8, 0.2, 0.2]);
        assert!((grey[0] - grey[1]).abs() < 1e-5 && (grey[1] - grey[2]).abs() < 1e-5);
        let more = Grade {
            saturation: 0.5,
            ..Grade::default()
        }
        .apply([0.6, 0.4, 0.4]);
        assert!(more[0] - more[1] > 0.29, "{more:?}");
    }

    #[test]
    fn contrast_spreads_around_mid_grey_and_gamma_lifts_the_mids() {
        let c = Grade {
            contrast: 0.5,
            ..Grade::default()
        };
        assert!(close(c.apply([0.5, 0.5, 0.5]), [0.5, 0.5, 0.5]));
        assert!(c.apply([0.7, 0.7, 0.7])[0] > 0.7 && c.apply([0.3, 0.3, 0.3])[0] < 0.3);
        let g = Grade {
            gamma: 2.0,
            ..Grade::default()
        };
        assert!(close(g.apply([0.25, 0.25, 0.25]), [0.5, 0.5, 0.5]));
        assert!(close(g.apply([0.0, 0.0, 0.0]), [0.0, 0.0, 0.0]));
        assert!(close(g.apply([1.0, 1.0, 1.0]), [1.0, 1.0, 1.0]));
    }

    #[test]
    fn lift_raises_only_the_blacks_and_temperature_warms() {
        let l = Grade {
            lift: 0.2,
            ..Grade::default()
        };
        assert!(close(l.apply([0.0, 0.0, 0.0]), [0.2, 0.2, 0.2]));
        assert!(close(l.apply([1.0, 1.0, 1.0]), [1.0, 1.0, 1.0]));
        let warm = Grade {
            temperature: 1.0,
            ..Grade::default()
        }
        .apply([0.5, 0.5, 0.5]);
        assert!(warm[0] > 0.5 && warm[2] < 0.5);
        let magenta = Grade {
            tint: 1.0,
            ..Grade::default()
        }
        .apply([0.5, 0.5, 0.5]);
        assert!(magenta[1] < 0.5);
    }

    #[test]
    fn bad_values_are_clamped_and_old_files_load() {
        let g = Grade {
            exposure: 99.0,
            gamma: f32::NAN,
            saturation: -7.0,
            ..Grade::default()
        }
        .sanitized();
        assert_eq!((g.exposure, g.gamma, g.saturation), (4.0, 1.0, -1.0));
        let old: Grade = serde_json::from_str("{\"exposure\":0.5}").unwrap();
        assert_eq!(old.exposure, 0.5);
        assert_eq!(old.gamma, 1.0);
        let none: Grade = serde_json::from_str("{}").unwrap();
        assert!(none.is_neutral());
        let json = serde_json::to_string(&g).unwrap();
        assert_eq!(serde_json::from_str::<Grade>(&json).unwrap(), g);
    }
}
