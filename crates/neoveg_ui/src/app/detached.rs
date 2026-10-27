//! Panels in their own native windows (AGENTS.md section 32).
//!
//! A panel dragged out of the dock becomes a floating `egui_dock` window; the moment it appears it
//! is promoted to an operating-system window (an egui viewport), which can be moved to any monitor.
//! Closing that window docks the panel back. `F11` toggles full screen on the window and `Esc`
//! leaves it. The pure parts (geometry, checks, saved format) are in `neoveg_common::window_layout`
//! and `crate::dock`.

use super::*;
use crate::dock;
use egui_dock::Surface;
use neoveg_common::window_layout::{SavedWindow, WindowGeometry};

/// A panel that lives in its own window.
pub(super) struct DetachedTab {
    pub tab: Tab,
    pub geometry: WindowGeometry,
    pub fullscreen: bool,
}

impl DetachedTab {
    fn viewport_id(&self) -> egui::ViewportId {
        viewport_id(self.tab)
    }
}

pub(super) fn viewport_id(tab: Tab) -> egui::ViewportId {
    egui::ViewportId::from_hash_of(("neoveg_detached", tab.id()))
}

const ALL_TABS: [Tab; 7] = [
    Tab::Preview,
    Tab::Timeline,
    Tab::Media,
    Tab::MasterBus,
    Tab::Loudness,
    Tab::AudioFx,
    Tab::Scopes,
];

impl Tab {
    /// Stable id used in the preferences.
    pub(super) fn id(self) -> &'static str {
        match self {
            Tab::Preview => "preview",
            Tab::Timeline => "timeline",
            Tab::Media => "media",
            Tab::MasterBus => "master_bus",
            Tab::Loudness => "loudness",
            Tab::AudioFx => "audio_fx",
            Tab::Scopes => "scopes",
        }
    }

    pub(super) fn from_id(id: &str) -> Option<Tab> {
        ALL_TABS.into_iter().find(|tab| tab.id() == id)
    }
}

/// Position of the main window's content on the screen, and the size of its monitor.
fn main_window(ctx: &egui::Context) -> (egui::Pos2, Option<(f32, f32)>) {
    ctx.input(|i| {
        let info = i.raw.viewports.get(&egui::ViewportId::ROOT);
        let origin = info
            .and_then(|v| v.inner_rect.or(v.outer_rect))
            .map_or(egui::Pos2::ZERO, |r| r.min);
        let monitor = info.and_then(|v| v.monitor_size).map(|m| (m.x, m.y));
        (origin, monitor)
    })
}

impl NeovegApp {
    /// Turns every floating dock window into native windows, one per panel.
    pub(super) fn promote_floating_windows(&mut self, ctx: &egui::Context) {
        let mut floating = Vec::new();
        for (index, surface) in self.dock.iter_surfaces().enumerate() {
            if let Surface::Window(_, state) = surface {
                let tabs: Vec<Tab> = surface.iter_all_tabs().map(|(_, tab)| *tab).collect();
                floating.push((index, state.rect(), tabs));
            }
        }
        if floating.is_empty() {
            return;
        }
        let (origin, _) = main_window(ctx);
        for (index, rect, tabs) in floating.into_iter().rev() {
            self.dock.remove_surface(egui_dock::SurfaceIndex(index));
            for (n, tab) in tabs.into_iter().enumerate() {
                let mut geometry = dock::from_dragged_panel(origin, rect);
                geometry.x += 30.0 * n as f32;
                geometry.y += 30.0 * n as f32;
                self.detach(tab, geometry);
            }
        }
        self.sync_detached_prefs(true);
    }

    fn detach(&mut self, tab: Tab, geometry: WindowGeometry) {
        if self.detached.iter().all(|d| d.tab != tab) {
            self.state.status = format!("{} em janela própria", tab.title());
            self.detached.push(DetachedTab {
                tab,
                geometry,
                fullscreen: false,
            });
        }
    }

    /// Puts the saved panel windows back, called once the preferences are loaded.
    pub(super) fn restore_detached(&mut self, ctx: &egui::Context) {
        let (_, monitor) = main_window(ctx);
        let saved = self.state.prefs.detached_windows.clone();
        for window in saved {
            let Some(tab) = Tab::from_id(&window.tab) else {
                continue;
            };
            if let Some(location) = self.dock.find_tab(&tab) {
                self.dock.remove_tab(location);
            }
            self.detach(tab, window.geometry.sanitized(monitor));
        }
    }

    /// A window that asked to be closed: its panel goes back into the dock.
    fn redock(&mut self, tab: Tab) {
        self.detached.retain(|d| d.tab != tab);
        if self.dock.find_tab(&tab).is_none() {
            self.dock.push_to_focused_leaf(tab);
        }
        self.state.status = format!("{} voltou para a janela principal", tab.title());
        self.sync_detached_prefs(true);
    }

    /// Mirrors the open windows into the preferences; `write` also saves them to disk.
    pub(super) fn sync_detached_prefs(&mut self, write: bool) {
        let saved: Vec<SavedWindow> = self
            .detached
            .iter()
            .map(|d| SavedWindow {
                tab: d.tab.id().to_owned(),
                geometry: d.geometry.clone(),
            })
            .collect();
        self.state.prefs.detached_windows = saved;
        if write {
            self.state.persist_preferences();
        }
    }

    /// Draws every detached panel in its window.
    pub(super) fn show_detached(&mut self, ctx: &egui::Context) {
        let mut windows = std::mem::take(&mut self.detached);
        let mut to_dock = Vec::new();
        for window in &mut windows {
            let builder = dock::viewport_builder(&window.geometry, window.tab.title());
            let tab = window.tab;
            let id = window.viewport_id();
            let state = &mut self.state;
            let mut close = false;
            let mut fullscreen = window.fullscreen;
            let mut geometry = window.geometry.clone();
            ctx.show_viewport_immediate(id, builder, |ctx, class| {
                if class == egui::ViewportClass::Embedded {
                    // The backend has no native windows: show the panel in the main window again
                    close = true;
                    return;
                }
                if ctx.input(|i| i.viewport().close_requested()) {
                    close = true;
                    return;
                }
                if ctx.input(|i| i.key_pressed(egui::Key::F11)) {
                    fullscreen = !fullscreen;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(fullscreen));
                } else if fullscreen && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                    fullscreen = false;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
                }
                state.handle_shortcuts(ctx);
                egui::CentralPanel::default()
                    .frame(egui::Frame::none())
                    .show(ctx, |ui| {
                        let mut tab = tab;
                        Viewer { state }.ui(ui, &mut tab);
                    });
                geometry = ctx.input(|i| dock::from_viewport(i.viewport(), &geometry));
            });
            window.fullscreen = fullscreen;
            window.geometry = geometry;
            if close {
                to_dock.push(tab);
            }
        }
        self.detached = windows;
        for tab in to_dock {
            self.redock(tab);
        }
        self.sync_detached_prefs(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_ids_are_unique_and_round_trip() {
        for tab in ALL_TABS {
            assert_eq!(Tab::from_id(tab.id()), Some(tab));
        }
        let mut ids: Vec<_> = ALL_TABS.iter().map(|t| t.id()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), ALL_TABS.len());
        assert_eq!(Tab::from_id("nope"), None);
    }

    fn app() -> NeovegApp {
        NeovegApp {
            dock: default_dock(),
            layouts: Default::default(),
            state: AppState::new(),
            splash: None,
            gpu: String::new(),
            detached: Vec::new(),
            detached_restored: true,
        }
    }

    #[test]
    fn a_floating_dock_window_becomes_a_native_window_and_redocks() {
        let ctx = egui::Context::default();
        let mut app = app();
        // The Scopes panel dragged out of the dock
        let location = app.dock.find_tab(&Tab::Scopes);
        if let Some(location) = location {
            app.dock.remove_tab(location);
        }
        app.dock.add_window(vec![Tab::Scopes]);
        app.promote_floating_windows(&ctx);
        assert_eq!(app.detached.len(), 1);
        assert_eq!(app.detached[0].tab, Tab::Scopes);
        assert!(app.dock.find_tab(&Tab::Scopes).is_none());
        assert!(app
            .dock
            .iter_surfaces()
            .all(|s| !matches!(s, Surface::Window(..))));
        assert_eq!(app.state.prefs.detached_windows.len(), 1);
        app.redock(Tab::Scopes);
        assert!(app.detached.is_empty());
        assert!(app.dock.find_tab(&Tab::Scopes).is_some());
        assert!(app.state.prefs.detached_windows.is_empty());
    }

    #[test]
    fn saved_windows_are_restored_and_unknown_panels_ignored() {
        let ctx = egui::Context::default();
        let mut app = app();
        app.state.prefs.detached_windows = vec![
            SavedWindow {
                tab: "timeline".into(),
                geometry: WindowGeometry {
                    x: f32::NAN,
                    ..WindowGeometry::default()
                },
            },
            SavedWindow {
                tab: "gone".into(),
                geometry: WindowGeometry::default(),
            },
        ];
        app.restore_detached(&ctx);
        assert_eq!(app.detached.len(), 1);
        assert!(app.detached[0].geometry.x.is_finite());
        assert!(app.dock.find_tab(&Tab::Timeline).is_none());
    }
}
