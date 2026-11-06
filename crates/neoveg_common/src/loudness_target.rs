//! The loudness the mixdown is normalised to when a project is rendered (*Alvo de loudness na
//! renderização*): the programme is measured (ITU-R BS.1770-4) and given one gain so its integrated
//! loudness lands on the target, unless that would push the true peak over the ceiling.

use serde::{Deserialize, Serialize};

/// The true-peak ceiling (dBTP) the normalising gain respects, as broadcast and streaming asks.
pub const TRUE_PEAK_CEILING_DBTP: f64 = -1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LoudnessTarget {
    /// The mixdown is rendered as it is.
    #[default]
    Off,
    /// EBU R128 broadcast: -23 LUFS.
    EbuR128,
    /// Streaming services: -14 LUFS.
    Streaming,
    /// Podcasts: -16 LUFS.
    Podcast,
}

impl LoudnessTarget {
    pub const ALL: [LoudnessTarget; 4] = [
        LoudnessTarget::Off,
        LoudnessTarget::EbuR128,
        LoudnessTarget::Streaming,
        LoudnessTarget::Podcast,
    ];

    /// The integrated loudness aimed at, in LUFS; `None` when off.
    pub fn lufs(self) -> Option<f64> {
        match self {
            LoudnessTarget::Off => None,
            LoudnessTarget::EbuR128 => Some(-23.0),
            LoudnessTarget::Streaming => Some(-14.0),
            LoudnessTarget::Podcast => Some(-16.0),
        }
    }

    /// Label in the default language (pt-BR).
    pub fn label(self) -> &'static str {
        use crate::i18n::tr;
        match self {
            LoudnessTarget::Off => tr("Sem normalização"),
            LoudnessTarget::EbuR128 => tr("EBU R128 (-23 LUFS)"),
            LoudnessTarget::Streaming => tr("Streaming (-14 LUFS)"),
            LoudnessTarget::Podcast => tr("Podcast (-16 LUFS)"),
        }
    }

    /// The gain in dB that takes a programme measured at `measured_lufs` with `true_peak_dbtp` to
    /// the target, kept so the true peak stays at or under [`TRUE_PEAK_CEILING_DBTP`]. `None` when
    /// off or when the programme cannot be measured (silence).
    pub fn gain_db(self, measured_lufs: f64, true_peak_dbtp: f64) -> Option<f64> {
        let target = self.lufs()?;
        if !measured_lufs.is_finite() {
            return None;
        }
        let wanted = target - measured_lufs;
        if true_peak_dbtp.is_finite() {
            Some(wanted.min(TRUE_PEAK_CEILING_DBTP - true_peak_dbtp))
        } else {
            Some(wanted)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_targets_are_the_usual_ones() {
        assert_eq!(LoudnessTarget::Off.lufs(), None);
        assert_eq!(LoudnessTarget::EbuR128.lufs(), Some(-23.0));
        assert_eq!(LoudnessTarget::Streaming.lufs(), Some(-14.0));
        assert_eq!(LoudnessTarget::Podcast.lufs(), Some(-16.0));
    }

    #[test]
    fn the_gain_reaches_the_target_unless_the_peak_would_pass_the_ceiling() {
        // A quiet programme is lifted to -16
        assert_eq!(LoudnessTarget::Podcast.gain_db(-24.0, -12.0), Some(8.0));
        // A loud one is brought down to -23
        assert_eq!(LoudnessTarget::EbuR128.gain_db(-14.0, -3.0), Some(-9.0));
        // Lifting -20 to -14 would put a -4 dBTP peak at +2: held back to the -1 ceiling
        assert_eq!(LoudnessTarget::Streaming.gain_db(-20.0, -4.0), Some(3.0));
        assert_eq!(LoudnessTarget::Off.gain_db(-20.0, -4.0), None);
        assert_eq!(
            LoudnessTarget::Podcast.gain_db(f64::NEG_INFINITY, f64::NEG_INFINITY),
            None
        );
    }
}
