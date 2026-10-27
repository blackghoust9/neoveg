//! Loudness readings shared between the audio callback and the UI.
//!
//! The callback owns a [`LoudnessMeter`] (EBU R128 / BS.1770-4) fed with the samples that are
//! really sent to the device, and publishes its readings here through atomics, so neither side
//! ever locks or allocates. Nothing in this module is simulated: with no playback every value is
//! `-inf`.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use crate::dsp::loudness::LoudnessMeter;

/// A copy of every reading at one instant. Loudness in LUFS, range in LU, true peak in dBTP;
/// `f64::NEG_INFINITY` means "nothing measured".
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoudnessSnapshot {
    pub momentary_lufs: f64,
    pub short_term_lufs: f64,
    pub integrated_lufs: f64,
    pub range_lu: f64,
    pub true_peak_dbtp: f64,
    pub max_momentary_lufs: f64,
    pub max_short_term_lufs: f64,
}

impl LoudnessSnapshot {
    pub const SILENT: Self = Self {
        momentary_lufs: f64::NEG_INFINITY,
        short_term_lufs: f64::NEG_INFINITY,
        integrated_lufs: f64::NEG_INFINITY,
        range_lu: 0.0,
        true_peak_dbtp: f64::NEG_INFINITY,
        max_momentary_lufs: f64::NEG_INFINITY,
        max_short_term_lufs: f64::NEG_INFINITY,
    };
}

#[derive(Debug)]
pub struct LoudnessReadout {
    momentary: AtomicU64,
    short_term: AtomicU64,
    integrated: AtomicU64,
    range: AtomicU64,
    true_peak: AtomicU64,
    max_momentary: AtomicU64,
    max_short_term: AtomicU64,
    reset_requested: AtomicBool,
}

fn bits(value: f64) -> AtomicU64 {
    AtomicU64::new(value.to_bits())
}

fn read(cell: &AtomicU64) -> f64 {
    f64::from_bits(cell.load(Ordering::Relaxed))
}

impl Default for LoudnessReadout {
    fn default() -> Self {
        let s = LoudnessSnapshot::SILENT;
        Self {
            momentary: bits(s.momentary_lufs),
            short_term: bits(s.short_term_lufs),
            integrated: bits(s.integrated_lufs),
            range: bits(s.range_lu),
            true_peak: bits(s.true_peak_dbtp),
            max_momentary: bits(s.max_momentary_lufs),
            max_short_term: bits(s.max_short_term_lufs),
            reset_requested: AtomicBool::new(false),
        }
    }
}

impl LoudnessReadout {
    pub fn new() -> Self {
        Self::default()
    }

    /// Publishes the cheap readings (window loudness and peaks). Called every audio buffer.
    pub fn store_fast(&self, meter: &LoudnessMeter) {
        self.momentary
            .store(meter.momentary().to_bits(), Ordering::Relaxed);
        self.short_term
            .store(meter.short_term().to_bits(), Ordering::Relaxed);
        self.true_peak
            .store(meter.true_peak_dbtp().to_bits(), Ordering::Relaxed);
        self.max_momentary
            .store(meter.max_momentary().to_bits(), Ordering::Relaxed);
        self.max_short_term
            .store(meter.max_short_term().to_bits(), Ordering::Relaxed);
    }

    /// Publishes the readings that scan the gating histogram (integrated loudness and range);
    /// the callback calls it about every 100 ms, the rate at which the meter produces blocks.
    pub fn store_slow(&self, meter: &LoudnessMeter) {
        self.integrated
            .store(meter.integrated().to_bits(), Ordering::Relaxed);
        self.range
            .store(meter.loudness_range().to_bits(), Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> LoudnessSnapshot {
        LoudnessSnapshot {
            momentary_lufs: read(&self.momentary),
            short_term_lufs: read(&self.short_term),
            integrated_lufs: read(&self.integrated),
            range_lu: read(&self.range),
            true_peak_dbtp: read(&self.true_peak),
            max_momentary_lufs: read(&self.max_momentary),
            max_short_term_lufs: read(&self.max_short_term),
        }
    }

    /// Asks the audio callback to restart the measurement; the readings are cleared at once.
    pub fn request_reset(&self) {
        self.reset_requested.store(true, Ordering::Relaxed);
        self.clear();
    }

    /// Callback side: `true` once per [`request_reset`](Self::request_reset).
    pub fn take_reset(&self) -> bool {
        self.reset_requested.swap(false, Ordering::Relaxed)
    }

    fn clear(&self) {
        let s = LoudnessSnapshot::SILENT;
        for (cell, value) in [
            (&self.momentary, s.momentary_lufs),
            (&self.short_term, s.short_term_lufs),
            (&self.integrated, s.integrated_lufs),
            (&self.range, s.range_lu),
            (&self.true_peak, s.true_peak_dbtp),
            (&self.max_momentary, s.max_momentary_lufs),
            (&self.max_short_term, s.max_short_term_lufs),
        ] {
            cell.store(value.to_bits(), Ordering::Relaxed);
        }
    }
}

/// Frames collected before they are handed to the meter in one call.
const CHUNK_FRAMES: usize = 256;

/// The callback's side of the loudness meter: it takes the stereo frames that are sent to the
/// device, feeds them to the meter and publishes the readings. It allocates nothing after
/// construction, so it may live inside the real-time audio callback.
pub struct LoudnessFeeder {
    meter: LoudnessMeter,
    readout: Arc<LoudnessReadout>,
    chunk: [f32; CHUNK_FRAMES * 2],
    filled: usize,
    since_slow: usize,
    slow_every: usize,
}

impl LoudnessFeeder {
    pub fn new(sample_rate: f64, readout: Arc<LoudnessReadout>) -> Self {
        Self {
            meter: LoudnessMeter::new(sample_rate, 2),
            readout,
            chunk: [0.0; CHUNK_FRAMES * 2],
            filled: 0,
            since_slow: 0,
            // One gating block is 100 ms
            slow_every: ((sample_rate / 10.0) as usize).max(1),
        }
    }

    /// Start of an audio buffer: restarts the measurement when the UI asked for it.
    pub fn begin_buffer(&mut self) {
        if self.readout.take_reset() {
            self.meter.reset();
            self.filled = 0;
            self.since_slow = 0;
        }
    }

    /// One stereo frame, exactly as it goes to the device.
    #[inline]
    pub fn push(&mut self, left: f32, right: f32) {
        self.chunk[self.filled] = left;
        self.chunk[self.filled + 1] = right;
        self.filled += 2;
        if self.filled == self.chunk.len() {
            self.meter.process_interleaved(&self.chunk);
            self.filled = 0;
        }
    }

    /// End of an audio buffer of `frames` frames: feeds the rest and publishes the readings.
    pub fn end_buffer(&mut self, frames: usize) {
        if self.filled > 0 {
            self.meter.process_interleaved(&self.chunk[..self.filled]);
            self.filled = 0;
        }
        self.readout.store_fast(&self.meter);
        self.since_slow += frames;
        if self.since_slow >= self.slow_every {
            self.since_slow = 0;
            self.readout.store_slow(&self.meter);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(freq: f64, amplitude: f64, rate: f64, seconds: f64) -> Vec<f32> {
        let n = (rate * seconds) as usize;
        let mut out = Vec::with_capacity(n * 2);
        for i in 0..n {
            let v =
                (amplitude * (2.0 * std::f64::consts::PI * freq * i as f64 / rate).sin()) as f32;
            out.push(v);
            out.push(v);
        }
        out
    }

    #[test]
    fn a_new_readout_reports_silence_not_made_up_numbers() {
        let snapshot = LoudnessReadout::new().snapshot();
        assert_eq!(snapshot, LoudnessSnapshot::SILENT);
        assert!(snapshot.momentary_lufs.is_infinite() && snapshot.true_peak_dbtp.is_infinite());
    }

    #[test]
    fn readings_come_from_the_meter_and_match_the_standard_calibration() {
        // BS.1770: a 997 Hz sine at -20 dBFS in both channels reads -20 LUFS
        let rate = 48_000.0;
        let mut meter = LoudnessMeter::new(rate, 2);
        meter.process_interleaved(&tone(997.0, 0.1, rate, 6.0));

        let readout = LoudnessReadout::new();
        readout.store_fast(&meter);
        readout.store_slow(&meter);
        let s = readout.snapshot();
        assert!((s.momentary_lufs - -20.0).abs() < 0.3, "{s:?}");
        assert!((s.short_term_lufs - -20.0).abs() < 0.3, "{s:?}");
        assert!((s.integrated_lufs - -20.0).abs() < 0.3, "{s:?}");
        assert!(
            s.range_lu < 0.5,
            "a steady tone has no loudness range: {s:?}"
        );
        assert!((s.true_peak_dbtp - -20.0).abs() < 0.5, "{s:?}");
    }

    #[test]
    fn a_reset_clears_the_readings_and_is_taken_by_the_callback_once() {
        let rate = 48_000.0;
        let mut meter = LoudnessMeter::new(rate, 2);
        meter.process_interleaved(&tone(997.0, 0.1, rate, 4.0));
        let readout = LoudnessReadout::new();
        readout.store_fast(&meter);
        readout.store_slow(&meter);
        assert!(readout.snapshot().integrated_lufs.is_finite());

        readout.request_reset();
        assert_eq!(readout.snapshot(), LoudnessSnapshot::SILENT);
        assert!(readout.take_reset());
        assert!(!readout.take_reset(), "the request is consumed");
    }

    #[test]
    fn the_feeder_publishes_what_it_is_given_buffer_by_buffer() {
        let rate = 48_000.0;
        let readout = Arc::new(LoudnessReadout::new());
        let mut feeder = LoudnessFeeder::new(rate, Arc::clone(&readout));
        let signal = tone(997.0, 0.1, rate, 5.0);
        // An odd buffer size, so chunks and buffers never line up
        for buffer in signal.chunks(2 * 441) {
            feeder.begin_buffer();
            for frame in buffer.chunks(2) {
                feeder.push(frame[0], frame[1]);
            }
            feeder.end_buffer(buffer.len() / 2);
        }
        let s = readout.snapshot();
        assert!((s.integrated_lufs - -20.0).abs() < 0.3, "{s:?}");
        assert!((s.momentary_lufs - -20.0).abs() < 0.3, "{s:?}");
    }

    #[test]
    fn the_feeder_restarts_the_measurement_on_request_and_nothing_else_changes() {
        let rate = 48_000.0;
        let readout = Arc::new(LoudnessReadout::new());
        let mut feeder = LoudnessFeeder::new(rate, Arc::clone(&readout));
        let loud = tone(997.0, 0.1, rate, 4.0);
        feeder.begin_buffer();
        for frame in loud.chunks(2) {
            feeder.push(frame[0], frame[1]);
        }
        feeder.end_buffer(loud.len() / 2);
        assert!(readout.snapshot().integrated_lufs > -25.0);

        readout.request_reset();
        // Quieter programme afterwards: -40 dBFS tone
        let quiet = tone(997.0, 0.01, rate, 4.0);
        feeder.begin_buffer();
        for frame in quiet.chunks(2) {
            feeder.push(frame[0], frame[1]);
        }
        feeder.end_buffer(quiet.len() / 2);
        let s = readout.snapshot();
        assert!(
            (s.integrated_lufs - -40.0).abs() < 0.5,
            "old loud audio leaked in: {s:?}"
        );
    }
}
