//! Applying or removing 3:2 pulldown on a video file, writing the result as a new file. The frame
//! shuffling is [`crate::pulldown`]; this module streams it: the output frame `i` is built from the
//! one cadence group it belongs to, so a clip never has to fit in memory.

#[cfg(feature = "ffmpeg")]
use std::path::Path;
#[cfg(feature = "ffmpeg")]
use std::sync::atomic::AtomicBool;

use crate::frame::FrameBuffer;
#[cfg(feature = "ffmpeg")]
use crate::pulldown::detect_phase;
use crate::pulldown::{
    apply_pulldown, film_rate, remove_pulldown, video_rate, FILM_GROUP, VIDEO_GROUP,
};

/// Frames looked at to find the cadence of a file.
#[cfg(feature = "ffmpeg")]
const PHASE_PROBE_FRAMES: u64 = 40;

/// Which way the conversion goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PulldownMode {
    /// 23.976 fps film carried in 29.97 fps video becomes 23.976 fps film again.
    Remove,
    /// 23.976 fps film becomes 29.97 fps video with the 2:3 cadence.
    Apply,
}

impl PulldownMode {
    /// Frame rate of the converted file for a source at `fps`.
    pub fn output_fps(self, fps: f64) -> f64 {
        match self {
            Self::Remove => film_rate(fps),
            Self::Apply => video_rate(fps),
        }
    }

    /// Suffix of the name of the converted file.
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Remove => "sem_pulldown",
            Self::Apply => "com_pulldown",
        }
    }
}

/// Number of frames the converted file has for `input` frames (`phase` frames are skipped first
/// when removing; an incomplete last group is dropped).
pub fn output_frame_count(mode: PulldownMode, input: u64, phase: usize) -> u64 {
    match mode {
        Mode::Remove => input.saturating_sub(phase as u64) / VIDEO_GROUP as u64 * FILM_GROUP as u64,
        Mode::Apply => input / FILM_GROUP as u64 * VIDEO_GROUP as u64,
    }
}
use PulldownMode as Mode;

/// Builds the frames of the converted file one at a time, keeping the cadence group in use.
pub struct Converter {
    mode: PulldownMode,
    phase: usize,
    cached: Option<(u64, Vec<FrameBuffer>)>,
}

impl Converter {
    pub fn new(mode: PulldownMode, phase: usize) -> Self {
        Self {
            mode,
            phase,
            cached: None,
        }
    }

    /// Output frame `index`; `fetch(n)` returns input frame `n`.
    pub fn frame(
        &mut self,
        index: u64,
        fetch: &mut dyn FnMut(u64) -> Result<FrameBuffer, String>,
    ) -> Result<FrameBuffer, String> {
        let (out_group, in_group) = match self.mode {
            Mode::Remove => (FILM_GROUP as u64, VIDEO_GROUP as u64),
            Mode::Apply => (VIDEO_GROUP as u64, FILM_GROUP as u64),
        };
        let (group, k) = (index / out_group, (index % out_group) as usize);
        if self.cached.as_ref().is_none_or(|(g, _)| *g != group) {
            let first = group * in_group
                + if self.mode == Mode::Remove {
                    self.phase as u64
                } else {
                    0
                };
            let input = (0..in_group)
                .map(|n| fetch(first + n))
                .collect::<Result<Vec<_>, _>>()?;
            let converted = match self.mode {
                Mode::Remove => remove_pulldown(&input, 0),
                Mode::Apply => apply_pulldown(&input),
            };
            self.cached = Some((group, converted));
        }
        self.cached
            .as_ref()
            .and_then(|(_, frames)| frames.get(k).cloned())
            .ok_or_else(|| "grupo de quadros incompleto".to_string())
    }
}

/// Finds the cadence phase of `src` from its first frames; `None` when it shows no 3:2 cadence.
#[cfg(feature = "ffmpeg")]
pub fn detect_file_phase(src: &Path, fps: f64) -> Result<Option<usize>, String> {
    let mut decoder = crate::decode::VideoDecoder::open(src)?;
    let frames = (0..PHASE_PROBE_FRAMES)
        .map(|i| decoder.frame_at(i as f64 / fps, 320))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(detect_phase(&frames))
}

/// Writes `src` converted by `mode` to `output` (video only). `on_progress(done, total)` counts
/// the output frames. Removing needs a clip that shows the cadence.
#[cfg(feature = "ffmpeg")]
pub fn convert_file(
    src: &Path,
    mode: PulldownMode,
    output: &Path,
    engine: crate::render::RenderEngine,
    cancel: &AtomicBool,
    on_progress: impl FnMut(u64, u64) + Send,
) -> Result<(), String> {
    use crate::decode::{probe, VideoDecoder};
    use crate::render::{encode_video, RenderFormat, VideoEncodeSpec};
    let info = probe(src)?;
    if info.fps <= 0.0 {
        return Err("a taxa de quadros do arquivo é desconhecida".into());
    }
    let phase = match mode {
        Mode::Remove => detect_file_phase(src, info.fps)?
            .ok_or("o arquivo não mostra o ritmo 3:2 de pulldown")?,
        Mode::Apply => 0,
    };
    let input = (info.duration * info.fps).floor() as u64;
    let total = output_frame_count(mode, input, phase);
    if total == 0 {
        return Err("o arquivo é curto demais para a conversão".into());
    }
    let (width, height) = (info.width.max(2) & !1, info.height.max(2) & !1);
    let spec = VideoEncodeSpec {
        format: RenderFormat::Mp4H264,
        width,
        height,
        fps: mode.output_fps(info.fps),
        crf: 18,
        total_frames: total,
        audio_wav: None,
        output,
    };
    let mut decoder = VideoDecoder::open(src)?;
    let mut converter = Converter::new(mode, phase);
    encode_video(engine, &spec, cancel, on_progress, |i| {
        converter.frame(i, &mut |n| decoder.frame_at(n as f64 / info.fps, 0))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pulldown::detect_phase;

    fn film_frame(i: usize) -> FrameBuffer {
        let (w, h) = (16u32, 12u32);
        let mut f = FrameBuffer::solid(w, h, [0, 0, 0, 255]);
        for y in 0..h {
            for x in 0..w {
                let p = ((y * w + x) * 4) as usize;
                f.rgba[p] = (i as u32 * 40 + x * 3) as u8;
                f.rgba[p + 1] = (y * 5) as u8 + i as u8;
                f.rgba[p + 2] = 90;
            }
        }
        f
    }

    #[test]
    fn frame_counts_follow_the_cadence_groups() {
        assert_eq!(output_frame_count(Mode::Apply, 12, 0), 15);
        assert_eq!(output_frame_count(Mode::Apply, 11, 0), 10);
        assert_eq!(output_frame_count(Mode::Remove, 15, 0), 12);
        assert_eq!(output_frame_count(Mode::Remove, 15, 3), 8);
        assert_eq!(output_frame_count(Mode::Remove, 2, 3), 0);
        assert!((Mode::Remove.output_fps(30000.0 / 1001.0) - 24000.0 / 1001.0).abs() < 1e-9);
    }

    #[test]
    fn applying_then_removing_streamed_frame_by_frame_gives_the_film_back() {
        let film: Vec<_> = (0..12).map(film_frame).collect();
        let mut apply = Converter::new(Mode::Apply, 0);
        let video: Vec<_> = (0..output_frame_count(Mode::Apply, 12, 0))
            .map(|i| {
                apply
                    .frame(i, &mut |n| Ok(film[n as usize].clone()))
                    .unwrap()
            })
            .collect();
        assert_eq!(video, apply_pulldown(&film));
        let phase = detect_phase(&video).unwrap();
        let mut remove = Converter::new(Mode::Remove, phase);
        let back: Vec<_> = (0..output_frame_count(Mode::Remove, 15, phase))
            .map(|i| {
                remove
                    .frame(i, &mut |n| Ok(video[n as usize].clone()))
                    .unwrap()
            })
            .collect();
        assert_eq!(back, film);
        // A missing input frame is an error, not a panic
        let mut broken = Converter::new(Mode::Apply, 0);
        assert!(broken.frame(0, &mut |_| Err("falta".into())).is_err());
    }

    #[cfg(feature = "ffmpeg")]
    #[test]
    fn a_telecined_file_is_converted_back_to_four_fifths_of_the_frames() {
        use crate::decode::probe;
        use crate::render::{encode_video, RenderEngine, RenderFormat, VideoEncodeSpec};
        let dir = std::env::temp_dir().join(format!("neoveg_pd_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (film_path, tele, back) = (
            dir.join("filme.mp4"),
            dir.join("tele.mp4"),
            dir.join("volta.mp4"),
        );
        let cancel = AtomicBool::new(false);
        let film: Vec<_> = (0..24)
            .map(|i| {
                let mut f = film_frame(i);
                // Larger to survive the encoder: replicate into 64x48
                let mut big = FrameBuffer::solid(64, 48, [0, 0, 0, 255]);
                for y in 0..48u32 {
                    for x in 0..64u32 {
                        let s = ((y / 4 * 16 + x / 4) * 4) as usize;
                        let d = ((y * 64 + x) * 4) as usize;
                        big.rgba[d..d + 4].copy_from_slice(&f.rgba[s..s + 4]);
                    }
                }
                f = big;
                f
            })
            .collect();
        let spec = VideoEncodeSpec {
            format: RenderFormat::Mp4H264,
            width: 64,
            height: 48,
            fps: 24.0,
            crf: 12,
            total_frames: 24,
            audio_wav: None,
            output: &film_path,
        };
        encode_video(
            RenderEngine::Internal,
            &spec,
            &cancel,
            |_, _| {},
            |i| Ok(film[i as usize].clone()),
        )
        .unwrap();
        convert_file(
            &film_path,
            Mode::Apply,
            &tele,
            RenderEngine::Internal,
            &cancel,
            |_, _| {},
        )
        .unwrap();
        let info = probe(&tele).unwrap();
        assert!((info.fps - 30.0).abs() < 0.1, "{}", info.fps);
        // Progressive material shows no cadence, so removing it from the original is refused
        assert!(convert_file(
            &film_path,
            Mode::Remove,
            &dir.join("x.mp4"),
            RenderEngine::Internal,
            &cancel,
            |_, _| {}
        )
        .is_err());
        let _ = back;
        let _ = std::fs::remove_dir_all(&dir);
    }
}
