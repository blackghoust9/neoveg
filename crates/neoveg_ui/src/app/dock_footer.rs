//! The footer tab strips of the docked windows: what the dock looks like before a frame, what the
//! strips ask for during it, and what the dock does about it afterwards (select, reorder, join another
//! group, open in a window of its own, close). `egui_dock` draws the windows and the splitters; its
//! own strip is hidden (height 0) because it can only sit on top.

use eframe::egui::{self, vec2, Align2, Color32, FontId, Id, LayerId, Order, Rect, Stroke};
use egui_dock::{DockState, Node, NodeIndex, SurfaceIndex, TabDestination, TabIndex, TabInsert};
use neoveg_common::i18n::tr;

use super::Tab;
use crate::footer_tabs::{self, Drop, Gesture, GroupAction, GroupMenu};

/// The places *Mover grupo para* offers, each found by the window that stands for it: a group goes
/// to the window group that window is in now. (The names are the ones of the reference layout; the
/// default layout puts each of these windows in a group of its own.)
pub(super) const DESTINATIONS: [(&str, Tab); 6] = [
    ("Origem", Tab::Preview),
    ("Propriedades", Tab::AudioFx),
    ("Projeto", Tab::Media),
    ("Ferramentas", Tab::Scopes),
    ("Linha do tempo", Tab::Timeline),
    ("Medidores de áudio", Tab::MasterBus),
];

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
    /// The dock is showing one maximized group.
    maximized: bool,
}

impl Default for DockFooter {
    fn default() -> Self {
        Self {
            leaves: Vec::new(),
            area: Rect::NOTHING,
            reports: Vec::new(),
            maximized: false,
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

    /// Tells the strips whether a group is maximized (their menu then offers to restore it).
    pub fn set_maximized(&mut self, on: bool) {
        self.maximized = on;
    }

    /// The group `tab` is in, when it is in the main dock (a detached panel is not).
    pub fn leaf_of(&self, tab: Tab) -> Option<&Leaf> {
        self.leaves.iter().find(|leaf| leaf.tabs.contains(&tab))
    }

    /// Draws the footer strip of the group `tab` is in, inside the footer area `ui`.
    pub fn strip(&mut self, ui: &mut egui::Ui, leaf: &Leaf) {
        let labels: Vec<String> = leaf.tabs.iter().map(|t| tr(t.title()).to_owned()).collect();
        let id = Id::new(("neoveg_dock_footer", leaf.id()));
        let menu = GroupMenu {
            maximized: self.maximized,
            destinations: DESTINATIONS
                .iter()
                .map(|(name, anchor)| {
                    let elsewhere = self
                        .leaf_of(*anchor)
                        .is_some_and(|other| other.id() != leaf.id());
                    (tr(name).to_owned(), elsewhere && !self.maximized)
                })
                .collect(),
        };
        let output = footer_tabs::show_with(ui, id, &labels, leaf.active, Some(&menu));
        self.reports.push(Report {
            leaf: leaf.clone(),
            output,
        });
    }

    /// Does what the strips asked for; call after the dock is drawn. What the group menus asked
    /// for is not done here (it can change more than the dock): it is handed back.
    pub fn apply(
        &mut self,
        dock: &mut DockState<Tab>,
        ctx: &egui::Context,
    ) -> Vec<(Leaf, GroupAction)> {
        let mut groups = Vec::new();
        let reports = std::mem::take(&mut self.reports);
        for Report { leaf, output } in reports {
            if let Some(action) = output.group {
                groups.push((leaf.clone(), action));
                continue;
            }
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
        groups
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

/// Closes the windows of a group (they come back with the window commands).
pub(super) fn close_group(dock: &mut DockState<Tab>, tabs: &[Tab]) {
    for tab in tabs {
        if let Some(location) = dock.find_tab(tab) {
            dock.remove_tab(location);
        }
    }
}

/// Floats every window of a group (the app then gives each a native window of its own).
pub(super) fn undock_group(dock: &mut DockState<Tab>, tabs: &[Tab], rect: Rect) {
    for (n, tab) in tabs.iter().enumerate() {
        if let Some(location) = dock.find_tab(tab) {
            let offset = vec2(30.0, 30.0) * n as f32;
            dock.detach_tab(
                location,
                Rect::from_min_size(rect.min + offset, rect.size()),
            );
        }
    }
}

/// Moves every window of a group into the group that holds `anchor`. The place is looked up again
/// for each window, because taking one out can renumber the groups. `false` when `anchor` is not in
/// the dock, or is in the group being moved.
pub(super) fn move_group(dock: &mut DockState<Tab>, tabs: &[Tab], anchor: Tab) -> bool {
    if tabs.contains(&anchor) || dock.find_tab(&anchor).is_none() {
        return false;
    }
    for tab in tabs {
        let (Some(from), Some((surface, node, _))) = (dock.find_tab(tab), dock.find_tab(&anchor))
        else {
            continue;
        };
        dock.move_tab(from, TabDestination::Node(surface, node, TabInsert::Append));
    }
    true
}

/// A dock with only `tabs`, `active` in front: the maximized view of a group.
pub(super) fn maximized_dock(tabs: &[Tab], active: Tab) -> DockState<Tab> {
    let mut dock = DockState::new(tabs.to_vec());
    if let Some(location) = dock.find_tab(&active) {
        dock.set_active_tab(location);
    }
    dock
}

/// The dock to go back to after a maximized view: `saved` as it was, without the windows that have
/// a native window of their own now, and with the windows that were opened meanwhile in the group
/// that was maximized.
pub(super) fn restored_dock(
    mut saved: DockState<Tab>,
    maximized: &DockState<Tab>,
    floating: &[Tab],
) -> DockState<Tab> {
    for tab in floating {
        if let Some(location) = saved.find_tab(tab) {
            saved.remove_tab(location);
        }
    }
    let opened: Vec<Tab> = maximized
        .iter_all_tabs()
        .map(|(_, tab)| *tab)
        .filter(|tab| saved.find_tab(tab).is_none())
        .collect();
    for tab in opened {
        saved.push_to_focused_leaf(tab);
    }
    saved
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

    fn group_of(dock: &DockState<Tab>, tab: Tab) -> Leaf {
        leaves_of(dock)
            .into_iter()
            .find(|l| l.tabs.contains(&tab))
            .expect("the panel is in a group")
    }

    fn panels(dock: &DockState<Tab>) -> Vec<Tab> {
        let mut all: Vec<Tab> = dock.iter_all_tabs().map(|(_, t)| *t).collect();
        all.sort_by_key(|t| t.id());
        all
    }

    #[test]
    fn closing_a_group_removes_all_its_windows_and_only_those() {
        let mut dock = dock();
        let before = panels(&dock);
        let effects = group_of(&dock, Tab::AudioFx);
        assert_eq!(effects.tabs.len(), 3);
        close_group(&mut dock, &effects.tabs);
        let after = panels(&dock);
        assert_eq!(after.len(), before.len() - 3);
        assert!(effects.tabs.iter().all(|t| !after.contains(t)));
        assert!(after.contains(&Tab::Timeline) && after.contains(&Tab::Preview));
        assert!(leaves_of(&dock).iter().all(|l| !l.tabs.is_empty()));
    }

    #[test]
    fn undocking_a_group_floats_every_window_of_it() {
        let mut dock = dock();
        let effects = group_of(&dock, Tab::AudioFx);
        undock_group(
            &mut dock,
            &effects.tabs,
            Rect::from_min_size(pos2(40.0, 40.0), vec2(300.0, 200.0)),
        );
        // None is in the main dock any more, and all are still somewhere in a floating surface
        assert!(leaves_of(&dock)
            .iter()
            .all(|l| effects.tabs.iter().all(|t| !l.tabs.contains(t))));
        let all = panels(&dock);
        assert!(effects.tabs.iter().all(|t| all.contains(t)));
        let floating = dock
            .iter_surfaces()
            .skip(1)
            .flat_map(|s| s.iter_all_tabs().map(|(_, t)| *t).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        assert_eq!(floating.len(), 3);
    }

    #[test]
    fn a_group_moves_into_the_group_of_the_place_it_is_sent_to() {
        let mut dock = dock();
        let before = panels(&dock);
        let effects = group_of(&dock, Tab::AudioFx);
        // Every place of the menu exists in the default layout
        for (_, anchor) in DESTINATIONS {
            assert!(dock.find_tab(&anchor).is_some(), "{anchor:?}");
        }
        assert!(move_group(&mut dock, &effects.tabs, Tab::Timeline));
        let timeline = group_of(&dock, Tab::Timeline);
        assert!(effects.tabs.iter().all(|t| timeline.tabs.contains(t)));
        assert_eq!(panels(&dock), before, "nothing is lost or duplicated");
        assert!(leaves_of(&dock).iter().all(|l| !l.tabs.is_empty()));
        // A group cannot go to a place that is inside itself, nor to one that is not on screen
        let again = group_of(&dock, Tab::AudioFx);
        assert!(!move_group(&mut dock, &again.tabs, Tab::Timeline));
        let mut small = DockState::new(vec![Tab::Preview]);
        assert!(!move_group(&mut small, &[Tab::Preview], Tab::Media));
    }

    #[test]
    fn a_maximized_group_shows_only_its_windows_and_the_dock_comes_back_whole() {
        let saved = dock();
        let effects = group_of(&saved, Tab::AudioFx);
        let shown = maximized_dock(&effects.tabs, Tab::VideoFx);
        assert_eq!(panels(&shown).len(), 3);
        let leaf = group_of(&shown, Tab::VideoFx);
        assert_eq!(
            leaf.tabs[leaf.active],
            Tab::VideoFx,
            "the active one stays in front"
        );
        // Back to the whole dock: nothing changed meanwhile
        let back = restored_dock(saved.clone(), &shown, &[]);
        assert_eq!(panels(&back), panels(&saved));
        // A window that got its own native window meanwhile is not docked again, and one opened
        // meanwhile is kept
        let mut without_scopes = saved.clone();
        let scopes = without_scopes.find_tab(&Tab::Scopes).unwrap();
        without_scopes.remove_tab(scopes);
        let mut shown = shown;
        shown.push_to_focused_leaf(Tab::Scopes);
        let back = restored_dock(without_scopes, &shown, &[Tab::Preview]);
        assert!(back.find_tab(&Tab::Preview).is_none());
        assert!(back.find_tab(&Tab::Scopes).is_some());
        assert_eq!(panels(&back).len(), panels(&saved).len() - 1);
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
