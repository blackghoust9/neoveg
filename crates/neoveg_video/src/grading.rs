//! Primary color grading, 4-way color wheels, transfer curves, white balance neutral picker,
//! OpenColorIO/ACES pipeline integration, and color match across clips.

pub use neoveg_common::grade::{
    ColorWheelChannel, ColorWheelVector, ColorWheelsGrade, CurvePoints, EventColorGrade, Grade,
    GradeUtilities, HslCurveData, HslCurvesData, OcioSettings, RgbCurvesData, WhiteBalanceGrade,
};
use serde::{Deserialize, Serialize};

pub use crate::ocio::{AcesSpace, AcesTransform, ViewTransform};
use crate::frame::FrameBuffer;
use crate::lut::Lut;

/// Extension trait providing FrameBuffer execution methods for [`ColorWheelsGrade`].
pub trait ColorWheelsExt {
    /// Applies 4-way color wheel adjustments to every pixel of `frame`.
    fn apply_to_frame(&self, frame: &mut FrameBuffer);

    /// Applies range-limited 4-way color wheels to `frame`, isolating shadows to luminance $\le$ `low_thresh`,
    /// highlights to luminance $\ge$ `high_thresh`, and midtones between them.
    fn apply_range_limited(&self, frame: &mut FrameBuffer, low_thresh: f32, high_thresh: f32);
}

impl ColorWheelsExt for ColorWheelsGrade {
    fn apply_to_frame(&self, frame: &mut FrameBuffer) {
        if self.is_neutral() {
            return;
        }
        for px in frame.rgba.as_chunks_mut::<4>().0 {
            let rgb = [
                f32::from(px[0]) / 255.0,
                f32::from(px[1]) / 255.0,
                f32::from(px[2]) / 255.0,
            ];
            let out = self.apply_to_rgb(rgb);
            px[0] = (out[0].clamp(0.0, 1.0) * 255.0).round() as u8;
            px[1] = (out[1].clamp(0.0, 1.0) * 255.0).round() as u8;
            px[2] = (out[2].clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }

    fn apply_range_limited(&self, frame: &mut FrameBuffer, low_thresh: f32, high_thresh: f32) {
        if self.is_neutral() {
            return;
        }
        let low = low_thresh.clamp(0.0, 0.5);
        let high = high_thresh.clamp(0.5, 1.0);

        let smooth_ramp = |a: f32, b: f32, x: f32| -> f32 {
            if (b - a).abs() < 1e-5 {
                if x >= b {
                    1.0
                } else {
                    0.0
                }
            } else {
                let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
                t * t * (3.0 - 2.0 * t)
            }
        };

        for px in frame.rgba.as_chunks_mut::<4>().0 {
            let rgb = [
                f32::from(px[0]) / 255.0,
                f32::from(px[1]) / 255.0,
                f32::from(px[2]) / 255.0,
            ];
            let y = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];

            let w_lift = 1.0 - smooth_ramp(0.0, low, y);
            let w_gain = smooth_ramp(high, 1.0, y);
            let w_gamma = (1.0 - w_lift - w_gain).max(0.0);

            let weighted = ColorWheelsGrade {
                lift: ColorWheelVector {
                    x: self.lift.x * w_lift,
                    y: self.lift.y * w_lift,
                    master: self.lift.master * w_lift,
                },
                gamma: ColorWheelVector {
                    x: self.gamma.x * w_gamma,
                    y: self.gamma.y * w_gamma,
                    master: self.gamma.master * w_gamma,
                },
                gain: ColorWheelVector {
                    x: self.gain.x * w_gain,
                    y: self.gain.y * w_gain,
                    master: self.gain.master * w_gain,
                },
                offset: self.offset,
            };

            let out = weighted.apply_to_rgb(rgb);
            px[0] = (out[0].clamp(0.0, 1.0) * 255.0).round() as u8;
            px[1] = (out[1].clamp(0.0, 1.0) * 255.0).round() as u8;
            px[2] = (out[2].clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }
}

/// Applies 4-way color wheels to `frame`.
pub fn apply_color_wheels(frame: &mut FrameBuffer, wheels: &ColorWheelsGrade) {
    wheels.apply_to_frame(frame);
}

/// Applies range-limited color wheels to `frame`.
pub fn apply_range_limited(
    wheels: &ColorWheelsGrade,
    frame: &mut FrameBuffer,
    low_thresh: f32,
    high_thresh: f32,
) {
    wheels.apply_range_limited(frame, low_thresh, high_thresh);
}

// ----------------------------------------------------------------------------
// White Balance Neutral Picker
// ----------------------------------------------------------------------------

/// Samples neutral RGB tone and computes Temperature and Tint offsets `[-1.0..=1.0]`
/// to neutralize color cast. Guards against dark pixels with $Y \le 10^{-4}$.
pub fn pick_white_balance(sample: [f32; 3]) -> (f32, f32) {
    let [r, g, b] = sample;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    if y <= 1e-4 || !y.is_finite() {
        return (0.0, 0.0);
    }
    let temp = (r - b) / y;
    let tint = (g - (r + b) * 0.5) / y;
    (-temp.clamp(-1.0, 1.0), -tint.clamp(-1.0, 1.0))
}

/// Averages pixels inside `rect` `[x, y, width, height]` of `frame` and computes
/// Temperature and Tint offsets. Clamps rectangle coordinates safely to frame bounds.
pub fn pick_white_balance_from_region(frame: &FrameBuffer, rect: [u32; 4]) -> (f32, f32) {
    let [rx, ry, rw, rh] = rect;
    if frame.width == 0 || frame.height == 0 || rw == 0 || rh == 0 {
        return (0.0, 0.0);
    }
    let x0 = rx.min(frame.width - 1);
    let y0 = ry.min(frame.height - 1);
    let x1 = (rx + rw).min(frame.width);
    let y1 = (ry + rh).min(frame.height);

    let mut sum_r = 0.0f32;
    let mut sum_g = 0.0f32;
    let mut sum_b = 0.0f32;
    let mut count = 0.0f32;

    for y in y0..y1 {
        for x in x0..x1 {
            let px = frame.pixel(x, y);
            sum_r += f32::from(px[0]) / 255.0;
            sum_g += f32::from(px[1]) / 255.0;
            sum_b += f32::from(px[2]) / 255.0;
            count += 1.0;
        }
    }

    if count <= 0.0 {
        return (0.0, 0.0);
    }
    pick_white_balance([sum_r / count, sum_g / count, sum_b / count])
}

/// Calibrates Lift wheel offset from a sampled shadow/black point.
pub fn pick_black_point(sample: [f32; 3]) -> ColorWheelVector {
    let [r, g, b] = sample;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    if y <= 1e-4 || !y.is_finite() {
        return ColorWheelVector::neutral();
    }
    let delta_r = -(r - y);
    let delta_g = -(g - y);
    let delta_b = -(b - y);
    ColorWheelVector::from_rgb_sliders(delta_r, delta_g, delta_b, -y)
}

/// Calibrates Gain wheel offset from a sampled highlight/white point.
pub fn pick_white_point(sample: [f32; 3]) -> ColorWheelVector {
    let [r, g, b] = sample;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    if y <= 1e-4 || !y.is_finite() {
        return ColorWheelVector::neutral();
    }
    let master_gain = (1.0 / y) - 1.0;
    let delta_r = -(r / y - 1.0);
    let delta_g = -(g / y - 1.0);
    let delta_b = -(b / y - 1.0);
    ColorWheelVector::from_rgb_sliders(delta_r, delta_g, delta_b, master_gain.clamp(-1.0, 1.0))
}

// ----------------------------------------------------------------------------
// Monotone Cubic Hermite Spline & RGB/HSL Curves
// ----------------------------------------------------------------------------

/// A smooth curve through arbitrary control points ($N \ge 2$) using the Fritsch-Carlson
/// monotone cubic Hermite algorithm. Preserves monotonicity and eliminates overshoot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SplineCurve {
    points: Vec<[f32; 2]>,
    #[serde(skip)]
    m: Vec<f32>,
}

impl Default for SplineCurve {
    fn default() -> Self {
        Self::linear()
    }
}

impl SplineCurve {
    pub fn new(raw_points: &[[f32; 2]]) -> Self {
        let mut pts: Vec<[f32; 2]> = raw_points.to_vec();
        pts.sort_by(|a, b| a[0].total_cmp(&b[0]));
        let mut points: Vec<[f32; 2]> = Vec::with_capacity(pts.len());
        for p in pts {
            if let Some(last) = points.last_mut() {
                if (p[0] - last[0]).abs() < 1e-4 {
                    last[1] = p[1];
                    continue;
                }
            }
            points.push(p);
        }
        let m = Self::compute_slopes(&points);
        Self { points, m }
    }

    pub fn linear() -> Self {
        Self::new(&[[0.0, 0.0], [1.0, 1.0]])
    }

    pub fn points(&self) -> &[[f32; 2]] {
        &self.points
    }

    fn compute_slopes(points: &[[f32; 2]]) -> Vec<f32> {
        let n = points.len();
        if n < 2 {
            return vec![0.0; n];
        }
        let mut h = Vec::with_capacity(n - 1);
        let mut d = Vec::with_capacity(n - 1);
        for w in points.windows(2) {
            let dx = (w[1][0] - w[0][0]).max(1e-6);
            let dy = w[1][1] - w[0][1];
            h.push(dx);
            d.push(dy / dx);
        }
        let mut m = vec![0.0; n];
        if n == 2 {
            m[0] = d[0];
            m[1] = d[0];
            return m;
        }
        for k in 1..n - 1 {
            if d[k - 1] * d[k] > 0.0 {
                let w1 = 2.0 * h[k] + h[k - 1];
                let w2 = h[k] + 2.0 * h[k - 1];
                m[k] = (w1 + w2) / (w1 / d[k - 1] + w2 / d[k]);
            }
        }
        m[0] = Self::end_slope(h[0], h[1], d[0], d[1]);
        m[n - 1] = Self::end_slope(h[n - 2], h[n - 3], d[n - 2], d[n - 3]);
        m
    }

    fn end_slope(h0: f32, h1: f32, d0: f32, d1: f32) -> f32 {
        let m = ((2.0 * h0 + h1) * d0 - h0 * d1) / (h0 + h1);
        if m * d0 <= 0.0 {
            0.0
        } else if d0 * d1 <= 0.0 && m.abs() > 3.0 * d0.abs() {
            3.0 * d0
        } else {
            m
        }
    }

    pub fn eval(&self, v: f32) -> f32 {
        let n = self.points.len();
        if n == 0 {
            return v;
        }
        if n == 1 {
            return self.points[0][1];
        }
        if v <= self.points[0][0] {
            return self.points[0][1];
        }
        if v >= self.points[n - 1][0] {
            return self.points[n - 1][1];
        }
        let k = self.points.partition_point(|p| p[0] <= v) - 1;
        let h = self.points[k + 1][0] - self.points[k][0];
        let t = (v - self.points[k][0]) / h;
        let t2 = t * t;
        let t3 = t2 * t;
        let m = if self.m.len() == n {
            &self.m
        } else {
            return self.points[k][1] * (1.0 - t) + self.points[k + 1][1] * t;
        };
        let out = (2.0 * t3 - 3.0 * t2 + 1.0) * self.points[k][1]
            + (t3 - 2.0 * t2 + t) * h * m[k]
            + (-2.0 * t3 + 3.0 * t2) * self.points[k + 1][1]
            + (t3 - t2) * h * m[k + 1];
        out.clamp(0.0, 1.0)
    }

    pub fn to_lut(&self, steps: usize) -> Vec<f32> {
        let steps = steps.max(2);
        (0..steps)
            .map(|i| self.eval(i as f32 / (steps - 1) as f32))
            .collect()
    }
}

/// Standard 4-channel RGB Curves (Master, Red, Green, Blue).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RgbCurves {
    pub master: SplineCurve,
    pub red: SplineCurve,
    pub green: SplineCurve,
    pub blue: SplineCurve,
}

impl Default for RgbCurves {
    fn default() -> Self {
        Self {
            master: SplineCurve::linear(),
            red: SplineCurve::linear(),
            green: SplineCurve::linear(),
            blue: SplineCurve::linear(),
        }
    }
}

impl RgbCurves {
    /// Generates pre-evaluated 256-step L1-cached lookup tables for Red, Green, Blue.
    pub fn generate_luts_256(&self) -> [[u8; 256]; 3] {
        let mut luts = [[0u8; 256]; 3];
        for i in 0..256 {
            let v = i as f32 / 255.0;
            let m = self.master.eval(v);
            luts[0][i] = (self.red.eval(m) * 255.0).round().clamp(0.0, 255.0) as u8;
            luts[1][i] = (self.green.eval(m) * 255.0).round().clamp(0.0, 255.0) as u8;
            luts[2][i] = (self.blue.eval(m) * 255.0).round().clamp(0.0, 255.0) as u8;
        }
        luts
    }
}

/// Evaluates RGB curves on every pixel of `frame` via fast 256-step lookup tables.
pub fn apply_rgb_curves(frame: &mut FrameBuffer, curves: &RgbCurves) {
    let luts = curves.generate_luts_256();
    for px in frame.rgba.as_chunks_mut::<4>().0 {
        px[0] = luts[0][px[0] as usize];
        px[1] = luts[1][px[1] as usize];
        px[2] = luts[2][px[2] as usize];
    }
}

/// The 6 HSL secondary curve types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HslCurveType {
    /// Shifts hue based on targeted input hue. Periodic wrapping at $0^\circ \equiv 360^\circ$.
    HueVsHue,
    /// Modulates saturation based on targeted input hue.
    HueVsSat,
    /// Modulates luminance based on targeted input hue.
    HueVsLum,
    /// Modulates saturation based on input luminance.
    LumVsSat,
    /// Direct remapping of saturation.
    SatVsSat,
    /// Modulates luminance based on input saturation.
    SatVsLum,
}

/// One secondary HSL curve definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HslCurve {
    pub curve_type: HslCurveType,
    pub spline: SplineCurve,
}

impl HslCurve {
    pub fn new(curve_type: HslCurveType, points: &[[f32; 2]]) -> Self {
        Self {
            curve_type,
            spline: SplineCurve::new(points),
        }
    }

    pub fn default_for(curve_type: HslCurveType) -> Self {
        match curve_type {
            HslCurveType::HueVsHue | HslCurveType::SatVsSat => {
                Self::new(curve_type, &[[0.0, 0.0], [1.0, 1.0]])
            }
            HslCurveType::HueVsSat
            | HslCurveType::HueVsLum
            | HslCurveType::LumVsSat
            | HslCurveType::SatVsLum => {
                Self::new(curve_type, &[[0.0, 0.5], [1.0, 0.5]])
            }
        }
    }

    /// Evaluates curve at normalized position `x \in [0.0..=1.0]`.
    /// `HueVsHue` wraps periodically so $0.0 \equiv 1.0$ ($0^\circ \equiv 360^\circ$).
    pub fn eval(&self, x: f32) -> f32 {
        if self.curve_type == HslCurveType::HueVsHue {
            let x_mod = x.rem_euclid(1.0);
            self.spline.eval(x_mod)
        } else {
            self.spline.eval(x.clamp(0.0, 1.0))
        }
    }
}

/// Container for the 6 HSL secondary color correction curves.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HslCurves {
    pub curves: Vec<HslCurve>,
}

impl Default for HslCurves {
    fn default() -> Self {
        Self {
            curves: vec![
                HslCurve::default_for(HslCurveType::HueVsHue),
                HslCurve::default_for(HslCurveType::HueVsSat),
                HslCurve::default_for(HslCurveType::HueVsLum),
                HslCurve::default_for(HslCurveType::LumVsSat),
                HslCurve::default_for(HslCurveType::SatVsSat),
                HslCurve::default_for(HslCurveType::SatVsLum),
            ],
        }
    }
}

#[inline]
pub fn rgb_to_hsl(c: [f32; 3]) -> [f32; 3] {
    let mx = c[0].max(c[1]).max(c[2]);
    let mn = c[0].min(c[1]).min(c[2]);
    let l = (mx + mn) / 2.0;
    let d = mx - mn;
    if d < 1e-6 {
        return [0.0, 0.0, l];
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs()).max(1e-6);
    let h = if mx == c[0] {
        ((c[1] - c[2]) / d).rem_euclid(6.0)
    } else if mx == c[1] {
        (c[2] - c[0]) / d + 2.0
    } else {
        (c[0] - c[1]) / d + 4.0
    };
    [h * 60.0, s.clamp(0.0, 1.0), l.clamp(0.0, 1.0)]
}

#[inline]
pub fn hsl_to_rgb(hsl: [f32; 3]) -> [f32; 3] {
    let (h, s, l) = (
        hsl[0].rem_euclid(360.0),
        hsl[1].clamp(0.0, 1.0),
        hsl[2].clamp(0.0, 1.0),
    );
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - (hp.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    [
        (r + m).clamp(0.0, 1.0),
        (g + m).clamp(0.0, 1.0),
        (b + m).clamp(0.0, 1.0),
    ]
}

/// Applies all 6 HSL secondary curves to `frame`.
pub fn apply_hsl_curves(frame: &mut FrameBuffer, curves: &HslCurves) {
    for px in frame.rgba.as_chunks_mut::<4>().0 {
        let rgb = [
            f32::from(px[0]) / 255.0,
            f32::from(px[1]) / 255.0,
            f32::from(px[2]) / 255.0,
        ];
        let mut hsl = rgb_to_hsl(rgb);
        for c in &curves.curves {
            match c.curve_type {
                HslCurveType::HueVsHue => {
                    let out = c.eval(hsl[0] / 360.0);
                    hsl[0] = (out * 360.0).rem_euclid(360.0);
                }
                HslCurveType::HueVsSat => {
                    let mult = 2.0 * c.eval(hsl[0] / 360.0);
                    hsl[1] = (hsl[1] * mult).clamp(0.0, 1.0);
                }
                HslCurveType::HueVsLum => {
                    let delta = c.eval(hsl[0] / 360.0) - 0.5;
                    hsl[2] = (hsl[2] + delta).clamp(0.0, 1.0);
                }
                HslCurveType::LumVsSat => {
                    let mult = 2.0 * c.eval(hsl[2]);
                    hsl[1] = (hsl[1] * mult).clamp(0.0, 1.0);
                }
                HslCurveType::SatVsSat => {
                    hsl[1] = c.eval(hsl[1]).clamp(0.0, 1.0);
                }
                HslCurveType::SatVsLum => {
                    let delta = c.eval(hsl[1]) - 0.5;
                    hsl[2] = (hsl[2] + delta).clamp(0.0, 1.0);
                }
            }
        }
        let out_rgb = hsl_to_rgb(hsl);
        px[0] = (out_rgb[0] * 255.0).round() as u8;
        px[1] = (out_rgb[1] * 255.0).round() as u8;
        px[2] = (out_rgb[2] * 255.0).round() as u8;
    }
}

// ----------------------------------------------------------------------------
// Color Match Across Clips
// ----------------------------------------------------------------------------

/// Trait for analyzing and matching color balance between reference and target video frames.
pub trait ColorMatcher: Send + Sync {
    fn analyze_and_match(&self, reference: &FrameBuffer, target: &FrameBuffer) -> ColorWheelsGrade;
}

/// Statistical color matcher calculating channel mean and variance distribution.
pub struct StatisticalColorMatcher;

impl ColorMatcher for StatisticalColorMatcher {
    fn analyze_and_match(&self, reference: &FrameBuffer, target: &FrameBuffer) -> ColorWheelsGrade {
        if target.width == 0 || target.height == 0 || reference.width == 0 || reference.height == 0 {
            return ColorWheelsGrade::default_neutral();
        }

        let stats = |fb: &FrameBuffer| -> ([f32; 3], [f32; 3]) {
            let mut sum = [0.0f32; 3];
            let mut sum_sq = [0.0f32; 3];
            let n = (fb.width * fb.height) as f32;
            for px in fb.rgba.as_chunks::<4>().0 {
                for c in 0..3 {
                    let v = f32::from(px[c]) / 255.0;
                    sum[c] += v;
                    sum_sq[c] += v * v;
                }
            }
            let mean = [sum[0] / n, sum[1] / n, sum[2] / n];
            let std = [
                ((sum_sq[0] / n - mean[0] * mean[0]).max(1e-6)).sqrt(),
                ((sum_sq[1] / n - mean[1] * mean[1]).max(1e-6)).sqrt(),
                ((sum_sq[2] / n - mean[2] * mean[2]).max(1e-6)).sqrt(),
            ];
            (mean, std)
        };

        let (ref_mean, ref_std) = stats(reference);
        let (tgt_mean, tgt_std) = stats(target);

        let delta_mean = [
            ref_mean[0] - tgt_mean[0],
            ref_mean[1] - tgt_mean[1],
            ref_mean[2] - tgt_mean[2],
        ];
        let offset = ColorWheelVector::from_rgb_sliders(
            delta_mean[0] * 0.5,
            delta_mean[1] * 0.5,
            delta_mean[2] * 0.5,
            (delta_mean[0] + delta_mean[1] + delta_mean[2]) / 6.0,
        );

        let gain_scale = [
            (ref_std[0] / tgt_std[0].max(1e-4) - 1.0).clamp(-0.8, 2.0),
            (ref_std[1] / tgt_std[1].max(1e-4) - 1.0).clamp(-0.8, 2.0),
            (ref_std[2] / tgt_std[2].max(1e-4) - 1.0).clamp(-0.8, 2.0),
        ];
        let gain = ColorWheelVector::from_rgb_sliders(
            gain_scale[0] * 0.5,
            gain_scale[1] * 0.5,
            gain_scale[2] * 0.5,
            (gain_scale[0] + gain_scale[1] + gain_scale[2]) / 3.0,
        );

        ColorWheelsGrade {
            lift: ColorWheelVector::neutral(),
            gamma: ColorWheelVector::neutral(),
            gain,
            offset,
        }
    }
}

/// Transfers color distribution from `reference` frame to `target` frame.
/// When `match_brightness` is false, preserves target frame's original luminance.
/// `strength` in `[0.0..=1.0]` blends linearly between original and matched frame.
pub fn match_color(
    target: &mut FrameBuffer,
    reference: &FrameBuffer,
    match_brightness: bool,
    strength: f32,
) {
    let k = if strength.is_finite() {
        strength.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if k <= 0.0 || target.width == 0 || target.height == 0 || reference.width == 0 || reference.height == 0 {
        return;
    }

    let mean_rgb = |fb: &FrameBuffer| -> [f32; 3] {
        let mut sum = [0.0f32; 3];
        let count = (fb.width * fb.height) as f32;
        for px in fb.rgba.as_chunks::<4>().0 {
            sum[0] += f32::from(px[0]);
            sum[1] += f32::from(px[1]);
            sum[2] += f32::from(px[2]);
        }
        [sum[0] / count, sum[1] / count, sum[2] / count]
    };

    let ref_mean = mean_rgb(reference);
    let tgt_mean = mean_rgb(target);
    let scale = [
        (ref_mean[0] / tgt_mean[0].max(1.0)).clamp(0.1, 10.0),
        (ref_mean[1] / tgt_mean[1].max(1.0)).clamp(0.1, 10.0),
        (ref_mean[2] / tgt_mean[2].max(1.0)).clamp(0.1, 10.0),
    ];

    for px in target.rgba.as_chunks_mut::<4>().0 {
        let orig = [f32::from(px[0]), f32::from(px[1]), f32::from(px[2])];
        let mut matched = [
            (orig[0] * scale[0]).clamp(0.0, 255.0),
            (orig[1] * scale[1]).clamp(0.0, 255.0),
            (orig[2] * scale[2]).clamp(0.0, 255.0),
        ];

        if !match_brightness {
            let y_orig = 0.2126 * orig[0] + 0.7152 * orig[1] + 0.0722 * orig[2];
            let y_matched = (0.2126 * matched[0] + 0.7152 * matched[1] + 0.0722 * matched[2]).max(1e-4);
            let luma_ratio = y_orig / y_matched;
            matched[0] = (matched[0] * luma_ratio).clamp(0.0, 255.0);
            matched[1] = (matched[1] * luma_ratio).clamp(0.0, 255.0);
            matched[2] = (matched[2] * luma_ratio).clamp(0.0, 255.0);
        }

        for c in 0..3 {
            let blended = orig[c] * (1.0 - k) + matched[c] * k;
            px[c] = blended.round().clamp(0.0, 255.0) as u8;
        }
    }
}

/// Backwards-compatible statistical color match.
pub fn match_color_statistics(target: &mut FrameBuffer, reference: &FrameBuffer) {
    match_color(target, reference, true, 1.0);
}

// ----------------------------------------------------------------------------
// Per-Event Color Grading Integration
// ----------------------------------------------------------------------------

/// Applies complete event-level color grading pipeline to `frame`.
pub fn apply_event_grade(frame: &mut FrameBuffer, grade: &EventColorGrade) {
    if grade.is_neutral() {
        return;
    }

    // 1. OCIO / ACES Transform if enabled
    if let Some(ocio) = &grade.ocio {
        if ocio.enabled {
            let input = match ocio.input_space.as_str() {
                "SLog1" => AcesSpace::SLog1,
                "SLog2" => AcesSpace::SLog2,
                "SLog3" => AcesSpace::SLog3,
                "Aces2065_1" => AcesSpace::Aces2065_1,
                "AcesCc" => AcesSpace::AcesCc,
                "AcesCct" => AcesSpace::AcesCct,
                "AcesProxy" => AcesSpace::AcesProxy,
                "Srgb" => AcesSpace::Srgb,
                _ => AcesSpace::Rec709,
            };
            let working = match ocio.working_space.as_str() {
                "AcesCc" => AcesSpace::AcesCc,
                "AcesCct" => AcesSpace::AcesCct,
                "Aces2065_1" => AcesSpace::Aces2065_1,
                _ => AcesSpace::AcesCg,
            };
            let view = match ocio.view_transform.as_str() {
                "Srgb" => ViewTransform::AcesRrtSrgb,
                "Rec2020" => ViewTransform::AcesRrtRec2020,
                "Hlg" => ViewTransform::AcesRrtHlg,
                "Raw" => ViewTransform::Raw,
                _ => ViewTransform::AcesRrtRec709,
            };
            let tf = AcesTransform::new(input, working, view);
            tf.apply_to_frame(frame);
        }
    }

    // 2. White Balance & Utilities & Wheels
    if !grade.white_balance.is_neutral()
        || !grade.utilities.is_neutral()
        || !grade.wheels.is_neutral()
    {
        for px in frame.rgba.as_chunks_mut::<4>().0 {
            let rgb = [
                f32::from(px[0]) / 255.0,
                f32::from(px[1]) / 255.0,
                f32::from(px[2]) / 255.0,
            ];
            let out = grade.apply_to_rgb(rgb);
            px[0] = (out[0].clamp(0.0, 1.0) * 255.0).round() as u8;
            px[1] = (out[1].clamp(0.0, 1.0) * 255.0).round() as u8;
            px[2] = (out[2].clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }

    // 3. RGB Curves
    if let Some(rgb_data) = &grade.rgb_curves {
        let curves = RgbCurves {
            master: SplineCurve::new(&rgb_data.master.points),
            red: SplineCurve::new(&rgb_data.red.points),
            green: SplineCurve::new(&rgb_data.green.points),
            blue: SplineCurve::new(&rgb_data.blue.points),
        };
        apply_rgb_curves(frame, &curves);
    }

    // 4. HSL Curves
    if let Some(hsl_data) = &grade.hsl_curves {
        let mut curves_vec = Vec::new();
        for entry in &hsl_data.curves {
            let curve_type = match entry.curve_type {
                0 => HslCurveType::HueVsHue,
                1 => HslCurveType::HueVsSat,
                2 => HslCurveType::HueVsLum,
                3 => HslCurveType::LumVsSat,
                4 => HslCurveType::SatVsSat,
                _ => HslCurveType::SatVsLum,
            };
            curves_vec.push(HslCurve::new(curve_type, &entry.points));
        }
        let curves = HslCurves { curves: curves_vec };
        apply_hsl_curves(frame, &curves);
    }
}

// ----------------------------------------------------------------------------
// Existing Grade & Look Functions
// ----------------------------------------------------------------------------

/// Corrects every pixel of `frame` (alpha is left alone). A neutral grade is a no-op.
pub fn apply_grade(frame: &mut FrameBuffer, grade: &Grade) {
    if grade.is_neutral() {
        return;
    }
    let grade = grade.sanitized();
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

    #[test]
    fn color_wheels_ext_apply_to_frame_and_range_limited() {
        let mut frame = FrameBuffer::solid(4, 4, [100, 100, 100, 255]);
        let mut wheels = ColorWheelsGrade::default_neutral();
        wheels.offset.master = 0.1;
        wheels.apply_to_frame(&mut frame);
        let p = frame.pixel(0, 0);
        assert!(p[0] > 100);

        // Range limited
        let mut frame2 = FrameBuffer::solid(4, 4, [20, 20, 20, 255]);
        let mut wheels2 = ColorWheelsGrade::default_neutral();
        wheels2.lift.master = 0.1;
        wheels2.gain.master = 0.5;
        wheels2.apply_range_limited(&mut frame2, 0.25, 0.75);
        let p2 = frame2.pixel(0, 0);
        assert!(p2[0] > 20);
    }

    #[test]
    fn white_balance_pickers() {
        let frame = FrameBuffer::solid(8, 8, [200, 150, 100, 255]);
        let (temp, _tint) = pick_white_balance_from_region(&frame, [2, 2, 4, 4]);
        assert!(temp < 0.0, "Warm sample should produce cooling compensation");

        let bp = pick_black_point([0.05, 0.05, 0.06]);
        assert!(bp.master < 0.0);

        let wp = pick_white_point([0.8, 0.8, 0.8]);
        assert!(wp.master > 0.0);
    }

    #[test]
    fn spline_curve_and_rgb_curves() {
        let curve = SplineCurve::new(&[[0.0, 0.0], [0.5, 0.8], [1.0, 1.0]]);
        assert_eq!(curve.eval(0.0), 0.0);
        assert_eq!(curve.eval(1.0), 1.0);
        assert!((curve.eval(0.5) - 0.8).abs() < 1e-3);

        let rgb = RgbCurves::default();
        let mut frame = FrameBuffer::solid(2, 2, [128, 128, 128, 255]);
        apply_rgb_curves(&mut frame, &rgb);
        let p = frame.pixel(0, 0);
        assert!((p[0] as i32 - 128).abs() <= 1);
    }

    #[test]
    fn hsl_curves_application() {
        let curves = HslCurves::default();
        let mut frame = FrameBuffer::solid(2, 2, [150, 80, 40, 255]);
        let original = frame.clone();
        apply_hsl_curves(&mut frame, &curves);
        assert_eq!(frame.pixel(0, 0), original.pixel(0, 0));
    }

    #[test]
    fn color_matcher_statistical() {
        let matcher = StatisticalColorMatcher;
        let reference = FrameBuffer::solid(4, 4, [180, 120, 60, 255]);
        let mut target = FrameBuffer::solid(4, 4, [100, 100, 100, 255]);
        let wheels = matcher.analyze_and_match(&reference, &target);
        assert!(!wheels.is_neutral());

        match_color(&mut target, &reference, true, 1.0);
        assert_eq!(target.pixel(0, 0), [180, 120, 60, 255]);
    }

    #[test]
    fn apply_event_grade_neutral_and_custom() {
        let grade = EventColorGrade::default();
        let mut frame = FrameBuffer::solid(2, 2, [100, 100, 100, 255]);
        let original = frame.clone();
        apply_event_grade(&mut frame, &grade);
        assert_eq!(frame, original);

        let mut custom = EventColorGrade::default();
        custom.utilities.brightness = 0.1;
        apply_event_grade(&mut frame, &custom);
        assert!(frame.pixel(0, 0)[0] > 100);
    }
}
