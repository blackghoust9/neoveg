//! Compact footer tabs (AGENTS.md section 4.1): a strip of flat 22 px headers at the bottom of a
//! window, font size 11, padding `6,2,6,2`, no selection pipe. When the tabs do not fit, arrow buttons
//! appear and the strip scrolls (also with the mouse wheel); the selected tab is always scrolled into
//! view. `egui_dock` cannot place its own strip at the bottom, so the dock draws its strips with this
//! widget and decides itself where a dragged tab goes ([`drop_target`]).

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Id, Rect, Response, Sense, Ui};
use neoveg_common::i18n::tr;

/// Height of the strip.
pub const HEIGHT: f32 = 22.0;
/// Font size of the tab names.
pub const FONT_SIZE: f32 = 11.0;
/// Horizontal padding of a tab header.
const PADDING_X: f32 = 6.0;
/// Width of each scroll arrow.
const ARROW_WIDTH: f32 = 16.0;
/// How far the pointer moves before a press on a tab is a drag.
const DRAG_THRESHOLD: f32 = 6.0;
/// Pixels scrolled by a click on an arrow and by one notch of the wheel.
const SCROLL_STEP: f32 = 60.0;

/// How the strip lays out its tabs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    /// The scroll arrows are shown (the tabs do not fit).
    pub arrows: bool,
    /// Width available for the tabs themselves, arrows excluded.
    pub view: f32,
    /// Scroll offset, kept inside the range that is possible and that shows the selected tab.
    pub scroll: f32,
}

/// Lays out tabs of the given `widths` in `available` pixels. `scroll` is the offset asked for, and
/// `selected` (when given) is brought into view, which wins over the offset asked for.
pub fn layout(widths: &[f32], available: f32, scroll: f32, selected: Option<usize>) -> Layout {
    let total: f32 = widths.iter().sum();
    if total <= available {
        return Layout {
            arrows: false,
            view: available,
            scroll: 0.0,
        };
    }
    let view = (available - 2.0 * ARROW_WIDTH).max(0.0);
    let max_scroll = (total - view).max(0.0);
    let mut scroll = scroll.clamp(0.0, max_scroll);
    if let Some(index) = selected.filter(|i| *i < widths.len()) {
        let start: f32 = widths[..index].iter().sum();
        let end = start + widths[index];
        if start < scroll {
            scroll = start;
        } else if end > scroll + view {
            scroll = end - view;
        }
        scroll = scroll.clamp(0.0, max_scroll);
    }
    Layout {
        arrows: true,
        view,
        scroll,
    }
}

/// What a press on a tab turned into.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Gesture {
    /// The tab is being dragged; `pos` is the pointer.
    Dragging { index: usize, pos: egui::Pos2 },
    /// The drag ended with the pointer at `pos`.
    Dropped { index: usize, pos: egui::Pos2 },
}

/// What the strip reports for a frame.
#[derive(Debug, Default)]
pub struct Output {
    /// A tab was clicked.
    pub clicked: Option<usize>,
    /// A tab is being dragged, or was dropped.
    pub gesture: Option<Gesture>,
    /// A tab asked to open in its own window from its menu.
    pub detach: Option<usize>,
    /// Where each visible tab is on the screen (empty for tabs scrolled out of view).
    pub tab_rects: Vec<(usize, Rect)>,
}

/// Where a dragged tab ends up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Drop<L> {
    /// Nothing happens.
    Cancel,
    /// The tab takes the place of the tab at this index in its own strip.
    Reorder(usize),
    /// The tab joins another window group.
    Merge(L),
    /// The tab becomes a window of its own.
    Undock,
}

/// Decides what dropping a tab at `pointer` does. `area` is the part of the window the dock covers,
/// `leaves` the window groups with their rectangles, `source` the group the tab came from and
/// `own_tabs` the tabs of its strip (so the tab can be put in another place of the same strip).
pub fn drop_target<L: Copy + PartialEq>(
    pointer: egui::Pos2,
    area: Rect,
    leaves: &[(L, Rect)],
    source: L,
    own_tabs: &[(usize, Rect)],
) -> Drop<L> {
    if !area.contains(pointer) {
        return Drop::Undock;
    }
    let Some((leaf, _)) = leaves.iter().find(|(_, rect)| rect.contains(pointer)) else {
        return Drop::Cancel;
    };
    if *leaf != source {
        return Drop::Merge(*leaf);
    }
    own_tabs
        .iter()
        .find(|(_, rect)| rect.x_range().contains(pointer.x) && rect.y_range().contains(pointer.y))
        .map_or(Drop::Cancel, |(index, _)| Drop::Reorder(*index))
}

/// Draws the strip in `ui` (which should be the 22 px footer) for tabs called `labels`, the tab at
/// `selected` being the active one. `id` must be the same from frame to frame. With `menu`, a tab has a
/// right-click menu to open it in a window of its own.
pub fn show(ui: &mut Ui, id: Id, labels: &[String], selected: usize, menu: bool) -> Output {
    let mut out = Output::default();
    let font = FontId::proportional(FONT_SIZE);
    let widths: Vec<f32> = labels
        .iter()
        .map(|label| {
            ui.fonts(|f| {
                f.layout_no_wrap(label.clone(), font.clone(), Color32::WHITE)
                    .size()
                    .x
            }) + 2.0 * PADDING_X
        })
        .collect();

    let full = ui.available_rect_before_wrap();
    let strip = Rect::from_min_size(full.min, vec2(full.width(), HEIGHT));
    ui.allocate_rect(strip, Sense::hover());

    let state_id = id.with("scroll");
    let asked = ui.data(|d| d.get_temp::<f32>(state_id)).unwrap_or(0.0);
    let hovering = ui.rect_contains_pointer(strip);
    let wheel = if hovering {
        let delta = ui.input(|i| i.smooth_scroll_delta);
        // a vertical wheel scrolls the strip sideways, a sideways one too
        -(delta.x + delta.y)
    } else {
        0.0
    };
    let lay = layout(&widths, strip.width(), asked + wheel, Some(selected));
    ui.data_mut(|d| d.insert_temp(state_id, lay.scroll));

    let visuals = ui.visuals().clone();
    ui.painter().rect_filled(strip, 0.0, visuals.faint_bg_color);
    let tabs_left = strip.min.x + if lay.arrows { ARROW_WIDTH } else { 0.0 };
    let tabs_clip = Rect::from_min_size(pos2(tabs_left, strip.min.y), vec2(lay.view, HEIGHT));

    let mut x = tabs_left - lay.scroll;
    for (index, label) in labels.iter().enumerate() {
        let rect = Rect::from_min_size(pos2(x, strip.min.y), vec2(widths[index], HEIGHT));
        x += widths[index];
        let visible = rect.intersect(tabs_clip);
        if visible.width() <= 0.0 {
            continue;
        }
        let response = ui.interact(visible, id.with(("tab", index)), Sense::click_and_drag());
        let active = index == selected;
        let fill = if active {
            visuals.selection.bg_fill
        } else if response.hovered() {
            visuals.widgets.hovered.weak_bg_fill
        } else {
            Color32::TRANSPARENT
        };
        let painter = ui.painter().with_clip_rect(tabs_clip);
        painter.rect_filled(rect, 0.0, fill);
        let color = if active {
            visuals.strong_text_color()
        } else {
            visuals.text_color()
        };
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            font.clone(),
            color,
        );
        out.tab_rects.push((index, visible));

        if response.clicked() {
            out.clicked = Some(index);
        }
        read_drag(&response, index, &mut out);
        if menu {
            response.context_menu(|ui| {
                if ui.button(tr("Abrir em janela própria")).clicked() {
                    out.detach = Some(index);
                    ui.close_menu();
                }
            });
        }
    }

    if lay.arrows {
        let left = Rect::from_min_size(strip.min, vec2(ARROW_WIDTH, HEIGHT));
        let right = Rect::from_min_size(
            pos2(strip.max.x - ARROW_WIDTH, strip.min.y),
            vec2(ARROW_WIDTH, HEIGHT),
        );
        let mut step = 0.0;
        for (rect, name, sign) in [(left, "left", -1.0), (right, "right", 1.0)] {
            let response = ui.interact(rect, id.with(name), Sense::click());
            let tint = if response.hovered() {
                visuals.text_color()
            } else {
                visuals.weak_text_color()
            };
            paint_arrow(ui, rect, sign < 0.0, tint);
            if response.clicked() {
                step += sign * SCROLL_STEP;
            }
        }
        if step != 0.0 {
            let moved = layout(&widths, strip.width(), lay.scroll + step, None).scroll;
            ui.data_mut(|d| d.insert_temp(state_id, moved));
            ui.ctx().request_repaint();
        }
    }
    out
}

fn read_drag(response: &Response, index: usize, out: &mut Output) {
    let Some(pos) = response.ctx.pointer_interact_pos() else {
        return;
    };
    if response.drag_stopped() {
        out.gesture = Some(Gesture::Dropped { index, pos });
    } else if response.dragged() {
        let start = response
            .ctx
            .input(|i| i.pointer.press_origin())
            .unwrap_or(pos);
        if (pos - start).length() >= DRAG_THRESHOLD {
            out.gesture = Some(Gesture::Dragging { index, pos });
        }
    }
}

fn paint_arrow(ui: &Ui, rect: Rect, left: bool, color: Color32) {
    let c = rect.center();
    let (a, b) = (3.5, 4.5);
    let points = if left {
        vec![
            pos2(c.x + a, c.y - b),
            pos2(c.x + a, c.y + b),
            pos2(c.x - a, c.y),
        ]
    } else {
        vec![
            pos2(c.x - a, c.y - b),
            pos2(c.x - a, c.y + b),
            pos2(c.x + a, c.y),
        ]
    };
    ui.painter().add(egui::Shape::convex_polygon(
        points,
        color,
        egui::Stroke::NONE,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect::from_min_max(pos2(x0, y0), pos2(x1, y1))
    }

    #[test]
    fn tabs_that_fit_need_no_arrows() {
        let l = layout(&[50.0, 60.0, 70.0], 300.0, 40.0, Some(2));
        assert!(!l.arrows);
        assert_eq!((l.scroll, l.view), (0.0, 300.0));
    }

    #[test]
    fn tabs_that_do_not_fit_get_arrows_and_a_narrower_view() {
        let l = layout(&[100.0, 100.0, 100.0], 200.0, 0.0, None);
        assert!(l.arrows);
        assert_eq!(l.view, 200.0 - 2.0 * ARROW_WIDTH);
    }

    #[test]
    fn the_selected_tab_is_always_scrolled_into_view() {
        let widths = [100.0; 6];
        // the last tab, from the start
        let l = layout(&widths, 250.0, 0.0, Some(5));
        let start: f32 = widths[..5].iter().sum();
        assert!(l.scroll + l.view >= start + widths[5] - 0.001);
        assert!(l.scroll <= start);
        // the first tab, from the end
        let l = layout(&widths, 250.0, 1000.0, Some(0));
        assert_eq!(l.scroll, 0.0);
        // a tab in the middle that already shows keeps the offset
        let l = layout(&widths, 250.0, 100.0, Some(2));
        assert_eq!(l.scroll, 100.0);
    }

    #[test]
    fn the_offset_is_kept_inside_what_is_possible() {
        let widths = [100.0; 4];
        let l = layout(&widths, 150.0, 9999.0, None);
        assert!(l.scroll <= 400.0 - l.view + 0.001);
        assert_eq!(layout(&widths, 150.0, -50.0, None).scroll, 0.0);
        // a selected index past the end is ignored, not a panic
        let _ = layout(&widths, 150.0, 0.0, Some(99));
    }

    #[test]
    fn dropping_outside_the_dock_area_makes_a_window() {
        let area = r(0.0, 0.0, 1000.0, 600.0);
        let leaves = [
            (1, r(0.0, 0.0, 500.0, 600.0)),
            (2, r(500.0, 0.0, 1000.0, 600.0)),
        ];
        assert_eq!(
            drop_target(pos2(1200.0, 300.0), area, &leaves, 1, &[]),
            Drop::Undock
        );
        assert_eq!(
            drop_target(pos2(300.0, -20.0), area, &leaves, 1, &[]),
            Drop::Undock
        );
    }

    #[test]
    fn dropping_on_another_group_joins_it() {
        let area = r(0.0, 0.0, 1000.0, 600.0);
        let leaves = [
            (1, r(0.0, 0.0, 500.0, 600.0)),
            (2, r(500.0, 0.0, 1000.0, 600.0)),
        ];
        assert_eq!(
            drop_target(pos2(800.0, 100.0), area, &leaves, 1, &[]),
            Drop::Merge(2)
        );
    }

    #[test]
    fn dropping_in_the_own_strip_reorders_and_elsewhere_in_the_own_group_cancels() {
        let area = r(0.0, 0.0, 1000.0, 600.0);
        let leaves = [
            (1, r(0.0, 0.0, 500.0, 600.0)),
            (2, r(500.0, 0.0, 1000.0, 600.0)),
        ];
        let tabs = [
            (0, r(0.0, 578.0, 80.0, 600.0)),
            (1, r(80.0, 578.0, 160.0, 600.0)),
        ];
        assert_eq!(
            drop_target(pos2(100.0, 590.0), area, &leaves, 1, &tabs),
            Drop::Reorder(1)
        );
        assert_eq!(
            drop_target(pos2(100.0, 200.0), area, &leaves, 1, &tabs),
            Drop::Cancel
        );
    }

    #[test]
    fn dropping_on_a_splitter_inside_the_area_cancels() {
        let area = r(0.0, 0.0, 1000.0, 600.0);
        let leaves = [
            (1, r(0.0, 0.0, 498.0, 600.0)),
            (2, r(502.0, 0.0, 1000.0, 600.0)),
        ];
        assert_eq!(
            drop_target(pos2(500.0, 100.0), area, &leaves, 1, &[]),
            Drop::Cancel
        );
    }
}
