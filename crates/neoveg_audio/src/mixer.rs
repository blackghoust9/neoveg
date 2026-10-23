//! Multi-track mixer: sums every clip active at a timeline position into one stereo frame.

use std::sync::Arc;

use neoveg_common::fade::FadeCurve;

/// One audio event prepared for playback.
#[derive(Debug, Clone, Default)]
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
    pub fade_in_curve: FadeCurve,
    pub fade_out_curve: FadeCurve,
    /// Length of the automatic crossfade at the start (0 = none); uses `cross_in_curve`.
    pub cross_in: f64,
    pub cross_in_curve: FadeCurve,
    /// Length of the automatic crossfade at the end (0 = none); uses `cross_out_curve`.
    pub cross_out: f64,
    pub cross_out_curve: FadeCurve,
}

/// Gain of a ramp `length` seconds long when `position` seconds are left until its full-gain end
/// (distance from the clip start for a fade-in, remaining time for a fade-out).
fn ramp(curve: FadeCurve, length: f64, position: f64) -> f32 {
    if length > 0.0 && position < length {
        curve.gain(position / length) as f32
    } else {
        1.0
    }
}

impl MixClip {
    pub fn end(&self) -> f64 {
        self.start + self.duration
    }

    /// Gain at `local` seconds from the clip start: own fades times automatic crossfades.
    fn envelope(&self, local: f64) -> f32 {
        let remaining = self.duration - local;
        self.gain
            * ramp(self.fade_in_curve, self.fade_in, local)
            * ramp(self.fade_out_curve, self.fade_out, remaining.max(0.0))
            * ramp(self.cross_in_curve, self.cross_in, local)
            * ramp(self.cross_out_curve, self.cross_out, remaining.max(0.0))
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
            gain: 1.0,
            ..Default::default()
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
    fn each_curve_shapes_the_fade_in() {
        // quarter of the way through a 1 s fade-in
        let expected = [
            (FadeCurve::Linear, 0.25),
            (FadeCurve::Fast, 0.4375),
            (FadeCurve::Slow, 0.0625),
            (FadeCurve::Smooth, 0.156_25),
        ];
        for (curve, gain) in expected {
            let mut c = clip(1.0, 0.0, 2.0);
            c.fade_in = 1.0;
            c.fade_in_curve = curve;
            assert!((c.sample_at(0.25).0 - gain).abs() < 1e-5, "{curve:?}");
        }
        let mut sharp = clip(1.0, 0.0, 2.0);
        sharp.fade_in = 1.0;
        sharp.fade_in_curve = FadeCurve::Sharp;
        assert!(sharp.sample_at(0.25).0 > 0.25);
    }

    #[test]
    fn fade_out_mirrors_the_curve_at_the_end() {
        let mut c = clip(1.0, 0.0, 1.0);
        c.fade_out = 0.5;
        c.fade_out_curve = FadeCurve::Slow;
        // 0.25 s before the end: remaining fraction 0.5, slow curve gives 0.25
        assert!((c.sample_at(0.75).0 - 0.25).abs() < 1e-5);
        assert_eq!(c.sample_at(0.4).0, 1.0);
    }

    #[test]
    fn crossfade_of_symmetric_curves_keeps_constant_amplitude() {
        // A (0..1) fades out over 0.4..1.0 while B (0.4..1.4) fades in
        for curve in [FadeCurve::Linear, FadeCurve::Smooth, FadeCurve::Sharp] {
            let a = MixClip {
                cross_out: 0.6,
                cross_out_curve: curve,
                ..clip(1.0, 0.0, 1.0)
            };
            let b = MixClip {
                cross_in: 0.6,
                cross_in_curve: curve,
                ..clip(1.0, 0.4, 1.0)
            };
            let clips = [a, b];
            for i in 0..60 {
                let t = 0.4 + 0.6 * f64::from(i) / 60.0;
                let (l, _) = mix_frame(&clips, t);
                assert!((l - 1.0).abs() < 0.02, "{curve:?} at {t}: {l}");
            }
        }
    }

    #[test]
    fn crossfade_is_inactive_outside_the_overlap() {
        let a = MixClip {
            cross_out: 0.6,
            ..clip(1.0, 0.0, 1.0)
        };
        assert_eq!(a.sample_at(0.2).0, 1.0);
        assert!(a.sample_at(0.9).0 < 0.5);
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
