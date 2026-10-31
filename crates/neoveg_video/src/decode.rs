//! FFmpeg video decoding with seek-to-frame (`ffmpeg` feature).

use std::path::{Path, PathBuf};

use ffmpeg::ffi;
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

// ---- decoding on the graphics card ------------------------------------------------------------

/// The way this platform decodes on the graphics card: the device type and the pixel format of its
/// surfaces. `None` where the program has no support.
#[cfg(windows)]
const HARDWARE: Option<(ffi::AVHWDeviceType, ffi::AVPixelFormat)> = Some((
    ffi::AVHWDeviceType::AV_HWDEVICE_TYPE_D3D11VA,
    ffi::AVPixelFormat::AV_PIX_FMT_D3D11,
));
#[cfg(target_os = "macos")]
const HARDWARE: Option<(ffi::AVHWDeviceType, ffi::AVPixelFormat)> = Some((
    ffi::AVHWDeviceType::AV_HWDEVICE_TYPE_VIDEOTOOLBOX,
    ffi::AVPixelFormat::AV_PIX_FMT_VIDEOTOOLBOX,
));
#[cfg(all(unix, not(target_os = "macos")))]
const HARDWARE: Option<(ffi::AVHWDeviceType, ffi::AVPixelFormat)> = Some((
    ffi::AVHWDeviceType::AV_HWDEVICE_TYPE_VAAPI,
    ffi::AVPixelFormat::AV_PIX_FMT_VAAPI,
));
#[cfg(not(any(windows, unix)))]
const HARDWARE: Option<(ffi::AVHWDeviceType, ffi::AVPixelFormat)> = None;

/// Asked by the decoder which pixel format to use: the one of the graphics card when the codec offers
/// it, otherwise the first ordinary (software) one.
unsafe extern "C" fn choose_format(
    _context: *mut ffi::AVCodecContext,
    formats: *const ffi::AVPixelFormat,
) -> ffi::AVPixelFormat {
    let Some((_, hardware)) = HARDWARE else {
        return ffi::AVPixelFormat::AV_PIX_FMT_NONE;
    };
    let mut software = ffi::AVPixelFormat::AV_PIX_FMT_NONE;
    let mut at = formats;
    while *at != ffi::AVPixelFormat::AV_PIX_FMT_NONE {
        if *at == hardware {
            return hardware;
        }
        let descriptor = ffi::av_pix_fmt_desc_get(*at);
        let is_hardware = !descriptor.is_null()
            && (*descriptor).flags & (ffi::AV_PIX_FMT_FLAG_HWACCEL as u64) != 0;
        if software == ffi::AVPixelFormat::AV_PIX_FMT_NONE && !is_hardware {
            software = *at;
        }
        at = at.add(1);
    }
    software
}

/// Makes the decoder of `context` use the graphics card; `false` when there is no device (no card,
/// no driver, a remote session) and the processor decodes as usual.
fn enable_hardware(context: &mut ffmpeg::codec::context::Context) -> bool {
    let Some((device_type, _)) = HARDWARE else {
        return false;
    };
    // SAFETY: `context` owns the codec context; the device reference taken here is released by it.
    unsafe {
        let mut device: *mut ffi::AVBufferRef = std::ptr::null_mut();
        let made = ffi::av_hwdevice_ctx_create(
            &mut device,
            device_type,
            std::ptr::null(),
            std::ptr::null_mut(),
            0,
        );
        if made < 0 || device.is_null() {
            return false;
        }
        let raw = context.as_mut_ptr();
        (*raw).hw_device_ctx = ffi::av_buffer_ref(device);
        (*raw).get_format = Some(choose_format);
        ffi::av_buffer_unref(&mut device);
        !(*raw).hw_device_ctx.is_null()
    }
}

/// Whether this machine can decode on the graphics card at all (a device can be created).
pub fn hardware_available() -> bool {
    let Some((device_type, _)) = HARDWARE else {
        return false;
    };
    // SAFETY: the device is created and released right here.
    unsafe {
        let mut device: *mut ffi::AVBufferRef = std::ptr::null_mut();
        let made = ffi::av_hwdevice_ctx_create(
            &mut device,
            device_type,
            std::ptr::null(),
            std::ptr::null_mut(),
            0,
        );
        if made >= 0 && !device.is_null() {
            ffi::av_buffer_unref(&mut device);
            true
        } else {
            false
        }
    }
}

/// Makes the scaler read the colours the way the picture says. Without this it assumes the limited
/// range (16 to 235) and the BT.601 matrix for everything: a picture marked as full range (phone
/// videos, screen recordings) lost its dark tones (everything under 16 turned black) and one marked
/// BT.709 or BT.2020 came out with shifted colours. A picture that says nothing keeps the
/// scaler's own choice.
fn follow_colours(scaler: &mut Scaler, range: ffmpeg::color::Range, space: ffmpeg::color::Space) {
    use ffmpeg::color::{Range, Space};
    let code = match space {
        Space::BT709 => Some(ffi::SWS_CS_ITU709),
        Space::BT2020NCL | Space::BT2020CL => Some(ffi::SWS_CS_BT2020),
        Space::SMPTE240M => Some(ffi::SWS_CS_SMPTE240M),
        Space::FCC => Some(ffi::SWS_CS_FCC),
        Space::SMPTE170M | Space::BT470BG => Some(ffi::SWS_CS_ITU601),
        _ => None,
    };
    let full = range == Range::JPEG;
    if code.is_none() && !full {
        return;
    }
    // SAFETY: the scaler owns its context for the whole call; the tables are static.
    unsafe {
        let table = ffi::sws_getCoefficients(code.unwrap_or(ffi::SWS_CS_DEFAULT));
        ffi::sws_setColorspaceDetails(
            scaler.as_mut_ptr(),
            table,
            i32::from(full),
            table,
            1,
            0,
            1 << 16,
            1 << 16,
        );
    }
}

/// When the graphics card decodes a video.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwareMode {
    /// Never: the processor does it.
    Off,
    /// Only for the codecs that are heavy for a processor (HEVC, AV1, VP9). Measured on a Ryzen with
    /// an integrated card, H.264 is faster on the processor (220 against 117 frames per second at
    /// 1080p, and a jump to another place 41 ms against 301 ms), while HEVC is faster on the card
    /// (a jump 68 ms against 132 ms at 1080p, 214 against 319 ms at 4K).
    Heavy,
    /// Whatever the codec, when the card can: for tests and measurements.
    Always,
}

/// The frame in ordinary memory: a frame that lives on the graphics card is copied down (NV12 or
/// P010), any other is kept as it is. The second value says whether the card had it.
fn into_memory(frame: &Video) -> Result<(Video, bool), String> {
    // SAFETY: both frames are valid for the whole call; the copy gets the properties of the original.
    unsafe {
        if (*frame.as_ptr()).hw_frames_ctx.is_null() {
            return Ok((frame.clone(), false));
        }
        let mut memory = Video::empty();
        let copied = ffi::av_hwframe_transfer_data(memory.as_mut_ptr(), frame.as_ptr(), 0);
        if copied < 0 {
            return Err(format!(
                "copying a frame from the graphics card failed ({copied})"
            ));
        }
        ffi::av_frame_copy_props(memory.as_mut_ptr(), frame.as_ptr());
        Ok((memory, true))
    }
}

/// Quarter turns clockwise (0 to 3) the container asks for when the picture is shown: a phone
/// held upright films sideways and stores a display matrix; a HEIF photo stores `irot`. Read as
/// the command line shows it (checked against it by a test: 90 is a clockwise turn).
fn stream_quarter_turns(stream: &ffmpeg::format::stream::Stream) -> u32 {
    for data in stream.side_data() {
        if data.kind() != ffmpeg::codec::packet::side_data::Type::DisplayMatrix {
            continue;
        }
        let bytes = data.data();
        if bytes.len() < 36 {
            continue;
        }
        let m: Vec<f64> = (0..9)
            .map(|i| {
                f64::from(i32::from_le_bytes(
                    bytes[i * 4..i * 4 + 4].try_into().unwrap(),
                ))
            })
            .collect();
        let (scale0, scale1) = (m[0].hypot(m[3]), m[1].hypot(m[4]));
        if scale0 == 0.0 || scale1 == 0.0 {
            continue;
        }
        let theta = (m[1] / scale1).atan2(m[0] / scale0).to_degrees();
        return ((theta / 90.0).round() as i32).rem_euclid(4) as u32;
    }
    0
}

/// `rgba` (`width` x `height`) turned `quarter_turns` times clockwise.
fn rotate_rgba(
    rgba: Vec<u8>,
    width: usize,
    height: usize,
    quarter_turns: u32,
) -> (Vec<u8>, usize, usize) {
    if quarter_turns.is_multiple_of(4) {
        return (rgba, width, height);
    }
    let (new_w, new_h) = if quarter_turns % 2 == 1 {
        (height, width)
    } else {
        (width, height)
    };
    let mut out = vec![0u8; rgba.len()];
    for y in 0..height {
        for x in 0..width {
            let (nx, ny) = match quarter_turns % 4 {
                1 => (height - 1 - y, x),
                2 => (width - 1 - x, height - 1 - y),
                _ => (y, width - 1 - x),
            };
            let from = (y * width + x) * 4;
            let to = (ny * new_w + nx) * 4;
            out[to..to + 4].copy_from_slice(&rgba[from..from + 4]);
        }
    }
    (out, new_w, new_h)
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
    // The size is the one shown, after the rotation the file asks for
    let (width, height) = if stream_quarter_turns(&stream) % 2 == 1 {
        (video.height(), video.width())
    } else {
        (video.width(), video.height())
    };
    Ok(VideoInfo {
        width,
        height,
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
    /// Quarter turns clockwise to apply to every frame (see [`stream_quarter_turns`]).
    quarter_turns: u32,
    path: PathBuf,
    /// The decoder was set up to use the graphics card.
    hardware: bool,
    /// A frame has come from the graphics card (the codec and the card did take part).
    used_hardware: bool,
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
    /// What the picture says about its colours (range and matrix), which the scaler must follow.
    range: ffmpeg::color::Range,
    space: ffmpeg::color::Space,
}

impl VideoDecoder {
    /// Opens `path`, decoding on the graphics card when the preference says so and the machine can.
    pub fn open(path: &Path) -> Result<Self, String> {
        let mode = if crate::info::hardware_decoding() {
            HardwareMode::Heavy
        } else {
            HardwareMode::Off
        };
        Self::open_with(path, mode)
    }

    /// Whether frames of this video are being decoded by the graphics card.
    pub fn is_hardware(&self) -> bool {
        self.used_hardware
    }

    pub fn open_with(path: &Path, mode: HardwareMode) -> Result<Self, String> {
        let input = open_input(path)?;
        let stream = input
            .streams()
            .best(Type::Video)
            .ok_or_else(|| format!("{} has no video stream", path.display()))?;
        let stream_index = stream.index();
        let tb = stream.time_base();
        let quarter_turns = stream_quarter_turns(&stream);
        let mut context = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
            .map_err(|e| format!("codec parameters: {e}"))?;
        let heavy = matches!(
            stream.parameters().id(),
            ffmpeg::codec::Id::HEVC | ffmpeg::codec::Id::AV1 | ffmpeg::codec::Id::VP9
        );
        let wanted = match mode {
            HardwareMode::Off => false,
            HardwareMode::Heavy => heavy,
            HardwareMode::Always => true,
        };
        let hardware = wanted && enable_hardware(&mut context);
        let decoder = context
            .decoder()
            .video()
            .map_err(|e| format!("video decoder: {e}"))?;
        Ok(Self {
            quarter_turns,
            path: path.to_path_buf(),
            hardware,
            used_hardware: false,
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
        match self.frame_at_inner(seconds, max_width) {
            // The card or the driver gave up on this video: the processor takes over, once and for good
            Err(error) if self.hardware => {
                tracing::warn!(
                    "[WARN] decoding on the graphics card failed ({error}); using the processor"
                );
                *self = Self::open_with(&self.path.clone(), HardwareMode::Off)?;
                self.frame_at_inner(seconds, max_width)
            }
            other => other,
        }
    }

    fn frame_at_inner(&mut self, seconds: f64, max_width: u32) -> Result<FrameBuffer, String> {
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
                    let (frame, from_card) = into_memory(&decoded)?;
                    self.used_hardware |= from_card;
                    let pts = frame.timestamp().or(frame.pts()).unwrap_or(0);
                    if consider(&mut best, &mut self.pending, pts, frame, target + eps) {
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
                let (frame, from_card) = into_memory(&decoded)?;
                self.used_hardware |= from_card;
                let pts = frame.timestamp().or(frame.pts()).unwrap_or(0);
                if consider(&mut best, &mut self.pending, pts, frame, target + eps) {
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
        // `max_width` and the result are measured on the picture as shown, after the rotation
        let sideways = self.quarter_turns % 2 == 1;
        let shown = if sideways {
            (source.1, source.0)
        } else {
            source
        };
        let shown = fit_size(shown.0, shown.1, max_width);
        let target = if sideways { (shown.1, shown.0) } else { shown };
        let format = frame.format();

        // The picture's own marks, or the file's when the picture says nothing
        let range = match frame.color_range() {
            ffmpeg::color::Range::Unspecified => self.decoder.color_range(),
            marked => marked,
        };
        let space = match frame.color_space() {
            ffmpeg::color::Space::Unspecified => self.decoder.color_space(),
            marked => marked,
        };
        let rebuild = self.scaler.as_ref().is_none_or(|s| {
            s.format != format
                || s.source != source
                || s.target != target
                || s.range != range
                || s.space != space
        });
        if rebuild {
            let mut scaler = Scaler::get(
                format,
                source.0,
                source.1,
                Pixel::RGBA,
                target.0,
                target.1,
                Flags::BILINEAR,
            )
            .map_err(|e| format!("scaler: {e}"))?;
            follow_colours(&mut scaler, range, space);
            self.scaler = Some(ScalerState {
                scaler,
                format,
                source,
                target,
                range,
                space,
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
        let (rgba, w, h) = rotate_rgba(out, w, h, self.quarter_turns);
        Ok(FrameBuffer {
            width: w as u32,
            height: h as u32,
            rgba,
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

    /// `clip` with a display rotation of `degrees` written in its stream (no re-encoding), the way a
    /// phone held upright stores a portrait video.
    fn rotated_copy(clip: &std::path::Path, degrees: i32) -> Option<std::path::PathBuf> {
        let out = clip.with_file_name(format!(
            "rot{degrees}_{}",
            clip.file_name()?.to_string_lossy()
        ));
        let ok = Command::new("ffmpeg")
            .args(["-y", "-v", "error", "-display_rotation"])
            .arg(degrees.to_string())
            .arg("-i")
            .arg(clip)
            .args(["-c", "copy"])
            .arg(&out)
            .status()
            .ok()?
            .success();
        ok.then_some(out)
    }

    #[test]
    fn rotating_a_picture_turns_it_clockwise_and_four_turns_are_the_identity() {
        // 3 x 2: row 0 = A B C, row 1 = D E F
        let px = |c: u8| [c, 0, 0, 255];
        let src: Vec<u8> = b"ABCDEF".iter().flat_map(|c| px(*c)).collect();
        let letters = |v: &[u8]| v.chunks(4).map(|p| p[0] as char).collect::<String>();
        let (a, w, h) = rotate_rgba(src.clone(), 3, 2, 1);
        assert_eq!((w, h, letters(&a).as_str()), (2, 3, "DAEBFC"));
        let (b, w, h) = rotate_rgba(src.clone(), 3, 2, 2);
        assert_eq!((w, h, letters(&b).as_str()), (3, 2, "FEDCBA"));
        let (c, w, h) = rotate_rgba(src.clone(), 3, 2, 3);
        assert_eq!((w, h, letters(&c).as_str()), (2, 3, "CFBEAD"));
        let mut turned = src.clone();
        let (mut tw, mut th) = (3, 2);
        for _ in 0..4 {
            (turned, tw, th) = rotate_rgba(turned, tw, th, 1);
        }
        assert_eq!((tw, th, turned), (3, 2, src));
    }

    /// The rotation stored in a file turns the pictures the way FFmpeg's own command line shows them.
    #[test]
    fn a_stored_display_rotation_is_applied_to_the_size_and_to_the_picture() {
        let Some(clip) = make_clip("rotate") else {
            return;
        };
        for degrees in [90, 180, 270] {
            let Some(rotated) = rotated_copy(&clip, degrees) else {
                return;
            };
            let info = probe(&rotated).unwrap();
            let expect = if degrees == 180 {
                (320, 240)
            } else {
                (240, 320)
            };
            assert_eq!((info.width, info.height), expect, "{degrees}");
            let ours = VideoDecoder::open(&rotated)
                .unwrap()
                .frame_at(0.4, 0)
                .unwrap();
            assert_eq!((ours.width, ours.height), expect, "{degrees}");
            // What the command line shows (it turns the picture by itself)
            let png = rotated.with_extension("png");
            let shown = Command::new("ffmpeg")
                .args(["-y", "-v", "error", "-ss", "0.4", "-i"])
                .arg(&rotated)
                .args(["-frames:v", "1", "-pix_fmt", "rgba", "-f", "rawvideo"])
                .arg(&png)
                .status()
                .unwrap()
                .success();
            assert!(shown);
            let reference = std::fs::read(&png).unwrap();
            assert_eq!(reference.len(), ours.rgba.len(), "{degrees}");
            let total: u64 = reference
                .chunks(4)
                .zip(ours.rgba.chunks(4))
                .map(|(a, b)| {
                    u64::from(a[0].abs_diff(b[0]))
                        + u64::from(a[1].abs_diff(b[1]))
                        + u64::from(a[2].abs_diff(b[2]))
                })
                .sum();
            let mean = total as f64 / (reference.len() / 4) as f64 / 3.0;
            assert!(
                mean < 6.0,
                "{degrees}: the picture is not the one FFmpeg shows (mean error {mean})"
            );
            let _ = std::fs::remove_file(png);
            let _ = std::fs::remove_file(rotated);
        }
        let _ = std::fs::remove_file(clip);
    }

    /// An H.264 clip (the codec every graphics card decodes) made with the `ffmpeg` command line.
    fn make_h264(tag: &str, size: &str, seconds: u32) -> Option<std::path::PathBuf> {
        let path = std::env::temp_dir().join(format!("neoveg_{tag}_{}.mp4", std::process::id()));
        let ok = Command::new("ffmpeg")
            .args(["-y", "-v", "error", "-f", "lavfi", "-i"])
            .arg(format!("testsrc2=duration={seconds}:size={size}:rate=30"))
            .args([
                "-c:v",
                "libopenh264",
                "-b:v",
                "8M",
                "-g",
                "30",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&path)
            .status()
            .ok()?
            .success();
        ok.then_some(path)
    }

    fn mean_difference(a: &FrameBuffer, b: &FrameBuffer) -> f64 {
        assert_eq!((a.width, a.height), (b.width, b.height));
        let total: u64 = a
            .rgba
            .iter()
            .zip(&b.rgba)
            .map(|(x, y)| u64::from(x.abs_diff(*y)))
            .sum();
        total as f64 / a.rgba.len() as f64
    }

    /// The graphics card and the processor show the same pictures (the card may round the colours a
    /// little differently). Where the machine has no usable card the test only checks that the
    /// request still gives frames.
    #[test]
    fn the_graphics_card_decodes_the_same_pictures_as_the_processor() {
        let Some(path) = make_h264("gpu_same", "1280x720", 2) else {
            eprintln!("skipped: the ffmpeg command line cannot write H.264 here");
            return;
        };
        let mut card = VideoDecoder::open_with(&path, HardwareMode::Always).unwrap();
        let mut processor = VideoDecoder::open_with(&path, HardwareMode::Off).unwrap();
        for t in [0.0, 0.5, 1.2, 1.9, 0.3] {
            let a = card.frame_at(t, 0).unwrap();
            let b = processor.frame_at(t, 0).unwrap();
            let error = mean_difference(&a, &b);
            assert!(error < 3.0, "t={t}: the card's picture differs by {error}");
        }
        if hardware_available() {
            assert!(
                card.is_hardware(),
                "a card is there, but the frames did not come from it"
            );
        }
        assert!(!processor.is_hardware());
        println!(
            "graphics card available: {}, used: {}",
            hardware_available(),
            card.is_hardware()
        );
        let _ = std::fs::remove_file(path);
    }

    /// The normal mode: the heavy codecs go to the card, H.264 stays on the processor.
    #[test]
    fn only_the_heavy_codecs_use_the_card_in_the_normal_mode() {
        let Some(h264) = make_h264("gpu_policy_h264", "1280x720", 1) else {
            return;
        };
        let mut dec = VideoDecoder::open(&h264).unwrap();
        dec.frame_at(0.3, 0).unwrap();
        assert!(!dec.is_hardware(), "H.264 should stay on the processor");
        let _ = std::fs::remove_file(h264);
        if !hardware_available() {
            return;
        }
        let Some(hevc) = make_hevc("gpu_policy_hevc", "1280x720", 1) else {
            return;
        };
        let mut dec = VideoDecoder::open(&hevc).unwrap();
        let on_card = dec.frame_at(0.3, 0).unwrap();
        assert!(dec.is_hardware(), "HEVC should be decoded by the card");
        let reference = VideoDecoder::open_with(&hevc, HardwareMode::Off)
            .unwrap()
            .frame_at(0.3, 0)
            .unwrap();
        assert!(mean_difference(&on_card, &reference) < 3.0);
        // The switch of the preferences
        crate::info::set_hardware_decoding(false);
        let mut off = VideoDecoder::open(&hevc).unwrap();
        off.frame_at(0.3, 0).unwrap();
        crate::info::set_hardware_decoding(true);
        assert!(!off.is_hardware(), "the preference turned the card off");
        let _ = std::fs::remove_file(hevc);
    }

    /// A codec the card does not decode (MPEG-4 part 2 here) still plays: the processor takes it.
    #[test]
    fn a_video_the_card_cannot_decode_falls_back_to_the_processor() {
        let Some(path) = make_clip("gpu_fallback") else {
            return;
        };
        let mut dec = VideoDecoder::open_with(&path, HardwareMode::Always).unwrap();
        let frame = dec.frame_at(0.8, 0).unwrap();
        assert_eq!((frame.width, frame.height), (320, 240));
        assert!(!dec.is_hardware());
        let _ = std::fs::remove_file(path);
    }

    /// An HEVC clip made by the graphics card's own encoder, when there is one.
    fn make_hevc(tag: &str, size: &str, seconds: u32) -> Option<std::path::PathBuf> {
        let path = std::env::temp_dir().join(format!("neoveg_{tag}_{}.mp4", std::process::id()));
        let ok = Command::new("ffmpeg")
            .args(["-y", "-v", "error", "-f", "lavfi", "-i"])
            .arg(format!("testsrc2=duration={seconds}:size={size}:rate=30"))
            .args([
                "-c:v", "hevc_amf", "-b:v", "20M", "-g", "30", "-pix_fmt", "nv12",
            ])
            .arg(&path)
            .status()
            .ok()?
            .success();
        ok.then_some(path)
    }

    /// Speed of a sequential decode and of jumping about (scrubbing), on the card and on the
    /// processor (`--ignored`: it prints the numbers).
    #[test]
    #[ignore = "benchmark: prints the decoding speed"]
    fn benchmark_card_against_processor() {
        let clips: Vec<(String, std::path::PathBuf)> = [
            ("H.264 1080p", make_h264("gpu_bench_a", "1920x1080", 6)),
            ("H.264 4K", make_h264("gpu_bench_b", "3840x2160", 6)),
            ("HEVC 1080p", make_hevc("gpu_bench_c", "1920x1080", 6)),
            ("HEVC 4K", make_hevc("gpu_bench_d", "3840x2160", 6)),
        ]
        .into_iter()
        .filter_map(|(label, path)| path.map(|p| (label.to_string(), p)))
        .collect();
        for (label, path) in &clips {
            for hardware in [false, true] {
                let mode = if hardware {
                    HardwareMode::Always
                } else {
                    HardwareMode::Off
                };
                let mut dec = VideoDecoder::open_with(path, mode).unwrap();
                let start = std::time::Instant::now();
                for k in 0..150 {
                    dec.frame_at(f64::from(k) / 30.0, 640).unwrap();
                }
                let sequential = 150.0 / start.elapsed().as_secs_f64();
                let start = std::time::Instant::now();
                // 20 jumps to unrelated places, as a scrub does
                for k in 0..20 {
                    dec.frame_at(f64::from((k * 7) % 19) * 0.3 + 0.05, 640)
                        .unwrap();
                }
                let jump_ms = start.elapsed().as_secs_f64() * 1000.0 / 20.0;
                println!(
                    "{label:12} {:11} sequencial {sequential:5.0} quadros/s   salto {jump_ms:5.1} ms   (placa usada: {})",
                    if hardware { "placa" } else { "processador" },
                    dec.is_hardware()
                );
            }
        }
        for (_, path) in clips {
            let _ = std::fs::remove_file(path);
        }
    }

    /// The movies of `test_media/` (see `scripts/fetch_test_media.py`) show the picture the `ffmpeg`
    /// command line shows, at several moments reached by jumping about. Skipped without them.
    #[test]
    fn real_movies_show_what_the_command_line_shows() {
        let folder = std::env::var_os("NEOVEG_TEST_MEDIA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test_media"));
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&folder)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| crate::info::is_video_path(p))
            .collect();
        files.sort();
        if files.is_empty() || Command::new("ffmpeg").arg("-version").output().is_err() {
            eprintln!("skipped: no movies in {}", folder.display());
            return;
        }
        let mut wrong = Vec::new();
        for file in &files {
            let name = file.file_name().unwrap().to_string_lossy().into_owned();
            let info = probe(file).unwrap();
            let mut decoder = VideoDecoder::open_with(file, HardwareMode::Off).unwrap();
            for fraction in [0.25, 0.6, 0.1, 0.9, 0.33, 0.77, 0.5] {
                let at = info.duration * fraction;
                let ours = decoder.frame_at(at, 160).unwrap();
                let raw =
                    std::env::temp_dir().join(format!("neoveg_real_{}.rgba", std::process::id()));
                let ok = Command::new("ffmpeg")
                    .args(["-y", "-v", "error", "-ss"])
                    .arg(format!("{at:.3}"))
                    .arg("-i")
                    .arg(file)
                    .args(["-frames:v", "1", "-vf"])
                    .arg(format!(
                        "scale={}:{}:flags=bilinear",
                        ours.width, ours.height
                    ))
                    .args(["-pix_fmt", "rgba", "-f", "rawvideo"])
                    .arg(&raw)
                    .status()
                    .unwrap()
                    .success();
                assert!(
                    ok,
                    "{name}: the command line could not read the frame at {at:.1} s"
                );
                let reference = std::fs::read(&raw).unwrap();
                let _ = std::fs::remove_file(&raw);
                assert_eq!(reference.len(), ours.rgba.len(), "{name}");
                let total: u64 = reference
                    .iter()
                    .zip(&ours.rgba)
                    .map(|(a, b)| u64::from(a.abs_diff(*b)))
                    .sum();
                let mean = total as f64 / reference.len() as f64;
                println!("{name:32} {at:7.1} s  mean difference {mean:5.1}");
                if mean > 12.0 {
                    wrong.push(format!("{name} at {at:.1} s ({mean:.1})"));
                }
            }
        }
        assert!(
            wrong.is_empty(),
            "frames that differ from the command line: {wrong:?}"
        );
    }

    /// A one-colour clip whose colour is written the way a camera or an editor would, with the range
    /// and the matrix stored in the file; `None` when the command line cannot make it.
    fn tagged_colour_clip(
        tag: &str,
        colour: &str,
        filter: &str,
        tags: &[&str],
    ) -> Option<std::path::PathBuf> {
        let path = std::env::temp_dir().join(format!("neoveg_{tag}_{}.mp4", std::process::id()));
        let ok = Command::new("ffmpeg")
            .args(["-y", "-v", "error", "-f", "lavfi", "-i"])
            .arg(format!("color=c={colour}:s=64x64:r=10:d=1"))
            .args([
                "-vf",
                filter,
                "-c:v",
                "mpeg4",
                "-qscale:v",
                "1",
                "-pix_fmt",
                "yuv420p",
            ])
            .args(tags)
            // The container keeps the marks only when asked to
            .args(["-movflags", "+write_colr"])
            .arg(&path)
            .status()
            .ok()?
            .success();
        ok.then_some(path)
    }

    fn centre_pixel(path: &std::path::Path) -> (u8, u8, u8) {
        let frame = VideoDecoder::open_with(path, HardwareMode::Off)
            .unwrap()
            .frame_at(0.3, 0)
            .unwrap();
        let i = ((frame.height / 2 * frame.width + frame.width / 2) * 4) as usize;
        (frame.rgba[i], frame.rgba[i + 1], frame.rgba[i + 2])
    }

    /// A video marked as full range keeps its dark tones (limited-range reading turned everything
    /// under 16 into black).
    #[test]
    fn a_full_range_video_keeps_its_dark_tones() {
        let Some(path) = tagged_colour_clip(
            "full_range",
            "0x0c0c0c",
            "scale=out_range=full,format=yuv420p",
            &["-color_range", "pc"],
        ) else {
            return;
        };
        let (r, g, b) = centre_pixel(&path);
        assert!(
            r >= 8 && g >= 8 && b >= 8,
            "dark grey read as ({r}, {g}, {b})"
        );
        let _ = std::fs::remove_file(path);
    }

    /// A video marked BT.709 is read with that matrix: pure red comes back as red, not as an orange
    /// the BT.601 matrix would make of it.
    #[test]
    fn a_bt709_video_gets_its_colours_back() {
        let Some(path) = tagged_colour_clip(
            "bt709",
            "0xe01010",
            "scale=out_color_matrix=bt709:out_range=tv,format=yuv420p",
            &[
                "-colorspace",
                "bt709",
                "-color_primaries",
                "bt709",
                "-color_trc",
                "bt709",
            ],
        ) else {
            return;
        };
        let (r, g, b) = centre_pixel(&path);
        let near = |got: u8, want: u8| (i32::from(got) - i32::from(want)).abs() < 10;
        assert!(
            near(r, 0xe0) && near(g, 0x10) && near(b, 0x10),
            "({r}, {g}, {b})"
        );
        let _ = std::fs::remove_file(path);
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
