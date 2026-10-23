//! FFmpeg video decoding (`ffmpeg` feature).

use std::path::Path;

use ffmpeg_next as ffmpeg;
use ffmpeg::format::Pixel;
use ffmpeg::media::Type;
use ffmpeg::software::scaling::{context::Context as Scaler, flag::Flags};
use ffmpeg::util::frame::video::Video;

use crate::frame::FrameBuffer;

/// Decodes the first video frame of `path` to RGBA.
pub fn decode_first_frame(path: &Path) -> Result<FrameBuffer, String> {
    ffmpeg::init().map_err(|e| format!("ffmpeg init: {e}"))?;
    let mut input = ffmpeg::format::input(&path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    let stream = input.streams().best(Type::Video).ok_or("no video stream")?;
    let index = stream.index();

    let context = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
        .map_err(|e| format!("codec parameters: {e}"))?;
    let mut decoder = context.decoder().video().map_err(|e| format!("video decoder: {e}"))?;
    let mut scaler = Scaler::get(
        decoder.format(),
        decoder.width(),
        decoder.height(),
        Pixel::RGBA,
        decoder.width(),
        decoder.height(),
        Flags::BILINEAR,
    )
    .map_err(|e| format!("scaler: {e}"))?;

    for (s, packet) in input.packets() {
        if s.index() != index {
            continue;
        }
        decoder.send_packet(&packet).map_err(|e| format!("send packet: {e}"))?;
        let mut decoded = Video::empty();
        if decoder.receive_frame(&mut decoded).is_ok() {
            let mut rgba = Video::empty();
            scaler.run(&decoded, &mut rgba).map_err(|e| format!("scale: {e}"))?;

            let (w, h) = (rgba.width() as usize, rgba.height() as usize);
            let stride = rgba.stride(0);
            let src = rgba.data(0);
            let mut out = Vec::with_capacity(w * h * 4);
            for row in 0..h {
                out.extend_from_slice(&src[row * stride..row * stride + w * 4]);
            }
            return Ok(FrameBuffer { width: w as u32, height: h as u32, rgba: out });
        }
    }
    Err(format!("no decodable video frame in {}", path.display()))
}
