//! Hardware audio output with cpal. The peak and loudness meters are fed from the samples actually
//! sent to the device.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;

use crate::fxmixer::{BlockMixer, FxPlan, MAX_BLOCK};
use crate::meter::PeakMeter;
use crate::mixer::MixClip;
use crate::readout::{LoudnessFeeder, LoudnessReadout};

/// Shared state between the UI thread and the audio callback.
struct PlaybackState {
    end: f64,
    /// Timeline position in seconds (`f64` bits).
    position: AtomicU64,
    playing: AtomicBool,
    /// Master gain (`f64` bits).
    volume: AtomicU64,
    /// Loop start and end in seconds (`f64` bits); the loop is off while `end <= start`.
    loop_start: AtomicU64,
    loop_end: AtomicU64,
}

/// Where `pos` lands after a loop `[start, end)` is applied: positions at or past `end` wrap to
/// the start of the loop, keeping the overshoot so playback stays sample-accurate. A loop that is
/// empty or reversed leaves `pos` unchanged.
pub fn wrap_loop(pos: f64, start: f64, end: f64) -> f64 {
    let length = end - start;
    if length <= 0.0 || pos < end {
        return pos;
    }
    start + (pos - end) % length
}

/// Plays a mix of clips through the default output device.
pub struct AudioPlayer {
    // Kept alive for the duration of playback; dropping it closes the device.
    _stream: cpal::Stream,
    state: Arc<PlaybackState>,
    meter: Arc<PeakMeter>,
    loudness: Arc<LoudnessReadout>,
}

impl AudioPlayer {
    pub fn new(
        clips: Vec<MixClip>,
        plan: &FxPlan,
        meter: Arc<PeakMeter>,
        loudness: Arc<LoudnessReadout>,
    ) -> Result<Self, String> {
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

        // Built here, outside the callback: the mixer allocates its buses and creates the effects
        let mut mixer = BlockMixer::new(clips, plan, config.sample_rate.0 as f32);
        let state = Arc::new(PlaybackState {
            end: mixer.end(),
            position: AtomicU64::new(0f64.to_bits()),
            playing: AtomicBool::new(false),
            volume: AtomicU64::new(1.0f64.to_bits()),
            loop_start: AtomicU64::new(0f64.to_bits()),
            loop_end: AtomicU64::new(0f64.to_bits()),
        });

        let cb_state = Arc::clone(&state);
        let cb_meter = Arc::clone(&meter);
        // Built here, outside the callback: the meter allocates its gating histograms once
        let mut feeder =
            LoudnessFeeder::new(f64::from(config.sample_rate.0), Arc::clone(&loudness));

        let stream = device
            .build_output_stream(
                &config,
                move |data: &mut [f32], _| {
                    if !cb_state.playing.load(Ordering::Relaxed) {
                        data.fill(0.0);
                        cb_meter.reset();
                        return;
                    }
                    feeder.begin_buffer();
                    let volume = f64::from_bits(cb_state.volume.load(Ordering::Relaxed)) as f32;
                    let mut pos = f64::from_bits(cb_state.position.load(Ordering::Relaxed));
                    let loop_start = f64::from_bits(cb_state.loop_start.load(Ordering::Relaxed));
                    let loop_end = f64::from_bits(cb_state.loop_end.load(Ordering::Relaxed));
                    let (mut peak_l, mut peak_r) = (0.0f32, 0.0f32);

                    // The buffer is cut into blocks: every block is mixed per frame (positions,
                    // loop and end of the programme), then the effect chains run on the blocks
                    for block in data.chunks_mut(out_channels * MAX_BLOCK) {
                        let frames = block.len() / out_channels;
                        mixer.begin(frames);
                        for i in 0..frames {
                            pos = wrap_loop(pos, loop_start, loop_end);
                            if pos < cb_state.end {
                                mixer.add_frame(i, pos);
                            } else {
                                cb_state.playing.store(false, Ordering::Relaxed);
                            }
                            pos += step;
                        }
                        mixer.finish(frames);
                        let (mix_l, mix_r) = mixer.out();
                        for (i, frame) in block.chunks_mut(out_channels).enumerate() {
                            let l = (mix_l[i] * volume).clamp(-1.0, 1.0);
                            let r = (mix_r[i] * volume).clamp(-1.0, 1.0);
                            for (c, out) in frame.iter_mut().enumerate() {
                                *out = match c {
                                    0 => l,
                                    1 => r,
                                    _ => 0.0,
                                };
                            }
                            feeder.push(l, r);
                            peak_l = peak_l.max(l.abs());
                            peak_r = peak_r.max(r.abs());
                        }
                    }

                    cb_state.position.store(pos.to_bits(), Ordering::Relaxed);
                    cb_meter.set(peak_l, peak_r);
                    feeder.end_buffer(data.len() / out_channels.max(1));
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
            loudness,
        })
    }

    /// Starts playback at timeline time `seconds`.
    pub fn play_from(&self, seconds: f64) {
        // Every start is a new measurement of the programme
        self.loudness.request_reset();
        self.state
            .position
            .store(seconds.max(0.0).to_bits(), Ordering::Relaxed);
        self.state.playing.store(true, Ordering::Relaxed);
    }

    /// Loops playback over `range` (seconds), or plays through to the end with `None`. Safe to
    /// call while playing.
    pub fn set_loop(&self, range: Option<(f64, f64)>) {
        let (start, end) = range.unwrap_or((0.0, 0.0));
        self.state
            .loop_start
            .store(start.max(0.0).to_bits(), Ordering::Relaxed);
        self.state.loop_end.store(end.to_bits(), Ordering::Relaxed);
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

#[cfg(test)]
mod tests {
    use super::wrap_loop;

    #[test]
    fn positions_before_the_end_are_untouched() {
        assert_eq!(wrap_loop(1.0, 2.0, 5.0), 1.0);
        assert_eq!(wrap_loop(4.999, 2.0, 5.0), 4.999);
    }

    #[test]
    fn reaching_the_end_wraps_to_the_start_keeping_the_overshoot() {
        assert_eq!(wrap_loop(5.0, 2.0, 5.0), 2.0);
        assert!((wrap_loop(5.25, 2.0, 5.0) - 2.25).abs() < 1e-12);
    }

    #[test]
    fn a_huge_overshoot_still_lands_inside_the_loop() {
        let p = wrap_loop(100.0, 2.0, 5.0);
        assert!((2.0..5.0).contains(&p), "{p}");
    }

    #[test]
    fn empty_or_reversed_loops_are_off() {
        assert_eq!(wrap_loop(9.0, 0.0, 0.0), 9.0);
        assert_eq!(wrap_loop(9.0, 5.0, 2.0), 9.0);
    }
}
