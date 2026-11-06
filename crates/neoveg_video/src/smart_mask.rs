//! Smart Mask AI Object Segmentation (Feature 19).
//!
//! Provides pure-CPU deterministic object segmentation using color seed clustering,
//! morphological boundary extraction, and vectorization into Bézier splines (FxMask).

use serde::{Deserialize, Serialize};

use crate::frame::FrameBuffer;
use neoveg_common::fx_mask::{FxMask, MaskShape, MAX_POINTS, MIN_POINTS};

/// Precision mode for object segmentation analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SmartMaskPrecision {
    Low,
    #[default]
    High,
}

/// User interaction prompt for object segmentation.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SegmentationPrompt {
    /// Optional normalized bounding box `[x, y, width, height]` in `0.0..=1.0`.
    pub bounding_box: Option<[f32; 4]>,
    /// Foreground inclusion anchor points `[x, y]` in `0.0..=1.0`.
    #[serde(default)]
    pub positive_points: Vec<[f32; 2]>,
    /// Background exclusion anchor points `[x, y]` in `0.0..=1.0`.
    #[serde(default)]
    pub negative_points: Vec<[f32; 2]>,
}

/// Binary/grayscale alpha matte resulting from segmentation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmartMaskMatte {
    pub width: u32,
    pub height: u32,
    pub alpha_channel: Vec<u8>,
}

impl SmartMaskMatte {
    /// Pure software CPU fallback segmentation using color difference / luminance thresholding.
    pub fn cpu_fallback_segment(frame: &FrameBuffer, threshold_luma: u8) -> Self {
        let (w, h) = (frame.width, frame.height);
        let mut alpha = Vec::with_capacity((w * h) as usize);
        for px in frame.rgba.as_chunks::<4>().0 {
            let luma = (0.2126 * f32::from(px[0])
                + 0.7152 * f32::from(px[1])
                + 0.0722 * f32::from(px[2])) as u8;
            alpha.push(if luma >= threshold_luma { 255 } else { 0 });
        }
        Self {
            width: w,
            height: h,
            alpha_channel: alpha,
        }
    }

    /// Segments an object from `frame` based on prompt points and/or bounding box.
    pub fn segment_frame(
        frame: &FrameBuffer,
        prompt: &SegmentationPrompt,
        precision: SmartMaskPrecision,
    ) -> Self {
        let (w, h) = (frame.width, frame.height);
        if w == 0 || h == 0 {
            return Self {
                width: 0,
                height: 0,
                alpha_channel: Vec::new(),
            };
        }

        let total_pixels = (w * h) as usize;
        let mut alpha = vec![0u8; total_pixels];

        let bbox = prompt.bounding_box.map(|[bx, by, bw, bh]| {
            let x0 = (bx.clamp(0.0, 1.0) * w as f32) as i32;
            let y0 = (by.clamp(0.0, 1.0) * h as f32) as i32;
            let x1 = ((bx + bw).clamp(0.0, 1.0) * w as f32) as i32;
            let y1 = ((by + bh).clamp(0.0, 1.0) * h as f32) as i32;
            (x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1))
        });

        // Compute mean foreground color inside prompt region or positive points
        let mut fg_r = 0.0f32;
        let mut fg_g = 0.0f32;
        let mut fg_b = 0.0f32;
        let mut fg_count = 0.0f32;

        if let Some((x0, y0, x1, y1)) = bbox {
            for y in y0..=y1 {
                for x in x0..=x1 {
                    if x >= 0 && x < w as i32 && y >= 0 && y < h as i32 {
                        let idx = (y as usize * w as usize + x as usize) * 4;
                        fg_r += f32::from(frame.rgba[idx]);
                        fg_g += f32::from(frame.rgba[idx + 1]);
                        fg_b += f32::from(frame.rgba[idx + 2]);
                        fg_count += 1.0;
                    }
                }
            }
        }

        for &[px, py] in &prompt.positive_points {
            let ix = (px.clamp(0.0, 1.0) * (w - 1) as f32).round() as usize;
            let iy = (py.clamp(0.0, 1.0) * (h - 1) as f32).round() as usize;
            let idx = (iy * w as usize + ix) * 4;
            fg_r += f32::from(frame.rgba[idx]) * 5.0;
            fg_g += f32::from(frame.rgba[idx + 1]) * 5.0;
            fg_b += f32::from(frame.rgba[idx + 2]) * 5.0;
            fg_count += 5.0;
        }

        if fg_count > 0.0 {
            fg_r /= fg_count;
            fg_g /= fg_count;
            fg_b /= fg_count;
        } else {
            // Default center seed
            let cx = w / 2;
            let cy = h / 2;
            let idx = (cy as usize * w as usize + cx as usize) * 4;
            fg_r = f32::from(frame.rgba[idx]);
            fg_g = f32::from(frame.rgba[idx + 1]);
            fg_b = f32::from(frame.rgba[idx + 2]);
        }

        // Color distance threshold
        let color_thresh = match precision {
            SmartMaskPrecision::Low => 70.0f32,
            SmartMaskPrecision::High => 55.0f32,
        };

        for y in 0..h {
            for x in 0..w {
                let idx = (y as usize * w as usize + x as usize) * 4;
                let r = f32::from(frame.rgba[idx]);
                let g = f32::from(frame.rgba[idx + 1]);
                let b = f32::from(frame.rgba[idx + 2]);

                // Check bounding box constraint if present
                let in_box = if let Some((x0, y0, x1, y1)) = bbox {
                    (x as i32) >= x0 && (x as i32) <= x1 && (y as i32) >= y0 && (y as i32) <= y1
                } else {
                    true
                };

                let mut is_fg = false;
                if in_box {
                    let dist = (0.299 * (r - fg_r).powi(2)
                        + 0.587 * (g - fg_g).powi(2)
                        + 0.114 * (b - fg_b).powi(2))
                    .sqrt();
                    if dist <= color_thresh {
                        is_fg = true;
                    }
                }

                // Point prompt overrides
                let nx = x as f32 / w.max(1) as f32;
                let ny = y as f32 / h.max(1) as f32;

                // Positive points include local radius
                for &[px, py] in &prompt.positive_points {
                    let d = ((nx - px).powi(2) + (ny - py).powi(2)).sqrt();
                    if d <= 0.05 {
                        is_fg = true;
                    }
                }

                // Negative points exclude local radius (takes precedence over positive)
                for &[px, py] in &prompt.negative_points {
                    let d = ((nx - px).powi(2) + (ny - py).powi(2)).sqrt();
                    if d <= 0.05 {
                        is_fg = false;
                    }
                }

                alpha[y as usize * w as usize + x as usize] = if is_fg { 255 } else { 0 };
            }
        }

        Self {
            width: w,
            height: h,
            alpha_channel: alpha,
        }
    }

    /// Vectorizes binary alpha matte into a smooth Bézier spline FxMask.
    pub fn vectorize_to_bezier(&self, max_points: usize) -> FxMask {
        let max_pts = max_points.clamp(MIN_POINTS, MAX_POINTS);

        if self.width == 0 || self.height == 0 || self.alpha_channel.is_empty() {
            return FxMask {
                shape: MaskShape::Bezier,
                center: [0.5, 0.5],
                size: [1.0, 1.0],
                rotation_deg: 0.0,
                feather: 0.0,
                invert: false,
                points: vec![[0.2, 0.2], [0.8, 0.2], [0.8, 0.8], [0.2, 0.8]],
            };
        }

        // Find bounding box and perimeter contour points of foreground
        let mut min_x = self.width;
        let mut max_x = 0;
        let mut min_y = self.height;
        let mut max_y = 0;

        let mut contour_points: Vec<[f32; 2]> = Vec::new();

        for y in 0..self.height {
            for x in 0..self.width {
                if self.alpha_channel[(y * self.width + x) as usize] > 128 {
                    min_x = min_x.min(x);
                    max_x = max_x.max(x);
                    min_y = min_y.min(y);
                    max_y = max_y.max(y);

                    // Check if edge pixel (neighbor is background)
                    let is_edge = x == 0
                        || y == 0
                        || x == self.width - 1
                        || y == self.height - 1
                        || self.alpha_channel[(y * self.width + x - 1) as usize] <= 128
                        || self.alpha_channel[(y * self.width + x + 1) as usize] <= 128
                        || self.alpha_channel[((y - 1) * self.width + x) as usize] <= 128
                        || self.alpha_channel[((y + 1) * self.width + x) as usize] <= 128;

                    if is_edge && contour_points.len() < 256 {
                        contour_points.push([
                            x as f32 / self.width as f32,
                            y as f32 / self.height as f32,
                        ]);
                    }
                }
            }
        }

        if contour_points.len() < MIN_POINTS {
            // Fallback to bounding box diamond
            return FxMask {
                shape: MaskShape::Bezier,
                center: [0.5, 0.5],
                size: [0.5, 0.5],
                rotation_deg: 0.0,
                feather: 0.0,
                invert: false,
                points: vec![[0.3, 0.5], [0.5, 0.3], [0.7, 0.5], [0.5, 0.7]],
            };
        }

        // Downsample contour points uniformly to max_pts
        let step = (contour_points.len() as f32 / max_pts as f32).max(1.0);
        let mut sampled_points = Vec::with_capacity(max_pts);
        let mut i = 0.0f32;
        while (i as usize) < contour_points.len() && sampled_points.len() < max_pts {
            sampled_points.push(contour_points[i as usize]);
            i += step;
        }

        while sampled_points.len() < MIN_POINTS {
            sampled_points.push([0.5, 0.5]);
        }

        FxMask {
            shape: MaskShape::Bezier,
            center: [0.5, 0.5],
            size: [1.0, 1.0],
            rotation_deg: 0.0,
            feather: 0.0,
            invert: false,
            points: sampled_points,
        }
    }

    /// Multiplies the frame buffer alpha channel by this matte.
    pub fn apply_to_frame(&self, frame: &mut FrameBuffer, invert: bool) {
        if frame.width != self.width || frame.height != self.height {
            return;
        }

        for (px, &alpha_val) in frame.rgba.as_chunks_mut::<4>().0.iter_mut().zip(self.alpha_channel.iter()) {
            let factor = if invert {
                255 - alpha_val
            } else {
                alpha_val
            };
            px[3] = ((f32::from(px[3]) * f32::from(factor)) / 255.0).round() as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smart_mask_prompt_segmentation() {
        let mut frame = FrameBuffer::solid(32, 32, [50, 50, 50, 255]);
        // Draw green object inside [8..24, 8..24]
        for y in 8..24 {
            for x in 8..24 {
                let idx = (y * 32 + x) * 4;
                frame.rgba[idx] = 0;
                frame.rgba[idx + 1] = 200;
                frame.rgba[idx + 2] = 0;
            }
        }

        let prompt = SegmentationPrompt {
            bounding_box: Some([0.2, 0.2, 0.6, 0.6]),
            positive_points: vec![[0.5, 0.5]],
            negative_points: vec![[0.05, 0.05]],
        };

        let matte = SmartMaskMatte::segment_frame(&frame, &prompt, SmartMaskPrecision::High);
        assert_eq!(matte.width, 32);
        assert_eq!(matte.height, 32);
        // Center pixel (16, 16) must be foreground
        assert_eq!(matte.alpha_channel[16 * 32 + 16], 255);
        // Corner pixel (0, 0) must be background
        assert_eq!(matte.alpha_channel[0], 0);
    }

    #[test]
    fn test_smart_mask_vectorize_to_bezier() {
        let frame = FrameBuffer::solid(16, 16, [255, 255, 255, 255]);
        let matte = SmartMaskMatte::cpu_fallback_segment(&frame, 128);
        let bezier = matte.vectorize_to_bezier(8);
        assert_eq!(bezier.shape, MaskShape::Bezier);
        assert!(bezier.points.len() >= MIN_POINTS);
        assert!(bezier.points.len() <= MAX_POINTS);
    }

    #[test]
    fn test_smart_mask_apply_to_frame() {
        let mut frame = FrameBuffer::solid(4, 4, [255, 255, 255, 255]);
        let matte = SmartMaskMatte {
            width: 4,
            height: 4,
            alpha_channel: vec![
                255, 255, 0, 0,
                255, 255, 0, 0,
                0, 0, 0, 0,
                0, 0, 0, 0,
            ],
        };

        matte.apply_to_frame(&mut frame, false);
        assert_eq!(frame.pixel(0, 0)[3], 255);
        assert_eq!(frame.pixel(3, 3)[3], 0);
    }
}
