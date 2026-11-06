//! Smart Upscale & Super-Resolution (Feature 22).
//!
//! Provides high-fidelity pure-CPU image upscaling and super-resolution, including
//! bilinear, 16-tap Catmull-Rom bicubic, directional edge-adaptive (NEDI), and
//! detail-preserving super-resolution with adaptive unsharp masking.

use serde::{Deserialize, Serialize};

use crate::frame::FrameBuffer;

/// Upscaling algorithm selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum UpscaleAlgorithm {
    Bilinear,
    #[default]
    Bicubic,
    EdgeDirected,
    SuperResolutionAi,
}

/// Configuration settings for frame upscaling.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct UpscaleConfig {
    pub algorithm: UpscaleAlgorithm,
    pub target_width: u32,
    pub target_height: u32,
    /// Sharpness and detail enhancement level `0.0..=1.0`.
    pub sharpness: f32,
}

impl Default for UpscaleConfig {
    fn default() -> Self {
        Self {
            algorithm: UpscaleAlgorithm::Bicubic,
            target_width: 1920,
            target_height: 1080,
            sharpness: 0.5,
        }
    }
}

pub struct FrameUpscaler;

impl FrameUpscaler {
    /// Resamples `src` into a new `FrameBuffer` matching `config.target_width` and `config.target_height`.
    pub fn upscale(src: &FrameBuffer, config: &UpscaleConfig) -> FrameBuffer {
        let (dw, dh) = (config.target_width, config.target_height);
        if dw == 0 || dh == 0 || src.width == 0 || src.height == 0 {
            return FrameBuffer::solid(dw, dh, [0, 0, 0, 0]);
        }

        if src.width == dw && src.height == dh {
            return src.clone();
        }

        let (sw, sh) = (src.width as f32, src.height as f32);
        let scale_x = sw / dw as f32;
        let scale_y = sh / dh as f32;

        let total_pixels = (dw * dh) as usize;
        let mut out_rgba = Vec::with_capacity(total_pixels * 4);

        match config.algorithm {
            UpscaleAlgorithm::Bilinear => {
                for y in 0..dh {
                    let sy = (y as f32 + 0.5) * scale_y - 0.5;
                    for x in 0..dw {
                        let sx = (x as f32 + 0.5) * scale_x - 0.5;
                        let px = sample_bilinear(src, sx, sy);
                        out_rgba.extend_from_slice(&px);
                    }
                }
            }
            UpscaleAlgorithm::Bicubic => {
                for y in 0..dh {
                    let sy = (y as f32 + 0.5) * scale_y - 0.5;
                    for x in 0..dw {
                        let sx = (x as f32 + 0.5) * scale_x - 0.5;
                        let px = sample_bicubic(src, sx, sy);
                        out_rgba.extend_from_slice(&px);
                    }
                }
            }
            UpscaleAlgorithm::EdgeDirected => {
                for y in 0..dh {
                    let sy = (y as f32 + 0.5) * scale_y - 0.5;
                    for x in 0..dw {
                        let sx = (x as f32 + 0.5) * scale_x - 0.5;
                        let px = sample_edge_directed(src, sx, sy);
                        out_rgba.extend_from_slice(&px);
                    }
                }
            }
            UpscaleAlgorithm::SuperResolutionAi => {
                // First pass: edge-directed directional interpolation
                for y in 0..dh {
                    let sy = (y as f32 + 0.5) * scale_y - 0.5;
                    for x in 0..dw {
                        let sx = (x as f32 + 0.5) * scale_x - 0.5;
                        let px = sample_edge_directed(src, sx, sy);
                        out_rgba.extend_from_slice(&px);
                    }
                }

                // Second pass: adaptive unsharp masking detail enhancement
                if config.sharpness > 0.01 {
                    apply_adaptive_unsharp(&mut out_rgba, dw, dh, config.sharpness);
                }
            }
        }

        FrameBuffer {
            width: dw,
            height: dh,
            rgba: out_rgba,
        }
    }
}

/// Clamped pixel fetch helper.
fn get_px(src: &FrameBuffer, x: i32, y: i32) -> [f32; 4] {
    let cx = x.clamp(0, src.width as i32 - 1) as usize;
    let cy = y.clamp(0, src.height as i32 - 1) as usize;
    let idx = (cy * src.width as usize + cx) * 4;
    [
        f32::from(src.rgba[idx]),
        f32::from(src.rgba[idx + 1]),
        f32::from(src.rgba[idx + 2]),
        f32::from(src.rgba[idx + 3]),
    ]
}

/// Bilinear sampling.
fn sample_bilinear(src: &FrameBuffer, x: f32, y: f32) -> [u8; 4] {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let u = x - x.floor();
    let v = y - y.floor();

    let p00 = get_px(src, x0, y0);
    let p10 = get_px(src, x0 + 1, y0);
    let p01 = get_px(src, x0, y0 + 1);
    let p11 = get_px(src, x0 + 1, y0 + 1);

    let w00 = (1.0 - u) * (1.0 - v);
    let w10 = u * (1.0 - v);
    let w01 = (1.0 - u) * v;
    let w11 = u * v;

    let mut out = [0u8; 4];
    for c in 0..4 {
        let val = p00[c] * w00 + p10[c] * w10 + p01[c] * w01 + p11[c] * w11;
        out[c] = val.round().clamp(0.0, 255.0) as u8;
    }
    out
}

/// 1D Catmull-Rom cubic spline kernel weight.
#[inline]
pub fn cubic_weight(x: f32) -> f32 {
    let ax = x.abs();
    if ax <= 1.0 {
        1.5 * ax * ax * ax - 2.5 * ax * ax + 1.0
    } else if ax < 2.0 {
        -0.5 * ax * ax * ax + 2.5 * ax * ax - 4.0 * ax + 2.0
    } else {
        0.0
    }
}

/// 16-tap Catmull-Rom bicubic sampling.
fn sample_bicubic(src: &FrameBuffer, x: f32, y: f32) -> [u8; 4] {
    let x_floor = x.floor() as i32;
    let y_floor = y.floor() as i32;
    let tx = x - x.floor();
    let ty = y - y.floor();

    let mut acc = [0.0f32; 4];
    let mut total_weight = 0.0f32;

    for m in -1..=2 {
        let wy = cubic_weight(m as f32 - ty);
        for n in -1..=2 {
            let wx = cubic_weight(n as f32 - tx);
            let w = wx * wy;
            let px = get_px(src, x_floor + n, y_floor + m);
            for c in 0..4 {
                acc[c] += px[c] * w;
            }
            total_weight += w;
        }
    }

    let inv = if total_weight.abs() > 1e-5 {
        1.0 / total_weight
    } else {
        1.0
    };

    let mut out = [0u8; 4];
    for c in 0..4 {
        out[c] = (acc[c] * inv).round().clamp(0.0, 255.0) as u8;
    }
    out
}

/// Directional edge-adaptive (NEDI) sampling along local image gradients.
fn sample_edge_directed(src: &FrameBuffer, x: f32, y: f32) -> [u8; 4] {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;

    let p00 = get_px(src, x0, y0);
    let p10 = get_px(src, x0 + 1, y0);
    let p01 = get_px(src, x0, y0 + 1);
    let p11 = get_px(src, x0 + 1, y0 + 1);

    // Compute luminance
    let luma = |p: [f32; 4]| 0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2];
    let y00 = luma(p00);
    let y10 = luma(p10);
    let y01 = luma(p01);
    let y11 = luma(p11);

    // Diagonal gradients
    let d_diag1 = (y00 - y11).abs();
    let d_diag2 = (y10 - y01).abs();

    if (d_diag1 - d_diag2).abs() > 15.0 {
        let mut out = [0u8; 4];
        if d_diag1 < d_diag2 {
            // Interpolate along diagonal 1 (0,0 to 1,1)
            for c in 0..4 {
                out[c] = ((p00[c] + p11[c]) * 0.5).round().clamp(0.0, 255.0) as u8;
            }
        } else {
            // Interpolate along diagonal 2 (1,0 to 0,1)
            for c in 0..4 {
                out[c] = ((p10[c] + p01[c]) * 0.5).round().clamp(0.0, 255.0) as u8;
            }
        }
        out
    } else {
        sample_bicubic(src, x, y)
    }
}

/// In-place adaptive unsharp masking filter for high-frequency edge crispness.
fn apply_adaptive_unsharp(rgba: &mut [u8], w: u32, h: u32, sharpness: f32) {
    let k = (sharpness * 0.6).clamp(0.0, 0.8);
    let stride = w as usize * 4;

    // Fast 3x3 box blur approximation on scanlines
    let copy = rgba.to_vec();

    for y in 1..h as usize - 1 {
        let y_prev = (y - 1) * stride;
        let y_curr = y * stride;
        let y_next = (y + 1) * stride;

        for x in 1..w as usize - 1 {
            let idx = y_curr + x * 4;

            for c in 0..3 {
                let orig = f32::from(copy[idx + c]);
                let sum = f32::from(copy[y_prev + (x - 1) * 4 + c])
                    + f32::from(copy[y_prev + x * 4 + c])
                    + f32::from(copy[y_prev + (x + 1) * 4 + c])
                    + f32::from(copy[y_curr + (x - 1) * 4 + c])
                    + orig
                    + f32::from(copy[y_curr + (x + 1) * 4 + c])
                    + f32::from(copy[y_next + (x - 1) * 4 + c])
                    + f32::from(copy[y_next + x * 4 + c])
                    + f32::from(copy[y_next + (x + 1) * 4 + c]);

                let blur = sum / 9.0;
                let high_freq = (orig - blur).clamp(-30.0, 30.0);
                rgba[idx + c] = (orig + k * high_freq).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cubic_weight_math() {
        assert_eq!(cubic_weight(0.0), 1.0);
        assert_eq!(cubic_weight(1.0), 0.0);
        assert_eq!(cubic_weight(2.0), 0.0);
    }

    #[test]
    fn test_upscale_identity() {
        let frame = FrameBuffer::solid(16, 16, [100, 150, 200, 255]);
        let config = UpscaleConfig {
            algorithm: UpscaleAlgorithm::Bicubic,
            target_width: 16,
            target_height: 16,
            sharpness: 0.5,
        };
        let out = FrameUpscaler::upscale(&frame, &config);
        assert_eq!(out, frame);
    }

    #[test]
    fn test_upscale_2x_resolution() {
        let frame = FrameBuffer::solid(8, 8, [50, 100, 150, 255]);
        let config = UpscaleConfig {
            algorithm: UpscaleAlgorithm::SuperResolutionAi,
            target_width: 16,
            target_height: 16,
            sharpness: 0.5,
        };
        let out = FrameUpscaler::upscale(&frame, &config);
        assert_eq!(out.width, 16);
        assert_eq!(out.height, 16);
        assert_eq!(out.rgba.len(), 16 * 16 * 4);
    }

    #[test]
    fn test_upscale_zero_dimensions_handled() {
        let frame = FrameBuffer::solid(10, 10, [0, 0, 0, 0]);
        let config = UpscaleConfig {
            algorithm: UpscaleAlgorithm::Bilinear,
            target_width: 0,
            target_height: 10,
            sharpness: 0.0,
        };
        let out = FrameUpscaler::upscale(&frame, &config);
        assert_eq!(out.width, 0);
        assert_eq!(out.height, 10);
    }
}
