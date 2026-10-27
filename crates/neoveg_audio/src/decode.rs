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

/// Decodes the first decodable audio track of `path` (the sound of a video file included).
pub fn decode_file(path: &Path) -> Result<DecodedAudio, String> {
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
    let mut samples: Vec<f32> = Vec::new();

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
                let mut buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
                buf.copy_interleaved_ref(decoded);
                samples.extend_from_slice(buf.samples());
            }
            // A corrupt packet is skipped; the rest of the file is still usable
            Err(Error::DecodeError(e)) => {
                tracing::warn!("decode error in {}: {e}", path.display());
            }
            Err(e) => return Err(format!("decode error in {}: {e}", path.display())),
        }
    }

    if sample_rate == 0 || channels == 0 || samples.is_empty() {
        return Err(format!("{} contains no decodable audio", path.display()));
    }
    Ok(DecodedAudio {
        sample_rate,
        channels,
        samples,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn missing_file_is_an_error() {
        assert!(decode_file(Path::new("does-not-exist.wav")).is_err());
    }
}
