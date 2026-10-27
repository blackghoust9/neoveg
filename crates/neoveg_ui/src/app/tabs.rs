//! Project tabs and nested sub-projects: the UI side of `neoveg_engine::project_manager`.
//!
//! The active project lives in the fields of [`AppState`]; the other open projects live in the
//! manager. Changing tab stashes the active project (with its decoded media and view) into the
//! manager and brings the other one back, so every editing function keeps working on `self.project`
//! and `self.timeline` unchanged.

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::Path;

use neoveg_engine::project_manager::{
    assign_event_ids, flatten_subprojects, Flattened, Project, ProjectId, ProjectManager,
};

use super::*;

/// What belongs to a tab besides the project itself.
pub(super) struct TabExtras {
    media: HashMap<u64, LoadedMedia>,
    video_info: HashMap<u64, VideoInfo>,
    playhead: f64,
    scroll_seconds: f64,
    selected_track: Option<TrackId>,
    selection: Selection,
    time_selection: Option<TimeRange>,
}

/// The sub-projects of the active project, expanded; valid while `(revision, epoch)` stays the same.
struct FlatCache {
    revision: u64,
    epoch: u64,
    flat: Option<Arc<Flattened>>,
}

#[derive(Default)]
pub(super) struct Tabs {
    manager: ProjectManager,
    /// The tab being edited (its project is in the `AppState` fields, not in the manager).
    active: Option<ProjectId>,
    extras: HashMap<ProjectId, TabExtras>,
    /// Bumped whenever a project is saved, switched or closed: sub-projects may have changed.
    epoch: u64,
    flat: RefCell<Option<FlatCache>>,
    /// Media of sub-projects already asked to load (their ids are hashes of the file path).
    nested_requested: RefCell<HashSet<u64>>,
    /// A dirty tab the user is closing, waiting for save / discard / cancel.
    pub(super) pending_close: Option<ProjectId>,
    /// A `.neoveg` to open in a tab (set by a double click, carried out with the window context).
    pub(super) open_request: Option<PathBuf>,
    pub(super) ctx: Option<egui::Context>,
}

impl Tabs {
    pub(super) fn bump(&mut self) {
        self.epoch += 1;
    }
}

impl AppState {
    /// Makes sure the active project has a tab (the first project gets one lazily).
    pub(super) fn ensure_tab(&mut self) -> ProjectId {
        if let Some(id) = self.tabs.active {
            return id;
        }
        let id = self
            .tabs
            .manager
            .register(ProjectInfo::create_empty(), Timeline::new(), None);
        // The manager only keeps the id and the position: the data is in this struct
        self.tabs.manager.take(id);
        self.tabs.active = Some(id);
        id
    }

    /// Title of a tab: the active one is computed from the live state.
    pub(super) fn tab_title(&self, id: ProjectId) -> String {
        if Some(id) == self.tabs.active {
            let name = self
                .project_path
                .as_deref()
                .and_then(std::path::Path::file_stem)
                .map_or_else(
                    || self.project.name.clone(),
                    |s| s.to_string_lossy().into_owned(),
                );
            return if self.is_dirty() {
                format!("{name} *")
            } else {
                name
            };
        }
        self.tabs
            .manager
            .get(id)
            .map_or_else(|| "?".into(), Project::title)
    }

    /// Ids in tab order.
    pub(super) fn tab_ids(&self) -> Vec<ProjectId> {
        self.tabs.manager.ids().to_vec()
    }

    /// Moves the active project into the manager.
    fn stash_active(&mut self) {
        let id = self.ensure_tab();
        let project = Project {
            id,
            info: std::mem::replace(&mut self.project, ProjectInfo::create_empty()),
            timeline: std::mem::take(&mut self.timeline),
            history: std::mem::take(&mut self.history),
            path: self.project_path.take(),
            saved_revision: self.saved_revision,
        };
        self.tabs.extras.insert(
            id,
            TabExtras {
                media: std::mem::take(&mut self.media),
                video_info: std::mem::take(&mut self.video_info),
                playhead: self.playhead,
                scroll_seconds: self.scroll_seconds,
                selected_track: self.selected_track.take(),
                selection: std::mem::take(&mut self.selection),
                time_selection: self.time_selection.take(),
            },
        );
        self.tabs.manager.put(project);
    }

    /// Clears what a project's view keeps (the caches follow the active project).
    fn reset_view(&mut self) {
        self.video_texture = None;
        self.last_frame_key = None;
        self.frame_scopes = None;
        self.tabs.flat.borrow_mut().take();
        self.tabs.nested_requested.borrow_mut().clear();
        self.tabs.bump();
    }

    /// Brings the project `id` out of the manager into the live fields. Returns whether its decoded
    /// media were kept (`false` = they must be loaded again).
    fn restore_tab(&mut self, id: ProjectId) -> bool {
        let Some(project) = self.tabs.manager.take(id) else {
            return false;
        };
        self.project = project.info;
        self.timeline = project.timeline;
        self.history = project.history;
        self.project_path = project.path;
        self.saved_revision = project.saved_revision;
        self.forced_dirty = false;
        self.last_autosave_revision = None;
        self.tabs.manager.switch_active_project(id);
        self.tabs.active = Some(id);
        let kept = match self.tabs.extras.remove(&id) {
            Some(x) => {
                self.media = x.media;
                self.video_info = x.video_info;
                self.playhead = x.playhead;
                self.scroll_seconds = x.scroll_seconds;
                self.selected_track = x.selected_track;
                self.selection = x.selection;
                self.time_selection = x.time_selection;
                true
            }
            None => {
                self.media.clear();
                self.video_info.clear();
                self.playhead = 0.0;
                self.scroll_seconds = 0.0;
                self.selected_track = None;
                self.selection.clear();
                self.time_selection = None;
                false
            }
        };
        self.reset_view();
        kept
    }

    /// Decodes the media of the active project again (after a tab brought it back without them).
    fn reload_media(&mut self, ctx: &egui::Context) {
        let wanted: Vec<(u64, PathBuf)> = self
            .project
            .media_pool
            .iter()
            .filter(|m| !m.subproject && Path::new(&m.path).exists())
            .map(|m| (m.id, PathBuf::from(&m.path)))
            .collect();
        for (id, path) in wanted {
            self.spawn_media(path, Some(id), ctx);
        }
    }

    pub(super) fn switch_tab(&mut self, id: ProjectId, ctx: &egui::Context) {
        let current = self.ensure_tab();
        if id == current || self.tabs.manager.get(id).is_none() {
            return;
        }
        self.stop();
        self.stash_active();
        if !self.restore_tab(id) {
            self.reload_media(ctx);
        }
        self.status = format!("Projeto: {}", self.tab_title(id));
    }

    /// File > New: an empty project in a new tab.
    pub(super) fn new_tab(&mut self) {
        self.ensure_tab();
        self.stop();
        self.stash_active();
        let id = self
            .tabs
            .manager
            .register(ProjectInfo::create_empty(), Timeline::new(), None);
        self.tabs.manager.take(id);
        self.tabs.active = Some(id);
        self.tabs.manager.switch_active_project(id);
        self.project = ProjectInfo::create_empty();
        if let Some(settings) = &self.prefs.new_project_settings {
            self.project.settings = settings.clone();
        }
        self.timeline = Timeline::new();
        self.history = History::new();
        self.project_path = None;
        self.saved_revision = 0;
        self.forced_dirty = false;
        self.last_autosave_revision = None;
        self.media.clear();
        self.video_info.clear();
        self.playhead = 0.0;
        self.scroll_seconds = 0.0;
        self.selected_track = None;
        self.selection.clear();
        self.time_selection = None;
        self.reset_view();
        self.status = "Novo projeto".into();
    }

    /// Opens a project file in a tab: the tab of the file when it is already open, else a new one.
    pub(super) fn open_path_in_tab(&mut self, path: &Path, ctx: &egui::Context) {
        self.ensure_tab();
        let same = |p: &Option<PathBuf>| {
            p.as_deref()
                .is_some_and(|p| p.canonicalize().ok() == path.canonicalize().ok())
        };
        if same(&self.project_path) {
            return;
        }
        if let Some(id) = self.tabs.manager.find_by_path(path) {
            self.switch_tab(id, ctx);
            return;
        }
        if !path.exists() {
            self.status = format!("Arquivo não encontrado: {}", path.display());
            return;
        }
        self.new_tab();
        let opened_tab = self.tabs.active;
        if !self.load_project_file(path, Some(path.to_path_buf()), ctx) {
            // Nothing was opened: do not leave an empty tab behind
            if let Some(id) = opened_tab {
                self.close_tab_now(id, ctx);
            }
        }
    }

    /// Closing a tab: a dirty project first asks what to do with its changes.
    pub(super) fn request_close_tab(&mut self, id: ProjectId, ctx: &egui::Context) {
        let dirty = if Some(id) == self.tabs.active {
            self.is_dirty()
        } else {
            self.tabs.manager.get(id).is_some_and(Project::is_dirty)
        };
        if dirty {
            self.tabs.pending_close = Some(id);
        } else {
            self.close_tab_now(id, ctx);
        }
    }

    /// Closes the tab without asking. The last tab is replaced by an empty project.
    pub(super) fn close_tab_now(&mut self, id: ProjectId, ctx: &egui::Context) {
        self.tabs.pending_close = None;
        let current = self.ensure_tab();
        if id == current {
            self.stop();
            self.discard_own_recovery();
            // The live project is dropped; the manager picks the neighbour tab
            self.tabs.manager.close_project(id);
            self.tabs.extras.remove(&id);
            match self.tabs.manager.active_project_id() {
                Some(next) => {
                    if !self.restore_tab(next) {
                        self.reload_media(ctx);
                    }
                }
                None => {
                    self.tabs.active = None;
                    self.ensure_tab();
                    self.project = ProjectInfo::create_empty();
                    self.timeline = Timeline::new();
                    self.history = History::new();
                    self.project_path = None;
                    self.saved_revision = 0;
                    self.forced_dirty = false;
                    self.media.clear();
                    self.video_info.clear();
                    self.selection.clear();
                    self.selected_track = None;
                    self.time_selection = None;
                    self.playhead = 0.0;
                    self.reset_view();
                }
            }
        } else {
            self.tabs.manager.close_project(id);
            self.tabs.extras.remove(&id);
            self.tabs.bump();
        }
    }

    /// The path of the project an event plays, when it is a sub-project.
    pub(super) fn subproject_path_of(
        &self,
        track_id: TrackId,
        event_id: EventId,
    ) -> Option<PathBuf> {
        let event = self
            .timeline
            .track(track_id)?
            .events
            .iter()
            .find(|e| e.id == event_id)?;
        let media = self
            .project
            .media_pool
            .iter()
            .find(|m| Some(m.id) == event.media_id && m.subproject)?;
        Some(PathBuf::from(&media.path))
    }

    /// Ctrl+Shift+N: the selected events become a project of their own and are replaced, in this
    /// project, by one event that plays it. Double-click that event to open the sub-project.
    pub(super) fn create_subproject_from_selection(&mut self) {
        let events = self.selection.refs();
        if events.is_empty() {
            self.status = "Selecione os eventos que formarão o sub-projeto".into();
            return;
        }
        self.ensure_tab();
        let dir = match self.project_path.as_deref().and_then(Path::parent) {
            Some(parent) => parent.join("subprojetos"),
            None => match neoveg_common::paths::app_data_dir() {
                Some(dir) => dir.join("subprojetos"),
                None => {
                    self.status = "Não há pasta para guardar o sub-projeto".into();
                    return;
                }
            },
        };
        let dest = dir.join(format!("Subprojeto_{}.neoveg", autosave::unix_now()));
        let media_id = self
            .project
            .media_pool
            .iter()
            .map(|m| m.id)
            .max()
            .unwrap_or(0)
            + 1;
        let created = self.tabs.manager.create_subproject_from_events(
            &self.timeline,
            &self.project,
            self.project_path.as_deref(),
            &events,
            &dest,
            media_id,
        );
        match created {
            Ok(mut made) => {
                assign_event_ids(&mut self.timeline, &mut made.command);
                self.project.media_pool.push(made.media);
                if self.history.execute(&mut self.timeline, made.command) {
                    self.selection.clear();
                    self.status = format!(
                        "Sub-projeto criado ({:.1} s): clique duas vezes no evento para abrir em uma aba",
                        made.duration
                    );
                }
                self.tabs.bump();
            }
            Err(e) => {
                tracing::warn!("[WARN] cannot create the sub-project: {e}");
                self.status = format!("Não foi possível criar o sub-projeto: {e}");
            }
        }
    }

    // ---- playback of nested projects -------------------------------------------------------------

    /// The active project with its sub-projects expanded (`None` when it has none). Cached until the
    /// project, a tab or a saved file changes.
    pub(super) fn flattened(&self) -> Option<Arc<Flattened>> {
        if !self.project.media_pool.iter().any(|m| m.subproject) {
            return None;
        }
        let revision = self.history.revision();
        let epoch = self.tabs.epoch;
        if let Some(cache) = &*self.tabs.flat.borrow() {
            if cache.revision == revision && cache.epoch == epoch {
                return cache.flat.clone();
            }
        }
        let manager = &self.tabs.manager;
        let mut loader = |path: &Path| -> Option<(ProjectInfo, Timeline)> {
            // A sub-project open in another tab is played as it is being edited
            if let Some(open) = manager.find_by_path(path).and_then(|id| manager.get(id)) {
                return Some((open.info.clone(), open.timeline.clone()));
            }
            project_file::load(path).ok()
        };
        let flat = flatten_subprojects(&self.timeline, &self.project.media_pool, &mut loader)
            .map(Arc::new);
        if let Some(flat) = &flat {
            self.request_nested_media(flat);
        }
        *self.tabs.flat.borrow_mut() = Some(FlatCache {
            revision,
            epoch,
            flat: flat.clone(),
        });
        flat
    }

    /// Starts decoding the media a sub-project brings (once per file); the sound and picture appear
    /// as soon as they are ready.
    fn request_nested_media(&self, flat: &Flattened) {
        let Some(ctx) = &self.tabs.ctx else {
            return;
        };
        for media in &flat.extra_media {
            if self.media.contains_key(&media.id)
                || self.video_info.contains_key(&media.id)
                || !self.tabs.nested_requested.borrow_mut().insert(media.id)
            {
                continue;
            }
            if Path::new(&media.path).exists() {
                self.spawn_media(PathBuf::from(&media.path), Some(media.id), ctx);
            }
        }
    }
}

// ---- drawing ---------------------------------------------------------------------------------------

/// The strip of project tabs under the menu.
pub(super) fn project_tab_bar(ui: &mut egui::Ui, state: &mut AppState) {
    state.ensure_tab();
    let ctx = ui.ctx().clone();
    let ids = state.tab_ids();
    let active = state.tabs.active;
    let mut switch = None;
    let mut close = None;
    let mut add = false;
    ui.horizontal(|ui| {
        for id in ids {
            let selected = Some(id) == active;
            let title = state.tab_title(id);
            if ui.selectable_label(selected, title).clicked() {
                switch = Some(id);
            }
            if ui
                .small_button("×")
                .on_hover_text("Fechar este projeto")
                .clicked()
            {
                close = Some(id);
            }
            ui.separator();
        }
        if ui
            .small_button("+")
            .on_hover_text("Novo projeto em uma nova aba (Ctrl+N)")
            .clicked()
        {
            add = true;
        }
    });
    if let Some(id) = switch {
        state.switch_tab(id, &ctx);
    }
    if let Some(id) = close {
        state.request_close_tab(id, &ctx);
    }
    if add {
        state.new_tab();
    }
}

/// The question asked when a tab with unsaved changes is closed.
pub(super) fn close_tab_window(ctx: &egui::Context, state: &mut AppState) {
    let Some(id) = state.tabs.pending_close else {
        return;
    };
    let title = state.tab_title(id);
    let (mut save, mut discard, mut cancel) = (false, false, false);
    egui::Window::new("Fechar projeto")
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(format!("\"{title}\" tem alterações não salvas."));
            ui.horizontal(|ui| {
                save = ui.button("Salvar e fechar").clicked();
                discard = ui.button("Descartar").clicked();
                cancel = ui.button("Cancelar").clicked();
            });
        });
    if cancel {
        state.tabs.pending_close = None;
    } else if discard {
        state.close_tab_now(id, ctx);
    } else if save {
        state.switch_tab(id, ctx);
        state.save_project(false);
        if !state.is_dirty() {
            state.close_tab_now(id, ctx);
        } else {
            state.tabs.pending_close = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> AppState {
        AppState::new()
    }

    fn add_event(s: &mut AppState, start: f64, duration: f64) -> (TrackId, EventId) {
        let track = s.timeline.new_track(TrackKind::Audio);
        let track_id = track.id;
        s.timeline.tracks.push(track);
        let event = TrackEvent {
            id: s.timeline.alloc_id(),
            name: "e".into(),
            media_id: Some(1),
            start,
            duration,
            ..TrackEvent::default()
        };
        let id = event.id;
        s.timeline.track_mut(track_id).unwrap().events.push(event);
        (track_id, id)
    }

    #[test]
    fn new_tabs_stash_and_restore_each_project() {
        let ctx = egui::Context::default();
        let mut s = state();
        s.project.name = "Primeiro".into();
        add_event(&mut s, 0.0, 2.0);
        let first = s.ensure_tab();
        s.new_tab();
        let second = s.tabs.active.unwrap();
        assert_ne!(first, second);
        assert!(s.timeline.tracks.is_empty(), "the new tab starts empty");
        assert_eq!(s.tab_ids(), vec![first, second]);
        s.project.name = "Segundo".into();
        add_event(&mut s, 5.0, 1.0);
        add_event(&mut s, 6.0, 1.0);
        s.switch_tab(first, &ctx);
        assert_eq!(s.project.name, "Primeiro");
        assert_eq!(s.timeline.tracks.len(), 1);
        s.switch_tab(second, &ctx);
        assert_eq!(s.project.name, "Segundo");
        assert_eq!(s.timeline.tracks.len(), 2);
        // Switching to the active tab or to an unknown one changes nothing
        s.switch_tab(second, &ctx);
        s.switch_tab(999, &ctx);
        assert_eq!(s.timeline.tracks.len(), 2);
    }

    #[test]
    fn closing_tabs_activates_the_neighbour_and_the_last_one_becomes_empty() {
        let ctx = egui::Context::default();
        let mut s = state();
        s.project.name = "A".into();
        let a = s.ensure_tab();
        s.new_tab();
        s.project.name = "B".into();
        let b = s.tabs.active.unwrap();
        s.close_tab_now(b, &ctx);
        assert_eq!(s.tabs.active, Some(a));
        assert_eq!(s.project.name, "A");
        assert_eq!(s.tab_ids(), vec![a]);
        s.close_tab_now(a, &ctx);
        assert_eq!(s.tab_ids().len(), 1, "never zero tabs");
        assert_eq!(s.project.name, "Untitled");
    }

    #[test]
    fn a_dirty_tab_asks_before_closing_and_a_clean_one_does_not() {
        let ctx = egui::Context::default();
        let mut s = state();
        let a = s.ensure_tab();
        s.new_tab();
        let b = s.tabs.active.unwrap();
        s.request_close_tab(b, &ctx);
        assert!(s.tabs.pending_close.is_none());
        assert_eq!(s.tab_ids(), vec![a]);
        s.new_tab();
        let c = s.tabs.active.unwrap();
        add_event(&mut s, 0.0, 1.0);
        s.add_track(TrackKind::Audio);
        assert!(s.is_dirty());
        s.request_close_tab(c, &ctx);
        assert_eq!(s.tabs.pending_close, Some(c));
        assert_eq!(s.tab_ids().len(), 2, "still open until the user decides");
        // The title of a dirty tab shows the star, also when it is not the active one
        s.tabs.pending_close = None;
        s.switch_tab(a, &ctx);
        assert!(s.tab_title(c).ends_with('*'));
    }

    #[test]
    fn a_selection_becomes_a_subproject_event_that_opens_in_a_tab() {
        let ctx = egui::Context::default();
        let dir = std::env::temp_dir().join(format!("neoveg_tabs_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = state();
        s.project.media_pool.push(MediaRef {
            id: 1,
            path: "/m/a.wav".into(),
            duration_seconds: 30.0,
            subproject: false,
        });
        let (t1, e1) = add_event(&mut s, 2.0, 3.0);
        let (t2, e2) = add_event(&mut s, 4.0, 2.0);
        s.project_path = Some(dir.join("pai.neoveg"));
        s.selection.select_only((t1, e1));
        s.selection.add_all([(t2, e2)]);
        s.create_subproject_from_selection();
        // The selection is one event now, playing a sub-project of 4 s
        let events: Vec<&TrackEvent> = s.timeline.tracks.iter().flat_map(|t| &t.events).collect();
        assert_eq!(events.len(), 1);
        let sub = events[0];
        assert!((sub.duration - 4.0).abs() < 1e-9 && (sub.start - 2.0).abs() < 1e-9);
        let (host, id) = (
            s.timeline
                .tracks
                .iter()
                .find(|t| !t.events.is_empty())
                .unwrap()
                .id,
            sub.id,
        );
        let path = s.subproject_path_of(host, id).expect("a sub-project event");
        assert!(path.exists());
        assert!(path.starts_with(&dir));
        assert!(s.project.media_pool.iter().any(|m| m.subproject));
        // The sub-project is open in a tab already; opening it switches there
        s.open_path_in_tab(&path, &ctx);
        assert_eq!(s.project_path.as_deref(), Some(path.as_path()));
        assert_eq!(s.timeline.tracks.len(), 2);
        // Opening the same file again does not add a tab
        let tabs = s.tab_ids().len();
        s.open_path_in_tab(&path, &ctx);
        assert_eq!(s.tab_ids().len(), tabs);
        // Nothing selected: nothing created
        let mut empty = state();
        empty.create_subproject_from_selection();
        assert!(empty.status.contains("Selecione"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn playback_expands_subprojects_and_caches_the_result() {
        let dir = std::env::temp_dir().join(format!("neoveg_flat_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = state();
        s.project.media_pool.push(MediaRef {
            id: 1,
            path: "/m/a.wav".into(),
            duration_seconds: 30.0,
            subproject: false,
        });
        assert!(
            s.flattened().is_none(),
            "no sub-projects, nothing to expand"
        );
        let (t, e) = add_event(&mut s, 10.0, 4.0);
        s.selection.select_only((t, e));
        s.project_path = Some(dir.join("pai.neoveg"));
        s.create_subproject_from_selection();
        let flat = s.flattened().expect("the sub-project is expanded");
        let events: Vec<&TrackEvent> = flat
            .timeline
            .tracks
            .iter()
            .flat_map(|t| &t.events)
            .collect();
        assert_eq!(events.len(), 1);
        assert!((events[0].start - 10.0).abs() < 1e-9);
        assert_eq!(
            events[0].media_id,
            Some(neoveg_engine::project_manager::nested_media_id("/m/a.wav"))
        );
        // The same answer comes from the cache until something changes
        let again = s.flattened().unwrap();
        assert!(Arc::ptr_eq(&flat, &again));
        s.tabs.bump();
        assert!(!Arc::ptr_eq(&flat, &s.flattened().unwrap()));
        let _ = std::fs::remove_dir_all(dir);
    }
}
