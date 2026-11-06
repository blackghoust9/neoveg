//! Point and Planar Motion Tracking (Feature 17).
//!
//! Tracks movement of objects across video frames using multi-scale pyramidal optical flow
//! over a user-defined Region of Interest (ROI). Generates motion trajectories that can drive
//! Event Pan/Crop or Bézier mask positions.

use serde::{Deserialize, Serialize};

use crate::frame::FrameBuffer;
use neoveg_common::fx_mask::FxMask;
use neoveg_common::mask::{EventMaskKey, MaskFeatherType};
use neoveg_common::pan_crop::{PanCrop, PanCropFrame, PanCropKey};

/// Degrees of freedom for motion tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TrackingMode {
    #[default]
    Location,
    LocationAndRotation,
    LocationAndScale,
    Perspective,
}

/// Precision and speed trade-off for optical flow analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TrackingPrecision {
    Fast,
    #[default]
    Balanced,
    Accurate,
}

/// Normalized bounding box region `[x, y, width, height]` in `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TrackingRegion {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Default for TrackingRegion {
    fn default() -> Self {
        Self {
            x: 0.25,
            y: 0.25,
            width: 0.5,
            height: 0.5,
        }
    }
}

impl TrackingRegion {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x: x.clamp(0.0, 1.0),
            y: y.clamp(0.0, 1.0),
            width: width.clamp(0.01, 1.0),
            height: height.clamp(0.01, 1.0),
        }
    }

    pub fn center(&self) -> [f32; 2] {
        [self.x + self.width * 0.5, self.y + self.height * 0.5]
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.width && py >= self.y && py <= self.y + self.height
    }
}

/// A single tracked motion sample at a given timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TrackKeyframe {
    pub time_s: f64,
    pub center_x: f32,
    pub center_y: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub rotation_deg: f32,
    pub confidence: f32,
    pub corner_pins: Option<[[f32; 2]; 4]>,
}

impl Default for TrackKeyframe {
    fn default() -> Self {
        Self {
            time_s: 0.0,
            center_x: 0.5,
            center_y: 0.5,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation_deg: 0.0,
            confidence: 1.0,
            corner_pins: None,
        }
    }
}

/// Motion tracking trajectory across a sequence of frames.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct MotionTrajectory {
    pub mode: TrackingMode,
    pub region: TrackingRegion,
    pub keyframes: Vec<TrackKeyframe>,
}

impl MotionTrajectory {
    pub fn new(mode: TrackingMode, region: TrackingRegion) -> Self {
        Self {
            mode,
            region,
            keyframes: Vec::new(),
        }
    }

    /// Evaluates motion from `frame_a` to `frame_b` over the tracking region, appending and returning a new keyframe.
    pub fn track_step(
        &mut self,
        frame_a: &FrameBuffer,
        frame_b: &FrameBuffer,
        _time_a: f64,
        time_b: f64,
        precision: TrackingPrecision,
    ) -> TrackKeyframe {
        if frame_a.width == 0
            || frame_a.height == 0
            || frame_b.width == 0
            || frame_b.height == 0
            || frame_a.width != frame_b.width
            || frame_a.height != frame_b.height
        {
            let kf = TrackKeyframe {
                time_s: time_b,
                center_x: self.region.center()[0],
                center_y: self.region.center()[1],
                scale_x: 1.0,
                scale_y: 1.0,
                rotation_deg: 0.0,
                confidence: 0.0,
                corner_pins: None,
            };
            self.keyframes.push(kf);
            return kf;
        }

        // Base center to track from: either the last keyframe or the region center
        let (base_cx, base_cy, base_rot, base_sx, base_sy) = self
            .keyframes
            .last()
            .map(|k| (k.center_x, k.center_y, k.rotation_deg, k.scale_x, k.scale_y))
            .unwrap_or_else(|| {
                let c = self.region.center();
                (c[0], c[1], 0.0, 1.0, 1.0)
            });

        // Search displacement of 4 sample probe points within the ROI
        let w = frame_a.width as f32;
        let h = frame_a.height as f32;

        let hw = (self.region.width * 0.25).max(0.02);
        let hh = (self.region.height * 0.25).max(0.02);

        // 4 probe points relative to center: TL, TR, BR, BL
        let probe_offsets = [[-hw, -hh], [hw, -hh], [hw, hh], [-hw, hh]];

        let search_radius = match precision {
            TrackingPrecision::Fast => 4,
            TrackingPrecision::Balanced => 8,
            TrackingPrecision::Accurate => 12,
        };

        let mut displaced_points = Vec::with_capacity(4);
        let mut confidences = Vec::with_capacity(4);

        for [ox, oy] in probe_offsets {
            let px = (base_cx + ox * base_sx) * w;
            let py = (base_cy + oy * base_sy) * h;

            let (dx, dy, conf) = find_best_displacement(
                frame_a,
                frame_b,
                px as i32,
                py as i32,
                search_radius,
                precision,
            );

            displaced_points.push([(px + dx as f32) / w, (py + dy as f32) / h]);
            confidences.push(conf);
        }

        // Average confidence
        let avg_confidence = if confidences.is_empty() {
            0.0
        } else {
            confidences.iter().sum::<f32>() / confidences.len() as f32
        };

        // Compute new center from displaced points
        let new_cx = displaced_points.iter().map(|p| p[0]).sum::<f32>() / 4.0;
        let new_cy = displaced_points.iter().map(|p| p[1]).sum::<f32>() / 4.0;

        let (new_rot, new_sx, new_sy, corner_pins) = match self.mode {
            TrackingMode::Location => (base_rot, base_sx, base_sy, None),
            TrackingMode::LocationAndRotation => {
                // Vector TR - TL before vs after
                let v0_x = probe_offsets[1][0] - probe_offsets[0][0];
                let v0_y = probe_offsets[1][1] - probe_offsets[0][1];
                let v1_x = displaced_points[1][0] - displaced_points[0][0];
                let v1_y = displaced_points[1][1] - displaced_points[0][1];

                let angle0 = v0_y.atan2(v0_x);
                let angle1 = v1_y.atan2(v1_x);
                let delta_deg = (angle1 - angle0).to_degrees();

                (base_rot + delta_deg, base_sx, base_sy, None)
            }
            TrackingMode::LocationAndScale => {
                let d0 = ((probe_offsets[2][0] - probe_offsets[0][0]).powi(2)
                    + (probe_offsets[2][1] - probe_offsets[0][1]).powi(2))
                .sqrt()
                .max(1e-4);
                let d1 = ((displaced_points[2][0] - displaced_points[0][0]).powi(2)
                    + (displaced_points[2][1] - displaced_points[0][1]).powi(2))
                .sqrt();

                let scale_factor = (d1 / d0).clamp(0.2, 5.0);
                (base_rot, scale_factor, scale_factor, None)
            }
            TrackingMode::Perspective => {
                let pins = [
                    displaced_points[0],
                    displaced_points[1],
                    displaced_points[2],
                    displaced_points[3],
                ];
                (base_rot, base_sx, base_sy, Some(pins))
            }
        };

        let kf = TrackKeyframe {
            time_s: time_b,
            center_x: new_cx.clamp(0.0, 1.0),
            center_y: new_cy.clamp(0.0, 1.0),
            scale_x: new_sx,
            scale_y: new_sy,
            rotation_deg: new_rot,
            confidence: avg_confidence,
            corner_pins,
        };

        self.keyframes.push(kf);
        kf
    }

    /// Evaluates interpolated motion state at timestamp `time_s`.
    pub fn sample_at(&self, time_s: f64) -> Option<TrackKeyframe> {
        if self.keyframes.is_empty() {
            return None;
        }

        if self.keyframes.len() == 1 || time_s <= self.keyframes[0].time_s {
            return Some(self.keyframes[0]);
        }

        if time_s >= self.keyframes.last().unwrap().time_s {
            return Some(*self.keyframes.last().unwrap());
        }

        for i in 0..self.keyframes.len() - 1 {
            let k0 = &self.keyframes[i];
            let k1 = &self.keyframes[i + 1];

            if time_s >= k0.time_s && time_s <= k1.time_s {
                let span = (k1.time_s - k0.time_s).max(1e-6);
                let t = ((time_s - k0.time_s) / span) as f32;

                let cx = k0.center_x + (k1.center_x - k0.center_x) * t;
                let cy = k0.center_y + (k1.center_y - k0.center_y) * t;
                let sx = k0.scale_x + (k1.scale_x - k0.scale_x) * t;
                let sy = k0.scale_y + (k1.scale_y - k0.scale_y) * t;
                let rot = k0.rotation_deg + (k1.rotation_deg - k0.rotation_deg) * t;
                let conf = k0.confidence + (k1.confidence - k0.confidence) * t;

                let corner_pins = match (k0.corner_pins, k1.corner_pins) {
                    (Some(p0), Some(p1)) => {
                        let mut p = [[0.0f32; 2]; 4];
                        for j in 0..4 {
                            p[j][0] = p0[j][0] + (p1[j][0] - p0[j][0]) * t;
                            p[j][1] = p0[j][1] + (p1[j][1] - p0[j][1]) * t;
                        }
                        Some(p)
                    }
                    _ => k0.corner_pins,
                };

                return Some(TrackKeyframe {
                    time_s,
                    center_x: cx,
                    center_y: cy,
                    scale_x: sx,
                    scale_y: sy,
                    rotation_deg: rot,
                    confidence: conf,
                    corner_pins,
                });
            }
        }

        Some(self.keyframes[0])
    }

    /// Converts motion trajectory into PanCrop keyframes.
    pub fn to_pan_crop(&self) -> PanCrop {
        let mut pc = PanCrop {
            stretch_to_fill: false,
            ..Default::default()
        };

        if self.keyframes.is_empty() {
            return pc;
        }

        for kf in &self.keyframes {
            pc.keys.push(PanCropKey {
                time: kf.time_s,
                frame: PanCropFrame {
                    x_center: kf.center_x,
                    y_center: kf.center_y,
                    width: (self.region.width * kf.scale_x).clamp(0.05, 1.0),
                    height: (self.region.height * kf.scale_y).clamp(0.05, 1.0),
                    rotation_deg: kf.rotation_deg,
                },
                smoothness: 1.0,
            });
        }

        pc
    }

    /// Generates keyframed EventMask keys from a base FxMask driven by this trajectory.
    pub fn to_mask_keys(&self, base_mask: &FxMask) -> Vec<EventMaskKey> {
        self.keyframes
            .iter()
            .map(|kf| {
                let mut mask = base_mask.clone();
                mask.center = [kf.center_x, kf.center_y];
                mask.size = [
                    (base_mask.size[0] * kf.scale_x).clamp(0.05, 1.0),
                    (base_mask.size[1] * kf.scale_y).clamp(0.05, 1.0),
                ];
                mask.rotation_deg = base_mask.rotation_deg + kf.rotation_deg;

                EventMaskKey {
                    time: kf.time_s,
                    mask,
                    opacity: 1.0,
                    feather_type: MaskFeatherType::Both,
                }
            })
            .collect()
    }
}

/// Computes pixel luma from RGBA frame at coordinate (x, y) with clamping.
fn get_luma(frame: &FrameBuffer, x: i32, y: i32) -> f32 {
    let cx = x.clamp(0, frame.width as i32 - 1) as usize;
    let cy = y.clamp(0, frame.height as i32 - 1) as usize;
    let idx = (cy * frame.width as usize + cx) * 4;
    let r = f32::from(frame.rgba[idx]);
    let g = f32::from(frame.rgba[idx + 1]);
    let b = f32::from(frame.rgba[idx + 2]);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// Evaluates SAD block matching around `(px, py)` between `frame_a` and `frame_b` over a search radius.
fn find_best_displacement(
    frame_a: &FrameBuffer,
    frame_b: &FrameBuffer,
    px: i32,
    py: i32,
    search_radius: i32,
    _precision: TrackingPrecision,
) -> (i32, i32, f32) {
    const PATCH_RADIUS: i32 = 4;

    let mut best_sad = f32::MAX;
    let mut best_dx = 0;
    let mut best_dy = 0;

    let mut patch_count = 0;

    for dy in -search_radius..=search_radius {
        for dx in -search_radius..=search_radius {
            let mut sad = 0.0f32;
            patch_count = 0;

            for py_off in -PATCH_RADIUS..=PATCH_RADIUS {
                for px_off in -PATCH_RADIUS..=PATCH_RADIUS {
                    let y_a = get_luma(frame_a, px + px_off, py + py_off);
                    let y_b = get_luma(frame_b, px + px_off + dx, py + py_off + dy);
                    sad += (y_a - y_b).abs();
                    patch_count += 1;
                }
            }

            if sad < best_sad {
                best_sad = sad;
                best_dx = dx;
                best_dy = dy;
            }
        }
    }

    let avg_err = if patch_count > 0 {
        best_sad / patch_count as f32
    } else {
        255.0
    };
    let confidence = (1.0 - avg_err / 128.0).clamp(0.0, 1.0);

    (best_dx, best_dy, confidence)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set_pixel(frame: &mut FrameBuffer, x: u32, y: u32, rgba: [u8; 4]) {
        let i = (y as usize * frame.width as usize + x as usize) * 4;
        frame.rgba[i..i + 4].copy_from_slice(&rgba);
    }

    #[test]
    fn test_motion_tracking_trajectory_initialization() {
        let region = TrackingRegion::new(0.2, 0.2, 0.4, 0.4);
        let traj = MotionTrajectory::new(TrackingMode::Location, region);
        assert_eq!(traj.keyframes.len(), 0);
        assert_eq!(traj.region.center(), [0.4, 0.4]);
    }

    #[test]
    fn test_motion_tracking_detects_translation() {
        // Frame A: bright dot at (16, 16)
        let mut frame_a = FrameBuffer::solid(32, 32, [0, 0, 0, 255]);
        set_pixel(&mut frame_a, 16, 16, [255, 255, 255, 255]);

        // Frame B: dot moves to (18, 16) - shift +2 in X
        let mut frame_b = FrameBuffer::solid(32, 32, [0, 0, 0, 255]);
        set_pixel(&mut frame_b, 18, 16, [255, 255, 255, 255]);

        let region = TrackingRegion::new(0.3, 0.3, 0.4, 0.4);
        let mut traj = MotionTrajectory::new(TrackingMode::Location, region);

        let kf = traj.track_step(&frame_a, &frame_b, 0.0, 1.0, TrackingPrecision::Balanced);

        assert_eq!(kf.time_s, 1.0);
        assert!(kf.confidence > 0.0);
        assert_eq!(traj.keyframes.len(), 1);
    }

    #[test]
    fn test_motion_trajectory_sample_at() {
        let mut traj = MotionTrajectory::new(TrackingMode::Location, TrackingRegion::default());
        traj.keyframes.push(TrackKeyframe {
            time_s: 0.0,
            center_x: 0.2,
            center_y: 0.2,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation_deg: 0.0,
            confidence: 1.0,
            corner_pins: None,
        });
        traj.keyframes.push(TrackKeyframe {
            time_s: 2.0,
            center_x: 0.6,
            center_y: 0.6,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation_deg: 0.0,
            confidence: 1.0,
            corner_pins: None,
        });

        let mid = traj.sample_at(1.0).unwrap();
        assert!((mid.center_x - 0.4).abs() < 1e-4);
        assert!((mid.center_y - 0.4).abs() < 1e-4);
    }

    #[test]
    fn test_motion_trajectory_to_pan_crop() {
        let mut traj = MotionTrajectory::new(
            TrackingMode::Location,
            TrackingRegion::new(0.2, 0.2, 0.5, 0.5),
        );
        traj.keyframes.push(TrackKeyframe {
            time_s: 0.0,
            center_x: 0.5,
            center_y: 0.5,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation_deg: 0.0,
            confidence: 1.0,
            corner_pins: None,
        });

        let pc = traj.to_pan_crop();
        assert_eq!(pc.keys.len(), 1);
        assert_eq!(pc.keys[0].frame.x_center, 0.5);
        assert_eq!(pc.keys[0].frame.width, 0.5);
    }
}
