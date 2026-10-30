//! Auto-ducking: turn a track down while other tracks (dialogue, effects) have sound.
//!
//! The controllers' audio is analysed once, when playback starts: its level is measured every
//! 10 ms, the stretches above the sensitivity threshold are found (short blips dropped, short gaps
//! bridged) and a gain curve is built that ramps down *before* the sound starts and back up after
//! it ends, which is why this is an analysis and not a live sidechain. The functions that find the
//! stretches and build the ramps are FilmCraft's (`dsp::ducking`); measuring the level of the
//! controller clips is Neoveg's.

use crate::dsp::ducking::{activity, duck_keyframes, sensitivity_threshold_db};
use crate::mixer::MixClip;

/// The controllers are sampled at this rate for the analysis, far below the audio rate: the level
/// of speech does not need more, and a long programme stays cheap to analyse.
const ANALYSIS_RATE: f64 = 4_000.0;
const HOP_S: f64 = 0.01;
/// The level of a hop is the mean power over this many hops around it (50 ms).
const WINDOW_HOPS: usize = 5;
/// Sound shorter than this does not duck.
const MIN_ON_S: f32 = 0.1;
/// Gaps shorter than this do not let the music back up.
const BRIDGE_S: f32 = 0.25;
const FLOOR_DB: f32 = -120.0;

/// Gain over time, as keyframes in dB with straight ramps between them. Outside the first and last
/// keyframe the nearest value holds; no keyframes means no ducking.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DuckCurve {
    keys: Vec<(f64, f32)>,
}

impl DuckCurve {
    pub fn keys(&self) -> &[(f64, f32)] {
        &self.keys
    }

    /// `true` when the curve never changes the gain.
    pub fn is_flat(&self) -> bool {
        self.keys.iter().all(|k| k.1 == 0.0)
    }

    /// Gain in dB at timeline time `t`.
    pub fn gain_db(&self, t: f64) -> f32 {
        let (Some(first), Some(last)) = (self.keys.first(), self.keys.last()) else {
            return 0.0;
        };
        if t <= first.0 {
            return first.1;
        }
        if t >= last.0 {
            return last.1;
        }
        let i = self.keys.partition_point(|k| k.0 <= t);
        let (a, b) = (self.keys[i - 1], self.keys[i]);
        let f = ((t - a.0) / (b.0 - a.0)) as f32;
        a.1 + (b.1 - a.1) * f
    }

    /// Linear gain at timeline time `t`.
    #[inline]
    pub fn gain(&self, t: f64) -> f32 {
        let db = self.gain_db(t);
        if db == 0.0 {
            1.0
        } else {
            10f32.powf(db / 20.0)
        }
    }
}

/// Level (dBFS) of the summed controller clips every 10 ms until `end` seconds.
pub fn trigger_envelope_db(controllers: &[&MixClip], end: f64) -> Vec<f32> {
    let frames_per_hop = ((ANALYSIS_RATE * HOP_S).round() as usize).max(1);
    let hops = (end.max(0.0) / HOP_S).ceil() as usize;
    let mut power = vec![0.0f64; hops];
    for (k, p) in power.iter_mut().enumerate() {
        let t0 = k as f64 * HOP_S;
        // Only clips that are sounding in this hop need to be read
        let active: Vec<&&MixClip> = controllers
            .iter()
            .filter(|c| c.start < t0 + HOP_S && c.end() > t0)
            .collect();
        if active.is_empty() {
            continue;
        }
        let mut sum = 0.0f64;
        for j in 0..frames_per_hop {
            let t = t0 + j as f64 / ANALYSIS_RATE;
            let (mut l, mut r) = (0.0f32, 0.0f32);
            for clip in &active {
                let (cl, cr) = clip.sample_at(t);
                l += cl;
                r += cr;
            }
            sum += 0.5 * (f64::from(l).powi(2) + f64::from(r).powi(2));
        }
        *p = sum / frames_per_hop as f64;
    }
    let half = WINDOW_HOPS / 2;
    (0..hops)
        .map(|k| {
            let (lo, hi) = (k.saturating_sub(half), (k + half + 1).min(hops));
            let mean = power[lo..hi].iter().sum::<f64>() / (hi - lo) as f64;
            ((10.0 * mean.max(1e-12).log10()) as f32).max(FLOOR_DB)
        })
        .collect()
}

/// The gain curve that ducks by `reduce_db` while `controllers` sound, over a programme of `end`
/// seconds. `sensitivity` is 0 to 10 (higher reacts to quieter sound), `fade_s` the ramp length.
pub fn analyze(
    controllers: &[&MixClip],
    end: f64,
    sensitivity: f32,
    reduce_db: f32,
    fade_s: f32,
) -> DuckCurve {
    if controllers.is_empty() || end <= 0.0 {
        return DuckCurve::default();
    }
    let envelope = trigger_envelope_db(controllers, end);
    let regions = activity(
        &envelope,
        HOP_S as f32,
        sensitivity_threshold_db(sensitivity),
        MIN_ON_S,
        BRIDGE_S,
    );
    let keys = duck_keyframes(
        &regions,
        0.0,
        end,
        0.0,
        f64::from(reduce_db),
        f64::from(fade_s),
    );
    DuckCurve {
        keys: keys.into_iter().map(|(t, db)| (t, db as f32)).collect(),
    }
}

/// Real-time zero-allocation sidechain envelope follower driven by the post-FX, post-fader audio
/// of controller buses inside [`crate::fxmixer::BlockMixer`].
#[derive(Debug, Clone)]
pub struct SidechainFollower {
    threshold_power: f32,
    target_gain: f32,
    det_alpha: f32,
    atk_alpha: f32,
    rel_alpha: f32,
    hold_samples: usize,
    hold_left: usize,
    power_env: f32,
    current_gain: f32,
}

impl SidechainFollower {
    pub fn new(sample_rate: f32, sensitivity: f32, reduce_db: f32, fade_s: f32) -> Self {
        let sr = sample_rate.max(1000.0);
        let threshold_db = sensitivity_threshold_db(sensitivity);
        let threshold_power = 10.0_f32.powf(threshold_db / 10.0);
        let target_gain = 10.0_f32.powf(-reduce_db.abs() / 20.0);
        let coeff = |tau_s: f32| 1.0 - (-1.0 / (tau_s.max(0.001) * sr)).exp();
        let attack_s = (fade_s * 0.15).clamp(0.005, 0.15);
        let release_s = fade_s.clamp(0.03, 5.0);
        Self {
            threshold_power,
            target_gain,
            det_alpha: coeff(0.008),
            atk_alpha: coeff(attack_s),
            rel_alpha: coeff(release_s * 0.35),
            hold_samples: (0.15 * sr).round() as usize,
            hold_left: 0,
            power_env: 0.0,
            current_gain: 1.0,
        }
    }

    /// Feeds one stereo key sample's mean square power `0.5 * (l*l + r*r)` and returns the linear
    /// ducking gain to apply to the target bus sample (`1.0` when idle, `target_gain` when keyed).
    #[inline]
    pub fn next_gain(&mut self, key_power: f32) -> f32 {
        self.power_env += self.det_alpha * (key_power - self.power_env);
        if self.power_env >= self.threshold_power {
            self.hold_left = self.hold_samples;
        } else if self.hold_left > 0 {
            self.hold_left -= 1;
        }
        let active = self.hold_left > 0;
        let goal = if active { self.target_gain } else { 1.0 };
        let alpha = if goal < self.current_gain {
            self.atk_alpha
        } else {
            self.rel_alpha
        };
        self.current_gain += alpha * (goal - self.current_gain);
        self.current_gain
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pcm::PcmStore;

    /// A clip of constant stereo `value` from `start` for `seconds` (source rate 1 kHz).
    fn constant(value: f32, start: f64, seconds: f64) -> MixClip {
        MixClip {
            samples: PcmStore::shared(vec![value; (seconds * 1000.0) as usize * 2]),
            source_rate: 1000,
            start,
            duration: seconds,
            gain: 1.0,
            ..Default::default()
        }
    }

    #[test]
    fn the_curve_ramps_down_before_the_sound_holds_and_comes_back_after_it() {
        let voice = constant(0.3, 1.0, 1.0);
        let curve = analyze(&[&voice], 4.0, 6.0, 12.0, 0.2);
        assert!(!curve.is_flat());
        assert_eq!(curve.gain_db(0.5), 0.0, "before the ramp");
        assert!(
            (curve.gain_db(0.9) - -6.0).abs() < 2.0,
            "on the way down: {}",
            curve.gain_db(0.9)
        );
        assert!((curve.gain_db(1.5) - -12.0).abs() < 1e-3, "ducked");
        assert!(
            (curve.gain_db(2.1) - -6.0).abs() < 2.0,
            "on the way up: {}",
            curve.gain_db(2.1)
        );
        assert_eq!(curve.gain_db(3.0), 0.0, "back to normal");
        assert!(
            (curve.gain(1.5) - 0.2512).abs() < 1e-3,
            "12 dB is a factor of 0.251"
        );
    }

    #[test]
    fn sound_below_the_sensitivity_threshold_does_not_duck() {
        // -60 dBFS against a sensitivity of 0 (threshold -20 dBFS)
        let hiss = constant(0.001, 0.0, 3.0);
        assert!(analyze(&[&hiss], 3.0, 0.0, 12.0, 0.2).is_flat());
        // Normal-level sound ducks even at the lowest sensitivity
        let normal = constant(0.3, 0.0, 3.0);
        assert!(!analyze(&[&normal], 3.0, 0.0, 12.0, 0.2).is_flat());
    }

    #[test]
    fn short_blips_are_ignored_and_short_gaps_are_bridged() {
        let blip = constant(0.3, 1.0, 0.05);
        assert!(
            analyze(&[&blip], 3.0, 6.0, 12.0, 0.2).is_flat(),
            "a 50 ms blip is not speech"
        );

        // Two words 100 ms apart: the music stays down between them
        let (a, b) = (constant(0.3, 1.0, 0.5), constant(0.3, 1.6, 0.5));
        let curve = analyze(&[&a, &b], 4.0, 6.0, 12.0, 0.2);
        assert!(
            (curve.gain_db(1.55) - -12.0).abs() < 1e-3,
            "{}",
            curve.gain_db(1.55)
        );
    }

    #[test]
    fn several_controllers_are_summed_and_no_controllers_means_no_curve() {
        let early = constant(0.3, 0.5, 0.5);
        let late = constant(0.3, 2.5, 0.5);
        let curve = analyze(&[&early, &late], 4.0, 6.0, 12.0, 0.1);
        assert!(curve.gain_db(0.75) < -11.0 && curve.gain_db(2.75) < -11.0);
        assert_eq!(curve.gain_db(1.5), 0.0);
        assert!(analyze(&[], 4.0, 6.0, 12.0, 0.2).keys().is_empty());
        assert_eq!(DuckCurve::default().gain(1.0), 1.0);
    }

    #[test]
    fn the_gain_curve_is_linear_in_db_between_keys_and_holds_at_the_ends() {
        let curve = DuckCurve {
            keys: vec![(1.0, 0.0), (2.0, -10.0)],
        };
        assert_eq!(curve.gain_db(0.0), 0.0);
        assert!((curve.gain_db(1.5) - -5.0).abs() < 1e-5);
        assert_eq!(curve.gain_db(9.0), -10.0);
    }

    #[test]
    fn realtime_sidechain_follower_attenuates_on_signal_holds_and_releases() {
        let sr = 48_000.0;
        let mut sc = SidechainFollower::new(sr, 6.0, 12.0, 0.2);
        // Silence keeps gain at unity
        for _ in 0..4800 {
            assert!((sc.next_gain(0.0) - 1.0).abs() < 1e-5);
        }
        // Loud key signal (0.3 amplitude -> power 0.09) drives gain to ~-12 dB (0.2512)
        let mut g = 1.0;
        for _ in 0..12_000 {
            g = sc.next_gain(0.09);
        }
        assert!((g - 0.2512).abs() < 0.02, "ducked gain: {g}");
        // 50 ms micro-gap stays held down
        for _ in 0..2400 {
            g = sc.next_gain(0.0);
        }
        assert!((g - 0.2512).abs() < 0.02, "held gain: {g}");
        // Extended silence releases smoothly back to unity
        for _ in 0..48_000 {
            g = sc.next_gain(0.0);
        }
        assert!((g - 1.0).abs() < 0.02, "released gain: {g}");
    }
}
