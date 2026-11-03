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

/// A dialog window: titled, not collapsible, opening centred and staying draggable.
pub fn dialog<'a>(ctx: &egui::Context, title: &str) -> egui::Window<'a> {
    let pass = ctx.cumulative_pass_nr();
    let screen = ctx.screen_rect();
    let pos = dialog_position(screen);
    let id = egui::Id::new(title);
    let dragged = ctx.is_being_dragged(id.with("move")) || ctx.is_being_dragged(id.with("resize"));
    let loaded = egui::AreaState::load(ctx, id);
    let place = ctx.data_mut(|data| {
        let mem = data.get_temp_mut_or_default::<DialogMemory>(egui::Id::new("neoveg_dialogs"));
        should_place_window(&mut mem.windows, title, pass, screen, dragged, loaded)
    });
    let win = egui::Window::new(title.to_owned())
        .collapsible(false)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(pos);
    if place {
        win.current_pos(pos)
    } else {
        win
    }
}

/// One open tool window: its title, the last pass that showed it, and the cascade slot it opened in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenTool {
    pub title: String,
    pub seen: u64,
    pub slot: usize,
}

#[derive(Clone, Default)]
struct DialogMemory {
    tools: Vec<OpenTool>,
    windows: std::collections::HashMap<String, TrackedWindow>,
}

#[derive(Clone)]
struct TrackedWindow {
    last_seen_pass: u64,
    seen_streak: u32,
    last_screen: egui::Rect,
    expected_pivot: Option<Pos2>,
    user_moved: bool,
}

/// Whether `dialogs` should place `title` at its computed default position on `pass`. Returns
/// `false` as soon as the user drags or resizes the window, so egui keeps the chosen position even
/// if the window is closed and reopened later.
fn should_place_window(
    windows: &mut std::collections::HashMap<String, TrackedWindow>,
    title: &str,
    pass: u64,
    screen: egui::Rect,
    dragged: bool,
    loaded: Option<egui::AreaState>,
) -> bool {
    let Some(entry) = windows.get_mut(title) else {
        windows.insert(
            title.to_owned(),
            TrackedWindow {
                last_seen_pass: pass,
                seen_streak: 1,
                last_screen: screen,
                expected_pivot: None,
                user_moved: dragged,
            },
        );
        return !dragged;
    };

    if dragged {
        entry.user_moved = true;
    }
    if let (Some(expected), Some(actual)) = (entry.expected_pivot, loaded.and_then(|s| s.pivot_pos))
    {
        if (actual - expected).length() > 1.5 {
            entry.user_moved = true;
        }
    }
    if entry.last_seen_pass == pass {
        return !entry.user_moved && entry.seen_streak <= 2;
    }

    let reopened = pass.saturating_sub(entry.last_seen_pass) > 1;
    let screen_changed = entry.last_screen != screen;
    entry.last_seen_pass = pass;
    entry.last_screen = screen;

    if entry.user_moved {
        return false;
    }
    if reopened || screen_changed {
        entry.seen_streak = if loaded.and_then(|s| s.size).is_some() {
            2
        } else {
            1
        };
        entry.expected_pivot = None;
        return true;
    }

    entry.seen_streak = entry.seen_streak.saturating_add(1);
    if entry.seen_streak == 2 {
        return true;
    }
    if entry.seen_streak == 3 {
        entry.expected_pivot = loaded.and_then(|s| s.pivot_pos);
    }
    false
}

/// The slot of the tool window `title` on `frame`: a window that stays open keeps its own slot
/// (because `egui` keeps an open window where it is on screen), while a newly opened window takes
/// one slot below the lowest currently open tool window (`0` when none are open) so its front-most
/// frame never hides an earlier window's title bar.
pub fn tool_slot(open: &mut Vec<OpenTool>, title: &str, frame: u64) -> usize {
    open.retain(|entry| frame.saturating_sub(entry.seen) <= 1);
    if let Some(entry) = open.iter_mut().find(|e| e.title == title) {
        entry.seen = frame;
        return entry.slot;
    }
    let slot = open.iter().map(|e| e.slot + 1).max().unwrap_or(0);
    open.push(OpenTool {
        title: title.to_owned(),
        seen: frame,
        slot,
    });
    slot
}

/// A tool window: opening at the right edge, one slot below the tool windows already open. The
/// position is only the default; egui remembers where the user moves a window.
pub fn tool_window<'a>(ctx: &egui::Context, title: &str) -> egui::Window<'a> {
    let frame = ctx.cumulative_pass_nr();
    let screen = ctx.screen_rect();
    let id = egui::Id::new(title);
    let dragged = ctx.is_being_dragged(id.with("move")) || ctx.is_being_dragged(id.with("resize"));
    let loaded = egui::AreaState::load(ctx, id);
    let (slot, place) = ctx.data_mut(|data| {
        let mem = data.get_temp_mut_or_default::<DialogMemory>(egui::Id::new("neoveg_dialogs"));
        let slot = tool_slot(&mut mem.tools, title, frame);
        let place = should_place_window(&mut mem.windows, title, frame, screen, dragged, loaded);
        (slot, place)
    });
    let pos = tool_position(screen, slot);
    let win = egui::Window::new(title.to_owned())
        .pivot(Align2::RIGHT_TOP)
        .default_pos(pos);
    if place {
        win.current_pos(pos)
    } else {
        win
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> egui::Rect {
        egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(1600.0, 900.0))
    }

    fn run_pass(ctx: &egui::Context, screen_rect: egui::Rect, mut f: impl FnMut(&egui::Context)) {
        let input = egui::RawInput {
            screen_rect: Some(screen_rect),
            ..Default::default()
        };
        let _ = ctx.run(input, &mut f);
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
        // Notes close on frame 12 while the details table stays open at slot 1: the next window
        // opens at slot 2 instead of colliding with the details table at slot 1
        assert_eq!(tool_slot(&mut open, "Detalhes de edição", 12), 1);
        assert_eq!(tool_slot(&mut open, "Detalhes de edição", 13), 1);
        assert_eq!(tool_slot(&mut open, "Inspetor", 13), 2);
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

    #[test]
    fn dialog_opens_centred_follows_resize_until_dragged_and_remembers_drag() {
        let ctx = egui::Context::default();
        let small = egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(1280.0, 720.0));
        let large = screen();

        for _ in 0..3 {
            run_pass(&ctx, small, |ctx| {
                dialog(ctx, "Inserir tempo")
                    .resizable(false)
                    .show(ctx, |ui| {
                        ui.label("00:00:05:00");
                    });
            });
        }
        let initial = egui::AreaState::load(&ctx, egui::Id::new("Inserir tempo"))
            .unwrap()
            .rect();
        assert!((initial.center() - small.center()).length() <= 2.0);

        // Before the user drags it, maximizing the main window keeps the dialog centred
        for _ in 0..3 {
            run_pass(&ctx, large, |ctx| {
                dialog(ctx, "Inserir tempo")
                    .resizable(false)
                    .show(ctx, |ui| {
                        ui.label("00:00:05:00");
                    });
            });
        }
        let resized = egui::AreaState::load(&ctx, egui::Id::new("Inserir tempo"))
            .unwrap()
            .rect();
        assert!((resized.center() - large.center()).length() <= 2.0);

        // Drag the title bar by (-200, -120): unlike `.anchor(...)`, the dialog moves and stays put
        let grab = Pos2::new(resized.center().x, resized.top() + 8.0);
        let drop = grab + egui::vec2(-200.0, -120.0);
        for events in [
            vec![
                egui::Event::PointerMoved(grab),
                egui::Event::PointerButton {
                    pos: grab,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            vec![egui::Event::PointerMoved(drop)],
            vec![egui::Event::PointerButton {
                pos: drop,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        ] {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(large),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    dialog(ctx, "Inserir tempo")
                        .resizable(false)
                        .show(ctx, |ui| {
                            ui.label("00:00:05:00");
                        });
                },
            );
        }
        let moved = egui::AreaState::load(&ctx, egui::Id::new("Inserir tempo"))
            .unwrap()
            .rect();
        assert!(
            (moved.center() - (resized.center() + egui::vec2(-200.0, -120.0))).length() <= 2.0,
            "resized={resized:?}, moved={moved:?}"
        );

        // Close for several passes and reopen: egui remembers where the user moved it
        for _ in 0..4 {
            run_pass(&ctx, large, |_ctx| {});
        }
        for _ in 0..3 {
            run_pass(&ctx, large, |ctx| {
                dialog(ctx, "Inserir tempo")
                    .resizable(false)
                    .show(ctx, |ui| {
                        ui.label("00:00:05:00");
                    });
            });
        }
        let reopened = egui::AreaState::load(&ctx, egui::Id::new("Inserir tempo"))
            .unwrap()
            .rect();
        assert!((reopened.center() - moved.center()).length() <= 2.0);
    }

    #[test]
    fn reopened_unmoved_tool_window_takes_new_cascade_slot_instead_of_hiding_earlier_title_bar() {
        let ctx = egui::Context::default();
        let rect = screen();

        // Open Notes first (slot 0), then close it
        for _ in 0..3 {
            run_pass(&ctx, rect, |ctx| {
                tool_window(ctx, "Notas do projeto").show(ctx, |ui| {
                    ui.label("nota");
                });
            });
        }
        let notes_first = egui::AreaState::load(&ctx, egui::Id::new("Notas do projeto"))
            .unwrap()
            .rect();
        assert!((notes_first.top() - FIRST_SLOT_TOP).abs() <= 2.0);

        for _ in 0..3 {
            run_pass(&ctx, rect, |_ctx| {});
        }

        // Open Edit Details (slot 0), then reopen Notes: Notes must land one slot lower (slot 1)
        for _ in 0..3 {
            run_pass(&ctx, rect, |ctx| {
                tool_window(ctx, "Detalhes de edição").show(ctx, |ui| {
                    ui.label("tabela");
                });
                tool_window(ctx, "Notas do projeto").show(ctx, |ui| {
                    ui.label("nota");
                });
            });
        }
        let details = egui::AreaState::load(&ctx, egui::Id::new("Detalhes de edição"))
            .unwrap()
            .rect();
        let notes_reopened = egui::AreaState::load(&ctx, egui::Id::new("Notas do projeto"))
            .unwrap()
            .rect();
        assert!((details.top() - FIRST_SLOT_TOP).abs() <= 2.0);
        assert!((notes_reopened.top() - (FIRST_SLOT_TOP + SLOT_STEP)).abs() <= 2.0);
    }
}
