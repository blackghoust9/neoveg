//! Writing a part of a file with a picture as a file of its own, forwards or backwards (*Subclipes*,
//! *Reverter evento*). The work is done by the `ffmpeg` program on the `PATH` (the same one the
//! conversion uses), on a worker thread chosen by the caller; the pure part is the command line.

use std::path::Path;
use std::process::{Command, Stdio};

use crate::render::{encoder_for, RenderFormat};

/// What goes into the command line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Request<'a> {
    pub input: &'a Path,
    pub output: &'a Path,
    /// Seconds into the input, and how many seconds.
    pub start: f64,
    pub length: f64,
    pub reverse: bool,
    /// The file has sound to carry along.
    pub has_audio: bool,
    /// The H.264 encoder of this machine's `ffmpeg`.
    pub encoder: &'a str,
}

/// The `ffmpeg` arguments for `request`. The part is cut after the input is opened (`-ss` after
/// `-i`), so it starts on the frame asked for and not on the key frame before it.
pub fn ffmpeg_args(request: &Request) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "-hide_banner".into(),
        "-y".into(),
        "-i".into(),
        request.input.to_string_lossy().into_owned(),
        "-ss".into(),
        format!("{:.6}", request.start.max(0.0)),
        "-t".into(),
        format!("{:.6}", request.length.max(0.0)),
        "-c:v".into(),
        request.encoder.into(),
    ];
    if request.encoder == "libx264" {
        args.extend(["-crf", "18", "-preset", "medium"].map(String::from));
    }
    let video = if request.reverse {
        "reverse,scale=trunc(iw/2)*2:trunc(ih/2)*2"
    } else {
        "scale=trunc(iw/2)*2:trunc(ih/2)*2"
    };
    args.extend(["-pix_fmt", "yuv420p", "-vf", video].map(String::from));
    if request.has_audio {
        if request.reverse {
            args.extend(["-af", "areverse"].map(String::from));
        }
        args.extend(["-c:a", "aac", "-b:a", "192k"].map(String::from));
    } else {
        args.push("-an".into());
    }
    args.extend(["-movflags", "+faststart"].map(String::from));
    args.push(request.output.to_string_lossy().into_owned());
    args
}

/// Writes the part with `ffmpeg` and waits for it. On failure the error carries the last lines it
/// printed and no half-written file is left.
pub fn write(
    input: &Path,
    output: &Path,
    start: f64,
    length: f64,
    reverse: bool,
    has_audio: bool,
) -> Result<(), String> {
    if let Some(dir) = output.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("não foi possível criar a pasta {}: {e}", dir.display()))?;
    }
    let encoder = encoder_for(RenderFormat::Mp4H264)?;
    let args = ffmpeg_args(&Request {
        input,
        output,
        start,
        length,
        reverse,
        has_audio,
        encoder,
    });
    let result = Command::new("ffmpeg")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("não foi possível iniciar o `ffmpeg`: {e}"))?;
    if result.status.success() && output.exists() {
        return Ok(());
    }
    let _ = std::fs::remove_file(output);
    let log = String::from_utf8_lossy(&result.stderr);
    let tail: Vec<&str> = log.lines().rev().take(3).collect();
    Err(format!(
        "o `ffmpeg` não conseguiu cortar {}: {}",
        input.display(),
        tail.into_iter().rev().collect::<Vec<_>>().join(" | ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request<'a>(reverse: bool, has_audio: bool, encoder: &'a str) -> Request<'a> {
        Request {
            input: Path::new("in.mp4"),
            output: Path::new("out.mp4"),
            start: 1.5,
            length: 2.25,
            reverse,
            has_audio,
            encoder,
        }
    }

    fn has(args: &[String], pair: [&str; 2]) -> bool {
        args.windows(2).any(|w| w == pair)
    }

    #[test]
    fn the_part_is_cut_after_the_input_and_the_output_comes_last() {
        let args = ffmpeg_args(&request(false, true, "libx264"));
        let input = args.iter().position(|a| a == "-i").unwrap();
        let ss = args.iter().position(|a| a == "-ss").unwrap();
        assert!(ss > input, "-ss after -i cuts on the frame");
        assert!(has(&args, ["-ss", "1.500000"]) && has(&args, ["-t", "2.250000"]));
        assert_eq!(args.last().unwrap(), "out.mp4");
        assert!(has(&args, ["-crf", "18"]));
        assert!(!args.iter().any(|a| a.contains("reverse")), "forwards");
        assert!(has(&args, ["-c:a", "aac"]));
    }

    #[test]
    fn reversing_turns_the_picture_and_the_sound_and_a_silent_file_has_no_sound_options() {
        let args = ffmpeg_args(&request(true, true, "h264_mf"));
        assert!(has(
            &args,
            ["-vf", "reverse,scale=trunc(iw/2)*2:trunc(ih/2)*2"]
        ));
        assert!(has(&args, ["-af", "areverse"]));
        assert!(
            !args.contains(&"-crf".to_string()),
            "x264 options only for x264"
        );
        let silent = ffmpeg_args(&request(true, false, "libx264"));
        assert!(silent.contains(&"-an".to_string()));
        assert!(!silent.contains(&"-af".to_string()) && !silent.contains(&"aac".to_string()));
    }

    #[test]
    fn a_missing_input_is_an_error_and_leaves_no_output() {
        let out = std::env::temp_dir()
            .join("neoveg_subclip_video_test")
            .join("nada.mp4");
        let err = write(
            Path::new("nao_existe_mesmo.mp4"),
            &out,
            0.0,
            1.0,
            false,
            true,
        );
        assert!(err.is_err());
        assert!(!out.exists());
        let _ = std::fs::remove_dir_all(out.parent().unwrap());
    }
}
