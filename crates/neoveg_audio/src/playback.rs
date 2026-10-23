//! Hardware audio output with cpal. The meter is fed from the samples actually sent to the device.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;

use crate::meter::PeakMeter;
use crate::mixer::{mix_frame, plan_end, MixClip};

/// Shared state between the UI thread and the audio callback.
struct PlaybackState {
    clips: Vec<MixClip>,
    end: f64,
    /// Timeline position in seconds (`f64` bits).
    position: AtomicU64,
    playing: AtomicBool,
    /// Master gain (`f64` bits).
    volume: AtomicU64,
}

/// Plays a mix of clips through the default output device.
pub struct AudioPlayer {
    // Kept alive for the duration of playback; dropping it closes the device.
    _stream: cpal::Stream,
    state: Arc<PlaybackState>,
    meter: Arc<PeakMeter>,
}

impl AudioPlayer {
    pub fn new(clips: Vec<MixClip>, meter: Arc<PeakMeter>) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("no audio output device")?;
        let supported = device
            .default_output_config()
            .map_err(|e| format!("output config: {e}"))?;
        if supported.sample_format() != SampleFormat::F32 {
            return Err(format!(
                "unsupported device sample format {:?}",
                supported.sample_format()
            ));
        }
        let config = supported.config();
        let step = 1.0 / config.sample_rate.0 as f64;
        let out_channels = config.channels as usize;

        let state = Arc::new(PlaybackState {
            end: plan_end(&clips),
            clips,
            position: AtomicU64::new(0f64.to_bits()),
            playing: AtomicBool::new(false),
            volume: AtomicU64::new(1.0f64.to_bits()),
        });

        let cb_state = Arc::clone(&state);
        let cb_meter = Arc::clone(&meter);

        let stream = device
            .build_output_stream(
                &config,
                move |data: &mut [f32], _| {
                    if !cb_state.playing.load(Ordering::Relaxed) {
                        data.fill(0.0);
                        cb_meter.reset();
                        return;
                    }
                    let volume = f64::from_bits(cb_state.volume.load(Ordering::Relaxed)) as f32;
                    let mut pos = f64::from_bits(cb_state.position.load(Ordering::Relaxed));
                    let (mut peak_l, mut peak_r) = (0.0f32, 0.0f32);

                    for frame in data.chunks_mut(out_channels) {
                        let (l, r) = if pos < cb_state.end {
                            let (l, r) = mix_frame(&cb_state.clips, pos);
                            ((l * volume).clamp(-1.0, 1.0), (r * volume).clamp(-1.0, 1.0))
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
        stream
            .play()
            .map_err(|e| format!("cannot start audio stream: {e}"))?;

        Ok(Self {
            _stream: stream,
            state,
            meter,
        })
    }

    /// Starts playback at timeline time `seconds`.
    pub fn play_from(&self, seconds: f64) {
        self.state
            .position
            .store(seconds.max(0.0).to_bits(), Ordering::Relaxed);
        self.state.playing.store(true, Ordering::Relaxed);
    }

    pub fn stop(&self) {
        self.state.playing.store(false, Ordering::Relaxed);
        self.meter.reset();
    }

    pub fn is_playing(&self) -> bool {
        self.state.playing.load(Ordering::Relaxed)
    }

    /// Current timeline position in seconds.
    pub fn position_seconds(&self) -> f64 {
        f64::from_bits(self.state.position.load(Ordering::Relaxed))
    }

    pub fn set_volume(&self, linear: f32) {
        self.state
            .volume
            .store((linear.max(0.0) as f64).to_bits(), Ordering::Relaxed);
    }
}
