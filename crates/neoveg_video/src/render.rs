//! Video & audio export (*Make Movie*): streams composited RGBA8 frames into `ffmpeg` along with
//! an offline master WAV mixdown, reporting live frame progress and supporting cancellation.

use neoveg_common::i18n::tr;
use std::collections::HashSet;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Once, OnceLock};

use serde::{Deserialize, Serialize};

use crate::frame::FrameBuffer;

/// Output container and codec templates for *Make Movie*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderFormat {
    #[default]
    Mp4H264,
    Mp4Hevc,
    MovProRes,
    WebmVp9,
    WavPcm16,
}

impl RenderFormat {
    pub const ALL: [Self; 5] = [
        Self::Mp4H264,
        Self::Mp4Hevc,
        Self::MovProRes,
        Self::WebmVp9,
        Self::WavPcm16,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Mp4H264 => "MP4 — H.264 (AVC) + AAC",
            Self::Mp4Hevc => "MP4 — HEVC (H.265) + AAC",
            Self::MovProRes => "MOV — Apple ProRes 422 HQ + PCM",
            Self::WebmVp9 => "WebM — VP9 + Opus",
            Self::WavPcm16 => tr("WAV — Somente Áudio PCM 16-bit"),
        }
    }

    pub fn default_extension(self) -> &'static str {
        match self {
            Self::Mp4H264 | Self::Mp4Hevc => "mp4",
            Self::MovProRes => "mov",
            Self::WebmVp9 => "webm",
            Self::WavPcm16 => "wav",
        }
    }

    pub fn has_video(self) -> bool {
        !matches!(self, Self::WavPcm16)
    }
}

/// Video encoders to try for `format`, best first. Software builds ship `libx264` / `libx265`; an
/// LGPL build (the one `winget` installs) has `libopenh264` and the Media Foundation encoders instead.
/// Hardware encoders (NVENC, AMF, QuickSync) are left out: `ffmpeg` lists them even on a machine
/// without the card, and they then fail when the render starts.
pub fn encoder_candidates(format: RenderFormat) -> &'static [&'static str] {
    match format {
        RenderFormat::Mp4H264 => &["libx264", "libopenh264", "h264_mf"],
        RenderFormat::Mp4Hevc => &["libx265", "hevc_mf"],
        RenderFormat::MovProRes => &["prores_ks", "prores_aw"],
        RenderFormat::WebmVp9 => &["libvpx-vp9"],
        RenderFormat::WavPcm16 => &[],
    }
}

/// The first of [`encoder_candidates`] that is in `available`.
pub fn pick_encoder(format: RenderFormat, available: &HashSet<String>) -> Option<&'static str> {
    encoder_candidates(format)
        .iter()
        .copied()
        .find(|name| available.contains(*name))
}

/// Encoder names in the output of `ffmpeg -encoders` (video, audio and subtitle rows alike).
pub fn parse_encoder_list(listing: &str) -> HashSet<String> {
    listing
        .lines()
        .filter_map(|line| {
            let mut columns = line.split_whitespace();
            let flags = columns.next()?;
            // rows look like ` V....D libx264 ...`; the header and legend rows do not match
            let is_row = flags.len() == 6
                && flags.starts_with(['V', 'A', 'S'])
                && flags[1..]
                    .chars()
                    .all(|c| c == '.' || c.is_ascii_uppercase());
            if is_row {
                // the legend (` V..... = Video`) has the same shape as a row
                columns
                    .next()
                    .filter(|name| *name != "=")
                    .map(str::to_string)
            } else {
                None
            }
        })
        .collect()
}

static ENCODERS: OnceLock<Option<HashSet<String>>> = OnceLock::new();

/// The encoders of the `ffmpeg` on the `PATH`, asked once (this starts `ffmpeg`, so not on the UI
/// thread: see [`probe_encoders_in_background`]); `None` when it cannot be started.
pub fn available_encoders() -> Option<&'static HashSet<String>> {
    ENCODERS
        .get_or_init(|| {
            let output = Command::new("ffmpeg")
                .args(["-hide_banner", "-encoders"])
                .stdin(Stdio::null())
                .output()
                .ok()?;
            output
                .status
                .success()
                .then(|| parse_encoder_list(&String::from_utf8_lossy(&output.stdout)))
        })
        .as_ref()
}

/// Asks `ffmpeg` for its encoders on a worker thread, so [`cached_encoder_for`] has an answer later.
pub fn probe_encoders_in_background() {
    // the dialog calls this every frame: start the probe once, not once per frame while it runs
    static STARTED: Once = Once::new();
    STARTED.call_once(|| {
        std::thread::spawn(|| {
            available_encoders();
        });
    });
}

/// The encoder for `format` if the probe has finished: `None` while it is running, `Some(Err)`
/// when `ffmpeg` is missing or lacks every candidate. Never blocks.
pub fn cached_encoder_for(format: RenderFormat) -> Option<Result<&'static str, String>> {
    ENCODERS.get().map(|_| encoder_for(format))
}

/// The encoder this machine's `ffmpeg` will use for `format`, or why there is none.
pub fn encoder_for(format: RenderFormat) -> Result<&'static str, String> {
    let available = available_encoders().ok_or_else(|| {
        "não foi possível iniciar o `ffmpeg` (verifique se está instalado no PATH)".to_string()
    })?;
    pick_encoder(format, available).ok_or_else(|| {
        format!(
            "o `ffmpeg` instalado não tem codificador para {} (procurados: {})",
            format.label(),
            encoder_candidates(format).join(", ")
        )
    })
}

/// A target bitrate for encoders without a constant-quality mode, from the CRF slider: about
/// 0.1 bit per pixel per frame at CRF 23, doubling for every 6 CRF steps lower (the x264 scale).
fn target_bitrate(width: u32, height: u32, fps: f64, crf: u8) -> String {
    let base = f64::from(width) * f64::from(height) * fps * 0.1;
    let scale = 2f64.powf((23.0 - f64::from(crf.clamp(0, 51))) / 6.0);
    format!("{}", (base * scale).clamp(200_000.0, 400_000_000.0) as u64)
}

/// Builds the exact `ffmpeg` CLI argument vector for streaming `width x height` RGBA8 raw video
/// from `stdin` (`pipe:0`) plus an optional pre-rendered stereo `audio_wav` file into `output`.
#[allow(clippy::too_many_arguments)] // one flat description of the encode, as `ffmpeg` takes it
pub fn build_ffmpeg_encode_args(
    format: RenderFormat,
    width: u32,
    height: u32,
    fps: f64,
    crf: u8,
    encoder: &str,
    audio_wav: Option<&Path>,
    output: &Path,
) -> Vec<String> {
    let w = width.max(2) & !1;
    let h = height.max(2) & !1;
    let rate = if fps.is_finite() && fps > 0.0 {
        fps
    } else {
        29.97
    };

    let mut args = vec![
        "-y".into(),
        "-v".into(),
        "error".into(),
        "-f".into(),
        "rawvideo".into(),
        "-pix_fmt".into(),
        "rgba".into(),
        "-s".into(),
        format!("{w}x{h}"),
        "-r".into(),
        format!("{rate:.6}"),
        "-i".into(),
        "pipe:0".into(),
    ];

    if let Some(wav) = audio_wav {
        args.push("-i".into());
        args.push(wav.to_string_lossy().into_owned());
    }

    let q = crf.clamp(0, 51).to_string();
    let bitrate = target_bitrate(w, h, rate, crf);
    match format {
        RenderFormat::Mp4H264 | RenderFormat::Mp4Hevc => {
            args.extend(["-c:v".into(), encoder.into()]);
            match encoder {
                // x264 / x265 take a constant rate factor
                "libx264" | "libx265" => {
                    args.extend(["-preset".into(), "medium".into(), "-crf".into(), q])
                }
                // OpenH264 and the Media Foundation encoders only take a target bitrate
                _ => args.extend(["-b:v".into(), bitrate]),
            }
            args.extend(["-pix_fmt".into(), "yuv420p".into()]);
            if format == RenderFormat::Mp4Hevc {
                args.extend(["-tag:v".into(), "hvc1".into()]);
            } else {
                args.extend(["-movflags".into(), "+faststart".into()]);
            }
            if audio_wav.is_some() {
                args.extend(["-c:a".into(), "aac".into(), "-b:a".into(), "192k".into()]);
            }
        }
        RenderFormat::MovProRes => {
            args.extend([
                "-c:v".into(),
                encoder.into(),
                "-profile:v".into(),
                "3".into(),
                "-pix_fmt".into(),
                "yuv422p10le".into(),
            ]);
            if audio_wav.is_some() {
                args.extend(["-c:a".into(), "pcm_s16le".into()]);
            }
        }
        RenderFormat::WebmVp9 => {
            args.extend([
                "-c:v".into(),
                encoder.into(),
                "-crf".into(),
                q,
                "-b:v".into(),
                "0".into(),
                "-pix_fmt".into(),
                "yuv420p".into(),
            ]);
            if audio_wav.is_some() {
                args.extend([
                    "-c:a".into(),
                    "libopus".into(),
                    "-b:a".into(),
                    "160k".into(),
                ]);
            }
        }
        RenderFormat::WavPcm16 => {}
    }

    if audio_wav.is_some() {
        args.push("-shortest".into());
    }
    args.push(output.to_string_lossy().into_owned());
    args
}

/// Which encoder writes the movie: the built-in one (H.264 + AAC in an MP4 file, no other program
/// needed) or `ffmpeg`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderEngine {
    /// The built-in encoder where it can write the format, `ffmpeg` for the others.
    #[default]
    Auto,
    /// The built-in encoder (`render_native`).
    Internal,
    /// An `ffmpeg` found on the `PATH`.
    Ffmpeg,
}

impl RenderEngine {
    pub const ALL: [Self; 3] = [Self::Auto, Self::Internal, Self::Ffmpeg];

    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => tr("Automático (interno para MP4 H.264, FFmpeg para os outros)"),
            Self::Internal => tr("Interno (Rust, sem FFmpeg)"),
            Self::Ffmpeg => "FFmpeg",
        }
    }

    /// The engine a render of `format` really uses: never [`RenderEngine::Auto`].
    pub fn resolve(self, format: RenderFormat) -> Self {
        match self {
            Self::Auto if crate::render_native::supports(format) => Self::Internal,
            Self::Auto => Self::Ffmpeg,
            other => other,
        }
    }

    /// `true` when this engine can write `format` (the internal one writes only MP4 H.264).
    pub fn can_write(self, format: RenderFormat) -> bool {
        !matches!(self.resolve(format), Self::Internal) || crate::render_native::supports(format)
    }
}

/// Encodes the movie with the chosen engine; the arguments are those of
/// [`encode_video_with_ffmpeg`].
pub fn encode_video(
    engine: RenderEngine,
    spec: &VideoEncodeSpec<'_>,
    cancel: &AtomicBool,
    on_progress: impl FnMut(u64, u64),
    produce_frame: impl FnMut(u64) -> Result<FrameBuffer, String>,
) -> Result<(), String> {
    match engine.resolve(spec.format) {
        RenderEngine::Internal => {
            crate::render_native::encode_video_native(spec, cancel, on_progress, produce_frame)
        }
        _ => encode_video_with_ffmpeg(spec, cancel, on_progress, produce_frame),
    }
}

/// Encoding parameters passed to [`encode_video_with_ffmpeg`].
pub struct VideoEncodeSpec<'a> {
    pub format: RenderFormat,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub crf: u8,
    pub total_frames: u64,
    pub audio_wav: Option<&'a Path>,
    pub output: &'a Path,
}

/// Streams `spec.total_frames` generated by `produce_frame(frame_idx)` into an `ffmpeg` subprocess.
pub fn encode_video_with_ffmpeg(
    spec: &VideoEncodeSpec<'_>,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(u64, u64),
    mut produce_frame: impl FnMut(u64) -> Result<FrameBuffer, String>,
) -> Result<(), String> {
    if let Some(parent) = spec.output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("falha ao criar pasta {}: {e}", parent.display()))?;
        }
    }
    let w = spec.width.max(2) & !1;
    let h = spec.height.max(2) & !1;
    let encoder = if spec.format.has_video() {
        encoder_for(spec.format)?
    } else {
        ""
    };
    let args = build_ffmpeg_encode_args(
        spec.format,
        w,
        h,
        spec.fps,
        spec.crf,
        encoder,
        spec.audio_wav,
        spec.output,
    );

    let mut child = Command::new("ffmpeg")
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            format!(
                "não foi possível iniciar o `ffmpeg` (verifique se está instalado no PATH): {e}"
            )
        })?;

    let expected_bytes = (w as usize) * (h as usize) * 4;
    let mut write_err: Option<String> = None;
    {
        let Some(mut stdin) = child.stdin.take() else {
            return Err("stdin do ffmpeg indisponível".into());
        };
        for idx in 0..spec.total_frames {
            if cancel.load(Ordering::Relaxed) {
                let _ = child.kill();
                let _ = child.wait();
                let _ = std::fs::remove_file(spec.output);
                return Err("renderização cancelada pelo usuário".into());
            }
            let frame = match produce_frame(idx) {
                Ok(f) => f,
                Err(e) => {
                    write_err = Some(e);
                    break;
                }
            };
            let fitted = crate::composite::fit_layer_to_canvas(&frame, w, h);
            if fitted.rgba.len() != expected_bytes {
                write_err = Some("tamanho de quadro inválido durante a renderização".into());
                break;
            }
            if let Err(e) = stdin.write_all(&fitted.rgba) {
                write_err = Some(format!("falha ao enviar quadro ao ffmpeg: {e}"));
                break;
            }
            on_progress(idx + 1, spec.total_frames);
        }
    }

    let status_out = child
        .wait_with_output()
        .map_err(|e| format!("falha ao aguardar término do ffmpeg: {e}"))?;
    if let Some(err) = write_err {
        let stderr = String::from_utf8_lossy(&status_out.stderr);
        if stderr.trim().is_empty() {
            return Err(err);
        }
        return Err(format!("{err} ({})", stderr.trim()));
    }
    if !status_out.status.success() {
        let stderr = String::from_utf8_lossy(&status_out.stderr);
        return Err(format!(
            "ffmpeg encerrou com erro: {}",
            stderr.lines().next().unwrap_or("código diferente de zero")
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_engine_choice_follows_the_format_and_refuses_what_it_cannot_write() {
        assert_eq!(
            RenderEngine::Auto.resolve(RenderFormat::Mp4H264),
            RenderEngine::Internal
        );
        assert_eq!(
            RenderEngine::Auto.resolve(RenderFormat::MovProRes),
            RenderEngine::Ffmpeg
        );
        assert_eq!(
            RenderEngine::Ffmpeg.resolve(RenderFormat::Mp4H264),
            RenderEngine::Ffmpeg
        );
        assert!(RenderEngine::Auto.can_write(RenderFormat::WebmVp9));
        assert!(RenderEngine::Ffmpeg.can_write(RenderFormat::WebmVp9));
        assert!(RenderEngine::Internal.can_write(RenderFormat::Mp4H264));
        assert!(!RenderEngine::Internal.can_write(RenderFormat::Mp4Hevc));
        assert!(RenderEngine::ALL.iter().all(|e| !e.label().is_empty()));
    }

    #[test]
    fn ffmpeg_args_enforce_even_dimensions_and_expected_codecs() {
        let h264 = build_ffmpeg_encode_args(
            RenderFormat::Mp4H264,
            1921,
            1081,
            29.97,
            20,
            "libx264",
            Some(Path::new("mix.wav")),
            Path::new("out.mp4"),
        );
        assert!(h264.contains(&"1920x1080".to_string()));
        assert!(h264.contains(&"libx264".to_string()));
        assert!(h264.contains(&"aac".to_string()));
        assert!(h264.contains(&"-shortest".to_string()));

        let prores = build_ffmpeg_encode_args(
            RenderFormat::MovProRes,
            1280,
            720,
            24.0,
            18,
            "prores_ks",
            None,
            Path::new("master.mov"),
        );
        assert!(prores.contains(&"prores_ks".to_string()));
        assert!(!prores.contains(&"-shortest".to_string()));
    }

    #[test]
    fn the_encoder_follows_what_ffmpeg_has() {
        let gpl: HashSet<String> = ["libx264", "libx265", "prores_ks", "libvpx-vp9"]
            .map(String::from)
            .into();
        let lgpl: HashSet<String> = ["libopenh264", "h264_mf", "hevc_mf", "h264_nvenc"]
            .map(String::from)
            .into();
        assert_eq!(pick_encoder(RenderFormat::Mp4H264, &gpl), Some("libx264"));
        assert_eq!(
            pick_encoder(RenderFormat::Mp4H264, &lgpl),
            Some("libopenh264")
        );
        assert_eq!(pick_encoder(RenderFormat::Mp4Hevc, &lgpl), Some("hevc_mf"));
        // no ProRes or VP9 in the LGPL build: nothing is picked and the render says so
        assert_eq!(pick_encoder(RenderFormat::MovProRes, &lgpl), None);
        assert_eq!(pick_encoder(RenderFormat::WebmVp9, &lgpl), None);
        // a hardware encoder alone is never chosen
        let only_hw: HashSet<String> = ["h264_nvenc".to_string()].into();
        assert_eq!(pick_encoder(RenderFormat::Mp4H264, &only_hw), None);
    }

    #[test]
    fn the_encoder_list_is_read_from_the_ffmpeg_listing() {
        let listing = "Encoders:
 V..... = Video
 ------
 V....D libx264              libx264 H.264
 A....D aac                  AAC
 V..... h264_mf             H264 via MediaFoundation
";
        let names = parse_encoder_list(listing);
        assert!(names.contains("libx264") && names.contains("aac") && names.contains("h264_mf"));
        assert!(!names.contains("=") && !names.contains("Encoders:") && !names.contains("------"));
    }

    #[test]
    fn bitrate_encoders_get_a_bitrate_and_x264_a_crf() {
        let args = |encoder: &str| {
            build_ffmpeg_encode_args(
                RenderFormat::Mp4H264,
                1920,
                1080,
                30.0,
                23,
                encoder,
                None,
                Path::new("o.mp4"),
            )
        };
        let x264 = args("libx264");
        assert!(x264.contains(&"-crf".to_string()) && !x264.contains(&"-b:v".to_string()));
        for encoder in ["libopenh264", "h264_mf"] {
            let a = args(encoder);
            assert!(a.contains(&encoder.to_string()));
            assert!(!a.contains(&"-crf".to_string()), "{encoder} has no -crf");
            let i = a.iter().position(|x| x == "-b:v").expect("bitrate");
            // CRF 23 at 1080p30 is about 6 Mbit/s
            let bits: u64 = a[i + 1].parse().unwrap();
            assert!((5_000_000..8_000_000).contains(&bits), "{bits}");
        }
        // a lower CRF asks for more
        assert!(
            target_bitrate(1920, 1080, 30.0, 17).parse::<u64>().unwrap()
                > target_bitrate(1920, 1080, 30.0, 23).parse::<u64>().unwrap()
        );
    }
}
