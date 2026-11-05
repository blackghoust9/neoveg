//! Non-destructive channel conversion of a stereo audio event (Edit > Channels).
//!
//! The source file is never touched: the mode only changes how its two channels reach the mix.
//! A "mono" mode puts the same signal on both sides (the track pan then places it, as for any mono
//! event).

use crate::i18n::{tr, tr_args};
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
            ChannelMode::Both => tr("Ambos (estéreo normal)"),
            ChannelMode::Left => tr("Somente esquerdo"),
            ChannelMode::Right => tr("Somente direito"),
            ChannelMode::Combine => tr("Combinar (mono)"),
            ChannelMode::Swap => tr("Trocar esquerdo e direito"),
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

/// Which sound of a media file an event plays: the audio stream (a file can hold several) and the
/// two channels taken from it, as zero-based numbers. `left == right` is a single channel, heard on
/// both sides. The default is the first stream, channels 1 and 2 (what the importer keeps).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SoundSource {
    #[serde(default)]
    pub stream: u8,
    #[serde(default)]
    pub left: u8,
    #[serde(default = "one")]
    pub right: u8,
}

fn one() -> u8 {
    1
}

impl Default for SoundSource {
    fn default() -> Self {
        Self {
            stream: 0,
            left: 0,
            right: 1,
        }
    }
}

impl SoundSource {
    /// The first stream with its first two channels.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// The source with the channels inside what a stream of `channels` channels has (one channel is
    /// heard on both sides).
    pub fn sanitized(self, channels: u16) -> Self {
        let last = u8::try_from(channels.max(1) - 1).unwrap_or(u8::MAX);
        Self {
            stream: self.stream,
            left: self.left.min(last),
            right: self.right.min(last),
        }
    }

    /// The choices a stream of `channels` channels offers: the pairs 1/2, 3/4 ... (an odd last
    /// channel alone), then each channel on its own. One channel has just itself.
    pub fn options(stream: u8, channels: u16) -> Vec<SoundSource> {
        let n = u8::try_from(channels.min(64)).unwrap_or(64);
        let mut list = Vec::new();
        if n >= 2 {
            let mut first = 0;
            while first < n {
                let right = if first + 1 < n { first + 1 } else { first };
                list.push(SoundSource {
                    stream,
                    left: first,
                    right,
                });
                first += 2;
            }
        }
        if n != 2 {
            list.extend((0..n.max(1)).map(|c| SoundSource {
                stream,
                left: c,
                right: c,
            }));
        }
        list
    }

    /// Name in the menu: `Canais 3/4`, `Canal 2`, with the stream when it is not the first one.
    pub fn label(&self) -> String {
        let channels = if self.left == self.right {
            tr_args("Canal {}", &[&(u32::from(self.left) + 1)])
        } else {
            tr_args(
                "Canais {}/{}",
                &[&(u32::from(self.left) + 1), &(u32::from(self.right) + 1)],
            )
        };
        if self.stream == 0 {
            channels
        } else {
            tr_args("Fluxo {}, {}", &[&(u32::from(self.stream) + 1), &channels])
        }
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

    #[test]
    fn a_multichannel_stream_offers_pairs_then_single_channels() {
        let one = |l: u8, r: u8| SoundSource {
            stream: 0,
            left: l,
            right: r,
        };
        assert_eq!(
            SoundSource::options(0, 2),
            vec![one(0, 1)],
            "stereo has nothing else to pick"
        );
        assert_eq!(SoundSource::options(0, 1), vec![one(0, 0)]);
        assert_eq!(
            SoundSource::options(0, 6),
            vec![
                one(0, 1),
                one(2, 3),
                one(4, 5),
                one(0, 0),
                one(1, 1),
                one(2, 2),
                one(3, 3),
                one(4, 4),
                one(5, 5),
            ]
        );
        // An odd count leaves the last channel alone in its pair
        assert_eq!(
            SoundSource::options(1, 3)[1],
            SoundSource {
                stream: 1,
                left: 2,
                right: 2
            }
        );
    }

    #[test]
    fn the_default_source_is_the_first_pair_and_a_pick_is_kept_inside_the_stream() {
        assert!(SoundSource::default().is_default());
        let pick = SoundSource {
            stream: 1,
            left: 4,
            right: 5,
        };
        assert!(!pick.is_default());
        assert_eq!(
            pick.sanitized(2),
            SoundSource {
                stream: 1,
                left: 1,
                right: 1
            }
        );
        assert_eq!(pick.sanitized(6), pick);
        assert_eq!(
            SoundSource::default().sanitized(1),
            SoundSource {
                stream: 0,
                left: 0,
                right: 0
            }
        );
        // Saved projects that lack the fields still load, and a plain choice round-trips
        let old: SoundSource = serde_json::from_str("{}").unwrap();
        assert!(old.is_default());
        let json = serde_json::to_string(&pick).unwrap();
        assert_eq!(serde_json::from_str::<SoundSource>(&json).unwrap(), pick);
    }
}
