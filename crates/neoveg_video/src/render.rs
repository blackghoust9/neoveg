//! Video & audio export (*Make Movie*): streams composited RGBA8 frames into `ffmpeg` along with
//! an offline master WAV mixdown, reporting live frame progress and supporting cancellation.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

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
            Self::WavPcm16 => "WAV — Somente Áudio PCM 16-bit",
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

/// Builds the exact `ffmpeg` CLI argument vector for streaming `width x height` RGBA8 raw video
/// from `stdin` (`pipe:0`) plus an optional pre-rendered stereo `audio_wav` file into `output`.
pub fn build_ffmpeg_encode_args(
    format: RenderFormat,
    width: u32,
    height: u32,
    fps: f64,
    crf: u8,
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
    match format {
        RenderFormat::Mp4H264 => {
            args.extend([
                "-c:v".into(),
                "libx264".into(),
                "-preset".into(),
                "medium".into(),
                "-crf".into(),
                q,
                "-pix_fmt".into(),
                "yuv420p".into(),
                "-movflags".into(),
                "+faststart".into(),
            ]);
            if audio_wav.is_some() {
                args.extend(["-c:a".into(), "aac".into(), "-b:a".into(), "192k".into()]);
            }
        }
        RenderFormat::Mp4Hevc => {
            args.extend([
                "-c:v".into(),
                "libx265".into(),
                "-preset".into(),
                "medium".into(),
                "-crf".into(),
                q,
                "-pix_fmt".into(),
                "yuv420p".into(),
                "-tag:v".into(),
                "hvc1".into(),
            ]);
            if audio_wav.is_some() {
                args.extend(["-c:a".into(), "aac".into(), "-b:a".into(), "192k".into()]);
            }
        }
        RenderFormat::MovProRes => {
            args.extend([
                "-c:v".into(),
                "prores_ks".into(),
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
                "libvpx-vp9".into(),
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
    let args = build_ffmpeg_encode_args(
        spec.format,
        w,
        h,
        spec.fps,
        spec.crf,
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
    fn ffmpeg_args_enforce_even_dimensions_and_expected_codecs() {
        let h264 = build_ffmpeg_encode_args(
            RenderFormat::Mp4H264,
            1921,
            1081,
            29.97,
            20,
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
            None,
            Path::new("master.mov"),
        );
        assert!(prores.contains(&"prores_ks".to_string()));
        assert!(!prores.contains(&"-shortest".to_string()));
    }
}
