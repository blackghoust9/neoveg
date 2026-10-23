//! Hardware audio output with cpal. The meter is fed from the samples actually sent to the device.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;

use crate::meter::PeakMeter;

/// Shared state between the UI thread and the audio callback.
struct PlaybackState {
    /// Interleaved stereo `f32` at `source_rate`.
    samples: Arc<Vec<f32>>,
    source_rate: u32,
    /// Fractional source-frame position (`f64` bits).
    position: AtomicU64,
    playing: AtomicBool,
    volume: AtomicU64,
}

/// Plays one stereo buffer through the default output device (linear resampling to the device rate).
pub struct AudioPlayer {
    // Kept alive for the duration of playback; dropping it closes the device.
    _stream: cpal::Stream,
    state: Arc<PlaybackState>,
    meter: Arc<PeakMeter>,
}

impl AudioPlayer {
    pub fn new(samples: Arc<Vec<f32>>, source_rate: u32, meter: Arc<PeakMeter>) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or("no audio output device")?;
        let supported = device.default_output_config().map_err(|e| format!("output config: {e}"))?;
        if supported.sample_format() != SampleFormat::F32 {
            return Err(format!("unsupported device sample format {:?}", supported.sample_format()));
        }
        let config = supported.config();
        let device_rate = config.sample_rate.0 as f64;
        let out_channels = config.channels as usize;

        let state = Arc::new(PlaybackState {
            samples,
            source_rate,
            position: AtomicU64::new(0f64.to_bits()),
            playing: AtomicBool::new(false),
            volume: AtomicU64::new(1.0f64.to_bits()),
        });

        let cb_state = Arc::clone(&state);
        let cb_meter = Arc::clone(&meter);
        let step = source_rate as f64 / device_rate;

        let stream = device
            .build_output_stream(
                &config,
                move |data: &mut [f32], _| {
                    if !cb_state.playing.load(Ordering::Relaxed) {
                        data.fill(0.0);
                        cb_meter.reset();
                        return;
                    }
                    let total = cb_state.samples.len() / 2;
                    let volume = f64::from_bits(cb_state.volume.load(Ordering::Relaxed)) as f32;
                    let mut pos = f64::from_bits(cb_state.position.load(Ordering::Relaxed));
                    let (mut peak_l, mut peak_r) = (0.0f32, 0.0f32);

                    for frame in data.chunks_mut(out_channels) {
                        let i = pos.floor() as usize;
                        let (l, r) = if i + 1 < total {
                            let t = (pos - i as f64) as f32;
                            let s = &cb_state.samples;
                            (
                                (s[i * 2] + (s[(i + 1) * 2] - s[i * 2]) * t) * volume,
                                (s[i * 2 + 1] + (s[(i + 1) * 2 + 1] - s[i * 2 + 1]) * t) * volume,
                            )
                        } else {
                            cb_state.playing.store(false, Ordering::Relaxed);
                            (0.0, 0.0)
                        };
                        for (c, out) in frame.iter_mut().enumerate() {
                            *out = match c {
                                0 => l,
                                1 => r,
                                _ => 0.0,
                            };
                        }
                        peak_l = peak_l.max(l.abs());
                        peak_r = peak_r.max(r.abs());
                        pos += step;
                    }

                    cb_state.position.store(pos.to_bits(), Ordering::Relaxed);
                    cb_meter.set(peak_l, peak_r);
                },
                |e| tracing::error!("audio stream error: {e}"),
                None,
            )
            .map_err(|e| format!("cannot open audio stream: {e}"))?;
        stream.play().map_err(|e| format!("cannot start audio stream: {e}"))?;

        Ok(Self { _stream: stream, state, meter })
    }

    /// Starts playback `seconds` into the buffer.
    pub fn play_from(&self, seconds: f64) {
        let frame = (seconds.max(0.0) * self.state.source_rate as f64).floor();
        self.state.position.store(frame.to_bits(), Ordering::Relaxed);
        self.state.playing.store(true, Ordering::Relaxed);
    }

    pub fn stop(&self) {
        self.state.playing.store(false, Ordering::Relaxed);
        self.meter.reset();
    }

    pub fn is_playing(&self) -> bool {
        self.state.playing.load(Ordering::Relaxed)
    }

    /// Current position in seconds of the source buffer.
    pub fn position_seconds(&self) -> f64 {
        f64::from_bits(self.state.position.load(Ordering::Relaxed)) / self.state.source_rate as f64
    }

    pub fn set_volume(&self, linear: f32) {
        self.state.volume.store((linear.max(0.0) as f64).to_bits(), Ordering::Relaxed);
    }
}
