//! The coverage of the mask of an effect: for every pixel of a frame, how much of the effect shows
//! (`1` inside the shape, `0` outside, in between along a soft edge). The description of the mask is
//! `neoveg_common::fx_mask`; the shape is filled with the anti-aliased rasteriser of the text
//! engine and the feather is a blur of that fill.

use std::sync::{Arc, Mutex, OnceLock};

use neoveg_common::fx_mask::{bezier_segments, FxMask, MaskShape};
use neoveg_text::mask::blur;
use neoveg_text::raster::{fill, Path};

/// Points of the polygon an ellipse is drawn with.
const ELLIPSE_STEPS: usize = 96;

/// The outline of `mask` on a `w` by `h` frame, in pixels.
fn outline(mask: &FxMask, w: f32, h: f32) -> Path {
    let (cx, cy) = (mask.center[0] * w, mask.center[1] * h);
    let (hw, hh) = (mask.size[0] * w * 0.5, mask.size[1] * h * 0.5);
    let (sin, cos) = mask.rotation_deg.to_radians().sin_cos();
    // A point of the shape in its own frame, turned and put at the centre
    let place = |x: f32, y: f32| (cx + x * cos - y * sin, cy + x * sin + y * cos);
    match mask.shape {
        MaskShape::Rectangle => Path::polygon(&[
            place(-hw, -hh),
            place(hw, -hh),
            place(hw, hh),
            place(-hw, hh),
        ]),
        MaskShape::Ellipse => {
            let points: Vec<(f32, f32)> = (0..ELLIPSE_STEPS)
                .map(|i| {
                    let a = i as f32 / ELLIPSE_STEPS as f32 * std::f32::consts::TAU;
                    place(hw * a.cos(), hh * a.sin())
                })
                .collect();
            Path::polygon(&points)
        }
        MaskShape::Bezier => {
            let segments = bezier_segments(&mask.points);
            let mut path = Path::new();
            for (i, s) in segments.iter().enumerate() {
                let at = |p: [f32; 2]| (p[0] * w, p[1] * h);
                if i == 0 {
                    path.move_to(at(s[0]).0, at(s[0]).1);
                }
                let (c1, c2, end) = (at(s[1]), at(s[2]), at(s[3]));
                path.cubic_to(c1.0, c1.1, c2.0, c2.1, end.0, end.1);
            }
            if !segments.is_empty() {
                path.close();
            }
            path
        }
    }
}

fn compute(mask: &FxMask, w: u32, h: u32) -> Vec<f32> {
    let mask = mask.sanitized();
    let (wf, hf) = (w as f32, h as f32);
    let filled = fill(&outline(&mask, wf, hf), w as usize, h as usize, 0.0, 0.0);
    // The feather is the extent of the soft edge; a blur of half of it grows the edge to about that
    let sigma = mask.feather * hf * 0.5;
    let soft = blur(&filled, sigma);
    let mut coverage = soft.a;
    for v in &mut coverage {
        *v = v.clamp(0.0, 1.0);
        if mask.invert {
            *v = 1.0 - *v;
        }
    }
    coverage
}

type Cached = (u64, u32, u32, Arc<Vec<f32>>);

/// The coverage of `mask` for a `w` by `h` frame, one value per pixel in rows from the top. The
/// last one is kept: a frame of a movie asks for the same mask again and again.
pub fn coverage(mask: &FxMask, w: u32, h: u32) -> Arc<Vec<f32>> {
    static LAST: OnceLock<Mutex<Option<Cached>>> = OnceLock::new();
    let key = {
        use std::hash::Hasher;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        mask.hash_into(&mut hasher);
        hasher.finish()
    };
    let cell = LAST.get_or_init(Default::default);
    if let Ok(guard) = cell.lock() {
        if let Some((k, cw, ch, data)) = guard.as_ref() {
            if (*k, *cw, *ch) == (key, w, h) {
                return Arc::clone(data);
            }
        }
    }
    let data = Arc::new(compute(mask, w, h));
    if let Ok(mut guard) = cell.lock() {
        *guard = Some((key, w, h, Arc::clone(&data)));
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(c: &[f32], w: u32, x: u32, y: u32) -> f32 {
        c[(y * w + x) as usize]
    }

    #[test]
    fn a_rectangle_covers_its_middle_and_nothing_outside() {
        let mask = FxMask::new(MaskShape::Rectangle); // half of the frame in the middle
        let c = coverage(&mask, 100, 80);
        assert_eq!(at(&c, 100, 50, 40), 1.0);
        assert_eq!(at(&c, 100, 2, 2), 0.0);
        assert_eq!(at(&c, 100, 97, 77), 0.0);
        let area: f32 = c.iter().sum();
        assert!((area - 100.0 * 80.0 / 4.0).abs() < 60.0, "{area}");
    }

    #[test]
    fn an_ellipse_leaves_the_corners_of_its_box_out_and_the_negative_mode_swaps_the_sides() {
        let mut mask = FxMask::new(MaskShape::Ellipse);
        mask.size = [0.8, 0.8];
        let c = coverage(&mask, 100, 100);
        assert_eq!(at(&c, 100, 50, 50), 1.0);
        assert_eq!(
            at(&c, 100, 12, 12),
            0.0,
            "the corner of the box is outside the ellipse"
        );
        let ellipse: f32 = c.iter().sum();
        let expected = std::f32::consts::PI * 40.0 * 40.0;
        assert!(
            (ellipse - expected).abs() < 80.0,
            "{ellipse} against {expected}"
        );
        mask.invert = true;
        let inverted = coverage(&mask, 100, 100);
        assert_eq!(at(&inverted, 100, 50, 50), 0.0);
        assert_eq!(at(&inverted, 100, 12, 12), 1.0);
        let total: f32 = inverted.iter().sum::<f32>() + ellipse;
        assert!(
            (total - 10_000.0).abs() < 1.0,
            "the two sides add up to the frame: {total}"
        );
    }

    #[test]
    fn the_feather_makes_a_soft_edge_and_a_rotation_turns_the_box() {
        let mut mask = FxMask::new(MaskShape::Rectangle);
        let hard = coverage(&mask, 120, 120);
        mask.feather = 0.1;
        let soft = coverage(&mask, 120, 120);
        let in_between = |c: &[f32]| c.iter().filter(|v| **v > 0.05 && **v < 0.95).count();
        assert!(
            in_between(&soft) > in_between(&hard) * 4,
            "a soft edge has many partial pixels"
        );
        // A thin bar turned a quarter is a tall bar
        let mut bar = FxMask::new(MaskShape::Rectangle);
        bar.size = [0.8, 0.1];
        let flat = coverage(&bar, 100, 100);
        assert_eq!(at(&flat, 100, 20, 50), 1.0);
        assert_eq!(at(&flat, 100, 50, 20), 0.0);
        bar.rotation_deg = 90.0;
        let tall = coverage(&bar, 100, 100);
        assert_eq!(at(&tall, 100, 50, 20), 1.0);
        assert_eq!(at(&tall, 100, 20, 50), 0.0);
    }

    #[test]
    fn a_bezier_mask_follows_its_points_and_the_same_mask_is_computed_once() {
        let mut mask = FxMask::new(MaskShape::Bezier);
        mask.points = vec![[0.2, 0.2], [0.8, 0.2], [0.8, 0.8], [0.2, 0.8]];
        let c = coverage(&mask, 100, 100);
        assert_eq!(at(&c, 100, 50, 50), 1.0);
        assert_eq!(at(&c, 100, 5, 5), 0.0);
        // The curve passes through the four corners of a 60 px square and bulges out between them, so
        // it covers more than the square (3600) and less than the circle through the corners (5655)
        let area: f32 = c.iter().sum();
        assert!(area > 3600.0 && area < 5655.0, "{area}");
        let again = coverage(&mask, 100, 100);
        assert!(Arc::ptr_eq(&c, &again), "the last mask is kept");
        mask.points[0][0] = 0.3;
        assert!(!Arc::ptr_eq(&c, &coverage(&mask, 100, 100)));
    }
}
