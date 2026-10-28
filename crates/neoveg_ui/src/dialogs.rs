//! Where the floating boxes open, so they stop landing at arbitrary places.
//!
//! * **Dialogs** (preferences, properties, render, generators, slideshow, shortcuts…) open in the
//!   middle of the window and can still be dragged.
//! * **Tool windows** (inspector, color correction, key, metronome, edit details, notes, logs) open
//!   cascaded at the right edge: each one opens one slot lower than the tool windows that are already
//!   open, so it is drawn over them without hiding their title bars (a fixed slot per window hid
//!   one behind another whenever the lower-numbered one opened last).
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

/// The tool windows that are open: title and the last frame that showed them.
#[derive(Clone, Default)]
struct OpenTools(Vec<(String, u64)>);

/// The slot of the tool window `title` on `frame`: its place in the list of the tool windows that
/// are open (shown on this or the previous frame), in the order they were opened. A window that is
/// not shown any more leaves the list, so the next one reuses its slot.
pub fn tool_slot(open: &mut Vec<(String, u64)>, title: &str, frame: u64) -> usize {
    open.retain(|(_, seen)| frame.saturating_sub(*seen) <= 1);
    if let Some(index) = open.iter().position(|(t, _)| t == title) {
        open[index].1 = frame;
        return index;
    }
    open.push((title.to_owned(), frame));
    open.len() - 1
}

/// A tool window: opening at the right edge, one slot below the tool windows already open. The
/// position is only the default; egui remembers where the user moves a window.
pub fn tool_window<'a>(ctx: &egui::Context, title: &str) -> egui::Window<'a> {
    let frame = ctx.cumulative_pass_nr();
    let slot = ctx.data_mut(|data| {
        let tools = data.get_temp_mut_or_default::<OpenTools>(egui::Id::new("neoveg_tool_windows"));
        tool_slot(&mut tools.0, title, frame)
    });
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

    #[test]
    fn a_tool_window_opens_below_the_ones_already_open_whatever_its_kind() {
        let mut open = Vec::new();
        // Notes first, then the details table: the later one is one slot lower, so it covers the
        // earlier one's body but not its title bar
        assert_eq!(tool_slot(&mut open, "Notas do projeto", 10), 0);
        assert_eq!(tool_slot(&mut open, "Detalhes de edição", 10), 1);
        // Both stay where they are while they keep being shown
        assert_eq!(tool_slot(&mut open, "Notas do projeto", 11), 0);
        assert_eq!(tool_slot(&mut open, "Detalhes de edição", 11), 1);
        // The notes close (they are missed from the next frame on): the table moves up in the list
        // and the next window takes the free slot
        assert_eq!(tool_slot(&mut open, "Detalhes de edição", 13), 0);
        assert_eq!(tool_slot(&mut open, "Inspetor", 13), 1);
    }

    #[test]
    fn a_window_that_was_closed_and_reopened_starts_again_from_the_top() {
        let mut open = Vec::new();
        assert_eq!(tool_slot(&mut open, "A", 1), 0);
        assert_eq!(tool_slot(&mut open, "B", 1), 1);
        // Neither is shown for a few frames
        assert_eq!(tool_slot(&mut open, "B", 9), 0);
        assert_eq!(tool_slot(&mut open, "A", 9), 1);
    }
}
