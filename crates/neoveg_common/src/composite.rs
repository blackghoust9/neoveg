//! Video track compositing modes and reference per-pixel / spatial blend math.
//!
//! Covers all 14 standard Classic NLE compositing modes (`SourceAlpha`, `Add`, `Subtract`,
//! `Multiply`, `Screen`, `Overlay`, `HardLight`, `Dodge`, `Burn`, `Darken`, `Lighten`,
//! `Difference`, `DifferenceSquared`, `Cut`) plus the three custom map modes
//! (`DisplacementMap`, `HeightMap`, `BumpMap`).

use crate::i18n::tr;
use serde::{Deserialize, Serialize};

/// Per-track video compositing mode. The higher track's mode governs how it blends over tracks
/// below (or over its compositing children when acting as a parent track).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositeMode {
    #[default]
    SourceAlpha,
    Add,
    Subtract,
    Multiply,
    Screen,
    Overlay,
    HardLight,
    Dodge,
    Burn,
    Darken,
    Lighten,
    Difference,
    DifferenceSquared,
    Cut,
    DisplacementMap,
    HeightMap,
    BumpMap,
}

impl CompositeMode {
    pub const ALL: [CompositeMode; 17] = [
        Self::SourceAlpha,
        Self::Add,
        Self::Subtract,
        Self::Multiply,
        Self::Screen,
        Self::Overlay,
        Self::HardLight,
        Self::Dodge,
        Self::Burn,
        Self::Darken,
        Self::Lighten,
        Self::Difference,
        Self::DifferenceSquared,
        Self::Cut,
        Self::DisplacementMap,
        Self::HeightMap,
        Self::BumpMap,
    ];

    pub fn is_default(&self) -> bool {
        matches!(self, Self::SourceAlpha)
    }

    /// True when the mode inspects neighboring pixels or warps UV coordinates (`DisplacementMap`,
    /// `HeightMap`, `BumpMap`).
    pub fn is_spatial(self) -> bool {
        matches!(
            self,
            Self::DisplacementMap | Self::HeightMap | Self::BumpMap
        )
    }

    /// Numeric code shared with the `wgpu` WGSL compute shader.
    pub fn code(self) -> u32 {
        match self {
            Self::SourceAlpha => 0,
            Self::Add => 1,
            Self::Subtract => 2,
            Self::Multiply => 3,
            Self::Screen => 4,
            Self::Overlay => 5,
            Self::HardLight => 6,
            Self::Dodge => 7,
            Self::Burn => 8,
            Self::Darken => 9,
            Self::Lighten => 10,
            Self::Difference => 11,
            Self::DifferenceSquared => 12,
            Self::Cut => 13,
            Self::DisplacementMap => 14,
            Self::HeightMap => 15,
            Self::BumpMap => 16,
        }
    }

    pub fn from_code(code: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.code() == code)
    }

    /// Localized `pt-BR` label for UI menus and track headers.
    pub fn label(self) -> &'static str {
        match self {
            Self::SourceAlpha => tr("Alfa da origem"),
            Self::Add => tr("Adicionar"),
            Self::Subtract => tr("Subtrair"),
            Self::Multiply => tr("Multiplicar (Máscara)"),
            Self::Screen => tr("Tela (Screen)"),
            Self::Overlay => tr("Sobrepor (Overlay)"),
            Self::HardLight => tr("Luz forte (Hard Light)"),
            Self::Dodge => tr("Subexposição (Dodge)"),
            Self::Burn => tr("Superexposição (Burn)"),
            Self::Darken => tr("Escurecer"),
            Self::Lighten => tr("Clarear"),
            Self::Difference => tr("Diferença"),
            Self::DifferenceSquared => tr("Diferença ao quadrado"),
            Self::Cut => tr("Recorte (Cut)"),
            Self::DisplacementMap => tr("Mapa de deslocamento"),
            Self::HeightMap => tr("Mapa de altura"),
            Self::BumpMap => tr("Mapa de relevo (Bump)"),
        }
    }

    /// Evaluates the separable channel blend function `B(dst, src)` for normalized `0.0..=1.0`
    /// channels (used by the standard non-spatial modes except `Multiply` and `Cut`).
    pub fn blend_channel(self, dst: f32, src: f32) -> f32 {
        let d = dst.clamp(0.0, 1.0);
        let s = src.clamp(0.0, 1.0);
        match self {
            Self::SourceAlpha | Self::Cut => s,
            Self::Add => (d + s).min(1.0),
            Self::Subtract => (d - s).max(0.0),
            Self::Multiply => d * s,
            Self::Screen => 1.0 - (1.0 - d) * (1.0 - s),
            Self::Overlay => {
                if d < 0.5 {
                    2.0 * d * s
                } else {
                    1.0 - 2.0 * (1.0 - d) * (1.0 - s)
                }
            }
            Self::HardLight => {
                if s < 0.5 {
                    2.0 * d * s
                } else {
                    1.0 - 2.0 * (1.0 - d) * (1.0 - s)
                }
            }
            Self::Dodge => {
                if s >= 0.9999 {
                    1.0
                } else {
                    (d / (1.0 - s)).min(1.0)
                }
            }
            Self::Burn => {
                if s <= 0.0001 {
                    0.0
                } else {
                    (1.0 - (1.0 - d) / s).max(0.0)
                }
            }
            Self::Darken => d.min(s),
            Self::Lighten => d.max(s),
            Self::Difference => (d - s).abs(),
            Self::DifferenceSquared => {
                let diff = d - s;
                diff * diff
            }
            Self::DisplacementMap | Self::HeightMap | Self::BumpMap => d,
        }
    }

    /// Composites one straight-alpha `src` pixel over `dst` (`0.0..=1.0` RGBA) with `opacity`
    /// (`0.0..=1.0`) for non-spatial modes.
    pub fn composite_rgba(self, dst: [f32; 4], src: [f32; 4], opacity: f32) -> [f32; 4] {
        let op = if opacity.is_finite() {
            opacity.clamp(0.0, 1.0)
        } else {
            1.0
        };
        if op <= 0.0 {
            return dst;
        }
        let d = [
            dst[0].clamp(0.0, 1.0),
            dst[1].clamp(0.0, 1.0),
            dst[2].clamp(0.0, 1.0),
            dst[3].clamp(0.0, 1.0),
        ];
        let s = [
            src[0].clamp(0.0, 1.0),
            src[1].clamp(0.0, 1.0),
            src[2].clamp(0.0, 1.0),
            src[3].clamp(0.0, 1.0),
        ];

        match self {
            Self::Cut => [
                d[0] + (s[0] - d[0]) * op,
                d[1] + (s[1] - d[1]) * op,
                d[2] + (s[2] - d[2]) * op,
                d[3] + (s[3] - d[3]) * op,
            ],
            Self::Multiply => {
                // Classic NLE "Multiply (Mask)": multiplies both RGB and destination alpha by the
                // premultiplied mask so transparent areas of a title/shape mask cut out child tracks.
                let mr = d[0] * s[0] * s[3];
                let mg = d[1] * s[1] * s[3];
                let mb = d[2] * s[2] * s[3];
                let ma = d[3] * s[3];
                [
                    (d[0] + (mr - d[0]) * op).clamp(0.0, 1.0),
                    (d[1] + (mg - d[1]) * op).clamp(0.0, 1.0),
                    (d[2] + (mb - d[2]) * op).clamp(0.0, 1.0),
                    (d[3] + (ma - d[3]) * op).clamp(0.0, 1.0),
                ]
            }
            _ => {
                let as_eff = (s[3] * op).clamp(0.0, 1.0);
                if as_eff <= 0.0 {
                    return d;
                }
                let da = d[3];
                let out_a = as_eff + da * (1.0 - as_eff);
                if out_a <= 1e-6 {
                    return [0.0, 0.0, 0.0, 0.0];
                }
                let mut out = [0.0_f32; 4];
                for c in 0..3 {
                    let blended = self.blend_channel(d[c], s[c]);
                    let premul = as_eff * (1.0 - da) * s[c]
                        + as_eff * da * blended
                        + (1.0 - as_eff) * da * d[c];
                    out[c] = (premul / out_a).clamp(0.0, 1.0);
                }
                out[3] = out_a.clamp(0.0, 1.0);
                out
            }
        }
    }
}

/// Rec. 709 luminance of a normalized RGBA pixel (`0.0..=1.0`).
#[inline]
pub fn luma709(rgba: [f32; 4]) -> f32 {
    0.2126 * rgba[0] + 0.7152 * rgba[1] + 0.0722 * rgba[2]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip_and_default_is_source_alpha() {
        assert_eq!(CompositeMode::default(), CompositeMode::SourceAlpha);
        assert!(CompositeMode::SourceAlpha.is_default());
        assert!(!CompositeMode::Add.is_default());
        for (idx, mode) in CompositeMode::ALL.into_iter().enumerate() {
            assert_eq!(mode.code(), idx as u32);
            assert_eq!(CompositeMode::from_code(mode.code()), Some(mode));
            assert!(!mode.label().is_empty());
        }
        assert_eq!(CompositeMode::from_code(999), None);
    }

    #[test]
    fn all_non_spatial_modes_produce_expected_reference_values() {
        let dst = [0.4_f32, 0.6, 0.25, 1.0];
        let src = [0.5_f32, 0.3, 0.8, 1.0];

        assert_eq!(
            CompositeMode::SourceAlpha.composite_rgba(dst, src, 1.0),
            src
        );
        assert_eq!(
            CompositeMode::SourceAlpha.composite_rgba(dst, src, 0.0),
            dst
        );

        let add = CompositeMode::Add.composite_rgba(dst, src, 1.0);
        assert!((add[0] - 0.9).abs() < 1e-5 && (add[2] - 1.0).abs() < 1e-5);

        let sub = CompositeMode::Subtract.composite_rgba(dst, src, 1.0);
        assert!(sub[0].abs() < 1e-5 && (sub[1] - 0.3).abs() < 1e-5);

        let mul = CompositeMode::Multiply.composite_rgba(dst, src, 1.0);
        assert!((mul[0] - 0.2).abs() < 1e-5 && (mul[1] - 0.18).abs() < 1e-5);

        // Multiply (Mask): transparent source cuts alpha to 0 so background tracks show through.
        let masked = CompositeMode::Multiply.composite_rgba(dst, [1.0, 1.0, 1.0, 0.0], 1.0);
        assert_eq!(masked, [0.0, 0.0, 0.0, 0.0]);

        let scr = CompositeMode::Screen.composite_rgba(dst, src, 1.0);
        assert!((scr[0] - 0.7).abs() < 1e-5);

        let ov = CompositeMode::Overlay.composite_rgba(dst, src, 1.0);
        assert!((ov[0] - 0.4).abs() < 1e-5 && (ov[1] - 0.44).abs() < 1e-5);

        let hl = CompositeMode::HardLight.composite_rgba(src, dst, 1.0);
        assert!((hl[0] - ov[0]).abs() < 1e-5 && (hl[1] - ov[1]).abs() < 1e-5);

        let dodge = CompositeMode::Dodge.composite_rgba(dst, [0.5, 0.99999, 0.0, 1.0], 1.0);
        assert!((dodge[0] - 0.8).abs() < 1e-5 && (dodge[1] - 1.0).abs() < 1e-5);

        let burn = CompositeMode::Burn.composite_rgba(dst, [0.8, 0.00001, 1.0, 1.0], 1.0);
        assert!((burn[0] - 0.25).abs() < 1e-5 && burn[1].abs() < 1e-5);

        let dark = CompositeMode::Darken.composite_rgba(dst, src, 1.0);
        assert!((dark[0] - 0.4).abs() < 1e-5 && (dark[1] - 0.3).abs() < 1e-5);

        let light = CompositeMode::Lighten.composite_rgba(dst, src, 1.0);
        assert!((light[0] - 0.5).abs() < 1e-5 && (light[2] - 0.8).abs() < 1e-5);

        let diff = CompositeMode::Difference.composite_rgba(dst, src, 1.0);
        assert!((diff[0] - 0.1).abs() < 1e-5 && (diff[1] - 0.3).abs() < 1e-5);

        let diff2 = CompositeMode::DifferenceSquared.composite_rgba(dst, src, 1.0);
        assert!((diff2[0] - 0.01).abs() < 1e-5 && (diff2[1] - 0.09).abs() < 1e-5);

        // Cut stencil punches transparent holes directly through destination.
        let hole = CompositeMode::Cut.composite_rgba(dst, [0.0, 0.0, 0.0, 0.0], 1.0);
        assert_eq!(hole, [0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn serde_round_trip_uses_snake_case() {
        let json = serde_json::to_string(&CompositeMode::DifferenceSquared).unwrap();
        assert_eq!(json, "\"difference_squared\"");
        let parsed: CompositeMode = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, CompositeMode::DifferenceSquared);
    }
}
