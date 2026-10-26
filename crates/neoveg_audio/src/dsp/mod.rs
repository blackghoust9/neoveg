//! Audio DSP adapted from FilmCraft's `filmcraft-audio-dsp` (MIT OR Apache-2.0, used under MIT):
//! the ITU-R BS.1770-4 / EBU R128 loudness meter and the effects the tracks can use (parametric
//! and 3-band EQ, compressor, gate, limiter, delay and reverb) with the filters they need.
//!
//! Provenance, licence and the list of changes are in `third_party/filmcraft/`. The code is
//! dependency-free; after construction `process`, `set_param` and `reset` never allocate, so
//! effects and the meter may run inside the real-time audio callback.

// The adapted code keeps its upstream style; these lints are noise for numeric DSP loops.
#![allow(
    clippy::needless_range_loop,
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::excessive_precision,
    clippy::manual_range_contains,
    dead_code
)]

pub mod biquad;
mod effect;
pub mod effects;
pub mod loudness;
pub mod oversample;
mod smooth;

pub(crate) use effect::{block_len, flush, flush32, param_plumbing, ParamValues};
pub use effect::{db_to_gain, gain_to_db, AudioEffect, ParamSpec, Unit};
pub use smooth::Smoothed;

#[cfg(test)]
pub(crate) mod testutil {
    /// Tiny deterministic PRNG (xorshift64*) for tests.
    pub struct Rng(u64);
    impl Rng {
        pub fn new(seed: u64) -> Self {
            Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
        }
        pub fn next_u64(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            self.0 = x;
            x.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }
        /// Uniform in [-1, 1).
        pub fn uniform(&mut self) -> f32 {
            ((self.next_u64() >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
        }
    }

    pub fn sine(freq: f64, amp: f64, sr: f64, n: usize, phase: f64) -> Vec<f32> {
        (0..n)
            .map(|i| {
                (amp * (2.0 * std::f64::consts::PI * freq * i as f64 / sr + phase).sin()) as f32
            })
            .collect()
    }

    pub fn rms(x: &[f32]) -> f64 {
        if x.is_empty() {
            return 0.0;
        }
        (x.iter().map(|&v| (v as f64) * (v as f64)).sum::<f64>() / x.len() as f64).sqrt()
    }

    pub fn db(x: f64) -> f64 {
        20.0 * x.log10()
    }

    /// Amplitude of the `freq` component of `x` (single-bin DFT / Goertzel-style correlation).
    pub fn tone_amplitude(x: &[f32], freq: f64, sr: f64) -> f64 {
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (i, &v) in x.iter().enumerate() {
            let w = 2.0 * std::f64::consts::PI * freq * i as f64 / sr;
            re += v as f64 * w.cos();
            im += v as f64 * w.sin();
        }
        2.0 * (re * re + im * im).sqrt() / x.len() as f64
    }
}
