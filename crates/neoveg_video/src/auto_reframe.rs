//! AI Auto Reframe (Feature 23).
//!
//! Automatically converts widescreen (16:9) video into vertical (9:16), square (1:1), or
//! social portrait (4:5) aspect ratios by analyzing subject saliency and planning
//! smooth, deadband-filtered Pan/Crop trajectories.

use serde::{Deserialize, Serialize};

use crate::frame::FrameBuffer;
use neoveg_common::pan_crop::{PanCrop, PanCropFrame, PanCropKey};

/// Target aspect ratio format for automated social reframing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TargetAspectRatio {
    Ratio16_9,
    #[default]
    Ratio9_16,
    Ratio1_1,
    Ratio4_5,
    Custom(u32, u32),
}

impl TargetAspectRatio {
    pub fn aspect_ratio_f32(&self) -> f32 {
        match self {
            Self::Ratio16_9 => 16.0 / 9.0,
            Self::Ratio9_16 => 9.0 / 16.0,
            Self::Ratio1_1 => 1.0,
            Self::Ratio4_5 => 4.0 / 5.0,
            Self::Custom(w, h) => (*w as f32) / (*h as f32).max(1.0),
        }
    }
}

/// Primary subject location detected in a single frame.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SubjectDetection {
    pub time_s: f64,
    /// Normalized center-X of subject `0.0..=1.0`.
    pub center_x: f32,
    /// Normalized center-Y of subject `0.0..=1.0`.
    pub center_y: f32,
    /// Confidence score `0.0..=1.0`.
    pub confidence: f32,
}

pub struct AutoReframeEngine;

impl AutoReframeEngine {
    /// Detects primary visual subject centroid via local luma variance and center-weighted saliency.
    pub fn detect_subject(frame: &FrameBuffer, time_s: f64) -> SubjectDetection {
        let (w, h) = (frame.width, frame.height);
        if w == 0 || h == 0 {
            return SubjectDetection {
                time_s,
                center_x: 0.5,
                center_y: 0.5,
                confidence: 0.0,
            };
        }

        // Subsample grid to 32x18 for rapid pure-CPU analysis
        let grid_w = 32usize;
        let grid_h = 18usize;

        let step_x = (w as f32 / grid_w as f32).max(1.0);
        let step_y = (h as f32 / grid_h as f32).max(1.0);

        let mut sum_saliency = 0.0f32;
        let mut sum_x = 0.0f32;
        let mut sum_y = 0.0f32;

        for gy in 0..grid_h {
            let py = ((gy as f32 + 0.5) * step_y) as usize;
            let ny = py as f32 / h as f32;

            for gx in 0..grid_w {
                let px = ((gx as f32 + 0.5) * step_x) as usize;
                let nx = px as f32 / w as f32;

                let idx = (py.min(h as usize - 1) * w as usize + px.min(w as usize - 1)) * 4;
                let r = f32::from(frame.rgba[idx]);
                let g = f32::from(frame.rgba[idx + 1]);
                let b = f32::from(frame.rgba[idx + 2]);
                let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;

                // Center-prior Gaussian weight
                let center_dist_sq = (nx - 0.5).powi(2) + (ny - 0.5).powi(2);
                let center_weight = (-center_dist_sq / 0.18).exp();

                let sal = (luma + 10.0) * center_weight;
                sum_saliency += sal;
                sum_x += nx * sal;
                sum_y += ny * sal;
            }
        }

        if sum_saliency > 1e-4 {
            let cx = (sum_x / sum_saliency).clamp(0.0, 1.0);
            let cy = (sum_y / sum_saliency).clamp(0.0, 1.0);
            SubjectDetection {
                time_s,
                center_x: cx,
                center_y: cy,
                confidence: 0.9,
            }
        } else {
            SubjectDetection {
                time_s,
                center_x: 0.5,
                center_y: 0.5,
                confidence: 0.5,
            }
        }
    }

    /// Plans smoothed Pan/Crop keyframes maintaining the subject within the target aspect ratio window.
    pub fn plan_reframe_trajectory(
        detections: &[SubjectDetection],
        source_aspect: f32,
        target: TargetAspectRatio,
        deadzone_threshold: f32,
        smoothness: f32,
    ) -> PanCrop {
        let mut pc = PanCrop {
            stretch_to_fill: false,
            ..Default::default()
        };

        if detections.is_empty() {
            return pc;
        }

        let target_aspect = target.aspect_ratio_f32();
        let src_aspect = if source_aspect > 1e-3 {
            source_aspect
        } else {
            16.0 / 9.0
        };

        // Determine crop window size in normalized source coords
        let (crop_w, crop_h) = if target_aspect <= src_aspect {
            let w = (target_aspect / src_aspect).clamp(0.05, 1.0);
            (w, 1.0f32)
        } else {
            let h = (src_aspect / target_aspect).clamp(0.05, 1.0);
            (1.0f32, h)
        };

        let half_w = crop_w * 0.5;
        let deadband = (crop_w * deadzone_threshold.clamp(0.05, 0.4)).max(0.02);

        let mut current_cam_x = detections[0].center_x.clamp(half_w, 1.0 - half_w);

        // First keyframe at start
        pc.keys.push(PanCropKey {
            time: detections[0].time_s,
            frame: PanCropFrame {
                x_center: current_cam_x,
                y_center: 0.5,
                width: crop_w,
                height: crop_h,
                rotation_deg: 0.0,
            },
            smoothness,
        });

        let mut last_kf_time = detections[0].time_s;

        for det in detections.iter().skip(1) {
            let target_x = det.center_x.clamp(half_w, 1.0 - half_w);
            let diff = target_x - current_cam_x;

            // Only track if subject leaves deadband region and min time elapsed
            if diff.abs() > deadband && (det.time_s - last_kf_time) >= 0.3 {
                current_cam_x = target_x;
                pc.keys.push(PanCropKey {
                    time: det.time_s,
                    frame: PanCropFrame {
                        x_center: current_cam_x,
                        y_center: 0.5,
                        width: crop_w,
                        height: crop_h,
                        rotation_deg: 0.0,
                    },
                    smoothness,
                });
                last_kf_time = det.time_s;
            }
        }

        pc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_aspect_ratio_values() {
        assert!((TargetAspectRatio::Ratio16_9.aspect_ratio_f32() - 16.0 / 9.0).abs() < 1e-4);
        assert!((TargetAspectRatio::Ratio9_16.aspect_ratio_f32() - 9.0 / 16.0).abs() < 1e-4);
        assert!((TargetAspectRatio::Ratio1_1.aspect_ratio_f32() - 1.0).abs() < 1e-4);
        assert!((TargetAspectRatio::Ratio4_5.aspect_ratio_f32() - 0.8).abs() < 1e-4);
    }

    #[test]
    fn test_detect_subject_saliency() {
        let mut frame = FrameBuffer::solid(32, 18, [10, 10, 10, 255]);
        // Set bright subject at right side
        for y in 4..14 {
            for x in 22..30 {
                let idx = (y * 32 + x) * 4;
                frame.rgba[idx] = 250;
                frame.rgba[idx + 1] = 250;
                frame.rgba[idx + 2] = 250;
            }
        }

        let det = AutoReframeEngine::detect_subject(&frame, 1.0);
        assert_eq!(det.time_s, 1.0);
        assert!(det.center_x > 0.5, "Centroid should be pulled to the right side");
    }

    #[test]
    fn test_plan_reframe_trajectory_deadband() {
        let detections = vec![
            SubjectDetection {
                time_s: 0.0,
                center_x: 0.5,
                center_y: 0.5,
                confidence: 1.0,
            },
            // Micro-movement within deadband
            SubjectDetection {
                time_s: 0.5,
                center_x: 0.52,
                center_y: 0.5,
                confidence: 1.0,
            },
            // Movement exiting deadband
            SubjectDetection {
                time_s: 1.0,
                center_x: 0.85,
                center_y: 0.5,
                confidence: 1.0,
            },
        ];

        let pc = AutoReframeEngine::plan_reframe_trajectory(
            &detections,
            16.0 / 9.0,
            TargetAspectRatio::Ratio9_16,
            0.15,
            1.0,
        );

        // Keyframe for 0.0 and keyframe for 1.0, but micro-movement at 0.5 filtered out!
        assert_eq!(pc.keys.len(), 2);
        assert_eq!(pc.keys[0].time, 0.0);
        assert_eq!(pc.keys[1].time, 1.0);
    }
}
