//! Zoom-adaptive timeline frame grid.
//!
//! - Close zoom: one minor line per frame (1/24, 1/25, 1/30, 1/60 s... from the project FPS).
//! - Medium/far zoom: lines grouped into 2/5/10 frames, then 1 s, 2 s, 5 s, 10 s, 15 s, 30 s, 1 min, 2 min, 5 min...
//! - Major lines carry the ruler labels; minor lines are the subtle subdivisions between them.
//!
//! Grid times are always derived from integer frame/step indices (never accumulated) so lines stay on
//! exact frame boundaries at any zoom or scroll position.

use neoveg_common::timecode::{is_drop_frame_rate, nominal_fps, timecode_seconds_to_frame};

pub const MIN_MINOR_SPACING_PX: f64 = 8.0;
pub const MIN_MAJOR_SPACING_PX: f64 = 90.0;
/// Only a guard against a zero or negative zoom: the timeline never goes below 15 px/s, and the
/// grid must follow the real zoom (a higher floor made the ruler stop short of the right edge).
const MIN_PIXELS_PER_SECOND: f64 = 0.5;
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
            // Non-drop timecode of a fractional rate runs slow (23.976 fps counts 24 frames a second, so
            // 0.1 % fewer timecode seconds than real ones); drop-frame timecode skips frame numbers exactly
            // to stay with the clock, so there a timecode second is a real second. Using the slow ratio
            // there made the lines fall 3.6 s behind the window after an hour and stop short of the edge.
            let tc_seconds_per_second = if is_drop_frame_rate(fps) {
                1.0
            } else {
                fps / nominal_fps(fps) as f64
            };
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
    fn the_grid_covers_the_whole_viewport_at_every_zoom_the_timeline_allows() {
        // The timeline zooms out to 15 px/s; the ruler must reach the right edge there too
        for pps in [15.0, 17.5, 19.9, 20.0, 60.0, 480.0, 480_000.0] {
            let width = 1000.0;
            let grid = FrameGrid::compute(29.97, pps, 0.0, width);
            let visible = width / pps;
            let last = grid.major_times.last().copied().unwrap_or(0.0);
            let step = choose_step(29.97, pps, MIN_MAJOR_SPACING_PX).seconds(29.97);
            assert!(
                visible - last <= step * 1.01 + 1e-6,
                "at {pps} px/s the last label is at {last} s of {visible} s (step {step})"
            );
            // Neighbouring labels never sit closer than the room a label needs
            for pair in grid.major_times.windows(2) {
                assert!(
                    (pair[1] - pair[0]) * pps >= MIN_MAJOR_SPACING_PX - 1.0,
                    "labels too close at {pps} px/s"
                );
            }
        }
    }

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

    /// Case found by the sweep below (30/10/2026): zoom 15 px/s, scrolled 10 hours into the project,
    /// a 1000 px wide ruler. The second lines were placed with the ratio of non-drop fractional rates
    /// (fps / nominal), which at 29.97 and 59.94 is wrong (drop-frame timecode follows the clock), and
    /// `is_drop_frame_rate` also called 30 and 60 fps drop-frame: either way the lines ran 3.6 s behind
    /// the window for every hour and the ruler stopped short of the right edge.
    #[test]
    fn far_into_a_project_the_ruler_still_reaches_the_right_edge() {
        for fps in [23.976, 24.0, 25.0, 29.97, 30.0, 59.94, 60.0] {
            for scroll in [3600.0, 36_000.5, 86_399.9] {
                let (pps, width) = (15.0, 1000.0);
                let grid = FrameGrid::compute(fps, pps, scroll, width);
                let (start, end) = (scroll, scroll + width / pps);
                let step = choose_step(fps, pps, MIN_MAJOR_SPACING_PX).seconds(fps);
                let first = *grid.major_times.first().expect("a major line");
                let last = *grid.major_times.last().expect("a major line");
                assert!(
                    first - start <= step * 1.01 + 4.0 / fps,
                    "{fps} fps at {scroll} s: first {first}"
                );
                assert!(
                    end - last <= step * 1.01 + 4.0 / fps,
                    "{fps} fps at {scroll} s: last {last} of {end}"
                );
            }
        }
    }

    #[test]
    fn second_lines_of_a_30_fps_project_are_whole_seconds_with_no_dropped_frames() {
        let mut times = Vec::new();
        append_times(&mut times, GridStep::Seconds(60.0), 30.0, 0.0, 300.0);
        // 30 fps is not drop-frame: the minute lines sit on frames 1800, 3600, ... exactly
        for (i, t) in times.iter().enumerate() {
            assert!((t - (i as f64 * 60.0)).abs() < 1e-9, "{i}: {t}");
        }
    }

    /// The whole grid at every zoom the timeline allows (and at the points where a step starts to
    /// fit), at the usual frame rates, at the edges of a minute and an hour and far into a project,
    /// in narrow and wide rulers. This is the check that found the case above.
    #[test]
    fn the_grid_holds_at_every_zoom_scroll_rate_and_width() {
        const FPS: [f64; 8] = [23.976, 24.0, 25.0, 29.97, 30.0, 50.0, 59.94, 60.0];
        let mut zooms = vec![
            15.0, 17.0, 19.9, 20.0, 30.0, 45.0, 60.0, 100.0, 180.0, 360.0, 480.0, 960.0, 2880.0,
            20_000.0, 480_000.0,
        ];
        for fps in FPS {
            for min in [MIN_MINOR_SPACING_PX, MIN_MAJOR_SPACING_PX] {
                for frames in [1.0, 5.0] {
                    zooms.push(min / (frames / fps) * 1.000_001);
                }
                for secs in [1.0, 5.0, 60.0, 600.0] {
                    zooms.push(min / secs * 1.000_001);
                }
            }
        }
        zooms.retain(|z| *z >= 15.0);

        let mut failures: Vec<String> = Vec::new();
        for fps in FPS {
            let scrolls = [
                0.0,
                1.0 / fps,
                0.5,
                59.9667,
                60.0,
                3599.9,
                3600.0,
                36_000.5,
                86_399.9,
            ];
            for &pps in &zooms {
                for width in [300.0, 1920.0] {
                    for scroll in scrolls {
                        let grid = FrameGrid::compute(fps, pps, scroll, width);
                        let (start, end) = (scroll.max(0.0), scroll + width / pps);
                        let step = choose_step(fps, pps, MIN_MAJOR_SPACING_PX).seconds(fps);
                        let slack = 4.0 / fps; // a dropped minute moves a line by 2 frames (4 at 59.94)
                        let tag =
                            format!("{fps} fps, {pps:.3} px/s, scroll {scroll}, width {width}");
                        for (name, list) in
                            [("major", &grid.major_times), ("minor", &grid.minor_times)]
                        {
                            let mut last = f64::NEG_INFINITY;
                            for &t in list.iter() {
                                if t < start - 1e-6 || t > end + 1e-6 {
                                    failures.push(format!("{tag}: {name} {t} outside the ruler"));
                                }
                                if t <= last {
                                    failures.push(format!("{tag}: {name} {t} not increasing"));
                                }
                                last = t;
                                if (t * fps - (t * fps).round()).abs() > 1e-6 {
                                    failures.push(format!("{tag}: {name} {t} is not on a frame"));
                                }
                            }
                        }
                        for pair in grid.major_times.windows(2) {
                            let px = (pair[1] - pair[0]) * pps;
                            if px < MIN_MAJOR_SPACING_PX - 1.0 - slack * pps {
                                failures.push(format!("{tag}: majors only {px:.1} px apart"));
                            }
                        }
                        if grid.major_times.len() >= MAX_LINES_PER_LIST
                            || grid.minor_times.len() >= MAX_LINES_PER_LIST
                        {
                            failures.push(format!("{tag}: a list hit its cap"));
                        } else if width / pps >= step * 1.05 {
                            match (grid.major_times.first(), grid.major_times.last()) {
                                (Some(&first), Some(&last)) => {
                                    if first - start > step * 1.01 + slack {
                                        failures
                                            .push(format!("{tag}: first major {first} is late"));
                                    }
                                    if end - last > step * 1.01 + slack {
                                        failures.push(format!(
                                            "{tag}: last major {last} stops short of {end}"
                                        ));
                                    }
                                }
                                _ => failures.push(format!("{tag}: no major line")),
                            }
                        }
                    }
                }
            }
        }
        assert!(
            failures.is_empty(),
            "{} failures, the first ones:\n{}",
            failures.len(),
            failures
                .iter()
                .take(8)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    #[test]
    fn snapping_centers_on_a_device_pixel() {
        assert_eq!(snap_to_device_pixel(10.2, 1.0), 10.5);
        assert_eq!(snap_to_device_pixel(10.2, 2.0), 10.25);
    }
}
