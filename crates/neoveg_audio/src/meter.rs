//! Peak meters fed exclusively by real audio samples (no timers, noise or oscillators).
//!
//! The meter holds the peak of the last processed buffer. It is zeroed when playback is stopped.

use std::sync::atomic::{AtomicU32, Ordering};

use crate::waveform::WaveformSource;

#[derive(Debug, Default)]
pub struct PeakMeter {
    left: AtomicU32,
    right: AtomicU32,
}

impl PeakMeter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores the linear peaks (absolute values, clamped to `[0, 1]`).
    pub fn set(&self, left: f32, right: f32) {
        self.left.store(left.abs().min(1.0).to_bits(), Ordering::Relaxed);
        self.right.store(right.abs().min(1.0).to_bits(), Ordering::Relaxed);
    }

    /// Zeroes both channels (`-inf dBFS`).
    pub fn reset(&self) {
        self.set(0.0, 0.0);
    }

    /// Linear `(left, right)` peaks.
    pub fn peak(&self) -> (f32, f32) {
        (
            f32::from_bits(self.left.load(Ordering::Relaxed)),
            f32::from_bits(self.right.load(Ordering::Relaxed)),
        )
    }

    /// Peak of an interleaved buffer with `channels` channels (mono feeds both meters).
    pub fn update_from_buffer(&self, interleaved: &[f32], channels: usize) {
        let ch = channels.max(1);
        let (mut l, mut r) = (0.0f32, 0.0f32);
        for frame in interleaved.chunks_exact(ch) {
            l = l.max(frame[0].abs());
            r = r.max(if ch > 1 { frame[1].abs() } else { frame[0].abs() });
        }
        self.set(l, r);
    }
}

/// Real peak of `frames` source frames starting at `start_frame`.
pub fn peak_of_range(source: &WaveformSource, start_frame: i64, frames: usize) -> (f32, f32) {
    let mut l = vec![0.0f32; frames];
    let mut r = vec![0.0f32; frames];
    let n = source.read_samples(start_frame, &mut l, &mut r);
    let max = |v: &[f32]| v.iter().take(n.max(0)).fold(0.0f32, |m, s| m.max(s.abs()));
    (max(&l), max(&r))
}

/// Linear amplitude to dBFS (`-inf` for silence).
pub fn linear_to_dbfs(linear: f32) -> f32 {
    if linear <= 0.0 {
        f32::NEG_INFINITY
    } else {
        20.0 * linear.log10()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_and_resets_to_silence() {
        let m = PeakMeter::new();
        assert_eq!(m.peak(), (0.0, 0.0));
        m.set(0.5, 0.25);
        m.reset();
        assert_eq!(m.peak(), (0.0, 0.0));
        assert_eq!(linear_to_dbfs(m.peak().0), f32::NEG_INFINITY);
    }

    #[test]
    fn buffer_peak_is_the_real_maximum() {
        let m = PeakMeter::new();
        m.update_from_buffer(&[0.1, -0.9, -0.5, 0.2], 2);
        assert_eq!(m.peak(), (0.5, 0.9));
    }

    #[test]
    fn full_scale_is_zero_dbfs() {
        assert!((linear_to_dbfs(1.0)).abs() < 1e-6);
        assert!((linear_to_dbfs(0.5) + 6.0206).abs() < 1e-3);
    }

    #[test]
    fn range_peak_reads_the_source() {
        let src = WaveformSource::from_interleaved_f32(48_000, 2, &[0.0, 0.0, 0.7, -0.2, 0.1, 0.1]).unwrap();
        assert_eq!(peak_of_range(&src, 0, 3), (0.7, 0.2));
        assert_eq!(peak_of_range(&src, 10, 3), (0.0, 0.0));
    }
}
