//! Zoom-adaptive timeline frame grid.
//!
//! - Close zoom: one minor line per frame (1/24, 1/25, 1/30, 1/60 s... from the project FPS).
//! - Medium/far zoom: lines grouped into 2/5/10 frames, then 1 s, 2 s, 5 s, 10 s, 15 s, 30 s, 1 min, 2 min, 5 min...
//! - Major lines carry the ruler labels; minor lines are the subtle subdivisions between them.
//!
//! Grid times are always derived from integer frame/step indices (never accumulated) so lines stay on
//! exact frame boundaries at any zoom or scroll position.

use neoveg_common::timecode::{nominal_fps, timecode_seconds_to_frame};

pub const MIN_MINOR_SPACING_PX: f64 = 8.0;
pub const MIN_MAJOR_SPACING_PX: f64 = 90.0;
/// Same lower bound the timeline uses when converting time to x.
const MIN_PIXELS_PER_SECOND: f64 = 20.0;
const MAX_LINES_PER_LIST: usize = 4096;

const FRAME_STEP_LADDER: [i64; 4] = [1, 2, 5, 10];
const SECOND_STEP_LADDER: [f64; 12] = [
    1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0, 1800.0, 3600.0,
];

/// Grid step expressed either in frames or in seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridStep {
    Frames(i64),
    Seconds(f64),
}

impl GridStep {
    pub fn seconds(self, fps: f64) -> f64 {
        match self {
            GridStep::Frames(n) => n as f64 / fps,
            GridStep::Seconds(s) => s,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct FrameGrid {
    pub minor_times: Vec<f64>,
    pub major_times: Vec<f64>,
}

impl FrameGrid {
    pub fn compute(
        project_fps: f64,
        pixels_per_second: f64,
        start_seconds: f64,
        viewport_width_px: f64,
    ) -> Self {
        let fps = if project_fps > 1.0 {
            project_fps
        } else {
            29.97
        };
        let pps = pixels_per_second.max(MIN_PIXELS_PER_SECOND);
        let t_start = start_seconds.max(0.0);
        let t_end = start_seconds + viewport_width_px.max(0.0) / pps;

        let minor = choose_step(fps, pps, MIN_MINOR_SPACING_PX);
        let mut major = choose_step(fps, pps, MIN_MAJOR_SPACING_PX);
        if major.seconds(fps) < minor.seconds(fps) {
            major = minor;
        }

        let mut grid = FrameGrid::default();
        append_times(&mut grid.major_times, major, fps, t_start, t_end);

        // Minor lines that coincide with a major line are drawn only once (as major)
        let mut minor_all = Vec::new();
        append_times(&mut minor_all, minor, fps, t_start, t_end);
        let half_pixel_sec = 0.5 / pps;
        let mut mi = 0usize;
        for t in minor_all {
            while mi < grid.major_times.len() && grid.major_times[mi] < t - half_pixel_sec {
                mi += 1;
            }
            let on_major =
                mi < grid.major_times.len() && (grid.major_times[mi] - t).abs() <= half_pixel_sec;
            if !on_major {
                grid.minor_times.push(t);
            }
        }
        grid
    }
}

/// Smallest step (frames first, then seconds) whose on-screen spacing is at least `min_spacing_px`.
pub fn choose_step(fps: f64, pixels_per_second: f64, min_spacing_px: f64) -> GridStep {
    for frames in FRAME_STEP_LADDER {
        if frames as f64 >= fps {
            break;
        }
        if frames as f64 / fps * pixels_per_second >= min_spacing_px {
            return GridStep::Frames(frames);
        }
    }
    for sec in SECOND_STEP_LADDER {
        if sec * pixels_per_second >= min_spacing_px {
            return GridStep::Seconds(sec);
        }
    }
    GridStep::Seconds(SECOND_STEP_LADDER[SECOND_STEP_LADDER.len() - 1])
}

fn append_times(target: &mut Vec<f64>, step: GridStep, fps: f64, t_start: f64, t_end: f64) {
    match step {
        GridStep::Frames(count) => {
            // Frame index space: t = frame_index / fps
            let first = (t_start * fps / count as f64 - 1e-9).ceil() as i64;
            let last = (t_end * fps / count as f64 + 1e-9).floor() as i64;
            let mut i = first.max(0);
            while i <= last && target.len() < MAX_LINES_PER_LIST {
                target.push((i * count) as f64 / fps);
                i += 1;
            }
        }
        GridStep::Seconds(count) => {
            // Second steps land on the frame where the SMPTE timecode reads HH:MM:SS;00 (not on wall-clock
            // seconds), so major lines and ruler labels agree at fractional rates (29.97 / 59.94).
            let tc_seconds_per_second = fps / nominal_fps(fps) as f64;
            let first_step = (t_start * tc_seconds_per_second / count).floor() as i64 - 1;
            let last_step = (t_end * tc_seconds_per_second / count).ceil() as i64 + 1;
            let mut i = first_step.max(0);
            while i <= last_step && target.len() < MAX_LINES_PER_LIST {
                let t = timecode_seconds_to_frame((i as f64 * count) as i64, fps) as f64 / fps;
                if t >= t_start - 1e-9 && t <= t_end + 1e-9 {
                    target.push(t);
                }
                i += 1;
            }
        }
    }
}

/// Centers a 1-device-pixel line on a physical pixel so it renders crisp (never blurred over two pixels).
pub fn snap_to_device_pixel(x: f32, pixels_per_point: f32) -> f32 {
    let scale = if pixels_per_point > 0.0 {
        pixels_per_point
    } else {
        1.0
    };
    ((x * scale).floor() + 0.5) / scale
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_zoom_draws_one_minor_line_per_frame() {
        // 24 fps at 480 px/s: one frame = 20 px
        assert_eq!(
            choose_step(24.0, 480.0, MIN_MINOR_SPACING_PX),
            GridStep::Frames(1)
        );
        assert_eq!(
            choose_step(24.0, 480.0, MIN_MAJOR_SPACING_PX),
            GridStep::Frames(5)
        );
    }

    #[test]
    fn far_zoom_climbs_the_second_ladder() {
        assert_eq!(
            choose_step(30.0, 20.0, MIN_MAJOR_SPACING_PX),
            GridStep::Seconds(5.0)
        );
        assert_eq!(
            choose_step(30.0, 1.0, MIN_MAJOR_SPACING_PX),
            GridStep::Seconds(120.0)
        );
    }

    #[test]
    fn spacing_never_drops_below_the_minimum() {
        for pps in [20.0, 45.0, 100.0, 480.0, 5_000.0, 480_000.0] {
            for min in [MIN_MINOR_SPACING_PX, MIN_MAJOR_SPACING_PX] {
                let step = choose_step(25.0, pps, min);
                let spacing = step.seconds(25.0) * pps;
                let is_last_rung = matches!(step, GridStep::Seconds(s) if s == 3600.0);
                assert!(
                    spacing >= min || is_last_rung,
                    "pps {pps} min {min} -> {spacing}"
                );
            }
        }
    }

    #[test]
    fn frame_lines_sit_on_exact_frame_boundaries() {
        let grid = FrameGrid::compute(24.0, 480.0, 0.0, 400.0);
        for t in grid.minor_times.iter().chain(grid.major_times.iter()) {
            let frame = t * 24.0;
            assert!(
                (frame - frame.round()).abs() < 1e-9,
                "{t} is not on a frame"
            );
        }
        assert!(!grid.minor_times.is_empty());
    }

    #[test]
    fn minor_lines_never_duplicate_major_lines() {
        let grid = FrameGrid::compute(24.0, 480.0, 0.0, 800.0);
        for m in &grid.minor_times {
            assert!(grid.major_times.iter().all(|j| (j - m).abs() > 0.5 / 480.0));
        }
    }

    #[test]
    fn minute_steps_land_on_drop_frame_boundaries() {
        let mut times = Vec::new();
        append_times(&mut times, GridStep::Seconds(60.0), 29.97, 0.0, 130.0);
        // ;00 does not exist at dropped minutes: the minute line sits on frame 1800
        assert!(times.iter().any(|t| (t - 1800.0 / 29.97).abs() < 1e-9));
    }

    #[test]
    fn snapping_centers_on_a_device_pixel() {
        assert_eq!(snap_to_device_pixel(10.2, 1.0), 10.5);
        assert_eq!(snap_to_device_pixel(10.2, 2.0), 10.25);
    }
}
