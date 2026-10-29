//! Converting media Neoveg cannot read into media it can: a video becomes MP4 (H.264 + AAC), a
//! sound becomes WAV (PCM 16-bit). The work is done by the `ffmpeg` program on the `PATH`, on a
//! worker thread chosen by the caller (this blocks until the file is written).
//!
//! The pure parts (what to convert to, the output name, the command line) are plain functions with
//! tests; only [`convert`] starts a process.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::render::{encoder_for, RenderFormat};

/// What a file is converted into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// MP4 with H.264 video and AAC sound.
    Video,
    /// WAV with 16-bit PCM sound, for files that are only sound.
    Audio,
}

impl Target {
    /// Extension of the converted file.
    pub fn extension(self) -> &'static str {
        match self {
            Target::Video => "mp4",
            Target::Audio => "wav",
        }
    }

    /// Short name for the dialog, in pt-BR.
    pub fn label(self) -> &'static str {
        match self {
            Target::Video => "MP4 (H.264 + AAC)",
            Target::Audio => "WAV (PCM 16 bits)",
        }
    }
}

/// Extensions of files that hold sound only.
const AUDIO_EXTENSIONS: [&str; 14] = [
    "wav", "mp3", "flac", "ogg", "oga", "opus", "m4a", "aac", "wma", "aif", "aiff", "ac3", "amr",
    "mka",
];

/// What `path` should be converted to: sound files to WAV, everything else to MP4.
pub fn target_for(path: &Path) -> Target {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if AUDIO_EXTENSIONS.contains(&ext.as_str()) {
        Target::Audio
    } else {
        Target::Video
    }
}

/// A file name for the converted copy of `input` in `dir` that does not exist yet:
/// `name_convertido.mp4`, then `name_convertido_2.mp4` and so on. `exists` says whether a path is
/// taken (the caller passes `Path::exists`; tests pass a fake).
pub fn output_path(
    input: &Path,
    dir: &Path,
    target: Target,
    exists: impl Fn(&Path) -> bool,
) -> PathBuf {
    let stem: String = input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "midia".into())
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let ext = target.extension();
    let first = dir.join(format!("{stem}_convertido.{ext}"));
    if !exists(&first) {
        return first;
    }
    (2..)
        .map(|n| dir.join(format!("{stem}_convertido_{n}.{ext}")))
        .find(|p| !exists(p))
        .expect("an unbounded range finds a free name")
}

/// The `ffmpeg` arguments that convert `input` to `output`. `video_encoder` is the H.264 encoder
/// of this machine's `ffmpeg` (ignored for sound).
pub fn ffmpeg_args(
    input: &Path,
    output: &Path,
    target: Target,
    video_encoder: &str,
) -> Vec<String> {
    let mut args: Vec<String> = ["-hide_banner", "-y", "-i"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    args.push(input.to_string_lossy().into_owned());
    match target {
        Target::Audio => args.extend(
            ["-vn", "-c:a", "pcm_s16le", "-ar", "48000"]
                .iter()
                .map(|s| s.to_string()),
        ),
        Target::Video => {
            args.extend(["-c:v", video_encoder].iter().map(|s| s.to_string()));
            if video_encoder == "libx264" {
                args.extend(
                    ["-crf", "18", "-preset", "medium"]
                        .iter()
                        .map(|s| s.to_string()),
                );
            }
            // Square pixels and 4:2:0 are what every decoder reads; the sound is AAC when there is any
            args.extend(
                [
                    "-pix_fmt",
                    "yuv420p",
                    "-vf",
                    "scale=trunc(iw/2)*2:trunc(ih/2)*2",
                    "-c:a",
                    "aac",
                    "-b:a",
                    "192k",
                    "-movflags",
                    "+faststart",
                ]
                .iter()
                .map(|s| s.to_string()),
            );
        }
    }
    args.push(output.to_string_lossy().into_owned());
    args
}

/// Converts `input` into `output` with `ffmpeg` and waits for it. On failure the error carries the
/// last lines `ffmpeg` printed.
pub fn convert(input: &Path, output: &Path, target: Target) -> Result<(), String> {
    if let Some(dir) = output.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("não foi possível criar a pasta {}: {e}", dir.display()))?;
    }
    let encoder = match target {
        Target::Video => encoder_for(RenderFormat::Mp4H264)?,
        Target::Audio => "",
    };
    let result = Command::new("ffmpeg")
        .args(ffmpeg_args(input, output, target, encoder))
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("não foi possível iniciar o `ffmpeg`: {e}"))?;
    if result.status.success() && output.exists() {
        return Ok(());
    }
    // A half-written file is worse than none
    let _ = std::fs::remove_file(output);
    let log = String::from_utf8_lossy(&result.stderr);
    let tail: Vec<&str> = log.lines().rev().take(3).collect();
    Err(format!(
        "o `ffmpeg` não conseguiu converter {}: {}",
        input.display(),
        tail.into_iter().rev().collect::<Vec<_>>().join(" | ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sound_files_become_wav_and_everything_else_mp4() {
        assert_eq!(target_for(Path::new("a/voz.WMA")), Target::Audio);
        assert_eq!(target_for(Path::new("musica.opus")), Target::Audio);
        assert_eq!(target_for(Path::new("filme.mkv")), Target::Video);
        assert_eq!(target_for(Path::new("sem_extensao")), Target::Video);
        assert_eq!(Target::Video.extension(), "mp4");
        assert_eq!(Target::Audio.extension(), "wav");
    }

    #[test]
    fn the_output_name_is_safe_and_never_takes_an_existing_file() {
        let dir = Path::new("out");
        let free = output_path(Path::new("meu filme (1).avi"), dir, Target::Video, |_| {
            false
        });
        assert_eq!(free, dir.join("meu_filme__1__convertido.mp4"));
        let taken = [
            dir.join("voz_convertido.wav"),
            dir.join("voz_convertido_2.wav"),
        ];
        let next = output_path(Path::new("voz.wma"), dir, Target::Audio, |p| {
            taken.contains(&p.to_path_buf())
        });
        assert_eq!(next, dir.join("voz_convertido_3.wav"));
    }

    #[test]
    fn the_command_line_reads_the_input_and_writes_the_output_last() {
        let v = ffmpeg_args(
            Path::new("in.avi"),
            Path::new("out.mp4"),
            Target::Video,
            "libx264",
        );
        assert_eq!(v[3], "in.avi");
        assert_eq!(v.last().unwrap(), "out.mp4");
        assert!(v.windows(2).any(|w| w == ["-c:v", "libx264"]));
        assert!(v.windows(2).any(|w| w == ["-crf", "18"]));
        assert!(v.contains(&"yuv420p".to_string()) && v.contains(&"aac".to_string()));
        // Another encoder gets no x264-only options
        let other = ffmpeg_args(
            Path::new("in.avi"),
            Path::new("out.mp4"),
            Target::Video,
            "h264_mf",
        );
        assert!(!other.contains(&"-crf".to_string()));
        let a = ffmpeg_args(Path::new("in.wma"), Path::new("out.wav"), Target::Audio, "");
        assert!(a.contains(&"-vn".to_string()) && a.contains(&"pcm_s16le".to_string()));
        assert!(!a.contains(&"-c:v".to_string()));
    }

    #[test]
    fn a_missing_input_is_an_error_and_leaves_no_output() {
        let dir = std::env::temp_dir().join("neoveg_convert_test");
        let out = dir.join("nada_convertido.wav");
        let err = convert(Path::new("nao_existe_mesmo.wma"), &out, Target::Audio);
        assert!(err.is_err());
        assert!(!out.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_real_file_is_converted_when_ffmpeg_is_there() {
        // Skips without `ffmpeg`: build a tiny Matroska clip, convert it, check the result
        let dir = std::env::temp_dir().join("neoveg_convert_real");
        let _ = std::fs::create_dir_all(&dir);
        let input = dir.join("clip.mkv");
        let made = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc=size=64x48:rate=10:duration=1",
            ])
            .args(["-c:v", "mpeg4"])
            .arg(&input)
            .stdin(Stdio::null())
            .output();
        if !made.is_ok_and(|o| o.status.success()) || encoder_for(RenderFormat::Mp4H264).is_err() {
            let _ = std::fs::remove_dir_all(&dir);
            return;
        }
        let out = output_path(&input, &dir, Target::Video, Path::exists);
        convert(&input, &out, Target::Video).expect("the clip converts");
        assert!(std::fs::metadata(&out).is_ok_and(|m| m.len() > 0));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
