//! Time-stretching for stereo PCM audio: pitch-preserving WSOLA (Waveform Similarity Overlap-Add)
//! and varispeed linear resampling.
//!
//! `rate` is the playback speed factor (`1.0` = original speed, `0.5` = half speed / twice as
//! long, `2.0` = double speed / half as long), clamped to [`MIN_STRETCH_RATE`]..=[`MAX_STRETCH_RATE`].
//! All buffers are interleaved stereo `f32` (`[L0, R0, L1, R1, ...]`).

use std::f64::consts::PI;
use std::sync::Arc;

use crate::pcm::PcmStore;

/// Slowest supported playback rate (`0.25x` = 400% duration).
pub const MIN_STRETCH_RATE: f64 = 0.25;
/// Fastest supported playback rate (`4.0x` = 25% duration).
pub const MAX_STRETCH_RATE: f64 = 4.0;

/// Clamps `rate` into [`MIN_STRETCH_RATE`]..=[`MAX_STRETCH_RATE`], treating non-finite or
/// non-positive values as `1.0`.
#[inline]
pub fn clamp_rate(rate: f64) -> f64 {
    if !rate.is_finite() || rate <= 0.0 {
        1.0
    } else {
        rate.clamp(MIN_STRETCH_RATE, MAX_STRETCH_RATE)
    }
}

/// `true` when `rate` is close enough to `1.0` that no stretching or resampling is needed.
#[inline]
pub fn is_unity_rate(rate: f64) -> bool {
    (clamp_rate(rate) - 1.0).abs() < 1e-6
}

/// Stretches or compresses interleaved stereo `samples` at `sample_rate` by playback speed `rate`.
///
/// When `preserve_pitch` is `true`, uses WSOLA (Verhelst & Roelands, 1993) with a 40 ms periodic
/// Hann window (50% overlap, exact Constant Overlap-Add) and normalized cross-correlation grain
/// alignment so the original pitch is preserved. When `preserve_pitch` is `false`, uses linear
/// interpolation (tape-style varispeed, where pitch scales proportionally with `rate`).
pub fn stretch_stereo(
    samples: &[f32],
    sample_rate: u32,
    rate: f64,
    preserve_pitch: bool,
) -> Vec<f32> {
    let in_frames = samples.len() / 2;
    if in_frames == 0 {
        return Vec::new();
    }
    let rate = clamp_rate(rate);
    if (rate - 1.0).abs() < 1e-6 {
        return samples[..in_frames * 2].to_vec();
    }
    let out_frames = ((in_frames as f64) / rate).round().max(1.0) as usize;
    if !preserve_pitch {
        return resample_stereo_linear(&samples[..in_frames * 2], out_frames);
    }

    let sr = sample_rate.max(8_000) as f64;
    let target_win = ((sr * 0.040).round() as usize / 2 * 2).clamp(64, 4_096);
    let win_frames = if in_frames >= target_win * 2 {
        target_win
    } else {
        (in_frames / 4) * 2
    };
    if win_frames < 64 {
        return resample_stereo_linear(&samples[..in_frames * 2], out_frames);
    }

    wsola_stereo(&samples[..in_frames * 2], sr, rate, win_frames, out_frames)
}

/// Extracts only the source region needed for a timeline clip (`trim_in` seconds, playing for
/// `timeline_duration` seconds at speed `rate`) and stretches it into a self-contained stereo
/// buffer whose `0..timeline_duration` maps `1:1` to timeline local time.
///
/// Slicing before stretching keeps editing responsive even on hour-long recordings.
pub fn stretch_clip_slice(
    samples: &PcmStore,
    source_rate: u32,
    trim_in: f64,
    timeline_duration: f64,
    rate: f64,
    preserve_pitch: bool,
) -> Arc<PcmStore> {
    let total_frames = samples.frames();
    if total_frames == 0 || timeline_duration <= 0.0 || source_rate == 0 {
        return PcmStore::shared(Vec::new());
    }
    let rate = clamp_rate(rate);
    let sr = f64::from(source_rate);
    let start_frame = ((trim_in.max(0.0) * sr).floor() as usize).min(total_frames);
    if start_frame >= total_frames {
        return PcmStore::shared(Vec::new());
    }
    let needed_source_seconds = timeline_duration * rate;
    // Extra 50 ms tail gives the last WSOLA grain real lookahead samples when available.
    let margin_frames = if preserve_pitch {
        (sr * 0.050).round() as usize
    } else {
        2
    };
    let source_frames = ((needed_source_seconds * sr).ceil() as usize)
        .max(1)
        .min(total_frames - start_frame);
    let slice_end = (start_frame + source_frames + margin_frames).min(total_frames);
    // Only the played part is read, wherever the sound is kept
    let slice = samples.to_vec(start_frame, slice_end - start_frame);

    let target_out_frames = ((source_frames as f64) / rate).round().max(1.0) as usize;
    let mut stretched = stretch_stereo(&slice, source_rate, rate, preserve_pitch);
    if stretched.len() > target_out_frames * 2 {
        stretched.truncate(target_out_frames * 2);
    }
    PcmStore::shared(stretched)
}

/// Linear stereo resampling (tape-speed varispeed or fallback for sub-window clips).
fn resample_stereo_linear(samples: &[f32], out_frames: usize) -> Vec<f32> {
    let in_frames = samples.len() / 2;
    if in_frames == 0 || out_frames == 0 {
        return Vec::new();
    }
    if in_frames == 1 {
        return samples[..2].repeat(out_frames);
    }
    let mut out = Vec::with_capacity(out_frames * 2);
    let scale = (in_frames - 1) as f64 / (out_frames.saturating_sub(1).max(1)) as f64;
    for i in 0..out_frames {
        let pos = (i as f64 * scale).min((in_frames - 1) as f64);
        let idx = pos.floor() as usize;
        let frac = (pos - idx as f64) as f32;
        let (l0, r0) = (samples[idx * 2], samples[idx * 2 + 1]);
        if idx + 1 < in_frames {
            let l1 = samples[(idx + 1) * 2];
            let r1 = samples[(idx + 1) * 2 + 1];
            out.push(l0 + (l1 - l0) * frac);
            out.push(r0 + (r1 - r0) * frac);
        } else {
            out.push(l0);
            out.push(r0);
        }
    }
    out
}

/// Pitch-preserving stereo WSOLA with periodic Hann window (`w[k] + w[k + Hs] == 1.0`).
fn wsola_stereo(
    samples: &[f32],
    sample_rate: f64,
    rate: f64,
    win_frames: usize,
    out_frames: usize,
) -> Vec<f32> {
    let in_frames = samples.len() / 2;
    let hop_s = win_frames / 2;
    let max_start = in_frames - win_frames;
    let search_radius = ((sample_rate * 0.015).round() as isize).clamp(16, hop_s as isize);

    // Mono downmix used only for phase-coherent grain similarity search across both channels.
    let mut mono = Vec::with_capacity(in_frames);
    for &[l, r] in samples.as_chunks::<2>().0 {
        mono.push(0.5 * (l + r));
    }

    // Periodic Hann window of even length W: w[k] + w[k + W/2] == 1.0 identically.
    let hann: Vec<f32> = (0..win_frames)
        .map(|k| (0.5 * (1.0 - (2.0 * PI * k as f64 / win_frames as f64).cos())) as f32)
        .collect();

    let alloc_frames = out_frames + win_frames;
    let mut out = vec![0.0f32; alloc_frames * 2];

    // Grain 0 starts at input frame 0; keep unity gain on the leading half-window so the attack
    // at t = 0 is never dulled by a half-window fade-in.
    let mut prev_pos: usize = 0;
    for k in 0..win_frames {
        let w = if k < hop_s { 1.0 } else { hann[k] };
        out[k * 2] = samples[k * 2] * w;
        out[k * 2 + 1] = samples[k * 2 + 1] * w;
    }

    let mut grain_idx: usize = 1;
    loop {
        let out_pos = grain_idx * hop_s;
        if out_pos >= out_frames {
            // Restore unity on the already-written tail of the previous grain so the end does not
            // droop when no further overlapping grain is added.
            let prev_tail_start = (grain_idx - 1) * hop_s + hop_s;
            let end_limit = (prev_tail_start + hop_s).min(out_frames);
            for out_f in prev_tail_start..end_limit {
                let k = out_f - prev_tail_start;
                let src_f = prev_pos + hop_s + k;
                if src_f < in_frames {
                    out[out_f * 2] = samples[src_f * 2];
                    out[out_f * 2 + 1] = samples[src_f * 2 + 1];
                }
            }
            break;
        }

        let natural_next = (prev_pos + hop_s).min(max_start);
        let nominal = ((grain_idx as f64 * hop_s as f64 * rate).round() as isize)
            .clamp(0, max_start as isize);
        let lo = (nominal - search_radius).max(0) as usize;
        let hi = (nominal + search_radius).min(max_start as isize) as usize;

        let chosen = best_wsola_offset(&mono, natural_next, lo, hi, nominal as usize, hop_s);
        let is_last = out_pos + hop_s >= out_frames;

        for (k, &hk) in hann.iter().enumerate() {
            let dst = out_pos + k;
            if dst >= alloc_frames {
                break;
            }
            let src = chosen + k;
            let w = if is_last && k >= hop_s { 1.0 } else { hk };
            out[dst * 2] += samples[src * 2] * w;
            out[dst * 2 + 1] += samples[src * 2 + 1] * w;
        }

        prev_pos = chosen;
        grain_idx += 1;
    }

    out.truncate(out_frames * 2);
    out
}

/// Finds the candidate start in `lo..=hi` whose leading `hop_s` samples best correlate with the
/// natural continuation `mono[natural_next .. natural_next + hop_s]`, using a two-stage coarse/fine
/// normalized cross-correlation search.
fn best_wsola_offset(
    mono: &[f32],
    natural_next: usize,
    lo: usize,
    hi: usize,
    nominal: usize,
    hop_s: usize,
) -> usize {
    if lo >= hi {
        return lo;
    }
    let ref_slice = &mono[natural_next..natural_next + hop_s];
    let ref_energy: f32 = ref_slice.iter().map(|&x| x * x).sum();
    if ref_energy < 1e-10 {
        return nominal.clamp(lo, hi);
    }

    let c_step = if hi - lo >= 64 { 4 } else { 1 };
    let k_step = if hop_s >= 128 { 2 } else { 1 };

    let mut best_pos = nominal.clamp(lo, hi);
    let mut best_score = f32::NEG_INFINITY;

    let mut cand = lo;
    while cand <= hi {
        let score = normalized_corr_strided(ref_slice, &mono[cand..cand + hop_s], k_step);
        if score > best_score {
            best_score = score;
            best_pos = cand;
        }
        cand += c_step;
    }
    if natural_next >= lo && natural_next <= hi {
        let score =
            normalized_corr_strided(ref_slice, &mono[natural_next..natural_next + hop_s], k_step);
        if score >= best_score {
            best_pos = natural_next;
        }
    }

    if c_step == 1 && k_step == 1 {
        return best_pos;
    }

    let fine_lo = best_pos.saturating_sub(c_step).max(lo);
    let fine_hi = (best_pos + c_step).min(hi);
    let mut fine_best = best_pos;
    let mut fine_score = f32::NEG_INFINITY;
    for pos in fine_lo..=fine_hi {
        let score = normalized_corr_strided(ref_slice, &mono[pos..pos + hop_s], 1);
        if score > fine_score {
            fine_score = score;
            fine_best = pos;
        }
    }
    fine_best
}

#[inline]
fn normalized_corr_strided(a: &[f32], b: &[f32], step: usize) -> f32 {
    let mut dot = 0.0f32;
    let mut ea = 0.0f32;
    let mut eb = 0.0f32;
    let mut i = 0;
    let len = a.len().min(b.len());
    while i < len {
        let va = a[i];
        let vb = b[i];
        dot += va * vb;
        ea += va * va;
        eb += vb * vb;
        i += step;
    }
    let denom = (ea * eb).sqrt();
    if denom < 1e-12 {
        0.0
    } else {
        dot / denom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    fn sine_stereo(freq_hz: f64, seconds: f64, sr: u32) -> Vec<f32> {
        let frames = (seconds * f64::from(sr)).round() as usize;
        let mut out = Vec::with_capacity(frames * 2);
        for i in 0..frames {
            let phase = 2.0 * PI * freq_hz * (i as f64 / f64::from(sr));
            let s = (phase.sin() * 0.8) as f32;
            out.push(s);
            out.push(-s * 0.5);
        }
        out
    }

    /// Estimates fundamental frequency from upward zero crossings of the left channel inside a
    /// steady interior window (skipping 80 ms at both ends).
    fn estimate_freq_hz(stereo: &[f32], sr: u32) -> f64 {
        let frames = stereo.len() / 2;
        let margin = ((f64::from(sr) * 0.08) as usize).min(frames / 4);
        let slice = &stereo[margin * 2..(frames - margin) * 2];
        let mut crossings = Vec::new();
        let n = slice.len() / 2;
        for i in 1..n {
            let prev = slice[(i - 1) * 2];
            let curr = slice[i * 2];
            if prev <= 0.0 && curr > 0.0 {
                let frac = f64::from(-prev) / f64::from(curr - prev).max(1e-12);
                crossings.push((i - 1) as f64 + frac);
            }
        }
        assert!(crossings.len() >= 4, "too few crossings");
        let cycles = (crossings.len() - 1) as f64;
        let span_samples = crossings.last().unwrap() - crossings.first().unwrap();
        cycles * f64::from(sr) / span_samples
    }

    #[test]
    fn unity_rate_returns_exact_samples() {
        let src = sine_stereo(440.0, 0.25, SR);
        assert_eq!(stretch_stereo(&src, SR, 1.0, true), src);
        assert_eq!(stretch_stereo(&src, SR, 1.0, false), src);
    }

    #[test]
    fn output_duration_scales_inversely_with_rate() {
        let src = sine_stereo(330.0, 1.0, SR);
        for &rate in &[0.25, 0.5, 0.75, 1.25, 2.0, 4.0] {
            let out = stretch_stereo(&src, SR, rate, true);
            let expected_frames = ((SR as f64) / rate).round() as usize;
            assert_eq!(out.len() / 2, expected_frames, "rate={rate}");
        }
    }

    #[test]
    fn wsola_preserves_pitch_while_varispeed_shifts_it() {
        let src = sine_stereo(440.0, 1.0, SR);

        let slow_wsola = stretch_stereo(&src, SR, 0.5, true);
        let slow_hz = estimate_freq_hz(&slow_wsola, SR);
        assert!(
            (slow_hz - 440.0).abs() < 12.0,
            "expected ~440 Hz at 0.5x WSOLA, got {slow_hz:.2} Hz"
        );

        let fast_wsola = stretch_stereo(&src, SR, 1.5, true);
        let fast_hz = estimate_freq_hz(&fast_wsola, SR);
        assert!(
            (fast_hz - 440.0).abs() < 15.0,
            "expected ~440 Hz at 1.5x WSOLA, got {fast_hz:.2} Hz"
        );

        let fast_vari = stretch_stereo(&src, SR, 1.5, false);
        let vari_hz = estimate_freq_hz(&fast_vari, SR);
        assert!(
            (vari_hz - 660.0).abs() < 10.0,
            "expected ~660 Hz at 1.5x varispeed, got {vari_hz:.2} Hz"
        );
    }

    #[test]
    fn constant_signal_stays_flat_across_overlap_add() {
        let src = vec![0.5f32; SR as usize * 2];
        for &rate in &[0.6, 1.4] {
            let out = stretch_stereo(&src, SR, rate, true);
            for (i, &v) in out.iter().enumerate() {
                assert!(
                    (v - 0.5).abs() < 1e-4,
                    "amplitude ripple at sample {i} (rate {rate}): {v}"
                );
            }
        }
    }

    #[test]
    fn short_inputs_and_extreme_rates_never_panic() {
        assert!(stretch_stereo(&[], SR, 0.5, true).is_empty());
        assert_eq!(stretch_stereo(&[0.25, -0.25], SR, 0.5, true).len(), 4);
        let tiny = vec![0.3f32; 40];
        assert_eq!(stretch_stereo(&tiny, SR, 0.5, true).len(), 80);
        assert_eq!(clamp_rate(0.01), MIN_STRETCH_RATE);
        assert_eq!(clamp_rate(99.0), MAX_STRETCH_RATE);
        assert_eq!(clamp_rate(f64::NAN), 1.0);
    }

    #[test]
    fn stretch_clip_slice_stretches_only_the_trimmed_region() {
        // 2 s silence followed by 2 s of 0.6 constant level at 10 kHz
        let sr = 10_000;
        let mut src = vec![0.0f32; sr * 2 * 2];
        src.extend(vec![0.6f32; sr * 2 * 2]);
        // Trim into the loud half at 2.0 s, stretch 1.0 s of source into 2.0 s on timeline (0.5x)
        let out = stretch_clip_slice(&PcmStore::from_vec(src), sr as u32, 2.0, 2.0, 0.5, true)
            .to_vec(0, usize::MAX);
        assert_eq!(out.len() / 2, sr * 2);
        assert!(out.iter().all(|&v| (v - 0.6).abs() < 1e-3));
    }
}
