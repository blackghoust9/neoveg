//! Several projects open at once (one tab each) and nested sub-projects.
//!
//! The manager keeps every open project and which one is active. The application edits the active
//! project in its own state and swaps it with the manager's copy when the user changes tab
//! ([`ProjectManager::take`] / [`ProjectManager::put`]). A sub-project is a `.neoveg` file used as
//! one clip of another project (a [`MediaRef`] with `subproject` set); [`validate_no_cycle`] refuses
//! nestings that would loop forever and [`flatten_subprojects`] expands them for playback.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use neoveg_common::project::{MediaRef, ProjectInfo};

use crate::command::{EditCommand, History, TrackChange};
use crate::project_file;
use crate::selection::EventRef;
use crate::timeline::{Timeline, TrackEvent, TrackId, TrackKind};

pub type ProjectId = u64;

/// Deepest nesting accepted (a project inside a project inside a project...).
pub const MAX_NESTING: usize = 8;

/// An open project.
pub struct Project {
    pub id: ProjectId,
    pub info: ProjectInfo,
    pub timeline: Timeline,
    pub history: History,
    /// The `.neoveg` file, `None` for a project never saved.
    pub path: Option<PathBuf>,
    /// `History::revision` at the last save, to know about unsaved changes.
    pub saved_revision: u64,
}

impl Project {
    pub fn is_dirty(&self) -> bool {
        self.history.revision() != self.saved_revision
    }

    /// The name shown on the tab.
    pub fn title(&self) -> String {
        let name = self
            .path
            .as_deref()
            .and_then(Path::file_stem)
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.info.name.clone());
        if self.is_dirty() {
            format!("{name} *")
        } else {
            name
        }
    }
}

/// The sub-project made from a selection.
pub struct SubProjectCreated {
    /// The new project, open in the manager.
    pub id: ProjectId,
    pub path: PathBuf,
    /// Length of the new project, seconds.
    pub duration: f64,
    /// Media pool entry (id chosen by the caller) of the sub-project.
    pub media: MediaRef,
    /// Replaces the selected events of the parent by one sub-project event (one undo step).
    pub command: EditCommand,
}

#[derive(Default)]
pub struct ProjectManager {
    projects: HashMap<ProjectId, Project>,
    /// Tab order.
    order: Vec<ProjectId>,
    active: Option<ProjectId>,
    next_id: ProjectId,
}

/// A path as a comparable key: canonical when the file exists, else as written.
fn key_of(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// `path` as found in the pool of the project saved at `project_file` (relative paths are taken
/// from the project's folder).
fn resolve(project_file: &Path, path: &str) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        project_file
            .parent()
            .map_or_else(|| p.to_path_buf(), |dir| dir.join(p))
    }
}

/// The sub-projects a project file refers to (read from its media pool).
fn dependencies_of(file: &Path) -> Vec<PathBuf> {
    match project_file::load(file) {
        Ok((info, _)) => info
            .media_pool
            .iter()
            .filter(|m| m.subproject)
            .map(|m| resolve(file, &m.path))
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Refuses `child` as a sub-project of `parent` when that would make a project contain itself
/// (directly or through other sub-projects), or nest deeper than [`MAX_NESTING`].
pub fn validate_no_cycle(parent: &Path, child: &Path) -> Result<(), String> {
    let target = key_of(parent);
    let mut visited: HashSet<PathBuf> = HashSet::new();
    let mut stack = vec![(key_of(child), 1usize)];
    while let Some((file, depth)) = stack.pop() {
        if file == target {
            return Err(format!(
                "dependência circular: {} já contém {}",
                child.display(),
                parent.display()
            ));
        }
        if depth > MAX_NESTING {
            return Err(format!(
                "sub-projetos aninhados demais (limite de {MAX_NESTING} níveis)"
            ));
        }
        if !visited.insert(file.clone()) {
            continue;
        }
        for dep in dependencies_of(&file) {
            stack.push((key_of(&dep), depth + 1));
        }
    }
    Ok(())
}

impl ProjectManager {
    pub fn new() -> Self {
        Self::default()
    }

    fn alloc_id(&mut self) -> ProjectId {
        self.next_id += 1;
        self.next_id
    }

    /// Adds an already built project and returns its id. The first project becomes the active one.
    pub fn register(
        &mut self,
        info: ProjectInfo,
        timeline: Timeline,
        path: Option<PathBuf>,
    ) -> ProjectId {
        let id = self.alloc_id();
        let history = History::new();
        let saved_revision = history.revision();
        self.projects.insert(
            id,
            Project {
                id,
                info,
                timeline,
                history,
                path,
                saved_revision,
            },
        );
        self.order.push(id);
        if self.active.is_none() {
            self.active = Some(id);
        }
        id
    }

    /// Opens a `.neoveg` file as a new project (or returns the id of the one already open).
    pub fn open_project(&mut self, path: &Path) -> Result<ProjectId, String> {
        let wanted = key_of(path);
        if let Some(existing) = self
            .projects
            .values()
            .find(|p| p.path.as_deref().map(key_of) == Some(wanted.clone()))
        {
            return Ok(existing.id);
        }
        // A project that reaches itself through its own sub-projects cannot be opened
        for dep in dependencies_of(path) {
            validate_no_cycle(path, &dep)?;
        }
        let (info, timeline) = project_file::load(path)?;
        Ok(self.register(info, timeline, Some(path.to_path_buf())))
    }

    /// Closes a project and returns it. The tab to its left (or right) becomes active.
    pub fn close_project(&mut self, id: ProjectId) -> Option<Project> {
        let removed = self.projects.remove(&id);
        let position = self.order.iter().position(|p| *p == id);
        self.order.retain(|p| *p != id);
        if self.active == Some(id) {
            let next = position
                .and_then(|i| self.order.get(i.saturating_sub(1)).or(self.order.first()))
                .copied();
            self.active = next;
        }
        removed
    }

    /// Makes `id` the active project. `false` when it is not open.
    pub fn switch_active_project(&mut self, id: ProjectId) -> bool {
        let known = self.order.contains(&id);
        if known {
            self.active = Some(id);
        }
        known
    }

    pub fn active_project_id(&self) -> Option<ProjectId> {
        self.active
    }

    /// Ids in tab order.
    pub fn ids(&self) -> &[ProjectId] {
        &self.order
    }

    pub fn get(&self, id: ProjectId) -> Option<&Project> {
        self.projects.get(&id)
    }

    pub fn get_mut(&mut self, id: ProjectId) -> Option<&mut Project> {
        self.projects.get_mut(&id)
    }

    /// The id of the open project saved at `path`.
    pub fn find_by_path(&self, path: &Path) -> Option<ProjectId> {
        let wanted = key_of(path);
        self.order.iter().copied().find(|id| {
            self.projects
                .get(id)
                .is_some_and(|p| p.path.as_deref().map(key_of) == Some(wanted.clone()))
        })
    }

    /// Moves a project out (to edit it) keeping its tab; give it back with [`put`](Self::put).
    pub fn take(&mut self, id: ProjectId) -> Option<Project> {
        self.projects.remove(&id)
    }

    /// Returns a project taken with [`take`](Self::take) to its tab.
    pub fn put(&mut self, project: Project) {
        if !self.order.contains(&project.id) {
            self.order.push(project.id);
        }
        self.projects.insert(project.id, project);
    }

    /// Builds a new project from the selected events of `parent`: copies of the events (and of the
    /// media they use) moved so the earliest one starts at zero, saved to `dest` and opened in the
    /// manager. Returns the replacement of the selection by one sub-project event.
    ///
    /// `media_id` is the id the sub-project gets in the parent's media pool; `parent_path` (when the
    /// parent was saved) is checked against circular dependencies.
    pub fn create_subproject_from_events(
        &mut self,
        parent: &Timeline,
        parent_info: &ProjectInfo,
        parent_path: Option<&Path>,
        events: &[EventRef],
        dest: &Path,
        media_id: u64,
    ) -> Result<SubProjectCreated, String> {
        if let Some(parent_path) = parent_path {
            validate_no_cycle(parent_path, dest)?;
        }
        let mut picked: Vec<(TrackId, TrackKind, TrackEvent)> = Vec::new();
        for track in &parent.tracks {
            for event in &track.events {
                if events.contains(&(track.id, event.id)) {
                    picked.push((track.id, track.kind, event.clone()));
                }
            }
        }
        if picked.is_empty() {
            return Err("selecione pelo menos um evento".into());
        }
        let start = picked
            .iter()
            .map(|(_, _, e)| e.start)
            .fold(f64::INFINITY, f64::min);
        let end = picked.iter().map(|(_, _, e)| e.end()).fold(0.0, f64::max);
        let duration = end - start;

        // The child: one track per source track, events moved to start at zero
        let mut child = Timeline::new();
        let mut child_tracks: Vec<(TrackId, TrackId)> = Vec::new();
        let mut used_media: Vec<u64> = Vec::new();
        for (track_id, kind, event) in &picked {
            let child_track = match child_tracks.iter().find(|(p, _)| p == track_id) {
                Some((_, c)) => *c,
                None => {
                    let t = child.new_track(*kind);
                    let id = t.id;
                    child.tracks.push(t);
                    child_tracks.push((*track_id, id));
                    id
                }
            };
            let mut copy = event.clone();
            copy.id = child.alloc_id();
            copy.start -= start;
            copy.group = None;
            if let Some(m) = copy.media_id {
                if !used_media.contains(&m) {
                    used_media.push(m);
                }
            }
            if let Some(t) = child.track_mut(child_track) {
                t.events.push(copy);
            }
        }
        let mut info = ProjectInfo::create_empty();
        info.name = dest.file_stem().map_or_else(
            || "Sub-projeto".into(),
            |s| s.to_string_lossy().into_owned(),
        );
        info.settings = parent_info.settings.clone();
        info.media_pool = parent_info
            .media_pool
            .iter()
            .filter(|m| used_media.contains(&m.id))
            .cloned()
            .collect();
        if let Some(dir) = dest.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("não foi possível criar {}: {e}", dir.display()))?;
        }
        project_file::save(dest, &info, &child)?;

        // The parent: the selection is replaced by one event of the sub-project, on the first
        // selected video track (else the first selected track)
        let host = picked
            .iter()
            .find(|(_, kind, _)| *kind == TrackKind::Video)
            .or_else(|| picked.first())
            .map(|(t, _, _)| *t)
            .ok_or("seleção vazia")?;
        let mut changes = Vec::new();
        for track in &parent.tracks {
            let touched = track
                .events
                .iter()
                .any(|e| events.contains(&(track.id, e.id)));
            if !touched && track.id != host {
                continue;
            }
            let mut after: Vec<TrackEvent> = track
                .events
                .iter()
                .filter(|e| !events.contains(&(track.id, e.id)))
                .cloned()
                .collect();
            if track.id == host {
                after.push(TrackEvent {
                    // The id is the next free one of the parent; the caller allocates it from the
                    // timeline when the command is built, see `ids_for_command`
                    id: 0,
                    name: info.name.clone(),
                    media_id: Some(media_id),
                    start,
                    duration,
                    ..TrackEvent::default()
                });
                after.sort_by(|a, b| a.start.total_cmp(&b.start));
            }
            changes.push(TrackChange {
                track_id: track.id,
                before: track.events.clone(),
                after,
            });
        }
        let command = EditCommand::ReplaceEvents { changes };
        let id = self.register(info.clone(), child, Some(dest.to_path_buf()));
        let media = MediaRef {
            id: media_id,
            path: dest.display().to_string(),
            duration_seconds: duration,
            subproject: true,
        };
        Ok(SubProjectCreated {
            id,
            path: dest.to_path_buf(),
            duration,
            media,
            command,
        })
    }
}

/// Gives the new sub-project event of `command` a real id from `timeline` (the engine builds it
/// with id `0` because only the timeline owns the id counter).
pub fn assign_event_ids(timeline: &mut Timeline, command: &mut EditCommand) {
    if let EditCommand::ReplaceEvents { changes } = command {
        for change in changes {
            for event in &mut change.after {
                if event.id == 0 {
                    event.id = timeline.alloc_id();
                }
            }
        }
    }
}

// ---- playback: nested projects expanded --------------------------------------------------------------

/// A timeline with its sub-projects expanded, and the media they bring.
pub struct Flattened {
    pub timeline: Timeline,
    /// Media of the sub-projects, with ids that cannot collide with the parent's.
    pub extra_media: Vec<MediaRef>,
}

/// Stable id for a media file of a sub-project: the same file always gets the same id, whatever the
/// project that uses it, and it never collides with the small ids of a project's own pool.
pub fn nested_media_id(path: &str) -> u64 {
    // FNV-1a
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in path.bytes() {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash | (1 << 63)
}

/// Expands every event that plays a sub-project: the events of the child that fall in the window
/// the event shows are copied onto new tracks of the result at the right time, and the event itself
/// is dropped. `load` reads a sub-project file (`None` when it is missing or broken, and that event
/// is then left out). Returns `None` when `timeline` has no sub-project events (nothing to expand).
///
/// The child's track effects, fader and pan are not carried over; its clip fades and trims are.
pub fn flatten_subprojects(
    timeline: &Timeline,
    pool: &[MediaRef],
    load: &mut dyn FnMut(&Path) -> Option<(ProjectInfo, Timeline)>,
) -> Option<Flattened> {
    flatten_level(timeline, pool, load, 0)
}

fn flatten_level(
    timeline: &Timeline,
    pool: &[MediaRef],
    load: &mut dyn FnMut(&Path) -> Option<(ProjectInfo, Timeline)>,
    depth: usize,
) -> Option<Flattened> {
    let is_sub = |media: Option<u64>| {
        media.is_some_and(|id| pool.iter().any(|m| m.id == id && m.subproject))
    };
    if !timeline
        .tracks
        .iter()
        .flat_map(|t| &t.events)
        .any(|e| is_sub(e.media_id))
    {
        return None;
    }
    let mut out = timeline.clone();
    let mut extra: Vec<MediaRef> = Vec::new();
    for track in &mut out.tracks {
        track.events.retain(|e| !is_sub(e.media_id));
    }
    if depth >= MAX_NESTING {
        return Some(Flattened {
            timeline: out,
            extra_media: extra,
        });
    }
    for track in &timeline.tracks {
        if track.muted {
            continue;
        }
        for event in track.events.iter().filter(|e| is_sub(e.media_id)) {
            let Some(media) = pool.iter().find(|m| Some(m.id) == event.media_id) else {
                continue;
            };
            let Some((child_info, child_timeline)) = load(Path::new(&media.path)) else {
                continue;
            };
            // Expand the child's own sub-projects first
            let (child_timeline, mut child_pool) =
                match flatten_level(&child_timeline, &child_info.media_pool, load, depth + 1) {
                    Some(f) => {
                        let mut pool = child_info.media_pool.clone();
                        pool.extend(f.extra_media);
                        (f.timeline, pool)
                    }
                    None => (child_timeline, child_info.media_pool.clone()),
                };
            let window_start = event.trim_in;
            let window_end = event.trim_in + event.duration;
            for child_track in &child_timeline.tracks {
                if child_track.muted {
                    continue;
                }
                let mut events: Vec<TrackEvent> = Vec::new();
                for ce in &child_track.events {
                    let (s, e) = (ce.start.max(window_start), ce.end().min(window_end));
                    if e - s <= 1e-6 {
                        continue;
                    }
                    let mut copy = ce.clone();
                    // The part of the clip inside the window, moved to the parent's time
                    let cut_front = s - ce.start;
                    copy.trim_in += cut_front * ce.playback_rate;
                    if let Some(env) = ce
                        .velocity
                        .as_ref()
                        .filter(|v| crate::velocity::is_active(Some(v)))
                    {
                        let rate = crate::edit::clamp_playback_rate(ce.playback_rate);
                        let (trim_in, shifted) =
                            crate::velocity::shift_start(env, ce.trim_in, rate, cut_front);
                        copy.trim_in = trim_in;
                        copy.velocity = Some(shifted);
                    }
                    copy.start = event.start + (s - window_start);
                    copy.duration = e - s;
                    copy.fade_in = if cut_front > 0.0 { 0.0 } else { copy.fade_in };
                    copy.fade_out = if e < ce.end() { 0.0 } else { copy.fade_out };
                    copy.group = None;
                    if let Some(mid) = copy.media_id {
                        let source = child_pool.iter_mut().find(|m| m.id == mid);
                        if let Some(source) = source {
                            let id = nested_media_id(&source.path);
                            if !extra.iter().any(|m| m.id == id) {
                                extra.push(MediaRef {
                                    id,
                                    path: source.path.clone(),
                                    duration_seconds: source.duration_seconds,
                                    subproject: false,
                                });
                            }
                            copy.media_id = Some(id);
                        }
                    }
                    copy.id = out.alloc_id();
                    events.push(copy);
                }
                if events.is_empty() {
                    continue;
                }
                let mut new_track = out.new_track(child_track.kind);
                new_track.name = format!("{} › {}", event.name, child_track.name);
                new_track.events = events;
                out.tracks.push(new_track);
            }
        }
    }
    Some(Flattened {
        timeline: out,
        extra_media: extra,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selection::EventRef;

    fn event(tl: &mut Timeline, start: f64, duration: f64, media: u64, name: &str) -> TrackEvent {
        TrackEvent {
            id: tl.alloc_id(),
            name: name.into(),
            media_id: Some(media),
            start,
            duration,
            ..TrackEvent::default()
        }
    }

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("neoveg_pm_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn parent_project() -> (Timeline, ProjectInfo, Vec<EventRef>) {
        let mut tl = Timeline::new();
        let mut video = tl.new_track(TrackKind::Video);
        let mut audio = tl.new_track(TrackKind::Audio);
        let a = event(&mut tl, 2.0, 4.0, 1, "A");
        let b = event(&mut tl, 8.0, 2.0, 2, "B");
        let c = event(&mut tl, 3.0, 5.0, 1, "voz");
        let keep = event(&mut tl, 20.0, 1.0, 2, "fica");
        let refs = vec![(video.id, a.id), (audio.id, c.id), (video.id, b.id)];
        video.events = vec![a, b, keep];
        audio.events = vec![c];
        tl.tracks.push(video);
        tl.tracks.push(audio);
        let mut info = ProjectInfo::create_empty();
        info.media_pool = vec![
            MediaRef {
                id: 1,
                path: "/m/a.mp4".into(),
                duration_seconds: 30.0,
                subproject: false,
            },
            MediaRef {
                id: 2,
                path: "/m/b.mp4".into(),
                duration_seconds: 30.0,
                subproject: false,
            },
        ];
        (tl, info, refs)
    }

    #[test]
    fn a_selection_becomes_a_subproject_and_one_event_in_the_parent() {
        let d = dir("create");
        let (mut tl, info, refs) = parent_project();
        let mut manager = ProjectManager::new();
        let dest = d.join("sub.neoveg");
        let mut made = manager
            .create_subproject_from_events(&tl, &info, None, &refs, &dest, 77)
            .unwrap();
        assert!((made.duration - 8.0).abs() < 1e-9, "from 2 s to 10 s");
        assert!(made.media.subproject && made.media.id == 77);
        // Saved to disk and open in the manager
        let (saved_info, saved_tl) = project_file::load(&dest).unwrap();
        assert_eq!(saved_info.media_pool.len(), 2);
        assert_eq!(saved_tl.tracks.len(), 2);
        let starts: Vec<f64> = saved_tl
            .tracks
            .iter()
            .flat_map(|t| t.events.iter().map(|e| e.start))
            .collect();
        assert!(starts.contains(&0.0) && starts.contains(&6.0), "{starts:?}");
        assert!(manager.get(made.id).is_some());
        // The parent: three events replaced by one, the untouched one stays; one undo step
        assign_event_ids(&mut tl, &mut made.command);
        let mut history = History::new();
        assert!(history.execute(&mut tl, made.command));
        let video = tl
            .tracks
            .iter()
            .find(|t| t.kind == TrackKind::Video)
            .unwrap();
        assert_eq!(video.events.len(), 2);
        let sub = video
            .events
            .iter()
            .find(|e| e.media_id == Some(77))
            .unwrap();
        assert!((sub.start - 2.0).abs() < 1e-9 && (sub.duration - 8.0).abs() < 1e-9);
        assert_ne!(sub.id, 0);
        assert!(tl
            .tracks
            .iter()
            .find(|t| t.kind == TrackKind::Audio)
            .unwrap()
            .events
            .is_empty());
        assert!(history.undo(&mut tl));
        assert_eq!(tl.tracks[0].events.len(), 3);
        assert!(manager
            .create_subproject_from_events(&tl, &info, None, &[], &dest, 1)
            .is_err());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn tabs_open_switch_and_close_in_order() {
        let d = dir("tabs");
        let (tl, info, _) = parent_project();
        let a = d.join("a.neoveg");
        let b = d.join("b.neoveg");
        project_file::save(&a, &info, &tl).unwrap();
        project_file::save(&b, &info, &Timeline::new()).unwrap();
        let mut manager = ProjectManager::new();
        let ida = manager.open_project(&a).unwrap();
        let idb = manager.open_project(&b).unwrap();
        assert_eq!(manager.ids(), &[ida, idb]);
        assert_eq!(manager.active_project_id(), Some(ida));
        // Opening the same file again finds the tab that exists
        assert_eq!(manager.open_project(&a).unwrap(), ida);
        assert_eq!(manager.ids().len(), 2);
        assert!(manager.switch_active_project(idb));
        assert_eq!(manager.active_project_id(), Some(idb));
        assert!(!manager.switch_active_project(999));
        assert_eq!(manager.find_by_path(&a), Some(ida));
        // Closing the active tab activates its neighbour
        assert!(manager.close_project(idb).is_some());
        assert_eq!(manager.active_project_id(), Some(ida));
        assert!(manager.close_project(idb).is_none());
        assert!(manager.close_project(ida).is_some());
        assert_eq!(manager.active_project_id(), None);
        assert!(manager.open_project(&d.join("missing.neoveg")).is_err());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn take_and_put_keep_the_tab_in_place() {
        let mut manager = ProjectManager::new();
        let id = manager.register(ProjectInfo::create_empty(), Timeline::new(), None);
        let other = manager.register(ProjectInfo::create_empty(), Timeline::new(), None);
        let project = manager.take(id).unwrap();
        assert!(
            manager.find_by_path(Path::new("nada.neoveg")).is_none(),
            "a taken project does not break lookups"
        );
        assert!(manager.get(id).is_none());
        assert_eq!(manager.ids(), &[id, other], "the tab stays");
        manager.put(project);
        assert!(manager.get(id).is_some());
        assert_eq!(manager.get(id).unwrap().title(), "Untitled");
    }

    /// Saves a project whose pool refers to `subs` as sub-projects.
    fn project_with_subs(path: &Path, subs: &[&Path]) {
        let mut info = ProjectInfo::create_empty();
        for (i, s) in subs.iter().enumerate() {
            info.media_pool.push(MediaRef {
                id: i as u64 + 1,
                path: s.display().to_string(),
                duration_seconds: 5.0,
                subproject: true,
            });
        }
        project_file::save(path, &info, &Timeline::new()).unwrap();
    }

    #[test]
    fn circular_dependencies_are_refused() {
        let d = dir("cycle");
        let (a, b, c) = (d.join("a.neoveg"), d.join("b.neoveg"), d.join("c.neoveg"));
        // a contains b, b contains c: c cannot contain a, a cannot contain itself
        project_with_subs(&a, &[&b]);
        project_with_subs(&b, &[&c]);
        project_with_subs(&c, &[]);
        assert!(validate_no_cycle(&a, &b).is_ok());
        assert!(validate_no_cycle(&c, &a).unwrap_err().contains("circular"));
        assert!(validate_no_cycle(&a, &a).is_err());
        assert!(validate_no_cycle(&b, &a).is_err());
        assert!(validate_no_cycle(&a, &c).is_ok());
        // A project that already contains itself cannot be opened
        let loop_a = d.join("loop_a.neoveg");
        let loop_b = d.join("loop_b.neoveg");
        project_with_subs(&loop_a, &[&loop_b]);
        project_with_subs(&loop_b, &[&loop_a]);
        assert!(ProjectManager::new().open_project(&loop_a).is_err());
        assert!(ProjectManager::new().open_project(&c).is_ok());
        // Creating a sub-project at the parent's own path is refused too
        let (tl, info, refs) = parent_project();
        let err = ProjectManager::new()
            .create_subproject_from_events(&tl, &info, Some(&a), &refs, &a, 9)
            .err()
            .unwrap();
        assert!(err.contains("circular"), "{err}");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn nesting_deeper_than_the_limit_is_refused() {
        let d = dir("deep");
        let files: Vec<PathBuf> = (0..=MAX_NESTING + 1)
            .map(|i| d.join(format!("p{i}.neoveg")))
            .collect();
        for i in 0..files.len() {
            let next: Vec<&Path> = files.get(i + 1).map(|p| p.as_path()).into_iter().collect();
            project_with_subs(&files[i], &next);
        }
        let outer = d.join("outer.neoveg");
        project_with_subs(&outer, &[]);
        assert!(validate_no_cycle(&outer, &files[0])
            .unwrap_err()
            .contains("aninhados"));
        assert!(validate_no_cycle(&outer, &files[MAX_NESTING + 1]).is_ok());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn flattening_puts_the_childs_clips_at_the_parents_time() {
        // Child: a clip of media 5 from 0 to 10 s, another of media 6 from 12 to 14 s
        let mut child = Timeline::new();
        let mut ct = child.new_track(TrackKind::Video);
        let c1 = event(&mut child, 0.0, 10.0, 5, "c1");
        let c2 = event(&mut child, 12.0, 2.0, 6, "c2");
        ct.events = vec![c1, c2];
        child.tracks.push(ct);
        let mut child_info = ProjectInfo::create_empty();
        child_info.media_pool = vec![
            MediaRef {
                id: 5,
                path: "/m/five.mp4".into(),
                duration_seconds: 10.0,
                subproject: false,
            },
            MediaRef {
                id: 6,
                path: "/m/six.mp4".into(),
                duration_seconds: 5.0,
                subproject: false,
            },
        ];
        // Parent: shows the child from 4 s to 13 s at 20 s on its timeline
        let mut parent = Timeline::new();
        let mut pt = parent.new_track(TrackKind::Video);
        let mut ev = event(&mut parent, 20.0, 9.0, 1, "Sub");
        ev.trim_in = 4.0;
        pt.events = vec![ev];
        parent.tracks.push(pt);
        let pool = vec![MediaRef {
            id: 1,
            path: "child.neoveg".into(),
            duration_seconds: 14.0,
            subproject: true,
        }];
        let mut loads = 0;
        let flat = flatten_subprojects(&parent, &pool, &mut |p| {
            loads += 1;
            assert_eq!(p, Path::new("child.neoveg"));
            Some((child_info.clone(), child.clone()))
        })
        .unwrap();
        assert_eq!(loads, 1);
        // The sub-project event is gone; the child's track is there
        let events: Vec<&TrackEvent> = flat
            .timeline
            .tracks
            .iter()
            .flat_map(|t| &t.events)
            .collect();
        assert_eq!(events.len(), 2);
        let first = events.iter().find(|e| e.name == "c1").unwrap();
        // c1 is cut at the window start (4 s of the child) and keeps 6 s
        assert!((first.start - 20.0).abs() < 1e-9);
        assert!((first.duration - 6.0).abs() < 1e-9);
        assert!((first.trim_in - 4.0).abs() < 1e-9);
        let second = events.iter().find(|e| e.name == "c2").unwrap();
        // c2 starts 8 s into the window and is cut at 13 s: 1 s long
        assert!((second.start - 28.0).abs() < 1e-9);
        assert!((second.duration - 1.0).abs() < 1e-9);
        // Media ids are stable and cannot collide with a project's own
        assert_eq!(first.media_id, Some(nested_media_id("/m/five.mp4")));
        assert!(first.media_id.unwrap() > u64::MAX / 2);
        assert_eq!(flat.extra_media.len(), 2);
        assert!(flat.extra_media.iter().any(|m| m.path == "/m/six.mp4"));
        // Nothing to expand: no sub-project events
        let plain = Timeline::new();
        assert!(flatten_subprojects(&plain, &[], &mut |_| None).is_none());
        // A missing child leaves the event out and does not fail
        let flat = flatten_subprojects(&parent, &pool, &mut |_| None).unwrap();
        assert!(flat.timeline.tracks.iter().all(|t| t.events.is_empty()));
    }

    #[test]
    fn nested_levels_are_expanded_and_runaway_nesting_stops() {
        // grandchild: media 9; child: a sub-project event of the grandchild; parent: the child
        let mut grand = Timeline::new();
        let mut gt = grand.new_track(TrackKind::Audio);
        let g1 = event(&mut grand, 0.0, 5.0, 9, "g");
        gt.events = vec![g1];
        grand.tracks.push(gt);
        let mut grand_info = ProjectInfo::create_empty();
        grand_info.media_pool = vec![MediaRef {
            id: 9,
            path: "/m/nine.wav".into(),
            duration_seconds: 5.0,
            subproject: false,
        }];

        let mut child = Timeline::new();
        let mut ct = child.new_track(TrackKind::Audio);
        let sub = event(&mut child, 10.0, 5.0, 1, "grand");
        ct.events = vec![sub];
        child.tracks.push(ct);
        let mut child_info = ProjectInfo::create_empty();
        child_info.media_pool = vec![MediaRef {
            id: 1,
            path: "grand.neoveg".into(),
            duration_seconds: 5.0,
            subproject: true,
        }];

        let mut parent = Timeline::new();
        let mut pt = parent.new_track(TrackKind::Audio);
        let top = event(&mut parent, 100.0, 15.0, 1, "child");
        pt.events = vec![top];
        parent.tracks.push(pt);
        let pool = vec![MediaRef {
            id: 1,
            path: "child.neoveg".into(),
            duration_seconds: 15.0,
            subproject: true,
        }];

        let flat = flatten_subprojects(&parent, &pool, &mut |p| {
            if p == Path::new("child.neoveg") {
                Some((child_info.clone(), child.clone()))
            } else {
                Some((grand_info.clone(), grand.clone()))
            }
        })
        .unwrap();
        let ev = flat
            .timeline
            .tracks
            .iter()
            .flat_map(|t| &t.events)
            .next()
            .unwrap();
        assert_eq!(ev.media_id, Some(nested_media_id("/m/nine.wav")));
        assert!((ev.start - 110.0).abs() < 1e-9, "100 + 10: {}", ev.start);

        // A project that contains itself (should have been refused) does not loop forever
        let mut selfish = Timeline::new();
        let mut st = selfish.new_track(TrackKind::Video);
        let me = event(&mut selfish, 0.0, 5.0, 1, "me");
        st.events = vec![me];
        selfish.tracks.push(st);
        let mut selfish_info = ProjectInfo::create_empty();
        selfish_info.media_pool = vec![MediaRef {
            id: 1,
            path: "me.neoveg".into(),
            duration_seconds: 5.0,
            subproject: true,
        }];
        let flat = flatten_subprojects(&selfish, &selfish_info.media_pool, &mut |_| {
            Some((selfish_info.clone(), selfish.clone()))
        });
        assert!(flat.is_some());
    }
}
