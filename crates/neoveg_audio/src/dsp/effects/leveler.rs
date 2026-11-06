//! Dialogue leveler: evens out the loudness of speech over an event (adaptive compression with a
//! downward expander, no machine learning).
//!
//! A slow RMS detector measures how loud the speech is right now; the gain that would bring that
//! level to the target is computed (boosts limited to `max_boost`, cuts to `max_cut`) and smoothed
//! (cuts follow quickly so loud words do not jump out, boosts follow slowly so breaths do not
//! pump). Below the noise floor the boost fades out and the signal is expanded downward, so room
//! noise between phrases is not lifted.

use crate::dsp::{
    block_len, db_to_gain, param_plumbing, AudioEffect, ParamSpec, ParamValues, Unit,
};

/// Width (dB) above the floor over which the boost fades in.
const FADE_DB: f32 = 8.0;
/// Expander slope below the floor: dB of extra attenuation per dB under the floor.
const EXPAND_SLOPE: f32 = 1.0;
/// Largest downward expansion (dB).
const EXPAND_RANGE: f32 = 24.0;

fn coef(ms: f32, sr: f32) -> f32 {
    if ms <= 0.0 {
        0.0
    } else {
        (-1.0 / (ms * 0.001 * sr)).exp()
    }
}

/// Gain (dB) the leveler wants for a speech level of `level_db`.
pub fn leveler_gain_db(
    level_db: f32,
    target: f32,
    max_boost: f32,
    max_cut: f32,
    floor: f32,
) -> f32 {
    let raw = (target - level_db).clamp(-max_cut, max_boost);
    // Boost fades in over FADE_DB above the floor; cuts are always allowed.
    let fade = ((level_db - floor) / FADE_DB).clamp(0.0, 1.0);
    let gain = if raw > 0.0 { raw * fade } else { raw };
    if level_db < floor {
        gain - ((floor - level_db) * EXPAND_SLOPE).min(EXPAND_RANGE)
    } else {
        gain
    }
}

pub struct DialogueLeveler {
    pv: ParamValues,
    sr: f32,
    target: f32,
    max_boost: f32,
    max_cut: f32,
    floor: f32,
    rms_coef: f32,
    up: f32,
    down: f32,
    /// Detector mean square.
    ms: f32,
    /// Smoothed gain (dB).
    env: f32,
}

impl DialogueLeveler {
    pub const PARAMS: &'static [ParamSpec] = &[
        ParamSpec::new(
            "target",
            "Nível alvo da fala",
            -50.0,
            -6.0,
            -22.0,
            Unit::Decibels,
        ),
        ParamSpec::new(
            "max_boost",
            "Reforço máximo",
            0.0,
            30.0,
            15.0,
            Unit::Decibels,
        ),
        ParamSpec::new("max_cut", "Redução máxima", 0.0, 30.0, 12.0, Unit::Decibels),
        ParamSpec::new(
            "floor",
            "Piso de ruído",
            -90.0,
            -20.0,
            -55.0,
            Unit::Decibels,
        ),
        ParamSpec::log(
            "speed",
            "Velocidade",
            50.0,
            3000.0,
            400.0,
            Unit::Milliseconds,
        ),
    ];

    pub fn new(sample_rate: f32, _channels: usize) -> Self {
        let mut s = DialogueLeveler {
            pv: ParamValues::new(Self::PARAMS),
            sr: sample_rate,
            target: -22.0,
            max_boost: 15.0,
            max_cut: 12.0,
            floor: -55.0,
            rms_coef: coef(150.0, sample_rate),
            up: 0.0,
            down: 0.0,
            ms: 0.0,
            env: 0.0,
        };
        s.apply_params(true);
        s
    }

    fn apply_params(&mut self, _snap: bool) {
        self.target = self.pv.v("target");
        self.max_boost = self.pv.v("max_boost");
        self.max_cut = self.pv.v("max_cut");
        self.floor = self.pv.v("floor");
        let speed = self.pv.v("speed");
        self.up = coef(speed, self.sr);
        self.down = coef(speed * 0.15, self.sr);
    }

    /// Current applied gain in dB, for metering.
    pub fn gain_db(&self) -> f32 {
        self.env
    }
}

impl AudioEffect for DialogueLeveler {
    param_plumbing!("dialogue_leveler");
    fn reset(&mut self) {
        self.ms = 0.0;
        self.env = 0.0;
    }
    fn process(&mut self, channels: &mut [&mut [f32]]) {
        let n = block_len(channels);
        for i in 0..n {
            let sq = channels.iter().fold(0.0f32, |a, c| a.max(c[i] * c[i]));
            self.ms = self.rms_coef * self.ms + (1.0 - self.rms_coef) * sq;
            if self.ms < 1e-30 {
                self.ms = 0.0;
            }
            let level = 10.0 * self.ms.max(1e-12).log10();
            let want =
                leveler_gain_db(level, self.target, self.max_boost, self.max_cut, self.floor);
            let c = if want < self.env { self.down } else { self.up };
            self.env = c * self.env + (1.0 - c) * want;
            if self.env.abs() < 1e-9 {
                self.env = 0.0;
            }
            let g = db_to_gain(self.env);
            for ch in channels.iter_mut() {
                ch[i] *= g;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::testutil::*;

    const SR: f32 = 48_000.0;

    fn rms_db(x: &[f32]) -> f32 {
        let s: f64 = x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / x.len() as f64;
        10.0 * (s.max(1e-20) as f32).log10()
    }

    /// Quiet phrase (-38 dB rms) followed by a loud one (-14 dB rms).
    fn two_phrases() -> Vec<f32> {
        let n = SR as usize * 3;
        let mut x = sine(220.0, 0.018, SR as f64, n, 0.0);
        x.extend(sine(220.0, 0.28, SR as f64, n, 0.0));
        x
    }

    #[test]
    fn narrows_the_gap_between_quiet_and_loud_speech() {
        let src = two_phrases();
        let n = SR as usize * 3;
        let before = rms_db(&src[n * 2 - 24_000..n * 2]) - rms_db(&src[n - 24_000..n]);
        let mut fx = DialogueLeveler::new(SR, 1);
        let mut y = src.clone();
        fx.process(&mut [&mut y]);
        let after = rms_db(&y[n * 2 - 24_000..n * 2]) - rms_db(&y[n - 24_000..n]);
        assert!(before > 20.0, "{before}");
        assert!(after < before - 10.0, "before {before}, after {after}");
    }

    #[test]
    fn does_not_lift_noise_below_the_floor() {
        let mut r = Rng::new(7);
        let mut y: Vec<f32> = (0..SR as usize * 2).map(|_| r.uniform() * 0.0003).collect();
        let before = rms_db(&y);
        let mut fx = DialogueLeveler::new(SR, 1);
        fx.process(&mut [&mut y]);
        assert!(before < -65.0, "{before}");
        assert!(
            rms_db(&y[SR as usize..]) <= before + 0.5,
            "noise was boosted"
        );
    }

    #[test]
    fn gain_curve_boosts_quiet_and_cuts_loud() {
        assert!(leveler_gain_db(-35.0, -22.0, 15.0, 12.0, -55.0) > 10.0);
        assert!(leveler_gain_db(-8.0, -22.0, 15.0, 12.0, -55.0) < -10.0);
        assert!(leveler_gain_db(-80.0, -22.0, 15.0, 12.0, -55.0) < 0.0);
    }
}
