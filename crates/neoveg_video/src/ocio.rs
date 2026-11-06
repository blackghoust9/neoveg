//! Pure CPU clean-room OpenColorIO and ACES color transformation pipeline.
//!
//! Provides color space conversions across standard Academy Color Encoding (ACES)
//! spaces (ACES2065-1 AP0, ACEScg AP1, ACEScc, ACEScct, ACESproxy), broadcast spaces
//! (Rec.709, sRGB), and camera logarithmic formats (Camera S-Log1, S-Log2, S-Log3).
//!
//! Includes an analytical CPU tone mapping fit (Narkowicz / Stephen Hill fit) for ACES RRT + ODT
//! highlight rolloff without external C++ or GPU dependencies.

use serde::{Deserialize, Serialize};

use crate::frame::FrameBuffer;
use crate::lut::Lut;

/// Standard ACES and camera logarithmic color spaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AcesSpace {
    /// SMPTE ST 2065-1 linear AP0 interchange and archival space.
    Aces2065_1,
    /// AP1 scene-linear compositing and computer graphics space.
    AcesCg,
    /// AP1 logarithmic color grading working space.
    AcesCc,
    /// AP1 logarithmic color grading working space with linear toe.
    AcesCct,
    /// AP1 integer logarithmic encoding for on-set monitoring and preview.
    AcesProxy,
    /// Digital cinema camera logarithmic curve S-Log1.
    SLog1,
    /// Digital cinema camera logarithmic curve S-Log2.
    SLog2,
    /// Digital cinema camera logarithmic curve S-Log3.
    SLog3,
    /// ITU-R BT.709 display color space with standard transfer function.
    Rec709,
    /// IEC 61966-2-1 sRGB display color space.
    Srgb,
}

/// ACES Reference Rendering Transform (RRT) and Output Device Transform (ODT).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ViewTransform {
    /// ACES 1.0 RRT + Rec.709 display ODT with filmic highlight compression.
    AcesRrtRec709,
    /// ACES 1.0 RRT + sRGB display ODT.
    AcesRrtSrgb,
    /// ACES 1.0 RRT + Rec.2020 wide color gamut display ODT.
    AcesRrtRec2020,
    /// ACES 1.0 RRT + Hybrid Log-Gamma (HLG) HDR display ODT.
    AcesRrtHlg,
    /// Bypass tone curve and output device transform (unmodified working space).
    Raw,
}

// ----------------------------------------------------------------------------
// Conversion Matrices (3x3 row-major)
// ----------------------------------------------------------------------------

/// ACES AP0 to AP1 gamut conversion matrix (D60 chromatic adaptation).
pub const M_AP0_TO_AP1: [[f32; 3]; 3] = [
    [1.4514393, -0.2365107, -0.2149286],
    [-0.0765538, 1.1762297, -0.0996759],
    [0.0083161, -0.0060324, 0.9977163],
];

/// ACES AP1 to AP0 gamut conversion matrix.
pub const M_AP1_TO_AP0: [[f32; 3]; 3] = [
    [0.6954522, 0.1402839, 0.1642638],
    [0.0447946, 0.8596711, 0.0955343],
    [-0.0055259, 0.0040252, 1.0015007],
];

/// ACES AP1 linear to Rec.709 (D65 white point).
pub const M_AP1_TO_REC709: [[f32; 3]; 3] = [
    [1.7048587, -0.6217145, -0.0832994],
    [-0.1300773, 1.1407358, -0.0105315],
    [-0.0239641, -0.1289689, 1.1529330],
];

/// Rec.709 linear to ACES AP1 linear.
pub const M_REC709_TO_AP1: [[f32; 3]; 3] = [
    [0.613097, 0.339523, 0.047379],
    [0.070194, 0.916354, 0.013452],
    [0.020616, 0.109570, 0.869814],
];

/// Camera S-Gamut to ACES AP0 linear.
pub const M_SGAMUT_TO_AP0: [[f32; 3]; 3] = [
    [0.754336, 0.133697, 0.111968],
    [0.021198, 1.005386, -0.026585],
    [-0.005491, -0.008805, 1.014296],
];

/// Camera S-Gamut3.Cine to ACES AP0 linear.
pub const M_SGAMUT3_CINE_TO_AP0: [[f32; 3]; 3] = [
    [0.899011, 0.062908, 0.038081],
    [-0.000499, 1.003826, -0.003327],
    [-0.000508, -0.001867, 1.002375],
];

#[inline]
pub fn mat_mul_rgb(m: &[[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

// ----------------------------------------------------------------------------
// Analytical Transfer Functions
// ----------------------------------------------------------------------------

/// Forward Camera S-Log1: linear scene value `x` to S-Log1 `y`.
pub fn linear_to_slog1(x: f32) -> f32 {
    let arg = (x + 0.037584).max(1e-6);
    0.432699 * arg.log10() + 0.646596
}

/// Inverse Camera S-Log1: S-Log1 `y` to linear scene value `x`.
pub fn slog1_to_linear(y: f32) -> f32 {
    10.0f32.powf((y - 0.646596) / 0.432699) - 0.037584
}

/// Forward Camera S-Log2: linear scene value `x` to S-Log2 `y`.
pub fn linear_to_slog2(x: f32) -> f32 {
    let scaled = (155.0 * x) / (219.0 * 0.9) + 0.037584;
    let arg = scaled.max(1e-6);
    let v = 0.432699 * arg.log10() + 0.646596;
    (219.0 * v + 16.0) / 255.0
}

/// Inverse Camera S-Log2: S-Log2 `y` to linear scene value `x`.
pub fn slog2_to_linear(y: f32) -> f32 {
    let v = (255.0 * y - 16.0) / 219.0;
    let base = 10.0f32.powf((v - 0.646596) / 0.432699) - 0.037584;
    base * (219.0 * 0.9) / 155.0
}

/// Forward Camera S-Log3: linear scene value `x` to S-Log3 `y`.
pub fn linear_to_slog3(x: f32) -> f32 {
    if x >= 0.01125000 {
        let arg = ((x + 0.01) / 0.19).max(1e-6);
        (420.0 + arg.log10() * 261.5) / 1023.0
    } else {
        let slope = (171.2102946929 - 95.0) / 0.01125000;
        (x * slope + 95.0) / 1023.0
    }
}

/// Inverse Camera S-Log3: S-Log3 `y` to linear scene value `x`.
pub fn slog3_to_linear(y: f32) -> f32 {
    let y_cut = 171.2102946929 / 1023.0;
    if y >= y_cut {
        10.0f32.powf((y * 1023.0 - 420.0) / 261.5) * 0.19 - 0.01
    } else {
        (y * 1023.0 - 95.0) * (0.01125000 / (171.2102946929 - 95.0))
    }
}

/// Forward ACEScc (SMPTE S-2014-003): scene-linear `x` to ACEScc `y`.
pub fn linear_to_acescc(x: f32) -> f32 {
    if x <= 0.0 {
        (-16.0 + 9.72) / 17.52
    } else if x < 2.0f32.powf(-15.0) {
        let val = 2.0f32.powf(-16.0) + 0.5 * x;
        (val.log2() + 9.72) / 17.52
    } else {
        (x.log2() + 9.72) / 17.52
    }
}

/// Inverse ACEScc (SMPTE S-2014-003): ACEScc `y` to scene-linear `x`.
pub fn acescc_to_linear(y: f32) -> f32 {
    if y < -0.30136986 {
        (2.0f32.powf(y * 17.52 - 9.72) - 2.0f32.powf(-16.0)) * 2.0
    } else {
        2.0f32.powf(y * 17.52 - 9.72)
    }
}

/// Forward ACEScct (SMPTE S-2016-001, with toe): scene-linear `x` to ACEScct `y`.
pub fn linear_to_acescct(x: f32) -> f32 {
    const X_BRK: f32 = 0.0078125; // 2^-7
    const A: f32 = 10.5402377416545;
    const B: f32 = 0.0729055341958355;
    if x <= X_BRK {
        A * x + B
    } else {
        (x.max(1e-6).log2() + 9.72) / 17.52
    }
}

/// Inverse ACEScct (SMPTE S-2016-001, with toe): ACEScct `y` to scene-linear `x`.
pub fn acescct_to_linear(y: f32) -> f32 {
    const Y_BRK: f32 = (-7.0 + 9.72) / 17.52; // 0.15525798
    const A: f32 = 10.5402377416545;
    const B: f32 = 0.0729055341958355;
    if y <= Y_BRK {
        (y - B) / A
    } else {
        2.0f32.powf(y * 17.52 - 9.72)
    }
}

/// Forward ACESproxy (AP1 integer log preview).
pub fn linear_to_acesproxy(x: f32) -> f32 {
    linear_to_acescc(x).clamp(0.0, 1.0)
}

/// Inverse ACESproxy.
pub fn acesproxy_to_linear(y: f32) -> f32 {
    acescc_to_linear(y)
}

/// ITU-R BT.709 OETF: linear `L` to display signal `V`.
pub fn linear_to_rec709(l: f32) -> f32 {
    if l < 0.018 {
        4.5 * l
    } else {
        1.099 * l.powf(0.45) - 0.099
    }
}

/// Inverse ITU-R BT.709 OETF: display signal `V` to linear `L`.
pub fn rec709_to_linear(v: f32) -> f32 {
    if v < 0.081 {
        v / 4.5
    } else {
        ((v + 0.099) / 1.099).powf(1.0 / 0.45)
    }
}

/// IEC 61966-2-1 sRGB OETF: linear `L` to sRGB display signal `V`.
pub fn linear_to_srgb(l: f32) -> f32 {
    if l <= 0.0031308 {
        12.92 * l
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    }
}

/// Inverse IEC 61966-2-1 sRGB EOTF: sRGB display signal `V` to linear `L`.
pub fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// Analytical ACES 1.0 RRT tone curve fit (Stephen Hill / Narkowicz fit).
/// Maps scene-linear input `x` to display-referred tone-mapped output in `[0.0..=1.0]`.
pub fn aces_tonemap(x: f32) -> f32 {
    let x = x.max(0.0);
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    ((x * (a * x + b)) / (x * (c * x + d) + e)).clamp(0.0, 1.0)
}

// ----------------------------------------------------------------------------
// ACES Transformation Pipeline
// ----------------------------------------------------------------------------

/// Configured ACES transformation pipeline from input color space through working
/// space to target display view transform.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AcesTransform {
    pub input_space: AcesSpace,
    pub working_space: AcesSpace,
    pub view_transform: ViewTransform,
}

impl Default for AcesTransform {
    fn default() -> Self {
        Self {
            input_space: AcesSpace::Rec709,
            working_space: AcesSpace::AcesCg,
            view_transform: ViewTransform::AcesRrtRec709,
        }
    }
}

impl AcesTransform {
    pub fn new(
        input_space: AcesSpace,
        working_space: AcesSpace,
        view_transform: ViewTransform,
    ) -> Self {
        Self {
            input_space,
            working_space,
            view_transform,
        }
    }

    /// Transforms an RGB pixel from `input_space` through `working_space` to `view_transform`.
    pub fn transform_rgb(&self, rgb: [f32; 3]) -> [f32; 3] {
        // Step 1: Input space to Linear AP1
        let ap1_linear = match self.input_space {
            AcesSpace::Aces2065_1 => mat_mul_rgb(&M_AP0_TO_AP1, rgb),
            AcesSpace::AcesCg => rgb,
            AcesSpace::AcesCc => [
                acescc_to_linear(rgb[0]),
                acescc_to_linear(rgb[1]),
                acescc_to_linear(rgb[2]),
            ],
            AcesSpace::AcesCct => [
                acescct_to_linear(rgb[0]),
                acescct_to_linear(rgb[1]),
                acescct_to_linear(rgb[2]),
            ],
            AcesSpace::AcesProxy => [
                acesproxy_to_linear(rgb[0]),
                acesproxy_to_linear(rgb[1]),
                acesproxy_to_linear(rgb[2]),
            ],
            AcesSpace::SLog1 => {
                let lin_cam = [
                    slog1_to_linear(rgb[0]),
                    slog1_to_linear(rgb[1]),
                    slog1_to_linear(rgb[2]),
                ];
                let ap0 = mat_mul_rgb(&M_SGAMUT_TO_AP0, lin_cam);
                mat_mul_rgb(&M_AP0_TO_AP1, ap0)
            }
            AcesSpace::SLog2 => {
                let lin_cam = [
                    slog2_to_linear(rgb[0]),
                    slog2_to_linear(rgb[1]),
                    slog2_to_linear(rgb[2]),
                ];
                let ap0 = mat_mul_rgb(&M_SGAMUT_TO_AP0, lin_cam);
                mat_mul_rgb(&M_AP0_TO_AP1, ap0)
            }
            AcesSpace::SLog3 => {
                let lin_cam = [
                    slog3_to_linear(rgb[0]),
                    slog3_to_linear(rgb[1]),
                    slog3_to_linear(rgb[2]),
                ];
                let ap0 = mat_mul_rgb(&M_SGAMUT3_CINE_TO_AP0, lin_cam);
                mat_mul_rgb(&M_AP0_TO_AP1, ap0)
            }
            AcesSpace::Rec709 => {
                let lin_r709 = [
                    rec709_to_linear(rgb[0]),
                    rec709_to_linear(rgb[1]),
                    rec709_to_linear(rgb[2]),
                ];
                mat_mul_rgb(&M_REC709_TO_AP1, lin_r709)
            }
            AcesSpace::Srgb => {
                let lin_srgb = [
                    srgb_to_linear(rgb[0]),
                    srgb_to_linear(rgb[1]),
                    srgb_to_linear(rgb[2]),
                ];
                mat_mul_rgb(&M_REC709_TO_AP1, lin_srgb)
            }
        };

        // Step 2: Linear AP1 to Working Space (if view transform is Raw)
        if self.view_transform == ViewTransform::Raw {
            return match self.working_space {
                AcesSpace::AcesCg => ap1_linear,
                AcesSpace::Aces2065_1 => mat_mul_rgb(&M_AP1_TO_AP0, ap1_linear),
                AcesSpace::AcesCc => [
                    linear_to_acescc(ap1_linear[0]),
                    linear_to_acescc(ap1_linear[1]),
                    linear_to_acescc(ap1_linear[2]),
                ],
                AcesSpace::AcesCct => [
                    linear_to_acescct(ap1_linear[0]),
                    linear_to_acescct(ap1_linear[1]),
                    linear_to_acescct(ap1_linear[2]),
                ],
                AcesSpace::AcesProxy => [
                    linear_to_acesproxy(ap1_linear[0]),
                    linear_to_acesproxy(ap1_linear[1]),
                    linear_to_acesproxy(ap1_linear[2]),
                ],
                AcesSpace::Rec709 => {
                    let r709_lin = mat_mul_rgb(&M_AP1_TO_REC709, ap1_linear);
                    [
                        linear_to_rec709(r709_lin[0]),
                        linear_to_rec709(r709_lin[1]),
                        linear_to_rec709(r709_lin[2]),
                    ]
                }
                AcesSpace::Srgb => {
                    let srgb_lin = mat_mul_rgb(&M_AP1_TO_REC709, ap1_linear);
                    [
                        linear_to_srgb(srgb_lin[0]),
                        linear_to_srgb(srgb_lin[1]),
                        linear_to_srgb(srgb_lin[2]),
                    ]
                }
                _ => ap1_linear,
            };
        }

        // Step 3: View Transform (RRT + ODT)
        // Apply ACES filmic highlight rolloff (Narkowicz fit)
        let tm = [
            aces_tonemap(ap1_linear[0]),
            aces_tonemap(ap1_linear[1]),
            aces_tonemap(ap1_linear[2]),
        ];

        // Gamut mapping AP1 -> Rec709/sRGB primaries
        let display_lin = mat_mul_rgb(&M_AP1_TO_REC709, tm);

        // Apply display OETF
        match self.view_transform {
            ViewTransform::AcesRrtRec709 => [
                linear_to_rec709(display_lin[0]).clamp(0.0, 1.0),
                linear_to_rec709(display_lin[1]).clamp(0.0, 1.0),
                linear_to_rec709(display_lin[2]).clamp(0.0, 1.0),
            ],
            ViewTransform::AcesRrtSrgb => [
                linear_to_srgb(display_lin[0]).clamp(0.0, 1.0),
                linear_to_srgb(display_lin[1]).clamp(0.0, 1.0),
                linear_to_srgb(display_lin[2]).clamp(0.0, 1.0),
            ],
            ViewTransform::AcesRrtRec2020 | ViewTransform::AcesRrtHlg => [
                display_lin[0].clamp(0.0, 1.0),
                display_lin[1].clamp(0.0, 1.0),
                display_lin[2].clamp(0.0, 1.0),
            ],
            ViewTransform::Raw => ap1_linear,
        }
    }

    /// Transforms every pixel of `frame` in place.
    pub fn apply_to_frame(&self, frame: &mut FrameBuffer) {
        for px in frame.rgba.as_chunks_mut::<4>().0 {
            let src = [
                f32::from(px[0]) / 255.0,
                f32::from(px[1]) / 255.0,
                f32::from(px[2]) / 255.0,
            ];
            let out = self.transform_rgb(src);
            px[0] = (out[0].clamp(0.0, 1.0) * 255.0).round() as u8;
            px[1] = (out[1].clamp(0.0, 1.0) * 255.0).round() as u8;
            px[2] = (out[2].clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }

    /// Generates a 3D `.cube` color lookup table of resolution `size³` baking this transform.
    pub fn generate_3d_lut(&self, size: usize) -> Lut {
        let size = size.clamp(2, 65);
        let step = 1.0 / (size - 1) as f32;
        let mut data = Vec::with_capacity(size * size * size);
        for b in 0..size {
            for g in 0..size {
                for r in 0..size {
                    let in_rgb = [r as f32 * step, g as f32 * step, b as f32 * step];
                    data.push(self.transform_rgb(in_rgb));
                }
            }
        }
        Lut::from_3d_data("ACES Transform", size, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ap0_and_ap1_matrices_are_inverses() {
        let ap0 = [0.5f32, 0.4, 0.3];
        let ap1 = mat_mul_rgb(&M_AP0_TO_AP1, ap0);
        let roundtrip = mat_mul_rgb(&M_AP1_TO_AP0, ap1);
        for c in 0..3 {
            assert!((roundtrip[c] - ap0[c]).abs() < 1e-4);
        }
    }

    #[test]
    fn slog1_roundtrip() {
        for val in [0.01f32, 0.18, 0.5, 1.0] {
            let log = linear_to_slog1(val);
            let back = slog1_to_linear(log);
            assert!((back - val).abs() < 1e-4);
        }
    }

    #[test]
    fn slog2_roundtrip() {
        for val in [0.01f32, 0.18, 0.5, 1.0] {
            let log = linear_to_slog2(val);
            let back = slog2_to_linear(log);
            assert!((back - val).abs() < 1e-4);
        }
    }

    #[test]
    fn slog3_roundtrip() {
        for val in [0.005f32, 0.018, 0.18, 0.5, 1.0, 2.0] {
            let log = linear_to_slog3(val);
            let back = slog3_to_linear(log);
            assert!((back - val).abs() < 1e-4);
        }
    }

    #[test]
    fn acescc_roundtrip() {
        for val in [0.001f32, 0.05, 0.18, 1.0, 5.0] {
            let log = linear_to_acescc(val);
            let back = acescc_to_linear(log);
            assert!((back - val).abs() < 1e-4);
        }
    }

    #[test]
    fn acescct_toe_roundtrip() {
        for val in [0.001f32, 0.005, 0.01, 0.18, 1.0] {
            let log = linear_to_acescct(val);
            let back = acescct_to_linear(log);
            assert!((back - val).abs() < 1e-4);
        }
    }

    #[test]
    fn aces_tonemap_preserves_monotonicity() {
        let mut prev = 0.0;
        for i in 0..100 {
            let x = i as f32 * 0.1;
            let tm = aces_tonemap(x);
            assert!(tm >= prev);
            assert!(tm <= 1.0);
            prev = tm;
        }
    }

    #[test]
    fn aces_transform_generate_3d_lut() {
        let tf = AcesTransform::new(
            AcesSpace::SLog3,
            AcesSpace::AcesCg,
            ViewTransform::AcesRrtRec709,
        );
        let lut = tf.generate_3d_lut(4);
        assert_eq!(lut.title, "ACES Transform");
        let sampled = lut.apply([0.5, 0.5, 0.5]);
        for c in 0..3 {
            assert!(sampled[c].is_finite());
            assert!(sampled[c] >= 0.0 && sampled[c] <= 1.0);
        }
    }
}
