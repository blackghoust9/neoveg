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
    if let Some(dir) = output.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("não foi possível criar a pasta {}: {e}", dir.display()))?;
    }
    let file = File::create(output)
        .map_err(|e| format!("não foi possível criar {}: {e}", output.display()))?;
    let mut writer = WavWriter::new(BufWriter::new(file), audio.channels, audio.sample_rate)
        .map_err(|e| format!("não foi possível escrever o WAV: {e}"))?;
    let frames = part.len() / usize::from(audio.channels.max(1));
    let result = writer
        .write_samples(&part)
        .and_then(|()| writer.finish().map(|_| ()));
    if let Err(e) = result {
        let _ = std::fs::remove_file(output);
        return Err(format!("não foi possível escrever o WAV: {e}"));
    }
    Ok(frames as f64 / f64::from(audio.sample_rate.max(1)))
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
