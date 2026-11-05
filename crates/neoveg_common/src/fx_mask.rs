//! The mask of a video effect (manual v21, *Bézier masks* and the mask modes of the Pan/Crop plug-in;
//! the *FX masks* of VEGAS Pro 12): an effect with a mask changes the picture only where the mask
//! lets it, with a soft edge, or everywhere but there (the negative mode).
//!
//! The shape is a rectangle, an ellipse or a Bézier path (a closed smooth curve through points the
//! user places). Positions are fractions of the frame, so the same mask fits the preview, the
//! render and any size of picture. Here is only the description of the mask; the coverage it gives
//! is computed with the text engine's rasteriser in `neoveg_video::fx_mask`.

use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};

/// Most points of a Bézier mask.
pub const MAX_POINTS: usize = 48;
/// Fewest points that make a closed curve.
pub const MIN_POINTS: usize = 3;
/// Largest feather, as a fraction of the frame height.
pub const MAX_FEATHER: f32 = 0.25;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MaskShape {
    #[default]
    Rectangle,
    Ellipse,
    Bezier,
}

impl MaskShape {
    pub const ALL: [MaskShape; 3] = [Self::Rectangle, Self::Ellipse, Self::Bezier];

    /// Name in the panel (a key of the translation files).
    pub fn label(self) -> &'static str {
        match self {
            Self::Rectangle => "Retângulo",
            Self::Ellipse => "Elipse",
            Self::Bezier => "Curva de Bézier",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FxMask {
    pub shape: MaskShape,
    /// Centre of a rectangle or an ellipse, as a fraction of the frame width and height.
    #[serde(default = "half_pair")]
    pub center: [f32; 2],
    /// Size of a rectangle or an ellipse, as a fraction of the frame width and height.
    #[serde(default = "half_pair")]
    pub size: [f32; 2],
    /// Clockwise rotation of a rectangle or an ellipse, in degrees.
    #[serde(default)]
    pub rotation_deg: f32,
    /// Softness of the edge, as a fraction of the frame height (`0` is a hard edge).
    #[serde(default)]
    pub feather: f32,
    /// The negative mode: the effect acts outside the shape.
    #[serde(default)]
    pub invert: bool,
    /// The points of a Bézier mask, as fractions of the frame; the curve passes through them.
    #[serde(default)]
    pub points: Vec<[f32; 2]>,
}

fn half_pair() -> [f32; 2] {
    [0.5, 0.5]
}

impl Default for FxMask {
    fn default() -> Self {
        Self {
            shape: MaskShape::Rectangle,
            center: half_pair(),
            size: half_pair(),
            rotation_deg: 0.0,
            feather: 0.0,
            invert: false,
            points: Vec::new(),
        }
    }
}

impl FxMask {
    /// A new mask of `shape` with a sensible first form: half of the frame, in the middle (and
    /// for a Bézier mask a rounded diamond of four points).
    pub fn new(shape: MaskShape) -> Self {
        let mut mask = Self {
            shape,
            ..Self::default()
        };
        if shape == MaskShape::Bezier {
            mask.points = vec![[0.5, 0.25], [0.75, 0.5], [0.5, 0.75], [0.25, 0.5]];
        }
        mask
    }

    /// The mask with every value inside what makes sense: numbers that are numbers, fractions of
    /// the frame, a Bézier mask with at least [`MIN_POINTS`] points (a mask with fewer is a
    /// rectangle) and at most [`MAX_POINTS`].
    pub fn sanitized(&self) -> Self {
        let unit = |v: f32, default: f32| {
            if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                default
            }
        };
        let points: Vec<[f32; 2]> = self
            .points
            .iter()
            .take(MAX_POINTS)
            .map(|p| [unit(p[0], 0.5), unit(p[1], 0.5)])
            .collect();
        let shape = if self.shape == MaskShape::Bezier && points.len() < MIN_POINTS {
            MaskShape::Rectangle
        } else {
            self.shape
        };
        Self {
            shape,
            center: [unit(self.center[0], 0.5), unit(self.center[1], 0.5)],
            size: [
                unit(self.size[0], 0.5).max(0.001),
                unit(self.size[1], 0.5).max(0.001),
            ],
            rotation_deg: if self.rotation_deg.is_finite() {
                self.rotation_deg.rem_euclid(360.0)
            } else {
                0.0
            },
            feather: if self.feather.is_finite() {
                self.feather.clamp(0.0, MAX_FEATHER)
            } else {
                0.0
            },
            invert: self.invert,
            points,
        }
    }

    /// Feeds the mask into a hasher (the worker's frame cache key).
    pub fn hash_into(&self, h: &mut impl Hasher) {
        (self.shape as u8, self.invert).hash(h);
        for v in [
            self.center[0],
            self.center[1],
            self.size[0],
            self.size[1],
            self.rotation_deg,
            self.feather,
        ] {
            v.to_bits().hash(h);
        }
        self.points.len().hash(h);
        for p in &self.points {
            p[0].to_bits().hash(h);
            p[1].to_bits().hash(h);
        }
    }
}

/// The cubic Bézier segments of the closed smooth curve through `points`: each segment is
/// `[start, control1, control2, end]`, the controls being the tangents of a Catmull-Rom spline (the
/// curve is smooth where two segments meet). Empty for fewer than [`MIN_POINTS`] points.
pub fn bezier_segments(points: &[[f32; 2]]) -> Vec<[[f32; 2]; 4]> {
    let n = points.len();
    if n < MIN_POINTS {
        return Vec::new();
    }
    (0..n)
        .map(|i| {
            let p0 = points[(i + n - 1) % n];
            let p1 = points[i];
            let p2 = points[(i + 1) % n];
            let p3 = points[(i + 2) % n];
            let c1 = [p1[0] + (p2[0] - p0[0]) / 6.0, p1[1] + (p2[1] - p0[1]) / 6.0];
            let c2 = [p2[0] - (p3[0] - p1[0]) / 6.0, p2[1] - (p3[1] - p1[1]) / 6.0];
            [p1, c1, c2, p2]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_mask_has_a_first_form_for_each_shape() {
        let rect = FxMask::new(MaskShape::Rectangle);
        assert_eq!((rect.center, rect.size), ([0.5, 0.5], [0.5, 0.5]));
        assert!(rect.points.is_empty() && !rect.invert);
        assert_eq!(FxMask::new(MaskShape::Bezier).points.len(), 4);
        for shape in MaskShape::ALL {
            assert!(!shape.label().is_empty());
        }
    }

    #[test]
    fn values_are_made_safe_and_a_curve_with_too_few_points_is_a_rectangle() {
        let odd = FxMask {
            shape: MaskShape::Ellipse,
            center: [f32::NAN, 3.0],
            size: [0.0, f32::INFINITY],
            rotation_deg: 450.0,
            feather: 9.0,
            invert: true,
            points: Vec::new(),
        };
        let safe = odd.sanitized();
        assert_eq!(safe.center, [0.5, 1.0]);
        assert!(safe.size[0] > 0.0 && safe.size[1] == 0.5);
        assert_eq!(safe.rotation_deg, 90.0);
        assert_eq!(safe.feather, MAX_FEATHER);
        assert!(safe.invert);
        let two = FxMask {
            shape: MaskShape::Bezier,
            points: vec![[0.1, 0.1], [0.9, 0.9]],
            ..FxMask::default()
        };
        assert_eq!(two.sanitized().shape, MaskShape::Rectangle);
        let many = FxMask {
            shape: MaskShape::Bezier,
            points: (0..100).map(|i| [i as f32 / 100.0, 0.5]).collect(),
            ..FxMask::default()
        };
        assert_eq!(many.sanitized().points.len(), MAX_POINTS);
    }

    #[test]
    fn the_curve_passes_through_its_points_and_closes() {
        let points = [[0.2, 0.2], [0.8, 0.2], [0.8, 0.8], [0.2, 0.8]];
        let segments = bezier_segments(&points);
        assert_eq!(segments.len(), 4);
        for (i, segment) in segments.iter().enumerate() {
            assert_eq!(segment[0], points[i], "starts at its point");
            assert_eq!(segment[3], points[(i + 1) % 4], "ends at the next one");
        }
        assert!(bezier_segments(&points[..2]).is_empty());
    }

    #[test]
    fn the_mask_is_kept_in_the_project_and_an_old_one_loads() {
        let mask = FxMask::new(MaskShape::Bezier);
        let json = serde_json::to_string(&mask).unwrap();
        assert_eq!(serde_json::from_str::<FxMask>(&json).unwrap(), mask);
        // Only the shape is needed
        let minimal: FxMask = serde_json::from_str(r#"{"shape":"Ellipse"}"#).unwrap();
        assert_eq!(minimal.shape, MaskShape::Ellipse);
        assert_eq!(minimal.size, [0.5, 0.5]);
        // The same mask hashes alike and a changed one does not
        let key = |m: &FxMask| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            m.hash_into(&mut h);
            h.finish()
        };
        let mut moved = mask.clone();
        assert_eq!(key(&mask), key(&moved));
        moved.points[0][0] += 0.01;
        assert_ne!(key(&mask), key(&moved));
    }
}
