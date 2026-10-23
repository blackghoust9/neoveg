//! Main application: dockable workspace (Preview, Timeline, Project Media, Trimmer, Master Bus).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, FontId, Key, Modifiers, Pos2, Rect, Rounding, Sense, Shape,
    Stroke, TextureHandle,
};
use egui_dock::{DockArea, DockState, NodeIndex, Style, TabViewer};
use neoveg_audio::decode::decode_file;
use neoveg_audio::meter::{linear_to_dbfs, PeakMeter};
use neoveg_audio::mixer::MixClip;
use neoveg_audio::playback::AudioPlayer;
use neoveg_audio::waveform::WaveformSource;
use neoveg_common::fade::FadeCurve;
use neoveg_common::paths::autosave_dir;
use neoveg_common::preferences::Preferences;
use neoveg_common::project::{MediaRef, ProjectInfo};
use neoveg_common::timecode::{is_drop_frame_rate, seconds_to_timecode};
use neoveg_engine::autosave::{self, RecoveryEntry};
use neoveg_engine::crossfade::{find_crossfades, Crossfade};
use neoveg_engine::edit;
use neoveg_engine::project_file;
use neoveg_engine::relink::relink_missing;
use neoveg_engine::EventGeometry;
use neoveg_engine::{EditCommand, EventId, History, Timeline, TrackEvent, TrackId, TrackKind};
use neoveg_video::info::{is_video_path, VideoInfo};
use neoveg_video::worker::{probe, FrameRequest, FrameWorker};

use crate::frame_grid::{snap_to_device_pixel, FrameGrid};
use crate::precision_slider::PrecisionSlider;
use crate::waveform_paint::paint_waveform;

const HEADER_WIDTH: f32 = 150.0;
const RULER_HEIGHT: f32 = 24.0;
const TRACK_HEIGHT: f32 = 64.0;
const EVENT_HEADER_HEIGHT: f32 = 14.0;
const MIN_PIXELS_PER_SECOND: f64 = 15.0;
const MAX_PIXELS_PER_SECOND: f64 = 480_000.0;
const OFFSCREEN_CLAMP_PX: f32 = 10_000.0;
const BLINK_SECONDS: f64 = 0.5;
const FADE_CURVE_STEPS: u32 = 20;
const SNAP_THRESHOLD_PX: f32 = 8.0;
const AUTOSCROLL_EDGE_PX: f32 = 28.0;
const AUTOSCROLL_PX_PER_SECOND: f64 = 700.0;

const LANGUAGES: [&str; 8] = [
    "pt-BR", "en-US", "de-DE", "es-ES", "fr-FR", "zh-CN", "ja-JP", "ru-RU",
];
const QUALITIES: [(&str, f32); 5] = [
    ("Draft", 0.25),
    ("Preview", 0.5),
    ("Good", 1.0),
    ("Best", 1.0),
    ("Auto", 0.0),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tab {
    Preview,
    Timeline,
    Media,
    MasterBus,
}

impl Tab {
    fn title(self) -> &'static str {
        match self {
            Tab::Preview => "Visualização",
            Tab::Timeline => "Linha do Tempo",
            Tab::Media => "Mídia",
            Tab::MasterBus => "Barramento Master",
        }
    }
}

/// Decoded audio kept in memory for waveform drawing and playback.
struct LoadedMedia {
    path: PathBuf,
    name: String,
    waveform: Arc<WaveformSource>,
    stereo: Arc<Vec<f32>>,
    sample_rate: u32,
    duration: f64,
}

enum LoadMsg {
    Loaded {
        media: LoadedMedia,
        /// Media-pool id when reloading a project's media; `None` for a fresh import.
        existing_id: Option<u64>,
    },
    VideoLoaded {
        path: PathBuf,
        info: VideoInfo,
        /// Audio track of the same file, when it has one.
        audio: Option<LoadedMedia>,
        existing_id: Option<u64>,
    },
    Failed(String),
}

/// What a mouse drag on an event does, decided by where the press landed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DragMode {
    Move,
    TrimLeft,
    TrimRight,
    FadeIn,
    FadeOut,
}

/// A drag in progress. The timeline is only touched (as one undo step) when the mouse is released.
struct EventDrag {
    mode: DragMode,
    track_id: TrackId,
    event_id: EventId,
    target_track: TrackId,
    origin: EventGeometry,
    current: EventGeometry,
    /// Pointer time minus event start at the press (keeps the grabbed point under the cursor).
    grab_offset: f64,
}

struct ActivePlayer {
    player: AudioPlayer,
}

struct AppState {
    project: ProjectInfo,
    timeline: Timeline,
    history: History,
    prefs: Preferences,
    media: HashMap<u64, LoadedMedia>,

    playhead: f64,
    playing: bool,
    blink_origin: f64,
    pixels_per_second: f64,
    scroll_seconds: f64,
    selected_track: Option<TrackId>,
    selected_event: Option<(TrackId, EventId)>,

    meter: Arc<PeakMeter>,
    master_volume: f32,
    active_player: Option<ActivePlayer>,

    show_bars: bool,
    bars_texture: Option<TextureHandle>,
    quality: usize,
    show_preferences: bool,
    show_inspector: bool,
    video_info: HashMap<u64, VideoInfo>,
    frame_worker: Option<FrameWorker>,
    video_texture: Option<TextureHandle>,
    last_frame_key: Option<(u64, i64, u32)>,
    preview_pane_width: f32,
    session_id: String,
    saved_revision: u64,
    forced_dirty: bool,
    last_autosave_time: f64,
    last_autosave_revision: Option<u64>,
    autosave_busy: Arc<AtomicBool>,
    pending_recovery: Vec<RecoveryEntry>,
    confirm_exit: bool,
    allow_close: bool,
    window_title: String,
    drag: Option<EventDrag>,
    context_event: Option<(TrackId, EventId)>,
    snapping: bool,
    media_tab: usize,
    project_path: Option<PathBuf>,
    status: String,

    loader_tx: Sender<LoadMsg>,
    loader_rx: Receiver<LoadMsg>,
}

impl AppState {
    fn new() -> Self {
        let prefs = Preferences::default_path()
            .map(|p| Preferences::load(&p))
            .unwrap_or_default();
        let (loader_tx, loader_rx) = channel();
        Self {
            project: ProjectInfo::create_empty(),
            timeline: Timeline::new(),
            history: History::new(),
            prefs,
            media: HashMap::new(),
            playhead: 0.0,
            playing: false,
            blink_origin: 0.0,
            pixels_per_second: 100.0,
            scroll_seconds: 0.0,
            selected_track: None,
            selected_event: None,
            meter: Arc::new(PeakMeter::new()),
            master_volume: 1.0,
            active_player: None,
            show_bars: false,
            bars_texture: None,
            quality: 4,
            show_preferences: false,
            show_inspector: false,
            video_info: HashMap::new(),
            frame_worker: None,
            video_texture: None,
            last_frame_key: None,
            preview_pane_width: 640.0,
            session_id: autosave::new_session_id(),
            saved_revision: 0,
            forced_dirty: false,
            last_autosave_time: 0.0,
            last_autosave_revision: None,
            autosave_busy: Arc::new(AtomicBool::new(false)),
            pending_recovery: Vec::new(),
            confirm_exit: false,
            allow_close: false,
            window_title: String::new(),
            drag: None,
            context_event: None,
            snapping: true,
            media_tab: 0,
            project_path: None,
            status: "Projeto vazio".into(),
            loader_tx,
            loader_rx,
        }
    }

    fn new_project(&mut self) {
        self.stop();
        let prefs = self.prefs.clone();
        let worker = self.frame_worker.take();
        let recovery = std::mem::take(&mut self.pending_recovery);
        self.discard_own_recovery();
        let (tx, rx) = (
            self.loader_tx.clone(),
            std::mem::replace(&mut self.loader_rx, channel().1),
        );
        *self = Self::new();
        self.prefs = prefs;
        self.frame_worker = worker;
        self.pending_recovery = recovery;
        self.loader_tx = tx;
        self.loader_rx = rx;
    }

    fn drop_frame(&self) -> bool {
        is_drop_frame_rate(self.project.settings.fps)
    }

    // ---- editing -----------------------------------------------------------------------------

    fn add_track(&mut self, kind: TrackKind) {
        let track = self.timeline.new_track(kind);
        let id = track.id;
        if self
            .history
            .execute(&mut self.timeline, EditCommand::AddTrack { track })
        {
            self.selected_track = Some(id);
        }
    }

    fn split_at_playhead(&mut self) {
        let Some(track_id) = self.selected_track else {
            return;
        };
        let Some(event_id) = self
            .timeline
            .event_at(track_id, self.playhead)
            .map(|e| e.id)
        else {
            return;
        };
        if let Some(cmd) = EditCommand::split(&mut self.timeline, track_id, event_id, self.playhead)
        {
            self.history.execute(&mut self.timeline, cmd);
        }
    }

    fn delete_selected_event(&mut self) {
        let Some((track_id, event_id)) = self.selected_event else {
            return;
        };
        let Some(track) = self.timeline.track(track_id) else {
            return;
        };
        let Some(index) = track.events.iter().position(|e| e.id == event_id) else {
            return;
        };
        let event = track.events[index].clone();
        self.history.execute(
            &mut self.timeline,
            EditCommand::RemoveEvent {
                track_id,
                index,
                event,
            },
        );
        self.selected_event = None;
    }

    // ---- media import --------------------------------------------------------------------------

    fn import_media(&mut self, ctx: &egui::Context) {
        let mut extensions = vec!["wav", "mp3", "flac", "ogg", "m4a", "aac"];
        extensions.extend(neoveg_video::info::VIDEO_EXTENSIONS);
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Mídia", &extensions)
            .pick_file()
        else {
            return;
        };
        self.status = format!("Importando {}…", path.display());
        self.spawn_media(path, None, ctx);
    }

    /// Loads audio or video media on a worker thread (never on the UI thread).
    fn spawn_media(&self, path: PathBuf, existing_id: Option<u64>, ctx: &egui::Context) {
        let (tx, ctx) = (self.loader_tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let msg = if is_video_path(&path) {
                match probe(&path) {
                    Ok(info) => LoadMsg::VideoLoaded {
                        // A video without a usable audio track is still a valid video clip
                        audio: load_audio(&path).ok(),
                        path,
                        info,
                        existing_id,
                    },
                    Err(e) => LoadMsg::Failed(e),
                }
            } else {
                match load_audio(&path) {
                    Ok(media) => LoadMsg::Loaded { media, existing_id },
                    Err(e) => LoadMsg::Failed(e),
                }
            };
            let _ = tx.send(msg);
            ctx.request_repaint();
        });
    }

    fn poll_loader(&mut self) {
        while let Ok(msg) = self.loader_rx.try_recv() {
            match msg {
                LoadMsg::Failed(e) => {
                    tracing::error!("[ERROR] import failed: {e}");
                    self.status = format!("Falha ao importar: {e}");
                }
                LoadMsg::Loaded {
                    media,
                    existing_id: Some(id),
                } => {
                    self.media.insert(id, media);
                }
                LoadMsg::Loaded {
                    media,
                    existing_id: None,
                } => self.place_media(media),
                LoadMsg::VideoLoaded {
                    path,
                    info,
                    audio,
                    existing_id: Some(id),
                } => {
                    let _ = path;
                    self.video_info.insert(id, info);
                    if let Some(audio) = audio {
                        self.media.insert(id, audio);
                    }
                    self.last_frame_key = None;
                }
                LoadMsg::VideoLoaded {
                    path,
                    info,
                    audio,
                    existing_id: None,
                } => self.place_video(path, info, audio),
            }
        }
    }

    // ---- project files (.neoveg) ---------------------------------------------------------------

    fn save_project(&mut self, save_as: bool) {
        let path = match (&self.project_path, save_as) {
            (Some(p), false) => p.clone(),
            _ => {
                let Some(p) = rfd::FileDialog::new()
                    .add_filter("Projeto Neoveg", &[project_file::EXTENSION])
                    .set_file_name("projeto.neoveg")
                    .save_file()
                else {
                    return;
                };
                p.with_extension(project_file::EXTENSION)
            }
        };
        if let Some(stem) = path.file_stem() {
            self.project.name = stem.to_string_lossy().into_owned();
        }
        match project_file::save(&path, &self.project, &self.timeline) {
            Ok(()) => {
                tracing::info!("[INFO] project saved: {}", path.display());
                self.status = format!("Projeto salvo: {}", path.display());
                self.project_path = Some(path);
                self.mark_clean();
            }
            Err(e) => {
                tracing::error!("[ERROR] save failed: {e}");
                self.status = format!("Falha ao salvar: {e}");
            }
        }
    }

    fn open_project(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Projeto Neoveg", &[project_file::EXTENSION])
            .pick_file()
        else {
            return;
        };
        self.load_project_file(&path, Some(path.clone()), ctx);
    }

    /// Loads `file` and associates the project with `associate` (the file itself for a normal open,
    /// the original project path when recovering an autosave).
    fn load_project_file(
        &mut self,
        file: &std::path::Path,
        associate: Option<PathBuf>,
        ctx: &egui::Context,
    ) -> bool {
        let (info, timeline) = match project_file::load(file) {
            Ok(loaded) => loaded,
            Err(e) => {
                tracing::error!("[ERROR] open failed: {e}");
                self.status = format!("Falha ao abrir: {e}");
                return false;
            }
        };
        self.stop();
        self.discard_own_recovery();
        self.project = info;
        self.timeline = timeline;
        self.history = History::new();
        self.media.clear();
        self.video_info.clear();
        self.video_texture = None;
        self.last_frame_key = None;
        self.selected_track = None;
        self.selected_event = None;
        self.playhead = 0.0;
        self.scroll_seconds = 0.0;

        // Missing media: relink by exact then fuzzy name in the project folder and the old folders
        let missing = project_file::find_missing(&self.project);
        let mut dirs: Vec<PathBuf> = file.parent().map(PathBuf::from).into_iter().collect();
        if let Some(parent) = associate.as_deref().and_then(std::path::Path::parent) {
            dirs.push(parent.to_path_buf());
        }
        for m in &self.project.media_pool {
            if let Some(parent) = std::path::Path::new(&m.path).parent() {
                if parent.is_dir() {
                    dirs.push(parent.to_path_buf());
                }
            }
        }
        let relinked = relink_missing(&mut self.project.media_pool, &missing, &dirs);
        let still_missing = project_file::find_missing(&self.project);

        let to_load: Vec<(u64, PathBuf)> = self
            .project
            .media_pool
            .iter()
            .filter(|m| !still_missing.contains(&m.id))
            .map(|m| (m.id, PathBuf::from(&m.path)))
            .collect();
        for (id, media_path) in to_load {
            self.spawn_media(media_path, Some(id), ctx);
        }

        self.status = if still_missing.is_empty() {
            format!("Projeto aberto ({relinked} mídia(s) religada(s))")
        } else {
            tracing::warn!("[WARN] {} media file(s) missing", still_missing.len());
            format!(
                "Projeto aberto com {} mídia(s) ausente(s)",
                still_missing.len()
            )
        };
        self.project_path = associate;
        self.mark_clean();
        true
    }

    fn recover_entry(&mut self, entry: &RecoveryEntry, ctx: &egui::Context) {
        let original = entry.meta.original_path.as_ref().map(PathBuf::from);
        if self.load_project_file(&entry.path, original, ctx) {
            autosave::remove(&entry.path);
            // Recovered work only exists in memory until the user saves it
            self.forced_dirty = true;
            self.status = "Projeto recuperado: salve para mantê-lo".into();
        }
        self.pending_recovery.retain(|e| e.path != entry.path);
    }

    // ---- mouse editing ---------------------------------------------------------------------------

    /// Times events can snap to: the origin, the playhead and the edges of every other event.
    fn snap_candidates(&self, exclude: (TrackId, EventId)) -> Vec<f64> {
        let mut times = vec![0.0, self.playhead];
        for track in &self.timeline.tracks {
            for ev in &track.events {
                if (track.id, ev.id) != exclude {
                    times.push(ev.start);
                    times.push(ev.end());
                }
            }
        }
        times
    }

    /// Length of the source media behind an event (`None` for generated media).
    fn source_len(&self, track_id: TrackId, event_id: EventId) -> Option<f64> {
        let track = self.timeline.track(track_id)?;
        let media_id = track.events.iter().find(|e| e.id == event_id)?.media_id?;
        let length = match track.kind {
            TrackKind::Audio => self.media.get(&media_id).map(|m| m.duration),
            TrackKind::Video => self.video_info.get(&media_id).map(|i| i.duration),
        };
        length.or_else(|| {
            self.project
                .media_pool
                .iter()
                .find(|m| m.id == media_id)
                .map(|m| m.duration_seconds)
        })
    }

    fn begin_drag(&mut self, hit: (TrackId, EventId, DragMode, EventGeometry), pointer_time: f64) {
        let (track_id, event_id, mode, origin) = hit;
        self.selected_track = Some(track_id);
        self.selected_event = Some((track_id, event_id));
        self.drag = Some(EventDrag {
            mode,
            track_id,
            event_id,
            target_track: track_id,
            origin,
            current: origin,
            grab_offset: pointer_time - origin.start,
        });
    }

    /// Recomputes the dragged geometry for the pointer at `pointer_time` over `row_track`.
    fn update_drag(
        &mut self,
        pointer_time: f64,
        row_track: Option<TrackId>,
        threshold: f64,
        snap_enabled: bool,
    ) {
        let Some(mut drag) = self.drag.take() else {
            return;
        };
        let fps = self.project.settings.fps;
        let min_len = 1.0 / fps.max(1.0);
        let candidates = if snap_enabled {
            self.snap_candidates((drag.track_id, drag.event_id))
        } else {
            Vec::new()
        };
        let snap = |t: f64| {
            edit::snap_time(t, &candidates, threshold).unwrap_or_else(|| edit::quantize(t, fps))
        };

        match drag.mode {
            DragMode::Move => {
                let raw = pointer_time - drag.grab_offset;
                let start = edit::snap_block(raw, drag.origin.duration, &candidates, threshold)
                    .unwrap_or_else(|| edit::quantize(raw, fps));
                drag.current = edit::move_to(drag.origin, start);
                // Events only move between tracks of their own kind
                if let Some(track) = row_track.and_then(|id| self.timeline.track(id)) {
                    let same_kind = self
                        .timeline
                        .track(drag.track_id)
                        .is_some_and(|t| t.kind == track.kind);
                    if same_kind {
                        drag.target_track = track.id;
                    }
                }
            }
            DragMode::TrimLeft => {
                drag.current = edit::trim_left(drag.origin, snap(pointer_time), min_len);
            }
            DragMode::TrimRight => {
                let len = self.source_len(drag.track_id, drag.event_id);
                drag.current = edit::trim_right(drag.origin, snap(pointer_time), min_len, len);
            }
            DragMode::FadeIn => {
                let length = edit::quantize(pointer_time - drag.origin.start, fps);
                drag.current = edit::set_fade_in(drag.origin, length);
            }
            DragMode::FadeOut => {
                let length = edit::quantize(drag.origin.end() - pointer_time, fps);
                drag.current = edit::set_fade_out(drag.origin, length);
            }
        }
        self.drag = Some(drag);
    }

    fn event_geometry(&self, track_id: TrackId, event_id: EventId) -> Option<EventGeometry> {
        self.timeline
            .track(track_id)?
            .events
            .iter()
            .find(|e| e.id == event_id)
            .map(TrackEvent::geometry)
    }

    /// Replaces an event's geometry (fades, curves...) as one undo step.
    fn apply_geometry(&mut self, track_id: TrackId, event_id: EventId, new: EventGeometry) {
        let Some(old) = self.event_geometry(track_id, event_id) else {
            return;
        };
        if old != new {
            self.history.execute(
                &mut self.timeline,
                EditCommand::EditEvent {
                    from_track: track_id,
                    to_track: track_id,
                    event_id,
                    old,
                    new,
                },
            );
        }
    }

    /// Commits the drag as a single undo step (nothing happens when the event did not change).
    fn finish_drag(&mut self) {
        let Some(d) = self.drag.take() else {
            return;
        };
        if d.current == d.origin && d.target_track == d.track_id {
            return;
        }
        self.history.execute(
            &mut self.timeline,
            EditCommand::EditEvent {
                from_track: d.track_id,
                to_track: d.target_track,
                event_id: d.event_id,
                old: d.origin,
                new: d.current,
            },
        );
        self.selected_track = Some(d.target_track);
        self.selected_event = Some((d.target_track, d.event_id));
    }

    // ---- unsaved changes and autosave -----------------------------------------------------------

    fn is_dirty(&self) -> bool {
        self.forced_dirty || self.history.revision() != self.saved_revision
    }

    /// The project now matches what is on disk: no unsaved changes, no pending recovery file.
    fn mark_clean(&mut self) {
        self.saved_revision = self.history.revision();
        self.forced_dirty = false;
        self.last_autosave_revision = None;
        self.discard_own_recovery();
    }

    /// Removes this session's recovery file (waits briefly for a write in progress).
    fn discard_own_recovery(&self) {
        for _ in 0..200 {
            if !self.autosave_busy.load(Ordering::SeqCst) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        if let Some(dir) = autosave_dir() {
            autosave::remove(&autosave::recovery_path(&dir, &self.session_id));
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let title = format!(
            "{}{} - Neoveg",
            if self.is_dirty() { "*" } else { "" },
            self.project.name
        );
        if title != self.window_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }
    }

    /// Writes a recovery file on a worker thread when there are unsaved changes and the
    /// configured interval has elapsed.
    fn maybe_autosave(&mut self, now: f64) {
        let minutes = self.prefs.autosave_minutes;
        let revision = self.history.revision();
        if minutes == 0
            || !self.is_dirty()
            || self.last_autosave_revision == Some(revision)
            || now - self.last_autosave_time < f64::from(minutes) * 60.0
        {
            return;
        }
        let Some(dir) = autosave_dir() else {
            return;
        };
        if self.autosave_busy.swap(true, Ordering::SeqCst) {
            return;
        }
        self.last_autosave_time = now;
        self.last_autosave_revision = Some(revision);

        let (info, timeline) = (self.project.clone(), self.timeline.clone());
        let (session, original) = (self.session_id.clone(), self.project_path.clone());
        let busy = Arc::clone(&self.autosave_busy);
        std::thread::spawn(move || {
            match autosave::write_recovery(&dir, &session, &info, &timeline, original.as_deref()) {
                Ok(path) => tracing::info!("[INFO] autosave written: {}", path.display()),
                Err(e) => tracing::warn!("[WARN] autosave failed: {e}"),
            }
            busy.store(false, Ordering::SeqCst);
        });
    }

    /// Selected track of `kind`, or a new one.
    fn track_of_kind(&mut self, kind: TrackKind) -> TrackId {
        let reuse = self
            .selected_track
            .and_then(|id| self.timeline.track(id))
            .filter(|t| t.kind == kind)
            .map(|t| t.id);
        match reuse {
            Some(id) => id,
            None => {
                self.add_track(kind);
                self.selected_track.unwrap_or_default()
            }
        }
    }

    fn place_video(&mut self, path: PathBuf, info: VideoInfo, audio: Option<LoadedMedia>) {
        // The first clip defines the project format
        if self.timeline.tracks.iter().all(|t| t.events.is_empty()) {
            let settings = &mut self.project.settings;
            settings.width = info.width;
            settings.height = info.height;
            settings.fps = info.fps;
        }
        let media_id = self.timeline.alloc_id();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.project.media_pool.push(MediaRef {
            id: media_id,
            path: path.to_string_lossy().into_owned(),
            duration_seconds: info.duration,
        });

        let fade = if self.prefs.auto_fade_on_import {
            0.01
        } else {
            0.0
        };
        let video_track = self.track_of_kind(TrackKind::Video);
        let event = TrackEvent {
            id: self.timeline.alloc_id(),
            name: name.clone(),
            media_id: Some(media_id),
            start: self.playhead,
            duration: info.duration,
            trim_in: 0.0,
            fade_in: 0.0,
            fade_out: 0.0,
            fade_in_curve: FadeCurve::default(),
            fade_out_curve: FadeCurve::default(),
        };
        self.history.execute(
            &mut self.timeline,
            EditCommand::AddEvent {
                track_id: video_track,
                event,
            },
        );

        if let Some(audio) = audio {
            let audio_track = self.track_of_kind(TrackKind::Audio);
            let event = TrackEvent {
                id: self.timeline.alloc_id(),
                name: name.clone(),
                media_id: Some(media_id),
                start: self.playhead,
                duration: audio.duration,
                trim_in: 0.0,
                fade_in: fade,
                fade_out: fade,
                fade_in_curve: FadeCurve::default(),
                fade_out_curve: FadeCurve::default(),
            };
            self.history.execute(
                &mut self.timeline,
                EditCommand::AddEvent {
                    track_id: audio_track,
                    event,
                },
            );
            self.media.insert(media_id, audio);
            self.selected_track = Some(video_track);
        }
        self.video_info.insert(media_id, info);
        self.last_frame_key = None;
        self.status = format!("Importado: {name}");
    }

    /// Largest decoded width for the current preview quality (Draft 1/4, Preview 1/2, Good/Best full,
    /// Auto fits the preview pane).
    fn preview_max_width(&self, ppp: f32) -> u32 {
        let (_, factor) = QUALITIES[self.quality];
        let width = if factor > 0.0 {
            self.project.settings.width as f32 * factor
        } else {
            self.preview_pane_width * ppp
        };
        (width as u32).max(64)
    }

    /// Asks the decoder thread for the frame under the playhead (topmost video track wins).
    fn request_preview_frame(&mut self, ppp: f32) {
        let max_width = self.preview_max_width(ppp);
        let target = self
            .timeline
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Video && !t.muted)
            .find_map(|t| self.timeline.event_at(t.id, self.playhead))
            .and_then(|e| Some((e.media_id?, self.playhead - e.start + e.trim_in)));
        let Some((media_id, seconds)) = target else {
            self.video_texture = None;
            self.last_frame_key = None;
            return;
        };
        let Some(worker) = &self.frame_worker else {
            return;
        };
        let Some(item) = self.project.media_pool.iter().find(|m| m.id == media_id) else {
            return;
        };
        let fps = self
            .video_info
            .get(&media_id)
            .map_or(self.project.settings.fps, |i| i.fps);
        let frame = (seconds * fps).round() as i64;
        let key = (media_id, frame, max_width);
        if self.last_frame_key != Some(key) {
            self.last_frame_key = Some(key);
            worker.request(FrameRequest {
                media_id,
                path: PathBuf::from(&item.path),
                // frame-aligned so the decoder returns exactly this picture
                seconds: frame as f64 / fps,
                max_width,
            });
        }
    }

    fn poll_frames(&mut self, ctx: &egui::Context) {
        let Some(worker) = &self.frame_worker else {
            return;
        };
        // Only the newest reply matters
        let mut latest = None;
        while let Ok(reply) = worker.rx.try_recv() {
            latest = Some(reply);
        }
        let Some(reply) = latest else {
            return;
        };
        match reply.result {
            Ok(frame) => {
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [frame.width as usize, frame.height as usize],
                    &frame.rgba,
                );
                match &mut self.video_texture {
                    Some(texture) => texture.set(image, egui::TextureOptions::LINEAR),
                    None => {
                        self.video_texture = Some(ctx.load_texture(
                            "preview_frame",
                            image,
                            egui::TextureOptions::LINEAR,
                        ));
                    }
                }
            }
            Err(e) => self.status = format!("Falha ao decodificar vídeo: {e}"),
        }
    }

    fn place_media(&mut self, media: LoadedMedia) {
        let media_id = self.timeline.alloc_id();
        self.project.media_pool.push(MediaRef {
            id: media_id,
            path: media.path.to_string_lossy().into_owned(),
            duration_seconds: media.duration,
        });

        let reuse = self
            .selected_track
            .and_then(|id| self.timeline.track(id))
            .filter(|t| t.kind == TrackKind::Audio)
            .map(|t| t.id);
        let track_id = match reuse {
            Some(id) => id,
            None => {
                self.add_track(TrackKind::Audio);
                self.selected_track.unwrap_or_default()
            }
        };

        // Automatic fades on import are off unless enabled in the preferences
        let fade = if self.prefs.auto_fade_on_import {
            0.01
        } else {
            0.0
        };
        let event = TrackEvent {
            id: self.timeline.alloc_id(),
            name: media.name.clone(),
            media_id: Some(media_id),
            start: self.playhead,
            duration: media.duration,
            trim_in: 0.0,
            fade_in: fade,
            fade_out: fade,
            fade_in_curve: FadeCurve::default(),
            fade_out_curve: FadeCurve::default(),
        };
        self.status = format!("Importado: {}", media.name);
        self.media.insert(media_id, media);
        self.history.execute(
            &mut self.timeline,
            EditCommand::AddEvent { track_id, event },
        );
    }

    // ---- transport -----------------------------------------------------------------------------

    fn toggle_play(&mut self, now: f64) {
        if self.playing {
            self.stop();
            return;
        }
        if self.timeline.duration() <= 0.0 {
            self.status = "Nada para reproduzir".into();
            return;
        }
        if self.playhead >= self.timeline.duration() {
            self.playhead = 0.0;
        }
        self.playing = true;
        self.blink_origin = now;
        self.active_player = self.start_audio();
    }

    /// Every unmuted audio event with decoded media, ready for the mixer. Overlapping events on a
    /// track crossfade when automatic crossfades are on.
    fn build_clips(&self) -> Vec<MixClip> {
        let mut clips = Vec::new();
        for track in self
            .timeline
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Audio && !t.muted)
        {
            let crossfades = if self.project.settings.auto_crossfade {
                find_crossfades(&track.events)
            } else {
                Vec::new()
            };
            let longest = |select: &dyn Fn(&Crossfade) -> bool| {
                crossfades
                    .iter()
                    .filter(|c| select(c))
                    .map(Crossfade::length)
                    .fold(0.0, f64::max)
            };
            for e in &track.events {
                let Some(media) = e.media_id.and_then(|id| self.media.get(&id)) else {
                    continue;
                };
                // The fade-out of the earlier event mirrors the later event's fade-in curve
                let out_curve = crossfades
                    .iter()
                    .filter(|c| c.earlier == e.id)
                    .filter_map(|c| track.events.iter().find(|o| o.id == c.later))
                    .map(|later| later.fade_in_curve)
                    .next()
                    .unwrap_or_default();
                clips.push(MixClip {
                    samples: Arc::clone(&media.stereo),
                    source_rate: media.sample_rate,
                    start: e.start,
                    duration: e.duration,
                    trim_in: e.trim_in,
                    gain: 1.0,
                    fade_in: e.fade_in,
                    fade_out: e.fade_out,
                    fade_in_curve: e.fade_in_curve,
                    fade_out_curve: e.fade_out_curve,
                    cross_in: longest(&|c| c.later == e.id),
                    cross_in_curve: e.fade_in_curve,
                    cross_out: longest(&|c| c.earlier == e.id),
                    cross_out_curve: out_curve,
                });
            }
        }
        clips
    }

    fn start_audio(&mut self) -> Option<ActivePlayer> {
        let clips = self.build_clips();
        if clips.is_empty() {
            return None;
        }
        match AudioPlayer::new(clips, Arc::clone(&self.meter)) {
            Ok(player) => {
                player.set_volume(self.master_volume);
                player.play_from(self.playhead);
                Some(ActivePlayer { player })
            }
            Err(e) => {
                tracing::error!("[ERROR] audio output unavailable: {e}");
                self.status = format!("Saída de áudio indisponível: {e}");
                None
            }
        }
    }

    fn stop(&mut self) {
        if let Some(active) = self.active_player.take() {
            active.player.stop();
        }
        self.playing = false;
        // Meters stay zeroed whenever nothing is playing
        self.meter.reset();
    }

    fn advance_transport(&mut self, ctx: &egui::Context) {
        if !self.playing {
            return;
        }
        let dt = ctx.input(|i| i.unstable_dt) as f64;
        let ended = match &self.active_player {
            Some(active) => {
                self.playhead = active.player.position_seconds();
                !active.player.is_playing()
            }
            None => {
                self.playhead += dt;
                false
            }
        };
        if ended || self.playhead >= self.timeline.duration() {
            self.stop();
        }
        ctx.request_repaint();
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }
        let now = ctx.input(|i| i.time);
        let fps = self.project.settings.fps;
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F8)) {
            self.snapping = !self.snapping;
            self.status = if self.snapping {
                "Alinhamento ligado".into()
            } else {
                "Alinhamento desligado".into()
            };
        }
        let (save_as, save, open) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::S),
                i.consume_key(Modifiers::COMMAND, Key::S),
                i.consume_key(Modifiers::COMMAND, Key::O),
            )
        });
        if save_as {
            self.save_project(true);
        } else if save {
            self.save_project(false);
        }
        if open {
            self.open_project(ctx);
        }
        // Shifted variants are consumed first so Ctrl+Shift+Q is not read as Ctrl+Q
        let keys = ctx.input_mut(|i| {
            let ctrl_shift = Modifiers::COMMAND | Modifiers::SHIFT;
            (
                i.consume_key(ctrl_shift, Key::Q),
                i.consume_key(ctrl_shift, Key::Z),
                i.consume_key(Modifiers::COMMAND, Key::Q),
                i.consume_key(Modifiers::COMMAND, Key::Z),
                i.consume_key(Modifiers::COMMAND, Key::Y),
                i.consume_key(Modifiers::NONE, Key::Space),
                i.consume_key(Modifiers::NONE, Key::S),
                i.consume_key(Modifiers::NONE, Key::Delete),
                i.consume_key(Modifiers::NONE, Key::ArrowLeft),
                i.consume_key(Modifiers::NONE, Key::ArrowRight),
            )
        });
        let (add_video, redo_shift, add_audio, undo, redo, space, split, delete, left, right) =
            keys;

        if add_video {
            self.add_track(TrackKind::Video);
        }
        if add_audio {
            self.add_track(TrackKind::Audio);
        }
        if undo {
            self.history.undo(&mut self.timeline);
        }
        if redo || redo_shift {
            self.history.redo(&mut self.timeline);
        }
        if space {
            self.toggle_play(now);
        }
        if split {
            self.split_at_playhead();
        }
        if delete {
            self.delete_selected_event();
        }
        if left || right {
            let frame = (self.playhead * fps).round() + if right { 1.0 } else { -1.0 };
            self.playhead = (frame / fps).max(0.0);
            // Keyboard frame navigation restarts the caret blink
            self.blink_origin = now;
        }
    }
}

// ============================================================================================
// Application
// ============================================================================================

pub struct NeovegApp {
    dock: DockState<Tab>,
    state: AppState,
}

impl NeovegApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        let mut dock = DockState::new(vec![Tab::Preview]);
        let surface = dock.main_surface_mut();
        let [top, _timeline] = surface.split_below(NodeIndex::root(), 0.52, vec![Tab::Timeline]);
        let [preview, _media] = surface.split_left(top, 0.28, vec![Tab::Media]);
        surface.split_right(preview, 0.82, vec![Tab::MasterBus]);

        let mut state = AppState::new();
        let repaint = cc.egui_ctx.clone();
        match FrameWorker::spawn(move || repaint.request_repaint()) {
            Ok(worker) => state.frame_worker = Some(worker),
            Err(e) => {
                tracing::warn!("[WARN] {e}");
                state.status = e;
            }
        }
        if let Some(dir) = autosave_dir() {
            autosave::prune_older_than(&dir, Duration::from_secs(30 * 86_400));
            state.pending_recovery = autosave::list_recoveries(&dir);
            if !state.pending_recovery.is_empty() {
                tracing::warn!(
                    "[WARN] {} unsaved project(s) found for recovery",
                    state.pending_recovery.len()
                );
            }
        }
        Self { dock, state }
    }
}

impl eframe::App for NeovegApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = ctx.input(|i| i.time);
        self.state.maybe_autosave(now);
        self.state.update_title(ctx);
        if ctx.input(|i| i.viewport().close_requested()) && !self.state.allow_close {
            if self.state.is_dirty() {
                // Ask before losing unsaved work
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.state.confirm_exit = true;
            } else {
                self.state.stop();
                self.state.discard_own_recovery();
            }
        }
        self.state.poll_loader();
        self.state.poll_frames(ctx);
        self.state.request_preview_frame(ctx.pixels_per_point());
        self.state.handle_shortcuts(ctx);
        self.state.advance_transport(ctx);
        // Master fader drives the live output gain
        if let Some(active) = &self.state.active_player {
            active.player.set_volume(self.state.master_volume);
        }

        egui::TopBottomPanel::top("menu").show(ctx, |ui| menu_bar(ui, &mut self.state));
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.state.status);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let s = &self.state.project.settings;
                    ui.label(format!(
                        "{}x{} · {:.3} fps · {} Hz",
                        s.width, s.height, s.fps, s.sample_rate
                    ));
                });
            });
        });

        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                let mut style = Style::from_egui(ui.style().as_ref());
                // egui_dock cannot place its strip at the bottom; grouped panels use `footer_tabs` instead
                style.tab_bar.height = 22.0;
                DockArea::new(&mut self.dock).style(style).show_inside(
                    ui,
                    &mut Viewer {
                        state: &mut self.state,
                    },
                );
            });

        preferences_window(ctx, &mut self.state);
        recovery_window(ctx, &mut self.state);
        exit_window(ctx, &mut self.state);
        inspector_window(ctx, &mut self.state);
    }
}

/// Decodes an audio file and builds its waveform source.
fn load_audio(path: &std::path::Path) -> Result<LoadedMedia, String> {
    let audio = decode_file(path)?;
    let stereo = audio.to_stereo();
    let waveform = WaveformSource::from_interleaved_f32(audio.sample_rate, 2, &stereo)?;
    Ok(LoadedMedia {
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        path: path.to_path_buf(),
        waveform: Arc::new(waveform),
        stereo: Arc::new(stereo),
        sample_rate: audio.sample_rate,
        duration: audio.duration_seconds(),
    })
}

fn menu_bar(ui: &mut egui::Ui, state: &mut AppState) {
    egui::menu::bar(ui, |ui| {
        ui.menu_button("Arquivo", |ui| {
            if ui.button("Novo projeto").clicked() {
                state.new_project();
                ui.close_menu();
            }
            if ui.button("Abrir…  Ctrl+O").clicked() {
                let ctx = ui.ctx().clone();
                state.open_project(&ctx);
                ui.close_menu();
            }
            if ui.button("Salvar  Ctrl+S").clicked() {
                state.save_project(false);
                ui.close_menu();
            }
            if ui.button("Salvar como…  Ctrl+Shift+S").clicked() {
                state.save_project(true);
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Importar mídia…").clicked() {
                let ctx = ui.ctx().clone();
                state.import_media(&ctx);
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Sair").clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
        ui.menu_button("Editar", |ui| {
            if ui
                .add_enabled(
                    state.history.can_undo(),
                    egui::Button::new("Desfazer  Ctrl+Z"),
                )
                .clicked()
            {
                state.history.undo(&mut state.timeline);
                ui.close_menu();
            }
            if ui
                .add_enabled(
                    state.history.can_redo(),
                    egui::Button::new("Refazer  Ctrl+Y"),
                )
                .clicked()
            {
                state.history.redo(&mut state.timeline);
                ui.close_menu();
            }
            ui.separator();
            let mut auto = state.project.settings.auto_crossfade;
            if ui.checkbox(&mut auto, "Crossfade automático").changed() {
                state.project.settings.auto_crossfade = auto;
                state.forced_dirty = true;
            }
            ui.separator();
            if ui.button("Preferências…").clicked() {
                state.show_preferences = true;
                ui.close_menu();
            }
        });
        ui.menu_button("Ferramentas", |ui| {
            if ui.button("Inspetor").clicked() {
                state.show_inspector = true;
                ui.close_menu();
            }
        });
        ui.menu_button("Exibir", |ui| {
            ui.checkbox(&mut state.show_bars, "Barras de cor SMPTE");
        });
    });
}

fn preferences_window(ctx: &egui::Context, state: &mut AppState) {
    let mut open = state.show_preferences;
    let mut prefs = state.prefs.clone();
    egui::Window::new("Preferências")
        .open(&mut open)
        .collapsible(false)
        .show(ctx, |ui| {
            egui::ComboBox::from_label("Idioma")
                .selected_text(prefs.language.clone())
                .show_ui(ui, |ui| {
                    for lang in LANGUAGES {
                        ui.selectable_value(&mut prefs.language, lang.to_string(), lang);
                    }
                });
            ui.horizontal(|ui| {
                ui.label("Autosave (minutos, 0 = desligado)");
                ui.add(egui::DragValue::new(&mut prefs.autosave_minutes).range(0..=120));
            });
            ui.checkbox(
                &mut prefs.auto_fade_on_import,
                "Fade-in/out automático ao importar mídia",
            );
        });
    if prefs != state.prefs {
        if let Some(path) = Preferences::default_path() {
            if let Err(e) = prefs.save(&path) {
                tracing::warn!("[WARN] cannot save preferences: {e}");
            }
        }
        state.prefs = prefs;
    }
    state.show_preferences = open;
}

/// "há 5 min", "há 2 h", "há 3 dias".
fn format_age(saved_unix: u64, now_unix: u64) -> String {
    let secs = now_unix.saturating_sub(saved_unix);
    match secs {
        0..=59 => "agora há pouco".into(),
        60..=3599 => format!("há {} min", secs / 60),
        3600..=86_399 => format!("há {} h", secs / 3600),
        _ => format!("há {} dia(s)", secs / 86_400),
    }
}

fn recovery_window(ctx: &egui::Context, state: &mut AppState) {
    if state.pending_recovery.is_empty() {
        return;
    }
    let (mut recover, mut discard, mut discard_all) = (None, None, false);
    let now = autosave::unix_now();
    egui::Window::new("Recuperar projetos")
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label("O Neoveg não foi encerrado normalmente. Projetos com alterações não salvas:");
            ui.separator();
            for entry in &state.pending_recovery {
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "{} ({})",
                        entry.meta.project_name,
                        format_age(entry.meta.saved_unix, now)
                    ));
                    if ui.button("Recuperar").clicked() {
                        recover = Some(entry.clone());
                    }
                    if ui.button("Descartar").clicked() {
                        discard = Some(entry.path.clone());
                    }
                });
            }
            ui.separator();
            if ui.button("Descartar todos").clicked() {
                discard_all = true;
            }
        });

    if let Some(entry) = recover {
        state.recover_entry(&entry, ctx);
    } else if let Some(path) = discard {
        autosave::remove(&path);
        state.pending_recovery.retain(|e| e.path != path);
    } else if discard_all {
        for entry in state.pending_recovery.drain(..) {
            autosave::remove(&entry.path);
        }
    }
}

fn exit_window(ctx: &egui::Context, state: &mut AppState) {
    if !state.confirm_exit {
        return;
    }
    let (mut save, mut discard, mut cancel) = (false, false, false);
    egui::Window::new("Alterações não salvas")
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(format!(
                "Salvar as alterações em \"{}\" antes de sair?",
                state.project.name
            ));
            ui.horizontal(|ui| {
                save = ui.button("Salvar e sair").clicked();
                discard = ui.button("Sair sem salvar").clicked();
                cancel = ui.button("Cancelar").clicked();
            });
        });

    if save {
        state.save_project(false);
        // A cancelled save dialog leaves the project dirty, so the prompt stays open
        if !state.is_dirty() {
            state.confirm_exit = false;
            state.allow_close = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    } else if discard {
        state.discard_own_recovery();
        state.confirm_exit = false;
        state.allow_close = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    } else if cancel {
        state.confirm_exit = false;
    }
}

fn inspector_window(ctx: &egui::Context, state: &mut AppState) {
    let mut open = state.show_inspector;
    let mut change = None;
    egui::Window::new("Inspetor")
        .open(&mut open)
        .show(ctx, |ui| {
            let selected = state.selected_event.and_then(|(track_id, event_id)| {
                let name = state
                    .timeline
                    .track(track_id)?
                    .events
                    .iter()
                    .find(|e| e.id == event_id)?
                    .name
                    .clone();
                Some((
                    track_id,
                    event_id,
                    name,
                    state.event_geometry(track_id, event_id)?,
                ))
            });
            let Some((track_id, event_id, name, g)) = selected else {
                ui.label("Nenhum evento selecionado");
                return;
            };
            ui.label(format!("Evento: {name}"));
            ui.label(format!("Início: {:.3} s", g.start));
            ui.label(format!("Duração: {:.3} s", g.duration));
            ui.label(format!("Corte de entrada: {:.3} s", g.trim_in));
            ui.separator();

            let mut new = g;
            ui.horizontal(|ui| {
                ui.label("Fade-in");
                ui.add(
                    egui::DragValue::new(&mut new.fade_in)
                        .range(0.0..=g.duration)
                        .speed(0.01)
                        .suffix(" s"),
                );
                egui::ComboBox::from_id_salt("fade_in_curve")
                    .selected_text(new.fade_in_curve.label())
                    .show_ui(ui, |ui| {
                        for curve in FadeCurve::ALL {
                            ui.selectable_value(&mut new.fade_in_curve, curve, curve.label());
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label("Fade-out");
                ui.add(
                    egui::DragValue::new(&mut new.fade_out)
                        .range(0.0..=g.duration)
                        .speed(0.01)
                        .suffix(" s"),
                );
                egui::ComboBox::from_id_salt("fade_out_curve")
                    .selected_text(new.fade_out_curve.label())
                    .show_ui(ui, |ui| {
                        for curve in FadeCurve::ALL {
                            ui.selectable_value(&mut new.fade_out_curve, curve, curve.label());
                        }
                    });
            });
            // The two fades share the event: whichever was edited wins the room
            if new.fade_in != g.fade_in {
                new = edit::set_fade_in(new, new.fade_in);
            } else if new.fade_out != g.fade_out {
                new = edit::set_fade_out(new, new.fade_out);
            }
            if new != g {
                change = Some((track_id, event_id, new));
            }
        });
    if let Some((track_id, event_id, new)) = change {
        state.apply_geometry(track_id, event_id, new);
    }
    state.show_inspector = open;
}

// ============================================================================================
// Tabs
// ============================================================================================

struct Viewer<'a> {
    state: &'a mut AppState,
}

impl TabViewer for Viewer<'_> {
    type Tab = Tab;

    fn title(&mut self, tab: &mut Tab) -> egui::WidgetText {
        tab.title().into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Tab) {
        match tab {
            Tab::Preview => preview_ui(self.state, ui),
            Tab::Timeline => timeline_ui(self.state, ui),
            Tab::Media => media_ui(self.state, ui),
            Tab::MasterBus => master_bus_ui(self.state, ui),
        }
    }

    fn closeable(&mut self, _tab: &mut Tab) -> bool {
        false
    }
}

fn preview_ui(state: &mut AppState, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("preview_quality")
            .selected_text(QUALITIES[state.quality].0)
            .show_ui(ui, |ui| {
                for (i, (name, _)) in QUALITIES.iter().enumerate() {
                    ui.selectable_value(&mut state.quality, i, *name);
                }
            })
            .response
            .on_hover_text("Qualidade da prévia (Draft 1/4, Preview 1/2, Good/Best total, Auto ajusta à janela)");
        ui.checkbox(&mut state.show_bars, "Barras SMPTE");
    });

    let avail = ui.available_size();
    let (rect, _) = ui.allocate_exact_size(avail, Sense::hover());
    let (pw, ph) = (
        state.project.settings.width as f32,
        state.project.settings.height as f32,
    );
    let scale = (rect.width() / pw).min(rect.height() / ph).max(0.01);
    let frame_rect = Rect::from_center_size(rect.center(), vec2(pw * scale, ph * scale));
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, Rounding::ZERO, Color32::from_gray(18));
    painter.rect_filled(frame_rect, Rounding::ZERO, Color32::BLACK);
    state.preview_pane_width = frame_rect.width();
    if let Some(texture) = &state.video_texture {
        painter.image(
            texture.id(),
            frame_rect,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }

    if state.show_bars {
        let texture = state.bars_texture.get_or_insert_with(|| {
            let bars = neoveg_video::generators::smpte_color_bars(640, 360);
            let image = egui::ColorImage::from_rgba_unmultiplied([640, 360], &bars.rgba);
            ui.ctx()
                .load_texture("smpte_bars", image, egui::TextureOptions::LINEAR)
        });
        painter.image(
            texture.id(),
            frame_rect,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }

    let factor = match QUALITIES[state.quality].1 {
        f if f > 0.0 => f,
        _ => scale.min(1.0),
    };
    painter.text(
        rect.left_bottom() + vec2(6.0, -6.0),
        Align2::LEFT_BOTTOM,
        format!(
            "{}x{} · prévia {}x{} · {}",
            pw as u32,
            ph as u32,
            (pw * factor) as u32,
            (ph * factor) as u32,
            seconds_to_timecode(
                state.playhead,
                state.project.settings.fps,
                state.drop_frame()
            )
        ),
        FontId::monospace(11.0),
        Color32::from_gray(170),
    );
}

/// Classic NLE compact footer tabs: flat 22 px headers in a strip at the bottom of the panel.
fn footer_tabs(ui: &mut egui::Ui, id: &str, labels: &[&str], selected: &mut usize) {
    egui::TopBottomPanel::bottom(egui::Id::new(id))
        .exact_height(22.0)
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for (i, label) in labels.iter().enumerate() {
                    let text = egui::RichText::new(*label).size(11.0);
                    if ui
                        .add(
                            egui::Button::new(text)
                                .rounding(Rounding::ZERO)
                                .selected(*selected == i),
                        )
                        .clicked()
                    {
                        *selected = i;
                    }
                }
            });
        });
}

fn media_ui(state: &mut AppState, ui: &mut egui::Ui) {
    let mut tab = state.media_tab;
    footer_tabs(
        ui,
        "media_footer",
        &["Mídia do Projeto", "Trimmer"],
        &mut tab,
    );
    state.media_tab = tab;
    if tab == 0 {
        project_media_ui(state, ui);
    } else {
        trimmer_ui(state, ui);
    }
}

fn project_media_ui(state: &mut AppState, ui: &mut egui::Ui) {
    if ui.button("Importar mídia…").clicked() {
        let ctx = ui.ctx().clone();
        state.import_media(&ctx);
    }
    ui.separator();
    if state.project.media_pool.is_empty() {
        ui.weak("Nenhuma mídia no projeto");
    }
    for item in &state.project.media_pool {
        if let Some(m) = state.media.get(&item.id) {
            ui.label(format!(
                "{}  ({:.2} s, {} Hz)",
                m.name, m.duration, m.sample_rate
            ));
        }
    }
}

fn trimmer_ui(state: &mut AppState, ui: &mut egui::Ui) {
    let Some(media) = state
        .project
        .media_pool
        .last()
        .and_then(|m| state.media.get(&m.id))
    else {
        ui.weak("Selecione uma mídia para pré-visualizar");
        return;
    };
    ui.label(&media.name);
    let (rect, _) = ui.allocate_exact_size(
        vec2(ui.available_width(), ui.available_height().max(60.0)),
        Sense::hover(),
    );
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, Rounding::ZERO, Color32::from_gray(22));
    let frames = media.waveform.frame_count() as f64;
    paint_waveform(
        &painter,
        rect,
        &media.waveform,
        0.0,
        frames / rect.width() as f64,
        Color32::from_rgb(86, 204, 242),
    );
}

fn fmt_db(linear: f32) -> String {
    let db = linear_to_dbfs(linear);
    if db.is_finite() {
        format!("{db:+.1}")
    } else {
        "-∞".into()
    }
}

fn master_bus_ui(state: &mut AppState, ui: &mut egui::Ui) {
    // Meters only ever show what the audio pipeline reported; stopped means zero
    let (l, r) = if state.playing {
        state.meter.peak()
    } else {
        (0.0, 0.0)
    };

    let meter_h = (ui.available_height() - 80.0).max(60.0);
    let (rect, _) = ui.allocate_exact_size(vec2(150.0, meter_h), Sense::hover());
    let painter = ui.painter_at(rect);
    let (top, bottom) = (rect.top() + 6.0, rect.bottom() - 6.0);
    let y_of = |db: f32| bottom - ((db + 60.0) / 60.0).clamp(0.0, 1.0) * (bottom - top);

    for (label, db) in [
        ("0", 0.0),
        ("-6", -6.0),
        ("-12", -12.0),
        ("-24", -24.0),
        ("-48", -48.0),
        ("-∞", -60.0),
    ] {
        painter.text(
            pos2(rect.left() + 34.0, y_of(db)),
            Align2::RIGHT_CENTER,
            label,
            FontId::monospace(10.0),
            Color32::from_gray(150),
        );
    }
    for (i, level) in [l, r].into_iter().enumerate() {
        let x = rect.left() + 44.0 + i as f32 * 36.0;
        let bar = Rect::from_min_max(pos2(x, top), pos2(x + 28.0, bottom));
        painter.rect_filled(bar, Rounding::ZERO, Color32::from_gray(14));
        let db = linear_to_dbfs(level);
        if db.is_finite() {
            let color = if db > -3.0 {
                Color32::from_rgb(231, 76, 60)
            } else if db > -12.0 {
                Color32::from_rgb(241, 196, 15)
            } else {
                Color32::from_rgb(46, 204, 113)
            };
            painter.rect_filled(
                Rect::from_min_max(pos2(x, y_of(db)), bar.right_bottom()),
                Rounding::ZERO,
                color,
            );
        }
    }

    ui.monospace(format!("L {} dB    R {} dB", fmt_db(l), fmt_db(r)));
    let master_db = 20.0 * state.master_volume.max(1e-6).log10();
    ui.monospace(format!("MASTER BUS: {master_db:+.1} dB"));
    ui.add(PrecisionSlider::new(&mut state.master_volume, 0.0..=2.0, 1.0).width(150.0));
}

// ============================================================================================
// Timeline
// ============================================================================================

fn x_to_time(state: &AppState, left: f32, x: f32) -> f64 {
    state.scroll_seconds + (x - left) as f64 / state.pixels_per_second
}

/// Width of the trim zones at both ends of an event.
const TRIM_EDGE_PX: f32 = 7.0;
/// Half-width of the fade handles and the minimum distance they sit from the event edge.
const FADE_HANDLE_PX: f32 = 6.0;
const FADE_HANDLE_MIN_OFFSET_PX: f32 = 8.0;
/// Events narrower than this have no fade handles.
const FADE_HANDLE_MIN_EVENT_PX: f32 = 28.0;

/// Handle centres (fade-in, fade-out) in pixels for an event spanning `x0..x1`.
fn fade_handle_x(ev_fade_in: f64, ev_fade_out: f64, pps: f64, x0: f32, x1: f32) -> (f32, f32) {
    (
        x0 + ((ev_fade_in * pps) as f32).max(FADE_HANDLE_MIN_OFFSET_PX),
        x1 - ((ev_fade_out * pps) as f32).max(FADE_HANDLE_MIN_OFFSET_PX),
    )
}

/// Event under `p` and what a drag from there would do.
fn hit_event(
    state: &AppState,
    lanes_left: f32,
    lane_top: f32,
    p: Pos2,
) -> Option<(TrackId, EventId, DragMode, EventGeometry)> {
    let row = ((p.y - lane_top) / TRACK_HEIGHT).floor();
    if row < 0.0 {
        return None;
    }
    let track = state.timeline.tracks.get(row as usize)?;
    let to_px = |t: f64| lanes_left + ((t - state.scroll_seconds) * state.pixels_per_second) as f32;
    track.events.iter().rev().find_map(|ev| {
        let (x0, x1) = (to_px(ev.start), to_px(ev.end()));
        if p.x < x0 || p.x > x1 {
            return None;
        }
        // Fade handles live in the event's header strip and win over the trim zones there
        let row_top = lane_top + row * TRACK_HEIGHT;
        if p.y - row_top <= 2.0 + EVENT_HEADER_HEIGHT && x1 - x0 >= FADE_HANDLE_MIN_EVENT_PX {
            let (hin, hout) =
                fade_handle_x(ev.fade_in, ev.fade_out, state.pixels_per_second, x0, x1);
            let (din, dout) = ((p.x - hin).abs(), (p.x - hout).abs());
            if din <= FADE_HANDLE_PX && din <= dout {
                return Some((track.id, ev.id, DragMode::FadeIn, ev.geometry()));
            }
            if dout <= FADE_HANDLE_PX {
                return Some((track.id, ev.id, DragMode::FadeOut, ev.geometry()));
            }
        }
        let edge = TRIM_EDGE_PX.min((x1 - x0) / 3.0);
        let mode = if p.x - x0 <= edge {
            DragMode::TrimLeft
        } else if x1 - p.x <= edge {
            DragMode::TrimRight
        } else {
            DragMode::Move
        };
        Some((track.id, ev.id, mode, ev.geometry()))
    })
}

fn curve_menu(
    ui: &mut egui::Ui,
    title: &str,
    current: FadeCurve,
    mut apply: impl FnMut(FadeCurve),
) {
    ui.menu_button(title, |ui| {
        for curve in FadeCurve::ALL {
            if ui
                .selectable_label(current == curve, curve.label())
                .clicked()
            {
                apply(curve);
                ui.close_menu();
            }
        }
    });
}

fn event_context_menu(
    state: &mut AppState,
    ui: &mut egui::Ui,
    track_id: TrackId,
    event_id: EventId,
) {
    let Some(g) = state.event_geometry(track_id, event_id) else {
        ui.label("Evento não encontrado");
        return;
    };
    curve_menu(ui, "Curva do fade-in", g.fade_in_curve, |c| {
        state.apply_geometry(
            track_id,
            event_id,
            EventGeometry {
                fade_in_curve: c,
                ..g
            },
        );
    });
    curve_menu(ui, "Curva do fade-out", g.fade_out_curve, |c| {
        state.apply_geometry(
            track_id,
            event_id,
            EventGeometry {
                fade_out_curve: c,
                ..g
            },
        );
    });
    ui.separator();
    let has_fades = g.fade_in > 0.0 || g.fade_out > 0.0;
    if ui
        .add_enabled(has_fades, egui::Button::new("Remover fades"))
        .clicked()
    {
        state.apply_geometry(
            track_id,
            event_id,
            EventGeometry {
                fade_in: 0.0,
                fade_out: 0.0,
                ..g
            },
        );
        ui.close_menu();
    }
}

fn timeline_ui(state: &mut AppState, ui: &mut egui::Ui) {
    let canvas_h = (ui.available_height() - 30.0).max(100.0);
    timeline_canvas(state, ui, canvas_h);
    toolbar_ui(state, ui);
}

fn toolbar_ui(state: &mut AppState, ui: &mut egui::Ui) {
    let now = ui.input(|i| i.time);
    ui.horizontal(|ui| {
        let icon = |ui: &mut egui::Ui, glyph: &str, tip: &str| {
            ui.button(glyph).on_hover_text(tip).clicked()
        };
        if icon(ui, "⏮", "Ir para o início") {
            state.playhead = 0.0;
            state.blink_origin = now;
        }
        let play_glyph = if state.playing { "⏸" } else { "▶" };
        if icon(ui, play_glyph, "Reproduzir / Pausar (Espaço)") {
            state.toggle_play(now);
        }
        if icon(ui, "⏹", "Parar") {
            state.stop();
        }
        ui.separator();
        if icon(ui, "✂", "Dividir no cursor (S)") {
            state.split_at_playhead();
        }
        if icon(ui, "↶", "Desfazer (Ctrl+Z)") {
            state.history.undo(&mut state.timeline);
        }
        if icon(ui, "↷", "Refazer (Ctrl+Y)") {
            state.history.redo(&mut state.timeline);
        }
        ui.separator();
        if icon(ui, "🔊+", "Inserir faixa de áudio (Ctrl+Q)") {
            state.add_track(TrackKind::Audio);
        }
        if icon(ui, "🎞+", "Inserir faixa de vídeo (Ctrl+Shift+Q)") {
            state.add_track(TrackKind::Video);
        }
        ui.separator();
        if ui
            .selectable_label(state.snapping, "🧲")
            .on_hover_text(
                "Alinhar a bordas, cursor e quadros (F8; segure Alt ao arrastar para ignorar)",
            )
            .clicked()
        {
            state.snapping = !state.snapping;
        }
        ui.separator();
        if ui
            .selectable_label(state.project.settings.auto_crossfade, "⤫")
            .on_hover_text("Crossfade automático entre eventos sobrepostos")
            .clicked()
        {
            state.project.settings.auto_crossfade = !state.project.settings.auto_crossfade;
            state.forced_dirty = true;
        }
        // Toolbar zoom is anchored at the edit cursor
        for (glyph, tip, factor) in [("−", "Reduzir zoom", 0.5), ("+", "Ampliar zoom", 2.0)] {
            if icon(ui, glyph, tip) {
                let offset = state.playhead - state.scroll_seconds;
                state.pixels_per_second = (state.pixels_per_second * factor)
                    .clamp(MIN_PIXELS_PER_SECOND, MAX_PIXELS_PER_SECOND);
                state.scroll_seconds = (state.playhead - offset * (1.0 / factor)).max(0.0);
            }
        }
    });
}

fn timeline_canvas(state: &mut AppState, ui: &mut egui::Ui, canvas_h: f32) {
    let size = vec2(ui.available_width(), canvas_h);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let ppp = ui.ctx().pixels_per_point();
    let now = ui.input(|i| i.time);
    let fps = state.project.settings.fps;
    let drop_frame = state.drop_frame();
    let lanes = Rect::from_min_max(pos2(rect.left() + HEADER_WIDTH, rect.top()), rect.max);

    // ---- zoom / scroll (zoom anchored at the pointer) ------------------------------------------
    if response.hovered() {
        let zoom = ui.input(|i| i.zoom_delta()) as f64;
        let scroll = ui.input(|i| i.smooth_scroll_delta);
        if let Some(p) = response.hover_pos() {
            let anchor = x_to_time(state, lanes.left(), p.x);
            if (zoom - 1.0).abs() > 1e-6 {
                state.pixels_per_second = (state.pixels_per_second * zoom)
                    .clamp(MIN_PIXELS_PER_SECOND, MAX_PIXELS_PER_SECOND);
                state.scroll_seconds =
                    (anchor - (p.x - lanes.left()) as f64 / state.pixels_per_second).max(0.0);
            } else if scroll != egui::Vec2::ZERO {
                state.scroll_seconds = (state.scroll_seconds
                    - (scroll.x + scroll.y) as f64 / state.pixels_per_second)
                    .max(0.0);
            }
        }
    }

    // ---- clicks: ruler scrubs the playhead, lanes and headers select tracks/events -------------
    if response.clicked() || response.dragged() {
        if let Some(p) = response.interact_pointer_pos() {
            let in_ruler = p.y < rect.top() + RULER_HEIGHT;
            let row = ((p.y - rect.top() - RULER_HEIGHT) / TRACK_HEIGHT).floor();
            let track_id = (row >= 0.0)
                .then(|| state.timeline.tracks.get(row as usize).map(|t| t.id))
                .flatten();
            if p.x >= lanes.left() {
                let t = x_to_time(state, lanes.left(), p.x).max(0.0);
                let snapped = (t * fps).floor() / fps;
                if in_ruler || response.clicked() {
                    state.playhead = snapped;
                    state.blink_origin = now;
                }
                if response.clicked() && !in_ruler {
                    state.selected_track = track_id;
                    state.selected_event =
                        track_id.and_then(|id| state.timeline.event_at(id, t).map(|e| (id, e.id)));
                }
            } else if response.clicked() && track_id.is_some() {
                state.selected_track = track_id;
            }
        }
    }

    // ---- dragging events: move, trim left/right, change track ------------------------------------
    let lane_top = rect.top() + RULER_HEIGHT;
    if response.drag_started() {
        let press = ui.input(|i| i.pointer.press_origin());
        if let Some(p) = press.filter(|p| p.x >= lanes.left() && p.y >= lane_top) {
            if let Some(hit) = hit_event(state, lanes.left(), lane_top, p) {
                let t = x_to_time(state, lanes.left(), p.x);
                state.begin_drag(hit, t);
            }
        }
    }
    if state.drag.is_some() {
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            state.drag = None;
        } else if let Some(p) = response.interact_pointer_pos() {
            let t = x_to_time(state, lanes.left(), p.x);
            let row = ((p.y - lane_top) / TRACK_HEIGHT).floor();
            let row_track = (row >= 0.0)
                .then(|| state.timeline.tracks.get(row as usize).map(|t| t.id))
                .flatten();
            let snap_enabled = state.snapping && !ui.input(|i| i.modifiers.alt);
            let threshold = f64::from(SNAP_THRESHOLD_PX) / state.pixels_per_second;
            state.update_drag(t, row_track, threshold, snap_enabled);

            // Scroll while the pointer rests near either end of the lane area
            let dt = f64::from(ui.input(|i| i.unstable_dt));
            let direction = if p.x < lanes.left() + AUTOSCROLL_EDGE_PX {
                -1.0
            } else if p.x > lanes.right() - AUTOSCROLL_EDGE_PX {
                1.0
            } else {
                0.0
            };
            if direction != 0.0 {
                state.scroll_seconds = (state.scroll_seconds
                    + direction * AUTOSCROLL_PX_PER_SECOND * dt / state.pixels_per_second)
                    .max(0.0);
            }
            ui.ctx().request_repaint();
        }
    }
    if response.drag_stopped() {
        state.finish_drag();
    }

    // Right-click on an event: fade curves
    if response.secondary_clicked() {
        let pressed = response
            .interact_pointer_pos()
            .filter(|p| p.x >= lanes.left() && p.y >= lane_top);
        state.context_event = pressed
            .and_then(|p| hit_event(state, lanes.left(), lane_top, p))
            .map(|h| (h.0, h.1));
        if let Some((track_id, event_id)) = state.context_event {
            state.selected_track = Some(track_id);
            state.selected_event = Some((track_id, event_id));
        }
    }
    response.context_menu(|ui| match state.context_event {
        Some((track_id, event_id)) => event_context_menu(state, ui, track_id, event_id),
        None => {
            ui.label("Clique com o botão direito em um evento");
        }
    });

    // Cursor feedback
    if let Some(drag) = &state.drag {
        ui.ctx().set_cursor_icon(match drag.mode {
            DragMode::Move => egui::CursorIcon::Grabbing,
            DragMode::FadeIn | DragMode::FadeOut => egui::CursorIcon::ResizeHorizontal,
            _ => egui::CursorIcon::ResizeHorizontal,
        });
    } else if let Some(p) = response.hover_pos().filter(|p| p.x >= lanes.left()) {
        match hit_event(state, lanes.left(), lane_top, p).map(|h| h.2) {
            Some(
                DragMode::TrimLeft | DragMode::TrimRight | DragMode::FadeIn | DragMode::FadeOut,
            ) => {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
            Some(DragMode::Move) => ui.ctx().set_cursor_icon(egui::CursorIcon::Grab),
            None => {}
        }
    }

    // ---- drawing ---------------------------------------------------------------------------------
    let painter = ui.painter_at(rect);
    let lane_painter = painter.with_clip_rect(lanes);
    painter.rect_filled(rect, Rounding::ZERO, Color32::from_rgb(26, 28, 33));

    for (i, _) in state.timeline.tracks.iter().enumerate() {
        let y = lane_top + i as f32 * TRACK_HEIGHT;
        let shade = if i % 2 == 0 { 31 } else { 28 };
        lane_painter.rect_filled(
            Rect::from_min_max(pos2(lanes.left(), y), pos2(lanes.right(), y + TRACK_HEIGHT)),
            Rounding::ZERO,
            Color32::from_rgb(shade, shade + 2, shade + 6),
        );
    }

    let hairline = 1.0 / ppp;
    let to_x = |t: f64| {
        snap_to_device_pixel(
            lanes.left() + ((t - state.scroll_seconds) * state.pixels_per_second) as f32,
            ppp,
        )
    };
    let grid = FrameGrid::compute(
        fps,
        state.pixels_per_second,
        state.scroll_seconds,
        lanes.width() as f64,
    );
    let minor = Stroke::new(hairline, Color32::from_rgb(0x2C, 0x30, 0x38));
    let major = Stroke::new(hairline, Color32::from_rgb(0x4A, 0x50, 0x5E));
    for &t in &grid.minor_times {
        let x = to_x(t);
        lane_painter.line_segment([pos2(x, lane_top), pos2(x, rect.bottom())], minor);
    }
    for &t in &grid.major_times {
        let x = to_x(t);
        lane_painter.line_segment(
            [
                pos2(x, rect.top() + RULER_HEIGHT - 6.0),
                pos2(x, rect.bottom()),
            ],
            major,
        );
        lane_painter.text(
            pos2(x + 3.0, rect.top() + 3.0),
            Align2::LEFT_TOP,
            seconds_to_timecode(t, fps, drop_frame),
            FontId::monospace(10.0),
            Color32::from_gray(160),
        );
    }

    // Events (culled and clamped so far zoom levels keep float precision)
    for (i, track) in state.timeline.tracks.iter().enumerate() {
        let y = lane_top + i as f32 * TRACK_HEIGHT;
        let base = match track.kind {
            TrackKind::Audio => Color32::from_rgb(38, 82, 120),
            TrackKind::Video => Color32::from_rgb(88, 66, 130),
        };
        for ev in &track.events {
            let dragging = state
                .drag
                .as_ref()
                .filter(|d| d.track_id == track.id && d.event_id == ev.id);
            let g = dragging.map_or_else(|| ev.geometry(), |d| d.current);
            let row_y = dragging
                .and_then(|d| {
                    state
                        .timeline
                        .tracks
                        .iter()
                        .position(|t| t.id == d.target_track)
                })
                .map_or(y, |row| lane_top + row as f32 * TRACK_HEIGHT);
            let x0 = (lanes.left()
                + ((g.start - state.scroll_seconds) * state.pixels_per_second) as f32)
                .clamp(
                    lanes.left() - OFFSCREEN_CLAMP_PX,
                    lanes.right() + OFFSCREEN_CLAMP_PX,
                );
            let x1 = (lanes.left()
                + ((g.end() - state.scroll_seconds) * state.pixels_per_second) as f32)
                .clamp(
                    lanes.left() - OFFSCREEN_CLAMP_PX,
                    lanes.right() + OFFSCREEN_CLAMP_PX,
                );
            if x1 < lanes.left() || x0 > lanes.right() {
                continue;
            }
            let ev_rect =
                Rect::from_min_max(pos2(x0, row_y + 2.0), pos2(x1, row_y + TRACK_HEIGHT - 2.0));
            let selected = state.selected_event == Some((track.id, ev.id));
            lane_painter.rect_filled(ev_rect, Rounding::ZERO, base);
            lane_painter.rect_filled(
                Rect::from_min_size(ev_rect.min, vec2(ev_rect.width(), EVENT_HEADER_HEIGHT)),
                Rounding::ZERO,
                Color32::from_black_alpha(90),
            );
            lane_painter.text(
                ev_rect.min + vec2(4.0, 1.0),
                Align2::LEFT_TOP,
                &ev.name,
                FontId::proportional(10.0),
                Color32::from_gray(225),
            );

            if let Some(media) = ev.media_id.and_then(|id| state.media.get(&id)) {
                let wave_rect = Rect::from_min_max(
                    pos2(
                        ev_rect.left().max(lanes.left()),
                        ev_rect.top() + EVENT_HEADER_HEIGHT,
                    ),
                    pos2(ev_rect.right().min(lanes.right()), ev_rect.bottom()),
                );
                let visible_offset = (wave_rect.left() - x0) as f64 / state.pixels_per_second;
                let start_frame = (g.trim_in + visible_offset) * media.sample_rate as f64;
                let frames_per_pixel = media.sample_rate as f64 / state.pixels_per_second;
                paint_waveform(
                    &lane_painter,
                    wave_rect,
                    &media.waveform,
                    start_frame,
                    frames_per_pixel,
                    Color32::from_rgb(150, 215, 245),
                );
            }
            if x1 - x0 >= FADE_HANDLE_MIN_EVENT_PX {
                let pps = state.pixels_per_second;
                let wave_top = ev_rect.top() + EVENT_HEADER_HEIGHT;
                let height = ev_rect.bottom() - wave_top;
                let curve_stroke = Stroke::new(1.5_f32, Color32::from_white_alpha(210));
                if g.fade_in > 0.0 {
                    let width = (g.fade_in * pps) as f32;
                    let points: Vec<Pos2> = (0..=FADE_CURVE_STEPS)
                        .map(|k| {
                            let u = f64::from(k) / f64::from(FADE_CURVE_STEPS);
                            pos2(
                                x0 + u as f32 * width,
                                wave_top + (1.0 - g.fade_in_curve.gain(u) as f32) * height,
                            )
                        })
                        .collect();
                    lane_painter.add(Shape::line(points, curve_stroke));
                }
                if g.fade_out > 0.0 {
                    let width = (g.fade_out * pps) as f32;
                    let points: Vec<Pos2> = (0..=FADE_CURVE_STEPS)
                        .map(|k| {
                            let u = f64::from(k) / f64::from(FADE_CURVE_STEPS);
                            pos2(
                                x1 - width + u as f32 * width,
                                wave_top + (1.0 - g.fade_out_curve.gain(1.0 - u) as f32) * height,
                            )
                        })
                        .collect();
                    lane_painter.add(Shape::line(points, curve_stroke));
                }
                let (hin, hout) = fade_handle_x(g.fade_in, g.fade_out, pps, x0, x1);
                for hx in [hin, hout] {
                    lane_painter.rect_filled(
                        Rect::from_center_size(
                            pos2(hx, ev_rect.top() + EVENT_HEADER_HEIGHT / 2.0),
                            vec2(8.0, 8.0),
                        ),
                        Rounding::ZERO,
                        Color32::from_white_alpha(if selected { 240 } else { 130 }),
                    );
                }
            }
            let border = if selected {
                Color32::WHITE
            } else {
                Color32::from_gray(15)
            };
            lane_painter.rect_stroke(ev_rect, Rounding::ZERO, Stroke::new(1.0_f32, border));
        }
        if track.kind == TrackKind::Audio && state.project.settings.auto_crossfade {
            for cf in find_crossfades(&track.events) {
                let cx0 = lanes.left()
                    + ((cf.start - state.scroll_seconds) * state.pixels_per_second) as f32;
                let cx1 = lanes.left()
                    + ((cf.end - state.scroll_seconds) * state.pixels_per_second) as f32;
                if cx1 < lanes.left() || cx0 > lanes.right() {
                    continue;
                }
                let area = Rect::from_min_max(
                    pos2(cx0.max(lanes.left() - 1.0), y + 2.0),
                    pos2(cx1.min(lanes.right() + 1.0), y + TRACK_HEIGHT - 2.0),
                );
                lane_painter.rect_filled(area, Rounding::ZERO, Color32::from_white_alpha(28));
                let stroke = Stroke::new(1.5_f32, Color32::from_white_alpha(200));
                lane_painter.line_segment([area.left_bottom(), area.right_top()], stroke);
                lane_painter.line_segment([area.left_top(), area.right_bottom()], stroke);
            }
        }
    }

    // Live read-out while dragging: start and length of the event
    if let (Some(drag), Some(p)) = (&state.drag, ui.input(|i| i.pointer.hover_pos())) {
        let label = format!(
            "{}  ·  {:.3} s",
            seconds_to_timecode(drag.current.start, fps, drop_frame),
            drag.current.duration
        );
        let at = pos2(p.x + 14.0, p.y - 22.0);
        let galley_rect =
            Rect::from_min_size(at, vec2(label.chars().count() as f32 * 6.4 + 10.0, 16.0));
        painter.rect_filled(galley_rect, Rounding::ZERO, Color32::from_black_alpha(200));
        painter.text(
            galley_rect.left_center() + vec2(5.0, 0.0),
            Align2::LEFT_CENTER,
            label,
            FontId::monospace(10.0),
            Color32::WHITE,
        );
    }

    // Track headers
    let header_painter = painter.with_clip_rect(Rect::from_min_max(
        rect.min,
        pos2(lanes.left(), rect.bottom()),
    ));
    header_painter.rect_filled(
        Rect::from_min_max(rect.min, pos2(lanes.left(), rect.bottom())),
        Rounding::ZERO,
        Color32::from_rgb(35, 38, 45),
    );
    // Only the selected track's number-box dot blinks (white/black); the others stay static
    let blink_on = ((now / BLINK_SECONDS) as i64) % 2 == 0;
    for (i, track) in state.timeline.tracks.iter().enumerate() {
        let y = lane_top + i as f32 * TRACK_HEIGHT;
        let row = Rect::from_min_max(pos2(rect.left(), y), pos2(lanes.left(), y + TRACK_HEIGHT));
        let selected = state.selected_track == Some(track.id);
        if selected {
            header_painter.rect_filled(row, Rounding::ZERO, Color32::from_rgb(48, 54, 66));
        }
        header_painter.rect_stroke(
            row,
            Rounding::ZERO,
            Stroke::new(1.0_f32, Color32::from_gray(20)),
        );
        let badge = Rect::from_min_size(row.min + vec2(6.0, 6.0), vec2(34.0, 18.0));
        header_painter.rect_filled(badge, Rounding::ZERO, Color32::from_gray(20));
        let dot = if selected {
            if blink_on {
                Color32::WHITE
            } else {
                Color32::BLACK
            }
        } else {
            Color32::from_gray(90)
        };
        header_painter.text(
            badge.left_center() + vec2(5.0, 0.0),
            Align2::LEFT_CENTER,
            "•",
            FontId::proportional(12.0),
            dot,
        );
        header_painter.text(
            badge.left_center() + vec2(16.0, 0.0),
            Align2::LEFT_CENTER,
            (i + 1).to_string(),
            FontId::monospace(11.0),
            Color32::from_gray(210),
        );
        header_painter.text(
            row.min + vec2(46.0, 8.0),
            Align2::LEFT_TOP,
            &track.name,
            FontId::proportional(11.0),
            Color32::from_gray(200),
        );
    }
    if state.timeline.tracks.is_empty() {
        header_painter.text(
            rect.left_top() + vec2(8.0, RULER_HEIGHT + 8.0),
            Align2::LEFT_TOP,
            "Sem faixas\nCtrl+Q: áudio\nCtrl+Shift+Q: vídeo",
            FontId::proportional(11.0),
            Color32::from_gray(120),
        );
    }

    // Playhead: blue pentagonal head, 1 physical-pixel guide line, caret blink while paused
    let blue = Color32::from_rgb(0x29, 0x80, 0xB9);
    let bright = Color32::from_rgb(0x34, 0x98, 0xDB);
    let px = to_x(state.playhead);
    if px >= lanes.left() - 8.0 && px <= lanes.right() + 8.0 {
        let alpha =
            if state.playing || (((now - state.blink_origin) / BLINK_SECONDS) as i64) % 2 == 0 {
                255
            } else {
                0
            };
        let line_color = Color32::from_rgba_unmultiplied(bright.r(), bright.g(), bright.b(), alpha);
        lane_painter.line_segment(
            [pos2(px, rect.top() + 14.0), pos2(px, rect.bottom())],
            Stroke::new(hairline, line_color),
        );

        let top = rect.top();
        lane_painter.add(Shape::convex_polygon(
            vec![
                pos2(px - 6.0, top),
                pos2(px + 6.0, top),
                pos2(px + 6.0, top + 9.0),
                pos2(px, top + 16.0),
                pos2(px - 6.0, top + 9.0),
            ],
            blue,
            Stroke::new(1.0_f32, bright),
        ));
        let tc = seconds_to_timecode(state.playhead, fps, drop_frame);
        let flag = Rect::from_min_size(pos2(px + 6.0, top), vec2(78.0, 12.0));
        lane_painter.rect_filled(flag, Rounding::ZERO, blue);
        lane_painter.text(
            flag.center(),
            Align2::CENTER_CENTER,
            tc,
            FontId::monospace(10.0),
            Color32::WHITE,
        );
    }

    // Repaint for the blink: the selected track dot and the paused caret both toggle every 500 ms
    if state.selected_track.is_some() || !state.playing {
        ui.ctx().request_repaint_after(Duration::from_millis(500));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with_clip() -> (AppState, TrackId, EventId) {
        let mut s = AppState::new();
        s.add_track(TrackKind::Audio);
        let track_id = s.selected_track.unwrap();
        s.project.media_pool.push(MediaRef {
            id: 1,
            path: "x.wav".into(),
            duration_seconds: 10.0,
        });
        let id = add_event(&mut s, track_id, 2.0, 4.0);
        (s, track_id, id)
    }

    fn add_event(s: &mut AppState, track_id: TrackId, start: f64, duration: f64) -> EventId {
        let event = TrackEvent {
            id: s.timeline.alloc_id(),
            name: "a".into(),
            media_id: Some(1),
            start,
            duration,
            trim_in: 0.0,
            fade_in: 0.0,
            fade_out: 0.0,
            fade_in_curve: FadeCurve::default(),
            fade_out_curve: FadeCurve::default(),
        };
        let id = event.id;
        s.history
            .execute(&mut s.timeline, EditCommand::AddEvent { track_id, event });
        id
    }

    fn geometry(s: &AppState, track_id: TrackId, event_id: EventId) -> EventGeometry {
        s.timeline
            .track(track_id)
            .unwrap()
            .events
            .iter()
            .find(|e| e.id == event_id)
            .unwrap()
            .geometry()
    }

    fn hit(
        s: &AppState,
        track_id: TrackId,
        event_id: EventId,
        mode: DragMode,
    ) -> (TrackId, EventId, DragMode, EventGeometry) {
        (track_id, event_id, mode, geometry(s, track_id, event_id))
    }

    #[test]
    fn dragging_snaps_to_a_neighbour_edge_and_is_one_undo_step() {
        let (mut s, track, ev) = state_with_clip();
        add_event(&mut s, track, 10.0, 2.0);
        let before = geometry(&s, track, ev);

        s.begin_drag(hit(&s, track, ev, DragMode::Move), 3.0);
        // the event's end would land at 9.95: within the threshold of the neighbour starting at 10
        s.update_drag(6.95, Some(track), 0.1, true);
        s.finish_drag();

        assert_eq!(geometry(&s, track, ev).start, 6.0);
        assert!(s.is_dirty());
        s.history.undo(&mut s.timeline);
        assert_eq!(geometry(&s, track, ev), before);
        assert!(s.drag.is_none());
    }

    #[test]
    fn without_snapping_the_start_lands_on_a_frame() {
        let (mut s, track, ev) = state_with_clip();
        s.begin_drag(hit(&s, track, ev, DragMode::Move), 3.0);
        s.update_drag(7.0, Some(track), 0.1, false);
        s.finish_drag();
        let start = geometry(&s, track, ev).start;
        let frame = start * s.project.settings.fps;
        assert!((frame - frame.round()).abs() < 1e-9, "{start}");
        assert!((start - 6.0).abs() < 0.02);
    }

    #[test]
    fn trim_right_stops_at_the_end_of_the_source() {
        let (mut s, track, ev) = state_with_clip();
        s.begin_drag(hit(&s, track, ev, DragMode::TrimRight), 6.0);
        s.update_drag(500.0, None, 0.1, false);
        s.finish_drag();
        let g = geometry(&s, track, ev);
        // 10 s of source, none trimmed at the start
        assert_eq!((g.start, g.duration), (2.0, 10.0));
    }

    #[test]
    fn trim_left_shifts_the_source_and_keeps_the_end() {
        let (mut s, track, ev) = state_with_clip();
        s.begin_drag(hit(&s, track, ev, DragMode::TrimLeft), 2.0);
        s.update_drag(3.0, None, 0.1, false);
        s.finish_drag();
        let g = geometry(&s, track, ev);
        assert!((g.end() - 6.0).abs() < 1e-9);
        assert!((g.trim_in - (g.start - 2.0)).abs() < 1e-9);
        assert!(g.start > 2.9 && g.start < 3.1);
    }

    #[test]
    fn events_change_tracks_only_within_their_kind() {
        let (mut s, track, ev) = state_with_clip();
        s.add_track(TrackKind::Audio);
        let audio2 = s.selected_track.unwrap();
        s.add_track(TrackKind::Video);
        let video = s.selected_track.unwrap();

        s.begin_drag(hit(&s, track, ev, DragMode::Move), 3.0);
        s.update_drag(3.0, Some(video), 0.1, false);
        assert_eq!(s.drag.as_ref().unwrap().target_track, track);
        s.update_drag(3.0, Some(audio2), 0.1, false);
        s.finish_drag();

        assert!(s.timeline.track(track).unwrap().events.is_empty());
        assert_eq!(s.timeline.track(audio2).unwrap().events.len(), 1);
        assert_eq!(s.selected_track, Some(audio2));
        s.history.undo(&mut s.timeline);
        assert_eq!(s.timeline.track(track).unwrap().events.len(), 1);
    }

    #[test]
    fn hit_testing_finds_trim_zones_and_the_body() {
        let (mut s, track, ev) = state_with_clip();
        s.pixels_per_second = 100.0;
        s.scroll_seconds = 0.0;
        // event spans 2.0 s .. 6.0 s = 200 px .. 600 px
        let mode_at = |x: f32, y: f32| hit_event(&s, 0.0, 0.0, pos2(x, y)).map(|h| (h.0, h.1, h.2));
        assert_eq!(mode_at(203.0, 40.0), Some((track, ev, DragMode::TrimLeft)));
        assert_eq!(mode_at(598.0, 40.0), Some((track, ev, DragMode::TrimRight)));
        assert_eq!(mode_at(400.0, 40.0), Some((track, ev, DragMode::Move)));
        assert_eq!(mode_at(400.0, 10.0), Some((track, ev, DragMode::Move)));
        // header strip: handles sit 8 px inside each edge while the fades are zero
        assert_eq!(mode_at(208.0, 10.0), Some((track, ev, DragMode::FadeIn)));
        assert_eq!(mode_at(592.0, 10.0), Some((track, ev, DragMode::FadeOut)));
        assert_eq!(mode_at(100.0, 10.0), None);
        assert_eq!(mode_at(400.0, TRACK_HEIGHT + 10.0), None);
        assert_eq!(mode_at(400.0, -5.0), None);
    }

    #[test]
    fn candidates_exclude_the_dragged_event() {
        let (mut s, track, ev) = state_with_clip();
        add_event(&mut s, track, 10.0, 2.0);
        s.playhead = 7.5;
        let c = s.snap_candidates((track, ev));
        assert!(c.contains(&7.5) && c.contains(&10.0) && c.contains(&12.0) && c.contains(&0.0));
        assert!(!c.contains(&2.0) && !c.contains(&6.0));
    }

    #[test]
    fn a_cancelled_drag_changes_nothing() {
        let (mut s, track, ev) = state_with_clip();
        let before = geometry(&s, track, ev);
        let revision = s.history.revision();
        s.begin_drag(hit(&s, track, ev, DragMode::Move), 3.0);
        s.update_drag(9.0, Some(track), 0.1, false);
        s.drag = None; // Esc
        s.finish_drag();
        assert_eq!(geometry(&s, track, ev), before);
        assert_eq!(s.history.revision(), revision);
    }

    #[test]
    fn ages_are_human_readable() {
        assert_eq!(format_age(100, 130), "agora há pouco");
        assert_eq!(format_age(0, 300), "há 5 min");
        assert_eq!(format_age(0, 7_300), "há 2 h");
        assert_eq!(format_age(0, 3 * 86_400 + 5), "há 3 dia(s)");
        assert_eq!(format_age(500, 100), "agora há pouco");
    }

    #[test]
    fn dragging_a_fade_handle_sets_the_fade_in_one_undo_step() {
        let (mut s, track, ev) = state_with_clip();
        s.begin_drag(hit(&s, track, ev, DragMode::FadeIn), 2.2);
        s.update_drag(3.0, None, 0.1, true);
        s.finish_drag();
        let g = geometry(&s, track, ev);
        assert!((g.fade_in - 1.0).abs() < 0.02, "{}", g.fade_in);
        assert_eq!(g.start, 2.0);

        s.begin_drag(hit(&s, track, ev, DragMode::FadeOut), 5.8);
        s.update_drag(1000.0, None, 0.1, true);
        // clamped at zero when dragged past the event's end
        s.finish_drag();
        assert_eq!(geometry(&s, track, ev).fade_out, 0.0);

        s.history.undo(&mut s.timeline);
        assert_eq!(geometry(&s, track, ev).fade_in, 0.0);
    }

    #[test]
    fn curve_changes_are_undoable_and_leave_lengths_alone() {
        let (mut s, track, ev) = state_with_clip();
        let g = geometry(&s, track, ev);
        s.apply_geometry(
            track,
            ev,
            EventGeometry {
                fade_in: 1.0,
                fade_in_curve: FadeCurve::Smooth,
                ..g
            },
        );
        let after = geometry(&s, track, ev);
        assert_eq!(after.fade_in_curve, FadeCurve::Smooth);
        assert_eq!(after.fade_in, 1.0);
        s.history.undo(&mut s.timeline);
        assert_eq!(geometry(&s, track, ev), g);
        // applying an identical geometry is not an edit
        let revision = s.history.revision();
        s.apply_geometry(track, ev, g);
        assert_eq!(s.history.revision(), revision);
    }

    #[test]
    fn mixer_clips_get_crossfades_only_when_enabled() {
        let (mut s, track, _) = state_with_clip(); // event 2..6
        add_event(&mut s, track, 5.0, 4.0); // overlaps 5..6
        s.media.insert(1, test_media());
        let clips = s.build_clips();
        assert_eq!(clips.len(), 2);
        assert!((clips[0].cross_out - 1.0).abs() < 1e-9 && clips[0].cross_in == 0.0);
        assert!((clips[1].cross_in - 1.0).abs() < 1e-9 && clips[1].cross_out == 0.0);

        s.project.settings.auto_crossfade = false;
        let clips = s.build_clips();
        assert!(clips
            .iter()
            .all(|c| c.cross_in == 0.0 && c.cross_out == 0.0));
    }

    fn test_media() -> LoadedMedia {
        let samples = vec![0.0f32; 200];
        LoadedMedia {
            path: PathBuf::from("x.wav"),
            name: "x.wav".into(),
            waveform: Arc::new(WaveformSource::from_interleaved_f32(100, 2, &samples).unwrap()),
            stereo: Arc::new(samples),
            sample_rate: 100,
            duration: 10.0,
        }
    }
}
