//! Non-destructive channel conversion of a stereo audio event (Edit > Channels).
//!
//! The source file is never touched: the mode only changes how its two channels reach the mix.
//! A "mono" mode puts the same signal on both sides (the track pan then places it, as for any mono
//! event).

use serde::{Deserialize, Serialize};

/// How an event uses the channels of its stereo media.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelMode {
    /// A normal stereo event.
    #[default]
    Both,
    /// A mono event made of the left channel only.
    Left,
    /// A mono event made of the right channel only.
    Right,
    /// A mono event that mixes both channels; the amplitude is halved so the sum cannot clip.
    Combine,
    /// The left and right channels exchanged.
    Swap,
}

impl ChannelMode {
    pub const ALL: [ChannelMode; 5] = [
        ChannelMode::Both,
        ChannelMode::Left,
        ChannelMode::Right,
        ChannelMode::Combine,
        ChannelMode::Swap,
    ];

    /// Label in the default language (pt-BR).
    pub fn label(self) -> &'static str {
        match self {
            ChannelMode::Both => "Ambos (estéreo normal)",
            ChannelMode::Left => "Somente esquerdo",
            ChannelMode::Right => "Somente direito",
            ChannelMode::Combine => "Combinar (mono)",
            ChannelMode::Swap => "Trocar esquerdo e direito",
        }
    }

    /// Short tag for the event title (`None` for a normal stereo event).
    pub fn tag(self) -> Option<&'static str> {
        match self {
            ChannelMode::Both => None,
            ChannelMode::Left => Some("E"),
            ChannelMode::Right => Some("D"),
            ChannelMode::Combine => Some("mono"),
            ChannelMode::Swap => Some("E<>D"),
        }
    }

    /// Applies the mode to one stereo frame.
    #[inline]
    pub fn apply(self, left: f32, right: f32) -> (f32, f32) {
        match self {
            ChannelMode::Both => (left, right),
            ChannelMode::Left => (left, left),
            ChannelMode::Right => (right, right),
            ChannelMode::Combine => {
                let mid = (left + right) * 0.5;
                (mid, mid)
            }
            ChannelMode::Swap => (right, left),
        }
    }

    /// Whether the mode leaves the signal alone (the common case, skipped in the mixer).
    pub fn is_default(&self) -> bool {
        *self == ChannelMode::Both
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mode_maps_a_frame_as_the_manual_describes() {
        let (l, r) = (0.8_f32, -0.2_f32);
        assert_eq!(ChannelMode::Both.apply(l, r), (l, r));
        assert_eq!(ChannelMode::Left.apply(l, r), (l, l));
        assert_eq!(ChannelMode::Right.apply(l, r), (r, r));
        assert_eq!(ChannelMode::Swap.apply(l, r), (r, l));
        let (a, b) = ChannelMode::Combine.apply(l, r);
        assert!((a - 0.3).abs() < 1e-6 && a == b);
    }

    #[test]
    fn combining_two_full_scale_channels_cannot_clip() {
        let (a, b) = ChannelMode::Combine.apply(1.0, 1.0);
        assert_eq!(
            (a, b),
            (1.0, 1.0),
            "halved before the sum, so full scale stays full scale"
        );
    }

    #[test]
    fn modes_round_trip_through_json_and_old_files_default_to_both() {
        for mode in ChannelMode::ALL {
            let text = serde_json::to_string(&mode).unwrap();
            assert_eq!(serde_json::from_str::<ChannelMode>(&text).unwrap(), mode);
        }
        assert!(ChannelMode::default().is_default());
        assert!(ChannelMode::ALL.iter().all(|m| !m.label().is_empty()));
    }
}
