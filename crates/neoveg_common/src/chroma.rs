//! Chroma key (green/blue screen) parameters and Rec. 709 luma-preserving matte + spill math.

use serde::{Deserialize, Serialize};

/// Project chroma-key settings. Disabled by default so existing timelines are untouched.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChromaKey {
    pub enabled: bool,
    /// Target screen color in linear/sRGB `0.0..=1.0` (`[0.0, 0.72, 0.08]` studio green by default).
    pub key_color: [f32; 3],
    /// Chroma radius inside which pixels become completely transparent (`0.01..=0.75`).
    pub similarity: f32,
    /// Softness of the matte transition outside `similarity` (`0.0..=0.50`).
    pub smoothness: f32,
    /// Strength of luma-preserving green/blue spill suppression (`0.0..=1.0`).
    pub spill: f32,
    /// When `true`, outputs the computed alpha matte as grayscale RGB for inspection.
    pub show_matte: bool,
}

impl Default for ChromaKey {
    fn default() -> Self {
        Self {
            enabled: false,
            key_color: Self::GREEN_SCREEN,
            similarity: 0.18,
            smoothness: 0.08,
            spill: 0.55,
            show_matte: false,
        }
    }
}

fn rgb_to_ycbcr([r, g, b]: [f32; 3]) -> (f32, f32, f32) {
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let cb = (b - y) / 1.8556;
    let cr = (r - y) / 1.5748;
    (y, cb, cr)
}

fn ycbcr_to_rgb(y: f32, cb: f32, cr: f32) -> [f32; 3] {
    let r = y + 1.5748 * cr;
    let b = y + 1.8556 * cb;
    let g = (y - 0.2126 * r - 0.0722 * b) / 0.7152;
    [r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0)]
}

impl ChromaKey {
    pub const GREEN_SCREEN: [f32; 3] = [0.05, 0.72, 0.10];
    pub const BLUE_SCREEN: [f32; 3] = [0.06, 0.22, 0.78];

    pub fn is_disabled(&self) -> bool {
        !self.enabled
    }

    /// Clamps every field to valid ranges and replaces `NaN` with safe defaults.
    pub fn sanitized(self) -> Self {
        let clamp = |v: f32, lo: f32, hi: f32, def: f32| {
            if v.is_finite() {
                v.clamp(lo, hi)
            } else {
                def
            }
        };
        let def = Self::default();
        Self {
            enabled: self.enabled,
            key_color: [
                clamp(self.key_color[0], 0.0, 1.0, def.key_color[0]),
                clamp(self.key_color[1], 0.0, 1.0, def.key_color[1]),
                clamp(self.key_color[2], 0.0, 1.0, def.key_color[2]),
            ],
            similarity: clamp(self.similarity, 0.01, 0.75, def.similarity),
            smoothness: clamp(self.smoothness, 0.0, 0.50, def.smoothness),
            spill: clamp(self.spill, 0.0, 1.0, def.spill),
            show_matte: self.show_matte,
        }
    }

    /// Evaluates the chroma key on one normalized RGBA pixel (`0.0..=1.0`).
    pub fn apply_rgba(&self, rgba: [f32; 4]) -> [f32; 4] {
        if !self.enabled {
            return rgba;
        }
        let rgb = [rgba[0], rgba[1], rgba[2]];
        let (y, cb, cr) = rgb_to_ycbcr(rgb);
        let (_, k_cb, k_cr) = rgb_to_ycbcr(self.key_color);

        let dist = (cb - k_cb).hypot(cr - k_cr);
        let matte = if self.smoothness <= 1e-4 {
            if dist >= self.similarity {
                1.0_f32
            } else {
                0.0_f32
            }
        } else {
            let t = ((dist - self.similarity) / self.smoothness).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        let out_alpha = (rgba[3] * matte).clamp(0.0, 1.0);

        if self.show_matte {
            return [out_alpha, out_alpha, out_alpha, 1.0];
        }

        let k_len = k_cb.hypot(k_cr);
        let despilled = if self.spill > 1e-4 && k_len > 1e-5 {
            let ux = k_cb / k_len;
            let uy = k_cr / k_len;
            let parallel = cb * ux + cr * uy;
            let perp = (cb * (-uy) + cr * ux).abs();
            // Suppress positive chroma along the screen axis while keeping perpendicular hues and Rec. 709 luma.
            let excess = (parallel - perp * 0.35).max(0.0) * self.spill;
            ycbcr_to_rgb(y, cb - excess * ux, cr - excess * uy)
        } else {
            rgb
        };

        [despilled[0], despilled[1], despilled[2], out_alpha]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_chroma_key_is_an_exact_noop() {
        let key = ChromaKey::default();
        assert!(key.is_disabled());
        let px = [0.1, 0.8, 0.15, 1.0];
        assert_eq!(key.apply_rgba(px), px);
    }

    #[test]
    fn green_screen_is_transparent_skin_tones_stay_opaque_and_spill_preserves_luma() {
        let key = ChromaKey {
            enabled: true,
            key_color: ChromaKey::GREEN_SCREEN,
            similarity: 0.16,
            smoothness: 0.08,
            spill: 0.85,
            show_matte: false,
        };

        let bg = key.apply_rgba([0.05, 0.72, 0.10, 1.0]);
        assert!(
            bg[3] < 1e-4,
            "screen background must key to 0 alpha: {bg:?}"
        );

        let skin = [0.78, 0.56, 0.46, 1.0];
        let out_skin = key.apply_rgba(skin);
        assert!(
            (out_skin[3] - 1.0).abs() < 1e-4,
            "warm foreground skin tone must stay 100% opaque: {out_skin:?}"
        );

        // Green-contaminated edge pixel: green channel drops while Rec. 709 luma stays close.
        let contaminated = [0.35, 0.62, 0.32, 1.0];
        let (y_before, _, _) = rgb_to_ycbcr([contaminated[0], contaminated[1], contaminated[2]]);
        let cleaned = key.apply_rgba(contaminated);
        let (y_after, _, _) = rgb_to_ycbcr([cleaned[0], cleaned[1], cleaned[2]]);
        assert!(
            cleaned[1] < contaminated[1],
            "spill suppression must reduce green halo"
        );
        assert!(
            (y_before - y_after).abs() < 0.03,
            "spill suppression must preserve luminance"
        );
    }

    #[test]
    fn show_matte_outputs_grayscale_alpha_and_serde_defaults_work() {
        let key = ChromaKey {
            enabled: true,
            show_matte: true,
            ..ChromaKey::default()
        };
        let m = key.apply_rgba([0.05, 0.72, 0.10, 1.0]);
        assert_eq!(m, [0.0, 0.0, 0.0, 1.0]);

        let loaded: ChromaKey = serde_json::from_str("{}").unwrap();
        assert!(loaded.is_disabled());
    }
}
