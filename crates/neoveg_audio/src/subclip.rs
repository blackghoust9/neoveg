//! Writing a part of a sound file as a file of its own, forwards or backwards (*Subclipes*, *Reverter
//! evento*): the part is decoded, cut, optionally reversed frame by frame (so every channel keeps its
//! place) and written as a 32-bit float WAV with all the channels the file had.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use crate::decode::decode_file;
use crate::record::WavWriter;

/// The frames of `samples` (interleaved, `channels` per frame) from `start` seconds, for `length`
/// seconds, in the order they play: reversed when `reverse`. Frames past the end are not there.
pub fn cut(
    samples: &[f32],
    channels: usize,
    rate: u32,
    start: f64,
    length: f64,
    reverse: bool,
) -> Vec<f32> {
    let channels = channels.max(1);
    let frames = samples.len() / channels;
    let rate = f64::from(rate.max(1));
    let from = ((start.max(0.0) * rate).round() as usize).min(frames);
    let to = (((start.max(0.0) + length.max(0.0)) * rate).round() as usize).clamp(from, frames);
    let part = &samples[from * channels..to * channels];
    if !reverse {
        return part.to_vec();
    }
    part.chunks_exact(channels)
        .rev()
        .flatten()
        .copied()
        .collect()
}

/// Writes the part of `input` from `start` for `length` seconds to `output` (a `.wav`). Returns the
/// seconds written, which is less than `length` when the file ends first.
pub fn write_wav(
    input: &Path,
    output: &Path,
    start: f64,
    length: f64,
    reverse: bool,
) -> Result<f64, String> {
    let audio = decode_file(input)?;
    let part = cut(
        &audio.samples,
        usize::from(audio.channels),
        audio.sample_rate,
        start,
        length,
        reverse,
    );
    if part.is_empty() {
        return Err("a parte pedida está fora do arquivo".into());
    }
    write_part(output, audio.channels, audio.sample_rate, &part)
}

/// Writes `part` (interleaved) as a 32-bit float WAV; returns the seconds written.
fn write_part(output: &Path, channels: u16, sample_rate: u32, part: &[f32]) -> Result<f64, String> {
    if let Some(dir) = output.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("não foi possível criar a pasta {}: {e}", dir.display()))?;
    }
    let file = File::create(output)
        .map_err(|e| format!("não foi possível criar {}: {e}", output.display()))?;
    let mut writer = WavWriter::new(BufWriter::new(file), channels, sample_rate)
        .map_err(|e| format!("não foi possível escrever o WAV: {e}"))?;
    let frames = part.len() / usize::from(channels.max(1));
    let result = writer
        .write_samples(part)
        .and_then(|()| writer.finish().map(|_| ()));
    if let Err(e) = result {
        let _ = std::fs::remove_file(output);
        return Err(format!("não foi possível escrever o WAV: {e}"));
    }
    Ok(frames as f64 / f64::from(sample_rate.max(1)))
}

/// Block size the effects are run with offline.
const OFFLINE_BLOCK: usize = 512;

/// Runs `samples` (interleaved, `channels` per frame) through the effects of `chain` in order, off
/// line (*efeito fora de tempo real*): the whole sound goes through each effect once, the delay of
/// effects that look ahead is taken out so the result lines up with the original, and the length
/// stays the same. Bypassed effects must already be left out of `chain`. An effect that is not
/// built in (a hosted plug-in) or unknown is an error: the result would not be what plays.
pub fn apply_effects(
    samples: &[f32],
    channels: usize,
    sample_rate: u32,
    chain: &[crate::fxmixer::EffectSpec],
) -> Result<Vec<f32>, String> {
    let channels = channels.max(1);
    let frames = samples.len() / channels;
    let mut planar: Vec<Vec<f32>> = (0..channels)
        .map(|c| (0..frames).map(|f| samples[f * channels + c]).collect())
        .collect();
    for spec in chain {
        let mut effect = crate::dsp::effects::create_effect(&spec.kind, sample_rate as f32, channels)
            .ok_or_else(|| format!("o efeito `{}` não roda fora de tempo real", spec.kind))?;
        for (id, value) in &spec.params {
            effect.set_param(id, *value);
        }
        effect.reset();
        let latency = effect.latency();
        // Feed `latency` frames of silence after the end so the delayed tail comes out
        for ch in &mut planar {
            ch.extend(std::iter::repeat(0.0).take(latency));
        }
        let total = frames + latency;
        let mut pos = 0;
        while pos < total {
            let end = (pos + OFFLINE_BLOCK).min(total);
            let mut slices: Vec<&mut [f32]> = planar.iter_mut().map(|c| &mut c[pos..end]).collect();
            effect.process(&mut slices);
            pos = end;
        }
        for ch in &mut planar {
            ch.drain(..latency);
        }
    }
    let mut out = Vec::with_capacity(frames * channels);
    for f in 0..frames {
        for ch in &planar {
            out.push(ch[f]);
        }
    }
    Ok(out)
}

/// Like [`write_wav`], with the part run through `chain` first (see [`apply_effects`]).
pub fn write_wav_with_effects(
    input: &Path,
    output: &Path,
    start: f64,
    length: f64,
    chain: &[crate::fxmixer::EffectSpec],
) -> Result<f64, String> {
    let audio = decode_file(input)?;
    let part = cut(
        &audio.samples,
        usize::from(audio.channels),
        audio.sample_rate,
        start,
        length,
        false,
    );
    if part.is_empty() {
        return Err("a parte pedida está fora do arquivo".into());
    }
    let part = apply_effects(&part, usize::from(audio.channels), audio.sample_rate, chain)?;
    write_part(output, audio.channels, audio.sample_rate, &part)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two channels, frame `i` holds `(i, -i)`.
    fn ramp(frames: usize) -> Vec<f32> {
        (0..frames).flat_map(|i| [i as f32, -(i as f32)]).collect()
    }

    #[test]
    fn a_part_is_cut_by_seconds_and_keeps_the_channels_together() {
        let samples = ramp(100);
        let part = cut(&samples, 2, 10, 2.0, 3.0, false);
        assert_eq!(part.len(), 60, "3 s at 10 Hz, two channels");
        assert_eq!(&part[..4], &[20.0, -20.0, 21.0, -21.0]);
        assert_eq!(&part[56..], &[48.0, -48.0, 49.0, -49.0]);
    }

    #[test]
    fn a_reversed_part_plays_its_frames_backwards_with_each_frame_intact() {
        let samples = ramp(100);
        let part = cut(&samples, 2, 10, 2.0, 3.0, true);
        assert_eq!(part.len(), 60);
        assert_eq!(&part[..4], &[49.0, -49.0, 48.0, -48.0]);
        assert_eq!(&part[56..], &[21.0, -21.0, 20.0, -20.0]);
        // Reversing twice gives the part back
        let again = cut(&part, 2, 10, 0.0, 3.0, true);
        assert_eq!(again, cut(&samples, 2, 10, 2.0, 3.0, false));
    }

    fn spec(kind: &str, params: &[(&str, f32)]) -> crate::fxmixer::EffectSpec {
        crate::fxmixer::EffectSpec {
            kind: kind.into(),
            params: params.iter().map(|(k, v)| ((*k).into(), *v)).collect(),
        }
    }

    #[test]
    fn effects_are_baked_into_the_samples_keeping_length_and_alignment() {
        // 1 kHz tone at 0.5, stereo; a -6 dB amplify halves it
        let n = 4_800;
        let samples: Vec<f32> = (0..n)
            .flat_map(|i| {
                let v = 0.5 * (i as f32 * 0.13).sin();
                [v, v]
            })
            .collect();
        let out = apply_effects(&samples, 2, 48_000, &[spec("amplify", &[("gain", -6.0206)])]).unwrap();
        assert_eq!(out.len(), samples.len());
        let ratio = out[2 * 1000] / samples[2 * 1000];
        assert!((ratio - 0.5).abs() < 0.01, "{ratio}");
        // A look-ahead effect (limiter) is lined up with the original: peaks stay where they were
        let mut loud = vec![0.0f32; 2 * n];
        loud[2 * 2_000] = 1.0;
        loud[2 * 2_000 + 1] = 1.0;
        let out = apply_effects(&loud, 2, 48_000, &[spec("limiter", &[("ceiling", -6.0)])]).unwrap();
        assert_eq!(out.len(), loud.len());
        let peak_at = (0..n).max_by(|&a, &b| out[2 * a].abs().total_cmp(&out[2 * b].abs())).unwrap();
        assert!((peak_at as i64 - 2_000).abs() <= 1, "{peak_at}");
        assert!(out[2 * peak_at].abs() < 0.55, "{}", out[2 * peak_at]);
        assert!(apply_effects(&samples, 2, 48_000, &[spec("nope", &[])]).is_err());
        assert_eq!(apply_effects(&samples, 2, 48_000, &[]).unwrap(), samples);
    }

    #[test]
    fn a_file_is_written_back_with_the_effect_applied() {
        let dir = std::env::temp_dir().join(format!("neoveg_bake_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("in.wav");
        let mut writer =
            WavWriter::new(BufWriter::new(File::create(&input).unwrap()), 1, 8_000).unwrap();
        writer.write_samples(&vec![0.4f32; 8_000]).unwrap();
        writer.finish().unwrap();
        let output = dir.join("out.wav");
        let seconds =
            write_wav_with_effects(&input, &output, 0.0, 0.5, &[spec("amplify", &[("gain", -6.0206)])])
                .unwrap();
        assert!((seconds - 0.5).abs() < 1e-9);
        let back = decode_file(&output).unwrap();
        assert_eq!(back.frame_count(), 4_000);
        assert!((back.samples[100] - 0.2).abs() < 0.01, "{}", back.samples[100]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_part_past_the_end_is_cut_short_and_one_outside_is_empty() {
        let samples = ramp(100);
        assert_eq!(cut(&samples, 2, 10, 8.0, 5.0, false).len(), 40);
        assert!(cut(&samples, 2, 10, 20.0, 1.0, false).is_empty());
        assert!(cut(&samples, 2, 10, 1.0, 0.0, true).is_empty());
        assert!(cut(&[], 2, 10, 0.0, 1.0, false).is_empty());
        // Odd input never panics
        assert_eq!(cut(&samples, 0, 0, -1.0, f64::NAN, false).len(), 0);
    }

    #[test]
    fn a_file_is_written_back_reversed_with_its_length_and_channels() {
        let dir = std::env::temp_dir().join(format!("neoveg_subclip_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("in.wav");
        let mut writer =
            WavWriter::new(BufWriter::new(File::create(&input).unwrap()), 2, 8_000).unwrap();
        writer.write_samples(&ramp(8_000)).unwrap();
        writer.finish().unwrap();
        let output = dir.join("sub").join("out.wav");
        let seconds = write_wav(&input, &output, 0.25, 0.5, true).unwrap();
        assert!((seconds - 0.5).abs() < 1e-9, "{seconds}");
        let back = decode_file(&output).unwrap();
        assert_eq!((back.channels, back.sample_rate), (2, 8_000));
        assert_eq!(back.frame_count(), 4_000);
        // The first frame played is the last one of the part (frame 2000 + 4000 - 1)
        assert_eq!(&back.samples[..2], &[5999.0, -5999.0]);
        assert!(write_wav(&input, &dir.join("x.wav"), 99.0, 1.0, false).is_err());
        assert!(!dir.join("x.wav").exists(), "nothing is left behind");
        let _ = std::fs::remove_dir_all(dir);
    }
}
