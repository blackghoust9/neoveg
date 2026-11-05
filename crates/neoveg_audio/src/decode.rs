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
    nth: usize,
) -> Result<(u32, Box<dyn symphonia::core::codecs::Decoder>), String> {
    let mut last_error = None;
    let mut skipped = 0;
    for track in format.tracks() {
        let params = &track.codec_params;
        let is_audio = params.codec != symphonia::core::codecs::CODEC_TYPE_NULL
            && (params.sample_rate.is_some() || params.channels.is_some());
        if !is_audio {
            continue;
        }
        match symphonia::default::get_codecs().make(params, &DecoderOptions::default()) {
            // The `nth` audio stream (the first one for 0)
            Ok(_) if skipped < nth => skipped += 1,
            Ok(decoder) => return Ok((track.id, decoder)),
            Err(e) => last_error = Some(e.to_string()),
        }
    }
    if skipped > 0 {
        return Err(format!(
            "{} has no audio stream {}",
            path.display(),
            nth + 1
        ));
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
    let (sample_rate, channels) = decode_into(path, 0, &mut sink)?;
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
    let (sample_rate, channels) = decode_into(path, 0, &mut writer)?;
    let store = writer
        .finish()
        .map_err(|e| format!("cannot keep the decoded sound: {e}"))?;
    Ok(DecodedStore {
        sample_rate,
        channels,
        store,
    })
}

/// Hands the stereo pair of channels `left` and `right` of every block to the next sink (a
/// channel past the last one of the stream is the last one).
struct PickSink<'a> {
    inner: &'a mut PcmWriter,
    left: usize,
    right: usize,
    pair: Vec<f32>,
}

impl Sink for PickSink<'_> {
    fn start(&mut self, frames: Option<usize>) {
        self.inner.start(frames);
    }

    fn block(&mut self, samples: &[f32], channels: usize) -> Result<(), String> {
        let channels = channels.max(1);
        self.pair.clear();
        for frame in samples.chunks_exact(channels) {
            self.pair.push(frame[self.left.min(channels - 1)]);
            self.pair.push(frame[self.right.min(channels - 1)]);
        }
        self.inner.block(&self.pair, 2)
    }
}

/// Decodes the stream `source.stream` of `path` keeping the channels `source.left` and `source.right`
/// as the stereo pair of the store (one channel twice when they are the same).
pub fn decode_to_store_source(
    path: &Path,
    source: neoveg_common::channels::SoundSource,
) -> Result<DecodedStore, String> {
    let mut writer = PcmWriter::new();
    let (sample_rate, channels) = {
        let mut sink = PickSink {
            inner: &mut writer,
            left: usize::from(source.left),
            right: usize::from(source.right),
            pair: Vec::new(),
        };
        decode_into(path, usize::from(source.stream), &mut sink)?
    };
    let store = writer
        .finish()
        .map_err(|e| format!("cannot keep the decoded sound: {e}"))?;
    Ok(DecodedStore {
        sample_rate,
        channels,
        store,
    })
}

/// The channel count of every audio stream of `path` that can be decoded (empty when it cannot be
/// read): a file with several languages or several mixes has more than one.
pub fn probe_audio_streams(path: &Path) -> Vec<u16> {
    let Ok(file) = File::open(path) else {
        return Vec::new();
    };
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let Ok(probed) = symphonia::default::get_probe().format(
        &hint,
        stream,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    ) else {
        return Vec::new();
    };
    probed
        .format
        .tracks()
        .iter()
        .filter(|t| {
            let p = &t.codec_params;
            p.codec != symphonia::core::codecs::CODEC_TYPE_NULL
                && (p.sample_rate.is_some() || p.channels.is_some())
                && symphonia::default::get_codecs()
                    .make(p, &DecoderOptions::default())
                    .is_ok()
        })
        .map(|t| t.codec_params.channels.map_or(2, |c| c.count() as u16))
        .collect()
}

/// Length and channel count of the first audio track without decoding it, when the container says.
pub fn probe_audio(path: &Path) -> Option<(f64, u16)> {
    if neoveg_media::mxf::is_mxf_path(path) {
        let info = neoveg_media::mxf::probe_file(path).ok()?;
        if info.has_audio && info.audio_channels > 0 {
            if let Some(dur) = info.duration_seconds {
                return Some((dur, info.audio_channels));
            }
        }
        return None;
    }
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
    let (track_id, _) = pick_audio_track(&*format, path, 0).ok()?;
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
fn decode_into(
    path: &Path,
    audio_stream: usize,
    sink: &mut dyn Sink,
) -> Result<(u32, u16), String> {
    if neoveg_media::mxf::is_mxf_path(path) {
        let mxf = neoveg_media::mxf::decode_audio_file(path)?;
        let ch = usize::from(mxf.channels.max(1));
        let frames = mxf.samples.len() / ch;
        sink.start(Some(frames));
        sink.block(&mxf.samples, ch)?;
        return Ok((mxf.sample_rate, mxf.channels));
    }
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

    let (track_id, mut decoder) = pick_audio_track(&*format, path, audio_stream)?;

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

    #[test]
    fn mxf_pcm_decodes_in_pure_rust_without_ffmpeg() {
        let dir = std::env::temp_dir().join(format!("neoveg_mxf_decode_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("clip.mxf");
        let mut mxf = Vec::new();
        // Header Partition Pack
        mxf.extend_from_slice(&[
            0x06, 0x0e, 0x2b, 0x34, 0x02, 0x05, 0x01, 0x01, 0x0d, 0x01, 0x02, 0x01, 0x01, 0x02,
            0x04, 0x00, 0x58,
        ]);
        mxf.extend_from_slice(&[0u8; 88]);
        // WaveAudioDescriptor: 48 kHz, 2 ch, 16-bit, duration 3 frames
        let mut desc = Vec::new();
        desc.extend_from_slice(&0x3d03u16.to_be_bytes());
        desc.extend_from_slice(&8u16.to_be_bytes());
        desc.extend_from_slice(&48_000i32.to_be_bytes());
        desc.extend_from_slice(&1i32.to_be_bytes());
        desc.extend_from_slice(&0x3d07u16.to_be_bytes());
        desc.extend_from_slice(&4u16.to_be_bytes());
        desc.extend_from_slice(&2u32.to_be_bytes());
        desc.extend_from_slice(&0x3d01u16.to_be_bytes());
        desc.extend_from_slice(&4u16.to_be_bytes());
        desc.extend_from_slice(&16u32.to_be_bytes());
        desc.extend_from_slice(&0x3002u16.to_be_bytes());
        desc.extend_from_slice(&8u16.to_be_bytes());
        desc.extend_from_slice(&3i64.to_be_bytes());
        mxf.extend_from_slice(&[
            0x06, 0x0e, 0x2b, 0x34, 0x02, 0x53, 0x01, 0x01, 0x0d, 0x01, 0x01, 0x01, 0x01, 0x01,
            0x48, 0x00,
        ]);
        mxf.push(desc.len() as u8);
        mxf.extend_from_slice(&desc);
        // Sound essence packet: 3 stereo 16-bit LE frames
        let mut pcm = Vec::new();
        for _ in 0..3 {
            pcm.extend_from_slice(&16_384i16.to_le_bytes());
            pcm.extend_from_slice(&(-8_192i16).to_le_bytes());
        }
        mxf.extend_from_slice(&[
            0x06, 0x0e, 0x2b, 0x34, 0x01, 0x02, 0x01, 0x01, 0x0d, 0x01, 0x03, 0x01, 0x16, 0x01,
            0x02, 0x01,
        ]);
        mxf.push(pcm.len() as u8);
        mxf.extend_from_slice(&pcm);

        std::fs::write(&path, &mxf).unwrap();
        let (dur, ch) = probe_audio(&path).unwrap();
        assert_eq!(ch, 2);
        assert!((dur - 3.0 / 48_000.0).abs() < 1e-9);
        let decoded = decode_to_store(&path).unwrap();
        assert_eq!(decoded.sample_rate, 48_000);
        assert_eq!(decoded.channels, 2);
        let stereo = decoded.store.to_vec(0, 10);
        assert_eq!(stereo.len(), 6);
        assert!((stereo[0] - 0.5).abs() < 1e-4);
        assert!((stereo[1] + 0.25).abs() < 1e-4);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A WAV of four channels where channel `c` holds the constant `(c + 1) / 10`.
    fn four_channel_wav(name: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("neoveg_pick_{}_{name}.wav", std::process::id()));
        let mut writer = crate::record::WavWriter::new(
            std::io::BufWriter::new(std::fs::File::create(&path).unwrap()),
            4,
            48_000,
        )
        .unwrap();
        let frames: Vec<f32> = (0..4800).flat_map(|_| [0.1, 0.2, 0.3, 0.4]).collect();
        writer.write_samples(&frames).unwrap();
        writer.finish().unwrap();
        path
    }

    fn first_frame(store: &DecodedStore) -> (f32, f32) {
        store.store.frame(0)
    }

    #[test]
    fn a_pair_or_a_single_channel_of_a_multichannel_file_is_what_the_store_keeps() {
        use neoveg_common::channels::SoundSource;
        let path = four_channel_wav("pairs");
        let pick = |left, right| SoundSource {
            stream: 0,
            left,
            right,
        };
        let near =
            |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6;
        let first = decode_to_store_source(&path, pick(0, 1)).unwrap();
        assert_eq!(first.channels, 4);
        assert!(near(first_frame(&first), (0.1, 0.2)));
        let second = decode_to_store_source(&path, pick(2, 3)).unwrap();
        assert!(near(first_frame(&second), (0.3, 0.4)), "channels 3 and 4");
        let single = decode_to_store_source(&path, pick(3, 3)).unwrap();
        assert!(
            near(first_frame(&single), (0.4, 0.4)),
            "one channel on both sides"
        );
        let past = decode_to_store_source(&path, pick(9, 9)).unwrap();
        assert!(
            near(first_frame(&past), (0.4, 0.4)),
            "past the last channel is the last one"
        );
        assert_eq!(first.store.frames(), 4800);
        // The default pick is what `decode_to_store` gives
        let plain = decode_to_store(&path).unwrap();
        assert!(near(first_frame(&plain), (0.1, 0.2)));
        // The file has one stream, so the second does not exist
        assert!(decode_to_store_source(
            &path,
            SoundSource {
                stream: 1,
                left: 0,
                right: 1
            }
        )
        .is_err());
        assert_eq!(probe_audio_streams(&path), vec![4]);
        assert!(probe_audio_streams(&path.with_extension("missing")).is_empty());
        let _ = std::fs::remove_file(path);
    }
}
