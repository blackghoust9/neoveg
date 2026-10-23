//! Main application: dockable workspace (Preview, Timeline, Project Media, Trimmer, Master Bus).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, FontId, Key, Modifiers, Rect, Rounding, Sense, Shape,
    Stroke, TextureHandle,
};
use egui_dock::{DockArea, DockState, NodeIndex, Style, TabViewer};
use neoveg_audio::decode::decode_file;
use neoveg_audio::meter::{linear_to_dbfs, PeakMeter};
use neoveg_audio::mixer::MixClip;
use neoveg_audio::playback::AudioPlayer;
use neoveg_audio::waveform::WaveformSource;
use neoveg_common::paths::autosave_dir;
use neoveg_common::preferences::Preferences;
use neoveg_common::project::{MediaRef, ProjectInfo};
use neoveg_common::timecode::{is_drop_frame_rate, seconds_to_timecode};
use neoveg_engine::autosave::{self, RecoveryEntry};
use neoveg_engine::project_file;
use neoveg_engine::relink::relink_missing;
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

    /// Every unmuted audio event with decoded media, ready for the mixer.
    fn build_clips(&self) -> Vec<MixClip> {
        self.timeline
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Audio && !t.muted)
            .flat_map(|t| t.events.iter())
            .filter_map(|e| {
                let media = self.media.get(&e.media_id?)?;
                Some(MixClip {
                    samples: Arc::clone(&media.stereo),
                    source_rate: media.sample_rate,
                    start: e.start,
                    duration: e.duration,
                    trim_in: e.trim_in,
                    gain: 1.0,
                    fade_in: e.fade_in,
                    fade_out: e.fade_out,
                })
            })
            .collect()
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
    egui::Window::new("Inspetor")
        .open(&mut open)
        .show(ctx, |ui| {
            let selected = state.selected_event.and_then(|(track_id, event_id)| {
                state
                    .timeline
                    .track(track_id)?
                    .events
                    .iter()
                    .find(|e| e.id == event_id)
            });
            match selected {
                Some(e) => {
                    ui.label(format!("Evento: {}", e.name));
                    ui.label(format!("Início: {:.3} s", e.start));
                    ui.label(format!("Duração: {:.3} s", e.duration));
                    ui.label(format!("Corte de entrada: {:.3} s", e.trim_in));
                    ui.label(format!(
                        "Fade-in/out: {:.3} s / {:.3} s",
                        e.fade_in, e.fade_out
                    ));
                }
                None => {
                    ui.label("Nenhum evento selecionado");
                }
            }
        });
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

    // ---- drawing ---------------------------------------------------------------------------------
    let painter = ui.painter_at(rect);
    let lane_painter = painter.with_clip_rect(lanes);
    painter.rect_filled(rect, Rounding::ZERO, Color32::from_rgb(26, 28, 33));

    let lane_top = rect.top() + RULER_HEIGHT;
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
            let x0 = (lanes.left()
                + ((ev.start - state.scroll_seconds) * state.pixels_per_second) as f32)
                .clamp(
                    lanes.left() - OFFSCREEN_CLAMP_PX,
                    lanes.right() + OFFSCREEN_CLAMP_PX,
                );
            let x1 = (lanes.left()
                + ((ev.end() - state.scroll_seconds) * state.pixels_per_second) as f32)
                .clamp(
                    lanes.left() - OFFSCREEN_CLAMP_PX,
                    lanes.right() + OFFSCREEN_CLAMP_PX,
                );
            if x1 < lanes.left() || x0 > lanes.right() {
                continue;
            }
            let ev_rect = Rect::from_min_max(pos2(x0, y + 2.0), pos2(x1, y + TRACK_HEIGHT - 2.0));
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
                let start_frame = (ev.trim_in + visible_offset) * media.sample_rate as f64;
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
            let border = if selected {
                Color32::WHITE
            } else {
                Color32::from_gray(15)
            };
            lane_painter.rect_stroke(ev_rect, Rounding::ZERO, Stroke::new(1.0_f32, border));
        }
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
    use super::format_age;

    #[test]
    fn ages_are_human_readable() {
        assert_eq!(format_age(100, 130), "agora há pouco");
        assert_eq!(format_age(0, 300), "há 5 min");
        assert_eq!(format_age(0, 7_300), "há 2 h");
        assert_eq!(format_age(0, 3 * 86_400 + 5), "há 3 dia(s)");
        assert_eq!(format_age(500, 100), "agora há pouco");
    }
}
