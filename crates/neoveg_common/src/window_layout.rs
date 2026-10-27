//! Detached panels: the pure model (no GUI types) behind panels that live in their own native window, possibly on
//! another monitor.
//!
//! A panel dragged out of the main window becomes an operating-system window (an egui viewport).
//! This module holds what can be tested without a window: the geometry that is remembered between
//! sessions, the checks that keep a remembered window from opening off-screen (monitor unplugged,
//! resolution changed) and the conversion to and from egui viewport data. egui 0.29 does not
//! enumerate monitors, so the checks use the size of the monitor the main window is on; scaling for
//! different pixel densities per monitor is done by the windowing layer for each viewport.

use serde::{Deserialize, Serialize};

/// Smallest size a detached window may have, in points.
pub const MIN_WIDTH: f32 = 240.0;
pub const MIN_HEIGHT: f32 = 160.0;
/// How far outside the main monitor a window may start, as a multiple of that monitor's size. Keeps
/// windows on a secondary screen while rejecting nonsense coordinates.
const MAX_SCREENS_AWAY: f32 = 3.0;

/// Where a detached window was and how big it was, in points of the screen it was on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowGeometry {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub maximized: bool,
    /// Size of the monitor the window was on (`0` when unknown).
    pub monitor_width: f32,
    pub monitor_height: f32,
    /// Pixel density of that monitor when the window was saved.
    pub pixels_per_point: f32,
}

impl Default for WindowGeometry {
    fn default() -> Self {
        Self {
            x: 80.0,
            y: 80.0,
            width: 640.0,
            height: 400.0,
            maximized: false,
            monitor_width: 0.0,
            monitor_height: 0.0,
            pixels_per_point: 1.0,
        }
    }
}

/// A detached panel as saved in the preferences (`tab` is the stable id of the panel).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedWindow {
    pub tab: String,
    #[serde(default)]
    pub geometry: WindowGeometry,
}

impl WindowGeometry {
    /// The geometry made safe to open: finite values, at least the minimum size, no larger than
    /// the monitor when it is known, and a position that is not absurdly far from the main monitor
    /// (`main_monitor` is its size; `None` when unknown, which only checks finiteness).
    pub fn sanitized(&self, main_monitor: Option<(f32, f32)>) -> WindowGeometry {
        let default = WindowGeometry::default();
        let finite = |v: f32, fallback: f32| if v.is_finite() { v } else { fallback };
        let mut g = WindowGeometry {
            x: finite(self.x, default.x),
            y: finite(self.y, default.y),
            width: finite(self.width, default.width).max(MIN_WIDTH),
            height: finite(self.height, default.height).max(MIN_HEIGHT),
            maximized: self.maximized,
            monitor_width: finite(self.monitor_width, 0.0).max(0.0),
            monitor_height: finite(self.monitor_height, 0.0).max(0.0),
            pixels_per_point: if self.pixels_per_point.is_finite() && self.pixels_per_point > 0.0 {
                self.pixels_per_point
            } else {
                1.0
            },
        };
        if let Some(monitor) = main_monitor.filter(|m| m.0 > 0.0 && m.1 > 0.0) {
            // Room for monitors left of / above the main one (negative coordinates) and right /
            // below it, but not for coordinates that no arrangement of screens reaches.
            let (limit_x, limit_y) = (monitor.0 * MAX_SCREENS_AWAY, monitor.1 * MAX_SCREENS_AWAY);
            if g.x < -limit_x || g.x > limit_x || g.y < -limit_y || g.y > limit_y {
                g.x = default.x;
                g.y = default.y;
            }
            // A window cannot be bigger than the screen it was saved on
            let (cap_w, cap_h) = if g.monitor_width > 0.0 && g.monitor_height > 0.0 {
                (g.monitor_width, g.monitor_height)
            } else {
                monitor
            };
            g.width = g.width.min(cap_w.max(MIN_WIDTH));
            g.height = g.height.min(cap_h.max(MIN_HEIGHT));
        }
        g
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MONITOR: (f32, f32) = (1920.0, 1080.0);

    #[test]
    fn sanitize_keeps_a_good_geometry() {
        let g = WindowGeometry {
            x: 2000.0,
            y: 100.0,
            width: 800.0,
            height: 600.0,
            ..WindowGeometry::default()
        };
        assert_eq!(g.sanitized(Some(MONITOR)), g);
    }

    #[test]
    fn sanitize_rejects_nan_and_tiny_sizes() {
        let g = WindowGeometry {
            x: f32::NAN,
            y: f32::INFINITY,
            width: 3.0,
            height: f32::NAN,
            pixels_per_point: -2.0,
            ..WindowGeometry::default()
        };
        let s = g.sanitized(None);
        assert!(s.x.is_finite() && s.y.is_finite());
        assert!(s.width >= MIN_WIDTH && s.height >= MIN_HEIGHT);
        assert_eq!(s.pixels_per_point, 1.0);
    }

    #[test]
    fn sanitize_pulls_back_a_window_far_off_screen() {
        let g = WindowGeometry {
            x: 90_000.0,
            y: -50_000.0,
            ..WindowGeometry::default()
        };
        let s = g.sanitized(Some(MONITOR));
        assert_eq!((s.x, s.y), (80.0, 80.0));
    }

    #[test]
    fn a_window_on_a_monitor_left_of_the_main_one_is_kept() {
        let g = WindowGeometry {
            x: -1800.0,
            y: 40.0,
            ..WindowGeometry::default()
        };
        assert_eq!(g.sanitized(Some(MONITOR)).x, -1800.0);
    }

    #[test]
    fn size_is_capped_by_the_monitor() {
        let g = WindowGeometry {
            width: 9000.0,
            height: 7000.0,
            monitor_width: 2560.0,
            monitor_height: 1440.0,
            ..WindowGeometry::default()
        };
        let s = g.sanitized(Some(MONITOR));
        assert_eq!((s.width, s.height), (2560.0, 1440.0));
    }

    #[test]
    fn saved_windows_round_trip_and_old_files_load() {
        let saved = SavedWindow {
            tab: "preview".into(),
            geometry: WindowGeometry {
                x: -1920.0,
                ..WindowGeometry::default()
            },
        };
        let json = serde_json::to_string(&saved).unwrap();
        assert_eq!(serde_json::from_str::<SavedWindow>(&json).unwrap(), saved);
        // Only the tab: every other field falls back to its default
        let bare: SavedWindow = serde_json::from_str(r#"{"tab":"timeline"}"#).unwrap();
        assert_eq!(bare.geometry, WindowGeometry::default());
    }
}
