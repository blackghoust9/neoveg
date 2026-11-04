//! Applies the primary color correction ([`Grade`]) and the project's LUT to a frame, in that order.

use neoveg_common::grade::Grade;

use crate::frame::FrameBuffer;
use crate::lut::Lut;

/// Corrects every pixel of `frame` (alpha is left alone). A neutral grade is a no-op.
pub fn apply_grade(frame: &mut FrameBuffer, grade: &Grade) {
    if grade.is_neutral() {
        return;
    }
    let grade = grade.sanitized();
    // Pixels repeat a lot in real pictures; remembering the last one skips most of the math on flat
    // areas without changing the result.
    let mut last_in = [u8::MAX; 3];
    let mut last_out = [0u8; 3];
    let mut have_last = false;
    for px in frame.rgba.as_chunks_mut::<4>().0 {
        let src = [px[0], px[1], px[2]];
        if !(have_last && src == last_in) {
            let rgb = grade.apply([
                f32::from(src[0]) / 255.0,
                f32::from(src[1]) / 255.0,
                f32::from(src[2]) / 255.0,
            ]);
            last_out = [
                (rgb[0] * 255.0).round() as u8,
                (rgb[1] * 255.0).round() as u8,
                (rgb[2] * 255.0).round() as u8,
            ];
            last_in = src;
            have_last = true;
        }
        px[..3].copy_from_slice(&last_out);
    }
}

/// Fades the layer toward `color`: every pixel's color moves `amount` (`0.0..=1.0`) of the way to
/// the color, the alpha is left alone. `0.0` is a no-op and `1.0` paints the layer's visible pixels
/// solid in the color (the transparent ones stay transparent, so lower tracks still show there).
pub fn fade_to_color(frame: &mut FrameBuffer, color: [u8; 3], amount: f32) {
    let amount = if amount.is_finite() {
        amount.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if amount <= 0.0 {
        return;
    }
    let keep = 1.0 - amount;
    for px in frame.rgba.as_chunks_mut::<4>().0.iter_mut() {
        for c in 0..3 {
            px[c] = (f32::from(px[c]) * keep + f32::from(color[c]) * amount).round() as u8;
        }
    }
}

/// The whole look: the primary correction, then the LUT (either may be absent).
pub fn apply_look(frame: &mut FrameBuffer, grade: &Grade, lut: Option<&Lut>) {
    apply_grade(frame, grade);
    if let Some(lut) = lut {
        lut.apply_to_frame(frame, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generators::smpte_color_bars;

    #[test]
    fn a_neutral_grade_leaves_the_frame_untouched() {
        let original = smpte_color_bars(70, 8);
        let mut frame = original.clone();
        apply_grade(&mut frame, &Grade::default());
        assert_eq!(frame, original);
    }

    #[test]
    fn exposure_brightens_the_pixels_and_keeps_alpha() {
        let mut frame = FrameBuffer::solid(4, 4, [50, 100, 120, 77]);
        apply_grade(
            &mut frame,
            &Grade {
                exposure: 1.0,
                ..Grade::default()
            },
        );
        assert_eq!(frame.pixel(0, 0), [100, 200, 240, 77]);
        assert_eq!(frame.pixel(3, 3), [100, 200, 240, 77]);
    }

    #[test]
    fn the_cache_of_the_last_pixel_does_not_change_the_result() {
        let grade = Grade {
            saturation: -0.5,
            contrast: 0.3,
            ..Grade::default()
        };
        let mut bars = smpte_color_bars(140, 4);
        let mut by_pixel = bars.clone();
        apply_grade(&mut bars, &grade);
        for px in by_pixel.rgba.as_chunks_mut::<4>().0 {
            let out = grade.apply([
                f32::from(px[0]) / 255.0,
                f32::from(px[1]) / 255.0,
                f32::from(px[2]) / 255.0,
            ]);
            for c in 0..3 {
                px[c] = (out[c] * 255.0).round() as u8;
            }
        }
        assert_eq!(bars, by_pixel);
    }

    #[test]
    fn the_look_applies_the_grade_before_the_lut() {
        // An inverting LUT after a darkening grade: 128 -> exposure -1 -> 64 -> inverted 191
        let mut text = String::from("LUT_3D_SIZE 2\n");
        for b in 0..2 {
            for g in 0..2 {
                for r in 0..2 {
                    text.push_str(&format!("{} {} {}\n", 1 - r, 1 - g, 1 - b));
                }
            }
        }
        let lut = Lut::parse_cube(&text).unwrap();
        let mut frame = FrameBuffer::solid(1, 1, [128, 128, 128, 255]);
        apply_look(
            &mut frame,
            &Grade {
                exposure: -1.0,
                ..Grade::default()
            },
            Some(&lut),
        );
        let [r, ..] = frame.pixel(0, 0);
        assert!((i32::from(r) - 191).abs() <= 1, "{r}");
        let mut plain = FrameBuffer::solid(1, 1, [10, 20, 30, 255]);
        apply_look(&mut plain, &Grade::default(), None);
        assert_eq!(plain.pixel(0, 0), [10, 20, 30, 255]);
    }

    #[test]
    fn fading_to_a_color_moves_the_rgb_and_keeps_the_alpha() {
        let original = FrameBuffer::solid(4, 4, [200, 100, 0, 128]);
        let mut frame = original.clone();
        fade_to_color(&mut frame, [0, 0, 0], 0.0);
        assert_eq!(frame, original, "zero changes nothing");
        fade_to_color(&mut frame, [0, 0, 100], 0.5);
        assert_eq!(frame.pixel(1, 1), [100, 50, 50, 128]);
        fade_to_color(&mut frame, [255, 255, 255], 1.0);
        assert_eq!(
            frame.pixel(0, 0),
            [255, 255, 255, 128],
            "fully faded, alpha untouched"
        );
        fade_to_color(&mut frame, [0, 0, 0], f32::NAN);
        assert_eq!(
            frame.pixel(0, 0),
            [255, 255, 255, 128],
            "a bad amount is ignored"
        );
    }
}
