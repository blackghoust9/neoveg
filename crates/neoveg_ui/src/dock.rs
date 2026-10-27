//! Detached panels, egui side: conversions between the saved window geometry
//! (`neoveg_common::window_layout`) and egui viewports, and promotion of the floating panels of the
//! dock into native operating-system windows.

use eframe::egui;
use neoveg_common::window_layout::{WindowGeometry, MIN_HEIGHT, MIN_WIDTH};

/// The window builder for a detached panel.
pub fn viewport_builder(geometry: &WindowGeometry, title: &str) -> egui::ViewportBuilder {
    egui::ViewportBuilder::default()
        .with_title(title)
        .with_decorations(true)
        .with_resizable(true)
        .with_min_inner_size([MIN_WIDTH, MIN_HEIGHT])
        .with_inner_size([geometry.width, geometry.height])
        .with_position([geometry.x, geometry.y])
        .with_maximized(geometry.maximized)
}

/// The geometry of a window from what egui reports about its viewport. Parts egui does not know
/// (a window that is not shown yet) keep the values of `previous`.
pub fn from_viewport(info: &egui::ViewportInfo, previous: &WindowGeometry) -> WindowGeometry {
    let mut g = previous.clone();
    if let Some(outer) = info.outer_rect {
        g.x = outer.min.x;
        g.y = outer.min.y;
    }
    if let Some(inner) = info.inner_rect {
        g.width = inner.width();
        g.height = inner.height();
    }
    if let Some(maximized) = info.maximized {
        g.maximized = maximized;
    }
    if let Some(monitor) = info.monitor_size {
        g.monitor_width = monitor.x;
        g.monitor_height = monitor.y;
    }
    if let Some(ppp) = info.native_pixels_per_point.filter(|p| *p > 0.0) {
        g.pixels_per_point = ppp;
    }
    g
}

/// Where a panel dragged out of the main window opens: the main window's inner origin on the screen
/// plus the panel's position inside it, at the size it had. A panel that was never shown (no
/// rectangle) gets a sensible default window.
pub fn from_dragged_panel(main_origin: egui::Pos2, panel: egui::Rect) -> WindowGeometry {
    let panel = if panel.is_finite() && panel.width() > 0.0 && panel.height() > 0.0 {
        panel
    } else {
        egui::Rect::from_min_size(egui::pos2(40.0, 40.0), egui::vec2(640.0, 400.0))
    };
    WindowGeometry {
        x: main_origin.x + panel.min.x,
        y: main_origin.y + panel.min.y,
        width: panel.width().max(MIN_WIDTH),
        height: panel.height().max(MIN_HEIGHT),
        ..WindowGeometry::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_viewport_overlays_known_values_only() {
        let previous = WindowGeometry::default();
        let info = egui::ViewportInfo {
            outer_rect: Some(egui::Rect::from_min_size(
                egui::pos2(-1500.0, 30.0),
                egui::vec2(900.0, 700.0),
            )),
            inner_rect: Some(egui::Rect::from_min_size(
                egui::pos2(-1498.0, 60.0),
                egui::vec2(896.0, 668.0),
            )),
            maximized: Some(true),
            monitor_size: Some(egui::vec2(1920.0, 1080.0)),
            native_pixels_per_point: Some(1.5),
            ..Default::default()
        };
        let g = from_viewport(&info, &previous);
        assert_eq!((g.x, g.y), (-1500.0, 30.0));
        assert_eq!((g.width, g.height), (896.0, 668.0));
        assert!(g.maximized);
        assert_eq!((g.monitor_width, g.pixels_per_point), (1920.0, 1.5));
        assert_eq!(
            from_viewport(&egui::ViewportInfo::default(), &previous),
            previous
        );
    }

    #[test]
    fn a_dragged_panel_opens_where_it_was_dropped() {
        let panel = egui::Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(500.0, 300.0));
        let g = from_dragged_panel(egui::pos2(10.0, 20.0), panel);
        assert_eq!((g.x, g.y, g.width, g.height), (110.0, 70.0, 500.0, 300.0));
        let g = from_dragged_panel(egui::pos2(0.0, 0.0), egui::Rect::NOTHING);
        assert!(g.width >= MIN_WIDTH && g.x.is_finite());
    }
}
