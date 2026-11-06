//! Automatic color: white balance and levels worked out from the picture itself, returned as an
//! ordinary [`EventColorGrade`] so the user can keep editing it in the color corrector.
//!
//! White balance follows the gray-world assumption (the average of the scene should be neutral);
//! levels stretch the 1st to 99th luma percentile to the full range. Both are a starting point:
//! a scene dominated by one color will fool the first, which is why the result is editable.

use neoveg_common::grade::{EventColorGrade, WhiteBalanceGrade};

use crate::frame::FrameBuffer;

/// What the analysis found in a frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameAnalysis {
    /// Average of the red, green and blue channels, `0..=1`.
    pub mean: [f32; 3],
    /// Luma at the 1st percentile after white balance, `0..=1`.
    pub black: f32,
    /// Luma at the 99th percentile after white balance, `0..=1`.
    pub white: f32,
    /// The white balance that makes the average neutral.
    pub white_balance: WhiteBalanceGrade,
}

/// Which parts of the automatic correction to compute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoColorOptions {
    pub white_balance: bool,
    pub levels: bool,
}

impl Default for AutoColorOptions {
    fn default() -> Self {
        Self {
            white_balance: true,
            levels: true,
        }
    }
}

fn luma(rgb: [f32; 3]) -> f32 {
    0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]
}

/// Temperature and tint (as [`WhiteBalanceGrade::apply`] defines them) that make the three channel
/// means equal. Closed form: red / blue are matched by the temperature, green by the tint.
pub fn neutralizing_white_balance(mean: [f32; 3]) -> WhiteBalanceGrade {
    let [r, g, b] = mean;
    if !(r.is_finite() && g.is_finite() && b.is_finite()) || r <= 1e-4 || b <= 1e-4 || g <= 1e-4 {
        return WhiteBalanceGrade::neutral();
    }
    // (1 + t/2) r = (1 - t/2) b  =>  t = 2 (k - 1) / (k + 1), k = b / r
    let k = b / r;
    let t = (2.0 * (k - 1.0) / (k + 1.0)).clamp(-1.0, 1.0);
    let v = r * (1.0 + 0.5 * t);
    // (1 + s/2) g = (1 - s/2) v  =>  s = 2 (v - g) / (v + g)
    let s = (2.0 * (v - g) / (v + g)).clamp(-1.0, 1.0);
    WhiteBalanceGrade {
        temperature: t,
        tint: s,
    }
}

/// Analyses `frame`, sampling at most about 65 000 of its opaque pixels. `None` for an empty or
/// fully transparent frame.
pub fn analyze_frame(frame: &FrameBuffer, options: AutoColorOptions) -> Option<FrameAnalysis> {
    let total = frame.width as usize * frame.height as usize;
    if total == 0 || frame.rgba.len() < total * 4 {
        return None;
    }
    let step = (total / 65_536).max(1);
    let mut sum = [0.0f64; 3];
    let mut n = 0usize;
    for px in frame.rgba.as_chunks::<4>().0.iter().step_by(step) {
        if px[3] == 0 {
            continue;
        }
        for c in 0..3 {
            sum[c] += f64::from(px[c]) / 255.0;
        }
        n += 1;
    }
    if n == 0 {
        return None;
    }
    let mean = [
        (sum[0] / n as f64) as f32,
        (sum[1] / n as f64) as f32,
        (sum[2] / n as f64) as f32,
    ];
    let white_balance = if options.white_balance {
        neutralizing_white_balance(mean)
    } else {
        WhiteBalanceGrade::neutral()
    };
    // Histogram of the luma after the white balance
    let mut hist = [0u32; 256];
    let mut counted = 0u32;
    for px in frame.rgba.as_chunks::<4>().0.iter().step_by(step) {
        if px[3] == 0 {
            continue;
        }
        let rgb = white_balance.apply([
            f32::from(px[0]) / 255.0,
            f32::from(px[1]) / 255.0,
            f32::from(px[2]) / 255.0,
        ]);
        let y = luma(rgb).clamp(0.0, 1.0);
        hist[(y * 255.0).round() as usize] += 1;
        counted += 1;
    }
    let percentile = |p: f32| -> f32 {
        let target = (counted as f32 * p).ceil().max(1.0) as u32;
        let mut acc = 0u32;
        for (i, &c) in hist.iter().enumerate() {
            acc += c;
            if acc >= target {
                return i as f32 / 255.0;
            }
        }
        1.0
    };
    Some(FrameAnalysis {
        mean,
        black: percentile(0.01),
        white: percentile(0.99),
        white_balance,
    })
}

/// The automatic correction of `frame`: white balance in the grade's white balance, levels in the
/// offset (black point) and gain (white point) master of the wheels. `None` when the frame has
/// nothing to analyse; an already well-exposed neutral frame yields a neutral grade.
pub fn auto_color_grade(frame: &FrameBuffer, options: AutoColorOptions) -> Option<EventColorGrade> {
    let a = analyze_frame(frame, options)?;
    let mut grade = EventColorGrade::default();
    // Changes smaller than this are noise of the estimate
    if a.white_balance.temperature.abs() > 0.004 || a.white_balance.tint.abs() > 0.004 {
        grade.white_balance = a.white_balance;
    }
    if options.levels && a.white - a.black > 0.05 {
        // Offset then gain: (x + offset) * (1 + gain) maps black to 0 and white to 1.
        let offset = if a.black > 0.004 { -a.black } else { 0.0 };
        let range = a.white - a.black;
        let gain = (1.0 / range - 1.0).clamp(0.0, 1.0);
        if offset != 0.0 {
            grade.wheels.offset.master = offset;
        }
        if gain > 0.004 {
            grade.wheels.gain.master = gain;
        }
    }
    Some(grade)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A horizontal ramp from `lo` to `hi` (per channel), multiplied by a color cast.
    fn ramp(lo: f32, hi: f32, cast: [f32; 3]) -> FrameBuffer {
        let (w, h) = (64u32, 16u32);
        let mut f = FrameBuffer::solid(w, h, [0, 0, 0, 255]);
        for y in 0..h {
            for x in 0..w {
                let t = lo + (hi - lo) * x as f32 / (w - 1) as f32;
                let i = ((y * w + x) * 4) as usize;
                for (c, &weight) in cast.iter().enumerate() {
                    f.rgba[i + c] = (t * weight * 255.0).clamp(0.0, 255.0).round() as u8;
                }
            }
        }
        f
    }

    fn mean_of(f: &FrameBuffer) -> [f32; 3] {
        let mut s = [0.0f32; 3];
        let n = (f.rgba.len() / 4) as f32;
        for px in f.rgba.as_chunks::<4>().0 {
            for (c, &val) in px[..3].iter().enumerate() {
                s[c] += f32::from(val);
            }
        }
        [s[0] / n, s[1] / n, s[2] / n]
    }

    #[test]
    fn a_warm_frame_is_neutralized_by_the_automatic_white_balance() {
        let mut f = ramp(0.0, 1.0, [1.0, 0.85, 0.6]);
        let before = mean_of(&f);
        assert!(before[0] - before[2] > 30.0);
        let grade = auto_color_grade(
            &f,
            AutoColorOptions {
                white_balance: true,
                levels: false,
            },
        )
        .unwrap();
        // A warm cast is cooled: negative temperature
        assert!(grade.white_balance.temperature < -0.05, "{grade:?}");
        crate::grading::apply_event_grade(&mut f, &grade);
        let after = mean_of(&f);
        assert!((after[0] - after[2]).abs() < 6.0, "{before:?} -> {after:?}");
        assert!((after[1] - after[2]).abs() < 8.0, "{before:?} -> {after:?}");
    }

    #[test]
    fn a_flat_low_contrast_frame_gets_its_levels_stretched() {
        let mut f = ramp(0.25, 0.75, [1.0, 1.0, 1.0]);
        let grade = auto_color_grade(
            &f,
            AutoColorOptions {
                white_balance: false,
                levels: true,
            },
        )
        .unwrap();
        assert!(grade.wheels.offset.master < -0.15, "{grade:?}");
        assert!(grade.wheels.gain.master > 0.5, "{grade:?}");
        crate::grading::apply_event_grade(&mut f, &grade);
        let first = f.pixel(0, 0)[0];
        let last = f.pixel(63, 0)[0];
        assert!(first < 25, "black point reaches the bottom: {first}");
        assert!(last > 235, "white point reaches the top: {last}");
    }

    #[test]
    fn a_neutral_well_exposed_frame_is_left_alone_and_empty_input_yields_nothing() {
        let f = ramp(0.0, 1.0, [1.0, 1.0, 1.0]);
        let grade = auto_color_grade(&f, AutoColorOptions::default()).unwrap();
        assert!(grade.is_neutral(), "{grade:?}");
        assert!(auto_color_grade(
            &FrameBuffer::solid(0, 0, [0; 4]),
            AutoColorOptions::default()
        )
        .is_none());
        assert!(auto_color_grade(
            &FrameBuffer::solid(4, 4, [9, 9, 9, 0]),
            AutoColorOptions::default()
        )
        .is_none());
    }
}
