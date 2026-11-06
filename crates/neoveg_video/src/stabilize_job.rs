//! Stabilizing a video file into a new file: the shake is measured on small copies of the frames
//! (cheap, and the whole clip fits in memory), then every full-size frame is shifted along the
//! smoothed path and the result is encoded.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::decode::{probe, VideoDecoder};
use crate::frame::FrameBuffer;
use crate::render::{encode_video, RenderEngine, RenderFormat, VideoEncodeSpec};
use crate::stabilize::{corrections, crop_zoom, shift_and_zoom, StabilizeOptions};

/// Width of the copies the shake is measured on.
const ANALYSIS_WIDTH: u32 = 320;
/// Longest clip the analysis keeps in memory, in frames.
pub const MAX_FRAMES: u64 = 3600;

/// Writes the stabilized `duration` seconds of `src` starting at `in_seconds` to `output` (video
/// only, `fps` frames per second). `on_progress(done, total)` counts the analysis as the first half.
pub fn stabilize_file(
    src: &Path,
    in_seconds: f64,
    duration: f64,
    fps: f64,
    output: &Path,
    options: StabilizeOptions,
    engine: RenderEngine,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(u64, u64) + Send,
) -> Result<(), String> {
    let info = probe(src)?;
    let total = (duration * fps).ceil().max(1.0) as u64;
    if total > MAX_FRAMES {
        return Err(format!(
            "o trecho tem {total} quadros; o limite da estabilização é {MAX_FRAMES}"
        ));
    }
    let (width, height) = (info.width.max(2) & !1, info.height.max(2) & !1);
    let mut decoder = VideoDecoder::open(src)?;
    let time = |i: u64| in_seconds + i as f64 / fps;
    // Pass 1: the shake, measured on small frames
    let mut small = Vec::with_capacity(total as usize);
    for i in 0..total {
        if cancel.load(Ordering::Relaxed) {
            return Err("estabilização cancelada pelo usuário".into());
        }
        small.push(decoder.frame_at(time(i), ANALYSIS_WIDTH)?);
        on_progress(i, total * 2);
    }
    let small_width = small.first().map_or(1, |f| f.width.max(1));
    let scale = width as f32 / small_width as f32;
    let measured = StabilizeOptions {
        max_shift: ((options.max_shift as f32 / scale).round() as u32).max(1),
        ..options
    };
    let fix: Vec<(f32, f32)> = corrections(&small, measured)
        .into_iter()
        .map(|c| (c.0 * scale, c.1 * scale))
        .collect();
    drop(small);
    let zoom = crop_zoom(&fix, width, height);
    // Pass 2: the full-size frames, shifted and encoded
    let spec = VideoEncodeSpec {
        format: RenderFormat::Mp4H264,
        width,
        height,
        fps,
        crf: 18,
        total_frames: total,
        audio_wav: None,
        output,
    };
    encode_video(
        engine,
        &spec,
        cancel,
        |done, _| on_progress(total + done, total * 2),
        |i| {
            let frame = decoder.frame_at(time(i), 0)?;
            let (dx, dy) = fix[i as usize];
            let mut out = shift_and_zoom(&frame, dx, dy, zoom);
            if (out.width, out.height) != (width, height) {
                out = FrameBuffer::solid(width, height, [0, 0, 0, 255]);
            }
            Ok(out)
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noise(x: u32, y: u32) -> u8 {
        let mut n = x.wrapping_mul(374_761_393) ^ y.wrapping_mul(668_265_263);
        n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
        ((n ^ (n >> 16)) & 255) as u8
    }

    #[test]
    fn a_shaky_clip_is_written_as_a_new_file_of_the_same_size_and_length() {
        let dir = std::env::temp_dir().join(format!("neoveg_stab_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (src, out) = (dir.join("tremido.mp4"), dir.join("estabilizado.mp4"));
        let spec = VideoEncodeSpec {
            format: RenderFormat::Mp4H264,
            width: 128,
            height: 72,
            fps: 10.0,
            crf: 18,
            total_frames: 10,
            audio_wav: None,
            output: &src,
        };
        let cancel = AtomicBool::new(false);
        encode_video(RenderEngine::Internal, &spec, &cancel, |_, _| {}, |i| {
            let shake = if i % 2 == 0 { 0 } else { 3 };
            let mut f = FrameBuffer::solid(128, 72, [0, 0, 0, 255]);
            for y in 0..72u32 {
                for x in 0..128u32 {
                    let v = noise((x + shake) / 4, y / 4);
                    let at = ((y * 128 + x) * 4) as usize;
                    f.rgba[at..at + 4].copy_from_slice(&[v, v, v, 255]);
                }
            }
            Ok(f)
        })
        .unwrap();
        let mut progress = Vec::new();
        stabilize_file(
            &src,
            0.0,
            1.0,
            10.0,
            &out,
            StabilizeOptions::default(),
            RenderEngine::Internal,
            &cancel,
            |d, t| progress.push((d, t)),
        )
        .unwrap();
        let info = probe(&out).unwrap();
        assert_eq!((info.width, info.height), (128, 72));
        assert!((info.duration - 1.0).abs() < 0.2, "{}", info.duration);
        assert_eq!(progress.last(), Some(&(20, 20)));
        // Too long a clip is refused, and a cancel stops it
        let too_long = stabilize_file(&src, 0.0, 1e6, 10.0, &out, StabilizeOptions::default(), RenderEngine::Internal, &cancel, |_, _| {});
        assert!(too_long.is_err());
        let stop = AtomicBool::new(true);
        let stopped = stabilize_file(&src, 0.0, 1.0, 10.0, &dir.join("x.mp4"), StabilizeOptions::default(), RenderEngine::Internal, &stop, |_, _| {});
        assert!(stopped.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
