//! Synchronizing two recordings of the same sound by their audio (the sync of a camera's own sound
//! with a recorder, as in the multicamera workflow of the manuals and in *Merge clips* of the
//! Premiere Pro reference): the offset between the two is found by correlating their loudness
//! envelopes.
//!
//! Only the shape of the loudness over time is compared, at [`ENVELOPE_RATE`], so two microphones
//! that heard the same room agree even when their quality differs. The result is good to a few
//! milliseconds, far inside one video frame.

/// Envelope samples a second: the precision of the offset is its inverse.
pub const ENVELOPE_RATE: f64 = 200.0;
/// Seconds of each recording that are compared (from the start of what the event uses).
pub const WINDOW_SECONDS: f64 = 60.0;
/// The offset searched for goes up to this far either way, seconds.
pub const MAX_OFFSET_SECONDS: f64 = 30.0;
/// The least overlap of the two envelopes for an offset to count, seconds.
pub const MIN_OVERLAP_SECONDS: f64 = 2.0;
/// Below this correlation the two do not share a sound worth trusting.
pub const MIN_CONFIDENCE: f32 = 0.3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sync {
    /// Seconds `other` has to be moved later (negative: earlier) to line up with `reference`.
    pub offset: f64,
    /// Correlation at the offset, 0 to 1.
    pub confidence: f32,
}

/// The loudness envelope of interleaved stereo `samples`: the mean absolute level of each
/// `1 / ENVELOPE_RATE` second, with its mean taken out, over at most [`WINDOW_SECONDS`].
pub fn envelope(samples: &[f32], rate: u32) -> Vec<f32> {
    let frames = samples.len() / 2;
    let bin = (f64::from(rate.max(1)) / ENVELOPE_RATE).round().max(1.0) as usize;
    let bins = (frames / bin).min((WINDOW_SECONDS * ENVELOPE_RATE) as usize);
    let mut out: Vec<f32> = (0..bins)
        .map(|b| {
            let from = b * bin;
            let sum: f32 = (from..from + bin)
                .map(|i| (samples[2 * i].abs() + samples[2 * i + 1].abs()) * 0.5)
                .sum();
            sum / bin as f32
        })
        .collect();
    if !out.is_empty() {
        let mean = out.iter().sum::<f32>() / out.len() as f32;
        for v in &mut out {
            *v -= mean;
        }
    }
    out
}

/// How much `other` has to move to line up with `reference` (both interleaved stereo, each from the
/// point the event plays from). `None` when either is too short or silent, or when nothing in them
/// matches (`confidence` under [`MIN_CONFIDENCE`]).
pub fn find_offset(
    reference: &[f32],
    reference_rate: u32,
    other: &[f32],
    other_rate: u32,
) -> Option<Sync> {
    let a = envelope(reference, reference_rate);
    let b = envelope(other, other_rate);
    let min_overlap = (MIN_OVERLAP_SECONDS * ENVELOPE_RATE) as usize;
    if a.len() < min_overlap || b.len() < min_overlap {
        return None;
    }
    // Running sums of squares, to normalize each lag by the energy it actually compares
    let prefix = |v: &[f32]| -> Vec<f64> {
        let mut sums = vec![0.0f64; v.len() + 1];
        for (i, x) in v.iter().enumerate() {
            sums[i + 1] = sums[i] + f64::from(*x) * f64::from(*x);
        }
        sums
    };
    let (pa, pb) = (prefix(&a), prefix(&b));
    let max_lag = (MAX_OFFSET_SECONDS * ENVELOPE_RATE) as i64;
    let (na, nb) = (a.len() as i64, b.len() as i64);

    // score[lag] compares a[i] with b[i - lag]; lag > 0 means `other` is early and moves later
    let lags: Vec<i64> = (-max_lag..=max_lag).collect();
    let mut scores = vec![f64::NEG_INFINITY; lags.len()];
    for (slot, &lag) in lags.iter().enumerate() {
        let from = lag.max(0);
        let to = na.min(nb + lag);
        if (to - from) < min_overlap as i64 {
            continue;
        }
        let (from, to) = (from as usize, to as usize);
        let dot: f64 = (from..to)
            .map(|i| f64::from(a[i]) * f64::from(b[(i as i64 - lag) as usize]))
            .sum();
        let ea = pa[to] - pa[from];
        let (bf, bt) = ((from as i64 - lag) as usize, (to as i64 - lag) as usize);
        let eb = pb[bt] - pb[bf];
        let norm = (ea * eb).sqrt();
        if norm > 1e-12 {
            scores[slot] = dot / norm;
        }
    }
    let (best, &top) = scores
        .iter()
        .enumerate()
        .max_by(|x, y| x.1.total_cmp(y.1))?;
    if !top.is_finite() || top < f64::from(MIN_CONFIDENCE) {
        return None;
    }
    // A parabola through the peak and its neighbours puts the offset between two bins
    let mut peak = lags[best] as f64;
    if best > 0 && best + 1 < scores.len() {
        let (l, r) = (scores[best - 1], scores[best + 1]);
        if l.is_finite() && r.is_finite() {
            let denom = l - 2.0 * top + r;
            if denom.abs() > 1e-12 {
                peak += (0.5 * (l - r) / denom).clamp(-0.5, 0.5);
            }
        }
    }
    Some(Sync {
        offset: peak / ENVELOPE_RATE,
        confidence: top.min(1.0) as f32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A burst-like signal that is not periodic: tones switched on and off in an irregular pattern.
    fn speech_like(seconds: f64, rate: u32, seed: u32) -> Vec<f32> {
        let frames = (seconds * f64::from(rate)) as usize;
        let mut state = seed;
        let mut next = || {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 8) as f32 / (1u32 << 24) as f32
        };
        let mut level = 0.0f32;
        let mut samples = Vec::with_capacity(frames * 2);
        for i in 0..frames {
            if i % (rate as usize / 20) == 0 {
                level = if next() < 0.5 { next() } else { 0.0 };
            }
            let v = level * (next() - 0.5);
            samples.push(v);
            samples.push(v);
        }
        samples
    }

    fn delayed(samples: &[f32], rate: u32, seconds: f64) -> Vec<f32> {
        let pad = (seconds * f64::from(rate)) as usize * 2;
        let mut out = vec![0.0; pad];
        out.extend_from_slice(samples);
        out
    }

    #[test]
    fn a_late_copy_is_found_and_has_to_move_earlier_by_its_delay() {
        let rate = 8_000;
        let a = speech_like(20.0, rate, 7);
        let b = delayed(&a, rate, 0.37);
        let sync = find_offset(&a, rate, &b, rate).unwrap();
        assert!((sync.offset + 0.37).abs() < 0.01, "{sync:?}");
        assert!(sync.confidence > 0.8);
        // The other way round: the copy is early and moves later
        let back = find_offset(&b, rate, &a, rate).unwrap();
        assert!((back.offset - 0.37).abs() < 0.01, "{back:?}");
    }

    #[test]
    fn the_two_recordings_may_differ_in_rate_and_level_and_carry_noise() {
        let a = speech_like(20.0, 8_000, 3);
        let mut b = delayed(&speech_like(20.0, 8_000, 3), 8_000, 1.25);
        for (i, s) in b.iter_mut().enumerate() {
            *s = *s * 0.4 + ((i * 2_654_435_761) % 1000) as f32 * 2.0e-6;
        }
        // The same sound at 16 kHz: the signal repeated per sample
        let wide: Vec<f32> = b.chunks(2).flat_map(|f| [f[0], f[1], f[0], f[1]]).collect();
        let sync = find_offset(&a, 8_000, &wide, 16_000).unwrap();
        assert!((sync.offset + 1.25).abs() < 0.015, "{sync:?}");
    }

    #[test]
    fn unrelated_silent_or_short_sound_gives_no_answer() {
        let rate = 8_000;
        let a = speech_like(20.0, rate, 1);
        let other = speech_like(20.0, rate, 99);
        assert!(find_offset(&a, rate, &other, rate).is_none_or(|s| s.confidence < 0.6));
        assert!(find_offset(&a, rate, &vec![0.0; 20 * rate as usize * 2], rate).is_none());
        assert!(
            find_offset(&a, rate, &a[..rate as usize], rate).is_none(),
            "one second"
        );
        assert!(find_offset(&[], rate, &a, rate).is_none());
        // An offset beyond the search range is not found (the copy is 40 s late)
        let far = delayed(&speech_like(60.0, rate, 5), rate, 40.0);
        let near = speech_like(60.0, rate, 5);
        assert!(find_offset(&near, rate, &far, rate)
            .is_none_or(|s| s.offset.abs() <= MAX_OFFSET_SECONDS));
    }
}
