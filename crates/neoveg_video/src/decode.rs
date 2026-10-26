//! FFmpeg video decoding with seek-to-frame (`ffmpeg` feature).

use std::path::Path;

use ffmpeg::format::{context::Input, Pixel};
use ffmpeg::media::Type;
use ffmpeg::software::scaling::{context::Context as Scaler, flag::Flags};
use ffmpeg::util::frame::video::Video;
use ffmpeg_next as ffmpeg;

use crate::frame::FrameBuffer;
use crate::info::{fit_size, snap_fps, target_pts, VideoInfo};

/// Seeking is skipped when the target is this close ahead of the last decoded frame.
const FORWARD_DECODE_WINDOW_SECONDS: f64 = 1.0;

fn open_input(path: &Path) -> Result<Input, String> {
    ffmpeg::init().map_err(|e| format!("ffmpeg init: {e}"))?;
    ffmpeg::format::input(&path).map_err(|e| format!("cannot open {}: {e}", path.display()))
}

/// Reads dimensions, frame rate and duration without decoding.
pub fn probe(path: &Path) -> Result<VideoInfo, String> {
    let input = open_input(path)?;
    let stream = input
        .streams()
        .best(Type::Video)
        .ok_or_else(|| format!("{} has no video stream", path.display()))?;
    let context = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
        .map_err(|e| format!("codec parameters: {e}"))?;
    let video = context
        .decoder()
        .video()
        .map_err(|e| format!("video decoder: {e}"))?;

    let rate = stream.avg_frame_rate();
    let fps = if rate.denominator() > 0 && rate.numerator() > 0 {
        rate.numerator() as f64 / rate.denominator() as f64
    } else {
        0.0
    };
    let container = input.duration();
    let duration = if container > 0 {
        container as f64 / f64::from(ffmpeg::ffi::AV_TIME_BASE)
    } else {
        let tb = stream.time_base();
        stream.duration().max(0) as f64 * tb.numerator() as f64 / tb.denominator().max(1) as f64
    };
    Ok(VideoInfo {
        width: video.width(),
        height: video.height(),
        fps: snap_fps(fps),
        duration,
    })
}

/// A decoder that can return the frame at any time, seeking only when needed.
pub struct VideoDecoder {
    input: Input,
    stream_index: usize,
    decoder: ffmpeg::decoder::Video,
    time_base: (i32, i32),
    /// Last frame returned (timestamp, picture), reused for repeated or slightly later requests.
    current: Option<(i64, Video)>,
    /// First decoded frame past the previous target; it belongs to the next request.
    pending: Option<(i64, Video)>,
    scaler: Option<ScalerState>,
}

/// Keeps `frame` as the best candidate while it is not after `limit`; the first frame past the limit
/// ends the search (and is parked as `pending` when a better candidate already exists).
fn consider(
    best: &mut Option<(i64, Video)>,
    pending: &mut Option<(i64, Video)>,
    pts: i64,
    frame: Video,
    limit: i64,
) -> bool {
    if pts > limit {
        if best.is_none() {
            *best = Some((pts, frame));
        } else {
            *pending = Some((pts, frame));
        }
        true
    } else {
        *best = Some((pts, frame));
        false
    }
}

struct ScalerState {
    scaler: Scaler,
    format: Pixel,
    source: (u32, u32),
    target: (u32, u32),
}

impl VideoDecoder {
    pub fn open(path: &Path) -> Result<Self, String> {
        let input = open_input(path)?;
        let stream = input
            .streams()
            .best(Type::Video)
            .ok_or_else(|| format!("{} has no video stream", path.display()))?;
        let stream_index = stream.index();
        let tb = stream.time_base();
        let context = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
            .map_err(|e| format!("codec parameters: {e}"))?;
        let decoder = context
            .decoder()
            .video()
            .map_err(|e| format!("video decoder: {e}"))?;
        Ok(Self {
            input,
            stream_index,
            decoder,
            time_base: (tb.numerator(), tb.denominator()),
            current: None,
            pending: None,
            scaler: None,
        })
    }

    /// Frame displayed at `seconds` (the last frame whose timestamp is not after it), scaled to at most
    /// `max_width` pixels wide (0 = source size).
    pub fn frame_at(&mut self, seconds: f64, max_width: u32) -> Result<FrameBuffer, String> {
        let seconds = seconds.max(0.0);
        let (num, den) = self.time_base;
        let target = target_pts(seconds, num, den);
        let eps = target_pts(0.001, num, den);
        let window = target_pts(FORWARD_DECODE_WINDOW_SECONDS, num, den);

        let can_continue = self
            .current
            .as_ref()
            .is_some_and(|(pts, _)| target + eps >= *pts && target - pts < window);
        if !can_continue {
            let micros = (seconds * f64::from(ffmpeg::ffi::AV_TIME_BASE)) as i64;
            self.input
                .seek(micros, ..micros)
                .map_err(|e| format!("seek: {e}"))?;
            self.decoder.flush();
            self.current = None;
            self.pending = None;
        }

        let mut best = self.current.take();
        let mut stopped = false;
        if let Some((pts, frame)) = self.pending.take() {
            stopped = consider(&mut best, &mut self.pending, pts, frame, target + eps);
        }

        let mut decoded = Video::empty();
        if !stopped {
            'packets: for (stream, packet) in self.input.packets() {
                if stream.index() != self.stream_index {
                    continue;
                }
                if self.decoder.send_packet(&packet).is_err() {
                    continue;
                }
                while self.decoder.receive_frame(&mut decoded).is_ok() {
                    let pts = decoded.timestamp().or(decoded.pts()).unwrap_or(0);
                    if consider(
                        &mut best,
                        &mut self.pending,
                        pts,
                        decoded.clone(),
                        target + eps,
                    ) {
                        stopped = true;
                        break 'packets;
                    }
                }
            }
        }
        if !stopped {
            // End of stream: drain the decoder and keep the last frame it produced
            let _ = self.decoder.send_eof();
            while self.decoder.receive_frame(&mut decoded).is_ok() {
                let pts = decoded.timestamp().or(decoded.pts()).unwrap_or(0);
                if consider(
                    &mut best,
                    &mut self.pending,
                    pts,
                    decoded.clone(),
                    target + eps,
                ) {
                    break;
                }
            }
        }

        let (pts, frame) = best.ok_or("no decodable video frame")?;
        self.current = Some((pts, frame.clone()));
        self.rgba_frame(&frame, max_width)
    }

    fn rgba_frame(&mut self, frame: &Video, max_width: u32) -> Result<FrameBuffer, String> {
        let source = (frame.width(), frame.height());
        let target = fit_size(source.0, source.1, max_width);
        let format = frame.format();

        let rebuild = self
            .scaler
            .as_ref()
            .is_none_or(|s| s.format != format || s.source != source || s.target != target);
        if rebuild {
            let scaler = Scaler::get(
                format,
                source.0,
                source.1,
                Pixel::RGBA,
                target.0,
                target.1,
                Flags::BILINEAR,
            )
            .map_err(|e| format!("scaler: {e}"))?;
            self.scaler = Some(ScalerState {
                scaler,
                format,
                source,
                target,
            });
        }
        let state = self.scaler.as_mut().ok_or("scaler unavailable")?;

        let mut rgba = Video::empty();
        state
            .scaler
            .run(frame, &mut rgba)
            .map_err(|e| format!("scale: {e}"))?;

        let (w, h) = (rgba.width() as usize, rgba.height() as usize);
        let stride = rgba.stride(0);
        let src = rgba.data(0);
        let mut out = Vec::with_capacity(w * h * 4);
        for row in 0..h {
            out.extend_from_slice(&src[row * stride..row * stride + w * 4]);
        }
        Ok(FrameBuffer {
            width: w as u32,
            height: h as u32,
            rgba: out,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    /// Builds a 2 s, 25 fps test clip with the `ffmpeg` CLI; `None` when the CLI is unavailable.
    fn make_clip(tag: &str) -> Option<std::path::PathBuf> {
        let path = std::env::temp_dir().join(format!("neoveg_{tag}_{}.mp4", std::process::id()));
        let ok = Command::new("ffmpeg")
            .args(["-y", "-v", "error", "-f", "lavfi", "-i"])
            .arg("testsrc=duration=2:size=320x240:rate=25")
            .args(["-c:v", "mpeg4", "-g", "10"])
            .arg(&path)
            .status()
            .ok()?
            .success();
        ok.then_some(path)
    }

    #[test]
    fn probe_reads_dimensions_rate_and_duration() {
        let Some(path) = make_clip("probe") else {
            return;
        };
        let info = probe(&path).unwrap();
        assert_eq!((info.width, info.height), (320, 240));
        assert_eq!(info.fps, 25.0);
        assert!((info.duration - 2.0).abs() < 0.1, "{}", info.duration);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn seeking_forward_and_backward_returns_different_frames() {
        let Some(path) = make_clip("seek") else {
            return;
        };
        let mut dec = VideoDecoder::open(&path).unwrap();
        let early = dec.frame_at(0.2, 0).unwrap();
        let late = dec.frame_at(1.6, 0).unwrap();
        let early_again = dec.frame_at(0.2, 0).unwrap();
        assert_eq!((early.width, early.height), (320, 240));
        assert_ne!(early.rgba, late.rgba);
        assert_eq!(early.rgba, early_again.rgba);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn sequential_stepping_matches_independent_seeks() {
        let Some(path) = make_clip("step") else {
            return;
        };
        let mut stepping = VideoDecoder::open(&path).unwrap();
        for k in 8..16 {
            let t = k as f64 / 25.0;
            let stepped = stepping.frame_at(t, 0).unwrap();
            let repeated = stepping.frame_at(t, 0).unwrap();
            let fresh = VideoDecoder::open(&path).unwrap().frame_at(t, 0).unwrap();
            assert_eq!(stepped.rgba, fresh.rgba, "frame {k}");
            assert_eq!(stepped.rgba, repeated.rgba, "repeat {k}");
        }
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn frames_are_scaled_to_the_requested_width() {
        let Some(path) = make_clip("scale") else {
            return;
        };
        let mut dec = VideoDecoder::open(&path).unwrap();
        let small = dec.frame_at(0.5, 160).unwrap();
        assert_eq!((small.width, small.height), (160, 120));
        assert_eq!(small.rgba.len(), 160 * 120 * 4);
        let past_end = dec.frame_at(10.0, 0).unwrap();
        assert_eq!(past_end.width, 320);
        let _ = std::fs::remove_file(path);
    }
}
