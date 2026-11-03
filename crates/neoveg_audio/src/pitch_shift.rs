//! Pitch change of a clip with the formants kept where they were (or moved on purpose).
//!
//! The pitch moves without changing the length by stretching the sound with WSOLA (which keeps the
//! pitch) by the pitch ratio and then reading it back at that ratio. That also drags the formants
//! (the resonances that make a voice sound like itself) along: a voice taken up an octave sounds
//! small. Putting them back works frame by frame on the spectrum: the smooth envelope of the
//! original sound (its formants) is taken from a cepstrum, the envelope of the shifted sound is
//! taken the same way, and the shifted spectrum is multiplied by the ratio of the two, with the
//! original envelope moved along the frequency axis by the extra formant shift when there is one.
//!
//! All buffers are interleaved stereo `f32`, like the rest of [`crate::stretch`].

use neoveg_common::pitch_change::PitchChange;

use crate::dsp::fft::Fft;
use crate::stretch::{resample_stereo_linear, stretch_stereo};

/// Highest fundamental the cepstral envelope has to leave out (it keeps what varies slower than a
/// harmonic spacing of this many Hz).
const ENVELOPE_F0_HZ: f64 = 700.0;
/// Largest correction of one bin, as a factor (about 18 dB).
const MAX_GAIN: f32 = 8.0;

/// Moves the pitch of interleaved stereo `samples` by `change`, keeping the length. The formants
/// follow the pitch unless `change.preserve_formants` is set.
pub fn pitch_shift_stereo(samples: &[f32], sample_rate: u32, change: &PitchChange) -> Vec<f32> {
    let frames = samples.len() / 2;
    let samples = &samples[..frames * 2];
    if frames == 0 || !change.changes_sound() {
        return samples.to_vec();
    }
    let shifted = if change.semitones == 0.0 {
        samples.to_vec()
    } else {
        // Longer by the ratio at the same pitch, then read back faster (or slower) by the ratio
        let ratio = change.pitch_ratio();
        let longer = stretch_stereo(samples, sample_rate, 1.0 / ratio, true);
        resample_stereo_linear(&longer, frames)
    };
    if change.needs_formant_correction() {
        correct_formants(samples, &shifted, sample_rate, change.formant_ratio())
    } else {
        shifted
    }
}

/// FFT size for the formant correction: about 46 ms, a power of two.
fn fft_size(sample_rate: u32) -> usize {
    ((f64::from(sample_rate.max(8_000)) * 0.046) as usize)
        .next_power_of_two()
        .clamp(512, 8_192)
}

/// Smooth log-magnitude envelope of a spectrum (`bins` = `n / 2 + 1` values): the cepstrum of the
/// log magnitude, cut at the quefrency of the highest fundamental, back to the frequency axis.
struct Envelope {
    fft: Fft,
    n: usize,
    lifter: usize,
    re: Vec<f32>,
    im: Vec<f32>,
}

impl Envelope {
    fn new(n: usize, sample_rate: u32) -> Self {
        let lifter = ((f64::from(sample_rate) / ENVELOPE_F0_HZ) as usize).clamp(8, n / 4);
        Self {
            fft: Fft::new(n),
            n,
            lifter,
            re: vec![0.0; n],
            im: vec![0.0; n],
        }
    }

    /// The log envelope of the `n / 2 + 1` bins whose magnitudes are `mag`.
    fn log_envelope(&mut self, mag: &[f32], out: &mut [f32]) {
        let n = self.n;
        // The log magnitude of a real signal is even: mirror it
        for (k, m) in mag.iter().enumerate().take(n / 2 + 1) {
            let v = m.max(1e-9).ln();
            self.re[k] = v;
            self.re[(n - k) % n] = v;
        }
        self.im.iter_mut().for_each(|x| *x = 0.0);
        self.fft.inverse(&mut self.re, &mut self.im);
        let scale = 1.0 / n as f32;
        // Keep the low quefrencies (a short taper at the cut), drop the rest
        for q in 0..n {
            let dist = q.min(n - q);
            let weight = if dist <= self.lifter {
                let edge = self.lifter.saturating_sub(self.lifter / 4);
                if dist <= edge {
                    1.0
                } else {
                    let t = (dist - edge) as f32 / (self.lifter - edge).max(1) as f32;
                    0.5 * (1.0 + (std::f32::consts::PI * t).cos())
                }
            } else {
                0.0
            };
            self.re[q] *= scale * weight;
            self.im[q] = 0.0;
        }
        self.fft.forward(&mut self.re, &mut self.im);
        out[..=n / 2].copy_from_slice(&self.re[..=n / 2]);
    }
}

/// Puts the formants of `reference` (the sound before the pitch moved, the same length) on
/// `shifted`, moved up by `formant_ratio` (`1.0` leaves them where they were).
fn correct_formants(
    reference: &[f32],
    shifted: &[f32],
    sample_rate: u32,
    formant_ratio: f64,
) -> Vec<f32> {
    let frames = shifted.len() / 2;
    let n = fft_size(sample_rate);
    if frames < n {
        return shifted.to_vec();
    }
    let hop = n / 4;
    let fft = Fft::new(n);
    let mut envelope = Envelope::new(n, sample_rate);
    let window: Vec<f32> = (0..n)
        .map(|k| (0.5 * (1.0 - (2.0 * std::f64::consts::PI * k as f64 / n as f64).cos())) as f32)
        .collect();
    // Hann analysis and synthesis at a quarter hop add up to 1.5 everywhere
    let norm = 1.0 / 1.5;
    let bins = n / 2 + 1;
    let (mut xr, mut xi) = (vec![0.0f32; n], vec![0.0f32; n]);
    let (mut yr, mut yi) = (vec![0.0f32; n], vec![0.0f32; n]);
    let (mut mag_x, mut mag_y) = (vec![0.0f32; bins], vec![0.0f32; bins]);
    let (mut env_x, mut env_y) = (vec![0.0f32; bins], vec![0.0f32; bins]);
    let mut out = vec![0.0f32; shifted.len() + n * 2];
    let s = formant_ratio as f32;

    for channel in 0..2 {
        // The signal is read from `start - n/2 ..`: pad the first frames with silence by starting
        // before zero, so the first samples get their share of the overlap-add
        let mut first = -(3 * hop as isize);
        while (first + n as isize) < (frames + 3 * hop) as isize {
            let at = first;
            for k in 0..n {
                let i = at + k as isize;
                let (x, y) = if i >= 0 && (i as usize) < frames {
                    (
                        reference[i as usize * 2 + channel],
                        shifted[i as usize * 2 + channel],
                    )
                } else {
                    (0.0, 0.0)
                };
                xr[k] = x * window[k];
                xi[k] = 0.0;
                yr[k] = y * window[k];
                yi[k] = 0.0;
            }
            fft.forward(&mut xr, &mut xi);
            fft.forward(&mut yr, &mut yi);
            let mut energy = 0.0f32;
            for k in 0..bins {
                mag_x[k] = (xr[k] * xr[k] + xi[k] * xi[k]).sqrt();
                mag_y[k] = (yr[k] * yr[k] + yi[k] * yi[k]).sqrt();
                energy += mag_y[k];
            }
            if energy > 1e-6 {
                envelope.log_envelope(&mag_x, &mut env_x);
                envelope.log_envelope(&mag_y, &mut env_y);
                for k in 0..bins {
                    // The original envelope read where the formant now sits: bin k holds what was
                    // at k / s
                    let src = (k as f32 / s).clamp(0.0, (bins - 1) as f32);
                    let lo = src.floor() as usize;
                    let hi = (lo + 1).min(bins - 1);
                    let t = src - lo as f32;
                    let target = env_x[lo] + (env_x[hi] - env_x[lo]) * t;
                    let gain = (target - env_y[k]).exp().clamp(1.0 / MAX_GAIN, MAX_GAIN);
                    yr[k] *= gain;
                    yi[k] *= gain;
                    if k > 0 && k < n / 2 {
                        yr[n - k] *= gain;
                        yi[n - k] *= gain;
                    }
                }
            }
            fft.inverse(&mut yr, &mut yi);
            for k in 0..n {
                let i = at + k as isize;
                if i >= 0 && (i as usize) < frames {
                    out[i as usize * 2 + channel] += yr[k] / n as f32 * window[k] * norm;
                }
            }
            first += hop as isize;
        }
    }
    out.truncate(shifted.len());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    const SR: u32 = 44_100;

    /// A voice-like sound: harmonics of `f0` whose strength follows one resonance at `formant`.
    fn vowel(f0: f64, formant: f64, seconds: f64) -> Vec<f32> {
        let frames = (seconds * f64::from(SR)) as usize;
        let mut out = Vec::with_capacity(frames * 2);
        let harmonics: Vec<(f64, f64)> = (1..)
            .map(|h| h as f64 * f0)
            .take_while(|f| *f < 9000.0)
            .map(|f| {
                let d = (f - formant) / 450.0;
                (f, 0.05 + (-d * d).exp())
            })
            .collect();
        for i in 0..frames {
            let t = i as f64 / f64::from(SR);
            let v: f64 = harmonics
                .iter()
                .map(|(f, a)| a * (2.0 * PI * f * t).sin())
                .sum();
            let v = (v * 0.05) as f32;
            out.push(v);
            out.push(v);
        }
        out
    }

    /// Frequency of the strongest part of the spectrum envelope, in Hz: the centroid of the
    /// magnitude weighted by itself above 200 Hz (stable for a sound with one resonance).
    fn centroid(stereo: &[f32]) -> f64 {
        let n = 8192;
        let mid = stereo.len() / 4;
        let fft = Fft::new(n);
        let mut re: Vec<f32> = (0..n)
            .map(|k| {
                let w = 0.5 * (1.0 - (2.0 * PI * k as f64 / n as f64).cos());
                stereo[(mid + k) * 2] * w as f32
            })
            .collect();
        let mut im = vec![0.0f32; n];
        fft.forward(&mut re, &mut im);
        let mut sum = 0.0f64;
        let mut weighted = 0.0f64;
        for k in 1..n / 2 {
            let f = k as f64 * f64::from(SR) / n as f64;
            if f < 200.0 {
                continue;
            }
            let m = f64::from(re[k] * re[k] + im[k] * im[k]);
            sum += m;
            weighted += m * f;
        }
        weighted / sum
    }

    /// The fundamental of a harmonic sound: the spacing of its strongest spectral peaks (the
    /// strongest one alone can be a high harmonic next to the resonance).
    fn fundamental(stereo: &[f32]) -> f64 {
        let n = 32768;
        let mid = stereo.len() / 4;
        let fft = Fft::new(n);
        let mut re: Vec<f32> = (0..n)
            .map(|k| {
                let w = 0.5 * (1.0 - (2.0 * PI * k as f64 / n as f64).cos());
                stereo[(mid + k) * 2] * w as f32
            })
            .collect();
        let mut im = vec![0.0f32; n];
        fft.forward(&mut re, &mut im);
        let power = |k: usize| re[k] * re[k] + im[k] * im[k];
        let mut peaks: Vec<(f32, f64)> = (1..n / 2 - 1)
            .filter(|&k| power(k) > power(k - 1) && power(k) > power(k + 1))
            .map(|k| (power(k), k as f64 * f64::from(SR) / n as f64))
            .collect();
        peaks.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut top: Vec<f64> = peaks.iter().take(10).map(|p| p.1).collect();
        top.sort_by(f64::total_cmp);
        let mut gaps: Vec<f64> = top
            .windows(2)
            .map(|w| w[1] - w[0])
            .filter(|g| *g > 30.0)
            .collect();
        gaps.sort_by(f64::total_cmp);
        gaps[gaps.len() / 2]
    }

    #[test]
    fn nothing_to_change_returns_the_sound_as_it_is() {
        let sound = vowel(110.0, 900.0, 0.5);
        assert_eq!(
            pitch_shift_stereo(&sound, SR, &PitchChange::default()),
            sound
        );
        // The flag alone changes nothing
        let flag = PitchChange {
            preserve_formants: true,
            ..PitchChange::default()
        };
        assert_eq!(pitch_shift_stereo(&sound, SR, &flag), sound);
        assert!(pitch_shift_stereo(&[], SR, &flag).is_empty());
    }

    #[test]
    fn an_octave_up_doubles_the_pitch_and_keeps_the_length() {
        let sound = vowel(110.0, 900.0, 1.5);
        let up = PitchChange {
            semitones: 12.0,
            ..PitchChange::default()
        };
        let out = pitch_shift_stereo(&sound, SR, &up);
        assert_eq!(out.len(), sound.len());
        let f = fundamental(&out);
        assert!((f - 220.0).abs() < 8.0, "{f}");
        let down = PitchChange {
            semitones: -5.0,
            ..PitchChange::default()
        };
        // Five semitones down from 110 Hz is 82.4 Hz
        let f = fundamental(&pitch_shift_stereo(&sound, SR, &down));
        assert!((f - 82.4).abs() < 4.0, "{f}");
    }

    #[test]
    fn preserving_the_formants_keeps_the_timbre_that_the_plain_shift_drags_along() {
        let sound = vowel(110.0, 900.0, 2.0);
        let original = centroid(&sound);
        let plain = pitch_shift_stereo(
            &sound,
            SR,
            &PitchChange {
                semitones: 12.0,
                ..PitchChange::default()
            },
        );
        let kept = pitch_shift_stereo(
            &sound,
            SR,
            &PitchChange {
                semitones: 12.0,
                preserve_formants: true,
                ..PitchChange::default()
            },
        );
        assert_eq!(kept.len(), sound.len());
        let (c_plain, c_kept) = (centroid(&plain), centroid(&kept));
        // The plain shift moves the resonance up with the pitch; preserving it leaves it close
        assert!(c_plain > original * 1.4, "{original} {c_plain}");
        assert!(c_kept < c_plain * 0.85, "{c_kept} {c_plain}");
        assert!(
            (c_kept / original - 1.0).abs() < 0.35,
            "{original} {c_kept}"
        );
        // And the pitch still moved
        assert!((fundamental(&kept) - 220.0).abs() < 8.0);
    }

    #[test]
    fn a_formant_shift_alone_moves_the_timbre_and_leaves_the_pitch() {
        let sound = vowel(110.0, 900.0, 2.0);
        let deeper = pitch_shift_stereo(
            &sound,
            SR,
            &PitchChange {
                preserve_formants: true,
                formant_shift: -7.0,
                ..PitchChange::default()
            },
        );
        assert_eq!(deeper.len(), sound.len());
        assert!(centroid(&deeper) < centroid(&sound) * 0.9);
        assert!((fundamental(&deeper) - 110.0).abs() < 5.0);
    }

    #[test]
    fn short_and_silent_input_never_panics() {
        let change = PitchChange {
            semitones: 7.0,
            preserve_formants: true,
            formant_shift: 2.0,
        };
        for frames in [1usize, 10, 300, 5_000] {
            let out = pitch_shift_stereo(&vec![0.0; frames * 2], SR, &change);
            assert_eq!(out.len(), frames * 2);
            assert!(out.iter().all(|v| v.is_finite()));
        }
        // Noise through the whole chain stays finite and inside a sane range
        let noise: Vec<f32> = (0..SR as usize * 2)
            .map(|i| (((i as u64 * 2_654_435_761) % 2000) as f32 / 1000.0 - 1.0) * 0.3)
            .collect();
        let out = pitch_shift_stereo(&noise, SR, &change);
        assert!(out.iter().all(|v| v.is_finite() && v.abs() < 4.0));
    }
}
