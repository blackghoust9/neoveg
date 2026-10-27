//! Metronome: a click on every beat (an accented one on the first beat of each bar), prepared as
//! ordinary mixer clips so it plays through the same path as the project audio.

use std::sync::Arc;

use crate::mixer::MixClip;

/// Most clicks prepared for one playback; the audio restarts at the cursor, so this is a horizon of
/// about 33 minutes at 120 bpm.
pub const MAX_CLICKS: usize = 4000;
/// Track index the clicks carry: outside any real track, so they use the dry bus.
pub const METRONOME_TRACK: usize = usize::MAX;
const CLICK_SECONDS: f64 = 0.03;

/// Tempo and meter of the click.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metronome {
    pub bpm: f32,
    pub beats_per_bar: u32,
    /// Linear loudness of the click, `0.0..=1.0`.
    pub volume: f32,
}

impl Default for Metronome {
    fn default() -> Self {
        Self {
            bpm: 120.0,
            beats_per_bar: 4,
            volume: 0.5,
        }
    }
}

impl Metronome {
    /// Lowest and highest tempo accepted.
    pub const BPM_RANGE: (f32, f32) = (20.0, 400.0);

    /// Seconds between two beats.
    pub fn beat_seconds(&self) -> f64 {
        60.0 / f64::from(self.bpm.clamp(Self::BPM_RANGE.0, Self::BPM_RANGE.1))
    }

    /// Index of the first beat at or after `time` seconds.
    pub fn first_beat_at_or_after(&self, time: f64) -> u64 {
        ((time.max(0.0) / self.beat_seconds()) - 1e-9)
            .ceil()
            .max(0.0) as u64
    }

    /// `true` when beat number `beat` (counted from the start of the project) opens a bar.
    pub fn is_downbeat(&self, beat: u64) -> bool {
        beat.is_multiple_of(u64::from(self.beats_per_bar.max(1)))
    }
}

/// A decaying sine burst at `freq`, interleaved stereo.
fn click_samples(freq: f64, sample_rate: u32) -> Vec<f32> {
    let frames = (CLICK_SECONDS * f64::from(sample_rate)) as usize;
    let mut out = Vec::with_capacity(frames * 2);
    for n in 0..frames {
        let t = n as f64 / f64::from(sample_rate);
        let envelope = (-t * 180.0).exp();
        let v = (2.0 * std::f64::consts::PI * freq * t).sin() * envelope;
        out.push(v as f32);
        out.push(v as f32);
    }
    out
}

/// The clicks that fall in `from..to` (timeline seconds), at most [`MAX_CLICKS`].
pub fn click_clips(metronome: &Metronome, from: f64, to: f64, sample_rate: u32) -> Vec<MixClip> {
    if to <= from || metronome.volume <= 0.0 {
        return Vec::new();
    }
    let accent = Arc::new(click_samples(1600.0, sample_rate));
    let normal = Arc::new(click_samples(1000.0, sample_rate));
    let step = metronome.beat_seconds();
    let mut clips = Vec::new();
    let mut beat = metronome.first_beat_at_or_after(from);
    while clips.len() < MAX_CLICKS {
        let start = beat as f64 * step;
        if start >= to {
            break;
        }
        clips.push(MixClip {
            samples: Arc::clone(if metronome.is_downbeat(beat) {
                &accent
            } else {
                &normal
            }),
            source_rate: sample_rate,
            start,
            duration: CLICK_SECONDS,
            gain: metronome.volume.clamp(0.0, 1.0),
            track: METRONOME_TRACK,
            ..Default::default()
        });
        beat += 1;
    }
    clips
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beats_follow_the_tempo() {
        let m = Metronome::default();
        assert!((m.beat_seconds() - 0.5).abs() < 1e-9);
        assert_eq!(m.first_beat_at_or_after(0.0), 0);
        assert_eq!(m.first_beat_at_or_after(0.5), 1);
        assert_eq!(m.first_beat_at_or_after(0.51), 2);
        assert!(m.is_downbeat(0) && m.is_downbeat(4) && !m.is_downbeat(3));
        let waltz = Metronome {
            beats_per_bar: 3,
            ..m
        };
        assert!(waltz.is_downbeat(3) && !waltz.is_downbeat(4));
    }

    #[test]
    fn clicks_fill_the_range_with_an_accent_on_each_bar() {
        let clips = click_clips(&Metronome::default(), 0.0, 4.0, 48_000);
        assert_eq!(clips.len(), 8);
        assert!((clips[1].start - 0.5).abs() < 1e-9);
        // The bar opener is a different (higher) sound from the other beats
        assert!(!Arc::ptr_eq(&clips[0].samples, &clips[1].samples));
        assert!(Arc::ptr_eq(&clips[1].samples, &clips[2].samples));
        assert!(Arc::ptr_eq(&clips[0].samples, &clips[4].samples));
        // Playing from the middle of a beat starts on the next one
        let later = click_clips(&Metronome::default(), 1.2, 3.0, 48_000);
        assert!((later[0].start - 1.5).abs() < 1e-9);
    }

    #[test]
    fn a_click_is_audible_and_dies_away() {
        let clips = click_clips(&Metronome::default(), 0.0, 0.1, 48_000);
        let click = &clips[0];
        let peak = (0..1440)
            .map(|i| click.sample_at(i as f64 / 48_000.0).0.abs())
            .fold(0.0, f32::max);
        assert!(peak > 0.1 && peak <= 0.5, "peak {peak}");
        let tail = click.sample_at(0.029).0.abs();
        assert!(tail < peak * 0.1, "tail {tail}");
    }

    #[test]
    fn silence_empty_ranges_and_the_cap_are_respected() {
        let m = Metronome::default();
        assert!(click_clips(&m, 2.0, 2.0, 48_000).is_empty());
        assert!(click_clips(&Metronome { volume: 0.0, ..m }, 0.0, 10.0, 48_000).is_empty());
        let fast = Metronome { bpm: 400.0, ..m };
        assert_eq!(click_clips(&fast, 0.0, 100_000.0, 48_000).len(), MAX_CLICKS);
        // Absurd tempos are clamped, not divided by zero
        let silly = Metronome { bpm: 0.0, ..m };
        assert!(silly.beat_seconds().is_finite());
    }
}
