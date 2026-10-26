//! Main application: dockable workspace (Preview, Timeline, Project Media, Trimmer, Master Bus).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, FontId, Key, Pos2, Rect, Rounding, Sense, Shape, Stroke,
    TextureHandle,
};
use egui_dock::{DockArea, DockState, NodeIndex, Style, TabViewer};
use neoveg_audio::decode::decode_file;
use neoveg_audio::meter::{linear_to_dbfs, PeakMeter};
use neoveg_audio::mixer::MixClip;
use neoveg_audio::playback::{wrap_loop, AudioPlayer};
use neoveg_audio::waveform::WaveformSource;
use neoveg_common::commands::{Category, Command};
use neoveg_common::fade::FadeCurve;
use neoveg_common::keymap::{KeyMap, Shortcut};
use neoveg_common::paths::autosave_dir;
use neoveg_common::preferences::Preferences;
use neoveg_common::project::{MediaRef, ProjectInfo};
use neoveg_common::timecode::{is_drop_frame_rate, seconds_to_timecode};
use neoveg_engine::autosave::{self, RecoveryEntry};
use neoveg_engine::crossfade::{find_crossfades, Crossfade};
use neoveg_engine::edit;
use neoveg_engine::markers::{self, MarkId, MarkPart};
use neoveg_engine::ops::{self, Clipboard, PasteMode, RepeatSpacing, Scope};
use neoveg_engine::project_file;
use neoveg_engine::relink::relink_missing;
use neoveg_engine::timeline::TIME_EPSILON;
use neoveg_engine::EventGeometry;
use neoveg_engine::{
    EditCommand, EventId, EventRef, History, Selection, TimeRange, Timeline, TrackEvent, TrackId,
    TrackKind,
};
use neoveg_video::info::{is_video_path, VideoInfo};
use neoveg_video::worker::{probe, FrameRequest, FrameWorker};

use crate::frame_grid::{snap_to_device_pixel, FrameGrid};
use crate::precision_slider::PrecisionSlider;
use crate::shortcuts::{from_egui, to_egui};
use crate::waveform_paint::paint_waveform;

const HEADER_WIDTH: f32 = 150.0;
const RULER_HEIGHT: f32 = 24.0;
/// Height of the marker / region strip at the bottom of the ruler.
const MARK_STRIP_HEIGHT: f32 = 10.0;
/// How close (in pixels) a right-click must be to a marker to pick it.
const MARK_HIT_PX: f32 = 6.0;
/// Width of the inline name box on the ruler.
const NAME_BOX_WIDTH: f32 = 150.0;
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

/// Options of the "Repetir colar" dialog.
struct RepeatDialog {
    count: u32,
    spaced: bool,
    seconds: f64,
}

struct AppState {
    project: ProjectInfo,
    timeline: Timeline,
    history: History,
    prefs: Preferences,
    keymap: KeyMap,
    media: HashMap<u64, LoadedMedia>,

    playhead: f64,
    playing: bool,
    blink_origin: f64,
    pixels_per_second: f64,
    scroll_seconds: f64,
    selected_track: Option<TrackId>,
    selection: Selection,
    /// Range chosen by dragging on empty lane space or with `I` / `O`.
    time_selection: Option<TimeRange>,
    range_anchor: Option<f64>,
    clipboard: Option<Clipboard>,
    repeat_dialog: Option<RepeatDialog>,

    meter: Arc<PeakMeter>,
    master_volume: f32,
    active_player: Option<ActivePlayer>,

    show_bars: bool,
    bars_texture: Option<TextureHandle>,
    quality: usize,
    show_preferences: bool,
    show_keymap: bool,
    keymap_filter: String,
    /// Command waiting for the next key press in the "Personalizar teclado" window.
    capture_for: Option<Command>,
    keymap_message: String,
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
    /// Marker or region under the pointer when the ruler's context menu was opened.
    context_mark: Option<MarkId>,
    name_edit: Option<NameEdit>,
    mark_drag: Option<MarkDrag>,
    snapping: bool,
    /// `Q`: playback repeats over [`AppState::loop_args`].
    looping: bool,
    /// `Shift+Q`: the span that loops; without one the time selection (or the whole project) does.
    loop_region: Option<TimeRange>,
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
        let (keymap, warnings) = prefs.key_map();
        for w in warnings {
            tracing::warn!("[WARN] keymap: {w}");
        }
        let (loader_tx, loader_rx) = channel();
        Self {
            project: ProjectInfo::create_empty(),
            timeline: Timeline::new(),
            history: History::new(),
            prefs,
            keymap,
            media: HashMap::new(),
            playhead: 0.0,
            playing: false,
            blink_origin: 0.0,
            pixels_per_second: 100.0,
            scroll_seconds: 0.0,
            selected_track: None,
            selection: Selection::new(),
            time_selection: None,
            range_anchor: None,
            clipboard: None,
            repeat_dialog: None,
            meter: Arc::new(PeakMeter::new()),
            master_volume: 1.0,
            active_player: None,
            show_bars: false,
            bars_texture: None,
            quality: 4,
            show_preferences: false,
            show_keymap: false,
            keymap_filter: String::new(),
            capture_for: None,
            keymap_message: String::new(),
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
            context_mark: None,
            name_edit: None,
            mark_drag: None,
            snapping: true,
            looping: false,
            loop_region: None,
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
        self.keymap = prefs.key_map().0;
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

    // ---- selection, clipboard and compound edits ---------------------------------------------------

    /// Runs an operation from `neoveg_engine::ops` as one undo step. Returns the outcome's
    /// `touched` events when something changed.
    fn apply_outcome(&mut self, outcome: Option<ops::Outcome>) -> Option<Vec<EventRef>> {
        let outcome = outcome?;
        let touched = outcome.touched.clone();
        self.history
            .execute(&mut self.timeline, outcome.command)
            .then(|| {
                self.selection.retain_existing(&self.timeline);
                touched
            })
    }

    /// Events the next trim/split acts on: the selection, or the event under the cursor on the
    /// selected track.
    fn trim_targets(&self) -> Vec<EventRef> {
        let refs = self.selection.refs();
        if !refs.is_empty() {
            return refs;
        }
        self.selected_track
            .and_then(|t| self.timeline.event_at(t, self.playhead).map(|e| (t, e.id)))
            .into_iter()
            .collect()
    }

    fn split_at_playhead(&mut self) {
        let refs = self.selection.refs();
        let scope = if refs.is_empty() {
            Scope::AllTracks
        } else {
            Scope::Events(&refs)
        };
        let outcome = match self.time_selection {
            Some(range) => ops::split_range(&mut self.timeline, range, scope),
            None => ops::split_at(&mut self.timeline, self.playhead, scope),
        };
        self.apply_outcome(outcome);
    }

    /// Delete: selected events, or the time selection (all tracks, or only the selected events).
    fn delete_selection(&mut self) {
        let refs = self.selection.refs();
        let outcome = match (self.time_selection, refs.is_empty()) {
            (Some(range), false) => {
                ops::remove_range(&mut self.timeline, range, Scope::Events(&refs), false)
            }
            (Some(range), true) => {
                ops::remove_range(&mut self.timeline, range, Scope::AllTracks, false)
            }
            (None, false) => ops::delete_events(&self.timeline, &refs),
            (None, true) => None,
        };
        self.apply_outcome(outcome);
    }

    /// Copies the time selection (or the selected events when there is none) to the clipboard.
    fn copy_selection(&mut self) -> bool {
        let refs = self.selection.refs();
        let clip = match self.time_selection {
            Some(range) => {
                let scope = if refs.is_empty() {
                    Scope::AllTracks
                } else {
                    Scope::Events(&refs)
                };
                ops::copy_range(&self.timeline, range, scope)
            }
            None if !refs.is_empty() => ops::copy_events(&self.timeline, &refs),
            None => None,
        };
        match clip {
            Some(clip) => {
                self.status = format!("{} evento(s) copiado(s)", clip.items.len());
                self.clipboard = Some(clip);
                true
            }
            None => false,
        }
    }

    fn cut_selection(&mut self) {
        if self.copy_selection() {
            self.delete_selection();
        }
    }

    /// Index of the track where a paste starts: the selected track, else the first one.
    fn paste_base_track(&self) -> usize {
        self.selected_track
            .and_then(|id| self.timeline.tracks.iter().position(|t| t.id == id))
            .unwrap_or(0)
    }

    fn report_paste(&mut self, result: Option<(ops::Outcome, usize)>) {
        let Some((outcome, skipped)) = result else {
            self.status = "Nada para colar nesta posição".into();
            return;
        };
        if let Some(touched) = self.apply_outcome(Some(outcome)) {
            self.status = if skipped == 0 {
                format!("{} evento(s) colado(s)", touched.len())
            } else {
                format!(
                    "{} evento(s) colado(s); {skipped} sem faixa do mesmo tipo",
                    touched.len()
                )
            };
            self.selection.set(touched);
        }
    }

    fn paste_clipboard(&mut self, mode: PasteMode) {
        let Some(clip) = self.clipboard.clone() else {
            self.status = "A área de transferência está vazia".into();
            return;
        };
        let base = self.paste_base_track();
        let result = ops::paste(&mut self.timeline, &clip, self.playhead, base, mode);
        self.report_paste(result);
    }

    fn paste_repeated(&mut self, count: usize, spacing: RepeatSpacing) {
        let Some(clip) = self.clipboard.clone() else {
            self.status = "A área de transferência está vazia".into();
            return;
        };
        let base = self.paste_base_track();
        let result = ops::paste_repeat(
            &mut self.timeline,
            &clip,
            self.playhead,
            base,
            count,
            spacing,
        );
        self.report_paste(result);
    }

    fn trim_to_time_selection(&mut self) {
        let Some(range) = self.time_selection else {
            return;
        };
        let mut targets = self.selection.refs();
        if targets.is_empty() {
            targets = self
                .timeline
                .tracks
                .iter()
                .flat_map(|t| t.events.iter().map(move |e| (t.id, e.id)))
                .collect();
        }
        let outcome = ops::trim_to_range(&self.timeline, &targets, range);
        self.apply_outcome(outcome);
    }

    fn trim_edge_to_playhead(&mut self, start: bool) {
        let targets = self.trim_targets();
        let outcome = if start {
            ops::trim_start_to(&self.timeline, &targets, self.playhead)
        } else {
            ops::trim_end_to(&self.timeline, &targets, self.playhead)
        };
        self.apply_outcome(outcome);
    }

    /// `I` / `O`: moves one end of the time selection to the cursor (at least one frame long).
    fn set_selection_edge(&mut self, start: bool) {
        let frame = 1.0 / self.project.settings.fps;
        let t = self.playhead;
        let range = match (self.time_selection, start) {
            (Some(r), true) => TimeRange::new(t, r.end.max(t + frame)),
            (Some(r), false) => TimeRange::new(r.start.min((t - frame).max(0.0)), t),
            (None, true) => TimeRange::new(t, t + frame),
            (None, false) => TimeRange::new((t - frame).max(0.0), t),
        };
        self.time_selection = (!range.is_empty()).then_some(range);
    }

    // ---- markers and regions -----------------------------------------------------------------

    /// `M`: marker at the cursor.
    fn insert_marker(&mut self) {
        let time = self.playhead;
        if self
            .timeline
            .markers
            .iter()
            .any(|m| (m.time - time).abs() <= TIME_EPSILON)
        {
            self.status = "Já existe um marcador neste ponto".into();
            return;
        }
        self.begin_name_edit(NameTarget::NewMarker(time));
    }

    /// `R`: region over the time selection.
    fn insert_region(&mut self) {
        let Some(range) = self.time_selection else {
            self.status = "Marque um intervalo (I / O) para criar uma região".into();
            return;
        };
        if !range.is_empty() {
            self.begin_name_edit(NameTarget::NewRegion(range));
        }
    }

    /// Opens the inline name box. A new mark is only created when the name is confirmed.
    fn begin_name_edit(&mut self, target: NameTarget) {
        let text = match &target {
            NameTarget::Existing(id) => self
                .timeline
                .markers
                .iter()
                .find(|m| m.id == *id)
                .map(|m| m.name.clone())
                .or_else(|| {
                    self.timeline
                        .regions
                        .iter()
                        .find(|r| r.id == *id)
                        .map(|r| r.name.clone())
                })
                .unwrap_or_default(),
            _ => String::new(),
        };
        self.name_edit = Some(NameEdit {
            target,
            text,
            focused: false,
        });
    }

    /// Enter / focus lost: creates the mark (a blank name keeps the default one) or renames it.
    fn commit_name_edit(&mut self) {
        let Some(edit) = self.name_edit.take() else {
            return;
        };
        let name = edit.text.trim();
        let name = (!name.is_empty()).then_some(name);
        let (command, done) = match edit.target {
            NameTarget::NewMarker(time) => (
                markers::add_marker(&mut self.timeline, time, name),
                "Marcador inserido",
            ),
            NameTarget::NewRegion(range) => (
                markers::add_region(&mut self.timeline, range, name),
                "Região inserida",
            ),
            NameTarget::Existing(id) => (
                name.and_then(|name| markers::rename_mark(&self.timeline, id, name)),
                "Nome alterado",
            ),
        };
        if let Some(command) = command {
            self.history.execute(&mut self.timeline, command);
            self.status = done.into();
        }
    }

    /// Esc: nothing is created or renamed.
    fn cancel_name_edit(&mut self) {
        self.name_edit = None;
    }

    /// Grabs the marker or region under `time` (within `tolerance` seconds). `false` when none.
    fn begin_mark_drag(&mut self, time: f64, tolerance: f64) -> bool {
        let Some((id, part)) = markers::hit_test(&self.timeline, time, tolerance) else {
            return false;
        };
        let Some(original) = markers::span_of(&self.timeline, id) else {
            return false;
        };
        self.mark_drag = Some(MarkDrag {
            id,
            part,
            original,
            grab_offset: time - markers::part_anchor(part, original),
            current: original,
        });
        true
    }

    fn update_mark_drag(&mut self, pointer: f64) {
        let fps = self.project.settings.fps;
        if let Some(d) = &mut self.mark_drag {
            d.current = markers::drag_span(d.part, d.original, d.grab_offset, pointer, fps);
        }
    }

    /// Release: one undo step for the whole drag.
    fn finish_mark_drag(&mut self) {
        let Some(d) = self.mark_drag.take() else {
            return;
        };
        if let Some(command) =
            markers::set_mark_span(&self.timeline, d.id, d.current.0, d.current.1)
        {
            self.history.execute(&mut self.timeline, command);
        }
    }

    /// Where a mark is drawn: its dragged position while it is being dragged.
    fn shown_span(&self, id: MarkId, stored: (f64, f64)) -> (f64, f64) {
        match &self.mark_drag {
            Some(d) if d.id == id => d.current,
            _ => stored,
        }
    }

    /// `Ctrl+←` / `Ctrl+→`: cursor to the previous / next marker or region edge.
    fn jump_to_mark(&mut self, forward: bool, now: f64) {
        let target = if forward {
            markers::next_stop(&self.timeline, self.playhead)
        } else {
            markers::previous_stop(&self.timeline, self.playhead)
        };
        match target {
            Some(t) => {
                self.playhead = t;
                self.blink_origin = now;
            }
            None => {
                self.status = if forward {
                    "Não há marcador adiante".into()
                } else {
                    "Não há marcador antes".into()
                };
            }
        }
    }

    fn remove_mark(&mut self, id: MarkId) {
        if let Some(command) = markers::remove_mark(&self.timeline, id) {
            self.history.execute(&mut self.timeline, command);
        }
    }

    /// Clears the markers and regions inside the time selection, or all of them without one.
    fn clear_marks(&mut self) {
        if let Some(command) = markers::clear_marks(&self.timeline, self.time_selection) {
            self.history.execute(&mut self.timeline, command);
        }
    }

    /// Marker or region under `time`, within `tolerance` seconds of a marker or inside a region.
    fn mark_at(&self, time: f64, tolerance: f64) -> Option<MarkId> {
        let marker = self
            .timeline
            .markers
            .iter()
            .find(|m| (m.time - time).abs() <= tolerance)
            .map(|m| m.id);
        marker.or_else(|| {
            self.timeline
                .regions
                .iter()
                .find(|r| time >= r.start && time <= r.end)
                .map(|r| r.id)
        })
    }

    // ---- loop playback -----------------------------------------------------------------------

    /// What loops: the loop region, else the time selection, else the whole project.
    fn loop_bounds(&self) -> Option<TimeRange> {
        let explicit = self
            .loop_region
            .or(self.time_selection)
            .filter(|r| !r.is_empty());
        explicit.or_else(|| {
            let duration = self.timeline.duration();
            (duration > 0.0).then(|| TimeRange::new(0.0, duration))
        })
    }

    /// Loop start and end handed to the player, `None` while loop playback is off.
    fn loop_args(&self) -> Option<(f64, f64)> {
        if !self.looping {
            return None;
        }
        self.loop_bounds().map(|r| (r.start, r.end))
    }

    fn sync_loop(&self) {
        if let Some(active) = &self.active_player {
            active.player.set_loop(self.loop_args());
        }
    }

    /// `Q`: loop playback on / off.
    fn toggle_loop(&mut self) {
        self.looping = !self.looping;
        self.sync_loop();
        self.status = if self.looping {
            "Reprodução em laço ligada".into()
        } else {
            "Reprodução em laço desligada".into()
        };
    }

    /// `Shift+Q`: the time selection (else the region under the cursor) becomes the loop region;
    /// with neither, the loop region is cleared.
    fn set_loop_region(&mut self) {
        let under_cursor = self
            .timeline
            .regions
            .iter()
            .find(|r| self.playhead >= r.start && self.playhead < r.end)
            .map(|r| r.range());
        self.loop_region = self
            .time_selection
            .filter(|r| !r.is_empty())
            .or(under_cursor);
        self.sync_loop();
        self.status = if self.loop_region.is_some() {
            "Região de laço definida".into()
        } else {
            "Região de laço removida".into()
        };
    }

    fn select_all_events(&mut self) {
        self.selection.select_all(&self.timeline);
    }

    fn clear_selection(&mut self) {
        self.selection.clear();
        self.time_selection = None;
    }

    /// Whether `command` can do something right now (greys out menu items).
    fn command_enabled(&self, command: Command) -> bool {
        let has_events = !self.selection.is_empty();
        let has_range = self.time_selection.is_some();
        match command {
            Command::EditUndo => self.history.can_undo(),
            Command::EditRedo => self.history.can_redo(),
            Command::EditCut
            | Command::EditCopy
            | Command::EditDelete
            | Command::EditSelectNone => has_events || has_range,
            Command::EditPaste | Command::EditPasteInsert | Command::EditPasteRepeat => {
                self.clipboard.is_some()
            }
            Command::EditTrimToSelection => has_range,
            _ => true,
        }
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
        self.selection.clear();
        self.time_selection = None;
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
        if !self.selection.contains((track_id, event_id)) {
            self.selection.select_only((track_id, event_id));
        }
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
        self.selection.select_only((d.target_track, d.event_id));
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
        // A loop starts from its own start when the cursor lies outside it
        if let Some((start, end)) = self.loop_args() {
            if self.playhead < start || self.playhead >= end {
                self.playhead = start;
            }
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
                player.set_loop(self.loop_args());
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
                if let Some((start, end)) = self.loop_args() {
                    self.playhead = wrap_loop(self.playhead, start, end);
                }
                false
            }
        };
        let finished = ended || self.playhead >= self.timeline.duration();
        if finished && self.loop_args().is_none() {
            self.stop();
        }
        ctx.request_repaint();
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() || self.capture_for.is_some() {
            return;
        }
        let now = ctx.input(|i| i.time);
        // Bindings with more modifiers are consumed first so Ctrl+Shift+Q is not read as Ctrl+Q
        let pressed: Vec<Command> = ctx.input_mut(|i| {
            self.keymap
                .dispatch_order()
                .into_iter()
                .filter(|(_, command)| command.implemented())
                .filter_map(|(shortcut, command)| {
                    let (modifiers, key) = to_egui(shortcut)?;
                    i.consume_key(modifiers, key).then_some(command)
                })
                .collect()
        });
        for command in pressed {
            self.run_command(command, ctx, now);
        }
    }

    /// Runs a command. Commands that are not implemented yet do nothing.
    fn run_command(&mut self, command: Command, ctx: &egui::Context, now: f64) {
        match command {
            Command::FileOpen => self.open_project(ctx),
            Command::FileSave => self.save_project(false),
            Command::FileSaveAs => self.save_project(true),
            Command::EditUndo => {
                self.history.undo(&mut self.timeline);
            }
            Command::EditRedo => {
                self.history.redo(&mut self.timeline);
            }
            Command::EditDelete => self.delete_selection(),
            Command::EditSplit => self.split_at_playhead(),
            Command::InsertAudioTrack => self.add_track(TrackKind::Audio),
            Command::InsertVideoTrack => self.add_track(TrackKind::Video),
            Command::CursorLeftByFrame => self.step_frames(-1.0, now),
            Command::CursorRightByFrame => self.step_frames(1.0, now),
            Command::TransportPlayToggle => self.toggle_play(now),
            Command::OptionsSnapping => {
                self.snapping = !self.snapping;
                self.status = if self.snapping {
                    "Alinhamento ligado".into()
                } else {
                    "Alinhamento desligado".into()
                };
            }
            Command::OptionsAutoCrossfade => {
                self.project.settings.auto_crossfade = !self.project.settings.auto_crossfade;
                self.forced_dirty = true;
            }
            Command::EditCut => self.cut_selection(),
            Command::EditCopy => {
                self.copy_selection();
            }
            Command::EditPaste => self.paste_clipboard(PasteMode::Overlay),
            Command::EditPasteInsert => self.paste_clipboard(PasteMode::Insert),
            Command::EditPasteRepeat => {
                if self.clipboard.is_some() {
                    self.repeat_dialog = Some(RepeatDialog {
                        count: 2,
                        spaced: false,
                        seconds: 1.0,
                    });
                } else {
                    self.status = "A área de transferência está vazia".into();
                }
            }
            Command::EditTrimToSelection => self.trim_to_time_selection(),
            Command::EditTrimStart => self.trim_edge_to_playhead(true),
            Command::EditTrimEnd => self.trim_edge_to_playhead(false),
            Command::EditSelectAll => self.select_all_events(),
            Command::EditSelectNone => self.clear_selection(),
            Command::SelectionSetIn => self.set_selection_edge(true),
            Command::SelectionSetOut => self.set_selection_edge(false),
            Command::TransportLoop => self.toggle_loop(),
            Command::SelectionLoopRegion => self.set_loop_region(),
            Command::InsertMarker => self.insert_marker(),
            Command::InsertRegion => self.insert_region(),
            Command::CursorPreviousMarker => self.jump_to_mark(false, now),
            Command::CursorNextMarker => self.jump_to_mark(true, now),
            _ => {}
        }
    }

    /// Moves the cursor by whole frames; keyboard navigation restarts the caret blink.
    fn step_frames(&mut self, frames: f64, now: f64) {
        let fps = self.project.settings.fps;
        let frame = (self.playhead * fps).round() + frames;
        self.playhead = (frame / fps).max(0.0);
        self.blink_origin = now;
    }

    /// Menu label followed by the first shortcut of `command`.
    fn with_hint(&self, text: &str, command: Command) -> String {
        match self.keymap.shortcuts_for(command).first() {
            Some(shortcut) => format!("{text}  {shortcut}"),
            None => text.to_string(),
        }
    }

    /// Saves the user's keyboard changes to the preferences file.
    fn persist_keymap(&mut self) {
        self.prefs.keymap = self.keymap.to_overrides();
        if let Some(path) = Preferences::default_path() {
            if let Err(e) = self.prefs.save(&path) {
                tracing::warn!("[WARN] cannot save preferences: {e}");
            }
        }
    }
}

// ============================================================================================
// Application
// ============================================================================================

/// What the inline name box on the ruler is naming.
#[derive(Debug, Clone, PartialEq)]
enum NameTarget {
    NewMarker(f64),
    NewRegion(TimeRange),
    Existing(MarkId),
}

/// The inline name box: it appears on the ruler at the mark and commits on Enter or when it loses
/// focus; Esc cancels.
struct NameEdit {
    target: NameTarget,
    text: String,
    focused: bool,
}

/// A marker or region being dragged along the ruler; the timeline changes only on release.
struct MarkDrag {
    id: MarkId,
    part: MarkPart,
    original: (f64, f64),
    /// Seconds between the pointer and the grabbed anchor when the drag began.
    grab_offset: f64,
    current: (f64, f64),
}

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
        self.state.selection.retain_existing(&self.state.timeline);
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
        keymap_window(ctx, &mut self.state);
        repeat_paste_window(ctx, &mut self.state);
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
            if ui
                .button(state.with_hint("Abrir…", Command::FileOpen))
                .clicked()
            {
                let ctx = ui.ctx().clone();
                state.open_project(&ctx);
                ui.close_menu();
            }
            if ui
                .button(state.with_hint("Salvar", Command::FileSave))
                .clicked()
            {
                state.save_project(false);
                ui.close_menu();
            }
            if ui
                .button(state.with_hint("Salvar como…", Command::FileSaveAs))
                .clicked()
            {
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
                    egui::Button::new(state.with_hint("Desfazer", Command::EditUndo)),
                )
                .clicked()
            {
                state.history.undo(&mut state.timeline);
                ui.close_menu();
            }
            if ui
                .add_enabled(
                    state.history.can_redo(),
                    egui::Button::new(state.with_hint("Refazer", Command::EditRedo)),
                )
                .clicked()
            {
                state.history.redo(&mut state.timeline);
                ui.close_menu();
            }
            ui.separator();
            for command in [
                Command::EditCut,
                Command::EditCopy,
                Command::EditPaste,
                Command::EditPasteInsert,
                Command::EditPasteRepeat,
                Command::EditDelete,
            ] {
                command_item(state, ui, command);
            }
            ui.separator();
            for command in [
                Command::EditSplit,
                Command::EditTrimToSelection,
                Command::EditTrimStart,
                Command::EditTrimEnd,
            ] {
                command_item(state, ui, command);
            }
            ui.separator();
            command_item(state, ui, Command::EditSelectAll);
            command_item(state, ui, Command::EditSelectNone);
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
            if ui.button("Personalizar teclado…").clicked() {
                state.show_keymap = true;
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

/// A menu entry that runs `command`, with its shortcut and enabled state.
fn command_item(state: &mut AppState, ui: &mut egui::Ui, command: Command) {
    let enabled = state.command_enabled(command);
    let label = state.with_hint(command.label(), command);
    if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        state.run_command(command, &ctx, now);
        ui.close_menu();
    }
}

/// "Repetir colar…": number of copies and spacing.
fn repeat_paste_window(ctx: &egui::Context, state: &mut AppState) {
    let Some(mut dialog) = state.repeat_dialog.take() else {
        return;
    };
    let mut keep_open = true;
    let mut confirmed = false;
    egui::Window::new("Repetir colar")
        .collapsible(false)
        .resizable(false)
        .open(&mut keep_open)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Número de cópias");
                ui.add(egui::DragValue::new(&mut dialog.count).range(1..=999));
            });
            ui.radio_value(&mut dialog.spaced, false, "Uma após a outra");
            ui.horizontal(|ui| {
                ui.radio_value(&mut dialog.spaced, true, "A cada");
                ui.add_enabled(
                    dialog.spaced,
                    egui::DragValue::new(&mut dialog.seconds)
                        .range(0.04..=3600.0)
                        .speed(0.1)
                        .suffix(" s"),
                );
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Colar").clicked() {
                    confirmed = true;
                }
            });
        });
    if confirmed {
        let spacing = if dialog.spaced {
            RepeatSpacing::Every(dialog.seconds)
        } else {
            RepeatSpacing::EndToEnd
        };
        state.paste_repeated(dialog.count as usize, spacing);
    } else if keep_open {
        state.repeat_dialog = Some(dialog);
    }
}

/// "Personalizar teclado": lists every command, lets the user capture a new shortcut for it,
/// remove shortcuts and restore the defaults. Changes are saved immediately.
fn keymap_window(ctx: &egui::Context, state: &mut AppState) {
    if !state.show_keymap {
        state.capture_for = None;
        return;
    }
    // While a command waits for a key, the next key press becomes its shortcut
    if let Some(target) = state.capture_for {
        let pressed = ctx.input(|i| {
            i.events.iter().find_map(|event| match event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    repeat: false,
                    ..
                } => Some((*key, *modifiers)),
                _ => None,
            })
        });
        if let Some((key, modifiers)) = pressed {
            if key == Key::Escape && !modifiers.any() {
                state.keymap_message = "Atribuição cancelada".into();
            } else if let Some(shortcut) = from_egui(key, modifiers) {
                state.keymap_message = match state.keymap.bind(target, shortcut.clone()) {
                    Some(other) => format!(
                        "{shortcut} saiu de «{}» e foi para «{}»",
                        other.label(),
                        target.label()
                    ),
                    None => format!("{shortcut} atribuído a «{}»", target.label()),
                };
                state.persist_keymap();
            } else {
                state.keymap_message = "Essa tecla não pode ser usada como atalho".into();
            }
            state.capture_for = None;
        }
        ctx.request_repaint();
    }

    let mut open = true;
    egui::Window::new("Personalizar teclado")
        .open(&mut open)
        .default_width(520.0)
        .default_height(480.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Filtro:");
                ui.text_edit_singleline(&mut state.keymap_filter);
                if ui.button("Restaurar tudo").clicked() {
                    state.keymap = KeyMap::defaults();
                    state.persist_keymap();
                    state.keymap_message = "Atalhos padrão restaurados".into();
                }
            });
            if let Some(target) = state.capture_for {
                ui.colored_label(
                    Color32::from_rgb(0xF1, 0xC4, 0x0F),
                    format!(
                        "Pressione a combinação para «{}» (Esc cancela)",
                        target.label()
                    ),
                );
            } else if !state.keymap_message.is_empty() {
                ui.label(state.keymap_message.clone());
            }
            ui.separator();
            let filter = state.keymap_filter.to_lowercase();
            let overridden = state.keymap.to_overrides();
            let mut remove: Option<Shortcut> = None;
            let mut reset: Option<Command> = None;
            egui::ScrollArea::vertical().show(ui, |ui| {
                for category in Category::ALL {
                    let commands: Vec<Command> = Command::ALL
                        .iter()
                        .copied()
                        .filter(|c| c.category() == category)
                        .filter(|c| {
                            filter.is_empty()
                                || c.label().to_lowercase().contains(&filter)
                                || c.id().contains(&filter)
                        })
                        .collect();
                    if commands.is_empty() {
                        continue;
                    }
                    ui.heading(category.label());
                    for command in commands {
                        ui.horizontal_wrapped(|ui| {
                            let label = if command.implemented() {
                                command.label().to_string()
                            } else {
                                format!("{} (em breve)", command.label())
                            };
                            ui.add_sized([230.0, 20.0], egui::Label::new(label).truncate());
                            for shortcut in state.keymap.shortcuts_for(command) {
                                if ui
                                    .small_button(format!("{shortcut} ✕"))
                                    .on_hover_text("Remover este atalho")
                                    .clicked()
                                {
                                    remove = Some(shortcut);
                                }
                            }
                            if ui
                                .small_button("＋")
                                .on_hover_text("Atribuir atalho")
                                .clicked()
                            {
                                state.capture_for = Some(command);
                                state.keymap_message.clear();
                            }
                            if overridden.contains_key(command.id())
                                && ui
                                    .small_button("↺")
                                    .on_hover_text("Restaurar o padrão deste comando")
                                    .clicked()
                            {
                                reset = Some(command);
                            }
                        });
                    }
                }
            });
            if let Some(shortcut) = remove {
                state.keymap.unbind(&shortcut);
                state.persist_keymap();
            }
            if let Some(command) = reset {
                state.keymap.reset_command(command);
                state.persist_keymap();
            }
        });
    state.show_keymap = open;
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
            let selected = state.selection.primary().and_then(|(track_id, event_id)| {
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
    for command in [Command::EditCut, Command::EditCopy, Command::EditDelete] {
        command_item(state, ui, command);
    }
    if ui.button("Selecionar eventos até o fim").clicked() {
        state
            .selection
            .select_to_end(&state.timeline, (track_id, event_id));
        ui.close_menu();
    }
    ui.separator();
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
        if ui
            .selectable_label(state.looping, "🔁")
            .on_hover_text("Reprodução em laço (Q); Shift+Q define a região de laço")
            .clicked()
        {
            state.toggle_loop();
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

    // ---- markers and regions: grab in the strip at the bottom of the ruler ----------------------
    let ruler_bottom = rect.top() + RULER_HEIGHT;
    let in_mark_strip = |p: Pos2| {
        p.x >= lanes.left() && p.y >= ruler_bottom - MARK_STRIP_HEIGHT && p.y < ruler_bottom
    };
    let mark_tolerance = f64::from(MARK_HIT_PX) / state.pixels_per_second;
    if response.drag_started() {
        let press = ui.input(|i| i.pointer.press_origin());
        if let Some(p) = press.filter(|p| in_mark_strip(*p)) {
            let t = x_to_time(state, lanes.left(), p.x);
            state.begin_mark_drag(t, mark_tolerance);
        }
    }
    if response.double_clicked() {
        let hit = response
            .interact_pointer_pos()
            .filter(|p| in_mark_strip(*p))
            .and_then(|p| {
                markers::hit_test(
                    &state.timeline,
                    x_to_time(state, lanes.left(), p.x),
                    mark_tolerance,
                )
            });
        if let Some((id, _)) = hit {
            state.begin_name_edit(NameTarget::Existing(id));
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
                if (in_ruler && state.mark_drag.is_none()) || response.clicked() {
                    state.playhead = snapped;
                    state.blink_origin = now;
                }
                if response.clicked() && !in_ruler {
                    state.selected_track = track_id;
                    let hit =
                        track_id.and_then(|id| state.timeline.event_at(id, t).map(|e| (id, e.id)));
                    let modifiers = ui.input(|i| i.modifiers);
                    match hit {
                        Some(event) if modifiers.command => state.selection.toggle(event),
                        Some(event) if modifiers.shift => {
                            state.selection.extend_to(&state.timeline, event);
                        }
                        Some(event) => {
                            state.selection.select_only(event);
                            state.time_selection = None;
                        }
                        None => {
                            state.selection.clear();
                            state.time_selection = None;
                        }
                    }
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
            let t = x_to_time(state, lanes.left(), p.x);
            if let Some(hit) = hit_event(state, lanes.left(), lane_top, p) {
                state.begin_drag(hit, t);
            } else {
                state.range_anchor = Some(edit::quantize(t.max(0.0), fps));
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
    if state.mark_drag.is_some() {
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            state.mark_drag = None;
        } else if let Some(p) = response.interact_pointer_pos() {
            let t = x_to_time(state, lanes.left(), p.x);
            state.update_mark_drag(t);
            ui.ctx().request_repaint();
        }
    }
    if let (Some(anchor), Some(p)) = (state.range_anchor, response.interact_pointer_pos()) {
        let t = edit::quantize(x_to_time(state, lanes.left(), p.x).max(0.0), fps);
        let range = TimeRange::new(anchor, t);
        state.time_selection = (!range.is_empty()).then_some(range);
        ui.ctx().request_repaint();
    }
    if response.drag_stopped() {
        state.finish_mark_drag();
        state.finish_drag();
        state.range_anchor = None;
    }

    // Right-click on an event: fade curves
    if response.secondary_clicked() {
        let pressed = response
            .interact_pointer_pos()
            .filter(|p| p.x >= lanes.left() && p.y >= lane_top);
        state.context_event = pressed
            .and_then(|p| hit_event(state, lanes.left(), lane_top, p))
            .map(|h| (h.0, h.1));
        state.context_mark = None;
        if let Some((track_id, event_id)) = state.context_event {
            state.selected_track = Some(track_id);
            if !state.selection.contains((track_id, event_id)) {
                state.selection.select_only((track_id, event_id));
            }
        }
    }
    if response.secondary_clicked() {
        let in_ruler = response
            .interact_pointer_pos()
            .filter(|p| p.x >= lanes.left() && p.y < lane_top);
        state.context_mark = in_ruler.and_then(|p| {
            let tolerance = f64::from(MARK_HIT_PX) / state.pixels_per_second;
            state.mark_at(x_to_time(state, lanes.left(), p.x), tolerance)
        });
    }
    response.context_menu(|ui| match (state.context_event, state.context_mark) {
        (Some((track_id, event_id)), _) => event_context_menu(state, ui, track_id, event_id),
        (None, Some(id)) => {
            if ui.button("Renomear").clicked() {
                state.begin_name_edit(NameTarget::Existing(id));
                ui.close_menu();
            }
            if ui.button("Remover marcador ou região").clicked() {
                state.remove_mark(id);
                ui.close_menu();
            }
            if ui.button("Apagar todos os marcadores e regiões").clicked() {
                state.time_selection = None;
                state.clear_marks();
                ui.close_menu();
            }
        }
        (None, None) => {
            for command in [Command::InsertMarker, Command::InsertRegion] {
                command_item(state, ui, command);
            }
            if ui
                .add_enabled(
                    !state.timeline.marks().is_empty(),
                    egui::Button::new("Apagar marcadores e regiões"),
                )
                .on_hover_text("Na seleção de tempo; sem ela, todos")
                .clicked()
            {
                state.clear_marks();
                ui.close_menu();
            }
        }
    });

    // Cursor feedback
    if let Some(drag) = &state.drag {
        ui.ctx().set_cursor_icon(match drag.mode {
            DragMode::Move => egui::CursorIcon::Grabbing,
            DragMode::FadeIn | DragMode::FadeOut => egui::CursorIcon::ResizeHorizontal,
            _ => egui::CursorIcon::ResizeHorizontal,
        });
    } else if let Some((_, part)) =
        response
            .hover_pos()
            .filter(|p| in_mark_strip(*p))
            .and_then(|p| {
                markers::hit_test(
                    &state.timeline,
                    x_to_time(state, lanes.left(), p.x),
                    mark_tolerance,
                )
            })
    {
        ui.ctx().set_cursor_icon(match part {
            MarkPart::RegionStart | MarkPart::RegionEnd => egui::CursorIcon::ResizeHorizontal,
            MarkPart::Marker | MarkPart::RegionBody => egui::CursorIcon::Grab,
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

    // Loop region: a bar along the top edge of the ruler, brighter while loop playback is on
    if let Some(range) = state.loop_region {
        let (x0, x1) = (to_x(range.start), to_x(range.end));
        let bar = Rect::from_min_max(
            pos2(x0.max(lanes.left()), rect.top()),
            pos2(x1.min(lanes.right()), rect.top() + 3.0),
        );
        if bar.width() > 0.0 {
            let alpha = if state.looping { 255 } else { 110 };
            lane_painter.rect_filled(
                bar,
                Rounding::ZERO,
                Color32::from_rgba_unmultiplied(0x3A, 0xC7, 0xE0, alpha),
            );
        }
    }

    // Regions (band along the bottom of the ruler) and markers (flag plus a guide line)
    let strip_top = rect.top() + RULER_HEIGHT - MARK_STRIP_HEIGHT;
    for region in &state.timeline.regions {
        let (start, end) = state.shown_span(region.id, (region.start, region.end));
        let (x0, x1) = (to_x(start), to_x(end));
        if x1 < lanes.left() || x0 > lanes.right() {
            continue;
        }
        let band = Rect::from_min_max(
            pos2(x0.max(lanes.left() - 1.0), strip_top),
            pos2(x1.min(lanes.right() + 1.0), rect.top() + RULER_HEIGHT),
        );
        lane_painter.rect_filled(
            band,
            Rounding::ZERO,
            Color32::from_rgba_unmultiplied(0x2E, 0xCC, 0x71, 70),
        );
        let edge = Stroke::new(hairline, Color32::from_rgb(0x2E, 0xCC, 0x71));
        for x in [x0, x1] {
            lane_painter.line_segment([pos2(x, strip_top), pos2(x, rect.bottom())], edge);
        }
        lane_painter.text(
            pos2(x0.max(lanes.left()) + 3.0, strip_top),
            Align2::LEFT_TOP,
            &region.name,
            FontId::proportional(9.0),
            Color32::from_gray(235),
        );
    }
    for marker in &state.timeline.markers {
        let x = to_x(state.shown_span(marker.id, (marker.time, marker.time)).0);
        if x < lanes.left() - 1.0 || x > lanes.right() + 1.0 {
            continue;
        }
        let color = Color32::from_rgb(0xF1, 0xC4, 0x0F);
        lane_painter.line_segment(
            [pos2(x, strip_top), pos2(x, rect.bottom())],
            Stroke::new(hairline, color.gamma_multiply(0.6)),
        );
        lane_painter.add(Shape::convex_polygon(
            vec![
                pos2(x, strip_top),
                pos2(
                    x + MARK_STRIP_HEIGHT * 0.7,
                    strip_top + MARK_STRIP_HEIGHT * 0.35,
                ),
                pos2(x, strip_top + MARK_STRIP_HEIGHT * 0.7),
            ],
            color,
            Stroke::NONE,
        ));
        lane_painter.text(
            pos2(x + MARK_STRIP_HEIGHT * 0.9, strip_top),
            Align2::LEFT_TOP,
            &marker.name,
            FontId::proportional(9.0),
            color,
        );
    }

    // Inline name box (the mark itself is created or renamed when it is confirmed, below)
    let mut name_outcome = None;
    if let Some(mut edit) = state.name_edit.take() {
        let at = match &edit.target {
            NameTarget::NewMarker(time) => *time,
            NameTarget::NewRegion(range) => range.start,
            NameTarget::Existing(id) => markers::span_of(&state.timeline, *id).map_or(0.0, |s| s.0),
        };
        let x = to_x(at).clamp(
            lanes.left(),
            (lanes.right() - NAME_BOX_WIDTH).max(lanes.left()),
        );
        let box_rect =
            Rect::from_min_size(pos2(x + 2.0, rect.top() + 2.0), vec2(NAME_BOX_WIDTH, 18.0));
        let name_response = ui.put(
            box_rect,
            egui::TextEdit::singleline(&mut edit.text)
                .font(FontId::proportional(11.0))
                .hint_text("Nome")
                .desired_width(NAME_BOX_WIDTH),
        );
        if !edit.focused {
            name_response.request_focus();
            edit.focused = true;
        }
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            name_outcome = Some(false);
        } else if name_response.lost_focus() {
            name_outcome = Some(true);
        }
        state.name_edit = Some(edit);
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
            let selected = state.selection.contains((track.id, ev.id));
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

    // Time selection: translucent band over the lanes and the ruler
    if let Some(range) = state.time_selection {
        let x0 = to_x(range.start).clamp(lanes.left(), lanes.right());
        let x1 = to_x(range.end).clamp(lanes.left(), lanes.right());
        if x1 > x0 {
            let band = Rect::from_min_max(
                pos2(x0, rect.top() + RULER_HEIGHT - 6.0),
                pos2(x1, rect.bottom()),
            );
            lane_painter.rect_filled(
                band,
                Rounding::ZERO,
                Color32::from_rgba_unmultiplied(0x29, 0x80, 0xB9, 48),
            );
            let edge = Stroke::new(hairline, Color32::from_rgb(0x34, 0x98, 0xDB));
            for x in [x0, x1] {
                lane_painter.line_segment([pos2(x, band.top()), pos2(x, band.bottom())], edge);
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

    // `to_x` borrows the state while drawing, so the name box's result is applied last
    match name_outcome {
        Some(true) => state.commit_name_edit(),
        Some(false) => state.cancel_name_edit(),
        None => {}
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

    // ---- selection, clipboard and compound edits ------------------------------------------------

    fn spans(s: &AppState, track_id: TrackId) -> Vec<(f64, f64)> {
        s.timeline
            .track(track_id)
            .unwrap()
            .events
            .iter()
            .map(|e| (e.start, e.end()))
            .collect()
    }

    fn run(s: &mut AppState, command: Command) {
        s.run_command(command, &egui::Context::default(), 0.0);
    }

    #[test]
    fn cut_then_paste_moves_an_event_and_each_step_is_undoable() {
        let (mut s, track, ev) = state_with_clip();
        s.selection.select_only((track, ev));
        run(&mut s, Command::EditCut);
        assert!(spans(&s, track).is_empty());
        assert!(s.clipboard.is_some());
        assert!(
            s.selection.is_empty(),
            "the cut event is no longer selected"
        );

        s.playhead = 10.0;
        run(&mut s, Command::EditPaste);
        assert_eq!(spans(&s, track), vec![(10.0, 14.0)]);
        assert_eq!(s.selection.len(), 1, "the pasted event ends up selected");

        s.history.undo(&mut s.timeline);
        assert!(spans(&s, track).is_empty());
        s.history.undo(&mut s.timeline);
        assert_eq!(spans(&s, track), vec![(2.0, 6.0)]);
    }

    #[test]
    fn copy_keeps_the_original_and_paste_insert_pushes_later_events() {
        let (mut s, track, ev) = state_with_clip();
        add_event(&mut s, track, 8.0, 2.0);
        s.selection.select_only((track, ev));
        run(&mut s, Command::EditCopy);
        assert_eq!(spans(&s, track).len(), 2);

        s.playhead = 7.0;
        run(&mut s, Command::EditPasteInsert);
        assert_eq!(
            spans(&s, track),
            vec![(2.0, 6.0), (7.0, 11.0), (12.0, 14.0)],
            "the clip is 4 s long, so the event that started at 8 s moves to 12 s"
        );
    }

    #[test]
    fn set_in_and_out_make_a_time_selection_that_delete_removes() {
        let (mut s, track, _) = state_with_clip();
        s.playhead = 3.0;
        run(&mut s, Command::SelectionSetIn);
        s.playhead = 5.0;
        run(&mut s, Command::SelectionSetOut);
        let range = s.time_selection.unwrap();
        assert_eq!((range.start, range.end), (3.0, 5.0));

        run(&mut s, Command::EditDelete);
        assert_eq!(spans(&s, track), vec![(2.0, 3.0), (5.0, 6.0)]);
        s.history.undo(&mut s.timeline);
        assert_eq!(spans(&s, track), vec![(2.0, 6.0)]);
    }

    #[test]
    fn trim_to_selection_keeps_only_the_selected_time() {
        let (mut s, track, _) = state_with_clip();
        s.time_selection = Some(TimeRange::new(3.0, 5.0));
        run(&mut s, Command::EditTrimToSelection);
        assert_eq!(spans(&s, track), vec![(3.0, 5.0)]);
        assert_eq!(s.timeline.track(track).unwrap().events[0].trim_in, 1.0);
    }

    #[test]
    fn trim_start_and_end_follow_the_cursor() {
        let (mut s, track, ev) = state_with_clip();
        s.selection.select_only((track, ev));
        s.playhead = 3.0;
        run(&mut s, Command::EditTrimStart);
        s.playhead = 5.0;
        run(&mut s, Command::EditTrimEnd);
        assert_eq!(spans(&s, track), vec![(3.0, 5.0)]);
    }

    #[test]
    fn split_without_a_selection_cuts_every_track_under_the_cursor() {
        let (mut s, first, _) = state_with_clip();
        s.add_track(TrackKind::Audio);
        let second = s.selected_track.unwrap();
        add_event(&mut s, second, 0.0, 8.0);
        s.selection.clear();
        s.playhead = 3.0;
        run(&mut s, Command::EditSplit);
        assert_eq!(spans(&s, first), vec![(2.0, 3.0), (3.0, 6.0)]);
        assert_eq!(spans(&s, second), vec![(0.0, 3.0), (3.0, 8.0)]);
        s.history.undo(&mut s.timeline);
        assert_eq!(spans(&s, first).len(), 1);
        assert_eq!(spans(&s, second).len(), 1);
    }

    #[test]
    fn split_with_a_selection_only_cuts_the_selected_events() {
        let (mut s, first, ev) = state_with_clip();
        s.add_track(TrackKind::Audio);
        let second = s.selected_track.unwrap();
        add_event(&mut s, second, 0.0, 8.0);
        s.selection.select_only((first, ev));
        s.playhead = 3.0;
        run(&mut s, Command::EditSplit);
        assert_eq!(spans(&s, first).len(), 2);
        assert_eq!(spans(&s, second).len(), 1);
    }

    #[test]
    fn select_all_and_delete_clears_everything_then_none_clears_the_range() {
        let (mut s, track, _) = state_with_clip();
        add_event(&mut s, track, 8.0, 2.0);
        run(&mut s, Command::EditSelectAll);
        assert_eq!(s.selection.len(), 2);
        run(&mut s, Command::EditDelete);
        assert!(spans(&s, track).is_empty());

        s.time_selection = Some(TimeRange::new(1.0, 2.0));
        run(&mut s, Command::EditSelectNone);
        assert!(s.time_selection.is_none() && s.selection.is_empty());
    }

    #[test]
    fn repeated_paste_is_one_undo_step() {
        let (mut s, track, ev) = state_with_clip();
        s.selection.select_only((track, ev));
        run(&mut s, Command::EditCopy);
        s.playhead = 20.0;
        s.paste_repeated(3, RepeatSpacing::EndToEnd);
        assert_eq!(
            spans(&s, track),
            vec![(2.0, 6.0), (20.0, 24.0), (24.0, 28.0), (28.0, 32.0)]
        );
        s.history.undo(&mut s.timeline);
        assert_eq!(spans(&s, track), vec![(2.0, 6.0)]);
    }

    #[test]
    fn marker_and_region_commands_are_undoable_and_mark_the_project_dirty() {
        let (mut s, _, _) = state_with_clip();
        let revision = s.history.revision();
        s.playhead = 2.0;
        run(&mut s, Command::InsertMarker);
        s.commit_name_edit();
        assert_eq!(s.timeline.markers.len(), 1);
        assert!(s.history.revision() > revision);

        run(&mut s, Command::InsertMarker);
        s.commit_name_edit();
        assert_eq!(s.timeline.markers.len(), 1, "same point twice adds nothing");

        run(&mut s, Command::InsertRegion);
        s.commit_name_edit();
        assert!(
            s.timeline.regions.is_empty(),
            "no time selection, no region"
        );
        s.time_selection = Some(TimeRange::new(1.0, 3.0));
        run(&mut s, Command::InsertRegion);
        s.commit_name_edit();
        assert_eq!(s.timeline.regions.len(), 1);

        run(&mut s, Command::EditUndo);
        assert!(s.timeline.regions.is_empty());
        run(&mut s, Command::EditUndo);
        assert!(s.timeline.markers.is_empty());
    }

    #[test]
    fn ctrl_arrows_jump_between_markers_and_region_edges() {
        let (mut s, _, _) = state_with_clip();
        for t in [1.0, 4.0] {
            s.playhead = t;
            run(&mut s, Command::InsertMarker);
            s.commit_name_edit();
        }
        s.playhead = 0.0;
        run(&mut s, Command::CursorNextMarker);
        assert_eq!(s.playhead, 1.0);
        run(&mut s, Command::CursorNextMarker);
        assert_eq!(s.playhead, 4.0);
        run(&mut s, Command::CursorNextMarker);
        assert_eq!(s.playhead, 4.0, "stays put after the last marker");
        run(&mut s, Command::CursorPreviousMarker);
        assert_eq!(s.playhead, 1.0);
    }

    #[test]
    fn mark_at_picks_a_marker_by_tolerance_and_a_region_by_span() {
        let (mut s, _, _) = state_with_clip();
        s.playhead = 5.0;
        run(&mut s, Command::InsertMarker);
        s.commit_name_edit();
        s.time_selection = Some(TimeRange::new(8.0, 10.0));
        run(&mut s, Command::InsertRegion);
        s.commit_name_edit();
        let marker = s.timeline.markers[0].id;
        let region = s.timeline.regions[0].id;
        assert_eq!(s.mark_at(5.04, 0.05), Some(marker));
        assert_eq!(s.mark_at(9.0, 0.05), Some(region));
        assert_eq!(s.mark_at(7.0, 0.05), None);

        s.remove_mark(marker);
        assert!(s.timeline.markers.is_empty());
        s.time_selection = None;
        s.clear_marks();
        assert!(s.timeline.regions.is_empty());
    }

    #[test]
    fn loop_bounds_prefer_the_loop_region_then_the_selection_then_the_project() {
        let (mut s, _, _) = state_with_clip();
        let whole = s.timeline.duration();
        assert_eq!(s.loop_bounds(), Some(TimeRange::new(0.0, whole)));

        s.time_selection = Some(TimeRange::new(3.0, 5.0));
        assert_eq!(s.loop_bounds(), Some(TimeRange::new(3.0, 5.0)));

        run(&mut s, Command::SelectionLoopRegion);
        s.time_selection = None;
        assert_eq!(s.loop_bounds(), Some(TimeRange::new(3.0, 5.0)));

        run(&mut s, Command::SelectionLoopRegion);
        assert!(
            s.loop_region.is_none(),
            "no selection and no region clears it"
        );
        assert_eq!(s.loop_bounds(), Some(TimeRange::new(0.0, whole)));
    }

    #[test]
    fn q_toggles_the_loop_and_only_then_are_bounds_sent_to_the_player() {
        let (mut s, _, _) = state_with_clip();
        assert!(s.loop_args().is_none());
        run(&mut s, Command::TransportLoop);
        assert!(s.looping && s.loop_args().is_some());
        run(&mut s, Command::TransportLoop);
        assert!(!s.looping && s.loop_args().is_none());
    }

    #[test]
    fn shift_q_takes_the_region_under_the_cursor() {
        let (mut s, _, _) = state_with_clip();
        s.time_selection = Some(TimeRange::new(6.0, 9.0));
        run(&mut s, Command::InsertRegion);
        s.commit_name_edit();
        s.time_selection = None;
        s.playhead = 7.0;
        run(&mut s, Command::SelectionLoopRegion);
        assert_eq!(s.loop_region, Some(TimeRange::new(6.0, 9.0)));
    }

    #[test]
    fn the_name_box_creates_the_mark_only_when_confirmed() {
        let (mut s, _, _) = state_with_clip();
        s.playhead = 2.0;
        run(&mut s, Command::InsertMarker);
        assert!(s.name_edit.is_some() && s.timeline.markers.is_empty());
        s.name_edit.as_mut().unwrap().text = "  Abertura ".into();
        s.commit_name_edit();
        assert_eq!(s.timeline.markers[0].name, "Abertura");

        s.playhead = 4.0;
        run(&mut s, Command::InsertMarker);
        s.cancel_name_edit();
        assert_eq!(s.timeline.markers.len(), 1, "Esc creates nothing");

        s.playhead = 6.0;
        run(&mut s, Command::InsertMarker);
        s.commit_name_edit();
        assert_eq!(
            s.timeline.markers[1].name, "Marcador 2",
            "blank keeps the default"
        );
    }

    #[test]
    fn double_click_renames_in_one_undo_step() {
        let (mut s, _, _) = state_with_clip();
        s.playhead = 2.0;
        run(&mut s, Command::InsertMarker);
        s.commit_name_edit();
        let id = s.timeline.markers[0].id;

        s.begin_name_edit(NameTarget::Existing(id));
        assert_eq!(s.name_edit.as_ref().unwrap().text, "Marcador 1");
        s.name_edit.as_mut().unwrap().text = "Cena 1".into();
        s.commit_name_edit();
        assert_eq!(s.timeline.markers[0].name, "Cena 1");

        run(&mut s, Command::EditUndo);
        assert_eq!(s.timeline.markers[0].name, "Marcador 1");
    }

    #[test]
    fn dragging_a_marker_previews_and_moves_it_in_one_undo_step() {
        let (mut s, _, _) = state_with_clip();
        s.playhead = 5.0;
        run(&mut s, Command::InsertMarker);
        s.commit_name_edit();
        let id = s.timeline.markers[0].id;
        let revision = s.history.revision();

        assert!(s.begin_mark_drag(5.02, 0.05));
        s.update_mark_drag(8.0);
        assert_eq!(s.timeline.markers[0].time, 5.0, "only the preview moves");
        assert!((s.shown_span(id, (5.0, 5.0)).0 - 8.0).abs() < 0.04);
        s.finish_mark_drag();
        assert!((s.timeline.markers[0].time - 8.0).abs() < 0.04);
        assert_eq!(s.history.revision(), revision + 1);

        run(&mut s, Command::EditUndo);
        assert_eq!(s.timeline.markers[0].time, 5.0);
    }

    #[test]
    fn dragging_a_region_edge_resizes_it_and_missing_the_mark_grabs_nothing() {
        let (mut s, _, _) = state_with_clip();
        s.time_selection = Some(TimeRange::new(2.0, 4.0));
        run(&mut s, Command::InsertRegion);
        s.commit_name_edit();
        assert!(!s.begin_mark_drag(9.0, 0.05));

        assert!(s.begin_mark_drag(4.01, 0.05));
        s.update_mark_drag(6.0);
        s.finish_mark_drag();
        let r = &s.timeline.regions[0];
        assert_eq!(r.start, 2.0);
        assert!((r.end - 6.0).abs() < 0.04);
    }

    #[test]
    fn a_cancelled_mark_drag_changes_nothing() {
        let (mut s, _, _) = state_with_clip();
        s.playhead = 5.0;
        run(&mut s, Command::InsertMarker);
        s.commit_name_edit();
        let revision = s.history.revision();
        assert!(s.begin_mark_drag(5.0, 0.05));
        s.update_mark_drag(9.0);
        s.mark_drag = None;
        s.finish_mark_drag();
        assert_eq!(s.timeline.markers[0].time, 5.0);
        assert_eq!(s.history.revision(), revision);
    }

    #[test]
    fn paste_with_an_empty_clipboard_reports_it_and_changes_nothing() {
        let (mut s, track, _) = state_with_clip();
        let revision = s.history.revision();
        run(&mut s, Command::EditPaste);
        assert_eq!(s.history.revision(), revision);
        assert_eq!(spans(&s, track).len(), 1);
        assert!(s.status.contains("vazia"));
    }
}
