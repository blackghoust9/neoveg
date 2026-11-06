//! Rasterization and application of Event Masks on video frames.
//!
//! Event masks apply geometric and Bézier clipping directly to a clip's picture
//! using SDF/scanline rasterization and fast 3-pass box feathering (None, In, Out, Both).

use neoveg_common::fx_mask::FxMask;
use neoveg_common::mask::{EventMask, MaskFeatherType};
use neoveg_text::mask::blur;
use neoveg_text::raster::fill;

use crate::frame::FrameBuffer;
use crate::fx_mask::outline;

/// Computes per-pixel mask coverage (`0.0..=1.0`) with support for directional feathering.
pub fn compute_event_mask_coverage(
    mask: &FxMask,
    w: u32,
    h: u32,
    feather_type: MaskFeatherType,
    invert: bool,
) -> Vec<f32> {
    if w == 0 || h == 0 {
        return Vec::new();
    }
    let mask = mask.sanitized();
    let (wf, hf) = (w as f32, h as f32);
    let path = outline(&mask, wf, hf);
    let hard_fill = fill(&path, w as usize, h as usize, 0.0, 0.0);

    let effective_invert = invert ^ mask.invert;

    if feather_type == MaskFeatherType::None || mask.feather <= 0.0 {
        let mut cov = hard_fill.a;
        for v in &mut cov {
            let clamped = v.clamp(0.0, 1.0);
            *v = if effective_invert {
                1.0 - clamped
            } else {
                clamped
            };
        }
        return cov;
    }

    let sigma = mask.feather * hf * 0.5;
    let soft_fill = blur(&hard_fill, sigma);

    let count = (w * h) as usize;
    let mut out = Vec::with_capacity(count);

    match feather_type {
        MaskFeatherType::None => unreachable!(),
        MaskFeatherType::Both => {
            for &s in soft_fill.a.iter().take(count) {
                let v = s.clamp(0.0, 1.0);
                out.push(if effective_invert { 1.0 - v } else { v });
            }
        }
        MaskFeatherType::In => {
            // Feathering restricted inside the shape boundary: soft.min(hard)
            for (&h_val, &s_val) in hard_fill.a.iter().zip(soft_fill.a.iter()).take(count) {
                let v = s_val.min(h_val).clamp(0.0, 1.0);
                out.push(if effective_invert { 1.0 - v } else { v });
            }
        }
        MaskFeatherType::Out => {
            // Feathering expands outward while shape interior remains 1.0: soft.max(hard)
            for (&h_val, &s_val) in hard_fill.a.iter().zip(soft_fill.a.iter()).take(count) {
                let v = s_val.max(h_val).clamp(0.0, 1.0);
                out.push(if effective_invert { 1.0 - v } else { v });
            }
        }
    }

    out
}

/// Applies an EventMask to a video frame at time `time_s`, modulating the frame's alpha channel.
pub fn apply_event_mask(frame: &mut FrameBuffer, mask: &EventMask, time_s: f64) {
    if !mask.enabled || mask.keys.is_empty() || frame.width == 0 || frame.height == 0 {
        return;
    }

    let (fx_mask, opacity, feather_type) = match mask.mask_at(time_s) {
        Some(m) => m,
        None => return,
    };

    if opacity <= 0.0 {
        // Fully transparent
        for px in frame.rgba.as_chunks_mut::<4>().0 {
            px[3] = 0;
        }
        return;
    }

    let coverage = compute_event_mask_coverage(
        &fx_mask,
        frame.width,
        frame.height,
        feather_type,
        mask.invert,
    );

    for (px, &cov) in frame
        .rgba
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(coverage.iter())
    {
        let factor = (cov * opacity).clamp(0.0, 1.0);
        px[3] = (f32::from(px[3]) * factor).round().clamp(0.0, 255.0) as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neoveg_common::fx_mask::MaskShape;
    use neoveg_common::mask::EventMaskKey;

    #[test]
    fn test_event_mask_disabled_noop() {
        let mut frame = FrameBuffer::solid(16, 16, [255, 0, 0, 255]);
        let mask = EventMask {
            enabled: false,
            invert: false,
            keys: vec![EventMaskKey {
                time: 0.0,
                mask: FxMask::default(),
                opacity: 1.0,
                feather_type: MaskFeatherType::None,
            }],
        };
        apply_event_mask(&mut frame, &mask, 0.0);
        assert_eq!(frame.pixel(8, 8)[3], 255);
    }

    #[test]
    fn test_event_mask_rectangular_hard_cut() {
        let mut frame = FrameBuffer::solid(32, 32, [255, 255, 255, 255]);
        let mask = EventMask {
            enabled: true,
            invert: false,
            keys: vec![EventMaskKey {
                time: 0.0,
                mask: FxMask {
                    shape: MaskShape::Rectangle,
                    center: [0.5, 0.5],
                    size: [0.5, 0.5],
                    rotation_deg: 0.0,
                    feather: 0.0,
                    invert: false,
                    points: Vec::new(),
                },
                opacity: 1.0,
                feather_type: MaskFeatherType::None,
            }],
        };
        apply_event_mask(&mut frame, &mask, 0.0);
        // Center pixel should be opaque inside rectangle
        assert_eq!(frame.pixel(16, 16)[3], 255);
        // Corner pixel (0, 0) should be fully transparent
        assert_eq!(frame.pixel(0, 0)[3], 0);
    }

    #[test]
    fn test_event_mask_inversion() {
        let mut frame = FrameBuffer::solid(32, 32, [255, 255, 255, 255]);
        let mask = EventMask {
            enabled: true,
            invert: true,
            keys: vec![EventMaskKey {
                time: 0.0,
                mask: FxMask {
                    shape: MaskShape::Rectangle,
                    center: [0.5, 0.5],
                    size: [0.5, 0.5],
                    rotation_deg: 0.0,
                    feather: 0.0,
                    invert: false,
                    points: Vec::new(),
                },
                opacity: 1.0,
                feather_type: MaskFeatherType::None,
            }],
        };
        apply_event_mask(&mut frame, &mask, 0.0);
        // Inverted: center is transparent, corner is opaque
        assert_eq!(frame.pixel(16, 16)[3], 0);
        assert_eq!(frame.pixel(0, 0)[3], 255);
    }

    #[test]
    fn test_event_mask_feather_types() {
        let mask = FxMask {
            shape: MaskShape::Rectangle,
            center: [0.5, 0.5],
            size: [0.5, 0.5],
            rotation_deg: 0.0,
            feather: 0.1,
            invert: false,
            points: Vec::new(),
        };

        let cov_in = compute_event_mask_coverage(&mask, 32, 32, MaskFeatherType::In, false);
        let cov_out = compute_event_mask_coverage(&mask, 32, 32, MaskFeatherType::Out, false);
        let cov_both = compute_event_mask_coverage(&mask, 32, 32, MaskFeatherType::Both, false);

        assert_eq!(cov_in.len(), 1024);
        assert_eq!(cov_out.len(), 1024);
        assert_eq!(cov_both.len(), 1024);

        // Outside pixel (0, 0) in "In" feather must be exactly 0
        assert_eq!(cov_in[0], 0.0);
        // Center pixel in "Out" feather must be 1.0
        assert_eq!(cov_out[16 * 32 + 16], 1.0);
    }
}
