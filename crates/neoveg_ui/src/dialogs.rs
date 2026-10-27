//! Where the floating boxes open, so they stop landing at arbitrary places.
//!
//! * **Dialogs** (preferences, properties, render, generators, slideshow, shortcuts…) open in the
//!   middle of the window and can still be dragged.
//! * **Tool windows** (inspector, color correction, key, metronome, edit details, notes, logs) open
//!   stacked at the right edge, one slot lower each, so several of them never hide one another.
//!
//! The first position is only a default: egui remembers where the user dragged a window.

use eframe::egui::{self, Align2, Pos2};

/// Distance between the right edge of the window and a tool window.
const EDGE_MARGIN: f32 = 16.0;
/// Top of the first tool slot, below the menu bar and the toolbar.
const FIRST_SLOT_TOP: f32 = 96.0;
/// Vertical step between two tool slots.
const SLOT_STEP: f32 = 30.0;

/// Default position of a dialog: the centre of the window.
pub fn dialog_position(screen: egui::Rect) -> Pos2 {
    screen.center()
}

/// Default position (top-right corner) of the tool window in `slot`, kept inside the screen.
pub fn tool_position(screen: egui::Rect, slot: usize) -> Pos2 {
    let step = SLOT_STEP * (slot % 8) as f32;
    Pos2::new(
        screen.right() - EDGE_MARGIN,
        (screen.top() + FIRST_SLOT_TOP + step).min(screen.bottom() - 80.0),
    )
}

/// A dialog window: titled, not collapsible, opening centred.
pub fn dialog<'a>(ctx: &egui::Context, title: &str) -> egui::Window<'a> {
    egui::Window::new(title.to_owned())
        .collapsible(false)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(dialog_position(ctx.screen_rect()))
}

/// A tool window: opening at the right edge in its own slot.
pub fn tool_window<'a>(ctx: &egui::Context, title: &str, slot: usize) -> egui::Window<'a> {
    egui::Window::new(title.to_owned())
        .pivot(Align2::RIGHT_TOP)
        .default_pos(tool_position(ctx.screen_rect(), slot))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> egui::Rect {
        egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(1600.0, 900.0))
    }

    #[test]
    fn dialogs_open_in_the_middle() {
        assert_eq!(dialog_position(screen()), Pos2::new(800.0, 450.0));
    }

    #[test]
    fn tool_windows_stack_at_the_right_edge_one_slot_apart() {
        let first = tool_position(screen(), 0);
        let second = tool_position(screen(), 1);
        assert_eq!(first.x, 1600.0 - EDGE_MARGIN);
        assert_eq!(second.x, first.x);
        assert!((second.y - first.y - SLOT_STEP).abs() < 1e-3);
    }

    #[test]
    fn a_slot_never_leaves_the_screen_even_on_a_short_window() {
        let short = egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 300.0));
        for slot in 0..20 {
            let p = tool_position(short, slot);
            assert!(p.y <= short.bottom() - 80.0 + 1e-3 && p.y >= short.top());
        }
    }
}
