//! Noise suppression by a recurrent neural network: RNNoise (Xiph.Org / Jean-Marc Valin) as ported
//! to Rust by the `nnnoiseless` crate (BSD-3-Clause), wrapped as an [`AudioEffect`].
//!
//! RNNoise works on 10 ms frames of 48 kHz audio. The effect collects the stream into frames, runs
//! the network and hands the result back, so it delays the sound by [`RnNoise::latency`] samples
//! (the frame it waits for plus the delay of RNNoise's own overlap-add); the mixer compensates
//! that delay. Other sample rates go through a streaming linear rate converter to 48 kHz and back,
//! with a low-pass on the higher-rate side against aliasing (adequate for suppressing noise on
//! speech). After construction nothing allocates while processing.

use std::collections::VecDeque;

use nnnoiseless::DenoiseState;

use crate::dsp::biquad::{Biquad, Coeffs, FilterType};
use crate::dsp::{
    block_len, db_to_gain, param_plumbing, AudioEffect, ParamSpec, ParamValues, Smoothed, Unit,
};

const RNN_RATE: f64 = 48_000.0;
const FRAME: usize = DenoiseState::FRAME_SIZE;
/// RNNoise expects 16-bit sample values stored as `f32`.
const PCM_SCALE: f32 = 32_768.0;
/// Room reserved for every queue; the longest block the mixer hands over is 512 frames.
const QUEUE_CAPACITY: usize = 8192;

/// Streaming linear-interpolation rate converter: `step` input samples are consumed per output.
#[derive(Debug, Clone)]
struct Linear {
    step: f64,
    pos: f64,
    prev: f32,
}

impl Linear {
    fn new(from: f64, to: f64) -> Self {
        Self {
            step: from / to,
            pos: 0.0,
            prev: 0.0,
        }
    }

    /// Feeds one input sample and emits every output sample that falls before it.
    #[inline]
    fn push(&mut self, x: f32, mut emit: impl FnMut(f32)) {
        while self.pos < 1.0 {
            emit(self.prev + (x - self.prev) * self.pos as f32);
            self.pos += self.step;
        }
        self.pos -= 1.0;
        self.prev = x;
    }
}

struct Chan {
    state: Box<DenoiseState<'static>>,
    up: Linear,
    down: Linear,
    /// The low-pass on the way in and on the way out (at the higher of the two rates).
    lp_in: Biquad,
    lp_out: Biquad,
    frame_in: [f32; FRAME],
    filled: usize,
    /// Denoised samples at the effect's own sample rate, waiting to be handed out.
    out_q: VecDeque<f32>,
    /// The untouched input, delayed by the latency, for the dry / wet mix.
    dry_q: VecDeque<f32>,
    mix: Smoothed,
    gain: Smoothed,
}

pub struct RnNoise {
    pv: ParamValues,
    sr: f32,
    latency: usize,
    /// Anti-aliasing low-pass shared by both directions.
    lp: Coeffs,
    chans: Vec<Chan>,
}

/// Samples of an RNNoise frame at `rate`, rounded up.
fn frame_at(rate: f64) -> usize {
    (FRAME as f64 * rate / RNN_RATE).ceil() as usize
}

/// Everything a channel needs to know about the rate conversion.
#[derive(Clone, Copy)]
struct Convert<'a> {
    native: bool,
    /// The effect runs above 48 kHz (the filters then sit on the effect's side of the converter).
    above: bool,
    lp: &'a Coeffs,
}

impl Chan {
    fn fresh(rate: f64, latency: usize, sample_rate: f32) -> Self {
        let mut out_q = VecDeque::with_capacity(QUEUE_CAPACITY);
        let mut dry_q = VecDeque::with_capacity(QUEUE_CAPACITY);
        // The wet path already lags by one frame; the dry path matches the full latency
        out_q.extend(std::iter::repeat_n(0.0, latency - frame_at(rate)));
        dry_q.extend(std::iter::repeat_n(0.0, latency));
        Chan {
            state: DenoiseState::new(),
            up: Linear::new(rate, RNN_RATE),
            down: Linear::new(RNN_RATE, rate),
            lp_in: Biquad::new(),
            lp_out: Biquad::new(),
            frame_in: [0.0; FRAME],
            filled: 0,
            out_q,
            dry_q,
            mix: Smoothed::with_ms(1.0, sample_rate, 20.0),
            gain: Smoothed::with_ms(1.0, sample_rate, 20.0),
        }
    }

    /// One sample at RNNoise's rate goes into the frame being collected; a full frame is run
    /// through the network and its result queued at the effect's rate.
    fn feed_48k(&mut self, x: f32, conv: Convert) {
        self.frame_in[self.filled] = x * PCM_SCALE;
        self.filled += 1;
        if self.filled < FRAME {
            return;
        }
        self.filled = 0;
        let mut out = [0.0f32; FRAME];
        self.state.process_frame(&mut out, &self.frame_in);
        for v in out {
            let v = v / PCM_SCALE;
            if conv.native {
                self.out_q.push_back(v);
            } else {
                // Below 48 kHz the filter must come before the decimation
                let v = if conv.above {
                    v
                } else {
                    self.lp_out.tick(conv.lp, f64::from(v)) as f32
                };
                let (down, q) = (&mut self.down, &mut self.out_q);
                down.push(v, |y| q.push_back(y));
            }
        }
    }
}

impl RnNoise {
    pub const PARAMS: &'static [ParamSpec] = &[
        ParamSpec::new("mix", "Mistura", 0.0, 100.0, 100.0, Unit::Percent),
        ParamSpec::new("output", "Ganho de saída", -24.0, 24.0, 0.0, Unit::Decibels),
    ];

    pub fn new(sample_rate: f32, channels: usize) -> Self {
        let rate = f64::from(sample_rate);
        // One frame to wait for, one for RNNoise's own overlap-add, at the effect's rate
        let latency = 2 * frame_at(rate) + 2;
        // Content above the band both rates share would fold back: cut it at 20 kHz or 45 % of the
        // lower rate, filtering at the higher rate
        let cutoff = (0.45 * rate.min(RNN_RATE)).min(20_000.0);
        let mut s = Self {
            pv: ParamValues::new(Self::PARAMS),
            sr: sample_rate,
            latency,
            lp: Coeffs::design(FilterType::LowPass, rate.max(RNN_RATE), cutoff, 0.707, 0.0),
            chans: (0..channels.max(1))
                .map(|_| Chan::fresh(rate, latency, sample_rate))
                .collect(),
        };
        s.apply_params(true);
        s
    }

    fn apply_params(&mut self, snap: bool) {
        let (mix, gain) = (self.pv.v("mix") / 100.0, db_to_gain(self.pv.v("output")));
        for c in &mut self.chans {
            c.mix.set(mix);
            c.gain.set(gain);
            if snap {
                c.mix.snap();
                c.gain.snap();
            }
        }
    }
}

impl AudioEffect for RnNoise {
    param_plumbing!("rnnoise");

    fn latency(&self) -> usize {
        self.latency
    }

    fn reset(&mut self) {
        // The network has no reset: fresh channels are made (this allocates, so it is only called
        // when the effect is built, never from the audio callback)
        let (rate, latency, sr) = (f64::from(self.sr), self.latency, self.sr);
        let count = self.chans.len();
        self.chans = (0..count).map(|_| Chan::fresh(rate, latency, sr)).collect();
        self.apply_params(true);
    }

    fn process(&mut self, channels: &mut [&mut [f32]]) {
        let n = block_len(channels);
        let native = (self.sr - RNN_RATE as f32).abs() < 0.5;
        let above = f64::from(self.sr) > RNN_RATE + 0.5;
        let conv = Convert {
            native,
            above,
            lp: &self.lp,
        };
        for (block, c) in channels.iter_mut().zip(self.chans.iter_mut()) {
            for sample in block[..n].iter_mut() {
                let x = *sample;
                c.dry_q.push_back(x);
                if native {
                    c.feed_48k(x, conv);
                } else {
                    // Above 48 kHz the filter is in front of the decimation
                    let xin = if above {
                        c.lp_in.tick(conv.lp, f64::from(x)) as f32
                    } else {
                        x
                    };
                    // The converter emits zero, one or two samples for one input
                    let mut pending = [0.0f32; 4];
                    let mut count = 0;
                    c.up.push(xin, |y| {
                        pending[count] = y;
                        count += 1;
                    });
                    for &y in &pending[..count] {
                        // Below 48 kHz the up-conversion images are removed at 48 kHz
                        let y = if above {
                            y
                        } else {
                            c.lp_in.tick(conv.lp, f64::from(y)) as f32
                        };
                        c.feed_48k(y, conv);
                    }
                }
                let mut wet = c.out_q.pop_front().unwrap_or(0.0);
                if above {
                    // And above 48 kHz the interpolation images are removed on the way out
                    wet = c.lp_out.tick(conv.lp, f64::from(wet)) as f32;
                }
                let dry = c.dry_q.pop_front().unwrap_or(0.0);
                let (mix, gain) = (c.mix.tick(), c.gain.tick());
                *sample = (dry + (wet - dry) * mix) * gain;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::testutil::*;

    fn white_noise(seed: u64, n: usize, amplitude: f32) -> Vec<f32> {
        let mut rng = Rng::new(seed);
        (0..n).map(|_| rng.uniform() * amplitude).collect()
    }

    /// A voiced-sounding signal: a 140 Hz harmonic series with a syllable-rate envelope.
    fn voice_like(n: usize, rate: f64) -> Vec<f32> {
        (0..n)
            .map(|i| {
                let t = i as f64 / rate;
                let envelope = 0.5 + 0.5 * (2.0 * std::f64::consts::PI * 3.5 * t).sin();
                let tone: f64 = (1..=12)
                    .map(|h| {
                        (2.0 * std::f64::consts::PI * 140.0 * f64::from(h) * t).sin() / f64::from(h)
                    })
                    .sum();
                (0.25 * envelope * tone) as f32
            })
            .collect()
    }

    fn run(effect: &mut RnNoise, input: &[f32], block: usize) -> Vec<f32> {
        let mut out = input.to_vec();
        for chunk in out.chunks_mut(block) {
            effect.process(&mut [chunk]);
        }
        out
    }

    #[test]
    fn the_effect_is_the_network_and_nothing_else_at_48_khz() {
        // Reference: the same stream through nnnoiseless directly, frame by frame
        let input = white_noise(5, FRAME * 40, 0.06);
        let mut reference = Vec::new();
        let mut state = DenoiseState::new();
        for frame in input.chunks(FRAME) {
            let scaled: Vec<f32> = frame.iter().map(|v| v * PCM_SCALE).collect();
            let mut out = [0.0f32; FRAME];
            state.process_frame(&mut out, &scaled);
            reference.extend(out.iter().map(|v| v / PCM_SCALE));
        }

        let mut fx = RnNoise::new(48_000.0, 1);
        let out = run(&mut fx, &input, 301);
        // What comes out is zeros for the start-up delay, then the network's frames in order
        let start = fx.latency() - frame_at(48_000.0);
        assert!(
            out[..start].iter().all(|v| v.abs() < 1e-9),
            "silence while it fills"
        );
        for (k, expected) in reference.iter().enumerate().take(input.len() - start) {
            assert!(
                (out[start + k] - expected).abs() < 1e-6,
                "sample {k}: {} vs {expected}",
                out[start + k]
            );
        }
    }

    #[test]
    fn the_network_really_removes_noise() {
        // White noise at about -29 dBFS: the trained network takes a good part of it away
        let mut fx = RnNoise::new(48_000.0, 1);
        let noise = white_noise(7, 96_000, 0.06);
        let out = run(&mut fx, &noise, 480);
        let change = db(rms(&out[48_000..]) / rms(&noise[48_000..]));
        assert!(change < -6.0, "only {change:.1} dB");
    }

    #[test]
    fn a_dry_mix_returns_the_input_delayed_by_exactly_the_latency() {
        for rate in [48_000.0f32, 44_100.0, 96_000.0, 32_000.0] {
            let mut fx = RnNoise::new(rate, 1);
            assert!(fx.set_param("mix", 0.0));
            fx.reset();
            let input = voice_like(rate as usize / 2, f64::from(rate));
            let out = run(&mut fx, &input, 333);
            let lag = fx.latency();
            assert!(
                lag >= 2 * frame_at(f64::from(rate)),
                "two RNNoise frames of delay at least: {lag}"
            );
            for i in 0..input.len() - lag {
                assert!(
                    (out[i + lag] - input[i]).abs() < 1e-6,
                    "{rate} Hz, sample {i}"
                );
            }
        }
    }

    #[test]
    fn the_wet_path_lines_up_with_the_dry_path() {
        // The reported latency has to be the real delay of the denoised sound, or a partial mix
        // and the mixer's delay compensation would smear the signal
        let mut fx = RnNoise::new(48_000.0, 1);
        let input = voice_like(96_000, 48_000.0);
        let out = run(&mut fx, &input, 480);
        let lag = fx.latency();
        let score = |s: usize| -> f64 {
            (48_000..90_000)
                .map(|i| f64::from(out[i]) * f64::from(input[i - s]))
                .sum()
        };
        let best = (lag.saturating_sub(64)..=lag + 64)
            .max_by(|&a, &b| score(a).total_cmp(&score(b)))
            .unwrap();
        assert!(
            (best as i64 - lag as i64).abs() <= 8,
            "the denoised sound lags by {best} samples, the effect reports {lag}"
        );
    }

    #[test]
    fn the_output_does_not_depend_on_how_the_stream_is_cut_into_blocks() {
        for rate in [48_000.0f32, 44_100.0] {
            let input = white_noise(3, 30_000, 0.3);
            let a = run(&mut RnNoise::new(rate, 1), &input, 30_000);
            let b = run(&mut RnNoise::new(rate, 1), &input, 137);
            let worst = a
                .iter()
                .zip(&b)
                .map(|(x, y)| (x - y).abs())
                .fold(0.0f32, f32::max);
            assert!(worst < 1e-6, "{rate} Hz: {worst}");
        }
    }

    #[test]
    fn other_sample_rates_work_and_still_remove_noise() {
        for rate in [44_100.0f32, 96_000.0, 32_000.0] {
            let mut fx = RnNoise::new(rate, 1);
            let noise = white_noise(7, rate as usize * 2, 0.06);
            let out = run(&mut fx, &noise, 441);
            assert!(out.iter().all(|v| v.is_finite()), "{rate}");
            let half = rate as usize;
            let change = db(rms(&out[half..]) / rms(&noise[half..]));
            assert!(change < -3.0, "{rate} Hz: only {change:.1} dB");
        }
    }

    #[test]
    fn every_channel_is_denoised_on_its_own() {
        let mut fx = RnNoise::new(48_000.0, 2);
        let (mut l, mut r) = (white_noise(1, 48_000, 0.06), vec![0.0f32; 48_000]);
        fx.process(&mut [&mut l, &mut r]);
        assert!(r.iter().all(|v| *v == 0.0), "silence stays silence");
        assert!(rms(&l[24_000..]) < 0.035, "the noisy channel is reduced");
    }

    #[test]
    fn the_output_gain_and_parameters_are_applied() {
        let mut fx = RnNoise::new(48_000.0, 1);
        assert!(fx.set_param("mix", 0.0));
        assert!(fx.set_param("output", 6.0));
        fx.reset();
        let input = voice_like(10_000, 48_000.0);
        let out = run(&mut fx, &input, 480);
        let lag = fx.latency();
        let ratio = out[lag + 500] / input[500];
        assert!((ratio - db_to_gain(6.0)).abs() < 1e-3, "{ratio}");
        assert_eq!(fx.param("mix"), Some(0.0));
        assert!(!fx.set_param("nope", 1.0));
    }
}
