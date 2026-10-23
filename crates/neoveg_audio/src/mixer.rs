//! Multi-track mixer: sums every clip active at a timeline position into one stereo frame.

use std::sync::Arc;

/// One audio event prepared for playback.
#[derive(Debug, Clone)]
pub struct MixClip {
    /// Interleaved stereo `f32` at `source_rate`.
    pub samples: Arc<Vec<f32>>,
    pub source_rate: u32,
    /// Timeline position (seconds) where the clip starts.
    pub start: f64,
    pub duration: f64,
    /// Offset (seconds) into the source where the clip begins.
    pub trim_in: f64,
    pub gain: f32,
    pub fade_in: f64,
    pub fade_out: f64,
}

impl MixClip {
    pub fn end(&self) -> f64 {
        self.start + self.duration
    }

    /// Fade/gain multiplier at `local` seconds from the clip start.
    fn envelope(&self, local: f64) -> f32 {
        let mut g = self.gain;
        if self.fade_in > 0.0 && local < self.fade_in {
            g *= (local / self.fade_in) as f32;
        }
        let remaining = self.duration - local;
        if self.fade_out > 0.0 && remaining < self.fade_out {
            g *= (remaining / self.fade_out).max(0.0) as f32;
        }
        g
    }

    /// Stereo sample at timeline time `t` (linear interpolation, silence outside the clip).
    pub fn sample_at(&self, t: f64) -> (f32, f32) {
        if t < self.start || t >= self.end() {
            return (0.0, 0.0);
        }
        let local = t - self.start;
        let pos = (local + self.trim_in) * self.source_rate as f64;
        if pos < 0.0 {
            return (0.0, 0.0);
        }
        let total = self.samples.len() / 2;
        let i = pos.floor() as usize;
        if i >= total {
            return (0.0, 0.0);
        }
        let s = &self.samples;
        let (l0, r0) = (s[i * 2], s[i * 2 + 1]);
        let (l, r) = if i + 1 < total {
            let f = (pos - i as f64) as f32;
            (
                l0 + (s[(i + 1) * 2] - l0) * f,
                r0 + (s[(i + 1) * 2 + 1] - r0) * f,
            )
        } else {
            (l0, r0)
        };
        let g = self.envelope(local);
        (l * g, r * g)
    }
}

/// Sum of every clip at timeline time `t`.
pub fn mix_frame(clips: &[MixClip], t: f64) -> (f32, f32) {
    clips.iter().fold((0.0, 0.0), |(l, r), clip| {
        let (cl, cr) = clip.sample_at(t);
        (l + cl, r + cr)
    })
}

/// End of the last clip.
pub fn plan_end(clips: &[MixClip]) -> f64 {
    clips.iter().map(MixClip::end).fold(0.0, f64::max)
}

/// Renders `out.len() / 2` stereo frames starting at `t0`, advancing `step` seconds per frame.
pub fn render(clips: &[MixClip], t0: f64, step: f64, out: &mut [f32]) {
    for (i, frame) in out.as_chunks_mut::<2>().0.iter_mut().enumerate() {
        let (l, r) = mix_frame(clips, t0 + i as f64 * step);
        frame[0] = l;
        frame[1] = r;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(value: f32, start: f64, duration: f64) -> MixClip {
        // 1 second at 100 Hz of constant stereo value
        MixClip {
            samples: Arc::new(vec![value; 200]),
            source_rate: 100,
            start,
            duration,
            trim_in: 0.0,
            gain: 1.0,
            fade_in: 0.0,
            fade_out: 0.0,
        }
    }

    #[test]
    fn silence_outside_the_clip() {
        let c = clip(0.5, 1.0, 0.5);
        assert_eq!(c.sample_at(0.99), (0.0, 0.0));
        assert_eq!(c.sample_at(1.5), (0.0, 0.0));
        assert_eq!(c.sample_at(1.2), (0.5, 0.5));
    }

    #[test]
    fn overlapping_clips_are_summed() {
        let clips = [clip(0.25, 0.0, 1.0), clip(0.5, 0.0, 1.0)];
        assert_eq!(mix_frame(&clips, 0.5), (0.75, 0.75));
        assert_eq!(plan_end(&clips), 1.0);
    }

    #[test]
    fn fades_ramp_linearly_to_silence() {
        let mut c = clip(1.0, 0.0, 1.0);
        c.fade_in = 0.5;
        c.fade_out = 0.5;
        assert_eq!(c.sample_at(0.0).0, 0.0);
        assert!((c.sample_at(0.25).0 - 0.5).abs() < 1e-6);
        assert_eq!(c.sample_at(0.5).0, 1.0);
        assert!((c.sample_at(0.75).0 - 0.5).abs() < 1e-6);
    }

    #[test]
    fn trim_in_offsets_into_the_source() {
        let mut samples = vec![0.0f32; 200];
        samples[100..].fill(0.8); // second half of the source is loud
        let c = MixClip {
            samples: Arc::new(samples),
            trim_in: 0.5,
            ..clip(0.0, 2.0, 0.5)
        };
        assert!((c.sample_at(2.0).0 - 0.8).abs() < 1e-6);
    }

    #[test]
    fn render_fills_interleaved_frames() {
        let c = [clip(0.5, 0.0, 1.0)];
        let mut out = [0.0f32; 8];
        render(&c, 0.0, 0.01, &mut out);
        assert!(out.iter().all(|&v| v == 0.5));
    }
}
