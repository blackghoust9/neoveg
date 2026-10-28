//! Draws the picture of an event for its Pan/Crop box (CPU).
//!
//! The box is relative to the source picture (`neoveg_common::pan_crop`), so the same
//! [`PanCropFrame`] gives the same framing at any decoded size. The output has the size of the
//! source; the compositor fits it to the canvas afterwards, exactly as it does for an event without
//! Pan/Crop.
//!
//! * **Stretch to fill** (the default): the box fills the output. A smaller box is a zoom, a moved
//!   box is a pan, a rotated box straightens what is inside it. What the box covers outside the
//!   source is transparent, so lower tracks show through.
//! * **No stretch**: the picture keeps its size and only the part inside the box stays visible.
//!
//! Sampling is bilinear on premultiplied colour, so the edges of the picture and of a rotated box do
//! not pick up dark fringes.

use neoveg_common::pan_crop::PanCropFrame;

use crate::frame::FrameBuffer;

/// Pan/Crop of one clip as the worker receives it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanCropRequest {
    pub frame: PanCropFrame,
    pub stretch_to_fill: bool,
}

impl PanCropRequest {
    /// `true` when drawing it would give back the same picture.
    pub fn is_identity(&self) -> bool {
        self.stretch_to_fill && self.frame.is_full()
    }
}

/// Bilinear sample at pixel position `(x, y)` (pixel centres at `+0.5`): premultiplied colour and
/// alpha, `0..=255`. Taps outside the picture are transparent.
fn sample(src: &FrameBuffer, x: f32, y: f32) -> [f32; 4] {
    let (w, h) = (src.width as i64, src.height as i64);
    let (fx, fy) = (x - 0.5, y - 0.5);
    let (x0, y0) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - x0, fy - y0);
    let (x0, y0) = (x0 as i64, y0 as i64);
    let mut acc = [0.0f32; 4];
    for (dy, wy) in [(0, 1.0 - ty), (1, ty)] {
        for (dx, wx) in [(0, 1.0 - tx), (1, tx)] {
            let (px, py) = (x0 + dx, y0 + dy);
            if px < 0 || py < 0 || px >= w || py >= h {
                continue;
            }
            let i = (py as usize * w as usize + px as usize) * 4;
            let weight = wx * wy;
            let a = f32::from(src.rgba[i + 3]);
            let k = weight * a / 255.0;
            acc[0] += f32::from(src.rgba[i]) * k;
            acc[1] += f32::from(src.rgba[i + 1]) * k;
            acc[2] += f32::from(src.rgba[i + 2]) * k;
            acc[3] += weight * a;
        }
    }
    acc
}

fn write_pixel(out: &mut [u8], premul: [f32; 4]) {
    let alpha = premul[3];
    if alpha < 0.5 {
        out.copy_from_slice(&[0, 0, 0, 0]);
        return;
    }
    let k = 255.0 / alpha;
    out[0] = (premul[0] * k).round().clamp(0.0, 255.0) as u8;
    out[1] = (premul[1] * k).round().clamp(0.0, 255.0) as u8;
    out[2] = (premul[2] * k).round().clamp(0.0, 255.0) as u8;
    out[3] = alpha.round().clamp(0.0, 255.0) as u8;
}

/// The picture of `src` seen through the box `frame`.
pub fn apply(src: &FrameBuffer, frame: &PanCropFrame, stretch_to_fill: bool) -> FrameBuffer {
    let frame = frame.sanitized();
    if src.width == 0 || src.height == 0 || (stretch_to_fill && frame.is_full()) {
        return src.clone();
    }
    let (sw, sh) = (src.width as f32, src.height as f32);
    let (cx, cy) = (frame.x_center * sw, frame.y_center * sh);
    let (half_w, half_h) = (frame.width * sw / 2.0, frame.height * sh / 2.0);
    let (sin, cos) = frame.rotation_deg.to_radians().sin_cos();
    let width = src.width as usize;
    let mut out = FrameBuffer {
        width: src.width,
        height: src.height,
        rgba: vec![0; src.rgba.len()],
    };

    let row = |y: usize, line: &mut [u8]| {
        for (x, px) in line.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let (ox, oy) = (x as f32 + 0.5, y as f32 + 0.5);
            let value = if stretch_to_fill {
                // Output position inside the box, then the box's rotation, then its place on the source
                let dx = (ox / sw - 0.5) * frame.width * sw;
                let dy = (oy / sh - 0.5) * frame.height * sh;
                sample(src, cx + dx * cos - dy * sin, cy + dx * sin + dy * cos)
            } else {
                // Same position on the source; only what the rotated box covers stays visible
                let (dx, dy) = (ox - cx, oy - cy);
                let (lx, ly) = (dx * cos + dy * sin, -dx * sin + dy * cos);
                if lx.abs() <= half_w && ly.abs() <= half_h {
                    sample(src, ox, oy)
                } else {
                    [0.0; 4]
                }
            };
            write_pixel(px, value);
        }
    };

    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let height = src.height as usize;
    if width * height < 64 * 1024 || threads < 2 {
        for (y, line) in out.rgba.chunks_mut(width * 4).enumerate() {
            row(y, line);
        }
    } else {
        let band = height.div_ceil(threads);
        std::thread::scope(|s| {
            for (b, chunk) in out.rgba.chunks_mut(band * width * 4).enumerate() {
                let row = &row;
                s.spawn(move || {
                    for (i, line) in chunk.chunks_mut(width * 4).enumerate() {
                        row(b * band + i, line);
                    }
                });
            }
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A square picture in four colours: red, green on top; blue, white below.
    fn quadrants(size: u32) -> FrameBuffer {
        let mut f = FrameBuffer::solid(size, size, [0, 0, 0, 255]);
        for y in 0..size {
            for x in 0..size {
                let color = match (x < size / 2, y < size / 2) {
                    (true, true) => [255, 0, 0, 255],
                    (false, true) => [0, 255, 0, 255],
                    (true, false) => [0, 0, 255, 255],
                    (false, false) => [255, 255, 255, 255],
                };
                let i = ((y * size + x) * 4) as usize;
                f.rgba[i..i + 4].copy_from_slice(&color);
            }
        }
        f
    }

    fn near(a: [u8; 4], b: [u8; 4]) -> bool {
        a.iter().zip(&b).all(|(x, y)| x.abs_diff(*y) <= 2)
    }

    #[test]
    fn the_whole_picture_comes_back_unchanged() {
        let src = quadrants(32);
        assert_eq!(apply(&src, &PanCropFrame::FULL, true), src);
    }

    #[test]
    fn a_smaller_box_zooms_and_a_moved_box_pans() {
        let src = quadrants(64);
        // The top-left quarter, filling the frame: all red
        let zoom = apply(
            &src,
            &PanCropFrame {
                x_center: 0.25,
                y_center: 0.25,
                width: 0.5,
                height: 0.5,
                rotation_deg: 0.0,
            },
            true,
        );
        for (x, y) in [(2, 2), (32, 32), (61, 61)] {
            assert!(
                near(zoom.pixel(x, y), [255, 0, 0, 255]),
                "{x},{y} {:?}",
                zoom.pixel(x, y)
            );
        }
        // Panning the same size over to the right half shows green above and white below
        let pan = apply(
            &src,
            &PanCropFrame {
                x_center: 0.75,
                y_center: 0.5,
                width: 0.5,
                height: 1.0,
                rotation_deg: 0.0,
            },
            true,
        );
        assert!(near(pan.pixel(10, 5), [0, 255, 0, 255]));
        assert!(near(pan.pixel(10, 58), [255, 255, 255, 255]));
    }

    #[test]
    fn a_rotated_box_straightens_what_is_inside_it() {
        let src = quadrants(64);
        let turned = apply(
            &src,
            &PanCropFrame {
                rotation_deg: 90.0,
                ..PanCropFrame::FULL
            },
            true,
        );
        // A box turned a quarter clockwise sees the top-right quadrant at the output's top-left
        assert!(near(turned.pixel(8, 8), [0, 255, 0, 255]));
        assert!(near(turned.pixel(55, 8), [255, 255, 255, 255]));
        assert!(near(turned.pixel(8, 55), [255, 0, 0, 255]));
    }

    #[test]
    fn what_the_box_covers_beyond_the_picture_is_transparent_with_clean_edges() {
        let src = FrameBuffer::solid(32, 32, [200, 100, 50, 255]);
        let wide = apply(
            &src,
            &PanCropFrame {
                width: 2.0,
                height: 2.0,
                ..PanCropFrame::FULL
            },
            true,
        );
        // The picture fills the middle quarter of the frame; the rest shows what is below
        assert_eq!(wide.pixel(1, 1)[3], 0);
        assert!(near(wide.pixel(16, 16), [200, 100, 50, 255]));
        for p in wide.rgba.as_chunks::<4>().0 {
            if p[3] > 10 {
                assert!(near(*p, [200, 100, 50, p[3]]), "no dark fringe: {p:?}");
            }
        }
    }

    #[test]
    fn without_stretching_the_picture_keeps_its_size_and_is_cropped_by_the_box() {
        let src = quadrants(64);
        let cropped = apply(
            &src,
            &PanCropFrame {
                x_center: 0.25,
                y_center: 0.5,
                width: 0.5,
                height: 1.0,
                rotation_deg: 0.0,
            },
            false,
        );
        assert!(
            near(cropped.pixel(8, 8), [255, 0, 0, 255]),
            "inside the box, same place"
        );
        assert!(near(cropped.pixel(8, 55), [0, 0, 255, 255]));
        assert_eq!(cropped.pixel(50, 8)[3], 0, "outside the box: transparent");
    }

    #[test]
    fn the_same_box_frames_a_big_and_a_small_picture_alike() {
        // Large enough to take the threaded path; compare with the zoom of a small copy
        let big = quadrants(512);
        let small = quadrants(64);
        let frame = PanCropFrame {
            x_center: 0.3,
            y_center: 0.6,
            width: 0.4,
            height: 0.4,
            rotation_deg: 20.0,
        };
        let (a, b) = (apply(&big, &frame, true), apply(&small, &frame, true));
        let mut worst = 0u32;
        for y in (0..64u32).step_by(7) {
            for x in (0..64u32).step_by(7) {
                let (p, q) = (a.pixel(x * 8, y * 8), b.pixel(x, y));
                let d: u32 = (0..3)
                    .map(|c| u32::from(p[c]).abs_diff(u32::from(q[c])))
                    .sum();
                worst = worst.max(d);
            }
        }
        // Only the colour boundaries differ (a few pixels wide); most samples must agree closely
        let agree = (0..64u32)
            .step_by(7)
            .flat_map(|y| (0..64u32).step_by(7).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let (p, q) = (a.pixel(x * 8, y * 8), b.pixel(x, y));
                near(p, q)
                    || (0..3)
                        .map(|c| u32::from(p[c]).abs_diff(u32::from(q[c])))
                        .sum::<u32>()
                        < 140
            })
            .count();
        assert!(agree >= 70, "{agree} of 81 samples agree (worst {worst})");
    }
}
