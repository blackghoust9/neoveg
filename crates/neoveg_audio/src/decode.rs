//! Audio file decoding with symphonia (WAV, MP3, FLAC, OGG, AAC, ...).

use std::fs::File;
use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::pcm::{PcmStore, PcmWriter};

/// Fully decoded audio, interleaved `f32`.
#[derive(Debug, Clone)]
pub struct DecodedAudio {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

impl DecodedAudio {
    pub fn frame_count(&self) -> usize {
        self.samples.len() / self.channels.max(1) as usize
    }

    pub fn duration_seconds(&self) -> f64 {
        self.frame_count() as f64 / self.sample_rate.max(1) as f64
    }

    /// Interleaved stereo copy (mono is mirrored, extra channels are dropped).
    pub fn to_stereo(&self) -> Vec<f32> {
        let ch = self.channels.max(1) as usize;
        let mut out = Vec::with_capacity(self.frame_count() * 2);
        for frame in self.samples.chunks_exact(ch) {
            out.push(frame[0]);
            out.push(if ch > 1 { frame[1] } else { frame[0] });
        }
        out
    }
}

/// The first track that is audio and has a decoder. The file's default track is not good enough: in a
/// video container (MP4, MOV, MKV) it is usually the picture, which symphonia cannot decode, and the
/// sound would be reported as unsupported.
fn pick_audio_track(
    format: &dyn symphonia::core::formats::FormatReader,
    path: &Path,
) -> Result<(u32, Box<dyn symphonia::core::codecs::Decoder>), String> {
    let mut last_error = None;
    for track in format.tracks() {
        let params = &track.codec_params;
        let is_audio = params.codec != symphonia::core::codecs::CODEC_TYPE_NULL
            && (params.sample_rate.is_some() || params.channels.is_some());
        if !is_audio {
            continue;
        }
        match symphonia::default::get_codecs().make(params, &DecoderOptions::default()) {
            Ok(decoder) => return Ok((track.id, decoder)),
            Err(e) => last_error = Some(e.to_string()),
        }
    }
    match last_error {
        Some(e) => Err(format!("no decoder for {}: {e}", path.display())),
        None => Err(format!("no audio track found in {}", path.display())),
    }
}

/// Where decoded blocks go: `block` gets interleaved samples of `channels` channels.
trait Sink {
    /// The container's frame count, when it says.
    fn start(&mut self, frames: Option<usize>);
    fn block(&mut self, samples: &[f32], channels: usize) -> Result<(), String>;
}

/// Everything in memory, as the file has it.
struct VecSink(Vec<f32>);

impl Sink for VecSink {
    fn start(&mut self, frames: Option<usize>) {
        if let Some(frames) = frames {
            // Beyond this a lying header would allocate gigabytes before a single packet
            self.0.reserve_exact(frames.saturating_mul(2).min(1 << 28));
        }
    }

    fn block(&mut self, samples: &[f32], _channels: usize) -> Result<(), String> {
        self.0.extend_from_slice(samples);
        Ok(())
    }
}

impl Sink for PcmWriter {
    fn start(&mut self, frames: Option<usize>) {
        if let Some(frames) = frames {
            self.reserve_frames(frames);
        }
    }

    fn block(&mut self, samples: &[f32], channels: usize) -> Result<(), String> {
        self.push(samples, channels)
            .map_err(|e| format!("cannot keep the decoded sound: {e}"))
    }
}

/// Decodes the first decodable audio track of `path` (the sound of a video file included).
pub fn decode_file(path: &Path) -> Result<DecodedAudio, String> {
    let mut sink = VecSink(Vec::new());
    let (sample_rate, channels) = decode_into(path, &mut sink)?;
    Ok(DecodedAudio {
        sample_rate,
        channels,
        samples: sink.0,
    })
}

/// A decoded file kept as stereo in a [`PcmStore`]: in memory when it is short, in a temporary file
/// when it is long, so a long recording never needs its length in memory.
pub struct DecodedStore {
    pub sample_rate: u32,
    /// Channels the file had (the store is always stereo).
    pub channels: u16,
    pub store: PcmStore,
}

impl DecodedStore {
    pub fn duration_seconds(&self) -> f64 {
        self.store.frames() as f64 / f64::from(self.sample_rate.max(1))
    }
}

/// Decodes `path` block by block into a [`PcmStore`] (see [`DecodedStore`]).
pub fn decode_to_store(path: &Path) -> Result<DecodedStore, String> {
    let mut writer = PcmWriter::new();
    let (sample_rate, channels) = decode_into(path, &mut writer)?;
    let store = writer
        .finish()
        .map_err(|e| format!("cannot keep the decoded sound: {e}"))?;
    Ok(DecodedStore {
        sample_rate,
        channels,
        store,
    })
}

/// Length and channel count of the first audio track without decoding it, when the container says.
pub fn probe_audio(path: &Path) -> Option<(f64, u16)> {
    let file = File::open(path).ok()?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .ok()?;
    let format = probed.format;
    let (track_id, _) = pick_audio_track(&*format, path).ok()?;
    let params = &format
        .tracks()
        .iter()
        .find(|t| t.id == track_id)?
        .codec_params;
    let frames = params.n_frames?;
    let rate = params.sample_rate?;
    let channels = params.channels?.count() as u16;
    (rate > 0).then(|| (frames as f64 / f64::from(rate), channels))
}

/// Decodes the first audio track of `path` into `sink`; returns the sample rate and channel count.
fn decode_into(path: &Path, sink: &mut dyn Sink) -> Result<(u32, u16), String> {
    let file = File::open(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("unsupported media {}: {e}", path.display()))?;
    let mut format = probed.format;

    let (track_id, mut decoder) = pick_audio_track(&*format, path)?;

    let mut sample_rate = 0u32;
    let mut channels = 0u16;
    let mut any = false;
    // The container usually knows the length: the sink can size itself once
    sink.start(
        format
            .tracks()
            .iter()
            .find(|t| t.id == track_id)
            .and_then(|t| t.codec_params.n_frames)
            .map(|n| n as usize),
    );
    // Reused for every packet, so decoding allocates once
    let mut buf: Option<SampleBuffer<f32>> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(Error::ResetRequired) => break,
            Err(e) => return Err(format!("read error in {}: {e}", path.display())),
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                sample_rate = spec.rate;
                channels = spec.channels.count() as u16;
                let capacity = decoded.capacity() as u64;
                let buf = match &mut buf {
                    Some(b) if b.capacity() as u64 >= capacity * u64::from(channels) => b,
                    slot => slot.insert(SampleBuffer::<f32>::new(capacity, spec)),
                };
                buf.copy_interleaved_ref(decoded);
                if !buf.samples().is_empty() {
                    any = true;
                }
                sink.block(buf.samples(), usize::from(channels))?;
            }
            // A corrupt packet is skipped; the rest of the file is still usable
            Err(Error::DecodeError(e)) => {
                tracing::warn!("decode error in {}: {e}", path.display());
            }
            Err(e) => return Err(format!("decode error in {}: {e}", path.display())),
        }
    }

    if sample_rate == 0 || channels == 0 || !any {
        return Err(format!("{} contains no decodable audio", path.display()));
    }
    Ok((sample_rate, channels))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Makes `name` in a scratch folder with the `ffmpeg` tool: an MP4 with a picture and a 440 Hz
    /// tone of two seconds as AAC. `None` when the tool is not installed (the test is skipped).
    fn tone_mp4(name: &str) -> Option<(std::path::PathBuf, std::path::PathBuf)> {
        let dir = std::env::temp_dir().join(format!("neoveg_decode_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).ok()?;
        let file = dir.join("tone.mp4");
        let made = std::process::Command::new("ffmpeg")
            .args(["-y", "-v", "error", "-f", "lavfi", "-i"])
            .arg("testsrc=duration=2:size=160x120:rate=25")
            .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=2"])
            .args(["-c:v", "mpeg4", "-c:a", "aac", "-shortest"])
            .arg(&file)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if made {
            Some((dir, file))
        } else {
            let _ = std::fs::remove_dir_all(&dir);
            None
        }
    }

    /// Frequency of a tone from its zero crossings, over `window` frames starting at `from`.
    fn tone_hz(samples: &[f32], channels: usize, rate: u32, from: usize, window: usize) -> f64 {
        let mono: Vec<f32> = samples
            .chunks_exact(channels)
            .skip(from)
            .take(window)
            .map(|f| f[0])
            .collect();
        let crossings = mono
            .windows(2)
            .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
            .count();
        crossings as f64 / 2.0 / (mono.len() as f64 / f64::from(rate))
    }

    fn rms(samples: &[f32]) -> f64 {
        (samples.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>() / samples.len() as f64).sqrt()
    }

    /// The sound of a video file is the tone that was put in it, at its level and its length: not
    /// silence, not noise, not another pitch.
    #[test]
    fn the_sound_of_an_mp4_is_the_tone_that_was_put_in_it() {
        let Some((dir, file)) = tone_mp4("tone") else {
            eprintln!("skipped: ffmpeg is not installed");
            return;
        };
        let audio = decode_file(&file).expect("the AAC track of the MP4 decodes");
        let _ = std::fs::remove_dir_all(&dir);
        let channels = usize::from(audio.channels);
        // About two seconds; AAC adds a little silence at the ends
        assert!(
            (1.8..2.3).contains(&audio.duration_seconds()),
            "duration {}",
            audio.duration_seconds()
        );
        // Measured in the middle second, away from the encoder's start and end
        let rate = audio.sample_rate as usize;
        let from = rate / 2;
        let hz = tone_hz(&audio.samples, channels, audio.sample_rate, from, rate);
        assert!((hz - 440.0).abs() < 8.0, "the pitch is {hz} Hz, not 440");
        let middle: Vec<f32> = audio
            .samples
            .chunks_exact(channels)
            .skip(from)
            .take(rate)
            .map(|f| f[0])
            .collect();
        // ffmpeg's sine is at 1/8 of full scale: an RMS near 0.088
        let level = rms(&middle);
        assert!((0.06..0.12).contains(&level), "the level is {level} (RMS)");
    }

    #[test]
    fn mono_is_mirrored_to_stereo() {
        let audio = DecodedAudio {
            sample_rate: 48_000,
            channels: 1,
            samples: vec![0.25, -0.5],
        };
        assert_eq!(audio.to_stereo(), vec![0.25, 0.25, -0.5, -0.5]);
        assert_eq!(audio.frame_count(), 2);
    }

    /// A 16-bit PCM WAV of `frames` frames; channel `c` of frame `i` holds `(i + c) * 100`.
    fn write_wav(path: &Path, channels: u16, frames: u32) {
        let mut data = Vec::new();
        for i in 0..frames {
            for c in 0..channels {
                data.extend_from_slice(&(((i + u32::from(c)) * 100) as i16).to_le_bytes());
            }
        }
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&8000u32.to_le_bytes());
        wav.extend_from_slice(&(8000 * u32::from(channels) * 2).to_le_bytes());
        wav.extend_from_slice(&(channels * 2).to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
        wav.extend_from_slice(&data);
        std::fs::write(path, wav).unwrap();
    }

    #[test]
    fn the_store_matches_the_full_decode_and_the_probe_needs_no_decoding() {
        let dir = std::env::temp_dir().join(format!("neoveg_decode_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for channels in [1u16, 2, 3] {
            let path = dir.join(format!("c{channels}.wav"));
            write_wav(&path, channels, 400);
            let full = decode_file(&path).unwrap();
            let decoded = decode_to_store(&path).unwrap();
            assert_eq!(decoded.channels, channels);
            assert_eq!(decoded.sample_rate, 8000);
            assert_eq!(
                decoded.store.to_vec(0, usize::MAX),
                full.to_stereo(),
                "{channels} channels"
            );
            let (seconds, found) = probe_audio(&path).unwrap();
            assert_eq!(found, channels);
            assert!((seconds - 400.0 / 8000.0).abs() < 1e-9, "{seconds}");
        }
        assert!(probe_audio(&dir.join("missing.wav")).is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_file_is_an_error() {
        assert!(decode_file(Path::new("does-not-exist.wav")).is_err());
    }
}
