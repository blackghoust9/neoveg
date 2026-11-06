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
use neoveg_common::i18n::{tr, tr_args};
use neoveg_common::window_layout::{SavedWindow, WindowGeometry};

/// A panel that lives in its own window.
pub(super) struct DetachedTab {
    pub tab: Tab,
    /// Where the window is now, as the system reports it (saved in the preferences).
    pub geometry: WindowGeometry,
    /// Where it was opened. The viewport builder is made from this and never changes: a builder
    /// that follows `geometry` makes egui send the position of the previous frame back to the
    /// window while it is dragged, and the window shakes.
    pub opened_at: WindowGeometry,
    pub fullscreen: bool,
    /// Opened only for a full-screen review: `Esc` closes it and the panel goes back to the dock.
    pub temporary: bool,
}

impl DetachedTab {
    fn viewport_id(&self) -> egui::ViewportId {
        viewport_id(self.tab)
    }
}

pub(super) fn viewport_id(tab: Tab) -> egui::ViewportId {
    egui::ViewportId::from_hash_of(("neoveg_detached", tab.id()))
}

const ALL_TABS: [Tab; 9] = [
    Tab::Preview,
    Tab::Timeline,
    Tab::Media,
    Tab::MasterBus,
    Tab::Loudness,
    Tab::AudioFx,
    Tab::VideoFx,
    Tab::Transitions,
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
            Tab::VideoFx => "video_fx",
            Tab::Transitions => "transitions",
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
        if std::mem::take(&mut self.state.fullscreen_preview_requested) {
            self.toggle_fullscreen_preview(ctx);
        }
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

    /// *Prévia em tela cheia*: the preview panel in a window of its own, full screen on the
    /// monitor of the main window, until `Esc` (or the command again) takes it back to the dock.
    fn toggle_fullscreen_preview(&mut self, ctx: &egui::Context) {
        if let Some(window) = self.detached.iter_mut().find(|d| d.tab == Tab::Preview) {
            // Already in a window: toggle its full screen; a review window goes back when it ends
            window.fullscreen = !window.fullscreen;
            ctx.send_viewport_cmd_to(
                window.viewport_id(),
                egui::ViewportCommand::Fullscreen(window.fullscreen),
            );
            if !window.fullscreen && window.temporary {
                self.redock(Tab::Preview);
            }
            return;
        }
        if let Some(location) = self.dock.find_tab(&Tab::Preview) {
            self.dock.remove_tab(location);
        }
        let (origin, monitor) = main_window(ctx);
        let (w, h) = monitor.unwrap_or((1280.0, 720.0));
        let geometry = WindowGeometry {
            x: origin.x,
            y: origin.y,
            width: w,
            height: h,
            ..WindowGeometry::default()
        };
        self.detach(Tab::Preview, geometry);
        if let Some(window) = self.detached.iter_mut().find(|d| d.tab == Tab::Preview) {
            window.fullscreen = true;
            window.temporary = true;
        }
        self.state.status = tr("Prévia em tela cheia: Esc para voltar").to_string();
    }

    fn detach(&mut self, tab: Tab, geometry: WindowGeometry) {
        if self.detached.iter().all(|d| d.tab != tab) {
            self.state.status = tr_args("{} em janela própria", &[&tab.title()]);
            self.detached.push(DetachedTab {
                tab,
                opened_at: geometry.clone(),
                geometry,
                fullscreen: false,
                temporary: false,
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
        self.state.status = tr_args("{} voltou para a janela principal", &[&tab.title()]);
        self.sync_detached_prefs(true);
    }

    /// The panels in their own windows, as the preferences and the saved layouts keep them.
    pub(super) fn saved_windows(&self) -> Vec<SavedWindow> {
        self.detached
            .iter()
            .filter(|d| !d.temporary)
            .map(|d| SavedWindow {
                tab: d.tab.id().to_owned(),
                geometry: d.geometry.clone(),
            })
            .collect()
    }

    /// A saved layout's panel windows take the place of the ones that are open: windows the layout
    /// does not have go away (their panel is simply not shown, as in the layout), the others open or
    /// move to where the layout had them.
    pub(super) fn apply_layout_windows(&mut self, windows: &[SavedWindow]) {
        let ctx = self.state.tabs.ctx.clone();
        let monitor = ctx.as_ref().and_then(|c| main_window(c).1);
        let open: Vec<Tab> = self.detached.iter().map(|d| d.tab).collect();
        self.detached.clear();
        for window in windows {
            let Some(tab) = Tab::from_id(&window.tab) else {
                continue;
            };
            if let Some(location) = self.dock.find_tab(&tab) {
                self.dock.remove_tab(location);
            }
            let geometry = window.geometry.sanitized(monitor);
            if let Some(ctx) = ctx.as_ref().filter(|_| open.contains(&tab)) {
                // The window is already open: it is moved, its builder only counts when it is created
                let id = viewport_id(tab);
                ctx.send_viewport_cmd_to(
                    id,
                    egui::ViewportCommand::OuterPosition(egui::pos2(geometry.x, geometry.y)),
                );
                ctx.send_viewport_cmd_to(
                    id,
                    egui::ViewportCommand::InnerSize(egui::vec2(geometry.width, geometry.height)),
                );
            }
            self.detach(tab, geometry);
        }
        self.sync_detached_prefs(true);
    }

    /// Mirrors the open windows into the preferences; `write` also saves them to disk.
    pub(super) fn sync_detached_prefs(&mut self, write: bool) {
        self.state.prefs.detached_windows = self.saved_windows();
        if write {
            self.state.persist_preferences();
        }
    }

    /// Draws every detached panel in its window.
    pub(super) fn show_detached(&mut self, ctx: &egui::Context) {
        let mut windows = std::mem::take(&mut self.detached);
        let mut to_dock = Vec::new();
        for window in &mut windows {
            // A review window opens in full screen; the flag never changes during its life
            let builder = dock::viewport_builder(&window.opened_at, window.tab.title())
                .with_fullscreen(window.temporary);
            let tab = window.tab;
            let id = window.viewport_id();
            let state = &mut self.state;
            let mut close = false;
            let mut fullscreen = window.fullscreen;
            let temporary = window.temporary;
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
                let (f11, esc) = ctx.input(|i| {
                    (
                        i.key_pressed(egui::Key::F11),
                        i.key_pressed(egui::Key::Escape),
                    )
                });
                let (next, ends) =
                    neoveg_common::window_layout::fullscreen_step(fullscreen, temporary, f11, esc);
                if next != fullscreen {
                    fullscreen = next;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(fullscreen));
                }
                if ends {
                    close = true;
                    return;
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
            state: AppState::new(),
            splash: None,
            gpu: String::new(),
            detached: Vec::new(),
            detached_restored: true,
        }
    }

    #[test]
    fn a_layout_brings_back_its_panel_windows_and_the_chosen_folder_is_used() {
        let base = std::env::temp_dir().join(format!("neoveg_app_detached_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (dir, other) = (base.join("a"), base.join("b"));
        let mut app = app();
        app.state.layouts_dir = Some(dir);
        // The scopes panel in a window of its own, out of the dock
        if let Some(found) = app.dock.find_tab(&Tab::Scopes) {
            app.dock.remove_tab(found);
        }
        app.detach(
            Tab::Scopes,
            WindowGeometry {
                x: 300.0,
                ..WindowGeometry::default()
            },
        );
        app.apply_window_action(WindowAction::SaveLayout {
            name: "Dois monitores".into(),
            slot: None,
        });
        let path = app
            .state
            .current_layout
            .clone()
            .expect("the layout is selected");
        assert_eq!(
            app.state.layouts[0].file.detached.as_ref().map(Vec::len),
            Some(1),
            "the window is in the file"
        );
        // Close the window (the panel goes back into the dock), then load: it is a window again
        app.redock(Tab::Scopes);
        assert!(app.detached.is_empty() && app.dock.find_tab(&Tab::Scopes).is_some());
        app.apply_window_action(WindowAction::LoadLayout(path));
        assert_eq!(app.detached.len(), 1);
        assert_eq!(app.detached[0].tab, Tab::Scopes);
        assert!(
            app.dock.find_tab(&Tab::Scopes).is_none(),
            "not in both places"
        );
        // A layout saved with no panel window closes the one that is open
        app.redock(Tab::Scopes);
        app.apply_window_action(WindowAction::SaveLayout {
            name: "Tudo junto".into(),
            slot: None,
        });
        let together = app.state.current_layout.clone().unwrap();
        app.dock
            .remove_tab(app.dock.find_tab(&Tab::Scopes).unwrap());
        app.detach(Tab::Scopes, WindowGeometry::default());
        app.apply_window_action(WindowAction::LoadLayout(together));
        assert!(app.detached.is_empty());
        assert!(app.dock.find_tab(&Tab::Scopes).is_some());
        // The folder of the layouts can be chosen, and the default one comes back
        app.apply_window_action(WindowAction::SetLayoutsDir(Some(other.clone())));
        assert_eq!(app.state.layouts_dir, Some(other.clone()));
        assert!(
            app.state.layouts.is_empty(),
            "that folder has no layouts yet"
        );
        assert_eq!(app.state.prefs.layouts_dir, other.to_string_lossy());
        app.apply_window_action(WindowAction::SetLayoutsDir(None));
        assert_eq!(app.state.layouts_dir, crate::layouts::dir());
        assert!(app.state.prefs.layouts_dir.is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn saved_layouts_are_saved_loaded_by_number_reloaded_and_deleted() {
        let dir = std::env::temp_dir().join(format!("neoveg_app_layouts_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = app();
        app.state.layouts_dir = Some(dir.clone());
        // Nothing saved: recalling a number says so and leaves the dock alone
        app.apply_window_action(WindowAction::RecallSlot(3));
        assert!(app.state.status.contains("Nenhum layout no atalho 3"));
        // Save the default arrangement as "Edição" on number 3
        let saved = default_dock();
        app.apply_window_action(WindowAction::SaveLayout {
            name: "Edição".into(),
            slot: Some(3),
        });
        assert!(app.state.current_layout.is_some());
        assert_eq!(app.state.layouts.len(), 1);
        // Rearrange the dock, then recall by number: the saved arrangement is back
        app.dock = DockState::new(vec![Tab::Preview]);
        assert!(app.dock.find_tab(&Tab::Timeline).is_none());
        app.apply_window_action(WindowAction::RecallSlot(3));
        assert_eq!(
            app.dock.find_tab(&Tab::Timeline).is_some(),
            saved.find_tab(&Tab::Timeline).is_some()
        );
        assert!(app.dock.find_tab(&Tab::Timeline).is_some());
        // Changes since the load are undone by reloading the selected layout
        app.dock = DockState::new(vec![Tab::Preview]);
        app.apply_window_action(WindowAction::ReloadLayout);
        assert!(app.dock.find_tab(&Tab::Timeline).is_some());
        // Save over the selected one keeps its name and number
        app.dock = DockState::new(vec![Tab::Preview, Tab::Timeline]);
        app.apply_window_action(WindowAction::SaveCurrentLayout);
        assert_eq!(app.state.layouts.len(), 1);
        assert_eq!(app.state.layouts[0].file.slot, Some(3));
        assert!(app.state.layouts[0]
            .file
            .dock
            .find_tab(&Tab::Media)
            .is_none());
        // Ctrl+Alt+D then 5 saves "Layout 5"; the shortcut can move and the layout can go
        app.apply_window_action(WindowAction::SaveSlot(5));
        assert!(app
            .state
            .layouts
            .iter()
            .any(|l| l.file.name == "Layout 5" && l.file.slot == Some(5)));
        let first = app
            .state
            .layouts
            .iter()
            .find(|l| l.file.slot == Some(3))
            .unwrap()
            .path
            .clone();
        app.apply_window_action(WindowAction::SetLayoutSlot(first.clone(), Some(5)));
        assert_eq!(
            app.state
                .layouts
                .iter()
                .filter(|l| l.file.slot == Some(5))
                .count(),
            1
        );
        app.apply_window_action(WindowAction::DeleteLayout(first));
        assert_eq!(app.state.layouts.len(), 1);
        assert!(
            app.state.current_layout.is_some(),
            "the saved Layout 5 stays selected"
        );
        // The chord: Alt+D, then a number key within the time
        let ctx = egui::Context::default();
        app.state.run_command(Command::ViewLayoutLoad, &ctx, 10.0);
        assert!(app.state.layout_chord.is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn show_brings_a_window_forward_or_back_and_never_closes_it() {
        let mut app = app();
        let ctx = egui::Context::default();
        let _ = &ctx;
        // In the dock: Show keeps it there (Toggle would close it when it is in front)
        assert!(app.dock.find_tab(&Tab::VideoFx).is_some());
        app.apply_window_action(WindowAction::Show(Tab::VideoFx));
        app.apply_window_action(WindowAction::Show(Tab::VideoFx));
        assert!(app.dock.find_tab(&Tab::VideoFx).is_some());
        // Closed: Show puts it back
        let found = app.dock.find_tab(&Tab::VideoFx).unwrap();
        app.dock.remove_tab(found);
        assert!(app.dock.find_tab(&Tab::VideoFx).is_none());
        app.apply_window_action(WindowAction::Show(Tab::VideoFx));
        assert!(app.dock.find_tab(&Tab::VideoFx).is_some());
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
    fn the_window_builder_stays_where_the_window_was_opened() {
        let mut app = app();
        app.detach(Tab::Scopes, WindowGeometry::default());
        // The system moves the window: the saved geometry follows, the builder does not
        app.detached[0].geometry.x += 500.0;
        let window = &app.detached[0];
        let builder = dock::viewport_builder(&window.opened_at, window.tab.title());
        assert_eq!(
            builder.position,
            Some(egui::pos2(window.opened_at.x, window.opened_at.y))
        );
        assert_ne!(window.opened_at.x, window.geometry.x);
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

#[cfg(test)]
mod trimmer_tests {
    use super::*;

    #[test]
    fn the_trimmer_adds_its_selection_as_one_undo_step() {
        let mut state = AppState::new();
        state.project.media_pool.push(MediaRef {
            subproject: false,
            format: None,
            id: 5,
            path: "C:/clips/take.wav".into(),
            duration_seconds: 10.0,
        });
        state.open_in_trimmer(5);
        let trimmer = state.trimmer.as_mut().unwrap();
        trimmer.select(2.0, 6.0);
        state.playhead = 3.0;
        state.trimmer_add(TrimmerAdd::FromCursor);
        let events: Vec<_> = state
            .timeline
            .tracks
            .iter()
            .flat_map(|t| &t.events)
            .collect();
        assert_eq!(events.len(), 1);
        // The marks sit on frames of the project rate (29.97 fps by default)
        assert_eq!(events[0].start, 3.0);
        assert!((events[0].duration - 4.0).abs() < 0.04);
        assert!((events[0].trim_in - 2.0).abs() < 0.04);
        assert!(state.history.undo(&mut state.timeline));
        assert!(state.timeline.tracks.iter().all(|t| t.events.is_empty()));
    }
}

#[cfg(test)]
mod fullscreen_preview_tests {
    use super::*;

    #[test]
    fn the_command_opens_a_temporary_full_screen_preview_and_the_second_press_docks_it_back() {
        let mut app = NeovegApp {
            dock: default_dock(),
            state: AppState::new(),
            splash: None,
            gpu: String::new(),
            detached: Vec::new(),
            detached_restored: true,
        };
        let ctx = egui::Context::default();
        app.toggle_fullscreen_preview(&ctx);
        assert_eq!(app.detached.len(), 1);
        let window = &app.detached[0];
        assert!(window.tab == Tab::Preview && window.fullscreen && window.temporary);
        assert!(
            app.dock.find_tab(&Tab::Preview).is_none(),
            "not in both places"
        );
        assert!(
            app.saved_windows().is_empty(),
            "a review window is not saved"
        );
        app.toggle_fullscreen_preview(&ctx);
        assert!(app.detached.is_empty());
        assert!(app.dock.find_tab(&Tab::Preview).is_some());
    }

    #[test]
    fn the_command_flag_is_consumed_by_the_window_code() {
        let mut app = NeovegApp {
            dock: default_dock(),
            state: AppState::new(),
            splash: None,
            gpu: String::new(),
            detached: Vec::new(),
            detached_restored: true,
        };
        app.state.fullscreen_preview_requested = true;
        app.promote_floating_windows(&egui::Context::default());
        assert!(!app.state.fullscreen_preview_requested);
        assert_eq!(app.detached.len(), 1);
    }
}
