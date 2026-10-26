//! Audio DSP adapted from FilmCraft's `filmcraft-audio-dsp` (MIT OR Apache-2.0, used under MIT):
//! the ITU-R BS.1770-4 / EBU R128 loudness meter and the filters it needs.
//!
//! Provenance, licence and the list of changes are in `third_party/filmcraft/`. The code is
//! dependency-free and its `process` calls never allocate, so a meter may run inside the real-time
//! audio callback.

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
pub mod loudness;
pub mod oversample;

/// Flush tiny values to zero (denormal protection for recursive state).
#[inline(always)]
pub(crate) fn flush(x: f64) -> f64 {
    if x.abs() < 1e-30 {
        0.0
    } else {
        x
    }
}

#[cfg(test)]
pub(crate) mod testutil {
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

    /// Amplitude of the `freq` component of `x` (single-bin DFT).
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
