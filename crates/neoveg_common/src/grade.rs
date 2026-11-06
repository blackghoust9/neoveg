//! Primary color correction of the video: exposure, color temperature and tint, lift, contrast,
//! gamma and saturation. Pure per-color math shared by the project file, the UI and the video
//! worker.

use serde::{Deserialize, Serialize};

/// The controls of the correction; every field is neutral at its default.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Grade {
    /// Exposure in stops (`2^x`), `-4.0..=4.0`.
    pub exposure: f32,
    /// Color temperature, cool (`-1`) to warm (`1`).
    pub temperature: f32,
    /// Tint, green (`-1`) to magenta (`1`).
    pub tint: f32,
    /// Raises (or lowers) the blacks, `-0.5..=0.5`.
    pub lift: f32,
    /// Contrast around mid grey, `-1.0..=1.0`.
    pub contrast: f32,
    /// Mid-tone gamma, `0.2..=3.0` (1 = none; above 1 brightens the mids).
    pub gamma: f32,
    /// Saturation, `-1.0` (grey) to `1.0` (twice as colorful).
    pub saturation: f32,
}

impl Default for Grade {
    fn default() -> Self {
        Self {
            exposure: 0.0,
            temperature: 0.0,
            tint: 0.0,
            lift: 0.0,
            contrast: 0.0,
            gamma: 1.0,
            saturation: 0.0,
        }
    }
}

/// Rec. 709 luma weights.
fn luma([r, g, b]: [f32; 3]) -> f32 {
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

impl Grade {
    pub const EXPOSURE_RANGE: (f32, f32) = (-4.0, 4.0);
    pub const LIFT_RANGE: (f32, f32) = (-0.5, 0.5);
    pub const GAMMA_RANGE: (f32, f32) = (0.2, 3.0);

    pub fn is_neutral(&self) -> bool {
        *self == Self::default()
    }

    /// Every control clamped to its range; NaN becomes neutral.
    pub fn sanitized(self) -> Self {
        let clamp = |v: f32, lo: f32, hi: f32, neutral: f32| {
            if v.is_finite() {
                v.clamp(lo, hi)
            } else {
                neutral
            }
        };
        Self {
            exposure: clamp(
                self.exposure,
                Self::EXPOSURE_RANGE.0,
                Self::EXPOSURE_RANGE.1,
                0.0,
            ),
            temperature: clamp(self.temperature, -1.0, 1.0, 0.0),
            tint: clamp(self.tint, -1.0, 1.0, 0.0),
            lift: clamp(self.lift, Self::LIFT_RANGE.0, Self::LIFT_RANGE.1, 0.0),
            contrast: clamp(self.contrast, -1.0, 1.0, 0.0),
            gamma: clamp(self.gamma, Self::GAMMA_RANGE.0, Self::GAMMA_RANGE.1, 1.0),
            saturation: clamp(self.saturation, -1.0, 1.0, 0.0),
        }
    }

    /// Corrects one color (components `0.0..=1.0`); the result is clamped to the same range.
    pub fn apply(&self, rgb: [f32; 3]) -> [f32; 3] {
        let gain = 2f32.powf(self.exposure);
        let warm = [
            1.0 + 0.25 * self.temperature,
            1.0 - 0.25 * self.tint,
            1.0 - 0.25 * self.temperature,
        ];
        let inv_gamma = 1.0 / self.gamma.max(0.01);
        let mut out = [0.0f32; 3];
        for c in 0..3 {
            let mut v = rgb[c] * gain * warm[c];
            v += self.lift * (1.0 - v.clamp(0.0, 1.0));
            v = (v - 0.5) * (1.0 + self.contrast) + 0.5;
            out[c] = v.clamp(0.0, 1.0).powf(inv_gamma);
        }
        let y = luma(out);
        let k = 1.0 + self.saturation;
        for v in &mut out {
            *v = (y + (*v - y) * k).clamp(0.0, 1.0);
        }
        out
    }
}

/// Color channels adjustable via keyboard or sliders on a color wheel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ColorWheelChannel {
    Red,
    Green,
    Blue,
    Cyan,
    Magenta,
    Yellow,
    Master,
}

/// A 2D chromatic displacement vector and master luma offset for one color wheel.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorWheelVector {
    /// Chroma X in `[-1.0..=1.0]`. Aligned with the Red/Cyan axis.
    pub x: f32,
    /// Chroma Y in `[-1.0..=1.0]`. Aligned with Green/Magenta perpendicular axis.
    pub y: f32,
    /// Master luma adjustment in `[-1.0..=1.0]`.
    pub master: f32,
}

impl Default for ColorWheelVector {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            master: 0.0,
        }
    }
}

impl ColorWheelVector {
    pub const fn neutral() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            master: 0.0,
        }
    }

    pub fn is_neutral(&self) -> bool {
        self.x == 0.0 && self.y == 0.0 && self.master == 0.0
    }

    /// Creates a wheel vector from polar coordinates: hue in degrees `[0.0..360.0)`,
    /// saturation radius in `[0.0..=1.0]`, and master luma in `[-1.0..=1.0]`.
    pub fn from_polar(hue_deg: f32, sat: f32, master: f32) -> Self {
        let hue_deg = if hue_deg.is_finite() { hue_deg } else { 0.0 };
        let rad = hue_deg.to_radians();
        let sat = if sat.is_finite() { sat.clamp(0.0, 1.0) } else { 0.0 };
        let master = if master.is_finite() { master.clamp(-1.0, 1.0) } else { 0.0 };
        Self {
            x: sat * rad.cos(),
            y: sat * rad.sin(),
            master,
        }
    }

    /// Converts the vector to polar coordinates: `(hue_deg, sat, master)`.
    /// `hue_deg` is in `[0.0..360.0)`.
    pub fn to_polar(&self) -> (f32, f32, f32) {
        let sat = (self.x * self.x + self.y * self.y).sqrt().clamp(0.0, 1.0);
        let mut deg = self.y.atan2(self.x).to_degrees();
        if !deg.is_finite() {
            deg = 0.0;
        }
        if deg < 0.0 {
            deg += 360.0;
        }
        if deg >= 360.0 {
            deg -= 360.0;
        }
        (deg, sat, self.master)
    }

    /// Creates a wheel vector from RGB chromatic offsets preserving luminance:
    /// `r + g + b = 0`.
    pub fn from_rgb_sliders(r: f32, g: f32, b: f32, master: f32) -> Self {
        let r = if r.is_finite() { r } else { 0.0 };
        let g = if g.is_finite() { g } else { 0.0 };
        let b = if b.is_finite() { b } else { 0.0 };
        let avg = (r + g + b) / 3.0;
        let r0 = r - avg;
        let g0 = g - avg;
        let b0 = b - avg;
        let x = r0;
        let y = (g0 - b0) / 3.0f32.sqrt();
        let master = if master.is_finite() { master.clamp(-1.0, 1.0) } else { 0.0 };
        Self {
            x: x.clamp(-1.0, 1.0),
            y: y.clamp(-1.0, 1.0),
            master,
        }
    }

    /// Converts to zero-sum RGB chromatic offsets and master luma:
    /// `(delta_r, delta_g, delta_b, master)`.
    pub fn to_rgb_sliders(&self) -> (f32, f32, f32, f32) {
        let sqrt3_over_2 = 3.0f32.sqrt() * 0.5;
        let r = self.x;
        let g = -0.5 * self.x + sqrt3_over_2 * self.y;
        let b = -0.5 * self.x - sqrt3_over_2 * self.y;
        (r, g, b, self.master)
    }

    /// Adjusts a channel by `delta`, matching numpad controls.
    pub fn adjust_channel(&mut self, channel: ColorWheelChannel, delta: f32) {
        let sqrt3_over_2 = 3.0f32.sqrt() * 0.5;
        match channel {
            ColorWheelChannel::Red => self.x = (self.x + delta).clamp(-1.0, 1.0),
            ColorWheelChannel::Green => {
                self.x = (self.x - 0.5 * delta).clamp(-1.0, 1.0);
                self.y = (self.y + sqrt3_over_2 * delta).clamp(-1.0, 1.0);
            }
            ColorWheelChannel::Blue => {
                self.x = (self.x - 0.5 * delta).clamp(-1.0, 1.0);
                self.y = (self.y - sqrt3_over_2 * delta).clamp(-1.0, 1.0);
            }
            ColorWheelChannel::Cyan => self.x = (self.x - delta).clamp(-1.0, 1.0),
            ColorWheelChannel::Magenta => {
                self.x = (self.x + 0.5 * delta).clamp(-1.0, 1.0);
                self.y = (self.y - sqrt3_over_2 * delta).clamp(-1.0, 1.0);
            }
            ColorWheelChannel::Yellow => {
                self.x = (self.x + 0.5 * delta).clamp(-1.0, 1.0);
                self.y = (self.y + sqrt3_over_2 * delta).clamp(-1.0, 1.0);
            }
            ColorWheelChannel::Master => self.master = (self.master + delta).clamp(-1.0, 1.0),
        }
    }

    pub fn adjust_red(&mut self, delta: f32) {
        self.adjust_channel(ColorWheelChannel::Red, delta);
    }

    pub fn adjust_green(&mut self, delta: f32) {
        self.adjust_channel(ColorWheelChannel::Green, delta);
    }

    pub fn adjust_blue(&mut self, delta: f32) {
        self.adjust_channel(ColorWheelChannel::Blue, delta);
    }

    pub fn adjust_cyan(&mut self, delta: f32) {
        self.adjust_channel(ColorWheelChannel::Cyan, delta);
    }

    pub fn adjust_magenta(&mut self, delta: f32) {
        self.adjust_channel(ColorWheelChannel::Magenta, delta);
    }

    pub fn adjust_yellow(&mut self, delta: f32) {
        self.adjust_channel(ColorWheelChannel::Yellow, delta);
    }

    pub fn adjust_luma(&mut self, delta: f32) {
        self.adjust_channel(ColorWheelChannel::Master, delta);
    }

    pub fn reset(&mut self) {
        *self = Self::neutral();
    }

    pub fn sanitized(self) -> Self {
        let sanitize = |v: f32| if v.is_finite() { v.clamp(-1.0, 1.0) } else { 0.0 };
        Self {
            x: sanitize(self.x),
            y: sanitize(self.y),
            master: sanitize(self.master),
        }
    }
}

/// Primary 4-way color wheels: Lift (shadows), Gamma (midtones), Gain (highlights), Offset (global).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorWheelsGrade {
    pub lift: ColorWheelVector,
    pub gamma: ColorWheelVector,
    pub gain: ColorWheelVector,
    pub offset: ColorWheelVector,
}

impl Default for ColorWheelsGrade {
    fn default() -> Self {
        Self::default_neutral()
    }
}

impl ColorWheelsGrade {
    pub const fn default_neutral() -> Self {
        Self {
            lift: ColorWheelVector::neutral(),
            gamma: ColorWheelVector::neutral(),
            gain: ColorWheelVector::neutral(),
            offset: ColorWheelVector::neutral(),
        }
    }

    pub fn is_neutral(&self) -> bool {
        self.lift.is_neutral()
            && self.gamma.is_neutral()
            && self.gain.is_neutral()
            && self.offset.is_neutral()
    }

    pub fn sanitized(self) -> Self {
        Self {
            lift: self.lift.sanitized(),
            gamma: self.gamma.sanitized(),
            gain: self.gain.sanitized(),
            offset: self.offset.sanitized(),
        }
    }

    /// Evaluates 4-way grading on a single normalized RGB pixel `[0.0..=1.0]` (or float `> 1.0`).
    pub fn apply_to_rgb(&self, rgb: [f32; 3]) -> [f32; 3] {
        let mut out = rgb;
        let (or, og, ob, _) = self.offset.to_rgb_sliders();
        let (gr, gg, gb, _) = self.gain.to_rgb_sliders();
        let (lr, lg, lb, _) = self.lift.to_rgb_sliders();
        let (mr, mg, mb, _) = self.gamma.to_rgb_sliders();

        let off_m = if self.offset.master.is_finite() { self.offset.master } else { 0.0 };
        let gain_m = if self.gain.master.is_finite() { self.gain.master } else { 0.0 };
        let lift_m = if self.lift.master.is_finite() { self.lift.master } else { 0.0 };
        let gamma_m = if self.gamma.master.is_finite() { self.gamma.master } else { 0.0 };

        // 1. Offset (additive global pedestal)
        out[0] += off_m + or;
        out[1] += off_m + og;
        out[2] += off_m + ob;

        // 2. Gain (multiplicative highlight scale)
        out[0] *= 1.0 + gain_m + gr;
        out[1] *= 1.0 + gain_m + gg;
        out[2] *= 1.0 + gain_m + gb;

        // 3. Lift (shadow pedestal, tapers linearly to 0 at 1.0)
        out[0] += (lift_m + lr) * (1.0 - out[0].clamp(0.0, 1.0));
        out[1] += (lift_m + lg) * (1.0 - out[1].clamp(0.0, 1.0));
        out[2] += (lift_m + lb) * (1.0 - out[2].clamp(0.0, 1.0));

        // 4. Gamma (midtones power)
        let g0 = (1.0 - gamma_m - mr).clamp(0.2, 5.0);
        let g1 = (1.0 - gamma_m - mg).clamp(0.2, 5.0);
        let g2 = (1.0 - gamma_m - mb).clamp(0.2, 5.0);
        out[0] = out[0].clamp(0.0, 10.0).powf(g0);
        out[1] = out[1].clamp(0.0, 10.0).powf(g1);
        out[2] = out[2].clamp(0.0, 10.0).powf(g2);

        out
    }
}

/// White balance adjustment: temperature (cool/warm) and tint (magenta/green).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WhiteBalanceGrade {
    /// Temperature offset in `[-1.0..=1.0]`. `-1.0` cools (adds blue), `+1.0` warms (adds red/orange).
    pub temperature: f32,
    /// Tint offset in `[-1.0..=1.0]`. `-1.0` adds magenta, `+1.0` adds green.
    pub tint: f32,
}

impl Default for WhiteBalanceGrade {
    fn default() -> Self {
        Self {
            temperature: 0.0,
            tint: 0.0,
        }
    }
}

impl WhiteBalanceGrade {
    pub const fn neutral() -> Self {
        Self {
            temperature: 0.0,
            tint: 0.0,
        }
    }

    pub fn is_neutral(&self) -> bool {
        self.temperature == 0.0 && self.tint == 0.0
    }

    pub fn sanitized(self) -> Self {
        let clamp = |v: f32| if v.is_finite() { v.clamp(-1.0, 1.0) } else { 0.0 };
        Self {
            temperature: clamp(self.temperature),
            tint: clamp(self.tint),
        }
    }

    pub fn apply(&self, rgb: [f32; 3]) -> [f32; 3] {
        let temp = if self.temperature.is_finite() { self.temperature.clamp(-1.0, 1.0) } else { 0.0 };
        let tint = if self.tint.is_finite() { self.tint.clamp(-1.0, 1.0) } else { 0.0 };
        let warm = 1.0 + 0.5 * temp;
        let cool = 1.0 - 0.5 * temp;
        let green = 1.0 + 0.5 * tint;
        let magenta = 1.0 - 0.5 * tint;
        [
            (rgb[0] * warm * magenta).clamp(0.0, 10.0),
            (rgb[1] * green).clamp(0.0, 10.0),
            (rgb[2] * cool * magenta).clamp(0.0, 10.0),
        ]
    }
}

/// Utility grading controls: exposure, brightness, contrast, contrast center, saturation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GradeUtilities {
    pub exposure: f32,
    pub brightness: f32,
    pub contrast: f32,
    pub contrast_center: f32,
    pub saturation: f32,
}

impl Default for GradeUtilities {
    fn default() -> Self {
        Self {
            exposure: 0.0,
            brightness: 0.0,
            contrast: 0.0,
            contrast_center: 0.5,
            saturation: 0.0,
        }
    }
}

impl GradeUtilities {
    pub const fn neutral() -> Self {
        Self {
            exposure: 0.0,
            brightness: 0.0,
            contrast: 0.0,
            contrast_center: 0.5,
            saturation: 0.0,
        }
    }

    pub fn is_neutral(&self) -> bool {
        self.exposure == 0.0
            && self.brightness == 0.0
            && self.contrast == 0.0
            && (self.contrast_center - 0.5).abs() < 1e-4
            && self.saturation == 0.0
    }

    pub fn sanitized(self) -> Self {
        let clamp = |v: f32, lo: f32, hi: f32, neutral: f32| {
            if v.is_finite() { v.clamp(lo, hi) } else { neutral }
        };
        Self {
            exposure: clamp(self.exposure, -4.0, 4.0, 0.0),
            brightness: clamp(self.brightness, -1.0, 1.0, 0.0),
            contrast: clamp(self.contrast, -1.0, 1.0, 0.0),
            contrast_center: clamp(self.contrast_center, 0.0, 1.0, 0.5),
            saturation: clamp(self.saturation, -1.0, 1.0, 0.0),
        }
    }

    pub fn apply(&self, rgb: [f32; 3]) -> [f32; 3] {
        let gain = 2f32.powf(self.exposure);
        let center = self.contrast_center;
        let mut out = [0.0f32; 3];
        for c in 0..3 {
            let mut v = rgb[c] * gain + self.brightness;
            v = (v - center) * (1.0 + self.contrast) + center;
            out[c] = v.clamp(0.0, 10.0);
        }
        if self.saturation != 0.0 {
            let y = luma(out);
            let k = (1.0 + self.saturation).max(0.0);
            for v in &mut out {
                *v = (y + (*v - y) * k).clamp(0.0, 10.0);
            }
        }
        out
    }
}

/// A list of 2D control points `[[x0, y0], [x1, y1], ...]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurvePoints {
    pub points: Vec<[f32; 2]>,
}

impl Default for CurvePoints {
    fn default() -> Self {
        Self {
            points: vec![[0.0, 0.0], [1.0, 1.0]],
        }
    }
}

/// Control points for the 4 RGB curves: Master, Red, Green, Blue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RgbCurvesData {
    pub master: CurvePoints,
    pub red: CurvePoints,
    pub green: CurvePoints,
    pub blue: CurvePoints,
}

/// One HSL curve data entry: curve type index and control points.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HslCurveData {
    pub curve_type: u8,
    pub points: Vec<[f32; 2]>,
}

/// Container for the 6 HSL curve definitions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct HslCurvesData {
    pub curves: Vec<HslCurveData>,
}

/// OpenColorIO / ACES project/event configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OcioSettings {
    pub enabled: bool,
    pub input_space: String,
    pub working_space: String,
    pub view_transform: String,
}

impl Default for OcioSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            input_space: "Rec709".into(),
            working_space: "AcesCg".into(),
            view_transform: "Rec709".into(),
        }
    }
}

/// Full color grade for a timeline event, incorporating color wheels, white balance,
/// utilities, curves, LUT references, and OCIO settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct EventColorGrade {
    pub wheels: ColorWheelsGrade,
    pub white_balance: WhiteBalanceGrade,
    pub utilities: GradeUtilities,
    pub rgb_curves: Option<RgbCurvesData>,
    pub hsl_curves: Option<HslCurvesData>,
    pub input_lut: Option<String>,
    pub look_lut: Option<String>,
    pub lut_strength: f32,
    pub ocio: Option<OcioSettings>,
}

impl EventColorGrade {
    pub fn is_neutral(&self) -> bool {
        self.wheels.is_neutral()
            && self.white_balance.is_neutral()
            && self.utilities.is_neutral()
            && self.rgb_curves.is_none()
            && self.hsl_curves.is_none()
            && self.input_lut.is_none()
            && self.look_lut.is_none()
            && self.ocio.as_ref().is_none_or(|o| !o.enabled)
    }

    /// Evaluates basic inline math of the grade (white balance, utilities, color wheels).
    pub fn apply_to_rgb(&self, rgb: [f32; 3]) -> [f32; 3] {
        let rgb = self.white_balance.apply(rgb);
        let rgb = self.utilities.apply(rgb);
        self.wheels.apply_to_rgb(rgb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-4)
    }

    #[test]
    fn the_default_grade_changes_nothing() {
        let g = Grade::default();
        assert!(g.is_neutral());
        for rgb in [
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.2, 0.5, 0.8],
            [0.9, 0.1, 0.4],
        ] {
            assert!(close(g.apply(rgb), rgb), "{rgb:?}");
        }
    }

    #[test]
    fn exposure_doubles_per_stop_and_clips_at_white() {
        let g = Grade {
            exposure: 1.0,
            ..Grade::default()
        };
        assert!(close(g.apply([0.2, 0.25, 0.4]), [0.4, 0.5, 0.8]));
        assert!(close(g.apply([0.8, 0.8, 0.8]), [1.0, 1.0, 1.0]));
        let dark = Grade {
            exposure: -1.0,
            ..Grade::default()
        };
        assert!(close(dark.apply([0.8, 0.4, 0.2]), [0.4, 0.2, 0.1]));
    }

    #[test]
    fn saturation_minus_one_is_grey_and_plus_one_doubles_the_chroma() {
        let grey = Grade {
            saturation: -1.0,
            ..Grade::default()
        }
        .apply([0.8, 0.2, 0.2]);
        assert!((grey[0] - grey[1]).abs() < 1e-5 && (grey[1] - grey[2]).abs() < 1e-5);
        let more = Grade {
            saturation: 0.5,
            ..Grade::default()
        }
        .apply([0.6, 0.4, 0.4]);
        assert!(more[0] - more[1] > 0.29, "{more:?}");
    }

    #[test]
    fn contrast_spreads_around_mid_grey_and_gamma_lifts_the_mids() {
        let c = Grade {
            contrast: 0.5,
            ..Grade::default()
        };
        assert!(close(c.apply([0.5, 0.5, 0.5]), [0.5, 0.5, 0.5]));
        assert!(c.apply([0.7, 0.7, 0.7])[0] > 0.7 && c.apply([0.3, 0.3, 0.3])[0] < 0.3);
        let g = Grade {
            gamma: 2.0,
            ..Grade::default()
        };
        assert!(close(g.apply([0.25, 0.25, 0.25]), [0.5, 0.5, 0.5]));
        assert!(close(g.apply([0.0, 0.0, 0.0]), [0.0, 0.0, 0.0]));
        assert!(close(g.apply([1.0, 1.0, 1.0]), [1.0, 1.0, 1.0]));
    }

    #[test]
    fn lift_raises_only_the_blacks_and_temperature_warms() {
        let l = Grade {
            lift: 0.2,
            ..Grade::default()
        };
        assert!(close(l.apply([0.0, 0.0, 0.0]), [0.2, 0.2, 0.2]));
        assert!(close(l.apply([1.0, 1.0, 1.0]), [1.0, 1.0, 1.0]));
        let warm = Grade {
            temperature: 1.0,
            ..Grade::default()
        }
        .apply([0.5, 0.5, 0.5]);
        assert!(warm[0] > 0.5 && warm[2] < 0.5);
        let magenta = Grade {
            tint: 1.0,
            ..Grade::default()
        }
        .apply([0.5, 0.5, 0.5]);
        assert!(magenta[1] < 0.5);
    }

    #[test]
    fn bad_values_are_clamped_and_old_files_load() {
        let g = Grade {
            exposure: 99.0,
            gamma: f32::NAN,
            saturation: -7.0,
            ..Grade::default()
        }
        .sanitized();
        assert_eq!((g.exposure, g.gamma, g.saturation), (4.0, 1.0, -1.0));
        let old: Grade = serde_json::from_str("{\"exposure\":0.5}").unwrap();
        assert_eq!(old.exposure, 0.5);
        assert_eq!(old.gamma, 1.0);
        let none: Grade = serde_json::from_str("{}").unwrap();
        assert!(none.is_neutral());
        let json = serde_json::to_string(&g).unwrap();
        assert_eq!(serde_json::from_str::<Grade>(&json).unwrap(), g);
    }

    #[test]
    fn color_wheel_vector_polar_and_rgb_roundtrip() {
        let v = ColorWheelVector::from_polar(60.0, 0.5, 0.2);
        let (deg, sat, master) = v.to_polar();
        assert!((deg - 60.0).abs() < 1e-4);
        assert!((sat - 0.5).abs() < 1e-4);
        assert!((master - 0.2).abs() < 1e-4);

        let (r, g, b, m) = v.to_rgb_sliders();
        assert!((r + g + b).abs() < 1e-5, "RGB sliders must sum to zero");
        assert_eq!(m, 0.2);

        let v2 = ColorWheelVector::from_rgb_sliders(r, g, b, m);
        assert!((v2.x - v.x).abs() < 1e-5);
        assert!((v2.y - v.y).abs() < 1e-5);
        assert_eq!(v2.master, v.master);
    }

    #[test]
    fn color_wheel_vector_numpad_adjustments() {
        let mut v = ColorWheelVector::neutral();
        v.adjust_red(0.1);
        assert!((v.x - 0.1).abs() < 1e-5);
        assert_eq!(v.y, 0.0);

        v.reset();
        assert!(v.is_neutral());
        v.adjust_cyan(0.1);
        assert!((v.x - (-0.1)).abs() < 1e-5);

        v.reset();
        v.adjust_luma(0.2);
        assert_eq!(v.master, 0.2);
    }

    #[test]
    fn color_wheels_grade_neutral_and_eval() {
        let wheels = ColorWheelsGrade::default_neutral();
        assert!(wheels.is_neutral());
        let res = wheels.apply_to_rgb([0.3, 0.5, 0.7]);
        assert!(close(res, [0.3, 0.5, 0.7]));

        let mut tinted = ColorWheelsGrade::default_neutral();
        tinted.offset.master = 0.1;
        let res2 = tinted.apply_to_rgb([0.1, 0.1, 0.1]);
        assert!(close(res2, [0.2, 0.2, 0.2]));
    }

    #[test]
    fn event_color_grade_serialization() {
        let mut grade = EventColorGrade::default();
        assert!(grade.is_neutral());
        grade.wheels.gain.master = 0.5;
        grade.white_balance.temperature = -0.2;
        let json = serde_json::to_string(&grade).unwrap();
        let deserialized: EventColorGrade = serde_json::from_str(&json).unwrap();
        assert_eq!(grade, deserialized);
    }
}
