//! Stabilization of shaky video clips: the global motion between consecutive frames is measured
//! (block matching on the luma, coarse then fine), the accumulated camera path is smoothed, and
//! each frame is shifted by the difference between the smooth path and the real one. A slight
//! zoom hides the borders the shift uncovers.

use crate::frame::FrameBuffer;

/// Settings of the stabilizer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StabilizeOptions {
    /// Largest shift between two frames to look for, in pixels of the full frame.
    pub max_shift: u32,
    /// Half width of the smoothing window, in frames (the larger, the steadier the shot).
    pub smoothing_radius: usize,
}

impl Default for StabilizeOptions {
    fn default() -> Self {
        Self {
            max_shift: 24,
            smoothing_radius: 8,
        }
    }
}

fn luma_plane(f: &FrameBuffer, factor: u32) -> (Vec<f32>, usize, usize) {
    let (w, h) = ((f.width / factor) as usize, (f.height / factor) as usize);
    let mut out = vec![0.0f32; w * h];
    let n = (factor * factor) as f32;
    for y in 0..h {
        for x in 0..w {
            let mut s = 0.0;
            for dy in 0..factor as usize {
                for dx in 0..factor as usize {
                    let i = (((y * factor as usize + dy) * f.width as usize)
                        + x * factor as usize
                        + dx)
                        * 4;
                    s += 0.2126 * f32::from(f.rgba[i])
                        + 0.7152 * f32::from(f.rgba[i + 1])
                        + 0.0722 * f32::from(f.rgba[i + 2]);
                }
            }
            out[y * w + x] = s / n;
        }
    }
    (out, w, h)
}

/// Mean absolute difference between `a` and `b` shifted by `(dx, dy)` (b[x+dx] against a[x]),
/// over the interior margin.
fn mad(a: &[f32], b: &[f32], w: usize, h: usize, dx: i32, dy: i32, margin: usize) -> f32 {
    let mut sum = 0.0;
    let mut n = 0u32;
    for y in margin..h - margin {
        let by = (y as i32 + dy) as usize;
        for x in margin..w - margin {
            let bx = (x as i32 + dx) as usize;
            sum += (a[y * w + x] - b[by * w + bx]).abs();
            n += 1;
        }
    }
    sum / n.max(1) as f32
}

/// Global translation `(dx, dy)` of the content from `a` to `b`, in pixels: a feature at `p` in
/// `a` is at `p + (dx, dy)` in `b`. Zero for frames of different size or too small to match.
pub fn estimate_shift(a: &FrameBuffer, b: &FrameBuffer, max_shift: u32) -> (f32, f32) {
    if a.width != b.width || a.height != b.height || a.width < 16 || a.height < 16 {
        return (0.0, 0.0);
    }
    // Coarse: on a plane of at most ~160 pixels wide
    let factor = (a.width / 160).max(1);
    let (pa, w, h) = luma_plane(a, factor);
    let (pb, _, _) = luma_plane(b, factor);
    let reach = ((max_shift / factor).max(1) as usize).min(w.min(h) / 3) as i32;
    let margin = reach as usize + 1;
    if w <= 2 * margin + 4 || h <= 2 * margin + 4 {
        return (0.0, 0.0);
    }
    let mut best = (0i32, 0i32, f32::MAX);
    for dy in -reach..=reach {
        for dx in -reach..=reach {
            let m = mad(&pa, &pb, w, h, dx, dy, margin);
            // Prefer the smaller shift on ties so that flat frames report no motion
            let bias = (dx.abs() + dy.abs()) as f32 * 1e-4;
            if m + bias < best.2 {
                best = (dx, dy, m + bias);
            }
        }
    }
    let (cdx, cdy) = (best.0 * factor as i32, best.1 * factor as i32);
    if factor == 1 {
        return (cdx as f32, cdy as f32);
    }
    // Fine: full resolution, within one coarse step of the coarse answer
    let (fa, fw, fh) = luma_plane(a, 1);
    let (fb, _, _) = luma_plane(b, 1);
    let fmargin = (cdx.unsigned_abs().max(cdy.unsigned_abs()) + factor + 1) as usize;
    if fw <= 2 * fmargin + 4 || fh <= 2 * fmargin + 4 {
        return (cdx as f32, cdy as f32);
    }
    let f = factor as i32;
    let mut fine = (cdx, cdy, f32::MAX);
    for dy in cdy - f..=cdy + f {
        for dx in cdx - f..=cdx + f {
            let m = mad(&fa, &fb, fw, fh, dx, dy, fmargin);
            if m < fine.2 {
                fine = (dx, dy, m);
            }
        }
    }
    (fine.0 as f32, fine.1 as f32)
}

/// Accumulated camera path of `frames`: the content position of every frame relative to the
/// first, in pixels.
pub fn camera_path(frames: &[FrameBuffer], max_shift: u32) -> Vec<(f32, f32)> {
    let mut path = Vec::with_capacity(frames.len());
    let mut acc = (0.0f32, 0.0f32);
    for (i, f) in frames.iter().enumerate() {
        if i > 0 {
            let (dx, dy) = estimate_shift(&frames[i - 1], f, max_shift);
            acc.0 += dx;
            acc.1 += dy;
        }
        path.push(acc);
    }
    path
}

/// Moving average of `path` with a window of `radius` frames on each side (shrinking at the ends).
pub fn smooth_path(path: &[(f32, f32)], radius: usize) -> Vec<(f32, f32)> {
    (0..path.len())
        .map(|i| {
            let lo = i.saturating_sub(radius);
            let hi = (i + radius).min(path.len() - 1);
            let n = (hi - lo + 1) as f32;
            let (sx, sy) = path[lo..=hi]
                .iter()
                .fold((0.0, 0.0), |(x, y), p| (x + p.0, y + p.1));
            (sx / n, sy / n)
        })
        .collect()
}

/// Shift to apply to every frame so that its content follows the smoothed path.
pub fn corrections(frames: &[FrameBuffer], options: StabilizeOptions) -> Vec<(f32, f32)> {
    let path = camera_path(frames, options.max_shift);
    let smooth = smooth_path(&path, options.smoothing_radius);
    path.iter()
        .zip(&smooth)
        .map(|(p, s)| (s.0 - p.0, s.1 - p.1))
        .collect()
}

/// Zoom factor that hides the borders uncovered by the largest of `corrections` on a frame of
/// `width` x `height`.
pub fn crop_zoom(corrections: &[(f32, f32)], width: u32, height: u32) -> f32 {
    if width == 0 || height == 0 {
        return 1.0;
    }
    let mx = corrections.iter().fold(0.0f32, |m, c| m.max(c.0.abs()));
    let my = corrections.iter().fold(0.0f32, |m, c| m.max(c.1.abs()));
    (1.0 + 2.0 * mx / width as f32).max(1.0 + 2.0 * my / height as f32)
}

/// Shifts `frame` by `(dx, dy)` pixels (content moves by that much) after zooming it by `zoom`
/// about its centre, with bilinear sampling. Pixels that come from outside repeat the edge.
pub fn shift_and_zoom(frame: &FrameBuffer, dx: f32, dy: f32, zoom: f32) -> FrameBuffer {
    let (w, h) = (frame.width as usize, frame.height as usize);
    if w == 0 || h == 0 {
        return frame.clone();
    }
    let zoom = if zoom.is_finite() { zoom.max(1.0) } else { 1.0 };
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let mut out = frame.clone();
    for y in 0..h {
        for x in 0..w {
            // Source position of the output pixel
            let sx = ((x as f32 + 0.5 - dx) - cx) / zoom + cx - 0.5;
            let sy = ((y as f32 + 0.5 - dy) - cy) / zoom + cy - 0.5;
            let sx = sx.clamp(0.0, (w - 1) as f32);
            let sy = sy.clamp(0.0, (h - 1) as f32);
            let (x0, y0) = (sx.floor() as usize, sy.floor() as usize);
            let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
            let (fx, fy) = (sx - x0 as f32, sy - y0 as f32);
            let o = (y * w + x) * 4;
            for c in 0..4 {
                let p = |xx: usize, yy: usize| f32::from(frame.rgba[(yy * w + xx) * 4 + c]);
                let top = p(x0, y0) * (1.0 - fx) + p(x1, y0) * fx;
                let bot = p(x0, y1) * (1.0 - fx) + p(x1, y1) * fx;
                out.rgba[o + c] = (top * (1.0 - fy) + bot * fy).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

/// Stabilizes a clip: the frames shifted along the smoothed camera path and zoomed to hide the
/// borders.
pub fn stabilize(frames: &[FrameBuffer], options: StabilizeOptions) -> Vec<FrameBuffer> {
    if frames.is_empty() {
        return Vec::new();
    }
    let fix = corrections(frames, options);
    let zoom = crop_zoom(&fix, frames[0].width, frames[0].height);
    frames
        .iter()
        .zip(&fix)
        .map(|(f, c)| shift_and_zoom(f, c.0, c.1, zoom))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smooth-ish value noise of a large virtual canvas; sampling it at an offset moves the view.
    fn canvas(x: i32, y: i32) -> u8 {
        let hash = |ix: i32, iy: i32| -> f32 {
            let mut n = (ix.wrapping_mul(374_761_393) ^ iy.wrapping_mul(668_265_263)) as u32;
            n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
            ((n ^ (n >> 16)) & 255) as f32
        };
        // Cells of 6 pixels, bilinear between lattice values
        let (gx, gy) = (x.div_euclid(6), y.div_euclid(6));
        let (fx, fy) = (x.rem_euclid(6) as f32 / 6.0, y.rem_euclid(6) as f32 / 6.0);
        let top = hash(gx, gy) * (1.0 - fx) + hash(gx + 1, gy) * fx;
        let bot = hash(gx, gy + 1) * (1.0 - fx) + hash(gx + 1, gy + 1) * fx;
        (top * (1.0 - fy) + bot * fy) as u8
    }

    /// A 96x72 view of the canvas whose top-left corner is at `(ox, oy)`.
    fn view(ox: i32, oy: i32) -> FrameBuffer {
        let (w, h) = (96u32, 72u32);
        let mut f = FrameBuffer::solid(w, h, [0, 0, 0, 255]);
        for y in 0..h {
            for x in 0..w {
                let v = canvas(ox + x as i32, oy + y as i32);
                let i = ((y * w + x) * 4) as usize;
                f.rgba[i] = v;
                f.rgba[i + 1] = v;
                f.rgba[i + 2] = v;
            }
        }
        f
    }

    fn error(a: &FrameBuffer, b: &FrameBuffer, margin: u32) -> f32 {
        let mut s = 0.0;
        let mut n = 0.0;
        for y in margin..a.height - margin {
            for x in margin..a.width - margin {
                s += (f32::from(a.pixel(x, y)[0]) - f32::from(b.pixel(x, y)[0])).abs();
                n += 1.0;
            }
        }
        s / n
    }

    #[test]
    fn the_shift_between_two_frames_is_measured() {
        // The view moves right/down on the canvas, so the content moves left/up in the frame
        let a = view(100, 100);
        let b = view(103, 98);
        assert_eq!(estimate_shift(&a, &b, 12), (-3.0, 2.0));
        assert_eq!(estimate_shift(&a, &a, 12), (0.0, 0.0));
        // Different sizes: no answer
        assert_eq!(estimate_shift(&a, &FrameBuffer::solid(8, 8, [0; 4]), 12), (0.0, 0.0));
    }

    #[test]
    fn a_large_frame_is_measured_through_the_coarse_and_fine_passes() {
        // 4x the canvas scale: pixel (x, y) shows canvas (x/4, y/4)
        let big = |ox: i32, oy: i32| {
            let (w, h) = (480u32, 320u32);
            let mut f = FrameBuffer::solid(w, h, [0, 0, 0, 255]);
            for y in 0..h {
                for x in 0..w {
                    let v = canvas((ox + x as i32).div_euclid(4), (oy + y as i32).div_euclid(4));
                    let i = ((y * w + x) * 4) as usize;
                    f.rgba[i] = v;
                    f.rgba[i + 1] = v;
                    f.rgba[i + 2] = v;
                }
            }
            f
        };
        let (dx, dy) = estimate_shift(&big(400, 400), &big(409, 396), 32);
        assert!((dx + 9.0).abs() <= 1.0 && (dy - 4.0).abs() <= 1.0, "{dx} {dy}");
    }

    #[test]
    fn a_shaky_clip_comes_out_steadier_than_it_went_in() {
        // Camera jitter around a fixed point
        let jitter = [(0, 0), (4, -3), (-3, 4), (5, 2), (-4, -4), (2, 5), (-5, 1), (3, -2), (0, 3), (-2, -5), (4, 4), (-3, 0)];
        let frames: Vec<_> = jitter.iter().map(|&(x, y)| view(200 + x, 200 + y)).collect();
        let truth = view(200, 200);
        let before: f32 = frames.iter().map(|f| error(f, &truth, 12)).sum::<f32>() / 12.0;
        let out = stabilize(
            &frames,
            StabilizeOptions {
                max_shift: 12,
                smoothing_radius: 20,
            },
        );
        assert_eq!(out.len(), frames.len());
        // Compare the stabilized frames with each other: they must agree far better than the input
        let spread = |fs: &[FrameBuffer]| -> f32 {
            fs.windows(2).map(|w| error(&w[0], &w[1], 12)).sum::<f32>() / (fs.len() - 1) as f32
        };
        let (s_in, s_out) = (spread(&frames), spread(&out));
        assert!(before > 5.0, "the input really shakes: {before}");
        assert!(s_out < s_in * 0.5, "frame-to-frame difference {s_in} -> {s_out}");
    }

    #[test]
    fn smoothing_and_zoom_do_the_obvious_thing() {
        let p = [(0.0, 0.0), (10.0, 0.0), (0.0, 0.0)];
        let s = smooth_path(&p, 1);
        assert!((s[1].0 - 10.0 / 3.0).abs() < 1e-5);
        assert_eq!(smooth_path(&p, 0), p.to_vec());
        assert!(smooth_path(&[], 3).is_empty());
        assert_eq!(crop_zoom(&[(0.0, 0.0)], 100, 100), 1.0);
        assert!((crop_zoom(&[(5.0, 0.0), (-2.0, 3.0)], 100, 100) - 1.1).abs() < 1e-5);
        // Zoom 1 with a whole-pixel shift moves the content exactly
        let f = view(0, 0);
        let moved = shift_and_zoom(&f, 3.0, 0.0, 1.0);
        assert_eq!(moved.pixel(10, 10), f.pixel(7, 10));
        assert!(stabilize(&[], StabilizeOptions::default()).is_empty());
    }
}
