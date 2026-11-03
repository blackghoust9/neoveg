//! The pitch of an audio event, apart from its speed: a change in semitones, whether the formants
//! (the resonances that make a voice sound like itself) stay where they were, and an extra shift of
//! the formants.
//!
//! Without formant preservation a pitch change moves the formants with the pitch (up: the
//! "chipmunk" voice). With it the pitch moves and the timbre stays, or moves by `formant_shift`.

use serde::{Deserialize, Serialize};

/// Largest pitch change, in semitones, either way (two octaves).
pub const MAX_SEMITONES: f64 = 24.0;
/// Largest extra formant shift, in semitones, either way.
pub const MAX_FORMANT_SHIFT: f64 = 12.0;

fn is_zero(value: &f64) -> bool {
    *value == 0.0
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PitchChange {
    /// How far the pitch moves, in semitones (negative: lower); fractions are cents.
    #[serde(skip_serializing_if = "is_zero")]
    pub semitones: f64,
    /// Keep the formants where they were while the pitch moves.
    #[serde(skip_serializing_if = "is_false")]
    pub preserve_formants: bool,
    /// With formants preserved: how far the timbre moves besides, in semitones.
    #[serde(skip_serializing_if = "is_zero")]
    pub formant_shift: f64,
}

impl PitchChange {
    /// `true` when the sound is left as it is.
    pub fn is_default(&self) -> bool {
        self.sanitized() == Self::default()
    }

    /// The same values made safe: finite, within [`MAX_SEMITONES`] and [`MAX_FORMANT_SHIFT`], and a
    /// formant shift that only counts while the formants are preserved.
    pub fn sanitized(&self) -> Self {
        let clean = |value: f64, limit: f64| {
            if value.is_finite() {
                value.clamp(-limit, limit)
            } else {
                0.0
            }
        };
        Self {
            semitones: clean(self.semitones, MAX_SEMITONES),
            preserve_formants: self.preserve_formants,
            formant_shift: if self.preserve_formants {
                clean(self.formant_shift, MAX_FORMANT_SHIFT)
            } else {
                0.0
            },
        }
    }

    /// Frequency ratio of the pitch change (`2.0` for an octave up).
    pub fn pitch_ratio(&self) -> f64 {
        2f64.powf(self.sanitized().semitones / 12.0)
    }

    /// Frequency ratio the formants end up moved by relative to the original sound: `1.0` while they
    /// are preserved and not shifted, the pitch ratio when they are not preserved.
    pub fn formant_ratio(&self) -> f64 {
        let s = self.sanitized();
        if s.preserve_formants {
            2f64.powf(s.formant_shift / 12.0)
        } else {
            self.pitch_ratio()
        }
    }

    /// `true` when the sound differs from the original at all (the flag alone changes nothing).
    pub fn changes_sound(&self) -> bool {
        let s = self.sanitized();
        s.semitones != 0.0 || self.needs_formant_correction()
    }

    /// `true` when the formants have to be put back after the pitch has moved.
    pub fn needs_formant_correction(&self) -> bool {
        let s = self.sanitized();
        s.preserve_formants && (s.semitones != 0.0 || s.formant_shift != 0.0)
    }

    /// A key for caches: every value that changes the sound.
    pub fn key(&self) -> (u64, bool, u64) {
        let s = self.sanitized();
        (
            s.semitones.to_bits(),
            s.preserve_formants,
            s.formant_shift.to_bits(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_leaves_the_sound_alone_and_is_not_written() {
        let change = PitchChange::default();
        assert!(
            change.is_default() && !change.needs_formant_correction() && !change.changes_sound()
        );
        assert_eq!(serde_json::to_string(&change).unwrap(), "{}");
        assert_eq!(change.pitch_ratio(), 1.0);
        assert_eq!(change.formant_ratio(), 1.0);
        // Preserving formants with nothing to change is still nothing to do
        let only_flag = PitchChange {
            preserve_formants: true,
            ..PitchChange::default()
        };
        assert!(!only_flag.needs_formant_correction() && !only_flag.changes_sound());
    }

    #[test]
    fn an_octave_doubles_the_pitch_and_the_formants_follow_unless_they_are_preserved() {
        let up = PitchChange {
            semitones: 12.0,
            ..PitchChange::default()
        };
        assert!((up.pitch_ratio() - 2.0).abs() < 1e-12);
        assert!(
            (up.formant_ratio() - 2.0).abs() < 1e-12,
            "they move with it"
        );
        assert!(!up.needs_formant_correction());
        let kept = PitchChange {
            semitones: 12.0,
            preserve_formants: true,
            ..PitchChange::default()
        };
        assert!((kept.pitch_ratio() - 2.0).abs() < 1e-12);
        assert_eq!(kept.formant_ratio(), 1.0);
        assert!(kept.needs_formant_correction());
        // The extra shift deepens the timbre without moving the pitch
        let deeper = PitchChange {
            preserve_formants: true,
            formant_shift: -7.0,
            ..PitchChange::default()
        };
        assert_eq!(deeper.pitch_ratio(), 1.0);
        assert!((deeper.formant_ratio() - 2f64.powf(-7.0 / 12.0)).abs() < 1e-12);
        assert!(deeper.needs_formant_correction());
    }

    #[test]
    fn odd_values_are_made_safe_and_the_formant_shift_needs_the_preserve_flag() {
        let wild = PitchChange {
            semitones: 90.0,
            preserve_formants: false,
            formant_shift: 5.0,
        }
        .sanitized();
        assert_eq!(wild.semitones, MAX_SEMITONES);
        assert_eq!(wild.formant_shift, 0.0, "no preserve, no shift");
        let nan = PitchChange {
            semitones: f64::NAN,
            preserve_formants: true,
            formant_shift: f64::INFINITY,
        }
        .sanitized();
        assert_eq!((nan.semitones, nan.formant_shift), (0.0, 0.0));
        let json = serde_json::to_string(&PitchChange {
            semitones: -3.0,
            preserve_formants: true,
            formant_shift: 2.0,
        })
        .unwrap();
        assert_eq!(
            serde_json::from_str::<PitchChange>(&json)
                .unwrap()
                .semitones,
            -3.0
        );
        assert_ne!(
            PitchChange {
                semitones: 1.0,
                ..PitchChange::default()
            }
            .key(),
            PitchChange::default().key()
        );
    }
}
