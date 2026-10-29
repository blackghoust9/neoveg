//! *Make Movie* without FFmpeg: H.264 video and AAC audio in an MP4 file, all in Rust.
//!
//! The frames come from the same compositor the FFmpeg path uses, and the sound from the same
//! mixdown WAV; only the encoder and the muxer differ ([`neoveg_h264`], [`neoveg_aac`] and
//! [`neoveg_media`], adapted from FilmCraft). The file is written with `moov` in front of `mdat`
//! (fast start) and the audio interleaved with the video, one second of each at a time.

use std::fs::OpenOptions;
use std::io::{BufReader, Read};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use neoveg_media::isobmff::{
    AvcConfig, Brand, Mp4Writer, SampleEntry, TrackConfig, WriteSample, WriterOptions,
};

use crate::frame::FrameBuffer;
use crate::render::{RenderFormat, VideoEncodeSpec};

/// `true` for the formats the built-in encoders can write.
pub fn supports(format: RenderFormat) -> bool {
    matches!(format, RenderFormat::Mp4H264)
}

/// A frame rate as the numerator and denominator the container and the encoder want (`30000/1001`
/// for 29.97, `24000/1001` for 23.976, `25/1` for 25).
pub fn frame_rate_ratio(fps: f64) -> (u32, u32) {
    let fps = if fps.is_finite() && fps > 0.0 {
        fps
    } else {
        25.0
    };
    for (num, den) in [
        (24000u32, 1001u32),
        (30000, 1001),
        (60000, 1001),
        (120000, 1001),
    ] {
        if (fps - f64::from(num) / f64::from(den)).abs() < 0.005 {
            return (num, den);
        }
    }
    if (fps - fps.round()).abs() < 0.005 {
        (fps.round().max(1.0) as u32, 1)
    } else {
        ((fps * 1000.0).round().max(1.0) as u32, 1000)
    }
}

/// 8-bit limited-range BT.709 4:2:0 from straight RGBA8, drawn over black (the 2 by 2 chroma
/// average takes the colour differences of the four pixels).
pub fn rgba_to_yuv420(
    rgba: &[u8],
    w: usize,
    h: usize,
    y: &mut Vec<u8>,
    u: &mut Vec<u8>,
    v: &mut Vec<u8>,
) {
    let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
    y.clear();
    y.resize(w * h, 16);
    u.clear();
    u.resize(cw * ch, 128);
    v.clear();
    v.resize(cw * ch, 128);
    let code = |x: f32| x.round().clamp(0.0, 255.0) as u8;
    for cy in 0..ch {
        for cx in 0..cw {
            let (mut sum_b, mut sum_r, mut count) = (0.0f32, 0.0f32, 0.0f32);
            for dy in 0..2 {
                for dx in 0..2 {
                    let (px, py) = ((cx * 2 + dx).min(w - 1), (cy * 2 + dy).min(h - 1));
                    let i = (py * w + px) * 4;
                    let Some(p) = rgba.get(i..i + 4) else {
                        continue;
                    };
                    let a = f32::from(p[3]) / 255.0 / 255.0;
                    let (r, g, b) = (
                        f32::from(p[0]) * a,
                        f32::from(p[1]) * a,
                        f32::from(p[2]) * a,
                    );
                    let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
                    if (cx * 2 + dx) < w && (cy * 2 + dy) < h {
                        y[py * w + px] = code(16.0 + 219.0 * luma);
                    }
                    sum_b += b - luma;
                    sum_r += r - luma;
                    count += 1.0;
                }
            }
            let count = count.max(1.0);
            u[cy * cw + cx] = code(128.0 + 224.0 * (sum_b / count) / 1.8556);
            v[cy * cw + cx] = code(128.0 + 224.0 * (sum_r / count) / 1.5748);
        }
    }
}

/// The sound of a 16-bit PCM WAV file, read a block at a time.
struct WavReader {
    file: BufReader<std::fs::File>,
    pub rate: u32,
    pub channels: usize,
    remaining: u64,
}

impl WavReader {
    fn open(path: &Path) -> Result<Self, String> {
        let file = std::fs::File::open(path).map_err(|e| format!("mixagem de áudio: {e}"))?;
        let mut file = BufReader::new(file);
        let mut head = [0u8; 12];
        file.read_exact(&mut head)
            .map_err(|e| format!("mixagem de áudio: {e}"))?;
        if &head[0..4] != b"RIFF" || &head[8..12] != b"WAVE" {
            return Err("a mixagem de áudio não é um arquivo WAV".into());
        }
        let (mut rate, mut channels, mut bits) = (0u32, 0usize, 0u16);
        loop {
            let mut chunk = [0u8; 8];
            file.read_exact(&mut chunk)
                .map_err(|_| "a mixagem de áudio não tem dados".to_string())?;
            let size = u64::from(u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]));
            match &chunk[0..4] {
                b"fmt " => {
                    let mut f = vec![0u8; size as usize];
                    file.read_exact(&mut f)
                        .map_err(|e| format!("mixagem de áudio: {e}"))?;
                    if f.len() < 16 {
                        return Err("cabeçalho WAV curto demais".into());
                    }
                    channels = usize::from(u16::from_le_bytes([f[2], f[3]]));
                    rate = u32::from_le_bytes([f[4], f[5], f[6], f[7]]);
                    bits = u16::from_le_bytes([f[14], f[15]]);
                    if size % 2 == 1 {
                        let mut pad = [0u8; 1];
                        let _ = file.read_exact(&mut pad);
                    }
                }
                b"data" => {
                    if bits != 16 || !(1..=2).contains(&channels) || rate == 0 {
                        return Err(format!(
                            "mixagem de áudio não suportada ({channels} canais, {bits} bits)"
                        ));
                    }
                    return Ok(WavReader {
                        file,
                        rate,
                        channels,
                        remaining: size,
                    });
                }
                _ => {
                    std::io::copy(
                        &mut (&mut file).take(size + (size % 2)),
                        &mut std::io::sink(),
                    )
                    .map_err(|e| format!("mixagem de áudio: {e}"))?;
                }
            }
        }
    }

    /// Up to `frames` sample frames as one `Vec` of floats per channel; empty at the end.
    fn read(&mut self, frames: usize) -> Result<Vec<Vec<f32>>, String> {
        let bytes_per_frame = (self.channels * 2) as u64;
        let want = (frames as u64 * bytes_per_frame)
            .min(self.remaining - self.remaining % bytes_per_frame);
        let mut raw = vec![0u8; want as usize];
        self.file
            .read_exact(&mut raw)
            .map_err(|e| format!("mixagem de áudio: {e}"))?;
        self.remaining -= want;
        let n = raw.len() / (self.channels * 2);
        let mut planar = vec![Vec::with_capacity(n); self.channels];
        for frame in raw.chunks_exact(self.channels * 2) {
            for (c, plane) in planar.iter_mut().enumerate() {
                let s = i16::from_le_bytes([frame[c * 2], frame[c * 2 + 1]]);
                plane.push(f32::from(s) / 32768.0);
            }
        }
        Ok(planar)
    }
}

fn cancelled() -> String {
    "renderização cancelada pelo usuário".into()
}

/// Encodes `spec.total_frames` generated by `produce_frame(frame_idx)`, and the mixdown in
/// `spec.audio_wav`, into the MP4 file `spec.output`.
pub fn encode_video_native(
    spec: &VideoEncodeSpec<'_>,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(u64, u64),
    mut produce_frame: impl FnMut(u64) -> Result<FrameBuffer, String>,
) -> Result<(), String> {
    if !supports(spec.format) {
        return Err(format!(
            "o codificador interno só escreve MP4 H.264 + AAC (formato pedido: {})",
            spec.format.label()
        ));
    }
    if let Some(parent) = spec.output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("falha ao criar pasta {}: {e}", parent.display()))?;
        }
    }
    let result = encode_inner(spec, cancel, &mut on_progress, &mut produce_frame);
    if result.is_err() {
        let _ = std::fs::remove_file(spec.output);
    }
    result
}

fn encode_inner(
    spec: &VideoEncodeSpec<'_>,
    cancel: &AtomicBool,
    on_progress: &mut impl FnMut(u64, u64),
    produce_frame: &mut impl FnMut(u64) -> Result<FrameBuffer, String>,
) -> Result<(), String> {
    let (w, h) = (spec.width.max(2) & !1, spec.height.max(2) & !1);
    let (num, den) = frame_rate_ratio(spec.fps);

    // The video encoder
    let mut cfg = neoveg_h264::EncoderConfig::new(w, h, num, den);
    cfg.format = neoveg_h264::PacketFormat::LengthPrefixed;
    cfg.rate = neoveg_h264::RateControl::Crf(f32::from(spec.crf.clamp(0, 51)));
    cfg.keyint = (f64::from(num) / f64::from(den) * 2.0).round().max(1.0) as u32;
    cfg.slices = (h.div_ceil(16) as usize).div_ceil(4).max(1);
    cfg.aud = false;
    let mut venc = neoveg_h264::Encoder::new(cfg).map_err(|e| format!("codificador H.264: {e}"))?;
    let avc = AvcConfig::parse(&venc.avcc()).map_err(|e| format!("configuração H.264: {e}"))?;

    // The sound: the whole mixdown is encoded first (AAC units are small)
    let mut audio: Option<(neoveg_aac::Encoder, u32, usize, Vec<Vec<u8>>)> = None;
    if let Some(path) = spec.audio_wav {
        let mut wav = WavReader::open(path)?;
        let bits = (192_000u32).min(6 * wav.rate * wav.channels as u32);
        let mut enc =
            neoveg_aac::Encoder::new(neoveg_aac::EncoderConfig::cbr(wav.rate, wav.channels, bits))
                .map_err(|e| format!("codificador AAC: {e}"))?;
        let mut units = Vec::new();
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(cancelled());
            }
            let block = wav.read(wav.rate as usize)?;
            if block.first().is_none_or(Vec::is_empty) {
                break;
            }
            let refs: Vec<&[f32]> = block.iter().map(Vec::as_slice).collect();
            units.extend(enc.encode(&refs));
        }
        units.extend(enc.flush());
        audio = Some((enc, wav.rate, wav.channels, units));
    }

    // The container
    let file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .read(true)
        .write(true)
        .open(spec.output)
        .map_err(|e| format!("falha ao criar {}: {e}", spec.output.display()))?;
    let mut mux =
        Mp4Writer::new(file, WriterOptions::new(Brand::Mp4)).map_err(|e| e.to_string())?;
    let mut vtrack = TrackConfig::new(SampleEntry::avc(avc, w as u16, h as u16), num);
    vtrack.media_start = (venc.delay() > 0).then_some(i64::from(den));
    let video_track = mux.add_track(vtrack).map_err(|e| e.to_string())?;
    let audio_track = match &audio {
        Some((enc, rate, channels, _)) => {
            let mut t = TrackConfig::new(
                SampleEntry::aac(enc.audio_specific_config(), *channels as u32, *rate),
                *rate,
            );
            t.media_start = Some(i64::from(enc.priming_samples()));
            Some(mux.add_track(t).map_err(|e| e.to_string())?)
        }
        None => None,
    };
    let mut next_unit = 0usize;
    let mut write_audio_until = |mux: &mut Mp4Writer<std::fs::File>,
                                 seconds: f64|
     -> Result<(), String> {
        let (Some(track), Some((_, rate, _, units))) = (audio_track, audio.as_ref()) else {
            return Ok(());
        };
        while next_unit < units.len() && (next_unit as f64 * 1024.0 / f64::from(*rate)) <= seconds {
            mux.write_sample(
                track,
                WriteSample {
                    data: &units[next_unit],
                    duration: 1024,
                    composition_offset: 0,
                    is_sync: true,
                },
            )
            .map_err(|e| e.to_string())?;
            next_unit += 1;
        }
        Ok(())
    };

    // The pictures
    let (mut y, mut u, mut v) = (Vec::new(), Vec::new(), Vec::new());
    let write_packets = |mux: &mut Mp4Writer<std::fs::File>,
                         packets: Vec<neoveg_h264::Packet>|
     -> Result<(), String> {
        for p in packets {
            mux.write_sample(
                video_track,
                WriteSample {
                    data: &p.data,
                    duration: den,
                    composition_offset: (p.pts - p.dts) as i32,
                    is_sync: p.keyframe,
                },
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    };
    for index in 0..spec.total_frames {
        if cancel.load(Ordering::Relaxed) {
            return Err(cancelled());
        }
        let frame = produce_frame(index)?;
        let fitted = crate::composite::fit_layer_to_canvas(&frame, w, h);
        if fitted.rgba.len() != (w as usize) * (h as usize) * 4 {
            return Err("tamanho de quadro inválido durante a renderização".into());
        }
        rgba_to_yuv420(&fitted.rgba, w as usize, h as usize, &mut y, &mut u, &mut v);
        let yuv = neoveg_h264::YuvFrame {
            y: &y,
            u: &u,
            v: &v,
            y_stride: w as usize,
            uv_stride: (w as usize).div_ceil(2),
        };
        let packets = venc
            .try_encode(&yuv, index as i64 * i64::from(den))
            .map_err(|e| format!("codificador H.264: {e}"))?;
        write_packets(&mut mux, packets)?;
        write_audio_until(
            &mut mux,
            (index + 1) as f64 * f64::from(den) / f64::from(num),
        )?;
        on_progress(index + 1, spec.total_frames);
    }
    write_packets(&mut mux, venc.flush())?;
    write_audio_until(&mut mux, f64::INFINITY)?;
    mux.finish_faststart().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use neoveg_media::isobmff::{open, TrackKind};

    fn write_wav(path: &Path, rate: u32, seconds: f64) {
        let frames = (f64::from(rate) * seconds) as usize;
        let mut data = Vec::new();
        for i in 0..frames {
            let s =
                ((i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 12_000.0) as i16;
            data.extend_from_slice(&s.to_le_bytes());
            data.extend_from_slice(&s.to_le_bytes());
        }
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&rate.to_le_bytes());
        out.extend_from_slice(&(rate * 4).to_le_bytes());
        out.extend_from_slice(&4u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&data);
        std::fs::write(path, out).unwrap();
    }

    fn spec<'a>(output: &'a Path, wav: Option<&'a Path>, frames: u64) -> VideoEncodeSpec<'a> {
        VideoEncodeSpec {
            format: RenderFormat::Mp4H264,
            width: 128,
            height: 72,
            fps: 25.0,
            crf: 23,
            total_frames: frames,
            audio_wav: wav,
            output,
        }
    }

    #[test]
    fn the_frame_rate_becomes_the_ratio_the_container_wants() {
        assert_eq!(frame_rate_ratio(25.0), (25, 1));
        assert_eq!(frame_rate_ratio(29.97), (30000, 1001));
        assert_eq!(frame_rate_ratio(23.976), (24000, 1001));
        assert_eq!(frame_rate_ratio(59.94), (60000, 1001));
        assert_eq!(frame_rate_ratio(12.5), (12500, 1000));
        assert_eq!(frame_rate_ratio(f64::NAN), (25, 1));
    }

    #[test]
    fn the_colour_conversion_matches_bt709_limited_range() {
        let px = |rgb: [u8; 3]| {
            let (mut y, mut u, mut v) = (Vec::new(), Vec::new(), Vec::new());
            rgba_to_yuv420(
                &[rgb[0], rgb[1], rgb[2], 255].repeat(4),
                2,
                2,
                &mut y,
                &mut u,
                &mut v,
            );
            (y[0], u[0], v[0])
        };
        assert_eq!(px([0, 0, 0]), (16, 128, 128));
        assert_eq!(px([255, 255, 255]), (235, 128, 128));
        let red = px([255, 0, 0]);
        assert_eq!(red.0, 63);
        assert!(red.2 > 230 && red.1 < 110, "{red:?}");
        // Transparent pixels are black
        let (mut y, mut u, mut v) = (Vec::new(), Vec::new(), Vec::new());
        rgba_to_yuv420(&[255, 255, 255, 0].repeat(4), 2, 2, &mut y, &mut u, &mut v);
        assert_eq!((y[0], u[0], v[0]), (16, 128, 128));
    }

    #[test]
    fn a_short_movie_is_written_as_a_valid_mp4_with_video_and_audio() {
        let dir = std::env::temp_dir().join(format!("neoveg_native_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (wav, out) = (dir.join("mix.wav"), dir.join("filme.mp4"));
        write_wav(&wav, 48_000, 1.0);
        let mut progress = Vec::new();
        encode_video_native(
            &spec(&out, Some(&wav), 25),
            &AtomicBool::new(false),
            |done, total| progress.push((done, total)),
            |i| {
                let shade = (i * 8) as u8;
                Ok(FrameBuffer::solid(128, 72, [shade, 80, 200 - shade, 255]))
            },
        )
        .unwrap();
        assert_eq!(progress.last(), Some(&(25, 25)));
        let bytes = std::fs::read(&out).unwrap();
        let file = open(bytes).unwrap();
        assert_eq!(file.tracks.len(), 2);
        let video = file
            .tracks
            .iter()
            .find(|t| t.kind == TrackKind::Video)
            .unwrap();
        let audio = file
            .tracks
            .iter()
            .find(|t| t.kind == TrackKind::Audio)
            .unwrap();
        assert_eq!((video.width, video.height), (128, 72));
        assert_eq!(video.samples.len(), 25);
        assert_eq!(audio.timescale, 48_000);
        assert!(
            audio.samples.len() >= 46,
            "{} audio units",
            audio.samples.len()
        );
        assert!(video.samples.iter().any(|s| s.is_sync));

        // A second opinion from FFmpeg when it is installed: the file decodes without errors
        if let Ok(status) = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-i"])
            .arg(&out)
            .args(["-f", "null", "-"])
            .status()
        {
            assert!(status.success(), "ffmpeg could not decode the file");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_cancelled_render_leaves_no_file_and_other_formats_are_refused() {
        let dir = std::env::temp_dir().join(format!("neoveg_native_c_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("cancelado.mp4");
        let cancel = AtomicBool::new(false);
        let result = encode_video_native(
            &spec(&out, None, 50),
            &cancel,
            |done, _| {
                if done == 3 {
                    cancel.store(true, Ordering::Relaxed);
                }
            },
            |_| Ok(FrameBuffer::solid(128, 72, [10, 20, 30, 255])),
        );
        assert_eq!(result, Err("renderização cancelada pelo usuário".into()));
        assert!(!out.exists());
        let mut other = spec(&out, None, 1);
        other.format = RenderFormat::WebmVp9;
        assert!(encode_video_native(&other, &cancel, |_, _| {}, |_| unreachable_frame()).is_err());
        assert!(supports(RenderFormat::Mp4H264) && !supports(RenderFormat::MovProRes));
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn unreachable_frame() -> Result<FrameBuffer, String> {
        Err("não deveria renderizar".into())
    }
}
