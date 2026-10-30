//! The footer tab strips of the docked windows: what the dock looks like before a frame, what the
//! strips ask for during it, and what the dock does about it afterwards (select, reorder, join another
//! group, open in a window of its own, close). `egui_dock` draws the windows and the splitters; its
//! own strip is hidden (height 0) because it can only sit on top.

use eframe::egui::{self, vec2, Align2, Color32, FontId, Id, LayerId, Order, Rect, Stroke};
use egui_dock::{DockState, Node, NodeIndex, SurfaceIndex, TabDestination, TabIndex, TabInsert};
use neoveg_common::i18n::tr;

use super::Tab;
use crate::footer_tabs::{self, Drop, Gesture};

/// A window group of the main dock, as it was laid out last frame.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Leaf {
    pub surface: SurfaceIndex,
    pub node: NodeIndex,
    pub rect: Rect,
    pub tabs: Vec<Tab>,
    pub active: usize,
}

impl Leaf {
    fn id(&self) -> (usize, usize) {
        (self.surface.0, self.node.0)
    }
}

/// What a strip asked for during a frame.
struct Report {
    leaf: Leaf,
    output: footer_tabs::Output,
}

pub(super) struct DockFooter {
    leaves: Vec<Leaf>,
    area: Rect,
    reports: Vec<Report>,
}

impl Default for DockFooter {
    fn default() -> Self {
        Self {
            leaves: Vec::new(),
            area: Rect::NOTHING,
            reports: Vec::new(),
        }
    }
}

impl DockFooter {
    /// Reads the groups of the main dock; call before the dock is drawn. `area` is the part of the
    /// window the dock covers.
    pub fn refresh(&mut self, dock: &DockState<Tab>, area: Rect) {
        self.area = area;
        self.reports.clear();
        self.leaves = leaves_of(dock);
    }

    /// The group `tab` is in, when it is in the main dock (a detached panel is not).
    pub fn leaf_of(&self, tab: Tab) -> Option<&Leaf> {
        self.leaves.iter().find(|leaf| leaf.tabs.contains(&tab))
    }

    /// Draws the footer strip of the group `tab` is in, inside the footer area `ui`.
    pub fn strip(&mut self, ui: &mut egui::Ui, leaf: &Leaf) {
        let labels: Vec<String> = leaf.tabs.iter().map(|t| tr(t.title()).to_owned()).collect();
        let id = Id::new(("neoveg_dock_footer", leaf.id()));
        let output = footer_tabs::show(ui, id, &labels, leaf.active, true);
        self.reports.push(Report {
            leaf: leaf.clone(),
            output,
        });
    }

    /// Does what the strips asked for; call after the dock is drawn.
    pub fn apply(&mut self, dock: &mut DockState<Tab>, ctx: &egui::Context) {
        let reports = std::mem::take(&mut self.reports);
        for Report { leaf, output } in reports {
            let source = |index: usize| (leaf.surface, leaf.node, TabIndex(index));
            if let Some(index) = output.clicked {
                dock.set_active_tab(source(index));
                dock.set_focused_node_and_surface((leaf.surface, leaf.node));
            }
            if let Some(index) = output.detach {
                dock.detach_tab(source(index), leaf.rect);
                continue;
            }
            match output.gesture {
                Some(Gesture::Dragging { index, pos }) => {
                    let drop = self.target(pos, &leaf, &output.tab_rects);
                    self.paint_drag(ctx, &leaf, index, pos, drop);
                }
                Some(Gesture::Dropped { index, pos }) => {
                    match self.target(pos, &leaf, &output.tab_rects) {
                        Drop::Cancel => {}
                        Drop::Reorder(to) if to == index => {}
                        Drop::Reorder(to) => dock.move_tab(
                            source(index),
                            TabDestination::Node(
                                leaf.surface,
                                leaf.node,
                                TabInsert::Insert(TabIndex(to)),
                            ),
                        ),
                        Drop::Merge((surface, node)) => dock.move_tab(
                            source(index),
                            TabDestination::Node(
                                SurfaceIndex(surface),
                                NodeIndex(node),
                                TabInsert::Append,
                            ),
                        ),
                        Drop::Undock => {
                            let at = Rect::from_min_size(pos - vec2(40.0, 10.0), leaf.rect.size());
                            dock.detach_tab(source(index), at);
                        }
                    }
                }
                None => {}
            }
        }
    }

    fn target(&self, pos: egui::Pos2, from: &Leaf, own: &[(usize, Rect)]) -> Drop<(usize, usize)> {
        let leaves: Vec<((usize, usize), Rect)> =
            self.leaves.iter().map(|l| (l.id(), l.rect)).collect();
        footer_tabs::drop_target(pos, self.area, &leaves, from.id(), own)
    }

    /// While a tab is dragged: the name at the pointer and a frame on where it would go.
    fn paint_drag(
        &self,
        ctx: &egui::Context,
        from: &Leaf,
        index: usize,
        pos: egui::Pos2,
        drop: Drop<(usize, usize)>,
    ) {
        let painter = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::new("neoveg_dock_drag")));
        let accent = ctx.style().visuals.selection.bg_fill;
        match drop {
            Drop::Merge(id) => {
                if let Some(leaf) = self.leaves.iter().find(|l| l.id() == id) {
                    painter.rect_stroke(
                        leaf.rect.shrink(1.0),
                        egui::Rounding::ZERO,
                        Stroke::new(2.0_f32, accent),
                    );
                }
            }
            Drop::Undock => {
                painter.rect_stroke(
                    self.area.shrink(1.0),
                    egui::Rounding::ZERO,
                    Stroke::new(2.0_f32, Color32::from_rgb(230, 170, 60)),
                );
            }
            Drop::Reorder(_) | Drop::Cancel => {}
        }
        if let Some(tab) = from.tabs.get(index) {
            let font = FontId::proportional(footer_tabs::FONT_SIZE);
            let text = tr(tab.title());
            let rect = Rect::from_min_size(pos + vec2(12.0, 8.0), vec2(120.0, footer_tabs::HEIGHT));
            painter.rect_filled(rect, 0.0, accent);
            painter.text(
                rect.left_center() + vec2(6.0, 0.0),
                Align2::LEFT_CENTER,
                text,
                font,
                Color32::WHITE,
            );
        }
        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        ctx.request_repaint();
    }
}

/// The leaves of the main surface of `dock`, with the rectangle each had in the last frame.
pub(super) fn leaves_of(dock: &DockState<Tab>) -> Vec<Leaf> {
    let mut out = Vec::new();
    for (index, node) in dock.main_surface().iter().enumerate() {
        if let Node::Leaf {
            rect, tabs, active, ..
        } = node
        {
            out.push(Leaf {
                surface: SurfaceIndex::main(),
                node: NodeIndex(index),
                rect: *rect,
                tabs: tabs.clone(),
                active: active.0.min(tabs.len().saturating_sub(1)),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::pos2;

    fn dock() -> DockState<Tab> {
        super::super::default_dock()
    }

    #[test]
    fn every_panel_of_the_default_layout_is_in_a_leaf() {
        let footer = {
            let mut f = DockFooter::default();
            f.refresh(
                &dock(),
                Rect::from_min_size(pos2(0.0, 0.0), vec2(1600.0, 900.0)),
            );
            f
        };
        for tab in [
            Tab::Preview,
            Tab::Timeline,
            Tab::Media,
            Tab::MasterBus,
            Tab::AudioFx,
            Tab::Loudness,
        ] {
            assert!(footer.leaf_of(tab).is_some(), "{tab:?}");
        }
        // the active index never points past the tabs
        for leaf in &footer.leaves {
            assert!(leaf.active < leaf.tabs.len());
        }
    }

    #[test]
    fn moving_a_tab_to_another_group_leaves_no_empty_group() {
        let mut dock = dock();
        let before = leaves_of(&dock);
        let from = before
            .iter()
            .find(|l| l.tabs.contains(&Tab::Preview))
            .unwrap()
            .clone();
        let to = before
            .iter()
            .find(|l| l.tabs.contains(&Tab::Timeline))
            .unwrap()
            .clone();
        let index = from.tabs.iter().position(|t| *t == Tab::Preview).unwrap();
        dock.move_tab(
            (from.surface, from.node, TabIndex(index)),
            TabDestination::Node(to.surface, to.node, TabInsert::Append),
        );
        let after = leaves_of(&dock);
        assert!(after.iter().all(|l| !l.tabs.is_empty()));
        let joined = after
            .iter()
            .find(|l| l.tabs.contains(&Tab::Preview))
            .unwrap();
        assert!(joined.tabs.contains(&Tab::Timeline), "joined the group");
        // every panel is still somewhere, once
        let total: usize = after.iter().map(|l| l.tabs.len()).sum();
        let total_before: usize = before.iter().map(|l| l.tabs.len()).sum();
        assert_eq!(total, total_before);
    }

    #[test]
    fn reordering_inside_a_group_keeps_the_tabs() {
        let mut dock = dock();
        let leaf = leaves_of(&dock)
            .into_iter()
            .find(|l| l.tabs.len() >= 3)
            .expect("the effects group has three tabs");
        let first = leaf.tabs[0];
        dock.move_tab(
            (leaf.surface, leaf.node, TabIndex(0)),
            TabDestination::Node(
                leaf.surface,
                leaf.node,
                TabInsert::Insert(TabIndex(leaf.tabs.len() - 1)),
            ),
        );
        let after = leaves_of(&dock)
            .into_iter()
            .find(|l| l.node == leaf.node)
            .unwrap();
        assert_eq!(after.tabs.len(), leaf.tabs.len());
        assert_eq!(*after.tabs.last().unwrap(), first);
    }

    #[test]
    fn a_detached_tab_leaves_the_main_dock_and_its_group_closes_when_empty() {
        let mut dock = dock();
        let leaf = leaves_of(&dock)
            .into_iter()
            .find(|l| l.tabs.contains(&Tab::MasterBus))
            .unwrap();
        let index = leaf.tabs.iter().position(|t| *t == Tab::MasterBus).unwrap();
        let alone = leaf.tabs.len() == 1;
        dock.detach_tab(
            (leaf.surface, leaf.node, TabIndex(index)),
            Rect::from_min_size(pos2(10.0, 10.0), vec2(300.0, 200.0)),
        );
        let after = leaves_of(&dock);
        assert!(after.iter().all(|l| !l.tabs.contains(&Tab::MasterBus)));
        if alone {
            assert!(after
                .iter()
                .all(|l| l.node != leaf.node || !l.tabs.is_empty()));
        }
        // the tab is now in a floating window surface
        assert!(dock
            .iter_surfaces()
            .any(|s| s.iter_all_tabs().any(|(_, t)| *t == Tab::MasterBus)));
    }
}
