//! What the Media Properties dialog changes in the picture of each decoded frame: how the alpha
//! channel is read, a background colour under the transparent parts, and the pixel aspect ratio
//! (the picture is made as wide as it is meant to be seen, so the rest of the pipeline can treat
//! every pixel as square).
//!
//! The pipeline composites straight (unmatted) alpha, so a file whose colours are already
//! multiplied by its alpha is divided back.

use neoveg_common::media_props::{AlphaMode, MediaProps};

use crate::frame::FrameBuffer;

/// Applies the alpha mode, the background and the pixel aspect ratio of `props` to `frame`. A frame
/// that needs none of them is returned as it is.
pub fn apply(frame: &FrameBuffer, props: &MediaProps) -> FrameBuffer {
    if !props.changes_picture() || frame.width == 0 || frame.height == 0 {
        return frame.clone();
    }
    let mut out = frame.clone();
    read_alpha(&mut out, props.alpha);
    if let Some(color) = props.background {
        fill_background(&mut out, color);
    }
    if (props.pixel_aspect - 1.0).abs() > 1e-9 {
        out = stretch_width(&out, props.pixel_aspect);
    }
    out
}

/// The width a picture has once its pixels are square: `width * pixel_aspect`, even and at least 2.
pub fn square_width(width: u32, pixel_aspect: f64) -> u32 {
    let scaled = (f64::from(width) * pixel_aspect).round();
    (scaled.clamp(2.0, 16384.0) as u32) & !1
}

fn read_alpha(frame: &mut FrameBuffer, mode: AlphaMode) {
    match mode {
        AlphaMode::Auto | AlphaMode::Straight => {}
        AlphaMode::Undefined | AlphaMode::None => {
            for px in frame.rgba.as_chunks_mut::<4>().0.iter_mut() {
                px[3] = 255;
            }
        }
        AlphaMode::Premultiplied | AlphaMode::PremultipliedDirty => {
            for px in frame.rgba.as_chunks_mut::<4>().0.iter_mut() {
                let alpha = u32::from(px[3]);
                if alpha == 0 {
                    px[0] = 0;
                    px[1] = 0;
                    px[2] = 0;
                } else if alpha < 255 {
                    // A "dirty" colour may exceed the alpha: the result is held at full
                    for c in &mut px[..3] {
                        *c = ((u32::from(*c) * 255 + alpha / 2) / alpha).min(255) as u8;
                    }
                }
            }
        }
    }
}

fn fill_background(frame: &mut FrameBuffer, color: [u8; 3]) {
    for px in frame.rgba.as_chunks_mut::<4>().0.iter_mut() {
        let alpha = u32::from(px[3]);
        if alpha == 255 {
            continue;
        }
        for (c, bg) in px[..3].iter_mut().zip(color) {
            *c = ((u32::from(*c) * alpha + u32::from(bg) * (255 - alpha) + 127) / 255) as u8;
        }
        px[3] = 255;
    }
}

/// Linear resampling of each row to the width the pixel aspect ratio asks for.
fn stretch_width(frame: &FrameBuffer, pixel_aspect: f64) -> FrameBuffer {
    let new_width = square_width(frame.width, pixel_aspect);
    let (w, h) = (frame.width as usize, frame.height as usize);
    let nw = new_width as usize;
    let mut rgba = vec![0u8; nw * h * 4];
    for y in 0..h {
        let row = &frame.rgba[y * w * 4..(y + 1) * w * 4];
        for x in 0..nw {
            // The centre of the new pixel, in the old pixels
            let at = ((x as f64 + 0.5) * w as f64 / nw as f64 - 0.5).clamp(0.0, (w - 1) as f64);
            let left = at.floor() as usize;
            let right = (left + 1).min(w - 1);
            let t = at - left as f64;
            for c in 0..4 {
                let a = f64::from(row[left * 4 + c]);
                let b = f64::from(row[right * 4 + c]);
                rgba[(y * nw + x) * 4 + c] = (a + (b - a) * t).round() as u8;
            }
        }
    }
    FrameBuffer {
        width: new_width,
        height: frame.height,
        rgba,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(f: impl FnOnce(&mut MediaProps)) -> MediaProps {
        let mut p = MediaProps::default();
        f(&mut p);
        p
    }

    #[test]
    fn a_frame_that_needs_nothing_comes_back_as_it_is() {
        let frame = FrameBuffer::solid(4, 2, [10, 20, 30, 128]);
        assert_eq!(apply(&frame, &MediaProps::default()), frame);
        // Only the rate and the timecode are set: they are not the picture's business
        let rate = props(|p| p.substitute_fps = Some(25.0));
        assert_eq!(apply(&frame, &rate), frame);
    }

    #[test]
    fn the_alpha_mode_decides_how_transparency_is_read() {
        let frame = FrameBuffer::solid(2, 2, [64, 32, 16, 128]);
        let undefined = apply(&frame, &props(|p| p.alpha = AlphaMode::Undefined));
        assert_eq!(undefined.pixel(0, 0), [64, 32, 16, 255]);
        let none = apply(&frame, &props(|p| p.alpha = AlphaMode::None));
        assert_eq!(none.pixel(1, 1), [64, 32, 16, 255]);
        // Straight is what the pipeline already assumes
        let straight = apply(&frame, &props(|p| p.alpha = AlphaMode::Straight));
        assert_eq!(straight, frame);
        // Premultiplied colours are divided by the alpha (128/255): 64 -> 128, 32 -> 64, 16 -> 32
        let premultiplied = apply(&frame, &props(|p| p.alpha = AlphaMode::Premultiplied));
        assert_eq!(premultiplied.pixel(0, 0), [128, 64, 32, 128]);
        // Dirty: a colour above the alpha is held at full instead of wrapping
        let dirty = FrameBuffer::solid(1, 1, [200, 10, 0, 100]);
        let fixed = apply(&dirty, &props(|p| p.alpha = AlphaMode::PremultipliedDirty));
        assert_eq!(fixed.pixel(0, 0)[0], 255);
        assert_eq!(fixed.pixel(0, 0)[3], 100);
        // Fully transparent pixels have no colour left; opaque ones are untouched
        let clear = FrameBuffer::solid(1, 1, [9, 9, 9, 0]);
        assert_eq!(
            apply(&clear, &props(|p| p.alpha = AlphaMode::Premultiplied)).pixel(0, 0),
            [0, 0, 0, 0]
        );
        let opaque = FrameBuffer::solid(1, 1, [9, 8, 7, 255]);
        assert_eq!(
            apply(&opaque, &props(|p| p.alpha = AlphaMode::Premultiplied)),
            opaque
        );
    }

    #[test]
    fn the_background_shows_through_the_transparent_parts() {
        let color = props(|p| p.background = Some([0, 0, 255]));
        let clear = apply(&FrameBuffer::solid(1, 1, [255, 0, 0, 0]), &color);
        assert_eq!(clear.pixel(0, 0), [0, 0, 255, 255], "fully transparent");
        let opaque = apply(&FrameBuffer::solid(1, 1, [255, 0, 0, 255]), &color);
        assert_eq!(opaque.pixel(0, 0), [255, 0, 0, 255], "opaque stays");
        let half = apply(&FrameBuffer::solid(1, 1, [255, 0, 0, 128]), &color);
        let px = half.pixel(0, 0);
        assert_eq!(px[3], 255);
        assert!((i32::from(px[0]) - 128).abs() <= 1 && (i32::from(px[2]) - 127).abs() <= 1);
        // Read as opaque first, the background has nothing to fill
        let both = props(|p| {
            p.background = Some([0, 0, 255]);
            p.alpha = AlphaMode::None;
        });
        assert_eq!(
            apply(&FrameBuffer::solid(1, 1, [255, 0, 0, 0]), &both).pixel(0, 0),
            [255, 0, 0, 255]
        );
    }

    #[test]
    fn the_pixel_aspect_makes_the_picture_as_wide_as_it_is_meant_to_be_seen() {
        assert_eq!(square_width(720, 0.9091), 654);
        assert_eq!(square_width(1440, 1.3333), 1920);
        assert_eq!(square_width(1, 0.25), 2, "never below 2");
        // A 4 x 1 picture 0 | 100 | 200 | 255 at aspect 2 is 8 wide, smooth and with the same ends
        let mut frame = FrameBuffer::solid(4, 1, [0, 0, 0, 255]);
        for (x, v) in [0u8, 100, 200, 255].into_iter().enumerate() {
            frame.rgba[x * 4] = v;
        }
        let wide = apply(&frame, &props(|p| p.pixel_aspect = 2.0));
        assert_eq!((wide.width, wide.height), (8, 1));
        let reds: Vec<u8> = (0..8).map(|x| wide.pixel(x, 0)[0]).collect();
        assert_eq!(reds[0], 0);
        assert_eq!(reds[7], 255);
        assert!(reds.windows(2).all(|w| w[0] <= w[1]), "{reds:?}");
        // Narrowing keeps the height
        let narrow = apply(
            &FrameBuffer::solid(100, 40, [1, 2, 3, 255]),
            &props(|p| p.pixel_aspect = 0.5),
        );
        assert_eq!((narrow.width, narrow.height), (50, 40));
        assert_eq!(narrow.pixel(10, 10), [1, 2, 3, 255]);
    }
}
