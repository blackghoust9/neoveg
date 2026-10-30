//! Main application: dockable workspace (Preview, Timeline, Project Media, Trimmer, Master Bus).

use neoveg_common::i18n::tr_args;
use neoveg_common::i18n::tr_text;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

mod composite;
mod detached;
mod dock_footer;
pub(crate) mod headless;
mod render_job;
mod tabs;

use crate::theme as ui_theme;
use eframe::egui::{
    self, pos2, vec2, Align2, Color32, FontId, Key, Pos2, Rect, Rounding, Sense, Shape, Stroke,
    TextureHandle,
};
use egui_dock::{DockArea, DockState, Node, NodeIndex, Style, TabViewer};
use neoveg_audio::decode::{decode_to_store, probe_audio};
use neoveg_audio::meter::{linear_to_dbfs, PeakMeter};
use neoveg_audio::metronome::{click_clips, Metronome};
use neoveg_audio::mixer::MixClip;
use neoveg_audio::pcm::PcmStore;
use neoveg_audio::playback::{wrap_loop, AudioPlayer};
use neoveg_audio::readout::LoudnessReadout;
use neoveg_audio::record::Recorder;
use neoveg_audio::waveform::WaveformSource;
use neoveg_common::chroma::ChromaKey;
use neoveg_common::commands::{Category, Command};
use neoveg_common::fade::FadeCurve;
use neoveg_common::grade::Grade;
use neoveg_common::i18n::tr;
use neoveg_common::keymap::{KeyMap, Shortcut};
use neoveg_common::paths::autosave_dir;
use neoveg_common::preferences::Preferences;
use neoveg_common::project::{MediaRef, ProjectInfo, ProjectSettings};
use neoveg_common::theme::{self, Theme};
use neoveg_common::timecode::{is_drop_frame_rate, seconds_to_timecode};
use neoveg_engine::aaf;
use neoveg_engine::attributes::{self, Attributes};
use neoveg_engine::autosave::{self, RecoveryEntry};
use neoveg_engine::captions;
use neoveg_engine::crossfade::{find_crossfades, Crossfade};
use neoveg_engine::edit;
use neoveg_engine::edl::{self, EdlEvent, MediaMatch};
use neoveg_engine::effects as fx;
use neoveg_engine::envelope::{self, AutomationTarget, EnvelopeShape};
use neoveg_engine::fcp7::{self, XmlClip, XmlMedia};
use neoveg_engine::fcpxml;
use neoveg_engine::groups;
use neoveg_engine::markers::{self, MarkId, MarkPart};
use neoveg_engine::mix;
use neoveg_engine::navigation;
use neoveg_engine::ops::{self, Clipboard, PasteMode, RepeatSpacing, Scope};
use neoveg_engine::otio;
use neoveg_engine::pan_crop as pcrop;
use neoveg_engine::project_file;
use neoveg_engine::relink::{self, Outcome, Rejected};
use neoveg_engine::ripple::RippleScope;
use neoveg_engine::switches::{self, Switch, SwitchState};
use neoveg_engine::timeline::TIME_EPSILON;
use neoveg_engine::track_motion as tmotion;
use neoveg_engine::transitions as trans;
use neoveg_engine::velocity as vel;
use neoveg_engine::video_effects as vfxe;
use neoveg_engine::EventGeometry;
use neoveg_engine::{
    EditCommand, EventId, EventRef, FxTarget, History, Selection, TimeRange, Timeline, TrackEvent,
    TrackId, TrackKind, VideoFxTarget,
};
use neoveg_video::info::{is_video_path, is_visual_media_path, VideoInfo};
use neoveg_video::lut::Lut;
use neoveg_video::scopes::Scopes;
use neoveg_video::worker::{probe, FrameRequest, FrameWorker, SourceRequest};
use tabs::{close_tab_window, project_tab_bar, Tabs};

use crate::captions_panel::{self, CaptionAction, CaptionsPanelState};
use crate::envelope_canvas::{self, EnvelopeDrag, EnvelopeHit};
use crate::event_header;
use crate::frame_grid::{snap_to_device_pixel, FrameGrid};
use crate::fx_panel::{self, DuckAction, FxAction, FxPanelState};
use crate::icons::{self, Icon};
use crate::mixer_panel::{self, MixAction, MixerPanelState};
use crate::pan_crop_window::{EditTarget, PanCropAction, PanCropDraft, PanCropView, PanCropWindow};
use crate::precision_slider::PrecisionSlider;
use crate::scopes_panel::{scopes_ui, ScopesPanelState};
use crate::shortcuts::{from_egui, to_egui};
use crate::splash::Splash;
use crate::timeline_scroll;
use crate::transitions_panel::{TransitionAction, TransitionsPanelState, TransitionsView};
use crate::velocity_editor::{VelocityAction, VelocityDraft, VelocityEditor, VelocityView};
use crate::video_fx_panel::{self, VfxAction, VfxDraft, VfxPanelState, VfxScope, VfxScopes};
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

/// How the preview picture is sampled when it is stretched to the pane. A reduced quality (Draft,
/// Preview) shows its lower resolution as visible pixels instead of smearing it into a blur; the full
/// size and the fit-to-pane levels stay smooth.
fn preview_texture_options(quality: usize) -> egui::TextureOptions {
    match QUALITIES.get(quality) {
        Some(&(_, factor)) if factor > 0.0 && factor < 1.0 => egui::TextureOptions::NEAREST,
        _ => egui::TextureOptions::LINEAR,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) enum Tab {
    Preview,
    Timeline,
    Media,
    MasterBus,
    Loudness,
    AudioFx,
    VideoFx,
    Transitions,
    Scopes,
}

impl Tab {
    fn title(self) -> &'static str {
        match self {
            Tab::Preview => "Prévia de vídeo",
            Tab::Timeline => "Linha do tempo",
            Tab::Media => "Mídia",
            Tab::MasterBus => "Barramento master",
            Tab::Loudness => "Medidores de loudness",
            Tab::AudioFx => "Efeitos de áudio",
            Tab::VideoFx => "Efeitos de vídeo",
            Tab::Transitions => "Transições",
            Tab::Scopes => "Scopes de vídeo",
        }
    }
}

/// How the Trimmer puts its selection on the timeline.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TrimmerAdd {
    /// After the timeline cursor (or into the timeline selection).
    FromCursor,
    /// Ending at the timeline cursor.
    UpToCursor,
    /// Compressed or stretched to fill the timeline selection.
    FitToFill,
}

/// What a window command asks of the dock.
#[derive(Clone, PartialEq, Eq, Debug)]
enum WindowAction {
    /// Show the window (bring it to the front, or put it back when it was closed); when it is
    /// already in front, close it.
    Toggle(Tab),
    /// Bring the window to the front (open it when it was closed), never close it.
    Show(Tab),
    DefaultLayout,
    /// Load the saved layout in this file.
    LoadLayout(PathBuf),
    /// Load the selected layout again, undoing changes made since.
    ReloadLayout,
    /// Save the dock as a layout with this name and shortcut number.
    SaveLayout {
        name: String,
        slot: Option<u8>,
    },
    /// Save the dock over the selected layout.
    SaveCurrentLayout,
    /// `Alt+D`, then a number: load the layout that has it.
    RecallSlot(u8),
    /// `Ctrl+Alt+D`, then a number: save the dock under that number.
    SaveSlot(u8),
    DeleteLayout(PathBuf),
    SetLayoutSlot(PathBuf, Option<u8>),
}

/// The *Salvar layout como* dialog.
struct LayoutDialog {
    name: String,
    slot: Option<u8>,
}

/// Decoded audio kept in memory for waveform drawing and playback.
struct LoadedMedia {
    path: PathBuf,
    name: String,
    waveform: Arc<WaveformSource>,
    stereo: Arc<PcmStore>,
    sample_rate: u32,
    /// Sound channels of the file (the waveform and the mixer use a stereo copy).
    channels: u16,
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
    /// The background search for a missing media came to an end.
    Relink { id: u64, outcome: Outcome },
    /// A found file was copied next to the project (or the copy failed).
    Copied {
        id: u64,
        result: Result<PathBuf, String>,
    },
    Failed {
        path: PathBuf,
        error: String,
        existing_id: Option<u64>,
        /// The file is already the result of a conversion: do not offer another one.
        converted: bool,
    },
}

/// A media file the project points to that is not on disk, as the *Mídia ausente* window shows it.
struct MissingMedia {
    id: u64,
    /// Name of the file, for the list.
    name: String,
    /// Where the project expected it.
    old_path: String,
    /// Candidates that were looked at and why they were not taken.
    rejected: Vec<Rejected>,
    /// The search is still running.
    searching: bool,
    /// *Ignorar e deixar offline*: the window does not ask about it again, and the events stay marked.
    ignored: bool,
}

/// A media file that was found outside the project's folder: it can be copied there.
struct FoundMedia {
    id: u64,
    name: String,
    path: PathBuf,
    copying: bool,
}

/// A search of the whole machine for the missing media, running on a worker thread.
#[derive(Default)]
struct DeepSearch {
    /// Set by the *Parar* button; the walk ends at the next folder.
    cancel: std::sync::atomic::AtomicBool,
    /// Folders entered so far.
    folders: std::sync::atomic::AtomicU64,
    /// The walk and the checks of what it found are over.
    done: std::sync::atomic::AtomicBool,
    /// A folder the walk is in now, for the progress line (updated every few folders).
    current: std::sync::Mutex<String>,
}

/// The places a search of everything starts from: every drive on Windows, the root of the file
/// system elsewhere.
fn search_roots() -> Vec<PathBuf> {
    if cfg!(windows) {
        ('A'..='Z')
            .map(|letter| PathBuf::from(format!("{letter}:\\")))
            .filter(|root| root.exists())
            .collect()
    } else {
        vec![PathBuf::from("/")]
    }
}

/// A media file the project points to that exists but could not be read, waiting for a choice.
struct MediaError {
    id: u64,
    path: String,
    error: String,
}

/// A file that could not be imported, waiting for the user to choose whether to convert it.
struct ConvertOffer {
    path: PathBuf,
    error: String,
}

/// The editing tool: what dragging the body of an event does.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum EditTool {
    /// Moves the event (the usual behaviour).
    #[default]
    Normal,
    /// Moves the media under the event, which stays where it is.
    Slip,
    /// Moves the event while its media keeps its place on the timeline.
    Slide,
    /// Dropping an event among the others reorders the sequence.
    Shuffle,
}

impl EditTool {
    const ALL: [EditTool; 4] = [
        EditTool::Normal,
        EditTool::Slip,
        EditTool::Slide,
        EditTool::Shuffle,
    ];

    fn label(self) -> &'static str {
        match self {
            EditTool::Normal => "Normal: seleciona, move e apara",
            EditTool::Slip => "Slip: desliza a mídia dentro do evento",
            EditTool::Slide => "Slide: move o evento sem mover a mídia",
            EditTool::Shuffle => "Embaralhar: reordena os eventos da faixa ao soltar",
        }
    }

    fn icon(self) -> icons::SvgIcon {
        match self {
            EditTool::Normal => icons::SvgIcon::ToolNormal,
            EditTool::Slip => icons::SvgIcon::ToolSlip,
            EditTool::Slide => icons::SvgIcon::ToolSlide,
            EditTool::Shuffle => icons::SvgIcon::ToolShuffle,
        }
    }

    /// The next tool in the cycle (`D`), or the previous one (`Shift+D`).
    fn step(self, forward: bool) -> EditTool {
        let i = Self::ALL.iter().position(|t| *t == self).unwrap_or(0);
        let n = Self::ALL.len();
        Self::ALL[if forward {
            (i + 1) % n
        } else {
            (i + n - 1) % n
        }]
    }
}

/// What a mouse drag on an event does, decided by where the press landed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DragMode {
    Move,
    /// Slip tool: the media moves under the event.
    Slip,
    /// Slide tool: the event moves, its media stays on the timeline.
    Slide,
    /// Shuffle tool: the event is dropped among the others of its track.
    Shuffle,
    TrimLeft,
    TrimRight,
    /// `Ctrl+Alt` on a left edge that touches the previous event: the cut between them moves.
    RollLeft,
    /// `Ctrl+Alt` on a right edge that touches the next event: the cut between them moves.
    RollRight,
    StretchLeft,
    StretchRight,
    FadeIn,
    FadeOut,
}

/// What the dragged edge or block is aligned to right now (drawn as a vertical line).
#[derive(Clone, Copy, PartialEq, Debug)]
enum SnapMark {
    /// Snapped to a target (marker, event edge, cursor, grid line): solid line.
    Target(f64),
    /// Not snapped, but quantizing moved it onto a frame: dashed line.
    Rounded(f64),
}

/// The neighbour that a rolling trim drags along (its edge is the same cut).
struct RollPartner {
    event_id: EventId,
    origin: EventGeometry,
    current: EventGeometry,
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
    /// Every event that moves with this one (the whole selection), empty for a single event.
    moving: Vec<EventRef>,
    /// The event on the other side of the cut during a rolling trim.
    partner: Option<RollPartner>,
    /// The alignment shown while dragging.
    snap_mark: Option<SnapMark>,
}

/// A rectangle being dragged over the lanes with Shift: it selects the events it touches.
struct Marquee {
    anchor_time: f64,
    /// Row in lane units from the top of the first track (fractional while the pointer is inside).
    anchor_row: f32,
    current_time: f64,
    current_row: f32,
    /// Ctrl: the events already selected stay selected.
    additive: bool,
}

struct ActivePlayer {
    player: AudioPlayer,
}

/// Loads one media file (the work of a loader thread): a probe for pictures and videos, a decode for
/// sound. A failure carries the path so the interface can offer to convert the file.
fn load_message(path: PathBuf, existing_id: Option<u64>, converted: bool) -> LoadMsg {
    let failed = |path: PathBuf, error: String| LoadMsg::Failed {
        path,
        error,
        existing_id,
        converted,
    };
    if is_visual_media_path(&path) {
        match probe(&path) {
            Ok(info) => LoadMsg::VideoLoaded {
                // A video without a usable audio track is still a valid video clip; images and
                // generators have no sound to look for
                audio: if is_video_path(&path) {
                    load_audio(&path).ok()
                } else {
                    None
                },
                path,
                info,
                existing_id,
            },
            Err(e) => failed(path, e),
        }
    } else {
        match load_audio(&path) {
            Ok(media) => LoadMsg::Loaded { media, existing_id },
            Err(e) => failed(path, e),
        }
    }
}

/// Options of the "Repetir colar" dialog.
struct RepeatDialog {
    count: u32,
    spaced: bool,
    seconds: f64,
}

type StretchCacheKey = (u64, u64, u64, u64, bool);

struct AppState {
    project: ProjectInfo,
    timeline: Timeline,
    history: History,
    prefs: Preferences,
    /// System font for Japanese and Chinese, loaded when one of them is the language.
    cjk_fonts: crate::fonts::CjkFonts,
    /// The footer strips of the docked windows.
    dock_footer: dock_footer::DockFooter,
    keymap: KeyMap,
    media: HashMap<u64, LoadedMedia>,
    /// Cached WSOLA / varispeed slices keyed by `(media_id, trim_bits, dur_bits, rate_bits, preserve_pitch)`.
    stretch_cache: std::cell::RefCell<HashMap<StretchCacheKey, Arc<PcmStore>>>,

    playhead: f64,
    playing: bool,
    blink_origin: f64,
    pixels_per_second: f64,
    scroll_seconds: f64,
    /// Vertical scroll of the track rows, in points (0 = the first track touches the ruler).
    scroll_y: f32,
    selected_track: Option<TrackId>,
    selection: Selection,
    /// Range chosen by dragging on empty lane space or with `I` / `O`.
    time_selection: Option<TimeRange>,
    range_anchor: Option<f64>,
    clipboard: Option<Clipboard>,
    repeat_dialog: Option<RepeatDialog>,
    /// The attributes picked in the *Colar atributos seletivamente* dialog.
    attributes_dialog: Option<Attributes>,
    /// Media the open project points to that is not on disk.
    missing: Vec<MissingMedia>,
    missing_open: bool,
    /// The search of the whole machine, while it runs.
    deep_search: Option<Arc<DeepSearch>>,
    /// Found media that lives outside the project's folder, offered for copying there.
    found: Vec<FoundMedia>,
    found_open: bool,
    /// Files of the project that could not be read, asked about one at a time.
    media_errors: std::collections::VecDeque<MediaError>,
    /// *Ignorar todos os erros seguintes*: from now on an unreadable file is left offline without asking.
    ignore_media_errors: bool,
    /// The *convert this media?* question after a failed import.
    convert_offer: Option<ConvertOffer>,
    /// Text of the open "go to time" box.
    goto_dialog: Option<String>,
    /// Exibir > Notas do projeto.
    notes: crate::notes_window::NotesState,
    /// Exibir > Detalhes de edição.
    show_details: bool,
    /// The editing tool (`D` cycles through them).
    edit_tool: EditTool,
    /// The welcome screen (recent projects, new project).
    welcome: crate::welcome::WelcomeState,
    /// File > Properties: the project's video and audio format.
    project_dialog: crate::project_dialog::ProjectDialogState,
    /// True once the user's real preferences are loaded: only then are changes written back.
    persist_prefs: bool,
    /// The picture format of a media file being read for "match media settings".
    match_media_rx: Option<Receiver<MediaFormat>>,
    /// Insert > Time…: the text of the length being typed, while the window is open.
    insert_time_dialog: Option<String>,
    /// Width of the lane area in points, from the last drawn frame.
    lanes_width: f32,

    meter: Arc<PeakMeter>,
    /// EBU R128 readings published by the audio callback.
    loudness: Arc<LoudnessReadout>,
    /// Index into `loudness_panel::TARGETS`.
    loudness_target: usize,
    show_credits: bool,
    /// Exibir > Visualizador de Logs.
    log_viewer: crate::log_viewer::LogViewerState,
    /// Arquivo > Renderizar como… (Make Movie).
    render_dialog: crate::render_dialog::RenderDialogState,
    /// Faixas > Inserir apresentação de slides.
    slideshow: crate::slideshow_window::SlideshowState,
    /// True once the project's frame size and rate were chosen (by the user or by a file), so the
    /// first media does not overwrite them.
    format_chosen: bool,
    /// Faixas > Inserir mídia gerada.
    generator_dialog: crate::generator_dialog::GeneratorDialogState,
    /// Subtitles tab (text and times being edited).
    captions_panel: CaptionsPanelState,
    /// Audio effects panel (target and the slider being dragged).
    fx: FxPanelState,
    /// Video effects panel (event or track scope and the slider being dragged).
    vfx: VfxPanelState,
    /// Transitions tab (entrance or exit, and the slider being dragged).
    transitions: TransitionsPanelState,
    /// The video effect slider being dragged, for the live preview.
    vfx_draft: Option<VfxDraft>,
    /// The Pan/Crop editor and the Pan/Crop it is changing right now (for the live preview).
    pan_crop: PanCropWindow,
    pan_crop_draft: Option<PanCropDraft>,
    /// The velocity envelope being dragged in the video effects panel (for the live preview).
    velocity_draft: Option<VelocityDraft>,
    /// The velocity graph of the video effects panel.
    velocity_editor: VelocityEditor,
    /// Per-track mixer strip (fader or pan being dragged).
    mixer: MixerPanelState,
    master_volume: f32,
    active_player: Option<ActivePlayer>,

    show_bars: bool,
    /// Scopes of the frame at the cursor, and a counter that changes with every new frame.
    frame_scopes: Option<Scopes>,
    frame_scopes_version: u64,
    /// Scopes of the SMPTE bars, computed when first needed.
    bars_scopes: Option<Scopes>,
    scopes_panel: ScopesPanelState,
    bars_texture: Option<TextureHandle>,
    quality: usize,
    show_preferences: bool,
    /// Tab of the preferences window.
    prefs_tab: PrefsTab,
    /// (theme id, theme file) the interface is currently painted with.
    theme_applied: Option<(String, String)>,
    /// Message about the theme file (read errors, saves), shown in the preferences window.
    theme_note: String,
    show_keymap: bool,
    keymap_filter: String,
    /// Command waiting for the next key press in the "Personalizar teclado" window.
    capture_for: Option<Command>,
    keymap_message: String,
    show_inspector: bool,
    /// The metronome clicks during playback.
    metronome_on: bool,
    /// The audio recording in progress and the timeline time it started at.
    recorder: Option<Recorder>,
    record_start: f64,
    metronome_volume: f32,
    show_metronome: bool,
    video_info: HashMap<u64, VideoInfo>,
    frame_worker: Option<FrameWorker>,
    video_texture: Option<TextureHandle>,
    last_frame_key: Option<(u64, i64, u32)>,
    /// The composite shown in the preview when several video clips overlap: frame, decoded
    /// width and project revision. `Some` means the preview is in composite mode.
    last_composite_key: Option<(i64, u32, u64, u64)>,
    /// The color table loaded for `Timeline::video_lut` (`None` inside when the file is unusable).
    lut_cache: Option<(String, Option<Arc<Lut>>)>,
    last_lut_path: Option<String>,
    last_grade: Grade,
    /// Color correction window: open flag and the value being dragged (applied live, committed on release).
    show_grade: bool,
    grade_draft: Option<Grade>,
    /// Chroma key window: open flag and live draft.
    last_chroma: ChromaKey,
    show_chroma: bool,
    chroma_draft: Option<ChromaKey>,
    /// The correction the SMPTE bars texture and scopes were built with.
    bars_grade: Option<(Grade, ChromaKey)>,
    bars_scopes_rev: u64,
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
    env_drag: Option<EnvelopeDrag>,
    context_event: Option<(TrackId, EventId)>,
    context_envelope: Option<EnvelopeHit>,
    /// Marker or region under the pointer when the ruler's context menu was opened.
    context_mark: Option<MarkId>,
    name_edit: Option<NameEdit>,
    mark_drag: Option<MarkDrag>,
    /// Ctrl+Shift+U: clicking one event of a group selects only that event.
    ignore_grouping: bool,
    /// `Shift` was held when the current edge drag started: only the dragged event is trimmed.
    drag_ignores_group: bool,
    marquee: Option<Marquee>,
    snapping: bool,
    /// Which optional sources snapping uses (grid, markers, events of other tracks).
    snap_sources: neoveg_engine::snapping::SnapSources,
    /// Quantize to Frames: moves, trims and the cursor land on frame boundaries.
    quantize_frames: bool,
    /// `Q`: playback repeats over [`AppState::loop_args`].
    looping: bool,
    /// `Shift+Q`: the span that loops; without one the time selection (or the whole project) does.
    loop_region: Option<TimeRange>,
    media_tab: usize,
    /// The media file open in the Trimmer, if any.
    trimmer: Option<neoveg_engine::trimmer::TrimmerState>,
    /// Trimmer: new events replace what they cover instead of lying over it.
    trimmer_overwrite: bool,
    /// Where the drag on the Trimmer timeline began.
    trimmer_drag_from: Option<f64>,
    /// A window command the dock must carry out (the dock lives outside `AppState`).
    window_request: Option<WindowAction>,
    /// A click to hand to the timeline on the next frame, as if the right button had been pressed
    /// there: the menu button of an event header opens the context menu this way.
    pending_context_click: Option<Pos2>,
    /// Where the saved window layouts live (`None` reads and writes nothing).
    layouts_dir: Option<PathBuf>,
    /// The saved layouts, read from `layouts_dir` when first needed and after each change.
    layouts: Vec<crate::layouts::SavedLayout>,
    layouts_loaded: bool,
    /// The layout the dock was loaded from or saved as (marked in the menu).
    current_layout: Option<PathBuf>,
    layout_dialog: Option<LayoutDialog>,
    organize_layouts: bool,
    /// `Alt+D` (load) or `Ctrl+Alt+D` (save) was pressed: the next number picks the layout, until this time.
    layout_chord: Option<(bool, f64)>,
    /// Auto ripple: deleting events closes the gap they leave (`Ctrl+L`).
    auto_ripple: bool,
    /// Edge selected by the keyboard trim mode (`[` / `]`); `←` / `→` then nudge it a frame.
    trim_edge: Option<neoveg_engine::keytrim::Edge>,
    /// What a ripple carries along (`F`, `Ctrl+F`, `Ctrl+Shift+F`); used by Auto Ripple and ripple delete.
    ripple_scope: neoveg_engine::ripple::RippleScope,
    /// The last edit that left a gap or an overlap, and the history revision it left behind:
    /// `F` / `Ctrl+F` / `Ctrl+Shift+F` ripple it while it is still the latest change.
    post_ripple: Option<(neoveg_engine::ripple::RippleHint, u64)>,
    /// Speed of the J / K / L keys: `1.0` normal, negative backwards.
    shuttle_speed: f64,
    /// Open projects (one tab each) and the sub-projects of the active one.
    tabs: Tabs,
    project_path: Option<PathBuf>,
    status: String,

    loader_tx: Sender<LoadMsg>,
    loader_rx: Receiver<LoadMsg>,
}

/// The media of the project an EDL event refers to: by the source file name, else by the clip
/// name (with or without extension), else by the reel name.
fn match_edl_media(event: &EdlEvent, pool: &[MediaRef]) -> Option<MediaMatch> {
    let file_name = |path: &str| {
        path.rsplit(['/', '\\'])
            .next()
            .unwrap_or(path)
            .to_ascii_lowercase()
    };
    let stem = |name: &str| {
        let name = name.to_ascii_lowercase();
        match name.rsplit_once('.') {
            Some((stem, ext)) if ext.len() <= 4 => stem.to_owned(),
            _ => name,
        }
    };
    let found = pool.iter().find(|m| {
        let media_file = file_name(&m.path);
        let media_stem = stem(&media_file);
        event
            .source_file
            .as_deref()
            .is_some_and(|f| file_name(f) == media_file)
            || event
                .clip_name
                .as_deref()
                .is_some_and(|n| stem(n) == media_stem || n.eq_ignore_ascii_case(&media_file))
            || (event.clip_name.is_none()
                && edl::reel_name(&media_stem).eq_ignore_ascii_case(&event.reel))
    })?;
    Some(MediaMatch {
        media_id: found.id,
        duration: found.duration_seconds,
    })
}

/// The project media a clip of an FCP7 XML refers to: by full path, else by file name, else by the
/// clip name without its extension.
fn match_xml_media(clip: &XmlClip, pool: &[MediaRef]) -> Option<MediaMatch> {
    let file_name = |path: &str| {
        path.rsplit(['/', '\\'])
            .next()
            .unwrap_or(path)
            .to_ascii_lowercase()
    };
    let stem = |name: &str| {
        let name = name.to_ascii_lowercase();
        match name.rsplit_once('.') {
            Some((stem, ext)) if ext.len() <= 4 => stem.to_owned(),
            _ => name,
        }
    };
    let found = pool.iter().find(|m| {
        let media_file = file_name(&m.path);
        clip.path.as_deref().is_some_and(|p| {
            p.replace('\\', "/")
                .eq_ignore_ascii_case(&m.path.replace('\\', "/"))
        }) || clip
            .path
            .as_deref()
            .or(clip.file_name.as_deref())
            .is_some_and(|f| file_name(f) == media_file)
            || stem(&clip.name) == stem(&media_file)
    })?;
    Some(MediaMatch {
        media_id: found.id,
        duration: found.duration_seconds,
    })
}

/// What the startup screen says it is loading, in the order the steps run.
const STARTUP_STEPS: [&str; 6] = [
    "Preferências e atalhos de teclado",
    "Placa de vídeo e renderizador",
    "Saída de áudio",
    "Efeitos de áudio",
    "Decodificador de vídeo",
    "Projetos para recuperar",
];

impl AppState {
    // ---- FCP7 XML & FCPXML -----------------------------------------------------------------------

    fn xml_media_pool(&self) -> Vec<XmlMedia> {
        self.project
            .media_pool
            .iter()
            .map(|m| XmlMedia {
                id: m.id,
                name: self.media.get(&m.id).map_or_else(
                    || {
                        m.path
                            .rsplit(['/', '\\'])
                            .next()
                            .unwrap_or(&m.path)
                            .to_owned()
                    },
                    |l| l.name.clone(),
                ),
                path: m.path.clone(),
                duration: m.duration_seconds,
            })
            .collect()
    }

    /// File > Export AAF: the whole timeline for Avid, Premiere Pro, DaVinci Resolve or Pro Tools.
    fn export_aaf(&mut self) {
        let media = self.xml_media_pool();
        let settings = &self.project.settings;
        let project = aaf::AafProject {
            name: &self.project.name,
            fps: settings.fps,
            width: settings.width,
            height: settings.height,
            sample_rate: settings.sample_rate,
        };
        let (bytes, warnings) = match aaf::export(&self.timeline, &media, &project, false) {
            Ok(done) => done,
            Err(e) => {
                self.status = tr_args("Não foi possível exportar o AAF: {}", &[&e]);
                return;
            }
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("AAF (Advanced Authoring Format)", &["aaf"])
            .set_file_name(format!("{}.aaf", self.project.name))
            .save_file()
        else {
            return;
        };
        for warning in &warnings {
            tracing::warn!("[WARN] AAF: {warning}");
        }
        match std::fs::write(&path, bytes) {
            Ok(()) => {
                self.status = if warnings.is_empty() {
                    tr_args("AAF exportado: {}", &[&path.display()])
                } else {
                    tr_args(
                        "AAF exportado: {} ({} aviso(s): {})",
                        &[&path.display(), &warnings.len(), &warnings.join("; ")],
                    )
                };
            }
            Err(e) => {
                tracing::error!("[ERROR] cannot write {}: {e}", path.display());
                self.status = tr_args("Falha ao exportar o AAF: {}", &[&e]);
            }
        }
    }

    /// File > Import AAF: places the clips whose media is in the project, as one undo step.
    fn import_aaf(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("AAF (Advanced Authoring Format)", &["aaf"])
            .pick_file()
        else {
            return;
        };
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::error!("[ERROR] cannot read {}: {e}", path.display());
                self.status = tr_args("Não foi possível ler o AAF: {}", &[&e]);
                return;
            }
        };
        let doc = match aaf::parse(&bytes) {
            Ok(doc) => doc,
            Err(e) => {
                tracing::warn!("[WARN] {}: {e}", path.display());
                self.status = tr_args("Não foi possível abrir o AAF: {}", &[&e]);
                return;
            }
        };
        for warning in &doc.warnings {
            tracing::warn!("[WARN] {}: {warning}", path.display());
        }
        let pool = self.project.media_pool.clone();
        let imported =
            fcp7::import_into_timeline(&mut self.timeline, &doc, |c| match_xml_media(c, &pool));
        self.status = match imported {
            Some((command, report)) => {
                self.history.execute(&mut self.timeline, command);
                tr_args(
                    "AAF importado: {} clipe(s); {} sem mídia no projeto",
                    &[&report.added, &report.skipped.len()],
                )
            }
            None => "Nenhum clipe do AAF tem mídia no projeto: importe as mídias antes".into(),
        };
    }

    /// File > Export FCP7 XML: the whole timeline for Premiere Pro, DaVinci Resolve or Final Cut 7.
    fn export_fcp7(&mut self) {
        if self.timeline.tracks.iter().all(|t| t.events.is_empty()) {
            self.status = "A linha do tempo está vazia: nada para exportar".into();
            return;
        }
        let media = self.xml_media_pool();
        let text = fcp7::export(
            &self.timeline,
            &media,
            &self.project.name,
            self.project.settings.fps,
        );
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Final Cut Pro 7 XML", &["xml"])
            .set_file_name(format!("{}.xml", self.project.name))
            .save_file()
        else {
            return;
        };
        match std::fs::write(&path, text) {
            Ok(()) => self.status = tr_args("XML exportado: {}", &[&path.display()]),
            Err(e) => {
                tracing::error!("[ERROR] cannot write {}: {e}", path.display());
                self.status = tr_args("Falha ao exportar o XML: {}", &[&e]);
            }
        }
    }

    /// File > Import FCP7 XML: places the clips whose media is in the project, as one undo step.
    fn import_fcp7(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Final Cut Pro 7 XML", &["xml"])
            .pick_file()
        else {
            return;
        };
        let text = match std::fs::read(&path) {
            Ok(bytes) => captions::decode_text(&bytes),
            Err(e) => {
                tracing::error!("[ERROR] cannot read {}: {e}", path.display());
                self.status = tr_args("Não foi possível ler o XML: {}", &[&e]);
                return;
            }
        };
        let doc = match fcp7::parse(&text) {
            Ok(doc) => doc,
            Err(e) => {
                tracing::warn!("[WARN] {}: {e}", path.display());
                self.status = tr_args("Não foi possível abrir o XML: {}", &[&e]);
                return;
            }
        };
        for warning in &doc.warnings {
            tracing::warn!("[WARN] {}: {warning}", path.display());
        }
        let pool = self.project.media_pool.clone();
        let imported =
            fcp7::import_into_timeline(&mut self.timeline, &doc, |c| match_xml_media(c, &pool));
        self.status = match imported {
            Some((command, report)) => {
                self.history.execute(&mut self.timeline, command);
                tr_args(
                    "XML importado: {} clipe(s); {} sem mídia no projeto",
                    &[&report.added, &report.skipped.len()],
                )
            }
            None => "Nenhum clipe do XML tem mídia no projeto: importe as mídias antes".into(),
        };
    }

    /// File > Export FCPXML: Final Cut Pro `<fcpxml version="1.10">` with rational frame times.
    fn export_fcpxml(&mut self) {
        if self.timeline.tracks.iter().all(|t| t.events.is_empty()) {
            self.status = "A linha do tempo está vazia: nada para exportar".into();
            return;
        }
        let media = self.xml_media_pool();
        let s = &self.project.settings;
        let text = fcpxml::export(
            &self.timeline,
            &media,
            &self.project.name,
            s.fps,
            s.width,
            s.height,
            s.sample_rate,
        );
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Final Cut Pro FCPXML", &["fcpxml"])
            .set_file_name(format!("{}.fcpxml", self.project.name))
            .save_file()
        else {
            return;
        };
        match std::fs::write(&path, text) {
            Ok(()) => self.status = tr_args("FCPXML exportado: {}", &[&path.display()]),
            Err(e) => {
                tracing::error!("[ERROR] cannot write {}: {e}", path.display());
                self.status = tr_args("Falha ao exportar o FCPXML: {}", &[&e]);
            }
        }
    }

    /// File > Import FCPXML: places the clips and markers whose media is in the project, as one undo step.
    fn import_fcpxml(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Final Cut Pro FCPXML", &["fcpxml", "xml"])
            .pick_file()
        else {
            return;
        };
        let text = match std::fs::read(&path) {
            Ok(bytes) => captions::decode_text(&bytes),
            Err(e) => {
                tracing::error!("[ERROR] cannot read {}: {e}", path.display());
                self.status = tr_args("Não foi possível ler o FCPXML: {}", &[&e]);
                return;
            }
        };
        let doc = match fcpxml::parse(&text) {
            Ok(doc) => doc,
            Err(e) => {
                tracing::warn!("[WARN] {}: {e}", path.display());
                self.status = tr_args("Não foi possível abrir o FCPXML: {}", &[&e]);
                return;
            }
        };
        for warning in &doc.warnings {
            tracing::warn!("[WARN] {}: {warning}", path.display());
        }
        let pool = self.project.media_pool.clone();
        let imported =
            fcpxml::import_into_timeline(&mut self.timeline, &doc, |c| match_xml_media(c, &pool));
        self.status = match imported {
            Some((command, report)) => {
                self.history.execute(&mut self.timeline, command);
                self.last_frame_key = None;
                tr_args(
                    "FCPXML importado: {} clipe(s); {} sem mídia no projeto",
                    &[&report.added, &report.skipped.len()],
                )
            }
            None => "Nenhum clipe do FCPXML tem mídia no projeto: importe as mídias antes".into(),
        };
    }

    /// File > Export OTIO: OpenTimelineIO `.otio` JSON (`Timeline.1`, `Stack.1`, `Track.1`, `Clip.1`, `Transition.1`).
    fn export_otio(&mut self) {
        if self.timeline.tracks.iter().all(|t| t.events.is_empty()) {
            self.status = "A linha do tempo está vazia: nada para exportar".into();
            return;
        }
        let media = self.xml_media_pool();
        let s = &self.project.settings;
        let text = otio::export(
            &self.timeline,
            &media,
            &self.project.name,
            s.fps,
            s.width,
            s.height,
        );
        let Some(path) = rfd::FileDialog::new()
            .add_filter("OpenTimelineIO", &["otio"])
            .set_file_name(format!("{}.otio", self.project.name))
            .save_file()
        else {
            return;
        };
        match std::fs::write(&path, text) {
            Ok(()) => self.status = tr_args("OTIO exportado: {}", &[&path.display()]),
            Err(e) => {
                tracing::error!("[ERROR] cannot write {}: {e}", path.display());
                self.status = tr_args("Falha ao exportar o OTIO: {}", &[&e]);
            }
        }
    }

    /// File > Import OTIO: imports OpenTimelineIO `.otio` clips, transitions, time-warps and markers in one undo step.
    fn import_otio(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("OpenTimelineIO", &["otio", "json"])
            .pick_file()
        else {
            return;
        };
        let text = match std::fs::read(&path) {
            Ok(bytes) => captions::decode_text(&bytes),
            Err(e) => {
                tracing::error!("[ERROR] cannot read {}: {e}", path.display());
                self.status = tr_args("Não foi possível ler o OTIO: {}", &[&e]);
                return;
            }
        };
        let doc = match otio::parse(&text) {
            Ok(doc) => doc,
            Err(e) => {
                tracing::warn!("[WARN] {}: {e}", path.display());
                self.status = tr_args("Não foi possível abrir o OTIO: {}", &[&e]);
                return;
            }
        };
        for warning in &doc.warnings {
            tracing::warn!("[WARN] {}: {warning}", path.display());
        }
        let pool = self.project.media_pool.clone();
        let imported =
            otio::import_into_timeline(&mut self.timeline, &doc, |c| match_xml_media(c, &pool));
        self.status = match imported {
            Some((command, report)) => {
                self.history.execute(&mut self.timeline, command);
                self.last_frame_key = None;
                tr_args(
                    "OTIO importado: {} clipe(s); {} sem mídia no projeto",
                    &[&report.added, &report.skipped.len()],
                )
            }
            None => "Nenhum clipe do OTIO tem mídia no projeto: importe as mídias antes".into(),
        };
    }

    // ---- edit decision lists -----------------------------------------------------------------

    /// File > Export EDL: the cuts of the selected track as a CMX 3600 list.
    fn export_edl(&mut self) {
        let Some(track_id) = self.selected_track else {
            self.status = "Selecione uma faixa para exportar a EDL dela".into();
            return;
        };
        let pool = self.project.media_pool.clone();
        let names: HashMap<u64, String> = self
            .media
            .iter()
            .map(|(id, m)| (*id, m.name.clone()))
            .collect();
        let clip_info = |media: Option<u64>| {
            let id = media?;
            let item = pool.iter().find(|m| m.id == id)?;
            let name = names.get(&id).cloned().unwrap_or_else(|| {
                item.path
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or(&item.path)
                    .to_owned()
            });
            Some(edl::ClipInfo {
                name,
                path: Some(item.path.clone()),
            })
        };
        let Some(text) = edl::export_track(
            &self.timeline,
            track_id,
            &self.project.name,
            self.project.settings.fps,
            1,
            clip_info,
        ) else {
            self.status = "A faixa selecionada não tem eventos para exportar".into();
            return;
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Lista de edição CMX 3600", &["edl"])
            .set_file_name(format!("{}.edl", self.project.name))
            .save_file()
        else {
            return;
        };
        match std::fs::write(&path, text) {
            Ok(()) => self.status = tr_args("EDL exportada: {}", &[&path.display()]),
            Err(e) => {
                tracing::error!("[ERROR] cannot write {}: {e}", path.display());
                self.status = tr_args("Falha ao exportar a EDL: {}", &[&e]);
            }
        }
    }

    /// File > Import EDL: places the events whose media is in the project, as one undo step.
    fn import_edl(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Lista de edição CMX 3600", &["edl"])
            .pick_file()
        else {
            return;
        };
        let text = match std::fs::read(&path) {
            Ok(bytes) => captions::decode_text(&bytes),
            Err(e) => {
                tracing::error!("[ERROR] cannot read {}: {e}", path.display());
                self.status = tr_args("Não foi possível ler a EDL: {}", &[&e]);
                return;
            }
        };
        let doc = match edl::parse(&text, self.project.settings.fps) {
            Ok(doc) => doc,
            Err(e) => {
                self.status = tr_args("Não foi possível abrir a EDL: {}", &[&e]);
                return;
            }
        };
        for warning in &doc.warnings {
            tracing::warn!("[WARN] {}: {warning}", path.display());
        }
        let pool = self.project.media_pool.clone();
        let imported =
            edl::import_into_timeline(&mut self.timeline, &doc, |e| match_edl_media(e, &pool));
        self.status = match imported {
            Some((command, report)) => {
                self.history.execute(&mut self.timeline, command);
                tr_args(
                    "EDL importada: {} evento(s); {} sem mídia no projeto{}",
                    &[
                        &report.added,
                        &report.skipped.len(),
                        &if doc.warnings.is_empty() {
                            String::new()
                        } else {
                            format!("; {} aviso(s) no log", doc.warnings.len())
                        },
                    ],
                )
            }
            None => "Nenhum evento da EDL tem mídia no projeto: importe as mídias antes".into(),
        };
    }

    // ---- subtitles ----------------------------------------------------------------------------

    /// File > Import subtitles: adds the cues of an SRT or WebVTT file as one undo step.
    fn import_captions(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Legendas (SRT, WebVTT)", &["srt", "vtt"])
            .pick_file()
        else {
            return;
        };
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::error!("[ERROR] cannot read {}: {e}", path.display());
                self.status = tr_args("Não foi possível ler as legendas: {}", &[&e]);
                return;
            }
        };
        let extension = path.extension().and_then(|e| e.to_str());
        match captions::parse_bytes(&bytes, None, extension) {
            Ok((format, doc)) => {
                for warning in &doc.warnings {
                    tracing::warn!("[WARN] {}: {warning}", path.display());
                }
                let count = doc.cues.len();
                match captions::import_document(&mut self.timeline, &doc, 0.0, false) {
                    Some(command) => {
                        self.history.execute(&mut self.timeline, command);
                        self.status = tr_args(
                            "{} legendas importadas ({}){}",
                            &[
                                &count,
                                &format.name(),
                                &if doc.warnings.is_empty() {
                                    String::new()
                                } else {
                                    format!("; {} aviso(s) no log", doc.warnings.len())
                                },
                            ],
                        );
                    }
                    None => self.status = "O arquivo não tem nenhuma legenda utilizável".into(),
                }
            }
            Err(e) => {
                tracing::error!("[ERROR] {}: {e}", path.display());
                self.status = tr_args("Não foi possível abrir as legendas: {}", &[&e]);
            }
        }
    }

    /// File > Export subtitles: writes the captions as SRT or WebVTT, by the file's extension.
    fn export_captions(&mut self) {
        if self.timeline.captions.is_empty() {
            self.status = "Não há legendas para exportar".into();
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .add_filter("SubRip (SRT)", &["srt"])
            .add_filter("WebVTT", &["vtt"])
            .set_file_name(format!("{}.srt", self.project.name))
            .save_file()
        else {
            return;
        };
        let format = path
            .extension()
            .and_then(|e| e.to_str())
            .and_then(captions::Format::from_extension)
            .unwrap_or(captions::Format::Srt);
        let text = captions::write(&captions::export_document(&self.timeline), format);
        match std::fs::write(&path, text) {
            Ok(()) => {
                self.status = tr_args(
                    "{} legendas exportadas ({})",
                    &[&self.timeline.captions.len(), &format.name()],
                )
            }
            Err(e) => {
                tracing::error!("[ERROR] cannot write {}: {e}", path.display());
                self.status = tr_args("Falha ao exportar as legendas: {}", &[&e]);
            }
        }
    }

    /// Runs what the user did in the subtitles tab; each action is one undo step.
    fn apply_caption_actions(&mut self, actions: Vec<CaptionAction>, now: f64) {
        for action in actions {
            let command = match action {
                CaptionAction::Add => captions::add_caption(
                    &mut self.timeline,
                    self.playhead,
                    captions_panel::NEW_CAPTION_SECONDS,
                    "",
                ),
                CaptionAction::Remove(id) => captions::remove_caption(&self.timeline, id),
                CaptionAction::SetText(id, text) => {
                    captions::set_caption_text(&self.timeline, id, &text)
                }
                CaptionAction::SetTimes(id, start, end) => {
                    captions::set_caption_times(&self.timeline, id, start, end)
                }
                CaptionAction::Seek(time) => {
                    self.playhead = time;
                    self.blink_origin = now;
                    None
                }
                CaptionAction::Import => {
                    self.import_captions();
                    None
                }
                CaptionAction::Export => {
                    self.export_captions();
                    None
                }
                CaptionAction::Clear => captions::clear_captions(&self.timeline),
                CaptionAction::PickWhisperModel => {
                    if let Some(dir) = rfd::FileDialog::new()
                        .set_title("Selecionar pasta do modelo Whisper (config.json, tokenizer.json, model.safetensors/.gguf)")
                        .pick_folder()
                    {
                        self.status = tr_args("Modelo Whisper selecionado: {}", &[&dir.display()]);
                        self.captions_panel.whisper_model_dir = Some(dir);
                        self.captions_panel.whisper_status = None;
                    }
                    None
                }
                CaptionAction::StartWhisper => {
                    let clips = self.build_clips();
                    let dur = neoveg_audio::mixer::plan_end(&clips);
                    let repaint = self.tabs.ctx.clone();
                    match self.captions_panel.start_whisper(clips, dur, move || {
                        if let Some(ctx) = &repaint {
                            ctx.request_repaint();
                        }
                    }) {
                        Ok(()) => {
                            self.status = "Transcrevendo fala com Whisper em segundo plano…".into();
                        }
                        Err(err) => {
                            self.status = err;
                        }
                    }
                    None
                }
                CaptionAction::ApplyWhisper {
                    segments,
                    replace_all,
                } => {
                    let mut doc = captions::Document {
                        cues: segments
                            .into_iter()
                            .map(|seg| captions::Cue {
                                start: seg.start_s,
                                end: seg.end_s,
                                text: seg.text,
                                ..Default::default()
                            })
                            .collect(),
                        ..Default::default()
                    };
                    doc.normalize();
                    let count = doc.cues.len();
                    let cmd = captions::import_document(&mut self.timeline, &doc, 0.0, replace_all);
                    if cmd.is_some() {
                        self.status = tr_args("Whisper: {} legenda(s) adicionada(s).", &[&count]);
                    }
                    cmd
                }
            };
            if let Some(command) = command {
                self.history.execute(&mut self.timeline, command);
            }
        }
    }

    /// Startup step 1: the user's preferences and keyboard map.
    fn load_preferences(&mut self) -> String {
        let prefs = Preferences::default_path()
            .map(|p| Preferences::load(&p))
            .unwrap_or_default();
        let (keymap, warnings) = prefs.key_map();
        for w in warnings {
            tracing::warn!("[WARN] keymap: {w}");
        }
        let overrides = prefs.keymap.len();
        let detail = tr_args(
            "Idioma {}; {} atalhos personalizados; salvamento automático a cada {} min",
            &[&prefs.language, &overrides, &prefs.autosave_minutes],
        );
        self.prefs = prefs;
        self.keymap = keymap;
        self.snap_sources = neoveg_engine::snapping::SnapSources {
            grid: self.prefs.snap_to_grid,
            markers: self.prefs.snap_to_markers,
            all_events: self.prefs.snap_to_all_events,
        };
        self.quantize_frames = self.prefs.quantize_to_frames;
        self.persist_prefs = true;
        // New projects start with the settings the user chose to keep
        let defaults = self.prefs.new_project_settings.clone().unwrap_or_default();
        if self.is_blank_project() {
            self.project.settings = defaults.clone();
        }
        if self.prefs.show_welcome_on_startup {
            self.welcome.open_with(&defaults);
        }
        detail
    }

    /// Startup step 3: the sound card the audio will play on.
    fn probe_audio(&mut self) -> String {
        match neoveg_audio::playback::probe_output() {
            Ok(info) => tr_args(
                "{} · {} Hz · {} canais",
                &[&info.device, &info.sample_rate, &info.channels],
            ),
            Err(e) => {
                tracing::warn!("[WARN] audio output: {e}");
                self.status = tr_args("Saída de áudio indisponível: {}", &[&e]);
                "Sem saída de áudio: o projeto abre sem som".into()
            }
        }
    }

    /// Startup step 5: the worker thread that decodes video frames.
    fn start_frame_worker(&mut self, ctx: &egui::Context) -> String {
        let repaint = ctx.clone();
        match FrameWorker::spawn(move || repaint.request_repaint()) {
            Ok(worker) => {
                self.frame_worker = Some(worker);
                "Pronto".into()
            }
            Err(e) => {
                tracing::warn!("[WARN] {e}");
                self.status = e.clone();
                e
            }
        }
    }

    /// Startup step 6: recovery files left by a session that did not close cleanly.
    fn find_recoveries(&mut self) -> String {
        let Some(dir) = autosave_dir() else {
            return "Pasta de recuperação indisponível".into();
        };
        autosave::prune_older_than(&dir, Duration::from_secs(30 * 86_400));
        self.pending_recovery = autosave::list_recoveries(&dir);
        if self.pending_recovery.is_empty() {
            return "Nenhum projeto para recuperar".into();
        }
        tracing::warn!(
            "[WARN] {} unsaved project(s) found for recovery",
            self.pending_recovery.len()
        );
        tr_args(
            "{} projeto(s) não salvo(s) encontrado(s)",
            &[&self.pending_recovery.len()],
        )
    }

    fn new() -> Self {
        // Defaults only: the user's preferences are read by the first startup step
        let prefs = Preferences::default();
        let keymap = prefs.key_map().0;
        let (loader_tx, loader_rx) = channel();
        Self {
            project: ProjectInfo::create_empty(),
            timeline: Timeline::new(),
            history: History::new(),
            prefs,
            cjk_fonts: Default::default(),
            dock_footer: Default::default(),
            keymap,
            media: HashMap::new(),
            stretch_cache: std::cell::RefCell::new(HashMap::new()),
            playhead: 0.0,
            playing: false,
            blink_origin: 0.0,
            pixels_per_second: 100.0,
            scroll_seconds: 0.0,
            scroll_y: 0.0,
            selected_track: None,
            selection: Selection::new(),
            time_selection: None,
            range_anchor: None,
            clipboard: None,
            repeat_dialog: None,
            attributes_dialog: None,
            missing: Vec::new(),
            missing_open: false,
            deep_search: None,
            found: Vec::new(),
            found_open: false,
            media_errors: std::collections::VecDeque::new(),
            ignore_media_errors: false,
            convert_offer: None,
            goto_dialog: None,
            insert_time_dialog: None,
            notes: Default::default(),
            show_details: false,
            edit_tool: EditTool::default(),
            welcome: Default::default(),
            project_dialog: Default::default(),
            persist_prefs: false,
            match_media_rx: None,
            lanes_width: 800.0,
            meter: Arc::new(PeakMeter::new()),
            loudness: Arc::new(LoudnessReadout::new()),
            loudness_target: 0,
            show_credits: false,
            log_viewer: Default::default(),
            generator_dialog: Default::default(),
            slideshow: Default::default(),
            format_chosen: false,
            render_dialog: Default::default(),
            captions_panel: CaptionsPanelState::default(),
            fx: FxPanelState::default(),
            vfx: VfxPanelState::default(),
            transitions: TransitionsPanelState::default(),
            vfx_draft: None,
            pan_crop: PanCropWindow::default(),
            pan_crop_draft: None,
            velocity_draft: None,
            velocity_editor: VelocityEditor::default(),
            mixer: MixerPanelState::default(),
            master_volume: 1.0,
            active_player: None,
            show_bars: false,
            frame_scopes: None,
            frame_scopes_version: 0,
            bars_scopes: None,
            scopes_panel: ScopesPanelState::default(),
            bars_texture: None,
            quality: 4,
            show_preferences: false,
            prefs_tab: PrefsTab::General,
            theme_applied: None,
            theme_note: String::new(),
            show_keymap: false,
            keymap_filter: String::new(),
            capture_for: None,
            keymap_message: String::new(),
            show_inspector: false,
            metronome_on: false,
            recorder: None,
            record_start: 0.0,
            metronome_volume: 0.5,
            show_metronome: false,
            video_info: HashMap::new(),
            frame_worker: None,
            video_texture: None,
            last_frame_key: None,
            last_composite_key: None,
            lut_cache: None,
            last_lut_path: None,
            last_grade: Grade::default(),
            show_grade: false,
            grade_draft: None,
            last_chroma: ChromaKey::default(),
            show_chroma: false,
            chroma_draft: None,
            bars_grade: None,
            bars_scopes_rev: 0,
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
            env_drag: None,
            context_event: None,
            context_envelope: None,
            context_mark: None,
            name_edit: None,
            mark_drag: None,
            ignore_grouping: false,
            drag_ignores_group: false,
            marquee: None,
            snapping: true,
            snap_sources: Default::default(),
            quantize_frames: true,
            looping: false,
            loop_region: None,
            media_tab: 0,
            trimmer: None,
            trimmer_overwrite: false,
            trimmer_drag_from: None,
            window_request: None,
            pending_context_click: None,
            layouts_dir: crate::layouts::dir(),
            layouts: Vec::new(),
            layouts_loaded: false,
            current_layout: None,
            layout_dialog: None,
            organize_layouts: false,
            layout_chord: None,
            auto_ripple: false,
            trim_edge: None,
            ripple_scope: Default::default(),
            post_ripple: None,
            shuttle_speed: 1.0,
            tabs: Tabs::default(),
            project_path: None,
            status: "Projeto vazio".into(),
            loader_tx,
            loader_rx,
        }
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
                self.restart_audio_if_playing();
                self.last_frame_key = None;
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
    /// Deletes the selected events and moves the later ones of each track earlier to close the gap.
    fn ripple_delete_selection(&mut self) {
        let refs = self.selection.refs();
        if refs.is_empty() {
            self.status = "Selecione os eventos a excluir".into();
            return;
        }
        let outcome =
            neoveg_engine::ripple::ripple_delete_scoped(&self.timeline, &refs, self.ripple_scope);
        self.apply_outcome(outcome);
        self.selection.clear();
    }

    fn toggle_text(name: &str, on: bool) -> String {
        format!("{name}: {}", if on { "ligado" } else { "desligado" })
    }

    /// Chooses what ripple moves along with the edited tracks.
    fn set_ripple_scope(&mut self, scope: RippleScope) {
        self.ripple_scope = scope;
        self.status = tr_args("Escopo do ripple: {}", &[&scope.label()]);
    }

    /// Remembers the gap or overlap the edit that just ran left, for a manual ripple.
    fn remember_post_ripple(&mut self, hint: Option<neoveg_engine::ripple::RippleHint>) {
        self.post_ripple = hint.map(|h| (h, self.history.revision()));
    }

    /// `F` / `Ctrl+F` / `Ctrl+Shift+F`: picks the scope and, when the last edit is still the latest
    /// change and left a gap or an overlap, ripples it now.
    fn ripple_key(&mut self, scope: RippleScope) {
        self.set_ripple_scope(scope);
        let Some((hint, revision)) = self.post_ripple.take() else {
            return;
        };
        if revision != self.history.revision() {
            return;
        }
        let outcome = hint.outcome(&self.timeline, scope);
        if self.apply_outcome(outcome).is_some() {
            self.status = tr_args("Ripple aplicado à última edição ({})", &[&scope.label()]);
        }
    }

    fn delete_selection(&mut self) {
        let refs = self.selection.refs();
        if self.auto_ripple && self.time_selection.is_none() && !refs.is_empty() {
            self.ripple_delete_selection();
            return;
        }
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
        let hint = (self.time_selection.is_none())
            .then(|| neoveg_engine::ripple::removal_hint(&self.timeline, &refs))
            .flatten();
        if self.apply_outcome(outcome).is_some() {
            self.remember_post_ripple(hint);
        }
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
                self.status = tr_args("{} evento(s) copiado(s)", &[&clip.items.len()]);
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
                tr_args("{} evento(s) colado(s)", &[&touched.len()])
            } else {
                tr_args(
                    "{} evento(s) colado(s); {} sem faixa do mesmo tipo",
                    &[&touched.len(), &skipped],
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

    /// `true` when the media the event plays is a generated one (a `.neoveggen` manifest).
    fn is_generated_event(&self, ev: &TrackEvent) -> bool {
        ev.media_id
            .and_then(|id| self.project.media_pool.iter().find(|m| m.id == id))
            .is_some_and(|m| {
                neoveg_video::generators::is_generator_manifest_path(std::path::Path::new(&m.path))
            })
    }

    /// The header button of an event was clicked: it selects the event and does what the button says.
    fn header_button_clicked(
        &mut self,
        track_id: TrackId,
        event_id: EventId,
        button: event_header::HeaderButton,
        center: Pos2,
    ) {
        use event_header::HeaderButton;
        self.selected_track = Some(track_id);
        if !self.selection.contains((track_id, event_id)) {
            self.click_select((track_id, event_id));
        }
        let video = self
            .timeline
            .track(track_id)
            .is_some_and(|t| t.kind == TrackKind::Video);
        match button {
            HeaderButton::Menu => self.pending_context_click = Some(center),
            HeaderButton::PanCrop => self.open_pan_crop(track_id, event_id),
            HeaderButton::Fx if video => {
                self.vfx.scope = VfxScope::Event;
                self.window_request = Some(WindowAction::Show(Tab::VideoFx));
            }
            HeaderButton::Fx => {
                self.fx.master = false;
                self.fx.bus = None;
                self.window_request = Some(WindowAction::Show(Tab::AudioFx));
            }
            HeaderButton::GeneratedMedia => self.edit_generated_media(track_id, event_id),
        }
    }

    /// Opens the generator dialog on the generated media the event plays.
    fn edit_generated_media(&mut self, track_id: TrackId, event_id: EventId) {
        let media_id = self
            .timeline
            .track(track_id)
            .and_then(|t| t.events.iter().find(|e| e.id == event_id))
            .and_then(|e| e.media_id);
        let Some((id, path)) = media_id.and_then(|id| {
            self.project
                .media_pool
                .iter()
                .find(|m| m.id == id)
                .map(|m| (id, PathBuf::from(&m.path)))
        }) else {
            return;
        };
        match neoveg_video::generators::GeneratorSpec::load_manifest(&path) {
            Ok(spec) => self.generator_dialog.edit(id, &spec),
            Err(e) => self.status = tr_args("Não foi possível abrir a mídia gerada: {}", &[&e]),
        }
    }

    /// The edited spec of a generated media: written as a new manifest (the render cache is keyed by the
    /// file, so a new name cannot show an old picture), and the media entry points at it from now on.
    fn apply_generated_edit(
        &mut self,
        media_id: u64,
        spec: neoveg_video::generators::GeneratorSpec,
        ctx: &egui::Context,
    ) {
        let Some(old) = self
            .project
            .media_pool
            .iter()
            .find(|m| m.id == media_id)
            .map(|m| PathBuf::from(&m.path))
        else {
            return;
        };
        let dir = old
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let stem: String = spec
            .name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        let path = dir.join(format!("{stem}_{}.neoveggen", autosave::unix_now()));
        match spec.save_manifest(&path) {
            Ok(()) => {
                relink::relink_to(&mut self.project.media_pool, media_id, &path);
                self.forced_dirty = true;
                self.last_frame_key = None;
                self.last_composite_key = None;
                self.spawn_media(path, Some(media_id), ctx);
                self.status = tr_args("Mídia gerada {} atualizada", &[&spec.name]);
            }
            Err(e) => self.status = tr_args("Não foi possível gravar a mídia gerada: {}", &[&e]),
        }
    }

    /// *Inserir barramento de áudio*: the next bus (up to 26), shown in the effects panel.
    fn add_audio_bus(&mut self) {
        let id = self.timeline.alloc_id();
        let Some(command) = neoveg_engine::buses::add_bus(&self.timeline, id) else {
            self.status = "O projeto já tem os 26 barramentos".into();
            return;
        };
        if self.history.execute(&mut self.timeline, command) {
            self.fx.master = false;
            self.fx.bus = Some(id);
            self.restart_audio_if_playing();
            self.status = tr_args(
                "{} inserido: escolha-o como saída de uma faixa ou use um send",
                &[&neoveg_engine::buses::label(&self.timeline, id)],
            );
        }
    }

    /// Runs what the user did on a bus strip. Faders and pan follow live; the rest is one undo step.
    fn apply_bus_actions(&mut self, actions: Vec<crate::buses_panel::BusAction>) {
        use crate::buses_panel::BusAction;
        for action in actions {
            let command = match action {
                BusAction::Draft(id, db, pan) => {
                    self.fx.bus_mix_draft = Some((id, db, pan));
                    self.push_bus_mix_live(id, db, pan);
                    None
                }
                BusAction::Mix(id, db, pan) => {
                    self.fx.bus_mix_draft = None;
                    self.timeline
                        .buses
                        .iter()
                        .find(|b| b.id == id)
                        .and_then(|b| {
                            let mut after = b.clone();
                            after.mix.volume_db = db;
                            after.mix.pan = pan;
                            neoveg_engine::buses::set_bus(&self.timeline, after)
                        })
                }
                BusAction::Rename(id, name) => self
                    .timeline
                    .buses
                    .iter()
                    .find(|b| b.id == id)
                    .and_then(|b| {
                        let mut after = b.clone();
                        after.name = name;
                        neoveg_engine::buses::set_bus(&self.timeline, after)
                    }),
                BusAction::Mute(id, on) => self
                    .timeline
                    .buses
                    .iter()
                    .find(|b| b.id == id)
                    .and_then(|b| {
                        let mut after = b.clone();
                        after.mix.muted = on;
                        neoveg_engine::buses::set_bus(&self.timeline, after)
                    }),
                BusAction::Phase(id, on) => self
                    .timeline
                    .buses
                    .iter()
                    .find(|b| b.id == id)
                    .and_then(|b| {
                        let mut after = b.clone();
                        after.mix.invert_phase = on;
                        neoveg_engine::buses::set_bus(&self.timeline, after)
                    }),
                BusAction::Output(id, output) => self
                    .timeline
                    .buses
                    .iter()
                    .find(|b| b.id == id)
                    .and_then(|b| {
                        let mut after = b.clone();
                        after.output = output;
                        neoveg_engine::buses::set_bus(&self.timeline, after)
                    }),
                BusAction::Remove(id) => {
                    if self.fx.bus == Some(id) {
                        self.fx.bus = None;
                    }
                    neoveg_engine::buses::remove_bus(&self.timeline, id)
                }
            };
            if let Some(command) = command {
                if self.history.execute(&mut self.timeline, command) {
                    self.mixing_changed();
                }
            }
        }
    }

    /// Runs what the user did on the routing of audio track `track_id`.
    fn apply_route_actions(
        &mut self,
        track_id: TrackId,
        actions: Vec<crate::buses_panel::RouteAction>,
    ) {
        use crate::buses_panel::RouteAction;
        for action in actions {
            let Some(track) = self.timeline.track(track_id).cloned() else {
                return;
            };
            let (mut output, mut sends) = (track.output, track.sends.clone());
            match action {
                RouteAction::Output(to) => output = to,
                RouteAction::AddSend(bus) => sends.push(crate::buses_panel::new_send(bus)),
                RouteAction::RemoveSend(bus) => sends.retain(|s| s.bus != bus),
                RouteAction::SendDraft(bus, db) => {
                    self.push_send_live(track_id, bus, db);
                    continue;
                }
                RouteAction::SendLevel(bus, db) => {
                    if let Some(s) = sends.iter_mut().find(|s| s.bus == bus) {
                        s.level_db = db;
                    }
                }
                RouteAction::SendPost(bus, on) => {
                    if let Some(s) = sends.iter_mut().find(|s| s.bus == bus) {
                        s.post_volume = on;
                    }
                }
                RouteAction::SendLinkPan(bus, on) => {
                    if let Some(s) = sends.iter_mut().find(|s| s.bus == bus) {
                        s.link_pan = on;
                    }
                }
            }
            if let Some(command) =
                neoveg_engine::buses::set_track_route(&self.timeline, track_id, output, sends)
            {
                if self.history.execute(&mut self.timeline, command) {
                    self.mixing_changed();
                }
            }
        }
    }

    /// The routing or a bus changed: the mixer is rebuilt at the cursor when playing.
    fn mixing_changed(&mut self) {
        self.restart_audio_if_playing();
    }

    /// Sends a bus fader / pan to the player while it is being dragged (`db`, `pan`).
    fn push_bus_mix_live(&self, id: neoveg_engine::BusId, db: f32, pan: f32) {
        let (Some(active), Some(index)) = (
            &self.active_player,
            self.timeline.buses.iter().position(|b| b.id == id),
        ) else {
            return;
        };
        let muted = self.timeline.buses[index].mix.muted;
        let invert = self.timeline.buses[index].mix.invert_phase;
        let mix = neoveg_engine::TrackMix {
            volume_db: db,
            pan,
            ..Default::default()
        };
        active.player.set_user_bus_mix(
            index,
            neoveg_audio::fxmixer::TrackMixSpec {
                gain: if muted { 0.0 } else { mix.gain() },
                pan,
                invert,
            },
        );
    }

    /// Sends the level of a send to the player while it is being dragged.
    fn push_send_live(&self, track_id: TrackId, bus: neoveg_engine::BusId, db: f32) {
        let Some(active) = &self.active_player else {
            return;
        };
        let Some(track_index) = self.timeline.tracks.iter().position(|t| t.id == track_id) else {
            return;
        };
        let Some(send_index) = self.timeline.tracks[track_index]
            .sends
            .iter()
            .position(|s| s.bus == bus)
        else {
            return;
        };
        active
            .player
            .set_send_gain(track_index, send_index, crate::fx_panel::send_gain(db));
    }

    /// *Inserir evento de ajuste*: an empty event on the selected video track at the cursor; the video
    /// tracks below it take its effects. It is selected so the effects panel shows its chain.
    fn insert_adjustment_event(&mut self) {
        let Some(track_id) = self.selected_track.filter(|id| {
            self.timeline
                .track(*id)
                .is_some_and(|t| t.kind == TrackKind::Video)
        }) else {
            self.status = "Selecione uma faixa de vídeo (de preferência uma nova, no topo)".into();
            return;
        };
        let id = self.timeline.alloc_id();
        let Some(command) =
            neoveg_engine::adjustment::insert_command(&self.timeline, track_id, self.playhead, id)
        else {
            return;
        };
        if self.history.execute(&mut self.timeline, command) {
            self.selection.select_only((track_id, id));
            self.last_frame_key = None;
            self.last_composite_key = None;
            self.status =
                "Evento de ajuste inserido: aplique efeitos na aba Efeitos de Vídeo".into();
        }
    }

    /// *Colar atributos do evento*: the first event on the clipboard lends its attributes to the
    /// selected events (one undo step).
    fn paste_attributes(&mut self, set: Attributes) {
        let Some(source) = self
            .clipboard
            .as_ref()
            .and_then(|c| c.items.first())
            .map(|item| item.event.clone())
        else {
            self.status = "A área de transferência está vazia".into();
            return;
        };
        let targets = self.selection.refs();
        if targets.is_empty() {
            self.status = "Selecione os eventos que vão receber os atributos".into();
            return;
        }
        match attributes::paste_command(&self.timeline, &source, &targets, set) {
            Some((command, count)) => {
                if self.history.execute(&mut self.timeline, command) {
                    self.last_frame_key = None;
                    self.last_composite_key = None;
                    self.restart_audio_if_playing();
                    self.status = tr_args("Atributos colados em {} evento(s)", &[&count]);
                }
            }
            None => self.status = "Nada para colar: os atributos já são os mesmos".into(),
        }
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

    /// `[` / `]`: selects the previous / next edit edge (from the cursor when none is selected) and
    /// puts the cursor on it.
    fn select_trim_edge(&mut self, forward: bool) {
        let edge = neoveg_engine::keytrim::step_edge(
            &self.timeline,
            &[],
            self.trim_edge,
            forward,
            self.playhead,
        );
        let Some(edge) = edge else {
            self.status = "Não há bordas de eventos para aparar".into();
            return;
        };
        if let Some(time) = neoveg_engine::keytrim::edge_time(&self.timeline, edge) {
            self.playhead = time;
        }
        self.trim_edge = Some(edge);
        self.selected_track = Some(edge.track);
        self.selection.select_only((edge.track, edge.event));
        self.status = tr_args(
            "Aparo por teclado: {} do evento — ← / → movem um quadro (Esc sai)",
            &[&match edge.side {
                neoveg_engine::keytrim::EdgeSide::Start => "início",
                neoveg_engine::keytrim::EdgeSide::End => "fim",
            }],
        );
    }

    /// Moves the selected edge `frames` frames (negative = earlier). Without an explicit `mode` the
    /// edit tool picks it: Slip and Slide nudge the media or the event, any other tool trims the edge.
    fn nudge_trim_edge(&mut self, frames: f64, mode: Option<neoveg_engine::keytrim::TrimMode>) {
        use neoveg_engine::keytrim::{nudge, TrimMode};
        let Some(edge) = self.trim_edge else {
            self.status = "Escolha uma borda com [ ou ] para aparar pelo teclado".into();
            return;
        };
        let fps = self.project.settings.fps.max(1.0);
        let mode = mode.unwrap_or(match self.edit_tool {
            EditTool::Slip => TrimMode::Slip,
            EditTool::Slide => TrimMode::Slide,
            EditTool::Normal | EditTool::Shuffle => TrimMode::Normal,
        });
        let outcome = nudge(
            &self.timeline,
            edge,
            frames / fps,
            mode,
            1.0 / fps,
            &|track, event| self.source_len(track, event),
        );
        if outcome.is_none() {
            self.status =
                "A borda não pode ir mais longe (mídia, evento bloqueado, 1 quadro ou sem vizinho colado)".into();
            return;
        }
        self.apply_outcome(outcome);
        if let Some(time) = neoveg_engine::keytrim::edge_time(&self.timeline, edge) {
            self.playhead = time;
        }
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
    /// Puts the cursor at `time` and scrolls the timeline when it would be off screen.
    fn go_to_time(&mut self, time: f64, now: f64) {
        self.playhead = time.max(0.0);
        self.blink_origin = now;
        let visible = f64::from(self.lanes_width.max(50.0)) / self.pixels_per_second;
        if self.playhead < self.scroll_seconds || self.playhead > self.scroll_seconds + visible {
            self.scroll_seconds = (self.playhead - visible * 0.2).max(0.0);
        }
    }

    /// `Home` / `End`: the start / end of the time selection, else of the visible area.
    fn go_to_selection_edge(&mut self, end: bool, now: f64) {
        let visible = f64::from(self.lanes_width.max(50.0)) / self.pixels_per_second;
        let target = match (self.time_selection, end) {
            (Some(range), false) => range.start,
            (Some(range), true) => range.end,
            (None, false) => self.scroll_seconds,
            (None, true) => self.scroll_seconds + visible,
        };
        self.playhead = target.max(0.0);
        self.blink_origin = now;
    }

    /// `Ctrl+Alt+Left` / `Ctrl+Alt+Right`: the previous / next event or fade edge.
    fn jump_to_edit_point(&mut self, forward: bool, now: f64) {
        let target = if forward {
            navigation::next_edit_point(&self.timeline, self.playhead)
        } else {
            navigation::previous_edit_point(&self.timeline, self.playhead)
        };
        match target {
            Some(t) => self.go_to_time(t, now),
            None => {
                self.status = if forward {
                    "Não há ponto de edição adiante".into()
                } else {
                    "Não há ponto de edição antes".into()
                };
            }
        }
    }

    /// `\`: scrolls so the cursor is in the middle of the view.
    fn center_view_on_cursor(&mut self) {
        let visible = f64::from(self.lanes_width.max(50.0)) / self.pixels_per_second;
        self.scroll_seconds = (self.playhead - visible / 2.0).max(0.0);
    }

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

    /// `1`–`9` and `0` (the tenth): cursor to the n-th marker by time.
    fn jump_to_marker_number(&mut self, n: usize, now: f64) {
        match markers::marker_by_number(&self.timeline, n) {
            Some(marker) => {
                self.playhead = marker.time;
                self.blink_origin = now;
            }
            None => self.status = tr_args("Não há marcador {}", &[&n]),
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
            Command::FileCreateSubproject => has_events,
            Command::EditUndo => self.history.can_undo(),
            Command::EditRedo => self.history.can_redo(),
            Command::EditCut
            | Command::EditCopy
            | Command::EditDelete
            | Command::EditSelectNone => has_events || has_range,
            Command::EditPaste | Command::EditPasteInsert | Command::EditPasteRepeat => {
                self.clipboard.is_some()
            }
            Command::EditPasteAttributes | Command::EditPasteAttributesSelective => {
                self.clipboard.is_some() && has_events
            }
            Command::InsertAdjustmentEvent => self
                .selected_track
                .and_then(|id| self.timeline.track(id))
                .is_some_and(|t| t.kind == TrackKind::Video),
            Command::EditTrimToSelection => has_range,
            Command::EditGroup => self.selection.len() >= 2,
            Command::EditSwitchMute
            | Command::EditSwitchLock
            | Command::EditSwitchInvertPhase
            | Command::EditSwitchLoop
            | Command::EditSwitchNormalize
            | Command::EditNormalizeRecalculate => has_events,
            Command::EditUngroup | Command::EditRemoveFromGroup | Command::EditSelectGroup => self
                .selection
                .refs()
                .into_iter()
                .any(|e| groups::group_members(&self.timeline, e).len() > 1),
            Command::TrackMute | Command::TrackMuteExclusive => self
                .selected_track
                .and_then(|id| self.timeline.track(id))
                .is_some(),
            Command::TrackSolo | Command::TrackSoloExclusive | Command::TrackPanEnvelope => self
                .selected_track
                .and_then(|id| self.timeline.track(id))
                .is_some_and(|t| t.kind == TrackKind::Audio),
            // On a video track `V` shows the level envelope and `Ctrl+Shift+E` the video ones
            Command::TrackVolumeEnvelope | Command::TrackToggleEnvelopes => self
                .selected_track
                .and_then(|id| self.timeline.track(id))
                .is_some(),
            _ => true,
        }
    }

    // ---- media import --------------------------------------------------------------------------

    fn import_media(&mut self, ctx: &egui::Context) {
        let mut extensions = vec!["wav", "mp3", "flac", "ogg", "m4a", "aac"];
        extensions.extend(neoveg_video::info::VIDEO_EXTENSIONS);
        // Still images, numbered image sequences and procedural generators land on video tracks
        extensions.extend(neoveg_video::still::IMAGE_EXTENSIONS);
        extensions.push(neoveg_video::still::SEQUENCE_EXTENSION);
        extensions.push("neoveggen");
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Mídia", &extensions)
            .add_filter("Todos os arquivos (converte se preciso)", &["*"])
            .pick_file()
        else {
            return;
        };
        self.status = tr_args("Importando {}…", &[&path.display()]);
        self.spawn_media(path, None, ctx);
    }

    /// Lays a slideshow on the video track: one event per picture, `settings.overlap` apart so that the
    /// neighbours crossfade, all as one undo step. A picture that cannot be read is skipped.
    fn insert_slideshow(
        &mut self,
        slides: &[neoveg_engine::slideshow::Slide],
        settings: neoveg_engine::slideshow::SlideshowSettings,
    ) {
        let fps = self.project.settings.fps;
        // Read the pictures first: one that cannot be read leaves no gap in the show
        let mut readable = Vec::new();
        let mut skipped = 0;
        for slide in slides {
            match neoveg_video::worker::probe_with_defaults(
                std::path::Path::new(&slide.path),
                fps,
                settings.per_photo,
            ) {
                Ok(info) => readable.push((slide, info)),
                Err(_) => skipped += 1,
            }
        }
        let laid = neoveg_engine::slideshow::layout(readable.len(), &settings);
        let video_track = self.track_of_kind(TrackKind::Video);
        let first_clip = self.timeline.tracks.iter().all(|t| t.events.is_empty());
        let mut commands = Vec::new();
        for ((slide, mut info), (start, duration)) in readable.into_iter().zip(laid) {
            if first_clip && commands.is_empty() && !self.format_chosen {
                self.project.settings.width = info.width;
                self.project.settings.height = info.height;
                self.format_chosen = true;
            }
            // A still has no length of its own: it lasts what the show gives it
            info.duration = duration;
            let media_id = self.timeline.alloc_id();
            self.project.media_pool.push(MediaRef {
                subproject: false,
                format: None,
                id: media_id,
                path: slide.path.clone(),
                duration_seconds: info.duration,
            });
            self.video_info.insert(media_id, info);
            commands.push(EditCommand::AddEvent {
                track_id: video_track,
                event: TrackEvent {
                    id: self.timeline.alloc_id(),
                    name: slide.name.clone(),
                    media_id: Some(media_id),
                    start,
                    duration,
                    ..Default::default()
                },
            });
        }
        let placed = commands.len();
        if placed > 0 {
            self.history
                .execute(&mut self.timeline, EditCommand::Batch(commands));
            self.last_frame_key = None;
        }
        self.status = match (placed, skipped) {
            (0, _) => "Nenhuma imagem pôde ser lida: nada foi inserido".into(),
            (n, 0) => tr_args("Apresentação de slides inserida: {} imagens", &[&n]),
            (n, s) => tr_args(
                "Apresentação de slides inserida: {} imagens ({} ilegíveis ignoradas)",
                &[&n, &s],
            ),
        };
    }

    /// Saves a procedural generator as a `.neoveggen` manifest (beside the project, or in the data
    /// folder of an unsaved one) and imports it like any other video media.
    fn insert_generated(
        &mut self,
        spec: neoveg_video::generators::GeneratorSpec,
        ctx: &egui::Context,
    ) {
        let dir = match self
            .project_path
            .as_deref()
            .and_then(std::path::Path::parent)
        {
            Some(parent) => parent.join("geradores"),
            None => match neoveg_common::paths::app_data_dir() {
                Some(dir) => dir.join("geradores"),
                None => {
                    self.status = "Não há pasta para guardar o gerador".into();
                    return;
                }
            },
        };
        let stem: String = spec
            .name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        let path = dir.join(format!("{stem}_{}.neoveggen", autosave::unix_now()));
        match spec.save_manifest(&path) {
            Ok(()) => {
                self.status = tr_args("Gerador {} criado", &[&spec.name]);
                self.spawn_media(path, None, ctx);
            }
            Err(e) => {
                tracing::error!("[ERROR] cannot save the generator: {e}");
                self.status = tr_args("Não foi possível criar o gerador: {}", &[&e]);
            }
        }
    }

    /// Loads audio or video media on a worker thread (never on the UI thread).
    fn spawn_media(&self, path: PathBuf, existing_id: Option<u64>, ctx: &egui::Context) {
        let (tx, ctx) = (self.loader_tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(load_message(path, existing_id, false));
            ctx.request_repaint();
        });
    }

    /// Where converted copies are written: beside the project, or in the data folder of an unsaved one.
    fn conversion_dir(&self) -> Option<PathBuf> {
        match self
            .project_path
            .as_deref()
            .and_then(std::path::Path::parent)
        {
            Some(parent) => Some(parent.join("convertidos")),
            None => neoveg_common::paths::app_data_dir().map(|d| d.join("convertidos")),
        }
    }

    /// Converts `path` to the media Neoveg reads, on a worker thread, and imports the result.
    fn start_conversion(&mut self, path: PathBuf, ctx: &egui::Context) {
        let Some(dir) = self.conversion_dir() else {
            self.status = "Não há pasta para guardar a mídia convertida".into();
            return;
        };
        let target = neoveg_video::convert::target_for(&path);
        let output = neoveg_video::convert::output_path(&path, &dir, target, |p| p.exists());
        self.status = tr_args(
            "Convertendo {} para {}…",
            &[&path.display(), &target.label()],
        );
        let (tx, ctx) = (self.loader_tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let msg = match neoveg_video::convert::convert(&path, &output, target) {
                Ok(()) => load_message(output, None, true),
                Err(error) => LoadMsg::Failed {
                    path,
                    error,
                    existing_id: None,
                    converted: true,
                },
            };
            let _ = tx.send(msg);
            ctx.request_repaint();
        });
    }

    fn poll_loader(&mut self) {
        while let Ok(msg) = self.loader_rx.try_recv() {
            match msg {
                LoadMsg::Relink { id, outcome } => self.apply_relink(id, outcome),
                LoadMsg::Copied { id, result } => self.apply_copy(id, result),
                LoadMsg::Failed {
                    path,
                    error,
                    existing_id,
                    converted,
                } => {
                    tracing::error!("[ERROR] import failed: {} ({error})", path.display());
                    self.status = tr_args("Falha ao importar: {}", &[&error]);
                    // A file of the project that exists but cannot be read: ask what to do with it
                    if let Some(id) = existing_id {
                        self.media_file_failed(id, &path, error.clone());
                    }
                    // A fresh import of a file that exists may be converted; a reload of a project's
                    // media, or the result of a conversion, may not
                    // (and not a video in a build that cannot decode video: that would fail again)
                    let hopeless =
                        is_video_path(&path) && !neoveg_video::info::video_decoding_enabled();
                    if existing_id.is_none() && !converted && !hopeless && path.exists() {
                        self.convert_offer = Some(ConvertOffer { path, error });
                    }
                }
                LoadMsg::Loaded {
                    media,
                    existing_id: Some(id),
                } => {
                    self.note_loaded(id, Some(media.channels), None, media.duration);
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
                    let fps = is_video_path(&path).then_some(info.fps);
                    let duration = info.duration;
                    let channels = audio.as_ref().map(|a| a.channels);
                    self.video_info.insert(id, info);
                    if let Some(audio) = audio {
                        self.media.insert(id, audio);
                    }
                    if is_video_path(&path) {
                        self.note_loaded(id, channels, fps, duration);
                    } else {
                        self.note_loaded(id, channels, None, f64::NAN);
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
                self.status = tr_args("Projeto salvo: {}", &[&path.display()]);
                self.remember_recent(&path);
                self.project_path = Some(path);
                self.mark_clean();
            }
            Err(e) => {
                tracing::error!("[ERROR] save failed: {e}");
                self.status = tr_args("Falha ao salvar: {}", &[&e]);
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
                self.status = tr_args("Falha ao abrir: {}", &[&e]);
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
        self.scroll_y = 0.0;

        // Missing media: searched for by exact then fuzzy name in the project folder and the old folders,
        // each candidate checked against what the project recorded, on a worker thread
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
        self.missing = missing
            .iter()
            .filter_map(|id| self.project.media_pool.iter().find(|m| m.id == *id))
            .map(|m| MissingMedia {
                id: m.id,
                name: file_name_of(&m.path),
                old_path: m.path.clone(),
                rejected: Vec::new(),
                searching: true,
                ignored: false,
            })
            .collect();
        self.missing_open = false;
        self.media_errors.clear();
        self.ignore_media_errors = false;
        let still_missing = missing.clone();
        if !missing.is_empty() {
            let wanted: Vec<MediaRef> = self
                .project
                .media_pool
                .iter()
                .filter(|m| missing.contains(&m.id))
                .cloned()
                .collect();
            self.start_relink_search(wanted, dirs, ctx);
        }

        let to_load: Vec<(u64, PathBuf)> = self
            .project
            .media_pool
            .iter()
            .filter(|m| !still_missing.contains(&m.id) && !m.subproject)
            .map(|m| (m.id, PathBuf::from(&m.path)))
            .collect();
        for (id, media_path) in to_load {
            self.spawn_media(media_path, Some(id), ctx);
        }

        self.status = if still_missing.is_empty() {
            "Projeto aberto".into()
        } else {
            tracing::warn!("[WARN] {} media file(s) missing", still_missing.len());
            tr_args(
                "Projeto aberto: procurando {} mídia(s) ausente(s)…",
                &[&still_missing.len()],
            )
        };
        if let Some(path) = &associate {
            self.remember_recent(path);
        }
        self.project_path = associate;
        self.format_chosen = true;
        self.mark_clean();
        true
    }

    /// Looks for the media in `wanted` in `dirs` on a worker thread, checking each candidate against what
    /// the project recorded; one `LoadMsg::Relink` comes back per media.
    fn start_relink_search(&self, wanted: Vec<MediaRef>, dirs: Vec<PathBuf>, ctx: &egui::Context) {
        let (tx, ctx) = (self.loader_tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            for media in wanted {
                let outcome = relink::resolve(&media, &dirs, &mut |p| probe_candidate(p));
                let _ = tx.send(LoadMsg::Relink {
                    id: media.id,
                    outcome,
                });
                ctx.request_repaint();
            }
        });
    }

    /// Takes the result of a search: a relink points the entry at the file and loads it, otherwise the
    /// media stays in the *Mídia ausente* list with the candidates that were turned down.
    fn apply_relink(&mut self, id: u64, outcome: Outcome) {
        match outcome {
            Outcome::Relinked {
                path,
                how,
                verified,
            } => {
                if relink::relink_to(&mut self.project.media_pool, id, &path) {
                    self.missing.retain(|m| m.id != id);
                    self.forced_dirty = true;
                    if let Some(ctx) = self.tabs.ctx.clone() {
                        self.spawn_media(path.clone(), Some(id), &ctx);
                    }
                    let how = match (how, verified) {
                        (relink::Match::Exact, true) => "nome igual, conferida",
                        (relink::Match::Exact, false) => "nome igual, não conferida",
                        (relink::Match::Fuzzy, _) => "nome parecido, conferida",
                    };
                    self.status = tr_args("Mídia religada a {} ({})", &[&path.display(), &how]);
                    self.offer_copy(id, &path);
                }
            }
            Outcome::Unresolved { rejected } => {
                if let Some(m) = self.missing.iter_mut().find(|m| m.id == id) {
                    m.rejected = rejected;
                    m.searching = false;
                }
            }
        }
        // The window opens once the searches are done and something is still missing
        let waiting = self.missing.iter().filter(|m| !m.ignored).count();
        if waiting > 0 && self.missing.iter().all(|m| !m.searching) {
            self.missing_open = true;
            self.status = tr_args(
                "{} mídia(s) ausente(s): veja a janela Mídia ausente",
                &[&waiting],
            );
        }
    }

    /// A file of the project could not be read: it is left offline at once when the user said to ignore
    /// such errors, and queued for the *Erro de arquivo de mídia* window otherwise.
    fn media_file_failed(&mut self, id: u64, path: &std::path::Path, error: String) {
        if self.ignore_media_errors {
            self.leave_offline(id, path, &error);
        } else if !self.media_errors.iter().any(|e| e.id == id) {
            self.media_errors.push_back(MediaError {
                id,
                path: path.to_string_lossy().into_owned(),
                error,
            });
        }
    }

    /// *Ignorar e deixar offline*: the media stays in the list, marked as ignored, with the reason.
    fn leave_offline(&mut self, id: u64, path: &std::path::Path, reason: &str) {
        let rejected = Rejected {
            path: path.to_path_buf(),
            reason: reason.to_owned(),
        };
        match self.missing.iter_mut().find(|m| m.id == id) {
            Some(m) => {
                m.ignored = true;
                m.searching = false;
                if m.rejected.iter().all(|r| r.path != rejected.path) {
                    m.rejected.push(rejected);
                }
            }
            None => self.missing.push(MissingMedia {
                id,
                name: file_name_of(&path.to_string_lossy()),
                old_path: path.to_string_lossy().into_owned(),
                rejected: vec![rejected],
                searching: false,
                ignored: true,
            }),
        }
    }

    /// Replaces the file a media entry points to by one the user picked, in every event that uses it.
    fn replace_media_file(&mut self, id: u64, ctx: &egui::Context) -> bool {
        let Some(path) = rfd::FileDialog::new().pick_file() else {
            return false;
        };
        if relink::relink_to(&mut self.project.media_pool, id, &path) {
            self.missing.retain(|m| m.id != id);
            self.forced_dirty = true;
            self.spawn_media(path.clone(), Some(id), ctx);
            self.status = tr_args("Mídia trocada por {}", &[&path.display()]);
            self.offer_copy(id, &path);
            return true;
        }
        false
    }

    /// Records what a reloaded file is like when the project did not know it, and warns when it is not
    /// as long as the project recorded (a replacement chosen by hand, for instance).
    fn note_loaded(&mut self, id: u64, channels: Option<u16>, fps: Option<f64>, duration: f64) {
        let Some(entry) = self.project.media_pool.iter_mut().find(|m| m.id == id) else {
            return;
        };
        if entry.format.is_none() && (channels.is_some() || fps.is_some()) {
            entry.format = Some(neoveg_common::project::MediaFormat { channels, fps });
        }
        if entry.duration_seconds > 0.0
            && duration.is_finite()
            && (duration - entry.duration_seconds).abs()
                > relink::duration_tolerance(entry.duration_seconds)
        {
            self.status = tr_args(
                "{} tem {} s, e o projeto guardou {} s: confira os eventos",
                &[
                    &file_name_of(&entry.path),
                    &format!("{:.2}", duration),
                    &format!("{:.2}", entry.duration_seconds),
                ],
            );
        }
    }

    /// *Localizar…*: the user picks the file for one missing media. It is taken as chosen, then checked
    /// for length when it loads.
    fn locate_missing(&mut self, id: u64, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new().pick_file() else {
            return;
        };
        if relink::relink_to(&mut self.project.media_pool, id, &path) {
            self.missing.retain(|m| m.id != id);
            self.forced_dirty = true;
            self.spawn_media(path.clone(), Some(id), ctx);
            self.status = tr_args("Mídia religada a {}", &[&path.display()]);
            self.offer_copy(id, &path);
        }
    }

    /// The folder of the saved project (`None` while it has no file).
    fn project_dir(&self) -> Option<PathBuf> {
        self.project_path
            .as_deref()
            .and_then(std::path::Path::parent)
            .map(PathBuf::from)
    }

    /// A media was found outside the project's folder: list it so it can be copied there.
    fn offer_copy(&mut self, id: u64, path: &std::path::Path) {
        let Some(dir) = self.project_dir() else {
            return;
        };
        if relink::is_in_folder(path, &dir) {
            return;
        }
        self.found.retain(|f| f.id != id);
        self.found.push(FoundMedia {
            id,
            name: file_name_of(&path.to_string_lossy()),
            path: path.to_path_buf(),
            copying: false,
        });
        self.found_open = true;
    }

    /// Copies the found files `ids` into the project's folder, one worker thread each.
    fn start_copy(&mut self, ids: Vec<u64>, ctx: &egui::Context) {
        let Some(dir) = self.project_dir() else {
            self.status = "Salve o projeto primeiro: a cópia vai para a pasta dele".into();
            return;
        };
        for found in self.found.iter_mut().filter(|f| ids.contains(&f.id)) {
            if found.copying {
                continue;
            }
            found.copying = true;
            let (tx, ctx, dir) = (self.loader_tx.clone(), ctx.clone(), dir.clone());
            let (id, source) = (found.id, found.path.clone());
            std::thread::spawn(move || {
                let result = relink::copy_into(&source, &dir);
                let _ = tx.send(LoadMsg::Copied { id, result });
                ctx.request_repaint();
            });
        }
    }

    /// A copy finished: the media points at the copy, which loads in place of the original.
    fn apply_copy(&mut self, id: u64, result: Result<PathBuf, String>) {
        match result {
            Ok(copy) => {
                self.found.retain(|f| f.id != id);
                if relink::relink_to(&mut self.project.media_pool, id, &copy) {
                    self.forced_dirty = true;
                    if let Some(ctx) = self.tabs.ctx.clone() {
                        self.spawn_media(copy.clone(), Some(id), &ctx);
                    }
                    self.status = tr_args("Mídia copiada para {}", &[&copy.display()]);
                }
            }
            Err(e) => {
                if let Some(f) = self.found.iter_mut().find(|f| f.id == id) {
                    f.copying = false;
                }
                tracing::error!("[ERROR] copy of found media failed: {e}");
                self.status = tr_args("Falha ao copiar a mídia: {}", &[&e]);
            }
        }
    }

    /// *Buscar em todos os lugares*: one walk of every drive for all the media that is still missing, on a
    /// worker thread that can be stopped. What it finds goes through the same check as any other search.
    fn search_missing_everywhere(&mut self, ctx: &egui::Context) {
        let wanted: Vec<MediaRef> = self
            .project
            .media_pool
            .iter()
            .filter(|m| self.missing.iter().any(|x| x.id == m.id))
            .cloned()
            .collect();
        if wanted.is_empty() {
            return;
        }
        for m in &mut self.missing {
            m.searching = true;
        }
        let search = Arc::new(DeepSearch::default());
        self.deep_search = Some(Arc::clone(&search));
        self.status = "Procurando a mídia ausente em todos os lugares…".into();
        let (tx, ctx) = (self.loader_tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let roots = search_roots();
            let mut found = relink::search_tree(
                &roots,
                &wanted,
                &|| search.cancel.load(Ordering::Relaxed),
                &mut |dir| {
                    let n = search.folders.fetch_add(1, Ordering::Relaxed) + 1;
                    if n % 200 == 0 {
                        if let Ok(mut current) = search.current.lock() {
                            *current = dir.display().to_string();
                        }
                        ctx.request_repaint();
                    }
                },
            );
            for media in wanted {
                let candidates = found.remove(&media.id).unwrap_or_default();
                let outcome =
                    relink::resolve_candidates(&media, candidates, &mut |p| probe_candidate(p));
                let _ = tx.send(LoadMsg::Relink {
                    id: media.id,
                    outcome,
                });
            }
            search.done.store(true, Ordering::Relaxed);
            ctx.request_repaint();
        });
    }

    /// *Buscar em uma pasta…*: the search runs again for what is still missing, in a folder the user picks.
    fn search_missing_in_folder(&mut self, ctx: &egui::Context) {
        let Some(folder) = rfd::FileDialog::new().pick_folder() else {
            return;
        };
        let wanted: Vec<MediaRef> = self
            .project
            .media_pool
            .iter()
            .filter(|m| self.missing.iter().any(|x| x.id == m.id))
            .cloned()
            .collect();
        for m in &mut self.missing {
            m.searching = true;
        }
        self.start_relink_search(wanted, vec![folder], ctx);
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
    fn snap_candidates(&self, exclude: &[EventRef]) -> Vec<f64> {
        neoveg_engine::snapping::candidates(
            &self.timeline,
            self.snap_sources,
            exclude,
            self.playhead,
            self.time_selection.map(|r| (r.start, r.end)),
        )
    }

    /// Seconds between two lines of the frame grid at the current zoom.
    fn grid_step_seconds(&self) -> f64 {
        let fps = self.project.settings.fps.max(1.0);
        crate::frame_grid::choose_step(
            fps,
            self.pixels_per_second,
            crate::frame_grid::MIN_MINOR_SPACING_PX,
        )
        .seconds(fps)
    }

    /// Length of the source media behind an event (`None` for generated media).
    fn source_len(&self, track_id: TrackId, event_id: EventId) -> Option<f64> {
        let track = self.timeline.track(track_id)?;
        let media_id = track.events.iter().find(|e| e.id == event_id)?.media_id?;
        // A still image or a generator has no end of its own: it can be dragged out as far as wanted
        if track.kind == TrackKind::Video
            && self
                .project
                .media_pool
                .iter()
                .find(|m| m.id == media_id)
                .is_some_and(|m| {
                    neoveg_video::info::is_unbounded_visual_path(std::path::Path::new(&m.path))
                })
        {
            return None;
        }
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
        // The body of an event does what the editing tool says
        let mode = match (mode, self.edit_tool) {
            (DragMode::Move, EditTool::Slip) => DragMode::Slip,
            (DragMode::Move, EditTool::Slide) => DragMode::Slide,
            (DragMode::Move, EditTool::Shuffle) => DragMode::Shuffle,
            (mode, _) => mode,
        };
        // A rolling trim needs a neighbour that touches the edge; without one it is a plain trim
        let (mode, partner) = match mode {
            DragMode::RollLeft | DragMode::RollRight => {
                match self.roll_partner(track_id, event_id, mode) {
                    Some(partner) => (mode, Some(partner)),
                    None if mode == DragMode::RollLeft => (DragMode::TrimLeft, None),
                    None => (DragMode::TrimRight, None),
                }
            }
            other => (other, None),
        };
        if let Some(p) = &partner {
            if switches::is_locked(&self.timeline, (track_id, p.event_id)) {
                self.status = "Evento bloqueado: desbloqueie para editá-lo".into();
                return;
            }
        }
        self.selected_track = Some(track_id);
        if !self.selection.contains((track_id, event_id)) {
            self.click_select((track_id, event_id));
        }
        // A move drags the whole selection (a group is selected as a whole)
        let moving = if mode == DragMode::Move && self.selection.len() > 1 {
            self.selection.refs()
        } else {
            Vec::new()
        };
        // A locked event cannot be dragged, and neither can a selection that holds one
        let dragged: Vec<EventRef> = if moving.is_empty() {
            vec![(track_id, event_id)]
        } else {
            moving.clone()
        };
        if dragged
            .iter()
            .any(|&e| switches::is_locked(&self.timeline, e))
        {
            self.status = "Evento bloqueado: desbloqueie para editá-lo".into();
            return;
        }
        self.drag = Some(EventDrag {
            mode,
            track_id,
            event_id,
            target_track: track_id,
            origin,
            current: origin,
            grab_offset: pointer_time - origin.start,
            moving,
            partner,
            snap_mark: None,
        });
    }

    /// The event that shares the cut at the edge being dragged (same track, touching).
    fn roll_partner(
        &self,
        track_id: TrackId,
        event_id: EventId,
        mode: DragMode,
    ) -> Option<RollPartner> {
        let track = self.timeline.track(track_id)?;
        let this = track.events.iter().find(|e| e.id == event_id)?;
        let touching = |other: &TrackEvent| match mode {
            DragMode::RollRight => (other.start - this.end()).abs() < 1e-6,
            _ => (other.end() - this.start).abs() < 1e-6,
        };
        track
            .events
            .iter()
            .find(|e| e.id != event_id && touching(e))
            .map(|e| RollPartner {
                event_id: e.id,
                origin: e.geometry(),
                current: e.geometry(),
            })
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
        let mut candidates = if snap_enabled {
            if drag.moving.len() > 1 {
                self.snap_candidates(&drag.moving)
            } else {
                self.snap_candidates(&[(drag.track_id, drag.event_id)])
            }
        } else {
            Vec::new()
        };
        if snap_enabled && self.snap_sources.grid {
            // Grid lines around the dragged block's start and end and around the pointer
            let step = self.grid_step_seconds();
            let start = pointer_time - drag.grab_offset;
            for t in [start, start + drag.origin.duration, pointer_time] {
                candidates.extend(neoveg_engine::snapping::grid_candidates(t, step));
            }
        }
        // Audio-only edits are left between frames when the preference says so
        let audio_exempt = !self.prefs.quantize_audio_edits
            && self
                .timeline
                .track(drag.track_id)
                .is_some_and(|t| t.kind == TrackKind::Audio);
        let quantize_frames = self.quantize_frames && !audio_exempt;
        let quant = |t: f64| {
            if quantize_frames {
                edit::quantize(t, fps)
            } else {
                t
            }
        };
        // The aligned time and what it was aligned to
        let snap = |t: f64| match edit::snap_time(t, &candidates, threshold) {
            Some(target) => (target, Some(SnapMark::Target(target))),
            None => {
                let q = quant(t);
                (q, ((q - t).abs() > 1e-6).then_some(SnapMark::Rounded(q)))
            }
        };

        match drag.mode {
            DragMode::Move => {
                let raw = pointer_time - drag.grab_offset;
                let snapped = edit::snap_block(raw, drag.origin.duration, &candidates, threshold);
                let start = snapped.unwrap_or_else(|| quant(raw));
                drag.snap_mark = match snapped {
                    // The line goes on whichever edge of the block met the target
                    Some(_) if candidates.iter().any(|c| (c - start).abs() < 1e-9) => {
                        Some(SnapMark::Target(start))
                    }
                    Some(_) => Some(SnapMark::Target(start + drag.origin.duration)),
                    None => ((start - raw).abs() > 1e-6).then_some(SnapMark::Rounded(start)),
                };
                drag.current = edit::move_to(drag.origin, start);
                if drag.moving.len() > 1 {
                    // A block moves in time only, and none of it may start before zero
                    let earliest = self.earliest_start(&drag.moving);
                    let delta = (drag.current.start - drag.origin.start).max(-earliest);
                    drag.current = edit::move_to(drag.origin, drag.origin.start + delta);
                } else if let Some(track) = row_track.and_then(|id| self.timeline.track(id)) {
                    // Events only move between tracks of their own kind
                    let same_kind = self
                        .timeline
                        .track(drag.track_id)
                        .is_some_and(|t| t.kind == track.kind);
                    if same_kind {
                        drag.target_track = track.id;
                    }
                }
            }
            DragMode::Shuffle => {
                // The event follows the pointer; the order changes when it is dropped
                let start = (pointer_time - drag.grab_offset).max(0.0);
                drag.current = edit::move_to(drag.origin, start);
            }
            DragMode::Slip | DragMode::Slide => {
                // How far the pointer has moved since the press, in whole frames
                let moved = pointer_time - drag.grab_offset - drag.origin.start;
                let delta = (moved * fps).round() / fps;
                // A looping event, a still or a generator has no end of its media to stop at
                let looping = self
                    .timeline
                    .track(drag.track_id)
                    .and_then(|t| t.events.iter().find(|e| e.id == drag.event_id))
                    .is_some_and(|e| e.looping);
                let len = if looping {
                    None
                } else {
                    self.source_len(drag.track_id, drag.event_id)
                };
                drag.current = if drag.mode == DragMode::Slip {
                    edit::slip(drag.origin, delta, len)
                } else {
                    edit::slide(drag.origin, delta, len)
                };
            }
            DragMode::TrimLeft => {
                let (edge, mark) = snap(pointer_time);
                drag.snap_mark = mark;
                drag.current = edit::trim_left(drag.origin, edge, min_len);
            }
            DragMode::TrimRight => {
                // A looping event may be dragged past the end of its source
                let looping = self
                    .timeline
                    .track(drag.track_id)
                    .and_then(|t| t.events.iter().find(|e| e.id == drag.event_id))
                    .is_some_and(|e| e.looping);
                let len = if looping {
                    None
                } else {
                    self.source_len(drag.track_id, drag.event_id)
                };
                let (edge, mark) = snap(pointer_time);
                drag.snap_mark = mark;
                drag.current = edit::trim_right(drag.origin, edge, min_len, len);
            }
            DragMode::RollLeft | DragMode::RollRight => {
                let (edge, mark) = snap(pointer_time);
                drag.snap_mark = mark;
                if let Some(partner) = drag.partner.as_mut() {
                    if drag.mode == DragMode::RollRight {
                        let len = self.source_len(drag.track_id, drag.event_id);
                        let (left, right) =
                            edit::roll(drag.origin, partner.origin, edge, min_len, len);
                        drag.current = left;
                        partner.current = right;
                    } else {
                        let len = self.source_len(drag.track_id, partner.event_id);
                        let (left, right) =
                            edit::roll(partner.origin, drag.origin, edge, min_len, len);
                        partner.current = left;
                        drag.current = right;
                    }
                }
            }
            DragMode::StretchLeft => {
                let (edge, mark) = snap(pointer_time);
                drag.snap_mark = mark;
                drag.current = edit::stretch_left(drag.origin, edge, min_len);
            }
            DragMode::StretchRight => {
                let (edge, mark) = snap(pointer_time);
                drag.snap_mark = mark;
                drag.current = edit::stretch_right(drag.origin, edge, min_len);
            }
            DragMode::FadeIn => {
                let length = quant(pointer_time - drag.origin.start);
                drag.current = edit::set_fade_in(drag.origin, length);
            }
            DragMode::FadeOut => {
                let length = quant(drag.origin.end() - pointer_time);
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
            self.restart_audio_if_playing();
            self.last_frame_key = None;
        }
    }

    /// Commits the drag as a single undo step (nothing happens when the event did not change).
    fn finish_drag(&mut self) {
        let Some(d) = self.drag.take() else {
            return;
        };
        if d.moving.len() > 1 {
            // The whole selection moved by the same amount: one undo step, selection unchanged
            let delta = d.current.start - d.origin.start;
            if let Some(outcome) = groups::move_events(&self.timeline, &d.moving, delta) {
                self.history.execute(&mut self.timeline, outcome.command);
                self.restart_audio_if_playing();
                self.last_frame_key = None;
            }
            return;
        }
        if let Some(p) = &d.partner {
            let edit_of = |event_id, old, new| EditCommand::EditEvent {
                from_track: d.track_id,
                to_track: d.track_id,
                event_id,
                old,
                new,
            };
            let mut commands = Vec::new();
            if d.current != d.origin {
                commands.push(edit_of(d.event_id, d.origin, d.current));
            }
            if p.current != p.origin {
                commands.push(edit_of(p.event_id, p.origin, p.current));
            }
            if !commands.is_empty() {
                self.history
                    .execute(&mut self.timeline, EditCommand::Batch(commands));
                self.restart_audio_if_playing();
                self.last_frame_key = None;
            }
            return;
        }
        // Trimming an edge of a grouped event trims the members that share that edge (`Shift` or
        // Ignore Grouping trims only this one: J and L cuts)
        if !self.auto_ripple
            && matches!(d.mode, DragMode::TrimLeft | DragMode::TrimRight)
            && d.target_track == d.track_id
            && !self.ignore_grouping
            && !self.drag_ignores_group
            && groups::group_members(&self.timeline, (d.track_id, d.event_id)).len() > 1
        {
            let left = d.mode == DragMode::TrimLeft;
            let edge = if left {
                d.current.start
            } else {
                d.current.end()
            };
            let fps = self.project.settings.fps.max(1.0);
            let outcome = groups::trim_group_edge(
                &self.timeline,
                (d.track_id, d.event_id),
                left,
                edge,
                1.0 / fps,
                &|track, event| self.source_len(track, event),
            );
            if self.apply_outcome(outcome).is_some() {
                return;
            }
        }
        // Auto Ripple: trimming or stretching an edge moves what follows by the change in length
        if self.auto_ripple
            && matches!(
                d.mode,
                DragMode::TrimLeft
                    | DragMode::TrimRight
                    | DragMode::StretchLeft
                    | DragMode::StretchRight
            )
            && d.target_track == d.track_id
        {
            if let Some(outcome) = neoveg_engine::ripple::ripple_resize(
                &self.timeline,
                d.track_id,
                d.event_id,
                d.current,
                self.ripple_scope,
            ) {
                self.history.execute(&mut self.timeline, outcome.command);
                self.restart_audio_if_playing();
                self.last_frame_key = None;
                return;
            }
        }
        if d.mode == DragMode::Shuffle {
            let drop_time = d.current.start + d.current.duration / 2.0;
            let outcome = neoveg_engine::ripple::shuffle_events(
                &self.timeline,
                d.track_id,
                d.event_id,
                drop_time,
            );
            match outcome {
                Some(outcome) => {
                    self.history.execute(&mut self.timeline, outcome.command);
                    self.restart_audio_if_playing();
                    self.last_frame_key = None;
                }
                None => {
                    self.status =
                        "Embaralhar precisa de eventos em sequência, sem sobreposição nem bloqueio"
                            .into()
                }
            }
            return;
        }
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
        if matches!(
            d.mode,
            DragMode::TrimLeft
                | DragMode::TrimRight
                | DragMode::StretchLeft
                | DragMode::StretchRight
        ) && d.target_track == d.track_id
        {
            self.remember_post_ripple(neoveg_engine::ripple::resize_hint(
                d.track_id, d.origin, d.current,
            ));
        }
        self.selected_track = Some(d.target_track);
        if d.target_track != d.track_id || !self.selection.contains((d.target_track, d.event_id)) {
            self.selection.select_only((d.target_track, d.event_id));
        }
        self.restart_audio_if_playing();
        self.last_frame_key = None;
    }

    fn earliest_start(&self, events: &[EventRef]) -> f64 {
        events
            .iter()
            .filter_map(|&(track_id, event_id)| {
                self.timeline
                    .track(track_id)?
                    .events
                    .iter()
                    .find(|e| e.id == event_id)
                    .map(|e| e.start)
            })
            .fold(f64::INFINITY, f64::min)
            .max(0.0)
    }

    // ---- multiple selection and groups -------------------------------------------------------

    /// The events a click on `event` selects: its whole group unless grouping is ignored.
    fn clicked_members(&self, event: EventRef) -> Vec<EventRef> {
        if self.ignore_grouping {
            vec![event]
        } else {
            groups::group_members(&self.timeline, event)
        }
    }

    /// Plain click: the event, or its whole group, becomes the selection.
    fn click_select(&mut self, event: EventRef) {
        let members = self.clicked_members(event);
        self.selection.select_many(members, event);
    }

    /// Ctrl-click: adds or removes the event, or its whole group.
    fn click_toggle(&mut self, event: EventRef) {
        let members = self.clicked_members(event);
        self.selection.toggle_many(&members, event);
    }

    /// Shift-click: the range up to `event`, completed with the groups it touches.
    fn click_extend(&mut self, event: EventRef) {
        self.selection.extend_to(&self.timeline, event);
        self.include_groups_in_selection();
    }

    fn include_groups_in_selection(&mut self) {
        if !self.ignore_grouping {
            let all = groups::expand_to_groups(&self.timeline, &self.selection.refs());
            self.selection.add_all(all);
        }
    }

    /// Rectangular selection: events touching `a_time..b_time` on the rows between the two
    /// fractional rows (first track = row 0). `additive` keeps the current selection.
    fn select_rect(&mut self, a_time: f64, b_time: f64, a_row: f32, b_row: f32, additive: bool) {
        let first = a_row.min(b_row).floor().max(0.0) as usize;
        let last = a_row.max(b_row).floor().max(0.0) as usize;
        let tracks: Vec<TrackId> = self
            .timeline
            .tracks
            .iter()
            .enumerate()
            .filter(|(i, _)| (first..=last).contains(i))
            .map(|(_, t)| t.id)
            .collect();
        let previous = self.selection.refs();
        self.selection.select_in_rect(
            &self.timeline,
            Some(&tracks),
            TimeRange::new(a_time, b_time),
        );
        if additive {
            self.selection.add_all(previous);
        }
        self.include_groups_in_selection();
        self.time_selection = None;
    }

    fn finish_marquee(&mut self) {
        if let Some(m) = self.marquee.take() {
            self.select_rect(
                m.anchor_time,
                m.current_time,
                m.anchor_row,
                m.current_row,
                m.additive,
            );
        }
    }

    /// `G`: the selected events become one group.
    fn group_selection(&mut self) {
        let refs = self.selection.refs();
        match groups::group_events(&mut self.timeline, &refs) {
            Some(outcome) => {
                self.history.execute(&mut self.timeline, outcome.command);
                self.status = tr_args("{} eventos agrupados", &[&refs.len()]);
            }
            None => self.status = "Selecione dois ou mais eventos para agrupar".into(),
        }
    }

    /// Chooses the editing tool (`Ctrl+D` normal, `D` next, `Shift+D` previous).
    fn set_edit_tool(&mut self, tool: EditTool) {
        self.edit_tool = tool;
        self.status = tr_args("Ferramenta de edição: {}", &[&tool.label()]);
    }

    /// `Shift+G`: selects every event of the groups the selected events belong to.
    fn select_group_members(&mut self) {
        let refs = self.selection.refs();
        let all = groups::expand_to_groups(&self.timeline, &refs);
        if all.len() <= refs.len() {
            self.status = "Os eventos selecionados não pertencem a um grupo maior".into();
            return;
        }
        self.status = tr_args("{} eventos do grupo selecionados", &[&all.len()]);
        self.selection.set(all);
    }

    /// `U`: takes the selected events out of their groups; the other members stay grouped.
    fn remove_selection_from_groups(&mut self) {
        let refs = self.selection.refs();
        match groups::remove_from_groups(&self.timeline, &refs) {
            Some(outcome) => {
                self.history.execute(&mut self.timeline, outcome.command);
                self.status = "Eventos retirados do grupo".into();
            }
            None => self.status = "Nenhum evento agrupado e desbloqueado na seleção".into(),
        }
    }

    /// `Alt+Shift+Up` / `Alt+Shift+Down`: moves the track focus one track up or down.
    fn move_track_focus(&mut self, delta: isize) {
        let count = self.timeline.tracks.len();
        if count == 0 {
            return;
        }
        let current = self
            .selected_track
            .and_then(|id| self.timeline.tracks.iter().position(|t| t.id == id));
        let next = match current {
            Some(i) => (i as isize + delta).clamp(0, count as isize - 1) as usize,
            None if delta < 0 => count - 1,
            None => 0,
        };
        let track = &self.timeline.tracks[next];
        self.selected_track = Some(track.id);
        self.status = tr_args("Faixa {}: {}", &[&(next + 1), &track.name]);
    }

    /// Insert > Time: opens `length` seconds of empty time at the cursor, on the tracks of the
    /// selected events or, with nothing selected, on every track.
    fn insert_time_at_cursor(&mut self, length: f64) {
        let mut chosen: Vec<TrackId> = self.selection.refs().iter().map(|r| r.0).collect();
        chosen.sort_unstable();
        chosen.dedup();
        let tracks = (!chosen.is_empty()).then_some(chosen.as_slice());
        match ops::insert_time(&mut self.timeline, self.playhead, length, tracks) {
            Some(outcome) => {
                self.history.execute(&mut self.timeline, outcome.command);
                self.status = tr_args(
                    "{} s de tempo inseridos no cursor",
                    &[&format!("{:.2}", length)],
                );
            }
            None => self.status = "Informe um tempo maior que zero".into(),
        }
    }

    /// Remembers `path` in the recent projects list (and on disk, once the real preferences are
    /// loaded: tests and the offscreen screenshots never write the user's file).
    fn remember_recent(&mut self, path: &std::path::Path) {
        self.prefs.add_recent_project(path);
        if self.persist_prefs {
            save_prefs(&self.prefs);
        }
    }

    /// Writes the preferences when they are the user's real ones.
    fn persist_preferences(&self) {
        if self.persist_prefs {
            save_prefs(&self.prefs);
        }
    }

    /// An untitled project with nothing in it and nothing to lose: a file opens right into it.
    fn is_blank_project(&self) -> bool {
        self.project_path.is_none()
            && !self.is_dirty()
            && self.timeline.tracks.iter().all(|t| t.events.is_empty())
    }

    /// Opens a recent project: into the blank project at startup, otherwise in a tab of its own so
    /// that the work in progress is never replaced.
    fn open_recent(&mut self, path: &std::path::Path, ctx: &egui::Context) {
        if !path.is_file() {
            self.status = tr_args("Arquivo não encontrado: {}", &[&path.display()]);
            self.prefs.remove_recent_project(&path.to_string_lossy());
            self.persist_preferences();
            return;
        }
        if self.is_blank_project() {
            self.load_project_file(path, Some(path.to_path_buf()), ctx);
        } else {
            self.tabs.open_request = Some(path.to_path_buf());
        }
    }

    /// File > Properties: opens the project's video and audio format.
    fn open_project_properties(&mut self) {
        let start_all_new = self.prefs.new_project_settings.is_some();
        self.project_dialog
            .open_with(&self.project.settings, start_all_new);
    }

    /// Puts new settings in the project (OK / Apply of the properties window).
    fn apply_project_settings(&mut self, settings: ProjectSettings) {
        if settings == self.project.settings {
            return;
        }
        self.project.settings = settings;
        self.format_chosen = true;
        // The settings are not an undoable edit, but the project has changed
        self.forced_dirty = true;
        self.last_frame_key = None;
        self.last_composite_key = None;
        self.status = tr_args(
            "Projeto: {}x{} · {} fps · {} Hz",
            &[
                &self.project.settings.width,
                &self.project.settings.height,
                &format!("{:.3}", self.project.settings.fps),
                &self.project.settings.sample_rate,
            ],
        );
    }

    /// Carries out what the properties window asked for.
    fn run_project_actions(&mut self, actions: Vec<crate::project_dialog::ProjectAction>) {
        use crate::project_dialog::ProjectAction;
        for action in actions {
            match action {
                ProjectAction::Apply {
                    settings,
                    start_all_new,
                    close: _,
                } => {
                    self.apply_project_settings(*settings.clone());
                    self.prefs.new_project_settings = start_all_new.then_some(*settings);
                    self.persist_preferences();
                }
                ProjectAction::SaveTemplate(template) => {
                    self.prefs
                        .custom_templates
                        .retain(|t| t.name != template.name);
                    self.status = tr_args("Modelo salvo: {}", &[&template.name]);
                    self.prefs.custom_templates.push(template);
                    self.persist_preferences();
                }
                ProjectAction::MatchMedia => self.pick_media_to_match(),
            }
        }
    }

    /// Chooses a media file and reads its picture format on a worker thread.
    fn pick_media_to_match(&mut self) {
        let mut extensions: Vec<&str> = neoveg_video::info::VIDEO_EXTENSIONS.to_vec();
        extensions.extend(neoveg_video::still::IMAGE_EXTENSIONS);
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Mídia com imagem", &extensions)
            .pick_file()
        else {
            return;
        };
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(probe(&path).map(|i| (i.width, i.height, i.fps)));
        });
        self.match_media_rx = Some(rx);
        self.project_dialog.note = "Lendo o formato da mídia…".into();
    }

    /// Takes the result of [`Self::pick_media_to_match`] when the worker is done.
    fn poll_match_media(&mut self) {
        let Some(rx) = &self.match_media_rx else {
            return;
        };
        let Ok(result) = rx.try_recv() else {
            return;
        };
        self.match_media_rx = None;
        match result {
            Ok((width, height, fps)) => {
                self.project_dialog.use_media(width, height, fps);
                self.project_dialog.note.clear();
            }
            Err(e) => self.project_dialog.note = tr_args("Não foi possível ler a mídia: {}", &[&e]),
        }
    }

    /// Starts a new project in a tab with the settings chosen on the welcome screen.
    fn new_project_with(&mut self, settings: ProjectSettings) {
        self.new_tab();
        self.project.settings = settings;
        self.format_chosen = true;
        self.last_frame_key = None;
        self.last_composite_key = None;
        self.status = tr_args(
            "Novo projeto: {}x{} · {} fps",
            &[
                &self.project.settings.width,
                &self.project.settings.height,
                &format!("{:.3}", self.project.settings.fps),
            ],
        );
    }

    /// `Ctrl+Alt+S` (*Save Version*): saves the project under the next numbered name
    /// (`projeto_0001.neoveg`, `projeto_0002.neoveg`…) beside it and goes on working in that file;
    /// the previous file keeps the work as it was.
    fn save_incremental(&mut self) {
        let Some(path) = self.project_path.clone() else {
            // A project that was never saved has no name to count from
            self.save_project(true);
            return;
        };
        let copy = incremental_path(&path, &|p| p.exists());
        match project_file::save(&copy, &self.project, &self.timeline) {
            Ok(()) => {
                tracing::info!("[INFO] new version saved: {}", copy.display());
                if let Some(stem) = copy.file_stem() {
                    self.project.name = stem.to_string_lossy().into_owned();
                }
                self.status = tr_args("Versão salva: {}", &[&copy.display()]);
                self.project_path = Some(copy);
                self.mark_clean();
            }
            Err(e) => {
                tracing::error!("[ERROR] incremental save failed: {e}");
                self.status = tr_args("Falha ao salvar a cópia: {}", &[&e]);
            }
        }
    }

    /// `U`: dissolves the groups of the selected events.
    fn ungroup_selection(&mut self) {
        let refs = self.selection.refs();
        match groups::ungroup_events(&self.timeline, &refs) {
            Some(outcome) => {
                self.history.execute(&mut self.timeline, outcome.command);
                self.status = "Eventos desagrupados".into();
            }
            None => self.status = "Nenhum evento agrupado na seleção".into(),
        }
    }

    /// Edit > Switches: turns a switch on for the selected events, or off when it is on for all
    /// of them. One undo step.
    fn toggle_event_switch(&mut self, switch: Switch) {
        let refs = self.selection.refs();
        let on = switches::state(&self.timeline, &refs, switch) != SwitchState::On;
        let command = if switch == Switch::Normalize && on {
            // Turning Normalize on measures the peak of what each event plays
            let gains = self.normalize_gains(&refs, false);
            if gains.is_empty() {
                self.status =
                    "Normalizar precisa de eventos de áudio desbloqueados com mídia carregada"
                        .into();
                return;
            }
            switches::set_normalize(&self.timeline, &gains)
        } else {
            switches::toggle(&self.timeline, &refs, switch)
        };
        match command {
            Some(command) => {
                self.history.execute(&mut self.timeline, command);
                let name = match switch {
                    Switch::Mute => "Mute",
                    Switch::Lock => "Bloqueio",
                    Switch::InvertPhase => "Inverter fase",
                    Switch::Loop => "Repetir",
                    Switch::Normalize => "Normalizar",
                };
                self.status = format!("{name} {}", if on { "ligado" } else { "desligado" });
            }
            None => {
                self.status = match switch {
                    Switch::InvertPhase | Switch::Normalize => {
                        "Vale só para eventos de áudio desbloqueados".into()
                    }
                    Switch::Loop => {
                        "Repetir vale só para eventos desbloqueados em velocidade normal".into()
                    }
                    Switch::Mute | Switch::Lock => "Nenhum evento desbloqueado na seleção".into(),
                };
            }
        }
    }

    /// Edit > Channels: converts the channels of the selected audio events, non-destructively.
    fn set_event_channels(&mut self, mode: neoveg_common::channels::ChannelMode) {
        let refs = self.selection.refs();
        match switches::set_channels(&self.timeline, &refs, mode) {
            Some(command) => {
                self.history.execute(&mut self.timeline, command);
                self.status = tr_args("Canais: {}", &[&mode.label()]);
            }
            None => {
                self.status = "Nenhum evento de áudio desbloqueado mudou de canais".into();
            }
        }
    }

    /// The gain that brings each listed audio event to the project's normalize peak level,
    /// measured on the part of the source the event plays (a looping event: from its in point to
    /// the end of the media). With `only_normalized` only events that already have the switch are
    /// listed (Recalculate).
    fn normalize_gains(&self, refs: &[EventRef], only_normalized: bool) -> Vec<(EventRef, f32)> {
        let target = self.project.settings.normalize_peak_db;
        refs.iter()
            .filter_map(|&r| {
                let track = self.timeline.track(r.0)?;
                if track.kind != TrackKind::Audio {
                    return None;
                }
                let e = track.events.iter().find(|e| e.id == r.1)?;
                if e.locked || (only_normalized && !e.normalize) {
                    return None;
                }
                let media = self.media.get(&e.media_id?)?;
                let rate = edit::clamp_playback_rate(e.playback_rate);
                let length = if e.looping {
                    edit::loop_length(media.duration, e.trim_in)
                } else {
                    e.duration * rate
                };
                let peak = neoveg_audio::mixer::peak_in_window(
                    &media.stereo,
                    media.sample_rate,
                    e.trim_in,
                    length,
                );
                Some((r, neoveg_audio::mixer::normalize_gain(peak, target)))
            })
            .collect()
    }

    /// Edit > Recalculate normalization: measures again the events that are normalized, which is
    /// needed after trimming or extending one (it may now play louder or quieter material).
    fn recalculate_normalize(&mut self) {
        let refs = self.selection.refs();
        let gains = self.normalize_gains(&refs, true);
        match switches::set_normalize(&self.timeline, &gains) {
            Some(command) => {
                self.history.execute(&mut self.timeline, command);
                self.status = "Normalização recalculada".into();
            }
            None => self.status = "Nenhum evento normalizado mudou de ganho".into(),
        }
    }

    /// `Ctrl+Shift+U`: clicks select single events of a group (or the whole group again).
    fn toggle_ignore_grouping(&mut self) {
        self.ignore_grouping = !self.ignore_grouping;
        self.status = if self.ignore_grouping {
            "Agrupamento ignorado: os cliques selecionam um evento".into()
        } else {
            "Agrupamento ativo".into()
        };
    }

    // ---- unsaved changes and autosave -----------------------------------------------------------

    fn is_dirty(&self) -> bool {
        self.forced_dirty || self.history.revision() != self.saved_revision
    }

    /// The project now matches what is on disk: no unsaved changes, no pending recovery file.
    fn mark_clean(&mut self) {
        self.tabs.bump();
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
        let title = tr_args(
            "{}{} - Neoveg",
            &[&if self.is_dirty() { "*" } else { "" }, &self.project.name],
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
        // The first clip defines the project format, unless the format was chosen
        if self.timeline.tracks.iter().all(|t| t.events.is_empty()) && !self.format_chosen {
            let settings = &mut self.project.settings;
            settings.width = info.width;
            settings.height = info.height;
            settings.fps = info.fps;
            self.format_chosen = true;
        }
        let media_id = self.timeline.alloc_id();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.project.media_pool.push(MediaRef {
            subproject: false,
            // A picture has no frame rate worth checking, and a still has no real length
            format: Some(neoveg_common::project::MediaFormat {
                channels: audio.as_ref().map(|a| a.channels),
                fps: is_video_path(&path).then_some(info.fps),
            }),
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
            group: None,
            id: self.timeline.alloc_id(),
            name: name.clone(),
            media_id: Some(media_id),
            start: self.playhead,
            duration: info.duration,
            trim_in: 0.0,
            fade_in: 0.0,
            fade_out: 0.0,
            ..Default::default()
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
                group: None,
                id: self.timeline.alloc_id(),
                name: name.clone(),
                media_id: Some(media_id),
                start: self.playhead,
                duration: audio.duration,
                trim_in: 0.0,
                fade_in: fade,
                fade_out: fade,
                ..Default::default()
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
        self.status = tr_args("Importado: {}", &[&name]);
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

    /// The correction in effect: the one being dragged in the window, else the project's.
    fn effective_grade(&self) -> Grade {
        self.grade_draft.unwrap_or(self.timeline.video_grade)
    }

    /// Commits the correction (one undo step) and redraws the preview.
    fn commit_grade(&mut self, grade: Grade) {
        self.grade_draft = None;
        if let Some(command) = mix::set_video_grade(&self.timeline, grade) {
            self.history.execute(&mut self.timeline, command);
        }
        self.last_frame_key = None;
    }

    fn effective_chroma(&self) -> ChromaKey {
        self.chroma_draft.unwrap_or(self.timeline.chroma_key)
    }

    fn commit_chroma(&mut self, key: ChromaKey) {
        self.chroma_draft = None;
        if let Some(command) = mix::set_chroma_key(&self.timeline, key) {
            self.history.execute(&mut self.timeline, command);
        }
        self.last_frame_key = None;
    }

    /// The color table of the project, loaded once per path (a broken file is reported once).
    fn current_lut(&mut self) -> Option<Arc<Lut>> {
        let path = self.timeline.video_lut.clone()?;
        if let Some((cached, lut)) = &self.lut_cache {
            if *cached == path {
                return lut.clone();
            }
        }
        let loaded = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|text| Lut::parse_cube(&text));
        let lut = match loaded {
            Ok(lut) => Some(Arc::new(lut)),
            Err(e) => {
                tracing::warn!("[WARN] cannot use the LUT {path}: {e}");
                self.status = tr_args("LUT inválida: {}", &[&e]);
                None
            }
        };
        self.lut_cache = Some((path, lut.clone()));
        self.last_frame_key = None;
        lut
    }

    /// Sets the color table of the video (one undo step) and redraws the preview.
    fn set_video_lut(&mut self, lut: Option<String>) {
        if let Some(command) = mix::set_video_lut(&self.timeline, lut) {
            self.history.execute(&mut self.timeline, command);
        }
        self.last_frame_key = None;
    }

    /// Asks the decoder thread for the frame under the playhead (topmost video track wins).
    fn request_preview_frame(&mut self, ppp: f32) {
        let max_width = self.preview_max_width(ppp);
        // A sub-project plays through the expansion of its events
        let flat = self.flattened();
        let timeline = flat.as_deref().map_or(&self.timeline, |f| &f.timeline);
        // Overlapping video clips (several tracks, or a crossfade) are composited
        // (also when an effect is on: only that path runs the video effects)
        if composite::visible_clips(timeline, self.playhead) > 1
            || composite::has_effects(timeline, self.playhead)
            || self.pan_crop_draft.is_some()
            || self.velocity_draft.is_some()
        {
            self.request_composite_frame(max_width, flat.as_deref());
            return;
        }
        self.last_composite_key = None;
        let timeline = flat.as_deref().map_or(&self.timeline, |f| &f.timeline);
        let target = timeline
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Video && timeline.audible(t))
            .find_map(|t| timeline.playing_event_at(t.id, self.playhead))
            .and_then(|e| {
                let rate = edit::clamp_playback_rate(e.playback_rate);
                let mut offset = (self.playhead - e.start) * rate;
                if e.looping {
                    let source = e.media_id.and_then(|id| self.video_info.get(&id));
                    let len = source.map_or(0.0, |i| edit::loop_length(i.duration, e.trim_in));
                    offset = edit::wrap_loop(offset, len);
                }
                Some((e.media_id?, offset + e.trim_in))
            });
        let Some((media_id, seconds)) = target else {
            self.video_texture = None;
            self.last_frame_key = None;
            return;
        };
        let lut = self.current_lut();
        let Some(worker) = &self.frame_worker else {
            return;
        };
        let item = flat
            .as_ref()
            .and_then(|f| f.extra_media.iter().find(|m| m.id == media_id))
            .or_else(|| self.project.media_pool.iter().find(|m| m.id == media_id));
        let Some(item) = item else {
            return;
        };
        let fps = self
            .video_info
            .get(&media_id)
            .map_or(self.project.settings.fps, |i| i.fps);
        let frame = (seconds * fps).round() as i64;
        let key = (media_id, frame, max_width);
        // An undo of the LUT changes the picture without changing the frame key
        if self.last_lut_path != self.timeline.video_lut
            || self.last_grade != self.effective_grade()
            || self.last_chroma != self.effective_chroma()
        {
            self.last_lut_path = self.timeline.video_lut.clone();
            self.last_grade = self.effective_grade();
            self.last_chroma = self.effective_chroma();
            self.last_frame_key = None;
        }
        if self.last_frame_key != Some(key) {
            self.last_frame_key = Some(key);
            worker.request(FrameRequest {
                media_id,
                path: PathBuf::from(&item.path),
                // frame-aligned so the decoder returns exactly this picture
                seconds: frame as f64 / fps,
                max_width,
                lut,
                grade: self.effective_grade(),
                chroma_key: self.effective_chroma(),
            });
        }
    }

    /// Asks the decoder thread for the composite of every video clip visible at the playhead.
    fn request_composite_frame(
        &mut self,
        max_width: u32,
        flat: Option<&neoveg_engine::project_manager::Flattened>,
    ) {
        let lut = self.current_lut();
        let (grade, chroma) = (self.effective_grade(), self.effective_chroma());
        // A change of LUT, grade or key repaints the same frame
        if self.last_lut_path != self.timeline.video_lut
            || self.last_grade != grade
            || self.last_chroma != chroma
        {
            self.last_lut_path = self.timeline.video_lut.clone();
            self.last_grade = grade;
            self.last_chroma = chroma;
            self.last_composite_key = None;
        }
        let settings = &self.project.settings;
        let fps = settings.fps;
        let frame = (self.playhead * fps).round() as i64;
        let key = (
            frame,
            max_width,
            self.history.revision(),
            VfxDraft::stamp(self.vfx_draft.as_ref())
                ^ PanCropDraft::stamp(self.pan_crop_draft.as_ref())
                ^ VelocityDraft::stamp(self.velocity_draft.as_ref()),
        );
        if self.last_composite_key == Some(key) {
            return;
        }
        self.last_composite_key = Some(key);
        self.last_frame_key = None;
        let frame_settings = composite::FrameSettings {
            fps,
            canvas: (settings.width, settings.height),
            max_width,
            playing: self.active_player.is_some(),
            auto_crossfade: settings.auto_crossfade,
            lut,
            grade,
            chroma_key: chroma,
            fx_draft: self.vfx_draft.clone(),
            pan_crop_draft: self.pan_crop_draft.clone(),
            velocity_draft: self.velocity_draft.clone(),
        };
        let sources = self.render_sources(flat);
        let timeline = flat.map_or(&self.timeline, |f| &f.timeline);
        let request =
            composite::composite_request(timeline, frame as f64 / fps, &frame_settings, &|id| {
                sources.get(&id).cloned()
            });
        match (request, &self.frame_worker) {
            (Some(request), Some(worker)) => worker.request_composite(request),
            // Nothing is visible here: black
            (None, _) => self.video_texture = None,
            // No decoder thread (the offscreen screenshots): there is nothing to ask, so the
            // picture on screen stays as it is
            (Some(_), None) => {}
        }
    }

    fn poll_frames(&mut self, ctx: &egui::Context) {
        let Some(worker) = &self.frame_worker else {
            return;
        };
        // The Pan/Crop editor's background picture
        while let Ok(reply) = worker.source_rx.try_recv() {
            match reply.result {
                Ok(frame) => self.pan_crop.set_source(ctx, &frame),
                Err(e) => tracing::debug!("[DEBUG] Pan/Crop source frame: {e}"),
            }
        }
        // Only the newest reply matters, and only from the kind of request now in use
        let mut single = None;
        while let Ok(reply) = worker.rx.try_recv() {
            single = Some(reply.result);
        }
        let mut composite = None;
        while let Ok(reply) = worker.composite_rx.try_recv() {
            composite = Some(reply.result);
        }
        let latest = if self.last_composite_key.is_some() {
            composite
        } else {
            single
        };
        let Some(result) = latest else {
            return;
        };
        match result {
            Ok(frame) => {
                self.frame_scopes = Some(Scopes::from_frame(&frame));
                self.frame_scopes_version += 1;
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [frame.width as usize, frame.height as usize],
                    &frame.rgba,
                );
                let options = preview_texture_options(self.quality);
                match &mut self.video_texture {
                    Some(texture) => texture.set(image, options),
                    None => {
                        self.video_texture =
                            Some(ctx.load_texture("preview_frame", image, options));
                    }
                }
            }
            Err(e) => self.status = tr_args("Falha ao decodificar vídeo: {}", &[&e]),
        }
    }

    /// Opens a media pool file in the Trimmer and shows the Trimmer tab.
    fn open_in_trimmer(&mut self, media_id: u64) {
        let Some(item) = self.project.media_pool.iter().find(|m| m.id == media_id) else {
            return;
        };
        let name = std::path::Path::new(&item.path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let fps = self.project.settings.fps;
        self.trimmer = Some(neoveg_engine::trimmer::TrimmerState::new(
            media_id,
            &name,
            item.duration_seconds,
            fps,
        ));
        self.media_tab = 1;
        self.status = tr_args("{} aberto no Trimmer", &[&name]);
    }

    /// What the Trimmer buttons ask for: put the selection on the timeline.
    fn trimmer_add(&mut self, mode: TrimmerAdd) {
        use neoveg_engine::trimmer::{add_command, Placement};
        let Some(trimmer) = &self.trimmer else {
            return;
        };
        let (media_id, name) = (trimmer.media_id, trimmer.name.clone());
        let placement: Option<Placement> = match mode {
            TrimmerAdd::FromCursor => match self.time_selection {
                // Two timeline points and one Trimmer point
                Some(range) if !range.is_empty() => trimmer.from_timeline_range(range),
                _ => trimmer.add_from_cursor(self.playhead),
            },
            TrimmerAdd::UpToCursor => trimmer.add_up_to_cursor(self.playhead),
            TrimmerAdd::FitToFill => match self.time_selection.filter(|r| !r.is_empty()) {
                Some(range) => trimmer.fit_to_fill(range).map(|(p, exact)| {
                    if !exact {
                        self.status = "Ajustar ao preenchimento: velocidade limitada".into();
                    }
                    p
                }),
                None => {
                    self.status = "Selecione um trecho da linha do tempo para preencher".into();
                    return;
                }
            },
        };
        let Some(placement) = placement else {
            self.status = "Nada para adicionar: marque a seleção e o cursor".into();
            return;
        };
        let has_video = self.video_info.contains_key(&media_id);
        let has_audio = self.media.contains_key(&media_id);
        let mut kinds = Vec::new();
        if has_video {
            kinds.push(TrackKind::Video);
        }
        if has_audio || !has_video {
            kinds.push(TrackKind::Audio);
        }
        let mut commands = Vec::new();
        for kind in kinds {
            let track_id = self.track_of_kind(kind);
            let event = placement.into_event(self.timeline.alloc_id(), &name, media_id);
            if let Some(command) =
                add_command(&mut self.timeline, track_id, event, self.trimmer_overwrite)
            {
                commands.push(command);
            }
        }
        if !commands.is_empty() {
            self.history
                .execute(&mut self.timeline, EditCommand::Batch(commands));
            self.status = tr_args("{} adicionado à linha do tempo", &[&name]);
        }
    }

    fn place_media(&mut self, media: LoadedMedia) {
        let media_id = self.timeline.alloc_id();
        self.project.media_pool.push(MediaRef {
            subproject: false,
            format: Some(neoveg_common::project::MediaFormat {
                channels: Some(media.channels),
                fps: None,
            }),
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
            group: None,
            id: self.timeline.alloc_id(),
            name: media.name.clone(),
            media_id: Some(media_id),
            start: self.playhead,
            duration: media.duration,
            trim_in: 0.0,
            fade_in: fade,
            fade_out: fade,
            ..Default::default()
        };
        self.status = tr_args("Importado: {}", &[&media.name]);
        self.media.insert(media_id, media);
        self.history.execute(
            &mut self.timeline,
            EditCommand::AddEvent { track_id, event },
        );
    }

    // ---- transport -----------------------------------------------------------------------------

    /// `L` / `J`: starts playing forward / backwards, and every further press of the same key goes
    /// faster (1x, 2x, 4x, 8x, 16x); the other key turns around. `K` pauses.
    fn shuttle(&mut self, forward: bool, now: f64) {
        let current = if self.playing {
            self.shuttle_speed
        } else {
            0.0
        };
        let speed = if forward {
            neoveg_audio::playback::shuttle_forward(current)
        } else {
            neoveg_audio::playback::shuttle_reverse(current)
        };
        if !self.playing {
            // Going backwards from the very start has nowhere to go
            if !forward && self.playhead <= 0.0 {
                self.status = "O cursor já está no início".into();
                return;
            }
            self.shuttle_speed = speed;
            self.toggle_play(now);
            self.shuttle_speed = speed;
        }
        self.shuttle_speed = speed;
        if let Some(active) = &self.active_player {
            active.player.set_speed(speed);
        }
        self.status = tr_args("Reprodução {}x", &[&format!("{:+.0}", speed)]);
    }

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

    /// Every audible audio event with decoded media, ready for the mixer. Overlapping events on a
    /// track crossfade when automatic crossfades are on.
    fn build_clips(&self) -> Vec<MixClip> {
        // Sub-projects are played as their own tracks, expanded at the right time
        match self.flattened() {
            Some(flat) => self.build_clips_for(&flat.timeline),
            None => self.build_clips_for(&self.timeline),
        }
    }

    fn build_clips_for(&self, timeline: &Timeline) -> Vec<MixClip> {
        let mut clips = Vec::new();
        for (track_index, track) in timeline
            .tracks
            .iter()
            .enumerate()
            .filter(|(_, t)| t.kind == TrackKind::Audio && timeline.audible(t))
        {
            // A muted event is out of playback, so it takes no part in the crossfades either
            let playable: Vec<TrackEvent> =
                track.events.iter().filter(|e| !e.muted).cloned().collect();
            let crossfades = if self.project.settings.auto_crossfade {
                find_crossfades(&playable)
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
            for e in &playable {
                let Some(media_id) = e.media_id else {
                    continue;
                };
                let Some(media) = self.media.get(&media_id) else {
                    continue;
                };
                // The fade-out of the earlier event mirrors the later event's fade-in curve
                let out_curve = crossfades
                    .iter()
                    .filter(|c| c.earlier == e.id)
                    .filter_map(|c| playable.iter().find(|o| o.id == c.later))
                    .map(|later| later.fade_in_curve)
                    .next()
                    .unwrap_or_default();
                let (samples, trim_in) = if neoveg_audio::stretch::is_unity_rate(e.playback_rate) {
                    (Arc::clone(&media.stereo), e.trim_in)
                } else {
                    let rate = neoveg_audio::stretch::clamp_rate(e.playback_rate);
                    let key = (
                        media_id,
                        e.trim_in.to_bits(),
                        e.duration.to_bits(),
                        rate.to_bits(),
                        e.preserve_pitch,
                    );
                    let mut cache = self.stretch_cache.borrow_mut();
                    // Bounded by size, not by count: one stretched clip can be tens of megabytes
                    const STRETCH_CACHE_FLOATS: usize = 32 << 20;
                    let held: usize = cache.values().map(|b| b.len()).sum();
                    if cache.len() > 64 || held > STRETCH_CACHE_FLOATS {
                        cache.clear();
                    }
                    let buf = cache
                        .entry(key)
                        .or_insert_with(|| {
                            neoveg_audio::stretch::stretch_clip_slice(
                                &media.stereo,
                                media.sample_rate,
                                e.trim_in,
                                e.duration,
                                rate,
                                e.preserve_pitch,
                            )
                        })
                        .clone();
                    (buf, 0.0)
                };
                clips.push(MixClip {
                    samples,
                    source_rate: media.sample_rate,
                    start: e.start,
                    duration: e.duration,
                    trim_in,
                    gain: {
                        let sign = if e.invert_phase { -1.0 } else { 1.0 };
                        let level = if e.normalize { e.normalize_gain } else { 1.0 };
                        sign * level
                    },
                    fade_in: e.fade_in,
                    fade_out: e.fade_out,
                    fade_in_curve: e.fade_in_curve,
                    fade_out_curve: e.fade_out_curve,
                    cross_in: longest(&|c| c.later == e.id),
                    cross_in_curve: e.fade_in_curve,
                    cross_out: longest(&|c| c.earlier == e.id),
                    cross_out_curve: out_curve,
                    track: track_index,
                    channels: e.channels,
                    volume: mix_gain_curve(e.volume_envelope.as_ref()),
                    // Only an event at normal speed loops (its source is read directly)
                    loop_len: if e.looping && neoveg_audio::stretch::is_unity_rate(e.playback_rate)
                    {
                        edit::loop_length(media.duration, e.trim_in)
                    } else {
                        0.0
                    },
                });
            }
        }
        clips
    }

    /// The chain the effects panel edits: the master bus, or the selected audio track.
    fn fx_target(&self) -> Option<(FxTarget, String)> {
        if self.fx.master {
            return Some((FxTarget::Master, "barramento master".into()));
        }
        if let Some(id) = self
            .fx
            .bus
            .filter(|id| self.timeline.buses.iter().any(|b| b.id == *id))
        {
            return Some((
                FxTarget::Bus(id),
                neoveg_engine::buses::label(&self.timeline, id),
            ));
        }
        let track = self.timeline.track(self.selected_track?)?;
        (track.kind == TrackKind::Audio).then(|| (FxTarget::Track(track.id), track.name.clone()))
    }

    /// The video chain the effects panel edits, with what can be chosen: the primary selected
    /// event of a video track, the media it plays, the video track (the selected one, or the track
    /// of the selected event) or the whole project.
    fn vfx_target(&self) -> (VideoFxTarget, String, VfxScopes) {
        let video_track = |id: TrackId| {
            self.timeline
                .track(id)
                .filter(|t| t.kind == TrackKind::Video)
        };
        let event = self.selection.primary().and_then(|(track_id, event_id)| {
            let track = video_track(track_id)?;
            let event = track.events.iter().find(|e| e.id == event_id)?;
            Some((track_id, event_id, event.name.clone(), event.media_id))
        });
        let track = event
            .as_ref()
            .map(|(t, ..)| *t)
            .or(self.selected_track)
            .and_then(video_track);
        let media = event.as_ref().and_then(|(.., m)| *m);
        let scopes = VfxScopes {
            event: event.is_some(),
            media: media.is_some(),
            track: track.is_some(),
        };
        match scopes.resolve(self.vfx.scope) {
            VfxScope::Event => match event {
                Some((track_id, event_id, name, _)) => {
                    (VideoFxTarget::Event { track_id, event_id }, name, scopes)
                }
                None => (VideoFxTarget::Project, "Projeto".into(), scopes),
            },
            VfxScope::Media => {
                let id = media.unwrap_or_default();
                let name = self
                    .project
                    .media_pool
                    .iter()
                    .find(|m| m.id == id)
                    .map(|m| {
                        std::path::Path::new(&m.path)
                            .file_name()
                            .map_or_else(|| m.path.clone(), |n| n.to_string_lossy().into_owned())
                    })
                    .unwrap_or_else(|| "mídia".into());
                (
                    VideoFxTarget::Media(id),
                    tr_args("{} (todos os eventos desta mídia)", &[&name]),
                    scopes,
                )
            }
            VfxScope::Track => match track {
                Some(t) => (VideoFxTarget::Track(t.id), t.name.clone(), scopes),
                None => (VideoFxTarget::Project, "Projeto".into(), scopes),
            },
            VfxScope::Project => (VideoFxTarget::Project, "Projeto inteiro".into(), scopes),
        }
    }

    /// The video event whose transition the Transitions tab edits: the primary selection.
    fn transitions_target(&self) -> Option<(TrackId, EventId)> {
        let (track_id, event_id) = self.selection.primary()?;
        trans::get(&self.timeline, track_id, event_id, false).map(|_| (track_id, event_id))
    }

    /// Runs what the user did in the Transitions tab; each action is one undo step. Choosing a
    /// transition on a side with no fade and no overlap also gives that side a one-second fade, in
    /// the same step, so the transition shows at once.
    fn apply_transition_actions(
        &mut self,
        track_id: TrackId,
        event_id: EventId,
        outgoing: bool,
        actions: Vec<TransitionAction>,
    ) {
        let mut changed = false;
        for action in actions {
            let command = match action {
                TransitionAction::Choose(kind) => {
                    let choose = trans::choose(&self.timeline, track_id, event_id, outgoing, kind);
                    let geometry = self.event_geometry(track_id, event_id);
                    let overlap = self.transition_overlap(track_id, event_id);
                    match (choose, geometry) {
                        (Some(choose), Some(g)) => {
                            let length = if outgoing {
                                g.fade_out
                            } else {
                                g.fade_in.max(overlap)
                            };
                            if length <= 0.0 {
                                let fade = (g.duration / 2.0).clamp(0.0, 1.0);
                                let new = if outgoing {
                                    EventGeometry {
                                        fade_out: fade,
                                        ..g
                                    }
                                } else {
                                    EventGeometry { fade_in: fade, ..g }
                                };
                                Some(EditCommand::Batch(vec![
                                    choose,
                                    EditCommand::EditEvent {
                                        from_track: track_id,
                                        to_track: track_id,
                                        event_id,
                                        old: g,
                                        new,
                                    },
                                ]))
                            } else {
                                Some(choose)
                            }
                        }
                        (choose, _) => choose,
                    }
                }
                TransitionAction::Clear => {
                    trans::clear(&self.timeline, track_id, event_id, outgoing)
                }
                TransitionAction::SetParam { param, value } => {
                    trans::set_param(&self.timeline, track_id, event_id, outgoing, &param, value)
                }
                TransitionAction::ResetParam(param) => {
                    trans::reset_param(&self.timeline, track_id, event_id, outgoing, &param)
                }
            };
            if let Some(command) = command {
                changed |= self.history.execute(&mut self.timeline, command);
            }
        }
        if changed {
            self.last_frame_key = None;
            self.last_composite_key = None;
        }
    }

    /// Puts `envelope` as the velocity of the event (one undo step). `true` when the project changed.
    fn set_velocity(
        &mut self,
        track_id: TrackId,
        event_id: EventId,
        envelope: Option<neoveg_engine::TrackEnvelope>,
    ) -> bool {
        let Some(command) = vel::set(&self.timeline, track_id, event_id, envelope) else {
            return false;
        };
        let changed = self.history.execute(&mut self.timeline, command);
        if changed {
            self.last_frame_key = None;
            self.last_composite_key = None;
        }
        changed
    }

    /// Freeze frame at the cursor: the velocity of the event drops to zero there and stays so to the
    /// end of the event. With the event inside a selection, every selected video event under the
    /// cursor freezes, on every track, in one undo step.
    fn freeze_frame_at_cursor(&mut self, track_id: TrackId, event_id: EventId) {
        let targets = if self.selection.contains((track_id, event_id)) {
            self.selection.refs()
        } else {
            vec![(track_id, event_id)]
        };
        let Some(command) = vel::freeze_command(&self.timeline, &targets, self.playhead) else {
            self.status = "Congelar quadro: leve o cursor para dentro de um evento de vídeo".into();
            return;
        };
        if self.history.execute(&mut self.timeline, command) {
            self.last_frame_key = None;
            self.last_composite_key = None;
            self.status = tr_args(
                "Quadro congelado em {}",
                &[&seconds_to_timecode(
                    self.playhead,
                    self.project.settings.fps,
                    false,
                )],
            );
        }
    }

    /// A velocity draft only lives while the mouse button is down (or was just let go): when the panel was hidden or the
    /// selection changed in the middle of a drag, nothing may keep the preview on the draft.
    fn drop_stale_velocity_draft(&mut self, ctx: &egui::Context) {
        // The frame of the release still belongs to the drag: the editor commits it then
        let dragging = ctx.input(|i| i.pointer.any_down() || i.pointer.any_released());
        if self.velocity_draft.is_some() && !dragging {
            self.velocity_draft = None;
            self.velocity_editor.reset();
            self.last_composite_key = None;
        }
    }

    fn apply_velocity_actions(
        &mut self,
        track_id: TrackId,
        event_id: EventId,
        actions: Vec<VelocityAction>,
    ) {
        for action in actions {
            match action {
                VelocityAction::Draft(envelope) => {
                    self.velocity_draft = envelope.map(|envelope| VelocityDraft {
                        track_id,
                        event_id,
                        envelope: Some(envelope),
                    });
                }
                VelocityAction::Commit(envelope) => {
                    self.velocity_draft = None;
                    self.set_velocity(track_id, event_id, envelope);
                }
                VelocityAction::Freeze => self.freeze_frame_at_cursor(track_id, event_id),
            }
        }
    }

    /// Length of the overlap with the event before `event_id` (the automatic crossfade), or `0`.
    fn transition_overlap(&self, track_id: TrackId, event_id: EventId) -> f64 {
        self.timeline.track(track_id).map_or(0.0, |t| {
            find_crossfades(&t.events)
                .iter()
                .filter(|c| c.later == event_id)
                .map(Crossfade::length)
                .fold(0.0, f64::max)
        })
    }

    /// Opens the Pan/Crop editor on a video event.
    fn open_pan_crop(&mut self, track_id: TrackId, event_id: EventId) {
        if pcrop::get(&self.timeline, track_id, event_id).is_none() {
            self.status = "O Pan/Crop vale para eventos de faixas de vídeo".into();
            return;
        }
        self.pan_crop.open_event(track_id, event_id);
    }

    /// Faixas > Movimento da trilha: the selected video track.
    fn open_track_motion(&mut self) {
        match self.selected_track {
            Some(track_id) if tmotion::get(&self.timeline, track_id).is_some() => {
                self.pan_crop.open_track(track_id);
            }
            _ => self.status = "Selecione uma faixa de vídeo para editar o movimento".into(),
        }
    }

    /// Ferramentas > Pan/Crop do evento: the primary selected event.
    fn open_pan_crop_for_selection(&mut self) {
        match self.selection.primary() {
            Some((track_id, event_id)) => self.open_pan_crop(track_id, event_id),
            None => self.status = "Selecione um evento de vídeo para editar o Pan/Crop".into(),
        }
    }

    /// Runs what the user did in the video effects panel; each action is one undo step and the
    /// preview is asked for a new frame.
    fn apply_vfx_actions(&mut self, target: VideoFxTarget, actions: Vec<VfxAction>) {
        let mut changed = false;
        for action in actions {
            let command = match action {
                VfxAction::Add(kind) => vfxe::add_effect(&self.timeline, target, kind),
                VfxAction::Remove(i) => vfxe::remove_effect(&self.timeline, target, i),
                VfxAction::Move { from, to } => vfxe::move_effect(&self.timeline, target, from, to),
                VfxAction::Bypass(i, bypass) => vfxe::set_bypass(&self.timeline, target, i, bypass),
                VfxAction::SetParam {
                    index,
                    param,
                    value,
                } => vfxe::set_param(&self.timeline, target, index, &param, value),
                VfxAction::ResetParam { index, param } => {
                    vfxe::reset_param(&self.timeline, target, index, &param)
                }
                VfxAction::FadeColor(color) => match target {
                    VideoFxTarget::Track(id) => envelope::set_fade_color(&self.timeline, id, color),
                    _ => None,
                },
            };
            if let Some(command) = command {
                changed |= self.history.execute(&mut self.timeline, command);
            }
        }
        if changed {
            self.last_frame_key = None;
        }
    }

    /// Runs what the user did in the effects panel; each action is one undo step. While playing,
    /// the audio restarts at the cursor so the new chain is heard at once.
    fn apply_fx_actions(&mut self, target: FxTarget, actions: Vec<FxAction>) {
        let mut changed = false;
        for action in actions {
            let command = match action {
                FxAction::Duck(duck) => match target {
                    FxTarget::Track(id) => match duck {
                        DuckAction::Enable(on) => fx::enable_ducking(&self.timeline, id, on),
                        DuckAction::Realtime(on) => fx::set_duck_realtime(&self.timeline, id, on),
                        DuckAction::Controller(controller, on) => {
                            fx::set_controller(&self.timeline, id, controller, on)
                        }
                        DuckAction::Param(param, value) => {
                            fx::set_duck_param(&self.timeline, id, param, value)
                        }
                    },
                    FxTarget::Master | FxTarget::Bus(_) => None,
                },
                FxAction::Add(kind) => fx::add_effect(&self.timeline, target, kind),
                FxAction::AddClap(kind) => fx::add_effect(&self.timeline, target, &kind),
                FxAction::BrowseVst3File => {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Plug-in VST3", &["vst3"])
                        .pick_file()
                    {
                        self.fx.start_file_vst3_scan(path);
                    }
                    None
                }
                FxAction::BrowseClapFile => {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Plug-in CLAP", &["clap"])
                        .pick_file()
                    {
                        self.fx.start_file_clap_scan(path);
                    }
                    None
                }
                FxAction::Remove(i) => fx::remove_effect(&self.timeline, target, i),
                FxAction::Move { from, to } => fx::move_effect(&self.timeline, target, from, to),
                FxAction::Bypass(i, bypass) => fx::set_bypass(&self.timeline, target, i, bypass),
                FxAction::SetParam {
                    index,
                    param,
                    value,
                } => fx::set_param(&self.timeline, target, index, &param, value),
                FxAction::ResetParam { index, param } => {
                    fx::reset_param(&self.timeline, target, index, &param)
                }
                FxAction::ToggleAutomate {
                    index,
                    param,
                    value,
                } => envelope::toggle_envelope(
                    &self.timeline,
                    target,
                    AutomationTarget::EffectParam {
                        effect_index: index,
                        param_id: param,
                    },
                    value,
                ),
            };
            if let Some(command) = command {
                changed |= self.history.execute(&mut self.timeline, command);
            }
        }
        if changed {
            self.restart_audio_if_playing();
            self.last_frame_key = None;
        }
    }

    /// Runs what the user did in a track's mixer strip; each action is one undo step.
    fn apply_mix_actions(&mut self, track_id: TrackId, actions: Vec<MixAction>) {
        let mut changed = false;
        for action in actions {
            let command = match action {
                MixAction::Mute(on) => mix::set_muted(&self.timeline, track_id, on),
                MixAction::MuteExclusive => mix::mute_exclusive(&self.timeline, track_id),
                MixAction::Solo(on) => mix::set_solo(&self.timeline, track_id, on),
                MixAction::SoloExclusive => mix::solo_exclusive(&self.timeline, track_id),
                MixAction::Phase(on) => mix::set_invert_phase(&self.timeline, track_id, on),
                MixAction::Volume(db) => mix::set_volume_db(&self.timeline, track_id, db),
                MixAction::Pan(pan) => mix::set_pan(&self.timeline, track_id, pan),
                MixAction::PanLaw(law) => mix::set_pan_law(&self.timeline, law),
            };
            if let Some(command) = command {
                changed |= self.history.execute(&mut self.timeline, command);
            }
        }
        if changed {
            self.restart_audio_if_playing();
            self.last_frame_key = None;
        }
    }

    /// `Z` / `Shift+Z` / `X` / `Shift+X`: mute or solo on the selected track.
    fn run_track_mix_command(&mut self, command: Command) {
        let Some(track_id) = self.selected_track else {
            self.status = "Selecione uma faixa primeiro".into();
            return;
        };
        let Some(track) = self.timeline.track(track_id) else {
            return;
        };
        let action = match command {
            Command::TrackMute => MixAction::Mute(!track.muted),
            Command::TrackMuteExclusive => MixAction::MuteExclusive,
            Command::TrackSolo => {
                if track.kind != TrackKind::Audio {
                    self.status = "Solo só se aplica a faixas de áudio".into();
                    return;
                }
                MixAction::Solo(!track.solo)
            }
            Command::TrackSoloExclusive => {
                if track.kind != TrackKind::Audio {
                    self.status = "Solo só se aplica a faixas de áudio".into();
                    return;
                }
                MixAction::SoloExclusive
            }
            _ => return,
        };
        self.apply_mix_actions(track_id, vec![action]);
    }

    fn run_track_envelope_command(&mut self, command: Command) {
        let Some(track_id) = self.selected_track else {
            self.status = "Selecione uma faixa primeiro".into();
            return;
        };
        let Some(track) = self.timeline.track(track_id) else {
            return;
        };
        let is_video = track.kind == TrackKind::Video;
        let target = FxTarget::Track(track_id);
        let cmd = match command {
            // `V`: the volume of an audio track, the composite level of a video track
            Command::TrackVolumeEnvelope if is_video => {
                envelope::toggle_envelope(&self.timeline, target, AutomationTarget::VideoLevel, 1.0)
            }
            Command::TrackVolumeEnvelope => envelope::toggle_envelope(
                &self.timeline,
                target,
                AutomationTarget::Volume,
                track.volume_db,
            ),
            Command::TrackPanEnvelope if is_video => {
                self.status =
                    "O envelope de pan vale para faixas de áudio; veja Faixas > Envelopes de vídeo"
                        .into();
                return;
            }
            Command::TrackPanEnvelope => {
                envelope::toggle_envelope(&self.timeline, target, AutomationTarget::Pan, track.pan)
            }
            Command::TrackToggleEnvelopes => envelope::toggle_all_envelopes(&self.timeline, target),
            _ => None,
        };
        if let Some(cmd) = cmd {
            if self.history.execute(&mut self.timeline, cmd) {
                self.restart_audio_if_playing();
                self.last_frame_key = None;
            }
        }
    }

    /// Faixas > Envelopes de vídeo: shows or hides (creating it the first time) the level, fade to
    /// color or mute envelope of the selected video track.
    fn toggle_video_envelope(&mut self, auto_target: AutomationTarget, initial: f32) {
        let Some(track_id) = self.selected_track else {
            self.status = "Selecione uma faixa de vídeo primeiro".into();
            return;
        };
        let cmd = envelope::toggle_envelope(
            &self.timeline,
            FxTarget::Track(track_id),
            auto_target,
            initial,
        );
        match cmd {
            Some(cmd) => {
                if self.history.execute(&mut self.timeline, cmd) {
                    self.last_frame_key = None;
                }
            }
            None => self.status = "Os envelopes de vídeo valem para faixas de vídeo".into(),
        }
    }

    /// Commits a finished keyframe drag as a single undo step.
    fn finish_envelope_drag(&mut self) {
        let Some(drag) = self.env_drag.take() else {
            return;
        };
        if let Some(cmd) = envelope::move_keyframe(
            &self.timeline,
            FxTarget::Track(drag.track_id),
            &drag.target,
            drag.index,
            (drag.time, drag.value),
            drag.bounds,
        ) {
            if self.history.execute(&mut self.timeline, cmd) {
                self.restart_audio_if_playing();
                self.last_frame_key = None;
            }
        }
    }

    /// Restarts live playback at the cursor so effect, mixer or undo/redo edits take effect
    /// immediately. Zeroes the peak meter when every track is now silent.
    fn restart_audio_if_playing(&mut self) {
        if !self.playing {
            return;
        }
        if let Some(active) = self.active_player.take() {
            self.playhead = active.player.position_seconds();
            active.player.stop();
        }
        self.active_player = self.start_audio();
        if self.active_player.is_none() {
            self.meter.reset();
        }
    }

    fn undo(&mut self) {
        if self.history.undo(&mut self.timeline) {
            self.selection.retain_existing(&self.timeline);
            self.restart_audio_if_playing();
            self.last_frame_key = None;
        }
    }

    fn redo(&mut self) {
        if self.history.redo(&mut self.timeline) {
            self.selection.retain_existing(&self.timeline);
            self.restart_audio_if_playing();
            self.last_frame_key = None;
        }
    }

    /// Ctrl+R: starts recording the default input at the cursor, or stops and puts the take on the
    /// timeline as a new event.
    fn toggle_recording(&mut self, ctx: &egui::Context) {
        if let Some(recorder) = self.recorder.take() {
            match recorder.stop() {
                Ok(recording) if recording.frames > 0 => {
                    tracing::info!(
                        "[INFO] recorded {:.2} s to {} ({} overruns)",
                        recording.seconds(),
                        recording.path.display(),
                        recording.overruns
                    );
                    self.playhead = self.record_start;
                    self.status = tr_args(
                        "Gravação de {} s salva em {}{}",
                        &[
                            &format!("{:.1}", recording.seconds()),
                            &recording.path.display(),
                            &if recording.overruns > 0 {
                                format!(" ({} trechos perdidos)", recording.overruns)
                            } else {
                                String::new()
                            },
                        ],
                    );
                    self.spawn_media(recording.path, None, ctx);
                }
                Ok(_) => self.status = "Gravação vazia descartada".into(),
                Err(e) => {
                    tracing::error!("[ERROR] recording failed: {e}");
                    self.status = tr_args("Falha na gravação: {}", &[&e]);
                }
            }
            return;
        }
        let Some(dir) = neoveg_common::paths::app_data_dir().map(|d| d.join("recordings")) else {
            self.status = "Não há pasta de dados para a gravação".into();
            return;
        };
        let path = dir.join(format!("Gravacao_{}.wav", autosave::unix_now()));
        match Recorder::start(&path) {
            Ok(recorder) => {
                self.status = tr_args(
                    "Gravando de {} (Ctrl+R para parar)",
                    &[&recorder.info().device],
                );
                self.record_start = self.playhead;
                self.recorder = Some(recorder);
            }
            Err(e) => {
                tracing::error!("[ERROR] cannot record: {e}");
                self.status = tr_args("Não foi possível gravar: {}", &[&e]);
            }
        }
    }

    /// The clicks of the metronome from the cursor on (none when it is off).
    fn metronome_clips(&self) -> Vec<MixClip> {
        if !self.metronome_on {
            return Vec::new();
        }
        let settings = &self.project.settings;
        let metronome = Metronome {
            bpm: settings.bpm,
            beats_per_bar: settings.beats_per_bar,
            volume: self.metronome_volume,
        };
        let end = navigation::project_end(&self.timeline).max(self.playhead + 60.0);
        click_clips(&metronome, self.playhead, end, settings.sample_rate)
    }

    fn start_audio(&mut self) -> Option<ActivePlayer> {
        let mut clips = self.build_clips();
        clips.extend(self.metronome_clips());
        if clips.is_empty() {
            return None;
        }
        match AudioPlayer::new(
            clips,
            &fx_panel::fx_plan(&self.timeline),
            Arc::clone(&self.meter),
            Arc::clone(&self.loudness),
        ) {
            Ok(player) => {
                player.set_volume(self.master_volume);
                player.set_speed(self.shuttle_speed);
                player.set_loop(self.loop_args());
                player.play_from(self.playhead);
                Some(ActivePlayer { player })
            }
            Err(e) => {
                tracing::error!("[ERROR] audio output unavailable: {e}");
                self.status = tr_args("Saída de áudio indisponível: {}", &[&e]);
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
                // The picture follows the sound at the loudspeaker, smoothly, not the buffer step
                self.playhead = neoveg_audio::clock::steady(
                    self.playhead,
                    active.player.synced_position(),
                    self.shuttle_speed,
                );
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

    /// Re-reads the saved layouts from the folder.
    fn refresh_layouts(&mut self) {
        self.layouts = self
            .layouts_dir
            .as_deref()
            .map(crate::layouts::list)
            .unwrap_or_default();
        self.layouts_loaded = true;
    }

    /// Reads the saved layouts the first time they are needed.
    fn ensure_layouts(&mut self) {
        if !self.layouts_loaded {
            self.refresh_layouts();
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() || self.capture_for.is_some() {
            return;
        }
        let now = ctx.input(|i| i.time);
        // `Alt+D` / `Ctrl+Alt+D` then a number (not the keypad's): the number picks the layout
        if let Some((save, until)) = self.layout_chord.take() {
            if now <= until {
                const DIGITS: [egui::Key; 9] = [
                    egui::Key::Num1,
                    egui::Key::Num2,
                    egui::Key::Num3,
                    egui::Key::Num4,
                    egui::Key::Num5,
                    egui::Key::Num6,
                    egui::Key::Num7,
                    egui::Key::Num8,
                    egui::Key::Num9,
                ];
                let digit = ctx.input_mut(|i| {
                    DIGITS
                        .iter()
                        .position(|k| i.consume_key(egui::Modifiers::NONE, *k))
                });
                match digit {
                    Some(index) => {
                        let slot = index as u8 + 1;
                        self.window_request = Some(if save {
                            WindowAction::SaveSlot(slot)
                        } else {
                            WindowAction::RecallSlot(slot)
                        });
                        return;
                    }
                    None => self.layout_chord = Some((save, until)),
                }
            }
        }
        // Esc leaves the keyboard trim mode before it can mean anything else
        if self.trim_edge.is_some()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.trim_edge = None;
            self.status = "Aparo por teclado encerrado".into();
            return;
        }
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
            Command::FileNew => self.new_tab(),
            Command::FileCreateSubproject => self.create_subproject_from_selection(),
            Command::FileOpen => self.open_project(ctx),
            Command::FileSave => self.save_project(false),
            Command::FileSaveAs => self.save_project(true),
            Command::FileProperties => self.open_project_properties(),
            Command::EditUndo => self.undo(),
            Command::EditRedo => self.redo(),
            Command::EditDelete => self.delete_selection(),
            Command::EditRippleDelete => self.ripple_delete_selection(),
            Command::OptionsAutoRipple => {
                self.auto_ripple = !self.auto_ripple;
                self.status = if self.auto_ripple {
                    tr_args(
                        "Ripple automático ligado ({}): excluir fecha o espaço",
                        &[&self.ripple_scope.label()],
                    )
                } else {
                    "Ripple automático desligado".into()
                };
            }
            Command::RippleAffectedTracks => self.ripple_key(RippleScope::AffectedTracks),
            Command::RippleWithMarkers => self.ripple_key(RippleScope::AffectedTracksAndMarks),
            Command::RippleAll => self.ripple_key(RippleScope::AllTracksAndMarks),
            Command::ViewFocusTimeline => {
                self.window_request = Some(WindowAction::Toggle(Tab::Timeline));
            }
            Command::ViewLayoutLoad | Command::ViewLayoutSave => {
                let save = command == Command::ViewLayoutSave;
                self.layout_chord = Some((save, now + 3.0));
                self.status = if save {
                    "Salvar layout: aperte um número de 1 a 9".into()
                } else {
                    "Carregar layout: aperte um número de 1 a 9".into()
                };
            }
            Command::ViewProjectMedia => {
                self.media_tab = 0;
                self.window_request = Some(WindowAction::Toggle(Tab::Media));
            }
            Command::ViewTrimmer => {
                self.media_tab = 1;
                self.window_request = Some(WindowAction::Toggle(Tab::Media));
            }
            Command::ViewMasterBus => {
                self.window_request = Some(WindowAction::Toggle(Tab::MasterBus))
            }
            Command::ViewPreview => self.window_request = Some(WindowAction::Toggle(Tab::Preview)),
            Command::ViewLoudnessMeters => {
                self.window_request = Some(WindowAction::Toggle(Tab::Loudness));
            }
            Command::ViewVideoScopes => {
                self.window_request = Some(WindowAction::Toggle(Tab::Scopes))
            }
            Command::ViewAudioEffects => {
                self.window_request = Some(WindowAction::Toggle(Tab::AudioFx))
            }
            Command::ViewVideoEffects => {
                self.window_request = Some(WindowAction::Toggle(Tab::VideoFx))
            }
            Command::ViewTransitions => {
                self.window_request = Some(WindowAction::Toggle(Tab::Transitions))
            }
            Command::TransportRecord => self.toggle_recording(ctx),
            Command::EditSplit => self.split_at_playhead(),
            Command::InsertAudioTrack => self.add_track(TrackKind::Audio),
            Command::InsertVideoTrack => self.add_track(TrackKind::Video),
            Command::InsertAdjustmentEvent => self.insert_adjustment_event(),
            Command::InsertAudioBus => self.add_audio_bus(),
            Command::CursorLeftByFrame if self.trim_edge.is_some() => {
                self.nudge_trim_edge(-1.0, None)
            }
            Command::CursorRightByFrame if self.trim_edge.is_some() => {
                self.nudge_trim_edge(1.0, None)
            }
            Command::EditTrimEdgeStretchLeft => {
                self.nudge_trim_edge(-1.0, Some(neoveg_engine::keytrim::TrimMode::Stretch))
            }
            Command::EditTrimEdgeStretchRight => {
                self.nudge_trim_edge(1.0, Some(neoveg_engine::keytrim::TrimMode::Stretch))
            }
            Command::EditTrimEdgeRollLeft => {
                self.nudge_trim_edge(-1.0, Some(neoveg_engine::keytrim::TrimMode::Roll))
            }
            Command::EditTrimEdgeRollRight => {
                self.nudge_trim_edge(1.0, Some(neoveg_engine::keytrim::TrimMode::Roll))
            }
            Command::CursorLeftByFrame => self.step_frames(-1.0, now),
            Command::CursorRightByFrame => self.step_frames(1.0, now),
            Command::EditTrimEdgePrevious => self.select_trim_edge(false),
            Command::EditTrimEdgeNext => self.select_trim_edge(true),
            Command::TransportPlayToggle => self.toggle_play(now),
            Command::OptionsSnapping => {
                self.snapping = !self.snapping;
                self.status = if self.snapping {
                    "Alinhamento ligado".into()
                } else {
                    "Alinhamento desligado".into()
                };
            }
            Command::OptionsSnapToGrid => {
                self.snap_sources.grid = !self.snap_sources.grid;
                self.prefs.snap_to_grid = self.snap_sources.grid;
                self.persist_preferences();
                self.status = Self::toggle_text("Alinhar à grade", self.snap_sources.grid);
            }
            Command::OptionsSnapToMarkers => {
                self.snap_sources.markers = !self.snap_sources.markers;
                self.prefs.snap_to_markers = self.snap_sources.markers;
                self.persist_preferences();
                self.status = Self::toggle_text("Alinhar a marcadores", self.snap_sources.markers);
            }
            Command::OptionsSnapToAllEvents => {
                self.snap_sources.all_events = !self.snap_sources.all_events;
                self.prefs.snap_to_all_events = self.snap_sources.all_events;
                self.persist_preferences();
                self.status = Self::toggle_text(
                    "Alinhar a eventos de outras faixas",
                    self.snap_sources.all_events,
                );
            }
            Command::OptionsQuantizeToFrames => {
                self.quantize_frames = !self.quantize_frames;
                self.prefs.quantize_to_frames = self.quantize_frames;
                self.persist_preferences();
                self.status = Self::toggle_text("Quantizar para quadros", self.quantize_frames);
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
            Command::EditPasteAttributes => self.paste_attributes(Attributes::ALL),
            Command::EditPasteAttributesSelective => {
                if self.clipboard.is_some() {
                    self.attributes_dialog = Some(Attributes::ALL);
                } else {
                    self.status = "A área de transferência está vazia".into();
                }
            }
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
            Command::EditGroup => self.group_selection(),
            Command::EditUngroup => self.ungroup_selection(),
            Command::EditRemoveFromGroup => self.remove_selection_from_groups(),
            Command::EditSelectGroup => self.select_group_members(),
            Command::FileSaveIncremental => self.save_incremental(),
            Command::InsertTime => self.insert_time_dialog = Some(String::new()),
            Command::TrackFocusPrevious => self.move_track_focus(-1),
            Command::TrackFocusNext => self.move_track_focus(1),
            Command::EditIgnoreGrouping => self.toggle_ignore_grouping(),
            Command::EditToolNormal => self.set_edit_tool(EditTool::Normal),
            Command::EditToolNext => self.set_edit_tool(self.edit_tool.step(true)),
            Command::EditToolPrevious => self.set_edit_tool(self.edit_tool.step(false)),
            Command::EditSwitchMute => self.toggle_event_switch(Switch::Mute),
            Command::EditSwitchLock => self.toggle_event_switch(Switch::Lock),
            Command::EditSwitchInvertPhase => self.toggle_event_switch(Switch::InvertPhase),
            Command::EditSwitchLoop => self.toggle_event_switch(Switch::Loop),
            Command::EditSwitchNormalize => self.toggle_event_switch(Switch::Normalize),
            Command::EditNormalizeRecalculate => self.recalculate_normalize(),
            Command::TransportLoop => self.toggle_loop(),
            Command::TransportShuttleForward => self.shuttle(true, now),
            Command::TransportShuttleReverse => self.shuttle(false, now),
            Command::TransportShuttlePause => {
                self.shuttle_speed = 1.0;
                self.stop();
            }
            Command::SelectionLoopRegion => self.set_loop_region(),
            Command::InsertMarker => self.insert_marker(),
            Command::InsertRegion => self.insert_region(),
            Command::CursorProjectStart => self.go_to_time(0.0, now),
            Command::CursorProjectEnd => {
                self.go_to_time(navigation::project_end(&self.timeline), now);
            }
            Command::CursorSelectionStart => self.go_to_selection_edge(false, now),
            Command::CursorSelectionEnd => self.go_to_selection_edge(true, now),
            Command::CursorPreviousEdit => self.jump_to_edit_point(false, now),
            Command::CursorNextEdit => self.jump_to_edit_point(true, now),
            Command::CursorCenterView => self.center_view_on_cursor(),
            Command::CursorGoTo => {
                self.goto_dialog = Some(seconds_to_timecode(
                    self.playhead,
                    self.project.settings.fps,
                    is_drop_frame_rate(self.project.settings.fps),
                ));
            }
            Command::CursorPreviousMarker => self.jump_to_mark(false, now),
            Command::CursorNextMarker => self.jump_to_mark(true, now),
            Command::TrackMute
            | Command::TrackMuteExclusive
            | Command::TrackSolo
            | Command::TrackSoloExclusive => self.run_track_mix_command(command),
            Command::TrackVolumeEnvelope
            | Command::TrackPanEnvelope
            | Command::TrackToggleEnvelopes => self.run_track_envelope_command(command),
            c if c.marker_number().is_some() => {
                self.jump_to_marker_number(c.marker_number().unwrap_or(1), now);
            }
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
    /// The startup screen, until it has faded out.
    splash: Option<Splash>,
    /// Name of the graphics adapter (known when the window is created).
    gpu: String,
    /// Panels that live in their own native window.
    detached: Vec<detached::DetachedTab>,
    /// The saved panel windows were put back (done on the first frame after the startup screen).
    detached_restored: bool,
}

/// What [`toggle_window`] did.
#[derive(Debug, PartialEq, Eq)]
enum WindowChange {
    /// The window was behind another tab and is now in front.
    Raised,
    /// The window was in front and is now closed.
    Closed,
    /// The window was closed and is now back.
    Opened,
}

/// Show / hide a window: bring it to the front when it is hidden behind a tab, close it when it is
/// in front, and put it back in the focused panel when it was closed.
fn toggle_window(dock: &mut DockState<Tab>, tab: Tab) -> WindowChange {
    match dock.find_tab(&tab) {
        Some((surface, node, index)) => {
            let in_front = matches!(
                &dock[surface][node],
                Node::Leaf { active, .. } if *active == index
            );
            if in_front {
                dock.remove_tab((surface, node, index));
                WindowChange::Closed
            } else {
                dock.set_active_tab((surface, node, index));
                WindowChange::Raised
            }
        }
        None => {
            dock.push_to_focused_leaf(tab);
            WindowChange::Opened
        }
    }
}

/// The workspace Neoveg opens with.
fn default_dock() -> DockState<Tab> {
    let mut dock = DockState::new(vec![Tab::Preview]);
    let surface = dock.main_surface_mut();
    let [top, _timeline] = surface.split_below(NodeIndex::root(), 0.52, vec![Tab::Timeline]);
    let [preview, media] = surface.split_left(top, 0.28, vec![Tab::Media]);
    // The effects of the selected track sit under the project media
    surface.split_below(
        media,
        0.55,
        vec![Tab::AudioFx, Tab::VideoFx, Tab::Transitions],
    );
    let [_, master] = surface.split_right(preview, 0.82, vec![Tab::MasterBus]);
    // The loudness readouts live in their own panel under the master bus
    // and the video scopes are a tab beside them
    surface.split_below(master, 0.55, vec![Tab::Loudness, Tab::Scopes]);
    dock
}

/// The window arrangement for sound work: the mixer, the loudness meters and the effects get the
/// room, the timeline is tall, and the preview shares a tab with the project media.
fn audio_dock() -> DockState<Tab> {
    let mut dock = DockState::new(vec![Tab::Media, Tab::Preview]);
    let surface = dock.main_surface_mut();
    let [top, _timeline] = surface.split_below(NodeIndex::root(), 0.40, vec![Tab::Timeline]);
    let [_, master] = surface.split_right(top, 0.30, vec![Tab::MasterBus]);
    let [_, effects] = surface.split_right(
        master,
        0.40,
        vec![Tab::AudioFx, Tab::VideoFx, Tab::Transitions],
    );
    surface.split_below(effects, 0.50, vec![Tab::Loudness, Tab::Scopes]);
    dock
}

impl NeovegApp {
    /// The edit details table: a click on a row selects the event and moves the cursor to it.
    fn run_details_window(&mut self, ctx: &egui::Context) {
        if !self.state.show_details {
            return;
        }
        let rows = neoveg_engine::details::rows(&self.state.timeline);
        let selected = self.state.selection.refs();
        let fps = self.state.project.settings.fps;
        let drop_frame = self.state.drop_frame();
        let mut open = true;
        let actions =
            crate::details_window::show(ctx, &mut open, &rows, &selected, fps, drop_frame);
        self.state.show_details = open;
        for action in actions {
            match action {
                crate::details_window::DetailsAction::Go(event, start) => {
                    self.state.selected_track = Some(event.0);
                    self.state.selection.set([event]);
                    let now = ctx.input(|i| i.time);
                    self.state.go_to_time(start, now);
                }
                crate::details_window::DetailsAction::Copy => {
                    ctx.copy_text(neoveg_engine::details::to_tsv(&rows, fps, drop_frame));
                    self.state.status = "Tabela de detalhes copiada".into();
                }
            }
        }
    }

    /// The project notes window: what the user did to the notes, and the cursor jump of "Ir".
    fn run_notes_window(&mut self, ctx: &egui::Context) {
        let fps = self.state.project.settings.fps;
        let drop_frame = self.state.drop_frame();
        let actions = self
            .state
            .notes
            .show(ctx, &self.state.project.notes, fps, drop_frame);
        for action in actions {
            let effect = crate::notes_window::apply_note_action(
                &mut self.state.project.notes,
                action,
                self.state.playhead,
            );
            if let Some(time) = effect.jump_to {
                let now = ctx.input(|i| i.time);
                self.state.go_to_time(time, now);
            }
            if effect.changed {
                // The notes are not an undoable edit, but the project has changed
                self.state.forced_dirty = true;
            }
        }
    }

    /// The welcome screen and the project properties window.
    fn run_start_dialogs(&mut self, ctx: &egui::Context) {
        use crate::welcome::WelcomeAction;
        self.state.poll_match_media();
        let actions = self
            .state
            .project_dialog
            .show(ctx, &self.state.prefs.custom_templates);
        self.state.run_project_actions(actions);

        if !self.state.welcome.open {
            return;
        }
        let recents =
            crate::welcome::recent_entries(&self.state.prefs.recent_projects, &|p| p.exists());
        let actions = self.state.welcome.show(
            ctx,
            &recents,
            &self.state.prefs.custom_templates,
            self.state.prefs.show_welcome_on_startup,
        );
        for action in actions {
            match action {
                WelcomeAction::OpenRecent(path) => {
                    self.state.welcome.open = false;
                    self.state.open_recent(std::path::Path::new(&path), ctx);
                }
                WelcomeAction::ForgetRecent(path) => {
                    self.state.prefs.remove_recent_project(&path);
                    self.state.persist_preferences();
                }
                WelcomeAction::OpenFile => {
                    self.state.welcome.open = false;
                    self.state.open_project(ctx);
                }
                WelcomeAction::NewProject { settings, layout } => {
                    self.state.welcome.open = false;
                    // The blank project the program started with is used as it is
                    if self.state.is_blank_project() {
                        self.state.project.settings = *settings;
                        self.state.format_chosen = true;
                    } else {
                        self.state.new_project_with(*settings);
                    }
                    self.dock = match layout {
                        crate::welcome::StartLayout::Video => default_dock(),
                        crate::welcome::StartLayout::Audio => audio_dock(),
                    };
                }
                WelcomeAction::ShowAtStartup(show) => {
                    self.state.prefs.show_welcome_on_startup = show;
                    self.state.persist_preferences();
                }
                WelcomeAction::Close => self.state.welcome.open = false,
            }
        }
    }

    /// Puts the dock of the saved layout at `path` on screen.
    fn load_layout(&mut self, path: &std::path::Path) {
        self.state.ensure_layouts();
        let Some(found) = self.state.layouts.iter().find(|l| l.path == path) else {
            self.state.status = "Esse layout não existe mais".into();
            return;
        };
        let (layout, name) = (found.file.dock.clone(), found.file.name.clone());
        self.detached.retain(|d| layout.find_tab(&d.tab).is_none());
        self.dock = layout;
        self.state.current_layout = Some(path.to_path_buf());
        self.state.status = tr_args("Layout {} carregado", &[&name]);
    }

    /// Saves the dock as the layout `name` (with the shortcut number `slot`).
    fn save_layout(&mut self, name: &str, slot: Option<u8>) {
        let Some(dir) = self.state.layouts_dir.clone() else {
            self.state.status = "Não há pasta para guardar o layout".into();
            return;
        };
        match crate::layouts::save(&dir, name, slot, &self.dock) {
            Ok(path) => {
                self.state.current_layout = Some(path);
                self.state.status = tr_args("Layout {} salvo", &[&name]);
            }
            Err(e) => self.state.status = tr_args("Não foi possível salvar o layout: {}", &[&e]),
        }
        self.state.refresh_layouts();
    }

    /// Carries out a window command: show / hide a window, or save / load a layout.
    fn apply_window_action(&mut self, action: WindowAction) {
        match action {
            WindowAction::Toggle(tab) if self.detached.iter().any(|d| d.tab == tab) => {
                // A panel in its own window: bring that window to the front
                if let Some(ctx) = self.state.tabs.ctx.clone() {
                    ctx.send_viewport_cmd_to(
                        detached::viewport_id(tab),
                        egui::ViewportCommand::Focus,
                    );
                }
            }
            WindowAction::Show(tab) => {
                if self.detached.iter().any(|d| d.tab == tab) {
                    if let Some(ctx) = self.state.tabs.ctx.clone() {
                        ctx.send_viewport_cmd_to(
                            detached::viewport_id(tab),
                            egui::ViewportCommand::Focus,
                        );
                    }
                } else {
                    match self.dock.find_tab(&tab) {
                        Some(found) => self.dock.set_active_tab(found),
                        None => self.dock.push_to_focused_leaf(tab),
                    }
                }
            }
            WindowAction::Toggle(tab) => {
                self.state.status = match toggle_window(&mut self.dock, tab) {
                    WindowChange::Closed => tr_args("{} fechada", &[&tab.title()]),
                    WindowChange::Opened => tr_args("{} aberta", &[&tab.title()]),
                    WindowChange::Raised => return,
                };
            }
            WindowAction::LoadLayout(path) => self.load_layout(&path),
            WindowAction::ReloadLayout => match self.state.current_layout.clone() {
                Some(path) => self.load_layout(&path),
                None => self.state.status = "Nenhum layout selecionado".into(),
            },
            WindowAction::SaveLayout { name, slot } => self.save_layout(&name, slot),
            WindowAction::SaveCurrentLayout => {
                self.state.ensure_layouts();
                let current = self.state.current_layout.clone();
                let found = self
                    .state
                    .layouts
                    .iter()
                    .find(|l| Some(&l.path) == current.as_ref())
                    .map(|l| (l.file.name.clone(), l.file.slot));
                match found {
                    Some((name, slot)) => self.save_layout(&name, slot),
                    None => self.state.status = "Nenhum layout selecionado".into(),
                }
            }
            WindowAction::RecallSlot(slot) => {
                self.state.ensure_layouts();
                match crate::layouts::by_slot(&self.state.layouts, slot).map(|l| l.path.clone()) {
                    Some(path) => self.load_layout(&path),
                    None => self.state.status = tr_args("Nenhum layout no atalho {}", &[&slot]),
                }
            }
            WindowAction::SaveSlot(slot) => {
                self.state.ensure_layouts();
                let name = crate::layouts::by_slot(&self.state.layouts, slot)
                    .map_or_else(|| tr_args("Layout {}", &[&slot]), |l| l.file.name.clone());
                self.save_layout(&name, Some(slot));
            }
            WindowAction::DeleteLayout(path) => {
                match crate::layouts::remove(&path) {
                    Ok(()) => self.state.status = "Layout excluído".into(),
                    Err(e) => self.state.status = e,
                }
                if self.state.current_layout.as_ref() == Some(&path) {
                    self.state.current_layout = None;
                }
                self.state.refresh_layouts();
            }
            WindowAction::SetLayoutSlot(path, slot) => {
                if let Some(dir) = self.state.layouts_dir.clone() {
                    if let Err(e) = crate::layouts::set_slot(&dir, &path, slot) {
                        self.state.status = e;
                    }
                }
                self.state.refresh_layouts();
            }
            WindowAction::DefaultLayout => {
                self.dock = default_dock();
                let dock = &self.dock;
                self.detached.retain(|d| dock.find_tab(&d.tab).is_none());
                self.state.status = "Layout padrão restaurado".into();
            }
        }
    }

    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        ui_theme::apply(&cc.egui_ctx, &Theme::default());
        icons::install(&cc.egui_ctx);

        let gpu = cc.wgpu_render_state.as_ref().map_or_else(
            || "Renderizador padrão".to_owned(),
            |render| {
                let info = render.adapter.get_info();
                format!("{} · {:?}", info.name, info.backend)
            },
        );
        Self {
            dock: default_dock(),
            state: AppState::new(),
            splash: Some(Splash::new(&STARTUP_STEPS, &gpu)),
            gpu,
            detached: Vec::new(),
            detached_restored: false,
        }
    }
}

impl eframe::App for NeovegApp {
    fn on_exit(&mut self) {
        self.sync_detached_prefs(true);
    }

    /// A press and release of the right button where the menu button of an event header asked for it.
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if let Some(pos) = self.state.pending_context_click.take() {
            let modifiers = egui::Modifiers::NONE;
            raw_input.events.push(egui::Event::PointerMoved(pos));
            for pressed in [true, false] {
                raw_input.events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers,
                });
            }
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = ctx.input(|i| i.time);
        if let Some(splash) = &mut self.splash {
            // One startup step per frame, so the screen keeps animating while the program loads
            if let Some(step) = splash.pacing.pending_step(now) {
                let detail = match step {
                    0 => self.state.load_preferences(),
                    1 => self.gpu.clone(),
                    2 => self.state.probe_audio(),
                    3 => tr_args(
                        "{} efeitos prontos",
                        &[&neoveg_audio::dsp::effects::warm_up(48_000.0)],
                    ),
                    4 => self.state.start_frame_worker(ctx),
                    _ => self.state.find_recoveries(),
                };
                splash.step_done(now, detail);
                // The startup screen already wears the user's theme
                sync_theme(ctx, &mut self.state);
            }
            if splash.show(ctx, now) {
                self.splash = None;
            } else {
                return;
            }
        }
        self.show(ctx);
    }
}

impl NeovegApp {
    /// One frame of the workspace, everything after the startup screen. The window draws it through
    /// `update`; the offscreen screenshots draw it directly.
    fn show(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if !self.detached_restored {
            self.detached_restored = true;
            self.restore_detached(ctx);
        }
        if let Some(action) = self.state.window_request.take() {
            self.apply_window_action(action);
        }
        if let Some(recorder) = &self.state.recorder {
            let peak = recorder.take_peak();
            self.state.status = tr_args(
                "Gravando {} s · nível {} dBFS (Ctrl+R para parar)",
                &[&format!("{:.1}", recorder.seconds()), &fmt_db(peak)],
            );
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        self.state.maybe_autosave(now);
        self.state.drop_stale_velocity_draft(ctx);
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
        if let Some(whisper_done) = self.state.captions_panel.poll_whisper() {
            self.state.apply_caption_actions(vec![whisper_done], now);
        }
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
        egui::TopBottomPanel::top("main_toolbar").show(ctx, |ui| main_toolbar(ui, &mut self.state));
        if self.state.tabs.ctx.is_none() {
            self.state.tabs.ctx = Some(ctx.clone());
        }
        egui::TopBottomPanel::top("project_tabs")
            .show(ctx, |ui| project_tab_bar(ui, &mut self.state));
        close_tab_window(ctx, &mut self.state);
        if let Some(path) = self.state.tabs.open_request.take() {
            self.state.open_path_in_tab(&path, ctx);
        }
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(tr_text(&self.state.status));
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
                // egui_dock can only put its strip on top, so it is hidden (height 0) and every group
                // of windows draws its own footer strip (`footer_tabs`, `dock_footer`)
                style.tab_bar.height = 0.0;
                self.state.dock_footer.refresh(&self.dock, ui.max_rect());
                DockArea::new(&mut self.dock).style(style).show_inside(
                    ui,
                    &mut Viewer {
                        state: &mut self.state,
                    },
                );
                self.state.dock_footer.apply(&mut self.dock, ui.ctx());
            });

        self.promote_floating_windows(ctx);
        self.show_detached(ctx);
        sync_theme(ctx, &mut self.state);
        preferences_window(ctx, &mut self.state);
        keymap_window(ctx, &mut self.state);
        repeat_paste_window(ctx, &mut self.state);
        paste_attributes_window(ctx, &mut self.state);
        layout_dialogs(ctx, &mut self.state);
        convert_offer_window(ctx, &mut self.state);
        missing_media_window(ctx, &mut self.state);
        media_error_window(ctx, &mut self.state);
        found_media_window(ctx, &mut self.state);
        goto_window(ctx, &mut self.state);
        metronome_window(ctx, &mut self.state);
        grade_window(ctx, &mut self.state);
        pan_crop_editor(ctx, &mut self.state);
        chroma_key_window(ctx, &mut self.state);
        recovery_window(ctx, &mut self.state);
        exit_window(ctx, &mut self.state);
        inspector_window(ctx, &mut self.state);
        credits_window(ctx, &mut self.state);
        let log_dir = crate::log_viewer::default_log_dir();
        self.state.log_viewer.show(ctx, &log_dir);
        insert_time_window(ctx, &mut self.state);
        self.run_start_dialogs(ctx);
        self.run_notes_window(ctx);
        self.run_details_window(ctx);
        if let Some(request) = self.state.slideshow.show(ctx, self.state.playhead) {
            self.state
                .insert_slideshow(&request.slides, request.settings);
        }
        let settings = &self.state.project.settings;
        let (width, height, fps) = (settings.width, settings.height, settings.fps);
        let editing = self.state.generator_dialog.editing.clone();
        if let Some(spec) = self.state.generator_dialog.show(ctx, width, height, fps) {
            match editing {
                Some((media_id, _)) => self.state.apply_generated_edit(media_id, spec, ctx),
                None => self.state.insert_generated(spec, ctx),
            }
            self.state.generator_dialog.editing = None;
        }
        let has_loop_region = self.state.loop_region.is_some();
        let duration = self.state.timeline.duration();
        let style_before = self.state.project.settings.caption_style.clone();
        let mut style = style_before.clone();
        let launch = self.state.render_dialog.show(
            ctx,
            width,
            height,
            fps,
            has_loop_region,
            duration,
            &mut style,
        );
        if style != style_before {
            // The style is part of the project, but not an edit that can be undone
            self.state.project.settings.caption_style = style;
            self.state.forced_dirty = true;
        }
        if let Some(request) = launch {
            self.state.start_render(request, ctx);
        }
    }
}

/// The volume envelope of an event as the mixer plays it; `None` when there is nothing to apply.
fn mix_gain_curve(
    envelope: Option<&neoveg_engine::envelope::TrackEnvelope>,
) -> Option<Arc<neoveg_audio::mixer::GainCurve>> {
    use neoveg_audio::mixer::{GainCurve, GainPoint};
    use neoveg_engine::envelope::EnvelopeShape;
    let env = envelope.filter(|e| neoveg_engine::event_volume::is_active(Some(e)))?;
    let points = env
        .points
        .iter()
        .map(|p| GainPoint {
            time: p.time,
            db: p.value,
            curve: match p.shape {
                EnvelopeShape::Linear => Some(FadeCurve::Linear),
                EnvelopeShape::Fast => Some(FadeCurve::Fast),
                EnvelopeShape::Slow => Some(FadeCurve::Slow),
                EnvelopeShape::Smooth => Some(FadeCurve::Smooth),
                EnvelopeShape::Sharp => Some(FadeCurve::Sharp),
                EnvelopeShape::Hold => None,
            },
        })
        .collect();
    Some(Arc::new(GainCurve { points }))
}

/// Width, height and frame rate of a media file, or why they could not be read.
type MediaFormat = Result<(u32, u32, f64), String>;

/// Decodes an audio file and builds its waveform source.
fn load_audio(path: &std::path::Path) -> Result<LoadedMedia, String> {
    // Decoded block by block: a long file goes to a temporary file, and the waveform and the
    // mixer share the one copy
    let audio = decode_to_store(path)?;
    let duration = audio.duration_seconds();
    let store = Arc::new(audio.store);
    let waveform = WaveformSource::from_pcm(audio.sample_rate, Arc::clone(&store))?;
    Ok(LoadedMedia {
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        path: path.to_path_buf(),
        waveform: Arc::new(waveform),
        sample_rate: audio.sample_rate,
        channels: audio.channels,
        duration,
        stereo: store,
    })
}

fn menu_bar(ui: &mut egui::Ui, state: &mut AppState) {
    // the interface language follows the preference, also right after the selector changes it
    neoveg_common::i18n::set_language(&state.prefs.language);
    state.cjk_fonts.update(ui.ctx(), &state.prefs.language);
    egui::menu::bar(ui, |ui| {
        ui.menu_button(tr("Arquivo"), |ui| {
            // New and open
            command_item(state, ui, Command::FileNew);
            if ui
                .button(state.with_hint(tr("Abrir…"), Command::FileOpen))
                .clicked()
            {
                let ctx = ui.ctx().clone();
                state.open_project(&ctx);
                ui.close_menu();
            }
            if ui.button(tr("Abrir em nova aba…")).clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Projeto Neoveg", &[project_file::EXTENSION])
                    .pick_file()
                {
                    state.tabs.open_request = Some(path);
                }
                ui.close_menu();
            }
            recent_projects_menu(state, ui);
            command_item(state, ui, Command::FileCreateSubproject);
            ui.separator();

            // Save
            if ui
                .button(state.with_hint(tr("Salvar"), Command::FileSave))
                .clicked()
            {
                state.save_project(false);
                ui.close_menu();
            }
            if ui
                .button(state.with_hint(tr("Salvar como…"), Command::FileSaveAs))
                .clicked()
            {
                state.save_project(true);
                ui.close_menu();
            }
            command_item(state, ui, Command::FileSaveIncremental);
            ui.separator();

            // Import and export, one submenu each
            let has_events = state.timeline.tracks.iter().any(|t| !t.events.is_empty());
            if ui
                .add_enabled(
                    !state.missing.is_empty(),
                    egui::Button::new(tr("Mídia ausente…")),
                )
                .clicked()
            {
                state.missing_open = true;
                ui.close_menu();
            }
            ui.menu_button(tr("Importar"), |ui| {
                if ui.button(tr("Mídia…")).clicked() {
                    let ctx = ui.ctx().clone();
                    state.import_media(&ctx);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button(tr("Lista de edição EDL (CMX 3600)…")).clicked() {
                    state.import_edl();
                    ui.close_menu();
                }
                if ui.button("XML (Final Cut Pro 7)…").clicked() {
                    state.import_fcp7();
                    ui.close_menu();
                }
                if ui.button("FCPXML (Final Cut Pro)…").clicked() {
                    state.import_fcpxml();
                    ui.close_menu();
                }
                if ui.button("OpenTimelineIO (.otio)…").clicked() {
                    state.import_otio();
                    ui.close_menu();
                }
                if ui.button("AAF (Advanced Authoring Format)…").clicked() {
                    state.import_aaf();
                    ui.close_menu();
                }
                ui.separator();
                if ui.button(tr("Legendas (SRT, WebVTT)…")).clicked() {
                    state.import_captions();
                    ui.close_menu();
                }
            });
            ui.menu_button(tr("Exportar"), |ui| {
                if ui.button(tr("Renderizar como… (Make Movie)")).clicked() {
                    state.render_dialog.open = true;
                    ui.close_menu();
                }
                ui.separator();
                if ui
                    .add_enabled(
                        state.selected_track.is_some(),
                        egui::Button::new(tr("Lista de edição EDL da faixa selecionada…")),
                    )
                    .clicked()
                {
                    state.export_edl();
                    ui.close_menu();
                }
                if ui
                    .add_enabled(has_events, egui::Button::new("XML (Final Cut Pro 7)…"))
                    .clicked()
                {
                    state.export_fcp7();
                    ui.close_menu();
                }
                if ui
                    .add_enabled(has_events, egui::Button::new("FCPXML (Final Cut Pro)…"))
                    .clicked()
                {
                    state.export_fcpxml();
                    ui.close_menu();
                }
                if ui
                    .add_enabled(has_events, egui::Button::new("OpenTimelineIO (.otio)…"))
                    .clicked()
                {
                    state.export_otio();
                    ui.close_menu();
                }
                if ui
                    .add_enabled(
                        has_events,
                        egui::Button::new("AAF (Advanced Authoring Format)…"),
                    )
                    .clicked()
                {
                    state.export_aaf();
                    ui.close_menu();
                }
                ui.separator();
                if ui
                    .add_enabled(
                        !state.timeline.captions.is_empty(),
                        egui::Button::new(tr("Legendas (SRT, WebVTT)…")),
                    )
                    .clicked()
                {
                    state.export_captions();
                    ui.close_menu();
                }
            });
            ui.separator();

            command_item(state, ui, Command::FileProperties);
            ui.separator();
            if ui.button(tr("Sair")).clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
        ui.menu_button(tr("Editar"), |ui| {
            if ui
                .add_enabled(
                    state.history.can_undo(),
                    egui::Button::new(state.with_hint(tr("Desfazer"), Command::EditUndo)),
                )
                .clicked()
            {
                state.undo();
                ui.close_menu();
            }
            if ui
                .add_enabled(
                    state.history.can_redo(),
                    egui::Button::new(state.with_hint(tr("Refazer"), Command::EditRedo)),
                )
                .clicked()
            {
                state.redo();
                ui.close_menu();
            }
            ui.separator();
            for command in [
                Command::EditCut,
                Command::EditCopy,
                Command::EditPaste,
                Command::EditPasteInsert,
                Command::EditPasteRepeat,
                Command::EditPasteAttributes,
                Command::EditPasteAttributesSelective,
                Command::EditDelete,
            ] {
                command_item(state, ui, command);
            }
            ui.separator();
            for command in [
                Command::EditGroup,
                Command::EditUngroup,
                Command::EditRemoveFromGroup,
                Command::EditSelectGroup,
                Command::EditIgnoreGrouping,
            ] {
                command_item(state, ui, command);
            }
            ui.menu_button(tr("Interruptores"), |ui| switch_items(state, ui));
            ui.menu_button(tr("Canais de áudio"), |ui| channel_items(state, ui));
            ui.menu_button(tr("Ferramenta de edição"), |ui| {
                for tool in EditTool::ALL {
                    if ui.radio(state.edit_tool == tool, tool.label()).clicked() {
                        state.set_edit_tool(tool);
                        ui.close_menu();
                    }
                }
                ui.separator();
                command_item(state, ui, Command::EditToolNext);
                command_item(state, ui, Command::EditToolPrevious);
            });
            ui.separator();
            for command in [
                Command::EditSplit,
                Command::EditTrimToSelection,
                Command::EditTrimStart,
                Command::EditTrimEnd,
                Command::EditTrimEdgePrevious,
                Command::EditTrimEdgeNext,
                Command::EditTrimEdgeStretchLeft,
                Command::EditTrimEdgeStretchRight,
                Command::EditTrimEdgeRollLeft,
                Command::EditTrimEdgeRollRight,
            ] {
                command_item(state, ui, command);
            }
            ui.separator();
            command_item(state, ui, Command::EditSelectAll);
            command_item(state, ui, Command::EditSelectNone);
            ui.separator();
            let mut auto = state.project.settings.auto_crossfade;
            if ui.checkbox(&mut auto, tr("Crossfade automático")).changed() {
                state.project.settings.auto_crossfade = auto;
                state.forced_dirty = true;
            }
            ui.separator();
            // Snap sources and frame quantization: a check mark per switch, each toggled by its command
            for (command, on) in [
                (Command::OptionsSnapToGrid, state.snap_sources.grid),
                (Command::OptionsSnapToMarkers, state.snap_sources.markers),
                (
                    Command::OptionsSnapToAllEvents,
                    state.snap_sources.all_events,
                ),
                (Command::OptionsQuantizeToFrames, state.quantize_frames),
            ] {
                let mut checked = on;
                let label = state.with_hint(tr(command.label()), command);
                if ui.checkbox(&mut checked, label).changed() {
                    let ctx = ui.ctx().clone();
                    let now = ctx.input(|i| i.time);
                    state.run_command(command, &ctx, now);
                }
            }
            ui.separator();
            if ui.button(tr("Preferências…")).clicked() {
                state.show_preferences = true;
                ui.close_menu();
            }
            if ui.button(tr("Personalizar teclado…")).clicked() {
                state.show_keymap = true;
                ui.close_menu();
            }
        });
        ui.menu_button(tr("Faixas"), |ui| {
            command_item(state, ui, Command::InsertAudioTrack);
            command_item(state, ui, Command::InsertVideoTrack);
            command_item(state, ui, Command::InsertAdjustmentEvent);
            command_item(state, ui, Command::InsertAudioBus);
            if ui.button(tr("Inserir mídia gerada…")).clicked() {
                state.generator_dialog.open = true;
                ui.close_menu();
            }
            if ui.button(tr("Inserir apresentação de slides…")).clicked() {
                state.slideshow.open = true;
                ui.close_menu();
            }
            command_item(state, ui, Command::InsertTime);
            ui.separator();
            for command in [
                Command::TrackMute,
                Command::TrackMuteExclusive,
                Command::TrackSolo,
                Command::TrackSoloExclusive,
            ] {
                command_item(state, ui, command);
            }
            ui.separator();
            for command in [
                Command::TrackVolumeEnvelope,
                Command::TrackPanEnvelope,
                Command::TrackToggleEnvelopes,
            ] {
                command_item(state, ui, command);
            }
            let video_track = state
                .selected_track
                .and_then(|id| state.timeline.track(id))
                .is_some_and(|t| t.kind == TrackKind::Video);
            if ui
                .add_enabled(video_track, egui::Button::new(tr("Movimento da trilha…")))
                .clicked()
            {
                state.open_track_motion();
                ui.close_menu();
            }
            ui.add_enabled_ui(video_track, |ui| {
                ui.menu_button(tr("Envelopes de vídeo"), |ui| {
                    for (label, target, initial) in [
                        ("Nível da faixa", AutomationTarget::VideoLevel, 1.0),
                        ("Esmaecer para cor", AutomationTarget::VideoFade, 0.0),
                        ("Mudo da faixa", AutomationTarget::VideoMute, 0.0),
                    ] {
                        if ui.button(label).clicked() {
                            state.toggle_video_envelope(target, initial);
                            ui.close_menu();
                        }
                    }
                });
            });
        });
        ui.menu_button(tr("Ferramentas"), |ui| {
            if ui.button(tr("Inspetor")).clicked() {
                state.show_inspector = true;
                ui.close_menu();
            }
            ui.separator();
            // Video tools
            let video_event = state
                .selection
                .primary()
                .is_some_and(|(t, e)| pcrop::get(&state.timeline, t, e).is_some());
            if ui
                .add_enabled(video_event, egui::Button::new(tr("Pan/Crop do evento…")))
                .clicked()
            {
                state.open_pan_crop_for_selection();
                ui.close_menu();
            }
            let video_track = state
                .selected_track
                .is_some_and(|id| tmotion::get(&state.timeline, id).is_some());
            if ui
                .add_enabled(video_track, egui::Button::new(tr("Movimento da trilha…")))
                .clicked()
            {
                state.open_track_motion();
                ui.close_menu();
            }
            if ui.button(tr("Correção de cor…")).clicked() {
                state.show_grade = true;
                ui.close_menu();
            }
            if ui
                .button(tr("Chaveamento de croma (Chroma Key)…"))
                .clicked()
            {
                state.show_chroma = true;
                ui.close_menu();
            }
            ui.menu_button(tr("LUT de cor do vídeo"), |ui| {
                let current = state.timeline.video_lut.clone();
                if ui
                    .selectable_label(current.is_none(), tr("Nenhuma"))
                    .clicked()
                {
                    state.set_video_lut(None);
                    ui.close_menu();
                }
                for (name, path) in bundled_luts() {
                    let on = current.as_deref() == Some(path.as_str());
                    if ui.selectable_label(on, name).clicked() {
                        state.set_video_lut(Some(path));
                        ui.close_menu();
                    }
                }
                ui.separator();
                if ui.button(tr("Abrir arquivo .cube…")).clicked() {
                    if let Some(file) = rfd::FileDialog::new()
                        .add_filter("LUT de cor", &["cube"])
                        .pick_file()
                    {
                        state.set_video_lut(Some(file.display().to_string()));
                    }
                    ui.close_menu();
                }
            });
            ui.separator();
            // Audio tools
            if ui.button(tr("Metrônomo…")).clicked() {
                state.show_metronome = true;
                ui.close_menu();
            }
        });
        ui.menu_button(tr("Exibir"), |ui| {
            ui.checkbox(&mut state.show_bars, tr("Barras de cor SMPTE"));
            if ui.button(tr("Notas do projeto…")).clicked() {
                state.notes.open = true;
                ui.close_menu();
            }
            if ui.button(tr("Detalhes de edição…")).clicked() {
                state.show_details = true;
                ui.close_menu();
            }
            ui.separator();
            for command in [
                Command::ViewPreview,
                Command::ViewProjectMedia,
                Command::ViewTrimmer,
                Command::ViewAudioEffects,
                Command::ViewVideoEffects,
                Command::ViewTransitions,
                Command::ViewMasterBus,
                Command::ViewLoudnessMeters,
                Command::ViewVideoScopes,
                Command::ViewFocusTimeline,
            ] {
                command_item(state, ui, command);
            }
            ui.separator();
            ui.menu_button(tr("Layouts"), |ui| {
                state.ensure_layouts();
                if ui.button(tr("Layout padrão")).clicked() {
                    state.window_request = Some(WindowAction::DefaultLayout);
                    ui.close_menu();
                }
                ui.separator();
                let items: Vec<(PathBuf, String, Option<u8>)> = state
                    .layouts
                    .iter()
                    .map(|l| (l.path.clone(), l.file.name.clone(), l.file.slot))
                    .collect();
                for (path, name, slot) in items {
                    let label = match slot {
                        Some(n) => format!("{name}  (Alt+D, {n})"),
                        None => name,
                    };
                    let current = state.current_layout.as_ref() == Some(&path);
                    if ui.radio(current, label).clicked() {
                        state.window_request = Some(WindowAction::LoadLayout(path));
                        ui.close_menu();
                    }
                }
                if state.layouts.is_empty() {
                    ui.weak(tr("Nenhum layout salvo"));
                }
                ui.separator();
                let has_current = state.current_layout.is_some();
                if ui
                    .add_enabled(
                        has_current,
                        egui::Button::new(tr("Recarregar layout selecionado")),
                    )
                    .clicked()
                {
                    state.window_request = Some(WindowAction::ReloadLayout);
                    ui.close_menu();
                }
                if ui
                    .add_enabled(has_current, egui::Button::new(tr("Salvar layout atual")))
                    .clicked()
                {
                    state.window_request = Some(WindowAction::SaveCurrentLayout);
                    ui.close_menu();
                }
                if ui.button(tr("Salvar layout como…")).clicked() {
                    state.layout_dialog = Some(LayoutDialog {
                        name: String::new(),
                        slot: None,
                    });
                    ui.close_menu();
                }
                if ui.button(tr("Organizar layouts…")).clicked() {
                    state.organize_layouts = true;
                    ui.close_menu();
                }
            });
        });
        ui.menu_button(tr("Ajuda"), |ui| {
            if ui.button(tr("Tela de boas-vindas…")).clicked() {
                let defaults = state.prefs.new_project_settings.clone().unwrap_or_default();
                state.welcome.open_with(&defaults);
                ui.close_menu();
            }
            if ui.button(tr("Visualizador de logs…")).clicked() {
                state.log_viewer.open = true;
                ui.close_menu();
            }
            if ui.button(tr("Créditos e licenças…")).clicked() {
                state.show_credits = true;
                ui.close_menu();
            }
        });
    });
}

/// The `.cube` tables that ship with Neoveg (`assets/luts` beside the executable or in the working
/// directory): display name and path.
fn bundled_luts() -> Vec<(String, String)> {
    let mut dirs = vec![std::path::PathBuf::from("assets/luts")];
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
    {
        dirs.push(dir.join("Assets/luts"));
        dirs.push(dir.join("assets/luts"));
    }
    let mut found: Vec<(String, String)> = Vec::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("cube"))
            {
                let name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if !found.iter().any(|(n, _)| *n == name) {
                    found.push((name, path.display().to_string()));
                }
            }
        }
    }
    found.sort();
    found
}

/// Help → Credits: authors and licences of the code Neoveg is built on.
fn credits_window(ctx: &egui::Context, state: &mut AppState) {
    let mut open = state.show_credits;
    crate::dialogs::dialog(ctx, tr("Créditos e licenças"))
        .open(&mut open)
        .default_size([480.0, 420.0])
        .show(ctx, crate::credits::credits_ui);
    state.show_credits = open;
}

/// A small padlock (body and shackle) with its top-left corner at `at`, for locked events.
fn paint_lock_mark(painter: &egui::Painter, at: Pos2) {
    let ink = Color32::from_gray(235);
    painter.rect_filled(
        Rect::from_min_size(at + vec2(1.0, 5.0), vec2(8.0, 6.0)),
        Rounding::ZERO,
        ink,
    );
    painter.rect_stroke(
        Rect::from_min_size(at + vec2(2.5, 1.0), vec2(5.0, 5.0)),
        Rounding::ZERO,
        Stroke::new(1.2_f32, ink),
    );
}

/// A checked menu item for one event switch: ticked when it is on for every selected event that
/// can take it, and greyed out when the selection holds none.
fn switch_item(state: &mut AppState, ui: &mut egui::Ui, switch: Switch, command: Command) {
    let refs = state.selection.refs();
    let current = switches::state(&state.timeline, &refs, switch);
    let enabled = state.command_enabled(command);
    let mut on = current == SwitchState::On;
    let label = state.with_hint(tr(command.label()), command);
    let label = if current == SwitchState::Mixed {
        tr_args("{} (parcial)", &[&label])
    } else {
        label
    };
    if ui
        .add_enabled(enabled, egui::Checkbox::new(&mut on, label))
        .clicked()
    {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        state.run_command(command, &ctx, now);
        ui.close_menu();
    }
}

/// The main toolbar under the menu: file, clipboard and history commands, as an icon each (names and
/// shortcuts are in the tooltips).
fn main_toolbar(ui: &mut egui::Ui, state: &mut AppState) {
    use icons::SvgIcon;
    let groups: [&[(SvgIcon, Command)]; 3] = [
        &[
            (SvgIcon::NewProject, Command::FileNew),
            (SvgIcon::OpenProject, Command::FileOpen),
            (SvgIcon::SaveProject, Command::FileSave),
            (SvgIcon::ProjectProperties, Command::FileProperties),
        ],
        &[
            (SvgIcon::Cut, Command::EditCut),
            (SvgIcon::Copy, Command::EditCopy),
            (SvgIcon::Paste, Command::EditPaste),
            (SvgIcon::Delete, Command::EditDelete),
        ],
        &[
            (SvgIcon::Undo, Command::EditUndo),
            (SvgIcon::Redo, Command::EditRedo),
        ],
    ];
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (i, group) in groups.iter().enumerate() {
            if i > 0 {
                ui.separator();
            }
            for &(icon, command) in *group {
                let enabled = state.command_enabled(command);
                let tooltip = state.with_hint(command.label(), command);
                let clicked = ui
                    .add_enabled_ui(enabled, |ui| icons::svg_button(ui, icon, &tooltip, false))
                    .inner
                    .clicked();
                if clicked {
                    let ctx = ui.ctx().clone();
                    let now = ctx.input(|i| i.time);
                    state.run_command(command, &ctx, now);
                }
            }
            // Render sits with the file commands, as in the manuals' toolbar
            if i == 0 {
                let tooltip = "Renderizar como… (Make Movie)";
                if icons::svg_button(ui, SvgIcon::ExportRender, tooltip, false).clicked() {
                    state.render_dialog.open = true;
                }
            }
        }
        ui.separator();
        if icons::svg_button(ui, SvgIcon::Welcome, "Tela de boas-vindas", false).clicked() {
            let defaults = state.prefs.new_project_settings.clone().unwrap_or_default();
            state.welcome.open_with(&defaults);
        }
        if icons::svg_button(ui, SvgIcon::Preferences, "Preferências", false).clicked() {
            state.show_preferences = true;
        }
    });
}

/// Arquivo > Projetos recentes: the last projects opened or saved, newest first.
fn recent_projects_menu(state: &mut AppState, ui: &mut egui::Ui) {
    let entries = state.prefs.recent_projects.clone();
    ui.add_enabled_ui(!entries.is_empty(), |ui| {
        ui.menu_button(tr("Projetos recentes"), |ui| {
            for path in &entries {
                let name = std::path::Path::new(path)
                    .file_stem()
                    .map_or_else(|| path.clone(), |s| s.to_string_lossy().into_owned());
                if ui.button(name).on_hover_text(path).clicked() {
                    let ctx = ui.ctx().clone();
                    state.open_recent(std::path::Path::new(path), &ctx);
                    ui.close_menu();
                }
            }
        });
    });
}

/// Editar > Canais de áudio: how the selected audio events use the channels of their media.
fn channel_items(state: &mut AppState, ui: &mut egui::Ui) {
    let refs = state.selection.refs();
    let enabled = !refs.is_empty();
    for mode in neoveg_common::channels::ChannelMode::ALL {
        // A mode is ticked when every selected audio event has it
        let on = enabled
            && refs.iter().all(|&(t, id)| {
                state
                    .timeline
                    .track(t)
                    .and_then(|track| track.events.iter().find(|e| e.id == id))
                    .is_none_or(|e| e.channels == mode)
            });
        let mut ticked = on;
        if ui
            .add_enabled(enabled, egui::Checkbox::new(&mut ticked, mode.label()))
            .clicked()
        {
            state.set_event_channels(mode);
            ui.close_menu();
        }
    }
}

/// Editar > Interruptores (and the same entries in the event context menu).
fn switch_items(state: &mut AppState, ui: &mut egui::Ui) {
    switch_item(state, ui, Switch::Mute, Command::EditSwitchMute);
    switch_item(state, ui, Switch::Lock, Command::EditSwitchLock);
    switch_item(
        state,
        ui,
        Switch::InvertPhase,
        Command::EditSwitchInvertPhase,
    );
    switch_item(state, ui, Switch::Loop, Command::EditSwitchLoop);
    switch_item(state, ui, Switch::Normalize, Command::EditSwitchNormalize);
    command_item(state, ui, Command::EditNormalizeRecalculate);
    resample_items(state, ui);
}

/// The Resample Mode of the selected video events: one radio entry per mode, checked when they all
/// share it. It only appears when the selection holds a video event.
fn resample_items(state: &mut AppState, ui: &mut egui::Ui) {
    let selected = state.selection.refs();
    let has_video = selected.iter().any(|&(t, e)| {
        state
            .timeline
            .track(t)
            .filter(|track| track.kind == TrackKind::Video)
            .is_some_and(|track| track.events.iter().any(|ev| ev.id == e && !ev.adjustment))
    });
    if !has_video {
        return;
    }
    let current = neoveg_engine::switches::resample_of(&state.timeline, &selected);
    ui.menu_button(tr("Reamostragem"), |ui| {
        for mode in neoveg_engine::ResampleMode::ALL {
            if ui.radio(current == Some(mode), mode.label()).clicked() {
                if let Some(command) =
                    neoveg_engine::switches::set_resample(&state.timeline, &selected, mode)
                {
                    if state.history.execute(&mut state.timeline, command) {
                        state.last_frame_key = None;
                        state.last_composite_key = None;
                    }
                }
                ui.close_menu();
            }
        }
    });
}

/// A menu entry that runs `command`, with its shortcut and enabled state.
/// Whether a drag snaps: `Alt` always ignores snapping; `Shift` inverts it while moving events (on
/// edges `Shift` means "ignore the group" instead).
fn effective_snap(snapping: bool, mode: Option<DragMode>, shift: bool, alt: bool) -> bool {
    if alt {
        return false;
    }
    if shift && matches!(mode, Some(DragMode::Move)) {
        !snapping
    } else {
        snapping
    }
}

fn command_item(state: &mut AppState, ui: &mut egui::Ui, command: Command) {
    let enabled = state.command_enabled(command);
    let label = state.with_hint(tr(command.label()), command);
    if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        state.run_command(command, &ctx, now);
        ui.close_menu();
    }
}

/// One slider of the color correction window: label, value, range, default and the readout.
type GradeRow<'a> = (
    &'static str,
    &'a mut f32,
    std::ops::RangeInclusive<f32>,
    f32,
    fn(f32) -> String,
);

/// Ferramentas → Correção de cor: primary correction of the video. The sliders act on a draft that
/// is shown at once and committed as one undo step when the drag ends.
/// The Pan/Crop editor: shows the window for what it was opened on (an event, or a video track for
/// its motion) and runs what the user did.
fn pan_crop_editor(ctx: &egui::Context, state: &mut AppState) {
    if !state.pan_crop.open {
        return;
    }
    let Some(target) = state.pan_crop.target else {
        return;
    };
    let fps = state.project.settings.fps;
    let project_aspect = {
        let s = &state.project.settings;
        s.width as f32 / s.height.max(1) as f32
    };
    // What is being edited: its name, the stored value, the time of the cursor in its own clock
    // and the length of that clock
    let (title, committed, time, duration, source_aspect, event_start) = match target {
        EditTarget::Event(track_id, event_id) => {
            let event = state
                .timeline
                .track(track_id)
                .filter(|t| t.kind == TrackKind::Video)
                .and_then(|t| t.events.iter().find(|e| e.id == event_id))
                .cloned();
            let Some(event) = event else {
                // The event was deleted (or undone away) while the window was open
                state.pan_crop.close();
                state.pan_crop_draft = None;
                return;
            };
            let info = event
                .media_id
                .and_then(|id| state.video_info.get(&id))
                .cloned();
            let time = (state.playhead - event.start).clamp(0.0, event.duration);
            // The source picture under the cursor, asked for once per frame number
            if let (Some(item), Some(worker)) = (
                event
                    .media_id
                    .and_then(|id| state.project.media_pool.iter().find(|m| m.id == id)),
                &state.frame_worker,
            ) {
                let source_fps = info.as_ref().map_or(fps, |i| i.fps).max(1.0);
                let source_duration = info.as_ref().map_or(event.duration, |i| i.duration);
                let seconds =
                    composite::source_seconds(&event, event.start + time, source_duration);
                let frame = (seconds * source_fps).round() as i64;
                let key = (PathBuf::from(&item.path), frame);
                if state.pan_crop.source_key.as_ref() != Some(&key) {
                    worker.request_source(SourceRequest {
                        path: key.0.clone(),
                        seconds: frame as f64 / source_fps,
                        max_width: 960,
                    });
                    state.pan_crop.source_key = Some(key);
                }
            }
            let aspect = info
                .as_ref()
                .map_or(16.0 / 9.0, |i| i.width as f32 / i.height.max(1) as f32);
            (
                event.name.clone(),
                event.pan_crop.clone(),
                time,
                event.duration,
                aspect,
                event.start,
            )
        }
        EditTarget::Track(track_id) => {
            let Some(track) = state
                .timeline
                .track(track_id)
                .filter(|t| t.kind == TrackKind::Video)
            else {
                state.pan_crop.close();
                state.pan_crop_draft = None;
                return;
            };
            // A track has no single source: its motion is in timeline time and the frame is the project's
            let length = state.timeline.duration().max(state.playhead + 1.0).max(1.0);
            (
                track.name.clone(),
                track.motion.clone(),
                state.playhead.clamp(0.0, length),
                length,
                project_aspect,
                0.0,
            )
        }
    };

    let view = PanCropView {
        title: &title,
        pan_crop: &committed,
        time,
        duration,
        fps,
        source_aspect,
        project_aspect,
        preview: state.video_texture.as_ref(),
    };
    let actions = state.pan_crop.show(ctx, &view);
    for action in actions {
        match action {
            PanCropAction::Draft(pan_crop) => {
                state.pan_crop_draft = Some(PanCropDraft { target, pan_crop });
            }
            PanCropAction::EndDraft => state.pan_crop_draft = None,
            PanCropAction::Commit(pan_crop) => {
                let cmd = match target {
                    EditTarget::Event(track_id, event_id) => {
                        pcrop::set(&state.timeline, track_id, event_id, pan_crop)
                    }
                    EditTarget::Track(track_id) => {
                        tmotion::set(&state.timeline, track_id, pan_crop)
                    }
                };
                if let Some(cmd) = cmd {
                    if state.history.execute(&mut state.timeline, cmd) {
                        state.last_frame_key = None;
                        state.last_composite_key = None;
                    }
                }
            }
            PanCropAction::SeekTo(seconds) => {
                let now = ctx.input(|i| i.time);
                state.go_to_time(event_start + seconds, now);
            }
            PanCropAction::Close => {
                state.pan_crop.close();
                state.pan_crop_draft = None;
            }
        }
    }
}

fn grade_window(ctx: &egui::Context, state: &mut AppState) {
    if !state.show_grade {
        return;
    }
    let mut open = true;
    let mut grade = state.effective_grade();
    let mut finished = false;
    let mut changed = false;
    let mut reset = false;
    crate::dialogs::tool_window(ctx, tr("Correção de cor"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            let rows: [GradeRow; 7] = [
                (
                    tr("Exposição"),
                    &mut grade.exposure,
                    -4.0..=4.0,
                    0.0,
                    |v| format!("{v:+.2} EV"),
                ),
                (
                    tr("Temperatura"),
                    &mut grade.temperature,
                    -1.0..=1.0,
                    0.0,
                    |v| format!("{:+.0}", v * 100.0),
                ),
                (tr("Matiz (tint)"), &mut grade.tint, -1.0..=1.0, 0.0, |v| {
                    format!("{:+.0}", v * 100.0)
                }),
                (tr("Pretos (lift)"), &mut grade.lift, -0.5..=0.5, 0.0, |v| {
                    format!("{:+.0}", v * 200.0)
                }),
                (tr("Contraste"), &mut grade.contrast, -1.0..=1.0, 0.0, |v| {
                    format!("{:+.0}", v * 100.0)
                }),
                (tr("Gama"), &mut grade.gamma, 0.2..=3.0, 1.0, |v| {
                    format!("{v:.2}")
                }),
                (
                    tr("Saturação"),
                    &mut grade.saturation,
                    -1.0..=1.0,
                    0.0,
                    |v| format!("{:+.0}", v * 100.0),
                ),
            ];
            for (label, value, range, default, format) in rows {
                ui.horizontal(|ui| {
                    ui.allocate_ui(egui::vec2(96.0, 18.0), |ui| ui.label(label));
                    let response = ui.add(PrecisionSlider::new(value, range, default).width(150.0));
                    changed |= response.changed();
                    finished |= response.drag_stopped()
                        || response.lost_focus()
                        || (response.changed() && !response.dragged() && !response.has_focus());
                    ui.monospace(format(*value));
                });
            }
            ui.separator();
            if ui.button(tr("Restaurar tudo")).clicked() {
                reset = true;
            }
        });
    if reset {
        state.commit_grade(Grade::default());
    } else if finished {
        state.commit_grade(grade);
    } else if changed {
        state.grade_draft = Some(grade.sanitized());
    }
    state.show_grade = open;
    if !open {
        // Closing in the middle of a drag keeps what was shown
        if let Some(draft) = state.grade_draft {
            state.commit_grade(draft);
        }
    }
}

/// Ferramentas → Chaveamento de croma (Chroma Key): green/blue screen matte + spill suppression.
fn chroma_key_window(ctx: &egui::Context, state: &mut AppState) {
    if !state.show_chroma {
        return;
    }
    let mut open = true;
    let mut key = state.effective_chroma();
    let mut finished = false;
    let mut changed = false;
    let mut reset = false;
    crate::dialogs::tool_window(ctx, tr("Chaveamento de croma (Chroma Key)"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            if ui
                .checkbox(&mut key.enabled, tr("Ativar chaveamento de croma"))
                .changed()
            {
                changed = true;
                finished = true;
            }
            ui.horizontal(|ui| {
                ui.label(tr("Cor da tela:"));
                if ui.color_edit_button_rgb(&mut key.key_color).changed() {
                    changed = true;
                    finished = true;
                }
                if ui.button(tr("Tela verde")).clicked() {
                    key.key_color = ChromaKey::GREEN_SCREEN;
                    key.enabled = true;
                    changed = true;
                    finished = true;
                }
                if ui.button(tr("Tela azul")).clicked() {
                    key.key_color = ChromaKey::BLUE_SCREEN;
                    key.enabled = true;
                    changed = true;
                    finished = true;
                }
            });
            ui.separator();
            let def = ChromaKey::default();
            let rows: [GradeRow; 3] = [
                (
                    tr("Similaridade"),
                    &mut key.similarity,
                    0.01..=0.75,
                    def.similarity,
                    |v| format!("{:.0} %", v * 100.0),
                ),
                (
                    "Suavidade",
                    &mut key.smoothness,
                    0.0..=0.50,
                    def.smoothness,
                    |v| format!("{:.0} %", v * 100.0),
                ),
                (
                    tr("Supressão de spill"),
                    &mut key.spill,
                    0.0..=1.0,
                    def.spill,
                    |v| format!("{:.0} %", v * 100.0),
                ),
            ];
            for (label, value, range, default, format) in rows {
                ui.horizontal(|ui| {
                    ui.allocate_ui(egui::vec2(118.0, 18.0), |ui| ui.label(label));
                    let response = ui.add(PrecisionSlider::new(value, range, default).width(150.0));
                    changed |= response.changed();
                    finished |= response.drag_stopped()
                        || response.lost_focus()
                        || (response.changed() && !response.dragged() && !response.has_focus());
                    ui.monospace(format(*value));
                });
            }
            if ui
                .checkbox(&mut key.show_matte, tr("Visualizar máscara (matte P&B)"))
                .changed()
            {
                changed = true;
                finished = true;
            }
            ui.separator();
            if ui.button(tr("Restaurar padrão")).clicked() {
                reset = true;
            }
        });
    if reset {
        state.commit_chroma(ChromaKey::default());
    } else if finished {
        state.commit_chroma(key);
    } else if changed {
        state.chroma_draft = Some(key.sanitized());
    }
    state.show_chroma = open;
    if !open {
        if let Some(draft) = state.chroma_draft {
            state.commit_chroma(draft);
        }
    }
}

/// Ferramentas → Metrônomo: on / off, tempo, beats per bar and loudness. The tempo belongs to the
/// project; changing anything while playing restarts the audio at the cursor.
fn metronome_window(ctx: &egui::Context, state: &mut AppState) {
    if !state.show_metronome {
        return;
    }
    let mut open = true;
    let before = (
        state.metronome_on,
        state.metronome_volume,
        state.project.settings.bpm,
        state.project.settings.beats_per_bar,
    );
    crate::dialogs::tool_window(ctx, tr("Metrônomo"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.checkbox(
                &mut state.metronome_on,
                tr("Tocar o clique durante a reprodução"),
            );
            ui.horizontal(|ui| {
                ui.label(tr("Andamento (bpm)"));
                let (lo, hi) = Metronome::BPM_RANGE;
                ui.add(
                    egui::DragValue::new(&mut state.project.settings.bpm)
                        .range(lo..=hi)
                        .speed(0.5)
                        .fixed_decimals(1),
                );
            });
            ui.horizontal(|ui| {
                ui.label(tr("Tempos por compasso"));
                ui.add(
                    egui::DragValue::new(&mut state.project.settings.beats_per_bar).range(1..=16),
                );
            });
            ui.horizontal(|ui| {
                ui.label(tr("Volume"));
                ui.add(
                    PrecisionSlider::new(&mut state.metronome_volume, 0.0..=1.0, 0.5).width(120.0),
                );
            });
        });
    state.show_metronome = open;
    let after = (
        state.metronome_on,
        state.metronome_volume,
        state.project.settings.bpm,
        state.project.settings.beats_per_bar,
    );
    if before != after {
        state.restart_audio_if_playing();
    }
}

/// Ctrl+G: type a time (timecode, minutes:seconds, seconds or frames) and the cursor goes there.
/// The next numbered version of a project file: `projeto.neoveg` becomes `projeto_0001.neoveg`, the
/// first number that is not taken; a name that already ends in `_NNNN` counts on from that number.
fn incremental_path(path: &std::path::Path, exists: &dyn Fn(&std::path::Path) -> bool) -> PathBuf {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "projeto".into());
    let (base, start) = match stem.rsplit_once('_') {
        Some((base, digits))
            if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) =>
        {
            (base.to_owned(), digits.parse::<u32>().unwrap_or(0) + 1)
        }
        _ => (stem.clone(), 1),
    };
    let extension = path
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_else(|| project_file::EXTENSION.into());
    (start..)
        .map(|n| path.with_file_name(format!("{base}_{n:04}.{extension}")))
        .find(|candidate| !exists(candidate))
        .expect("a free number exists")
}

/// Insert > Time…: asks how much empty time to open at the cursor.
fn insert_time_window(ctx: &egui::Context, state: &mut AppState) {
    let Some(mut text) = state.insert_time_dialog.take() else {
        return;
    };
    let fps = state.project.settings.fps;
    let (mut insert, mut cancel) = (false, false);
    egui::Window::new(tr("Inserir tempo"))
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            let edit = ui.add(egui::TextEdit::singleline(&mut text).desired_width(160.0));
            if !edit.has_focus() && !edit.lost_focus() {
                edit.request_focus();
            }
            let valid = navigation::parse_time(&text, fps).is_some_and(|t| t > 0.0);
            if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && valid {
                insert = true;
            }
            ui.weak(tr(
                "Duração a abrir em todas as faixas: 00:00:05:00, 5, 2,5 ou 120f",
            ));
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(valid, egui::Button::new(tr("Inserir")))
                    .clicked()
                {
                    insert = true;
                }
                if ui.button(tr("Cancelar")).clicked() {
                    cancel = true;
                }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                cancel = true;
            }
        });
    if insert {
        if let Some(length) = navigation::parse_time(&text, fps) {
            state.insert_time_at_cursor(length);
        }
    } else if !cancel {
        state.insert_time_dialog = Some(text);
    }
}

fn goto_window(ctx: &egui::Context, state: &mut AppState) {
    let Some(mut text) = state.goto_dialog.take() else {
        return;
    };
    let fps = state.project.settings.fps;
    let (mut go, mut cancel) = (false, false);
    egui::Window::new(tr("Ir para o tempo"))
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            let edit = ui.add(egui::TextEdit::singleline(&mut text).desired_width(160.0));
            if !edit.has_focus() && !edit.lost_focus() {
                edit.request_focus();
            }
            let valid = navigation::parse_time(&text, fps).is_some();
            if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && valid {
                go = true;
            }
            ui.weak("00:01:02:10, 1:02,5, 62,5 ou 1500f");
            ui.horizontal(|ui| {
                if ui.add_enabled(valid, egui::Button::new("Ir")).clicked() {
                    go = true;
                }
                if ui.button(tr("Cancelar")).clicked() {
                    cancel = true;
                }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                cancel = true;
            }
        });
    if go {
        if let Some(time) = navigation::parse_time(&text, fps) {
            let now = ctx.input(|i| i.time);
            state.go_to_time(time, now);
        }
    } else if !cancel {
        state.goto_dialog = Some(text);
    }
}

/// "Repetir colar…": number of copies and spacing.
fn repeat_paste_window(ctx: &egui::Context, state: &mut AppState) {
    let Some(mut dialog) = state.repeat_dialog.take() else {
        return;
    };
    let mut keep_open = true;
    let mut confirmed = false;
    crate::dialogs::dialog(ctx, tr("Repetir colar"))
        .collapsible(false)
        .resizable(false)
        .open(&mut keep_open)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(tr("Número de cópias"));
                ui.add(egui::DragValue::new(&mut dialog.count).range(1..=999));
            });
            ui.radio_value(&mut dialog.spaced, false, tr("Uma após a outra"));
            ui.horizontal(|ui| {
                ui.radio_value(&mut dialog.spaced, true, tr("A cada"));
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
                if ui.button(tr("Colar")).clicked() {
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

/// The last part of a path, whichever separator it uses.
fn file_name_of(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_owned()
}

/// What a candidate for a missing media is like: its duration, its sound channels and its frame
/// rate, read the way an import reads it (slow for long files, so only on a worker thread). `None`
/// when the file cannot be read.
fn probe_candidate(path: &std::path::Path) -> Option<relink::Probed> {
    if is_visual_media_path(path) {
        let info = probe(path).ok()?;
        let video = is_video_path(path);
        return Some(relink::Probed {
            // A still has no length of its own, so there is nothing to compare
            duration_seconds: if video { info.duration } else { f64::NAN },
            channels: None,
            fps: video.then_some(info.fps),
        });
    }
    // The header answers without decoding; a file whose header is silent is decoded as before
    let (duration_seconds, channels) = match probe_audio(path) {
        Some(found) => found,
        None => {
            let audio = neoveg_audio::decode::decode_file(path).ok()?;
            (audio.duration_seconds(), audio.channels)
        }
    };
    Some(relink::Probed {
        duration_seconds,
        channels: Some(channels),
        fps: None,
    })
}

/// "Mídia ausente": the media the project points to that is not on disk, what was looked at for
/// each, and the ways to find it: *Localizar…* for one file, *Buscar em uma pasta…* for all.
fn missing_media_window(ctx: &egui::Context, state: &mut AppState) {
    if !state.missing_open {
        return;
    }
    if state.missing.is_empty() {
        state.missing_open = false;
        return;
    }
    let mut keep_open = true;
    let mut locate = None;
    let mut search = false;
    let mut ignore = None;
    let mut ignore_all = false;
    let mut everywhere = false;
    let mut stop = false;
    let searching = state.missing.iter().any(|m| m.searching);
    crate::dialogs::dialog(ctx, tr("Mídia ausente"))
        .collapsible(false)
        .open(&mut keep_open)
        .show(ctx, |ui| {
            ui.label(tr("O projeto aponta para arquivos que não estão mais no lugar. Os eventos continuam na linha do tempo."));
            ui.separator();
            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                for m in &state.missing {
                    ui.horizontal(|ui| {
                        ui.strong(&m.name);
                        if m.searching {
                            ui.weak(tr("procurando…"));
                        } else {
                            if ui.button(tr("Localizar…")).clicked() {
                                locate = Some(m.id);
                            }
                            if m.ignored {
                                ui.weak(tr("deixada offline"));
                            } else if ui.button(tr("Ignorar e deixar offline")).clicked() {
                                ignore = Some(m.id);
                            }
                        }
                    });
                    ui.weak(&m.old_path);
                    for r in &m.rejected {
                        ui.weak(tr_args("  recusado: {} ({})", &[&file_name_of(&r.path.to_string_lossy()), &r.reason]));
                    }
                    ui.separator();
                }
            });
            if let Some(deep) = &state.deep_search {
                if deep.done.load(Ordering::Relaxed) {
                    ui.weak(tr_args("Busca em todos os lugares terminada: {} pastas.", &[&deep.folders.load(Ordering::Relaxed)]));
                } else {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(tr_args("Procurando em todos os lugares: {} pastas", &[&deep.folders.load(Ordering::Relaxed)]));
                        if ui.button(tr("Parar")).clicked() {
                            stop = true;
                        }
                    });
                    if let Ok(current) = deep.current.lock() {
                        ui.weak(current.as_str());
                    }
                }
            }
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!searching, egui::Button::new(tr("Buscar em uma pasta…")))
                    .clicked()
                {
                    search = true;
                }
                if ui
                    .add_enabled(!searching, egui::Button::new(tr("Buscar em todos os lugares")))
                    .clicked()
                {
                    everywhere = true;
                }
                if ui
                    .add_enabled(
                        state.missing.iter().any(|m| !m.ignored),
                        egui::Button::new(tr("Ignorar todas")),
                    )
                    .clicked()
                {
                    ignore_all = true;
                }
            });
        });
    if let Some(id) = locate {
        state.locate_missing(id, ctx);
    }
    if search {
        state.search_missing_in_folder(ctx);
    }
    if everywhere {
        state.search_missing_everywhere(ctx);
    }
    if stop {
        if let Some(deep) = &state.deep_search {
            deep.cancel.store(true, Ordering::Relaxed);
        }
    }
    if let Some(id) = ignore {
        if let Some(m) = state.missing.iter_mut().find(|m| m.id == id) {
            m.ignored = true;
        }
    }
    if ignore_all {
        for m in &mut state.missing {
            m.ignored = true;
        }
        state.missing_open = false;
        state.status =
            "Mídia ausente deixada offline; Arquivo > Mídia ausente… abre a lista".into();
    } else {
        state.missing_open = keep_open
            && !state.missing.is_empty()
            && (state.missing.iter().any(|m| !m.ignored) || ignore.is_none());
    }
}

/// "Mídia encontrada": media that was found outside the project's folder, with the choice of copying it
/// next to the project. The original stays where it is; the project then points at the copy.
fn found_media_window(ctx: &egui::Context, state: &mut AppState) {
    if !state.found_open {
        return;
    }
    if state.found.is_empty() {
        state.found_open = false;
        return;
    }
    let dir = state.project_dir();
    let mut keep_open = true;
    let mut copy: Option<Vec<u64>> = None;
    crate::dialogs::dialog(ctx, tr("Mídia encontrada"))
        .collapsible(false)
        .open(&mut keep_open)
        .show(ctx, |ui| {
            ui.label(tr(
                "Estes arquivos foram encontrados fora da pasta do projeto.",
            ));
            match &dir {
                Some(dir) => {
                    ui.weak(tr_args(
                        "Copiar para {} deixa o projeto junto da mídia. O original não é apagado.",
                        &[&dir.display()],
                    ));
                }
                None => {
                    ui.weak(tr("Salve o projeto para escolher a pasta da cópia."));
                }
            }
            ui.separator();
            egui::ScrollArea::vertical()
                .max_height(260.0)
                .show(ui, |ui| {
                    for f in &state.found {
                        ui.horizontal(|ui| {
                            ui.strong(&f.name);
                            if f.copying {
                                ui.spinner();
                                ui.weak(tr("copiando…"));
                            } else if ui
                                .add_enabled(
                                    dir.is_some(),
                                    egui::Button::new(tr("Copiar para a pasta do projeto")),
                                )
                                .clicked()
                            {
                                copy = Some(vec![f.id]);
                            }
                        });
                        ui.weak(f.path.display().to_string());
                    }
                });
            ui.separator();
            ui.horizontal(|ui| {
                let idle = state.found.iter().any(|f| !f.copying);
                if ui
                    .add_enabled(
                        dir.is_some() && idle,
                        egui::Button::new(tr("Copiar todas para a pasta do projeto")),
                    )
                    .clicked()
                {
                    copy = Some(state.found.iter().map(|f| f.id).collect());
                }
            });
        });
    if let Some(ids) = copy {
        state.start_copy(ids, ctx);
    }
    state.found_open = keep_open && !state.found.is_empty();
}

/// "Erro de arquivo de mídia": a file of the project exists but could not be read. One question at a
/// time: pick another file for it, leave it offline, or leave offline this and every later error.
fn media_error_window(ctx: &egui::Context, state: &mut AppState) {
    let Some(error) = state.media_errors.front() else {
        return;
    };
    let (id, path, text) = (error.id, error.path.clone(), error.error.clone());
    let mut replace = false;
    let mut ignore = false;
    let mut ignore_all = false;
    crate::dialogs::dialog(ctx, tr("Erro de arquivo de mídia"))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(tr_args("Não foi possível ler {}.", &[&file_name_of(&path)]));
            ui.weak(&path);
            ui.weak(&text);
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(tr("Escolher outro arquivo…")).clicked() {
                    replace = true;
                }
                if ui.button(tr("Ignorar")).clicked() {
                    ignore = true;
                }
                if ui.button(tr("Ignorar todos os erros seguintes")).clicked() {
                    ignore_all = true;
                }
            });
        });
    if replace {
        if state.replace_media_file(id, ctx) {
            state.media_errors.pop_front();
        }
    } else if ignore {
        state.media_errors.pop_front();
        state.leave_offline(id, std::path::Path::new(&path), &text);
    } else if ignore_all {
        state.ignore_media_errors = true;
        while let Some(e) = state.media_errors.pop_front() {
            state.leave_offline(e.id, std::path::Path::new(&e.path), &e.error);
        }
    }
}

/// "Converter mídia": the question after an import that failed because the file is not compatible.
fn convert_offer_window(ctx: &egui::Context, state: &mut AppState) {
    let Some(offer) = state.convert_offer.take() else {
        return;
    };
    let target = neoveg_video::convert::target_for(&offer.path);
    let mut keep_open = true;
    let mut convert = false;
    let mut cancel = false;
    crate::dialogs::dialog(ctx, tr("Mídia não compatível"))
        .collapsible(false)
        .resizable(false)
        .open(&mut keep_open)
        .show(ctx, |ui| {
            ui.label(tr_args(
                "O Neoveg não conseguiu abrir {}.",
                &[&offer.path.display()],
            ));
            ui.weak(&offer.error);
            ui.separator();
            ui.label(tr_args(
                "Converter uma cópia para {} e importá-la? O arquivo original não é alterado.",
                &[&target.label()],
            ));
            ui.horizontal(|ui| {
                if ui.button(tr("Converter")).clicked() {
                    convert = true;
                }
                if ui.button(tr("Cancelar")).clicked() {
                    cancel = true;
                }
            });
        });
    if convert {
        state.start_conversion(offer.path, ctx);
    } else if keep_open && !cancel {
        state.convert_offer = Some(offer);
    }
}

/// The two layout windows: *Salvar layout como* (a name and a shortcut number) and *Organizar layouts*
/// (the saved ones, with their shortcut numbers, to load or delete).
fn layout_dialogs(ctx: &egui::Context, state: &mut AppState) {
    if let Some(mut dialog) = state.layout_dialog.take() {
        let mut keep_open = true;
        let mut save = false;
        let mut cancel = false;
        crate::dialogs::dialog(ctx, tr("Salvar layout como"))
            .collapsible(false)
            .resizable(false)
            .open(&mut keep_open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(tr("Nome"));
                    ui.text_edit_singleline(&mut dialog.name);
                });
                ui.horizontal(|ui| {
                    ui.label(tr("Atalho"));
                    slot_combo(ui, "layout_save_slot", &mut dialog.slot);
                });
                if let Some(dir) = &state.layouts_dir {
                    ui.weak(tr_args("Pasta: {}", &[&dir.display()]));
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            !dialog.name.trim().is_empty(),
                            egui::Button::new(tr("Salvar")),
                        )
                        .clicked()
                    {
                        save = true;
                    }
                    if ui.button(tr("Cancelar")).clicked() {
                        cancel = true;
                    }
                });
            });
        if save {
            state.window_request = Some(WindowAction::SaveLayout {
                name: dialog.name.trim().to_owned(),
                slot: dialog.slot,
            });
        } else if keep_open && !cancel {
            state.layout_dialog = Some(dialog);
        }
    }
    if state.organize_layouts {
        state.ensure_layouts();
        let mut keep_open = true;
        let mut request = None;
        let rows: Vec<(PathBuf, String, Option<u8>)> = state
            .layouts
            .iter()
            .map(|l| (l.path.clone(), l.file.name.clone(), l.file.slot))
            .collect();
        crate::dialogs::dialog(ctx, tr("Organizar layouts"))
            .collapsible(false)
            .open(&mut keep_open)
            .show(ctx, |ui| {
                if let Some(dir) = &state.layouts_dir {
                    ui.weak(tr_args("Pasta: {}", &[&dir.display()]));
                }
                if rows.is_empty() {
                    ui.label(tr(
                        "Nenhum layout salvo ainda. Use Exibir > Layouts > Salvar layout como.",
                    ));
                }
                for (i, (path, name, slot)) in rows.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(name);
                        let mut chosen = *slot;
                        slot_combo(ui, &format!("layout_slot_{i}"), &mut chosen);
                        if chosen != *slot {
                            request = Some(WindowAction::SetLayoutSlot(path.clone(), chosen));
                        }
                        if ui.button(tr("Carregar")).clicked() {
                            request = Some(WindowAction::LoadLayout(path.clone()));
                        }
                        if ui.button(tr("Excluir")).clicked() {
                            request = Some(WindowAction::DeleteLayout(path.clone()));
                        }
                    });
                }
            });
        if request.is_some() {
            state.window_request = request;
        }
        state.organize_layouts = keep_open;
    }
}

/// A drop-down with *Nenhum* and the numbers 1 to 9 for the layout shortcut.
fn slot_combo(ui: &mut egui::Ui, id: &str, slot: &mut Option<u8>) {
    let text = slot.map_or_else(|| "Nenhum".to_owned(), |n| n.to_string());
    egui::ComboBox::from_id_salt(id)
        .selected_text(text)
        .show_ui(ui, |ui| {
            ui.selectable_value(slot, None, tr("Nenhum"));
            for n in crate::layouts::SLOTS {
                ui.selectable_value(slot, Some(n), n.to_string());
            }
        });
}

/// "Colar atributos do evento seletivamente": one check box per attribute.
fn paste_attributes_window(ctx: &egui::Context, state: &mut AppState) {
    let Some(mut set) = state.attributes_dialog.take() else {
        return;
    };
    let mut keep_open = true;
    let mut confirmed = false;
    crate::dialogs::dialog(ctx, tr("Colar atributos do evento"))
        .collapsible(false)
        .resizable(false)
        .open(&mut keep_open)
        .show(ctx, |ui| {
            ui.checkbox(&mut set.switches, tr("Interruptores do evento"));
            ui.checkbox(&mut set.pitch, tr("Tom do evento de áudio"));
            ui.checkbox(&mut set.rate, tr("Velocidade de reprodução do vídeo"));
            ui.checkbox(&mut set.effects, tr("Efeitos de vídeo do evento"));
            ui.checkbox(&mut set.pan_crop, tr("Pan/Crop do evento"));
            ui.checkbox(&mut set.velocity, tr("Envelope de velocidade"));
            ui.checkbox(&mut set.transitions, tr("Transições de entrada e saída"));
            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!set.is_empty(), egui::Button::new(tr("Colar")))
                    .clicked()
                {
                    confirmed = true;
                }
                if ui.button(tr("Marcar tudo")).clicked() {
                    set = Attributes::ALL;
                }
                if ui.button(tr("Limpar")).clicked() {
                    set = Attributes::NONE;
                }
            });
        });
    if confirmed {
        state.paste_attributes(set);
    } else if keep_open {
        state.attributes_dialog = Some(set);
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
                    Some(other) => tr_args(
                        "{} saiu de «{}» e foi para «{}»",
                        &[&shortcut, &other.label(), &target.label()],
                    ),
                    None => tr_args("{} atribuído a «{}»", &[&shortcut, &target.label()]),
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
    crate::dialogs::dialog(ctx, tr("Personalizar teclado"))
        .open(&mut open)
        .default_width(520.0)
        .default_height(480.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(tr("Filtro:"));
                ui.text_edit_singleline(&mut state.keymap_filter);
                if ui.button(tr("Restaurar tudo")).clicked() {
                    state.keymap = KeyMap::defaults();
                    state.persist_keymap();
                    state.keymap_message = "Atalhos padrão restaurados".into();
                }
            });
            if let Some(target) = state.capture_for {
                ui.colored_label(
                    Color32::from_rgb(0xF1, 0xC4, 0x0F),
                    tr_args(
                        "Pressione a combinação para «{}» (Esc cancela)",
                        &[&target.label()],
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
                                tr_args("{} (em breve)", &[&command.label()])
                            };
                            ui.add_sized([230.0, 20.0], egui::Label::new(label).truncate());
                            for shortcut in state.keymap.shortcuts_for(command) {
                                if ui
                                    .small_button(format!("{shortcut} ×"))
                                    .on_hover_text(tr("Remover este atalho"))
                                    .clicked()
                                {
                                    remove = Some(shortcut);
                                }
                            }
                            if ui
                                .small_button("+")
                                .on_hover_text(tr("Atribuir atalho"))
                                .clicked()
                            {
                                state.capture_for = Some(command);
                                state.keymap_message.clear();
                            }
                            if overridden.contains_key(command.id())
                                && icons::icon_button_sized(
                                    ui,
                                    Icon::Reset,
                                    "Restaurar o padrão deste comando",
                                    false,
                                    20.0,
                                )
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrefsTab {
    General,
    Themes,
}

/// Repaints the interface when the chosen theme (or the file it points to) changed.
fn sync_theme(ctx: &egui::Context, state: &mut AppState) {
    icons::set_color_icons(state.prefs.color_icons && state.prefs.theme != "high-contrast");
    let key = (state.prefs.theme.clone(), state.prefs.theme_file.clone());
    if state.theme_applied.as_ref() == Some(&key) {
        return;
    }
    let (resolved, warning) = state.prefs.resolve_theme();
    if let Some(warning) = warning {
        tracing::warn!("[WARN] theme: {warning}");
        state.theme_note = warning;
    }
    ui_theme::apply(ctx, &resolved);
    state.theme_applied = Some(key);
}

fn save_prefs(prefs: &Preferences) {
    if let Some(path) = Preferences::default_path() {
        if let Err(e) = prefs.save(&path) {
            tracing::warn!("[WARN] cannot save preferences: {e}");
        }
    }
}

fn preferences_window(ctx: &egui::Context, state: &mut AppState) {
    let mut open = state.show_preferences;
    let mut prefs = state.prefs.clone();
    let mut tab = state.prefs_tab;
    let mut reload = false;
    let mut note = std::mem::take(&mut state.theme_note);
    crate::dialogs::dialog(ctx, tr("Preferências"))
        .open(&mut open)
        .collapsible(false)
        .default_width(420.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut tab, PrefsTab::General, tr("Geral"));
                ui.selectable_value(&mut tab, PrefsTab::Themes, tr("Temas"));
            });
            ui.separator();
            match tab {
                PrefsTab::General => {
                    egui::ComboBox::from_label(tr("Idioma"))
                        .selected_text(prefs.language.clone())
                        .show_ui(ui, |ui| {
                            for lang in LANGUAGES {
                                ui.selectable_value(&mut prefs.language, lang.to_string(), lang);
                            }
                        });
                    ui.horizontal(|ui| {
                        ui.label(tr("Autosave (minutos, 0 = desligado)"));
                        ui.add(egui::DragValue::new(&mut prefs.autosave_minutes).range(0..=120));
                    });
                    ui.checkbox(
                        &mut prefs.auto_fade_on_import,
                        tr("Fade-in/out automático ao importar mídia"),
                    );
                    ui.checkbox(
                        &mut prefs.quantize_audio_edits,
                        tr("Quantizar para quadros também as edições só de áudio"),
                    );
                    ui.checkbox(
                        &mut prefs.color_icons,
                        tr("Ícones coloridos (BMP) onde existirem; o tema High Contrast usa sempre os monocromáticos"),
                    );
                    ui.checkbox(
                        &mut prefs.show_welcome_on_startup,
                        tr("Mostrar a tela de boas-vindas ao iniciar o programa"),
                    );
                    ui.horizontal(|ui| {
                        ui.label(tr("Projetos recentes a lembrar (0 = desligado)"));
                        ui.add(egui::DragValue::new(&mut prefs.recent_projects_max).range(0..=30));
                    });
                    prefs.trim_recent_projects();
                }
                PrefsTab::Themes => {
                    for id in theme::BUILTIN_IDS {
                        ui.radio_value(&mut prefs.theme, id.to_string(), theme::builtin_name(id));
                    }
                    ui.radio_value(
                        &mut prefs.theme,
                        theme::CUSTOM_THEME_ID.to_string(),
                        tr("Personalizado (arquivo .json)"),
                    );
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button(tr("Abrir tema…")).clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("Tema do Neoveg", &["json"])
                                .pick_file()
                            {
                                match Theme::load(&path) {
                                    Ok(theme) => {
                                        note = tr_args("Tema \"{}\" carregado", &[&theme.name]);
                                        prefs.theme = theme::CUSTOM_THEME_ID.into();
                                        prefs.theme_file = path.display().to_string();
                                    }
                                    Err(e) => {
                                        tracing::warn!("[WARN] theme: {e}");
                                        note = e;
                                    }
                                }
                            }
                        }
                        if ui.button(tr("Exportar tema atual…")).clicked() {
                            let (current, _) = prefs.resolve_theme();
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("Tema do Neoveg", &["json"])
                                .set_file_name("meu_tema.json")
                                .save_file()
                            {
                                note = match current.save(&path) {
                                    Ok(()) => tr_args("Tema salvo em {}", &[&path.display()]),
                                    Err(e) => tr_args("Falha ao salvar o tema: {}", &[&e]),
                                };
                            }
                        }
                        if ui
                            .add_enabled(
                                prefs.theme == theme::CUSTOM_THEME_ID,
                                egui::Button::new(tr("Recarregar")),
                            )
                            .on_hover_text(tr("Lê o arquivo de tema de novo"))
                            .clicked()
                        {
                            reload = true;
                        }
                    });
                    if prefs.theme == theme::CUSTOM_THEME_ID {
                        ui.small(tr_args("Arquivo: {}", &[&prefs.theme_file]));
                    }
                    if !note.is_empty() {
                        ui.label(note.as_str());
                    }
                }
            }
        });
    if prefs != state.prefs {
        save_prefs(&prefs);
        state.prefs = prefs;
    }
    if reload {
        state.theme_applied = None;
    }
    state.prefs_tab = tab;
    state.theme_note = note;
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
    egui::Window::new(tr("Recuperar projetos"))
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(tr(
                "O Neoveg não foi encerrado normalmente. Projetos com alterações não salvas:",
            ));
            ui.separator();
            for entry in &state.pending_recovery {
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "{} ({})",
                        entry.meta.project_name,
                        format_age(entry.meta.saved_unix, now)
                    ));
                    if ui.button(tr("Recuperar")).clicked() {
                        recover = Some(entry.clone());
                    }
                    if ui.button(tr("Descartar")).clicked() {
                        discard = Some(entry.path.clone());
                    }
                });
            }
            ui.separator();
            if ui.button(tr("Descartar todos")).clicked() {
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
    egui::Window::new(tr("Alterações não salvas"))
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(tr_args(
                "Salvar as alterações em \"{}\" antes de sair?",
                &[&state.project.name],
            ));
            ui.horizontal(|ui| {
                save = ui.button(tr("Salvar e sair")).clicked();
                discard = ui.button(tr("Sair sem salvar")).clicked();
                cancel = ui.button(tr("Cancelar")).clicked();
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
    // What the controls asked for: applied after the window is drawn
    let mut target: Option<(TrackId, EventId)> = None;
    let mut rename: Option<String> = None;
    let mut switch: Option<(Switch, bool)> = None;
    let mut resample: Option<neoveg_engine::ResampleMode> = None;
    let mut header: Option<event_header::HeaderButton> = None;
    crate::dialogs::tool_window(ctx, tr("Inspetor"))
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
                ui.label(tr("Nenhum evento selecionado"));
                return;
            };
            target = Some((track_id, event_id));
            let kind = state
                .timeline
                .track(track_id)
                .map_or(TrackKind::Audio, |t| t.kind);
            let ev = state
                .timeline
                .track(track_id)
                .and_then(|t| t.events.iter().find(|e| e.id == event_id))
                .cloned();
            let Some(ev) = ev else {
                return;
            };
            let generated = state.is_generated_event(&ev);
            let mut new_name = name.clone();
            ui.horizontal(|ui| {
                ui.label(tr("Nome"));
                let response = ui.text_edit_singleline(&mut new_name);
                if response.lost_focus() && new_name.trim() != name {
                    rename = Some(new_name.clone());
                }
            });
            let what = if ev.adjustment {
                tr("Evento de ajuste (sem mídia: os efeitos valem para as faixas abaixo)")
            } else if generated {
                tr("Mídia gerada")
            } else if kind == TrackKind::Video {
                tr("Evento de vídeo")
            } else {
                tr("Evento de áudio")
            };
            ui.weak(what);
            if let Some(media) = ev
                .media_id
                .and_then(|id| state.project.media_pool.iter().find(|m| m.id == id))
            {
                ui.weak(&media.path);
                ui.weak(tr_args(
                    "Duração da mídia: {} s",
                    &[&format!("{:.3}", media.duration_seconds)],
                ));
            }
            ui.label(tr_args(
                "Corte de entrada: {} s",
                &[&format!("{:.3}", g.trim_in)],
            ));
            ui.separator();

            let mut new = g;
            let fps_min_len = 1.0 / state.project.settings.fps.max(1.0);
            // Where it starts and how long it lasts: both go through the same rules as the mouse
            let source_len = state.source_len(track_id, event_id);
            ui.horizontal(|ui| {
                ui.label(tr("Início"));
                let mut start = g.start;
                if ui
                    .add(
                        egui::DragValue::new(&mut start)
                            .range(0.0..=100_000.0)
                            .speed(0.04)
                            .fixed_decimals(3)
                            .suffix(" s"),
                    )
                    .changed()
                {
                    new = edit::move_to(new, start);
                }
                ui.label(tr("Duração"));
                let mut length = g.duration;
                if ui
                    .add(
                        egui::DragValue::new(&mut length)
                            .range(fps_min_len..=100_000.0)
                            .speed(0.04)
                            .fixed_decimals(3)
                            .suffix(" s"),
                    )
                    .changed()
                {
                    new = edit::trim_right(new, new.start + length, fps_min_len, source_len);
                }
            });
            ui.horizontal(|ui| {
                ui.label(tr("Velocidade"));
                let mut rate = edit::clamp_playback_rate(new.playback_rate);
                if ui
                    .add(
                        egui::DragValue::new(&mut rate)
                            .range(edit::MIN_PLAYBACK_RATE..=edit::MAX_PLAYBACK_RATE)
                            .speed(0.01)
                            .fixed_decimals(2)
                            .suffix("×"),
                    )
                    .on_hover_text(tr(
                        "0,25× a 4,00× (Ctrl + arrastar borda do evento na timeline)",
                    ))
                    .changed()
                {
                    new = edit::set_playback_rate(new, rate, fps_min_len);
                }
                if ui
                    .small_button("1,00×")
                    .on_hover_text(tr("Restaurar velocidade original"))
                    .clicked()
                {
                    new = edit::set_playback_rate(new, 1.0, fps_min_len);
                }
            });
            ui.checkbox(
                &mut new.preserve_pitch,
                tr("Preservar tom ao esticar (WSOLA)"),
            );
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(tr("Fade-in"));
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
                ui.label(tr("Fade-out"));
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
            ui.separator();
            // Switches (the ones that make sense for the kind of event)
            ui.horizontal_wrapped(|ui| {
                for (label, which, audio_only) in [
                    (tr("Mudo"), Switch::Mute, false),
                    (tr("Travado"), Switch::Lock, false),
                    (tr("Laço"), Switch::Loop, false),
                    (tr("Inverter fase"), Switch::InvertPhase, true),
                    (tr("Normalizar"), Switch::Normalize, true),
                ] {
                    if audio_only && kind != TrackKind::Audio {
                        continue;
                    }
                    let mut on = which.is_on(&ev);
                    if ui.checkbox(&mut on, label).changed() {
                        switch = Some((which, on));
                    }
                }
            });
            if kind == TrackKind::Video && !ev.adjustment {
                ui.horizontal(|ui| {
                    ui.label(tr("Reamostragem"));
                    egui::ComboBox::from_id_salt("inspector_resample")
                        .selected_text(ev.resample.label())
                        .show_ui(ui, |ui| {
                            for mode in neoveg_engine::ResampleMode::ALL {
                                if ui
                                    .selectable_label(ev.resample == mode, mode.label())
                                    .clicked()
                                {
                                    resample = Some(mode);
                                }
                            }
                        });
                });
            }
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                if kind == TrackKind::Video
                    && !ev.adjustment
                    && ui.button(tr("Pan/Crop…")).clicked()
                {
                    header = Some(event_header::HeaderButton::PanCrop);
                }
                if ui.button(tr("Efeitos…")).clicked() {
                    header = Some(event_header::HeaderButton::Fx);
                }
                if generated && ui.button(tr("Editar mídia gerada…")).clicked() {
                    header = Some(event_header::HeaderButton::GeneratedMedia);
                }
            });
            if new != g {
                change = Some((track_id, event_id, new));
            }
        });
    if let Some((track_id, event_id, new)) = change {
        state.apply_geometry(track_id, event_id, new);
    }
    if let Some((track_id, event_id)) = target {
        let command = if let Some(name) = rename {
            neoveg_engine::switches::rename_event(&state.timeline, (track_id, event_id), &name)
        } else if let Some((which, on)) = switch {
            neoveg_engine::switches::set(&state.timeline, &[(track_id, event_id)], which, on)
        } else if let Some(mode) = resample {
            neoveg_engine::switches::set_resample(&state.timeline, &[(track_id, event_id)], mode)
        } else {
            None
        };
        if let Some(command) = command {
            if state.history.execute(&mut state.timeline, command) {
                state.last_frame_key = None;
                state.last_composite_key = None;
                state.restart_audio_if_playing();
            }
        }
        if let Some(button) = header {
            state.header_button_clicked(track_id, event_id, button, Pos2::ZERO);
        }
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
        tr(tab.title()).into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Tab) {
        // The footer strip of the group this window is in (a detached panel has none). The Media panel
        // alone in its group already has strips of its own, so it gets none from the dock.
        let leaf = self
            .state
            .dock_footer
            .leaf_of(*tab)
            .filter(|leaf| !(*tab == Tab::Media && leaf.tabs.len() == 1))
            .cloned();
        let Some(leaf) = leaf else {
            self.content(ui, tab);
            return;
        };
        egui::TopBottomPanel::bottom(egui::Id::new((
            "neoveg_dock_footer_area",
            leaf.surface.0,
            leaf.node.0,
        )))
        .exact_height(crate::footer_tabs::HEIGHT)
        .frame(egui::Frame::none())
        .show_separator_line(false)
        .show_inside(ui, |ui| self.state.dock_footer.strip(ui, &leaf));
        // The window draws in what is left, clipped: a panel taller than its group must not paint
        // over the footer strip
        let rest = ui.available_rect_before_wrap();
        let mut body = ui.new_child(egui::UiBuilder::new().max_rect(rest).layout(*ui.layout()));
        body.set_clip_rect(rest.intersect(ui.clip_rect()));
        self.content(&mut body, tab);
    }

    fn closeable(&mut self, _tab: &mut Tab) -> bool {
        false
    }
}

impl Viewer<'_> {
    fn content(&mut self, ui: &mut egui::Ui, tab: &mut Tab) {
        match tab {
            Tab::Preview => preview_ui(self.state, ui),
            Tab::Timeline => timeline_ui(self.state, ui),
            Tab::Media => media_ui(self.state, ui),
            Tab::MasterBus => master_bus_ui(self.state, ui),
            Tab::AudioFx => audio_fx_ui(self.state, ui),
            Tab::VideoFx => video_fx_ui(self.state, ui),
            Tab::Transitions => transitions_ui(self.state, ui),
            Tab::Scopes => {
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| scopes_tab_ui(self.state, ui))
                    .inner
            }
            Tab::Loudness => {
                let snapshot = self.state.loudness.snapshot();
                let restart = crate::loudness_panel::loudness_ui(
                    ui,
                    snapshot,
                    self.state.playing,
                    &mut self.state.loudness_target,
                );
                if restart {
                    self.state.loudness.request_reset();
                }
            }
        }
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
            .on_hover_text(tr("Qualidade da prévia: Draft 1/4 e Preview 1/2 mostram a resolução reduzida em pixels; Good e Best usam a resolução total; Auto ajusta à janela"));
        ui.checkbox(&mut state.show_bars, tr("Barras SMPTE"));
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
        let grade = state.effective_grade();
        let chroma = state.effective_chroma();
        if state.bars_grade != Some((grade, chroma)) {
            state.bars_texture = None;
            state.bars_scopes = None;
            state.bars_scopes_rev += 1;
            state.bars_grade = Some((grade, chroma));
        }
        let texture = state.bars_texture.get_or_insert_with(|| {
            let mut bars = neoveg_video::generators::smpte_color_bars(640, 360);
            neoveg_video::chroma::apply_chroma_key(&mut bars, &chroma);
            neoveg_video::grading::apply_grade(&mut bars, &grade);
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

    // Subtitles showing at the cursor
    let showing = captions::active_at(&state.timeline, state.playhead);
    if !showing.is_empty() {
        captions_panel::paint_overlay(&painter.with_clip_rect(frame_rect), frame_rect, &showing);
    }

    let factor = match QUALITIES[state.quality].1 {
        f if f > 0.0 => f,
        _ => scale.min(1.0),
    };
    painter.text(
        rect.left_bottom() + vec2(6.0, -6.0),
        Align2::LEFT_BOTTOM,
        tr_args(
            "{}x{} · prévia {}x{} · {}",
            &[
                &(pw as u32),
                &(ph as u32),
                &((pw * factor) as u32),
                &((ph * factor) as u32),
                &seconds_to_timecode(
                    state.playhead,
                    state.project.settings.fps,
                    state.drop_frame(),
                ),
            ],
        ),
        FontId::monospace(11.0),
        Color32::from_gray(170),
    );
}

/// Classic NLE compact footer tabs: flat 22 px headers in a strip at the bottom of the panel.
fn footer_tabs(ui: &mut egui::Ui, id: &str, labels: &[&'static str], selected: &mut usize) {
    egui::TopBottomPanel::bottom(egui::Id::new(id))
        .exact_height(crate::footer_tabs::HEIGHT)
        .frame(egui::Frame::none())
        .show_separator_line(false)
        .show_inside(ui, |ui| {
            let names: Vec<String> = labels.iter().map(|l| tr(l).to_owned()).collect();
            let out = crate::footer_tabs::show(ui, egui::Id::new(id), &names, *selected, false);
            if let Some(index) = out.clicked {
                *selected = index;
            }
        });
}

fn media_ui(state: &mut AppState, ui: &mut egui::Ui) {
    let mut tab = state.media_tab;
    footer_tabs(
        ui,
        "media_footer",
        &["Mídia do projeto", "Trimmer", "Legendas"],
        &mut tab,
    );
    state.media_tab = tab;
    match tab {
        0 => project_media_ui(state, ui),
        1 => trimmer_ui(state, ui),
        _ => {
            let now = ui.input(|i| i.time);
            let actions = captions_panel::captions_ui(
                ui,
                &mut state.captions_panel,
                &state.timeline.captions,
                state.playhead,
            );
            state.apply_caption_actions(actions, now);
        }
    }
}

fn project_media_ui(state: &mut AppState, ui: &mut egui::Ui) {
    if ui.button(tr("Importar mídia…")).clicked() {
        let ctx = ui.ctx().clone();
        state.import_media(&ctx);
    }
    ui.separator();
    if state.project.media_pool.is_empty() {
        ui.weak(tr("Nenhuma mídia no projeto"));
    }
    let mut open = None;
    for item in &state.project.media_pool {
        if let Some(m) = state.media.get(&item.id) {
            let row = ui
                .selectable_label(
                    state
                        .trimmer
                        .as_ref()
                        .is_some_and(|t| t.media_id == item.id),
                    format!("{}  ({:.2} s, {} Hz)", m.name, m.duration, m.sample_rate),
                )
                .on_hover_text(tr("Duplo clique: abrir no Trimmer"));
            if row.double_clicked() {
                open = Some(item.id);
            }
        }
    }
    if let Some(id) = open {
        state.open_in_trimmer(id);
    }
}

fn trimmer_ui(state: &mut AppState, ui: &mut egui::Ui) {
    if state.trimmer.is_none() {
        // Without a choice, the last media of the project
        if let Some(id) = state.project.media_pool.last().map(|m| m.id) {
            state.open_in_trimmer(id);
            state.media_tab = 1;
        }
    }
    let Some(trimmer) = state.trimmer.as_mut() else {
        ui.weak(tr(
            "Abra uma mídia no Trimmer (duplo clique em Mídia do Projeto)",
        ));
        return;
    };
    let fps = state.project.settings.fps;
    let mut add = None;
    ui.horizontal(|ui| {
        ui.label(&trimmer.name);
        ui.weak(format!(
            "{} → {}",
            seconds_to_timecode(trimmer.selection().0, fps, false),
            seconds_to_timecode(trimmer.selection().1, fps, false)
        ));
        if icons::svg_button(ui, icons::SvgIcon::StepBack, "Quadro anterior", false).clicked() {
            trimmer.step_frames(-1);
        }
        if icons::svg_button(ui, icons::SvgIcon::StepForward, "Próximo quadro", false).clicked() {
            trimmer.step_frames(1);
        }
        if icons::svg_button(ui, icons::SvgIcon::TrimStart, "Marcar entrada no cursor do Trimmer", false)
            .clicked()
        {
            trimmer.mark_in();
        }
        if icons::svg_button(ui, icons::SvgIcon::TrimEnd, "Marcar saída no cursor do Trimmer", false)
            .clicked()
        {
            trimmer.mark_out();
        }
        if icons::svg_button(ui, icons::SvgIcon::Undo, "Desfazer marcação", false).clicked() {
            trimmer.undo_marks();
        }
        if icons::svg_button(ui, icons::SvgIcon::Redo, "Refazer marcação", false).clicked() {
            trimmer.redo_marks();
        }
        ui.separator();
        if icons::svg_button(
            ui,
            icons::SvgIcon::Add,
            "Adicionar à linha do tempo a partir do cursor (três pontos; com seleção na linha do tempo, dentro dela)",
            false,
        )
        .clicked()
        {
            add = Some(TrimmerAdd::FromCursor);
        }
        if icons::svg_button(
            ui,
            icons::SvgIcon::Cut,
            "Adicionar à linha do tempo até o cursor",
            false,
        )
        .clicked()
        {
            add = Some(TrimmerAdd::UpToCursor);
        }
        if icons::svg_button(
            ui,
            icons::SvgIcon::PitchStretch,
            "Ajustar ao preenchimento: comprime ou estica a seleção sobre a seleção da linha do tempo (quatro pontos)",
            false,
        )
        .clicked()
        {
            add = Some(TrimmerAdd::FitToFill);
        }
        let overwrite = state.trimmer_overwrite;
        if icons::svg_button(
            ui,
            icons::SvgIcon::Remove,
            "Sobrescrever a linha do tempo: o novo evento substitui o que ele cobre",
            overwrite,
        )
        .clicked()
        {
            state.trimmer_overwrite = !overwrite;
        }
    });
    let duration = trimmer.duration.max(1e-6);
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width(), ui.available_height().max(60.0)),
        Sense::click_and_drag(),
    );
    let to_time = |x: f32| ((x - rect.left()) / rect.width().max(1.0)) as f64 * duration;
    if response.double_clicked() {
        trimmer.reset_marks();
    } else if response.drag_started() {
        if let Some(pos) = response.interact_pointer_pos() {
            state.trimmer_drag_from = Some(to_time(pos.x));
        }
    } else if response.dragged() {
        if let (Some(from), Some(pos)) = (state.trimmer_drag_from, response.interact_pointer_pos())
        {
            trimmer.select(from, to_time(pos.x));
        }
    } else if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            trimmer.set_cursor(to_time(pos.x));
        }
    }
    if response.drag_stopped() {
        state.trimmer_drag_from = None;
    }
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, Rounding::ZERO, Color32::from_gray(22));
    if let Some(media) = state.media.get(&trimmer.media_id) {
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
    let x_of = |t: f64| rect.left() + (t / duration) as f32 * rect.width();
    let (a, b) = trimmer.selection();
    let shade = egui::Rect::from_x_y_ranges(x_of(a)..=x_of(b), rect.y_range());
    painter.rect_filled(
        shade,
        Rounding::ZERO,
        Color32::from_rgba_unmultiplied(86, 204, 242, 40),
    );
    let cursor_x = x_of(trimmer.cursor());
    painter.line_segment(
        [pos2(cursor_x, rect.top()), pos2(cursor_x, rect.bottom())],
        egui::Stroke::new(1.0_f32, Color32::from_rgb(52, 152, 219)),
    );
    if let Some(mode) = add {
        state.trimmer_add(mode);
    }
}

fn fmt_db(linear: f32) -> String {
    let db = linear_to_dbfs(linear);
    if db.is_finite() {
        format!("{db:+.1}")
    } else {
        "-∞".into()
    }
}

/// The audio effects and per-track mixer panel: the strip + chain of the selected audio track,
/// mute for a selected video track, or the master bus chain.
/// The Video Scopes tab: the color bars when they are on, else the frame at the cursor.
fn scopes_tab_ui(state: &mut AppState, ui: &mut egui::Ui) {
    if state.show_bars {
        let grade = state.effective_grade();
        let chroma = state.effective_chroma();
        if state.bars_grade != Some((grade, chroma)) {
            state.bars_scopes = None;
            state.bars_scopes_rev += 1;
        }
        let bars = state.bars_scopes.get_or_insert_with(|| {
            let mut frame = neoveg_video::generators::smpte_color_bars(640, 360);
            neoveg_video::chroma::apply_chroma_key(&mut frame, &chroma);
            neoveg_video::grading::apply_grade(&mut frame, &grade);
            Scopes::from_frame(&frame)
        });
        scopes_ui(
            ui,
            &mut state.scopes_panel,
            Some(bars),
            u64::MAX - state.bars_scopes_rev,
        );
        return;
    }
    let showing_video = state.video_texture.is_some();
    let scopes = state.frame_scopes.as_ref().filter(|_| showing_video);
    scopes_ui(
        ui,
        &mut state.scopes_panel,
        scopes,
        state.frame_scopes_version,
    );
}

fn transitions_ui(state: &mut AppState, ui: &mut egui::Ui) {
    let Some((track_id, event_id)) = state.transitions_target() else {
        state.transitions.show(ui, None);
        return;
    };
    let outgoing = state.transitions.outgoing;
    let overlap = state.transition_overlap(track_id, event_id);
    let Some(event) = state
        .timeline
        .track(track_id)
        .and_then(|t| t.events.iter().find(|e| e.id == event_id))
        .cloned()
    else {
        return;
    };
    let current = if outgoing {
        event.transition_out.clone()
    } else {
        event.transition_in.clone()
    };
    let length = if outgoing {
        event.fade_out
    } else {
        event.fade_in.max(overlap)
    };
    let view = TransitionsView {
        event_name: &event.name,
        current: current.as_ref(),
        length,
        overlaps: !outgoing && overlap > event.fade_in,
    };
    let actions = state.transitions.show(ui, Some(&view));
    let outgoing = state.transitions.outgoing;
    state.apply_transition_actions(track_id, event_id, outgoing, actions);
}

fn video_fx_ui(state: &mut AppState, ui: &mut egui::Ui) {
    let (target, title, scopes) = state.vfx_target();
    let chain = vfxe::chain(&state.timeline, target)
        .map(<[_]>::to_vec)
        .unwrap_or_default();
    let fade_color = match target {
        VideoFxTarget::Track(id) => state.timeline.track(id).map(|t| t.fade_color),
        _ => None,
    };
    let actions = video_fx_panel::vfx_ui(ui, &mut state.vfx, &title, &chain, scopes, fade_color);
    // A slider being dragged shows in the preview at once; the project changes on release
    state.vfx_draft = state
        .vfx
        .draft_view()
        .map(|(index, param, value)| VfxDraft {
            target,
            index,
            param: param.to_owned(),
            value,
        });
    state.apply_vfx_actions(target, actions);
    if let VideoFxTarget::Event { track_id, event_id } = target {
        let adjustment = state
            .timeline
            .track(track_id)
            .and_then(|t| t.events.iter().find(|e| e.id == event_id))
            .is_some_and(|e| e.adjustment);
        if !adjustment {
            velocity_section(state, ui, track_id, event_id);
        }
    }
}

/// The velocity graph of the selected video event, under its effects.
fn velocity_section(state: &mut AppState, ui: &mut egui::Ui, track_id: TrackId, event_id: EventId) {
    let Some(event) = state
        .timeline
        .track(track_id)
        .and_then(|t| t.events.iter().find(|e| e.id == event_id))
        .cloned()
    else {
        return;
    };
    ui.separator();
    let open = event.velocity.is_some();
    let section = egui::CollapsingHeader::new(tr("Velocidade do evento"))
        .id_salt("event_velocity")
        .default_open(open)
        .show(ui, |ui| {
            let view = VelocityView {
                envelope: event.velocity.as_ref(),
                duration: event.duration,
                time: (state.playhead - event.start).clamp(0.0, event.duration),
            };
            let actions = state.velocity_editor.show(ui, (track_id, event_id), &view);
            state.apply_velocity_actions(track_id, event_id, actions);
        });
    if section.body_returned.is_none() {
        // Folded away: nothing of an earlier drag may linger in the preview
        state.velocity_editor.reset();
        state.velocity_draft = None;
    }
}

/// Which chain the effects tab edits: the selected track's, one audio bus's or the master's.
fn fx_target_picker(state: &mut AppState, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        let on_track = !state.fx.master && state.fx.bus.is_none();
        if ui.selectable_label(on_track, tr("Faixa")).clicked() {
            state.fx.master = false;
            state.fx.bus = None;
        }
        let current = state
            .fx
            .bus
            .filter(|id| state.timeline.buses.iter().any(|b| b.id == *id));
        let selected = current.map_or_else(
            || tr("Barramento").to_owned(),
            |id| neoveg_engine::buses::label(&state.timeline, id),
        );
        egui::ComboBox::from_id_salt("fx_bus_pick")
            .selected_text(selected)
            .width(120.0)
            .show_ui(ui, |ui| {
                let list: Vec<(neoveg_engine::BusId, String)> = state
                    .timeline
                    .buses
                    .iter()
                    .map(|b| (b.id, neoveg_engine::buses::label(&state.timeline, b.id)))
                    .collect();
                if list.is_empty() {
                    ui.weak(tr("Nenhum barramento"));
                }
                for (id, label) in list {
                    if ui.selectable_label(current == Some(id), label).clicked() {
                        state.fx.master = false;
                        state.fx.bus = Some(id);
                    }
                }
            });
        if ui.selectable_label(state.fx.master, tr("Master")).clicked() {
            state.fx.master = true;
            state.fx.bus = None;
        }
        if ui
            .button(tr("+ Barramento"))
            .on_hover_text(tr("Inserir barramento de áudio (Ctrl+Alt+B)"))
            .clicked()
        {
            state.add_audio_bus();
        }
    });
}

fn audio_fx_ui(state: &mut AppState, ui: &mut egui::Ui) {
    fx_target_picker(state, ui);
    let (target, title) = match state.fx_target() {
        Some(found) => found,
        None => {
            if let Some(video) = state
                .selected_track
                .and_then(|id| state.timeline.track(id).cloned())
                .filter(|t| t.kind == TrackKind::Video)
            {
                let exclusive = ui.input(|i| i.modifiers.command || i.modifiers.shift);
                ui.horizontal(|ui| {
                    if icons::icon_button(
                        ui,
                        Icon::Mute,
                        "Mudo na faixa de vídeo (Z; Ctrl+clique = só esta faixa)",
                        video.muted,
                    )
                    .clicked()
                    {
                        let action = if exclusive {
                            MixAction::MuteExclusive
                        } else {
                            MixAction::Mute(!video.muted)
                        };
                        state.apply_mix_actions(video.id, vec![action]);
                    }
                    ui.label(&video.name);
                });
                ui.weak(tr("Faixa de vídeo: sem efeitos de áudio."));
            } else {
                ui.weak(tr(
                    "Selecione uma faixa de áudio para ver o mixer e os efeitos dela.",
                ));
            }
            return;
        }
    };
    if let FxTarget::Track(id) = target {
        if let Some(track) = state.timeline.track(id).cloned() {
            let mix_actions =
                mixer_panel::mixer_ui(ui, &mut state.mixer, &track, state.timeline.pan_law);
            if let (Some(active), Some(preview)) =
                (&state.active_player, state.mixer.preview_mix(&track))
            {
                if let Some(idx) = state.timeline.tracks.iter().position(|t| t.id == id) {
                    active.player.set_track_mix(
                        idx,
                        neoveg_audio::fxmixer::TrackMixSpec {
                            gain: preview.gain(),
                            pan: preview.pan,
                            invert: preview.invert_phase,
                        },
                    );
                }
            }
            state.apply_mix_actions(id, mix_actions);
            ui.separator();
            // Where the track goes and its sends
            let route_actions = crate::buses_panel::route_ui(ui, &state.timeline, &track);
            state.apply_route_actions(id, route_actions);
            ui.separator();
        }
    }
    if let FxTarget::Bus(id) = target {
        if let Some(bus) = state.timeline.buses.iter().find(|b| b.id == id).cloned() {
            let actions = crate::buses_panel::bus_strip(ui, &state.timeline, &bus);
            state.apply_bus_actions(actions);
            ui.separator();
        }
    }
    let chain = fx::chain(&state.timeline, target)
        .map(<[_]>::to_vec)
        .unwrap_or_default();
    let duck = match target {
        FxTarget::Track(id) => fx_panel::duck_view(&state.timeline, id),
        FxTarget::Master | FxTarget::Bus(_) => None,
    };
    let actions = fx_panel::fx_ui(ui, &mut state.fx, &title, &chain, duck.as_ref());
    if let (Some(active), Some((fx_idx, param, val))) =
        (&state.active_player, state.fx.active_effect_draft(&chain))
    {
        let track_idx = match target {
            FxTarget::Track(id) => state.timeline.tracks.iter().position(|t| t.id == id),
            FxTarget::Master | FxTarget::Bus(_) => None,
        };
        // A bus effect is not in the live table of the player: it is heard after the next restart
        if matches!(target, FxTarget::Master) || track_idx.is_some() {
            active
                .player
                .set_effect_param(track_idx, fx_idx, param, val);
        }
    }
    state.apply_fx_actions(target, actions);
}

/// Shortest meter that keeps the 0 / -6 / -12 scale labels apart.
const METER_MIN_HEIGHT: f32 = 130.0;

fn master_bus_ui(state: &mut AppState, ui: &mut egui::Ui) {
    // A short panel scrolls instead of squeezing the dBFS scale into overlapping labels
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| master_bus_content(state, ui));
}

fn master_bus_content(state: &mut AppState, ui: &mut egui::Ui) {
    // Meters only ever show what the audio pipeline reported; stopped means zero
    let (l, r) = if state.playing {
        state.meter.peak()
    } else {
        (0.0, 0.0)
    };

    let meter_h = (ui.available_height() - 80.0).max(METER_MIN_HEIGHT);
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
            ui.visuals().weak_text_color(),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeaderControl {
    Mute,
    Solo,
    Phase,
}

impl HeaderControl {
    fn icon(self) -> Icon {
        match self {
            Self::Mute => Icon::Mute,
            Self::Solo => Icon::Solo,
            Self::Phase => Icon::Phase,
        }
    }

    fn tooltip(self) -> &'static str {
        match self {
            Self::Mute => "Mudo (Z; Ctrl+clique = só esta faixa)",
            Self::Solo => "Solo (X; Ctrl+clique = só esta faixa)",
            Self::Phase => "Inverter a fase",
        }
    }
}

/// Rectangles of the Mute, Solo and Phase buttons inside a track header row.
fn header_buttons(row: Rect, kind: TrackKind) -> Vec<(HeaderControl, Rect)> {
    let side = 20.0;
    let y = row.top() + 34.0;
    let button = |x: f32| Rect::from_min_size(pos2(row.left() + x, y), vec2(side, side));
    match kind {
        TrackKind::Audio => vec![
            (HeaderControl::Mute, button(6.0)),
            (HeaderControl::Solo, button(29.0)),
            (HeaderControl::Phase, button(52.0)),
        ],
        TrackKind::Video => vec![(HeaderControl::Mute, button(6.0))],
    }
}

fn hit_header_control(row: Rect, kind: TrackKind, p: Pos2) -> Option<HeaderControl> {
    header_buttons(row, kind)
        .into_iter()
        .find_map(|(control, r)| r.contains(p).then_some(control))
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

/// The header button of an event under `p`, with the event and the button's rectangle.
fn header_button_at(
    state: &AppState,
    lanes: Rect,
    lane_top: f32,
    p: Pos2,
) -> Option<(TrackId, EventId, event_header::HeaderButton, Rect)> {
    let row = ((p.y - lane_top) / TRACK_HEIGHT).floor();
    if row < 0.0 || p.x < lanes.left() || p.x > lanes.right() {
        return None;
    }
    let track = state.timeline.tracks.get(row as usize)?;
    let row_y = lane_top + row * TRACK_HEIGHT;
    let to_px = |t: f64| {
        (lanes.left() + ((t - state.scroll_seconds) * state.pixels_per_second) as f32).clamp(
            lanes.left() - OFFSCREEN_CLAMP_PX,
            lanes.right() + OFFSCREEN_CLAMP_PX,
        )
    };
    track.events.iter().rev().find_map(|ev| {
        let (x0, x1) = (to_px(ev.start), to_px(ev.end()));
        if p.x < x0 || p.x > x1 {
            return None;
        }
        let ev_rect =
            Rect::from_min_max(pos2(x0, row_y + 2.0), pos2(x1, row_y + TRACK_HEIGHT - 2.0));
        let buttons =
            event_header::buttons_for(track.kind, state.is_generated_event(ev), ev.adjustment);
        let placed = event_header::layout(ev_rect, &buttons, EVENT_HEADER_HEIGHT);
        let button = event_header::hit(&placed, p)?;
        let rect = placed.iter().find(|(b, _)| *b == button)?.1;
        Some((track.id, ev.id, button, rect))
    })
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

fn envelope_context_menu(state: &mut AppState, ui: &mut egui::Ui, hit: &EnvelopeHit) {
    let target = FxTarget::Track(hit.track_id);
    let Some(env) = state
        .timeline
        .track(hit.track_id)
        .and_then(|t| t.envelopes.iter().find(|e| e.target == hit.target))
        .cloned()
    else {
        ui.label(tr("Envelope não encontrado"));
        return;
    };
    let title = state
        .timeline
        .track(hit.track_id)
        .map(|t| envelope_canvas::target_title(&t.effects, &hit.target))
        .unwrap_or_else(|| hit.target.label());
    ui.strong(title);
    ui.separator();

    let mut cmd_to_run = None;
    if let Some(idx) = hit.point_index {
        let current_shape = env
            .points
            .get(idx)
            .map_or(EnvelopeShape::Linear, |p| p.shape);
        ui.menu_button(tr("Curva de interpolação"), |ui| {
            for shape in EnvelopeShape::ALL {
                if ui
                    .selectable_label(shape == current_shape, shape.label())
                    .clicked()
                {
                    cmd_to_run = envelope::set_keyframe_shape(
                        &state.timeline,
                        target,
                        &hit.target,
                        idx,
                        shape,
                    );
                    ui.close_menu();
                }
            }
        });
        if ui.button(tr("Restaurar valor padrão")).clicked() {
            cmd_to_run = envelope::move_keyframe(
                &state.timeline,
                target,
                &hit.target,
                idx,
                (hit.time, hit.default),
                hit.bounds,
            );
            ui.close_menu();
        }
        if ui
            .add_enabled(env.points.len() > 1, egui::Button::new(tr("Remover ponto")))
            .clicked()
        {
            cmd_to_run = envelope::remove_keyframe(&state.timeline, target, &hit.target, idx);
            ui.close_menu();
        }
    } else if ui.button(tr("Adicionar ponto aqui")).clicked() {
        let pt = envelope_canvas::new_point_on_curve(&env, hit.time, hit.value);
        cmd_to_run =
            envelope::upsert_keyframe(&state.timeline, target, &hit.target, pt, hit.bounds, 1e-4);
        ui.close_menu();
    }

    ui.separator();
    let mut enabled = env.enabled;
    if ui.checkbox(&mut enabled, tr("Automação ativa")).changed() {
        cmd_to_run = envelope::set_envelope_enabled(&state.timeline, target, &hit.target, enabled);
        ui.close_menu();
    }
    if ui.button(tr("Ocultar envelope")).clicked() {
        cmd_to_run =
            envelope::toggle_envelope(&state.timeline, target, hit.target.clone(), hit.default);
        ui.close_menu();
    }
    if ui.button(tr("Remover envelope")).clicked() {
        cmd_to_run = envelope::remove_envelope(&state.timeline, target, &hit.target);
        ui.close_menu();
    }

    if let Some(cmd) = cmd_to_run {
        if state.history.execute(&mut state.timeline, cmd) {
            state.restart_audio_if_playing();
        }
    }
}

fn event_context_menu(
    state: &mut AppState,
    ui: &mut egui::Ui,
    track_id: TrackId,
    event_id: EventId,
) {
    let Some(g) = state.event_geometry(track_id, event_id) else {
        ui.label(tr("Evento não encontrado"));
        return;
    };
    if let Some(path) = state.subproject_path_of(track_id, event_id) {
        if ui.button(tr("Abrir sub-projeto em uma aba")).clicked() {
            state.tabs.open_request = Some(path);
            ui.close_menu();
        }
        ui.separator();
    }
    command_item(state, ui, Command::FileCreateSubproject);
    if pcrop::get(&state.timeline, track_id, event_id).is_some()
        && ui.button(tr("Pan/Crop do evento…")).clicked()
    {
        state.open_pan_crop(track_id, event_id);
        ui.close_menu();
    }
    // Only video events have a velocity: `Some(true)` when one is already there
    if let Some(has_velocity) = vel::get(&state.timeline, track_id, event_id).map(|v| v.is_some()) {
        let label = if has_velocity {
            "Remover envelope de velocidade"
        } else {
            "Inserir envelope de velocidade"
        };
        if ui.button(label).clicked() {
            let next = (!has_velocity).then(|| vel::new_envelope(1.0));
            state.set_velocity(track_id, event_id, next);
            ui.close_menu();
        }
        if ui.button(tr("Congelar quadro no cursor")).clicked() {
            state.freeze_frame_at_cursor(track_id, event_id);
            ui.close_menu();
        }
    }
    ui.separator();
    for command in [
        Command::EditCut,
        Command::EditCopy,
        Command::EditPasteAttributes,
        Command::EditPasteAttributesSelective,
        Command::EditDelete,
        Command::EditGroup,
        Command::EditUngroup,
        Command::EditRemoveFromGroup,
    ] {
        command_item(state, ui, command);
    }
    ui.menu_button(tr("Interruptores"), |ui| switch_items(state, ui));
    ui.menu_button(tr("Canais de áudio"), |ui| channel_items(state, ui));
    if ui.button(tr("Selecionar eventos até o fim")).clicked() {
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
        .add_enabled(has_fades, egui::Button::new(tr("Remover fades")))
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
    ui.separator();
    let fps_min_len = 1.0 / state.project.settings.fps.max(1.0);
    ui.menu_button("Velocidade (Time-Stretch)", |ui| {
        for &(rate, label) in &[
            (0.25, "0,25× (câmera lenta 4×)"),
            (0.50, "0,50× (metade)"),
            (0.75, "0,75×"),
            (1.00, "1,00× (normal)"),
            (1.25, "1,25×"),
            (1.50, "1,50×"),
            (2.00, "2,00× (dobro)"),
            (4.00, "4,00×"),
        ] {
            let active = (g.playback_rate - rate).abs() < 1e-3;
            if ui.selectable_label(active, label).clicked() {
                state.apply_geometry(
                    track_id,
                    event_id,
                    edit::set_playback_rate(g, rate, fps_min_len),
                );
                ui.close_menu();
            }
        }
        ui.separator();
        let mut preserve = g.preserve_pitch;
        if ui
            .checkbox(&mut preserve, "Preservar tom (WSOLA)")
            .changed()
        {
            state.apply_geometry(
                track_id,
                event_id,
                EventGeometry {
                    preserve_pitch: preserve,
                    ..g
                },
            );
            ui.close_menu();
        }
    });
}

fn timeline_ui(state: &mut AppState, ui: &mut egui::Ui) {
    let canvas_h = (ui.available_height() - 30.0 - timeline_scroll::BAR).max(100.0);
    timeline_canvas(state, ui, canvas_h);
    timeline_hscrollbar(state, ui);
    toolbar_ui(state, ui);
}

/// The horizontal scrollbar under the lanes: it spans the project (plus room to work) and drives the
/// same time offset as the mouse wheel.
fn timeline_hscrollbar(state: &mut AppState, ui: &mut egui::Ui) {
    let (bar, _) = ui.allocate_exact_size(
        vec2(ui.available_width(), timeline_scroll::BAR),
        Sense::hover(),
    );
    ui.painter()
        .rect_filled(bar, Rounding::ZERO, ui_theme::palette().header);
    let track = Rect::from_min_max(
        pos2(bar.left() + HEADER_WIDTH, bar.top()),
        pos2(bar.right() - timeline_scroll::BAR, bar.bottom()),
    );
    let view = f64::from(state.lanes_width.max(1.0)) / state.pixels_per_second;
    let content = timeline_scroll::content_seconds(
        state.timeline.duration(),
        state.playhead,
        view,
        state.scroll_seconds,
    );
    if let Some(offset) = scrollbar(
        ui,
        "timeline_hscroll",
        track,
        true,
        content,
        view,
        state.scroll_seconds,
    ) {
        state.scroll_seconds = offset.max(0.0);
    }
}

/// Draws a scrollbar in `rect` and returns the offset the user asks for by dragging the thumb or
/// clicking the track (the thumb then centres on the click). `None` while it is left alone.
fn scrollbar(
    ui: &mut egui::Ui,
    name: &str,
    rect: Rect,
    horizontal: bool,
    content: f64,
    view: f64,
    offset: f64,
) -> Option<f64> {
    let id = ui.id().with(name);
    let response = ui.interact(rect, id, Sense::click_and_drag());
    let track_len = if horizontal {
        rect.width()
    } else {
        rect.height()
    };
    let (start, len) = timeline_scroll::thumb(track_len, content, view, offset);
    let mut asked = None;
    if let Some(p) = response.interact_pointer_pos() {
        let pos = if horizontal {
            p.x - rect.left()
        } else {
            p.y - rect.top()
        };
        if response.drag_started() || response.clicked() {
            // Grab the thumb where it was pressed, or centre it on a press in the empty track
            let grab = if pos >= start && pos <= start + len {
                pos - start
            } else {
                len / 2.0
            };
            ui.data_mut(|d| d.insert_temp(id, grab));
        }
        let grab = ui.data(|d| d.get_temp::<f32>(id)).unwrap_or(len / 2.0);
        if response.dragged() || response.clicked() {
            asked = Some(timeline_scroll::offset_for_thumb_start(
                track_len,
                content,
                view,
                len,
                pos - grab,
            ));
        }
    }
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, Rounding::ZERO, Color32::from_gray(24));
    let thumb_rect = if horizontal {
        Rect::from_min_max(
            pos2(rect.left() + start + 1.0, rect.top() + 2.0),
            pos2(rect.left() + start + len - 1.0, rect.bottom() - 2.0),
        )
    } else {
        Rect::from_min_max(
            pos2(rect.left() + 2.0, rect.top() + start + 1.0),
            pos2(rect.right() - 2.0, rect.top() + start + len - 1.0),
        )
    };
    let shade = if response.dragged() {
        150
    } else if response.hovered() {
        120
    } else {
        90
    };
    painter.rect_filled(thumb_rect, Rounding::same(2.0), Color32::from_gray(shade));
    asked
}

fn toolbar_ui(state: &mut AppState, ui: &mut egui::Ui) {
    let now = ui.input(|i| i.time);
    ui.horizontal(|ui| {
        let icon = |ui: &mut egui::Ui, which: Icon, tip: &str| {
            icons::icon_button(ui, which, tip, false).clicked()
        };
        if icon(ui, Icon::GoToStart, "Ir para o início") {
            state.playhead = 0.0;
            state.blink_origin = now;
        }
        let play_icon = if state.playing {
            Icon::Pause
        } else {
            Icon::Play
        };
        if icon(ui, play_icon, "Reproduzir / Pausar (Espaço)") {
            state.toggle_play(now);
        }
        if icon(ui, Icon::Stop, "Parar") {
            state.stop();
        }
        if icons::icon_button(
            ui,
            Icon::Record,
            "Gravar do dispositivo de entrada (Ctrl+R)",
            state.recorder.is_some(),
        )
        .clicked()
        {
            let ctx = ui.ctx().clone();
            state.toggle_recording(&ctx);
        }
        ui.separator();
        if icon(ui, Icon::Split, "Dividir no cursor (S)") {
            state.split_at_playhead();
        }
        if icon(ui, Icon::Undo, "Desfazer (Ctrl+Z)") {
            state.undo();
        }
        if icon(ui, Icon::Redo, "Refazer (Ctrl+Y)") {
            state.redo();
        }
        ui.separator();
        if icon(ui, Icon::AddAudioTrack, "Inserir faixa de áudio (Ctrl+Q)") {
            state.add_track(TrackKind::Audio);
        }
        if icon(
            ui,
            Icon::AddVideoTrack,
            "Inserir faixa de vídeo (Ctrl+Shift+Q)",
        ) {
            state.add_track(TrackKind::Video);
        }
        if icons::icon_button(
            ui,
            Icon::Loop,
            "Reprodução em laço (Q); Shift+Q define a região de laço",
            state.looping,
        )
        .clicked()
        {
            state.toggle_loop();
        }
        if icons::icon_button(
            ui,
            Icon::Group,
            "Agrupamento de eventos (G agrupa, U desagrupa, Ctrl+Shift+U ignora)",
            !state.ignore_grouping,
        )
        .clicked()
        {
            state.toggle_ignore_grouping();
        }
        let selected_refs = state.selection.refs();
        let all_locked = !selected_refs.is_empty()
            && switches::state(&state.timeline, &selected_refs, Switch::Lock) == SwitchState::On;
        if icons::icon_button(
            ui,
            Icon::Lock,
            "Bloquear ou desbloquear os eventos selecionados (Editar > Interruptores)",
            all_locked,
        )
        .clicked()
        {
            state.toggle_event_switch(Switch::Lock);
        }
        ui.separator();
        for tool in EditTool::ALL {
            let selected = state.edit_tool == tool;
            if icons::svg_button(ui, tool.icon(), tool.label(), selected).clicked() {
                state.set_edit_tool(tool);
            }
        }
        ui.separator();
        if icons::icon_button(
            ui,
            Icon::Snap,
            "Alinhar a bordas, cursor e quadros (F8; segure Alt ao arrastar para ignorar)",
            state.snapping,
        )
        .clicked()
        {
            state.snapping = !state.snapping;
        }
        ui.separator();
        if icons::icon_button(
            ui,
            Icon::Crossfade,
            "Crossfade automático entre eventos sobrepostos",
            state.project.settings.auto_crossfade,
        )
        .clicked()
        {
            state.project.settings.auto_crossfade = !state.project.settings.auto_crossfade;
            state.forced_dirty = true;
        }
        // Toolbar zoom is anchored at the edit cursor
        for (which, tip, factor) in [
            (Icon::ZoomOut, "Reduzir zoom", 0.5),
            (Icon::ZoomIn, "Ampliar zoom", 2.0),
        ] {
            if icon(ui, which, tip) {
                let offset = state.playhead - state.scroll_seconds;
                state.pixels_per_second = (state.pixels_per_second * factor)
                    .clamp(MIN_PIXELS_PER_SECOND, MAX_PIXELS_PER_SECOND);
                state.scroll_seconds = (state.playhead - offset * (1.0 / factor)).max(0.0);
            }
        }
    });
}

fn timeline_canvas(state: &mut AppState, ui: &mut egui::Ui, canvas_h: f32) {
    // The vertical scrollbar takes its own column at the right edge
    let size = vec2(
        (ui.available_width() - timeline_scroll::BAR).max(100.0),
        canvas_h,
    );
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let ppp = ui.ctx().pixels_per_point();
    let now = ui.input(|i| i.time);
    let fps = state.project.settings.fps;
    let drop_frame = state.drop_frame();
    let lanes = Rect::from_min_max(pos2(rect.left() + HEADER_WIDTH, rect.top()), rect.max);
    state.lanes_width = lanes.width();

    // ---- zoom / scroll (zoom anchored at the pointer) ------------------------------------------
    if response.hovered() {
        let zoom = ui.input(|i| i.zoom_delta()) as f64;
        let scroll = ui.input(|i| i.smooth_scroll_delta);
        if let Some(p) = response.hover_pos() {
            let anchor = x_to_time(state, lanes.left(), p.x);
            if p.x < lanes.left() {
                // Over the track headers the wheel scrolls the rows
                if scroll != egui::Vec2::ZERO {
                    state.scroll_y = (state.scroll_y - scroll.y).max(0.0);
                }
            } else if (zoom - 1.0).abs() > 1e-6 {
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
    // The rows scroll under the ruler: every row position hangs from `lane_top`
    let max_scroll_y = timeline_scroll::max_scroll_y(
        state.timeline.tracks.len(),
        TRACK_HEIGHT,
        (rect.bottom() - ruler_bottom).max(1.0),
    );
    state.scroll_y = state.scroll_y.clamp(0.0, max_scroll_y);
    let lane_top = ruler_bottom - state.scroll_y;
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
        let on_event = response
            .interact_pointer_pos()
            .filter(|p| p.x >= lanes.left() && p.y >= ruler_bottom)
            .and_then(|p| hit_event(state, lanes.left(), lane_top, p));
        if let Some((track_id, event_id, _, _)) = on_event {
            if let Some(path) = state.subproject_path_of(track_id, event_id) {
                state.tabs.open_request = Some(path);
            }
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
        } else if let Some(env_hit) = response
            .interact_pointer_pos()
            .filter(|p| p.x >= lanes.left() && p.y >= ruler_bottom)
            .and_then(|p| {
                envelope_canvas::hit_test(
                    &state.timeline.tracks,
                    lanes.left(),
                    lane_top,
                    TRACK_HEIGHT,
                    state.scroll_seconds,
                    state.pixels_per_second,
                    p,
                )
            })
        {
            let target = FxTarget::Track(env_hit.track_id);
            let cmd = match env_hit.point_index {
                Some(idx) => envelope::move_keyframe(
                    &state.timeline,
                    target,
                    &env_hit.target,
                    idx,
                    (env_hit.time, env_hit.default),
                    env_hit.bounds,
                ),
                None => state
                    .timeline
                    .track(env_hit.track_id)
                    .and_then(|t| t.envelopes.iter().find(|e| e.target == env_hit.target))
                    .and_then(|env| {
                        let pt =
                            envelope_canvas::new_point_on_curve(env, env_hit.time, env_hit.value);
                        envelope::upsert_keyframe(
                            &state.timeline,
                            target,
                            &env_hit.target,
                            pt,
                            env_hit.bounds,
                            1e-4,
                        )
                    }),
            };
            if let Some(cmd) = cmd {
                if state.history.execute(&mut state.timeline, cmd) {
                    state.restart_audio_if_playing();
                }
            }
        }
    }

    // ---- the buttons of an event header act on a click, without moving the cursor or a drag -----
    let header_click = if response.clicked() {
        response
            .interact_pointer_pos()
            .and_then(|p| header_button_at(state, lanes, lane_top, p))
    } else {
        None
    };
    if let Some((track_id, event_id, button, rect)) = header_click {
        state.header_button_clicked(track_id, event_id, button, rect.center());
    }

    // ---- clicks: ruler scrubs the playhead, lanes and headers select tracks/events -------------
    if header_click.is_none() && (response.clicked() || response.dragged()) {
        if let Some(p) = response.interact_pointer_pos() {
            let in_ruler = p.y < rect.top() + RULER_HEIGHT;
            let row = ((p.y - lane_top) / TRACK_HEIGHT).floor();
            let track_id = (row >= 0.0 && !in_ruler)
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
                        Some(event) if modifiers.command => state.click_toggle(event),
                        Some(event) if modifiers.shift => {
                            state.click_extend(event);
                        }
                        Some(event) => {
                            state.click_select(event);
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
                let row_idx = row as usize;
                if let Some(track) = state.timeline.tracks.get(row_idx).cloned() {
                    let y = lane_top + row_idx as f32 * TRACK_HEIGHT;
                    let row_rect = Rect::from_min_max(
                        pos2(rect.left(), y),
                        pos2(lanes.left(), y + TRACK_HEIGHT),
                    );
                    if let Some(ctrl) = hit_header_control(row_rect, track.kind, p) {
                        let exclusive = ui.input(|i| i.modifiers.command || i.modifiers.shift);
                        let action = match ctrl {
                            HeaderControl::Mute if exclusive => MixAction::MuteExclusive,
                            HeaderControl::Mute => MixAction::Mute(!track.muted),
                            HeaderControl::Solo if exclusive => MixAction::SoloExclusive,
                            HeaderControl::Solo => MixAction::Solo(!track.solo),
                            HeaderControl::Phase => MixAction::Phase(!track.invert_phase),
                        };
                        state.apply_mix_actions(track.id, vec![action]);
                    }
                }
            }
        }
    }

    // ---- dragging events: move, trim left/right, change track ------------------------------------
    if response.drag_started() {
        let press = ui.input(|i| i.pointer.press_origin());
        if let Some(p) = press.filter(|p| {
            p.x >= lanes.left()
                && p.y >= ruler_bottom
                && header_button_at(state, lanes, lane_top, *p).is_none()
        }) {
            let t = x_to_time(state, lanes.left(), p.x);
            let env_node = envelope_canvas::hit_test(
                &state.timeline.tracks,
                lanes.left(),
                lane_top,
                TRACK_HEIGHT,
                state.scroll_seconds,
                state.pixels_per_second,
                p,
            )
            .filter(|h| h.point_index.is_some());
            if let Some(h) = env_node {
                state.selected_track = Some(h.track_id);
                state.env_drag = Some(EnvelopeDrag {
                    track_id: h.track_id,
                    target: h.target,
                    index: h.point_index.unwrap_or(0),
                    time: h.time,
                    value: h.value,
                    bounds: h.bounds,
                });
            } else if let Some(mut hit) = hit_event(state, lanes.left(), lane_top, p) {
                if ui.input(|i| i.modifiers.command) {
                    let adjacent = ui.input(|i| i.modifiers.alt);
                    hit.2 = match (hit.2, adjacent) {
                        (DragMode::TrimLeft, true) => DragMode::RollLeft,
                        (DragMode::TrimRight, true) => DragMode::RollRight,
                        (DragMode::TrimLeft, false) => DragMode::StretchLeft,
                        (DragMode::TrimRight, false) => DragMode::StretchRight,
                        (other, _) => other,
                    };
                }
                state.drag_ignores_group = ui.input(|i| i.modifiers.shift);
                state.begin_drag(hit, t);
            } else if ui.input(|i| i.modifiers.shift) {
                // Shift + drag over empty lane: rectangular selection
                let row = (p.y - lane_top) / TRACK_HEIGHT;
                state.marquee = Some(Marquee {
                    anchor_time: t.max(0.0),
                    anchor_row: row,
                    current_time: t.max(0.0),
                    current_row: row,
                    additive: ui.input(|i| i.modifiers.command),
                });
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
            let (shift, alt) = ui.input(|i| (i.modifiers.shift, i.modifiers.alt));
            let snap_enabled = effective_snap(
                state.snapping,
                state.drag.as_ref().map(|d| d.mode),
                shift,
                alt,
            );
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
    if state.marquee.is_some() {
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            state.marquee = None;
        } else if let Some(p) = response.interact_pointer_pos() {
            let t = x_to_time(state, lanes.left(), p.x).max(0.0);
            if let Some(m) = &mut state.marquee {
                m.current_time = t;
                m.current_row = (p.y - lane_top) / TRACK_HEIGHT;
            }
            ui.ctx().request_repaint();
        }
    }
    if let (Some(anchor), Some(p)) = (state.range_anchor, response.interact_pointer_pos()) {
        let t = edit::quantize(x_to_time(state, lanes.left(), p.x).max(0.0), fps);
        let range = TimeRange::new(anchor, t);
        state.time_selection = (!range.is_empty()).then_some(range);
        ui.ctx().request_repaint();
    }
    if state.env_drag.is_some() {
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            state.env_drag = None;
        } else if let Some(p) = response.interact_pointer_pos() {
            let t = x_to_time(state, lanes.left(), p.x).max(0.0);
            if let Some(d) = &mut state.env_drag {
                if let Some(row) = state
                    .timeline
                    .tracks
                    .iter()
                    .position(|tr| tr.id == d.track_id)
                {
                    let y0 = lane_top + row as f32 * TRACK_HEIGHT;
                    let sensitivity = if ui.input(|i| i.modifiers.command) {
                        0.2
                    } else {
                        1.0
                    };
                    let raw_v =
                        envelope_canvas::y_to_value(y0, TRACK_HEIGHT, p.y, d.bounds.0, d.bounds.1);
                    d.time = t;
                    d.value = if sensitivity < 1.0 {
                        (d.value + (raw_v - d.value) * sensitivity).clamp(d.bounds.0, d.bounds.1)
                    } else {
                        raw_v
                    };
                }
            }
            ui.ctx().request_repaint();
        }
    }
    if response.drag_stopped() {
        state.finish_envelope_drag();
        state.finish_mark_drag();
        state.finish_marquee();
        state.finish_drag();
        state.range_anchor = None;
    }

    // Right-click on an envelope, an event or a ruler mark
    if response.secondary_clicked() {
        let pressed = response
            .interact_pointer_pos()
            .filter(|p| p.x >= lanes.left() && p.y >= ruler_bottom);
        state.context_envelope = pressed.and_then(|p| {
            envelope_canvas::hit_test(
                &state.timeline.tracks,
                lanes.left(),
                lane_top,
                TRACK_HEIGHT,
                state.scroll_seconds,
                state.pixels_per_second,
                p,
            )
        });
        state.context_event = if state.context_envelope.is_some() {
            None
        } else {
            pressed
                .and_then(|p| hit_event(state, lanes.left(), lane_top, p))
                .map(|h| (h.0, h.1))
        };
        state.context_mark = None;
        if let Some(env_hit) = &state.context_envelope {
            state.selected_track = Some(env_hit.track_id);
        } else if let Some((track_id, event_id)) = state.context_event {
            state.selected_track = Some(track_id);
            if !state.selection.contains((track_id, event_id)) {
                state.click_select((track_id, event_id));
            }
        }
    }
    if response.secondary_clicked() {
        let in_ruler = response
            .interact_pointer_pos()
            .filter(|p| p.x >= lanes.left() && p.y < ruler_bottom);
        state.context_mark = in_ruler.and_then(|p| {
            let tolerance = f64::from(MARK_HIT_PX) / state.pixels_per_second;
            state.mark_at(x_to_time(state, lanes.left(), p.x), tolerance)
        });
    }
    response.context_menu(|ui| {
        if let Some(env_hit) = state.context_envelope.clone() {
            envelope_context_menu(state, ui, &env_hit);
            return;
        }
        match (state.context_event, state.context_mark) {
            (Some((track_id, event_id)), _) => event_context_menu(state, ui, track_id, event_id),
            (None, Some(id)) => {
                if ui.button(tr("Renomear")).clicked() {
                    state.begin_name_edit(NameTarget::Existing(id));
                    ui.close_menu();
                }
                if ui.button(tr("Remover marcador ou região")).clicked() {
                    state.remove_mark(id);
                    ui.close_menu();
                }
                if ui
                    .button(tr("Apagar todos os marcadores e regiões"))
                    .clicked()
                {
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
                        egui::Button::new(tr("Apagar marcadores e regiões")),
                    )
                    .on_hover_text(tr("Na seleção de tempo; sem ela, todos"))
                    .clicked()
                {
                    state.clear_marks();
                    ui.close_menu();
                }
            }
        }
    });

    // Cursor feedback
    if state.env_drag.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    } else if let Some(drag) = &state.drag {
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
    } else if let Some(p) = response
        .hover_pos()
        .filter(|p| p.x >= lanes.left() && p.y >= ruler_bottom)
    {
        let header_hover = header_button_at(state, lanes, lane_top, p);
        if let Some((track_id, _, button, _)) = header_hover {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            let video = state
                .timeline
                .track(track_id)
                .is_some_and(|t| t.kind == TrackKind::Video);
            response.clone().on_hover_text(button.tooltip(video));
        }
        match hit_event(state, lanes.left(), lane_top, p)
            .map(|h| h.2)
            .filter(|_| header_hover.is_none())
        {
            Some(
                DragMode::TrimLeft
                | DragMode::TrimRight
                | DragMode::StretchLeft
                | DragMode::StretchRight
                | DragMode::FadeIn
                | DragMode::FadeOut,
            ) => {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
            Some(DragMode::Move) => {
                // The body of an event: the slip and slide tools show the sideways arrows
                let icon = if state.edit_tool == EditTool::Normal {
                    egui::CursorIcon::Grab
                } else {
                    egui::CursorIcon::ResizeHorizontal
                };
                ui.ctx().set_cursor_icon(icon);
            }
            Some(
                DragMode::Slip
                | DragMode::Slide
                | DragMode::Shuffle
                | DragMode::RollLeft
                | DragMode::RollRight,
            )
            | None => {}
        }
    }

    // ---- drawing ---------------------------------------------------------------------------------
    let painter = ui.painter_at(rect);
    // The ruler keeps the full lane width; the rows are clipped below it so they scroll under it
    let ruler_painter = painter.with_clip_rect(lanes);
    let lane_painter = painter.with_clip_rect(Rect::from_min_max(
        pos2(lanes.left(), ruler_bottom),
        lanes.max,
    ));
    let pal = ui_theme::palette();
    painter.rect_filled(rect, Rounding::ZERO, pal.timeline_bg);

    for (i, _) in state.timeline.tracks.iter().enumerate() {
        let y = lane_top + i as f32 * TRACK_HEIGHT;
        lane_painter.rect_filled(
            Rect::from_min_max(pos2(lanes.left(), y), pos2(lanes.right(), y + TRACK_HEIGHT)),
            Rounding::ZERO,
            if i % 2 == 0 { pal.lane } else { pal.lane_alt },
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
    let minor = Stroke::new(hairline, pal.grid_minor);
    let major = Stroke::new(hairline, pal.grid_major);
    for &t in &grid.minor_times {
        let x = to_x(t);
        lane_painter.line_segment([pos2(x, lane_top), pos2(x, rect.bottom())], minor);
    }
    for &t in &grid.major_times {
        let x = to_x(t);
        ruler_painter.line_segment(
            [
                pos2(x, rect.top() + RULER_HEIGHT - 6.0),
                pos2(x, rect.bottom()),
            ],
            major,
        );
        ruler_painter.text(
            pos2(x + 3.0, rect.top() + 3.0),
            Align2::LEFT_TOP,
            seconds_to_timecode(t, fps, drop_frame),
            FontId::monospace(10.0),
            ui.visuals().weak_text_color(),
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
            ruler_painter.rect_filled(
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
        ruler_painter.rect_filled(
            band,
            Rounding::ZERO,
            Color32::from_rgba_unmultiplied(0x2E, 0xCC, 0x71, 70),
        );
        let edge = Stroke::new(hairline, Color32::from_rgb(0x2E, 0xCC, 0x71));
        for x in [x0, x1] {
            ruler_painter.line_segment([pos2(x, strip_top), pos2(x, rect.bottom())], edge);
        }
        ruler_painter.text(
            pos2(x0.max(lanes.left()) + 3.0, strip_top),
            Align2::LEFT_TOP,
            &region.name,
            FontId::proportional(9.0),
            Color32::from_gray(235),
        );
    }
    // Notes of the project: a square on the ruler, hollow once resolved
    for note in &state.project.notes {
        let x = to_x(note.time);
        if x < lanes.left() - 1.0 || x > lanes.right() + 1.0 {
            continue;
        }
        let [r, g, b] = note.rgb();
        let color = Color32::from_rgb(r, g, b);
        let square = Rect::from_min_size(
            pos2(x - 3.0, rect.top() + RULER_HEIGHT - MARK_STRIP_HEIGHT - 3.0),
            vec2(6.0, 6.0),
        );
        if note.resolved {
            ruler_painter.rect_stroke(square, Rounding::ZERO, Stroke::new(1.0_f32, color));
        } else {
            ruler_painter.rect_filled(square, Rounding::ZERO, color);
        }
    }
    for marker in &state.timeline.markers {
        let x = to_x(state.shown_span(marker.id, (marker.time, marker.time)).0);
        if x < lanes.left() - 1.0 || x > lanes.right() + 1.0 {
            continue;
        }
        let color = Color32::from_rgb(0xF1, 0xC4, 0x0F);
        ruler_painter.line_segment(
            [pos2(x, strip_top), pos2(x, rect.bottom())],
            Stroke::new(hairline, color.gamma_multiply(0.6)),
        );
        ruler_painter.add(Shape::convex_polygon(
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
        ruler_painter.text(
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
                .hint_text(tr("Nome"))
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
            TrackKind::Audio => pal.audio_clip,
            TrackKind::Video => pal.video_clip,
        };
        let base = if state.timeline.audible(track) {
            base
        } else {
            base.gamma_multiply(0.45)
        };
        for ev in &track.events {
            let dragging = state
                .drag
                .as_ref()
                .filter(|d| d.track_id == track.id && d.event_id == ev.id);
            let shifted = state
                .drag
                .as_ref()
                .filter(|d| {
                    d.moving.len() > 1
                        && !(d.track_id == track.id && d.event_id == ev.id)
                        && d.moving.contains(&(track.id, ev.id))
                })
                .map(|d| d.current.start - d.origin.start);
            let partner_geometry = state
                .drag
                .as_ref()
                .filter(|d| d.track_id == track.id)
                .and_then(|d| d.partner.as_ref())
                .filter(|p| p.event_id == ev.id)
                .map(|p| p.current);
            let g = match (partner_geometry, dragging, shifted) {
                (Some(partner), _, _) => partner,
                (None, Some(d), _) => d.current,
                (None, None, Some(delta)) => EventGeometry {
                    start: ev.start + delta,
                    ..ev.geometry()
                },
                _ => ev.geometry(),
            };
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
            if ev.adjustment {
                // Diagonal hatching marks an adjustment event, drawn only over the visible part
                let shown = ev_rect.intersect(lane_painter.clip_rect());
                let hatch = lane_painter.with_clip_rect(shown);
                let step = 10.0;
                let h = ev_rect.height();
                let mut x = (shown.left() - h).max(ev_rect.left() - h);
                while x < shown.right() {
                    hatch.line_segment(
                        [pos2(x, ev_rect.bottom()), pos2(x + h, ev_rect.top())],
                        Stroke::new(1.0_f32, Color32::from_white_alpha(40)),
                    );
                    x += step;
                }
            }
            lane_painter.rect_filled(
                Rect::from_min_size(ev_rect.min, vec2(ev_rect.width(), EVENT_HEADER_HEIGHT)),
                Rounding::ZERO,
                Color32::from_black_alpha(90),
            );
            let rate = edit::clamp_playback_rate(g.playback_rate);
            let mut title = if (rate - 1.0).abs() > 1e-3 {
                format!("{} [{rate:.2}×]", ev.name)
            } else {
                ev.name.clone()
            };
            if ev.looping {
                title.push_str(" [laço]");
            }
            if let Some(tag) = ev.channels.tag() {
                title.push_str(&format!(" [{tag}]"));
            }
            if ev.normalize {
                title.push_str(&tr_args(
                    " [norm {} dB]",
                    &[&format!("{:+.1}", 20.0 * ev.normalize_gain.log10())],
                ));
            }
            // The header: the name up to the first button, then the buttons
            let buttons =
                event_header::buttons_for(track.kind, state.is_generated_event(ev), ev.adjustment);
            let placed = event_header::layout(ev_rect, &buttons, EVENT_HEADER_HEIGHT);
            let name_right = event_header::name_limit(ev_rect, &placed).max(ev_rect.left());
            let title_painter = lane_painter.with_clip_rect(
                Rect::from_min_max(
                    ev_rect.min,
                    pos2(name_right, ev_rect.top() + EVENT_HEADER_HEIGHT + 1.0),
                )
                .intersect(lane_painter.clip_rect()),
            );
            title_painter.text(
                ev_rect.min + vec2(4.0, 1.0),
                Align2::LEFT_TOP,
                &title,
                FontId::proportional(10.0),
                Color32::from_gray(225),
            );
            let hover = ui.input(|i| i.pointer.hover_pos());
            for (button, rect) in &placed {
                // Only a button that is wholly in view is drawn (and answers)
                if rect.left() < lanes.left() || rect.right() > lanes.right() {
                    continue;
                }
                let over = hover.is_some_and(|p| rect.expand(1.0).contains(p));
                if over {
                    lane_painter.rect_filled(*rect, Rounding::ZERO, Color32::from_white_alpha(50));
                }
                let color = Color32::from_gray(if over { 255 } else { 215 });
                match button {
                    event_header::HeaderButton::Fx => {
                        icons::paint_svg(ui, icons::SvgIcon::EventFx, rect.shrink(1.0), color);
                    }
                    event_header::HeaderButton::PanCrop => {
                        icons::paint_svg(ui, icons::SvgIcon::PanCrop, rect.shrink(1.0), color);
                    }
                    event_header::HeaderButton::Menu => {
                        for line in 0..3 {
                            let y = rect.top() + 3.0 + line as f32 * 3.0;
                            lane_painter.line_segment(
                                [pos2(rect.left() + 2.0, y), pos2(rect.right() - 2.0, y)],
                                Stroke::new(1.0_f32, color),
                            );
                        }
                    }
                    event_header::HeaderButton::GeneratedMedia => {
                        let c = rect.center();
                        lane_painter.add(egui::Shape::convex_polygon(
                            vec![
                                pos2(c.x, c.y - 5.0),
                                pos2(c.x + 5.0, c.y),
                                pos2(c.x, c.y + 5.0),
                                pos2(c.x - 5.0, c.y),
                            ],
                            color,
                            Stroke::NONE,
                        ));
                    }
                }
            }
            // The media this event plays is not on disk (manual v21 p.143: the event says so)
            if ev
                .media_id
                .is_some_and(|m| state.missing.iter().any(|x| x.id == m))
            {
                lane_painter.text(
                    ev_rect.center() + vec2(0.0, EVENT_HEADER_HEIGHT / 2.0),
                    Align2::CENTER_CENTER,
                    "MÍDIA OFFLINE",
                    FontId::proportional(10.0),
                    Color32::from_rgb(0xE7, 0x4C, 0x3C),
                );
            }
            if let Some(edge) = state
                .trim_edge
                .filter(|e| e.event == ev.id && e.track == track.id)
            {
                // Keyboard trim mode: a red bar marks the selected edge
                let x = match edge.side {
                    neoveg_engine::keytrim::EdgeSide::Start => ev_rect.left(),
                    neoveg_engine::keytrim::EdgeSide::End => ev_rect.right(),
                };
                lane_painter.line_segment(
                    [pos2(x, ev_rect.top()), pos2(x, ev_rect.bottom())],
                    Stroke::new(3.0_f32, Color32::from_rgb(0xE7, 0x4C, 0x3C)),
                );
            }
            if ev.group.is_some() {
                // Orange corner: the event belongs to a group
                lane_painter.add(Shape::convex_polygon(
                    vec![
                        pos2(ev_rect.right() - 9.0, ev_rect.top()),
                        pos2(ev_rect.right(), ev_rect.top()),
                        pos2(ev_rect.right(), ev_rect.top() + 9.0),
                    ],
                    Color32::from_rgb(0xE6, 0x7E, 0x22),
                    Stroke::NONE,
                ));
            }

            if let Some(media) = ev.media_id.and_then(|id| state.media.get(&id)) {
                let wave_rect = Rect::from_min_max(
                    pos2(
                        ev_rect.left().max(lanes.left()),
                        ev_rect.top() + EVENT_HEADER_HEIGHT,
                    ),
                    pos2(ev_rect.right().min(lanes.right()), ev_rect.bottom()),
                );
                let visible_offset = (wave_rect.left() - x0) as f64 / state.pixels_per_second;
                let start_frame = (g.trim_in + visible_offset * rate) * media.sample_rate as f64;
                let frames_per_pixel = media.sample_rate as f64 * rate / state.pixels_per_second;
                paint_waveform(
                    &lane_painter,
                    wave_rect,
                    &media.waveform,
                    start_frame,
                    frames_per_pixel,
                    Color32::from_rgb(150, 215, 245),
                );
            }
            if ev.looping {
                // A tick at every repeat of the source
                let source = state.source_len(track.id, ev.id).unwrap_or(0.0);
                let each = edit::loop_length(source, ev.trim_in);
                let spacing = (each * state.pixels_per_second) as f32;
                if spacing >= 6.0 {
                    let mut at = ev.start + each;
                    while at < ev.end() - TIME_EPSILON {
                        let x = lanes.left()
                            + ((at - state.scroll_seconds) * state.pixels_per_second) as f32;
                        if x > lanes.right() {
                            break;
                        }
                        if x >= lanes.left() {
                            lane_painter.line_segment(
                                [
                                    pos2(x, ev_rect.top() + EVENT_HEADER_HEIGHT),
                                    pos2(x, ev_rect.bottom()),
                                ],
                                Stroke::new(1.0_f32, Color32::from_white_alpha(150)),
                            );
                        }
                        at += each;
                    }
                }
            }
            if ev.muted {
                // A muted event is darkened: it is out of playback
                lane_painter.rect_filled(ev_rect, Rounding::ZERO, Color32::from_black_alpha(150));
            }
            if ev.locked && x1 - x0 >= 18.0 {
                paint_lock_mark(
                    &lane_painter,
                    pos2(ev_rect.right() - 16.0, ev_rect.top() + 2.0),
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
        envelope_canvas::paint_track_envelopes(
            &lane_painter,
            lanes,
            y,
            TRACK_HEIGHT,
            state.scroll_seconds,
            state.pixels_per_second,
            track,
            state.env_drag.as_ref(),
        );
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

    if let Some(m) = &state.marquee {
        let (x0, x1) = (to_x(m.anchor_time), to_x(m.current_time));
        let (y0, y1) = (
            lane_top + m.anchor_row * TRACK_HEIGHT,
            lane_top + m.current_row * TRACK_HEIGHT,
        );
        let area = Rect::from_two_pos(pos2(x0, y0), pos2(x1, y1)).intersect(lanes);
        lane_painter.rect_filled(
            area,
            Rounding::ZERO,
            Color32::from_rgba_unmultiplied(0xF3, 0x9C, 0x12, 40),
        );
        lane_painter.rect_stroke(
            area,
            Rounding::ZERO,
            Stroke::new(hairline, Color32::from_rgb(0xF3, 0x9C, 0x12)),
        );
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
        pos2(rect.left(), ruler_bottom),
        pos2(lanes.left(), rect.bottom()),
    ));
    header_painter.rect_filled(
        Rect::from_min_max(rect.min, pos2(lanes.left(), rect.bottom())),
        Rounding::ZERO,
        pal.header,
    );
    // Only the selected track's number-box dot blinks (white/black); the others stay static
    let blink_on = ((now / BLINK_SECONDS) as i64) % 2 == 0;
    for (i, track) in state.timeline.tracks.iter().enumerate() {
        let y = lane_top + i as f32 * TRACK_HEIGHT;
        let row = Rect::from_min_max(pos2(rect.left(), y), pos2(lanes.left(), y + TRACK_HEIGHT));
        let selected = state.selected_track == Some(track.id);
        if selected {
            header_painter.rect_filled(row, Rounding::ZERO, pal.header_selected);
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
            ui.visuals().weak_text_color()
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
            ui.visuals().text_color(),
        );
        header_painter.text(
            row.min + vec2(46.0, 8.0),
            Align2::LEFT_TOP,
            &track.name,
            FontId::proportional(11.0),
            ui.visuals().text_color(),
        );

        // Per-track Mute / Solo / Phase buttons and compact dB + Pan readout
        let hover_pos = response.hover_pos();
        for (ctrl, btn_rect) in header_buttons(row, track.kind) {
            let active = match ctrl {
                HeaderControl::Mute => track.muted,
                HeaderControl::Solo => track.solo,
                HeaderControl::Phase => track.invert_phase,
            };
            let hovered = hover_pos.is_some_and(|p| btn_rect.contains(p));
            let fill = match (ctrl, active, hovered) {
                (HeaderControl::Mute, true, _) => Color32::from_rgb(0xC0, 0x39, 0x2B),
                (HeaderControl::Solo, true, _) => Color32::from_rgb(0xD3, 0x54, 0x00),
                (HeaderControl::Phase, true, _) => ui.visuals().selection.bg_fill,
                (_, false, true) => ui.visuals().widgets.hovered.bg_fill,
                _ => Color32::from_black_alpha(45),
            };
            header_painter.rect_filled(btn_rect, Rounding::ZERO, fill);
            let fg = if active {
                Color32::WHITE
            } else {
                ui.visuals().text_color()
            };
            header_painter.extend(icons::shapes(ctrl.icon(), btn_rect.shrink(3.5), fg));
        }
        if track.kind == TrackKind::Audio {
            let db_short = if track.volume_db <= neoveg_engine::TrackMix::VOLUME_RANGE_DB.0 {
                "-∞".to_string()
            } else {
                format!("{:+.1}", track.volume_db)
            };
            let pan_short = mixer_panel::format_pan(track.pan).replace(' ', "");
            header_painter.text(
                row.min + vec2(77.0, 44.0),
                Align2::LEFT_CENTER,
                format!("{db_short} {pan_short}"),
                FontId::monospace(9.5),
                ui.visuals().weak_text_color(),
            );
        }
    }
    if let Some(p) = response
        .hover_pos()
        .filter(|p| p.x < lanes.left() && p.y >= ruler_bottom)
    {
        let row_idx = ((p.y - lane_top) / TRACK_HEIGHT).floor();
        if row_idx >= 0.0 {
            if let Some(track) = state.timeline.tracks.get(row_idx as usize) {
                let y = lane_top + row_idx * TRACK_HEIGHT;
                let row =
                    Rect::from_min_max(pos2(rect.left(), y), pos2(lanes.left(), y + TRACK_HEIGHT));
                if let Some(ctrl) = hit_header_control(row, track.kind, p) {
                    response.clone().on_hover_text(ctrl.tooltip());
                }
            }
        }
    }
    if state.timeline.tracks.is_empty() {
        header_painter.text(
            rect.left_top() + vec2(8.0, RULER_HEIGHT + 8.0),
            Align2::LEFT_TOP,
            tr("Sem faixas\nCtrl+Q: áudio\nCtrl+Shift+Q: vídeo"),
            FontId::proportional(11.0),
            ui.visuals().weak_text_color(),
        );
    }

    // Vertical scrollbar: its own column beside the lanes, under the ruler
    {
        let bar = Rect::from_min_max(
            pos2(rect.right(), ruler_bottom),
            pos2(rect.right() + timeline_scroll::BAR, rect.bottom()),
        );
        let body = f64::from((rect.bottom() - ruler_bottom).max(1.0));
        let content = (state.timeline.tracks.len() as f64 * f64::from(TRACK_HEIGHT)).max(body);
        if let Some(offset) = scrollbar(
            ui,
            "timeline_vscroll",
            bar,
            false,
            content,
            body,
            f64::from(state.scroll_y),
        ) {
            state.scroll_y = (offset as f32).clamp(0.0, max_scroll_y);
        }
    }

    // Snap indicator: a line through the lanes where the drag is aligned (dashed when it only rounds)
    if let Some(mark) = state.drag.as_ref().and_then(|d| d.snap_mark) {
        let (time, dashed) = match mark {
            SnapMark::Target(t) => (t, false),
            SnapMark::Rounded(t) => (t, true),
        };
        let x = to_x(time);
        if x >= lanes.left() && x <= lanes.right() {
            let points = [pos2(x, lanes.top()), pos2(x, lanes.bottom())];
            let stroke = Stroke::new(1.0_f32, Color32::from_rgb(0xF1, 0xC4, 0x0F));
            if dashed {
                lane_painter.extend(Shape::dashed_line(&points, stroke, 4.0, 3.0));
            } else {
                lane_painter.line_segment(points, stroke);
            }
        }
    }

    // Playhead: blue pentagonal head, 1 physical-pixel guide line, caret blink while paused
    let blue = pal.accent;
    let bright = pal.accent_bright;
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
    #[test]
    fn windows_toggle_between_front_closed_and_back() {
        let mut dock = default_dock();
        // Scopes sit behind Loudness: the first press raises it, the second closes it
        assert_eq!(toggle_window(&mut dock, Tab::Scopes), WindowChange::Raised);
        assert_eq!(toggle_window(&mut dock, Tab::Scopes), WindowChange::Closed);
        assert!(dock.find_tab(&Tab::Scopes).is_none());
        assert_eq!(toggle_window(&mut dock, Tab::Scopes), WindowChange::Opened);
        assert!(dock.find_tab(&Tab::Scopes).is_some());
        // Every window of the default layout can be found
        for tab in [
            Tab::Preview,
            Tab::Timeline,
            Tab::Media,
            Tab::MasterBus,
            Tab::Loudness,
            Tab::AudioFx,
            Tab::VideoFx,
            Tab::Transitions,
        ] {
            assert!(default_dock().find_tab(&tab).is_some(), "{tab:?}");
        }
    }

    #[test]
    fn a_saved_layout_survives_closing_windows() {
        let mut dock = default_dock();
        let saved = dock.clone();
        toggle_window(&mut dock, Tab::MasterBus);
        assert!(dock.find_tab(&Tab::MasterBus).is_none());
        dock = saved;
        assert!(dock.find_tab(&Tab::MasterBus).is_some());
    }

    use super::*;

    fn state_with_clip() -> (AppState, TrackId, EventId) {
        let mut s = AppState::new();
        s.add_track(TrackKind::Audio);
        let track_id = s.selected_track.unwrap();
        s.project.media_pool.push(MediaRef {
            subproject: false,
            format: None,
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
            ..Default::default()
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
    fn a_looping_event_can_be_extended_past_the_source_and_plays_it_again() {
        let (mut s, track, ev) = state_with_clip();
        s.media.insert(1, test_media());
        s.selection.set([(track, ev)]);
        run(&mut s, Command::EditSwitchLoop);

        s.begin_drag(hit(&s, track, ev, DragMode::TrimRight), 6.0);
        s.update_drag(500.0, None, 0.1, false);
        s.finish_drag();
        let g = geometry(&s, track, ev);
        assert!(
            g.duration > 10.0,
            "Loop lifts the end-of-source limit: {}",
            g.duration
        );

        let clip = s.build_clips().into_iter().next().unwrap();
        assert_eq!(
            clip.loop_len, 10.0,
            "the whole 10 s source repeats from the in point"
        );

        run(&mut s, Command::EditSwitchLoop);
        let clip = s.build_clips().into_iter().next().unwrap();
        assert_eq!(clip.loop_len, 0.0, "turned off, the source plays once");
    }

    #[test]
    fn a_generated_media_is_saved_beside_the_project_and_lands_on_a_video_track() {
        let dir = std::env::temp_dir().join(format!("neoveg_gen_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = AppState::new();
        s.project_path = Some(dir.join("projeto.neoveg"));
        let spec = neoveg_video::generators::GeneratorSpec {
            name: "Título: abertura".into(),
            width: 640,
            height: 360,
            fps: 25.0,
            duration: 4.0,
            kind: neoveg_video::generators::GeneratorKind::default_title(),
        };
        s.insert_generated(spec, &egui::Context::default());

        // The manifest is written at once, with a name that is safe on every file system
        let made: Vec<_> = std::fs::read_dir(dir.join("geradores"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(made.len(), 1);
        assert!(made[0].starts_with("T_tulo__abertura_") && made[0].ends_with(".neoveggen"));

        // The loader thread probes it and the clip is placed like any other video
        for _ in 0..200 {
            s.poll_loader();
            if !s.timeline.tracks.is_empty()
                && s.timeline.tracks.iter().any(|t| !t.events.is_empty())
            {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let track = s
            .timeline
            .tracks
            .iter()
            .find(|t| !t.events.is_empty())
            .expect("the generated media was placed");
        assert_eq!(track.kind, TrackKind::Video);
        assert!((track.events[0].duration - 4.0).abs() < 1e-9);
        assert_eq!(s.project.media_pool.len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn select_group_and_remove_from_group_work_on_the_selected_events() {
        let (mut s, [a, b, c]) = state_with_three_events();
        s.selection.set([a, b, c]);
        run(&mut s, Command::EditGroup);
        assert_eq!(groups::group_members(&s.timeline, a).len(), 3);

        // Selecting one event and asking for its group selects all three
        s.selection.set([a]);
        run(&mut s, Command::EditSelectGroup);
        assert_eq!(s.selection.len(), 3);

        // `U` takes only the selected event out; the others stay together
        s.selection.set([a]);
        run(&mut s, Command::EditRemoveFromGroup);
        assert_eq!(groups::group_members(&s.timeline, a), vec![a]);
        assert_eq!(groups::group_members(&s.timeline, b).len(), 2);
        run(&mut s, Command::EditUndo);
        assert_eq!(groups::group_members(&s.timeline, a).len(), 3);

        // `Ctrl+U` dissolves the whole group
        run(&mut s, Command::EditUngroup);
        assert_eq!(groups::group_members(&s.timeline, b), vec![b]);
    }

    #[test]
    fn the_track_focus_keys_walk_the_tracks_and_stop_at_the_ends() {
        let mut s = AppState::new();
        s.add_track(TrackKind::Audio);
        s.add_track(TrackKind::Video);
        s.add_track(TrackKind::Audio);
        let ids: Vec<TrackId> = s.timeline.tracks.iter().map(|t| t.id).collect();
        s.selected_track = None;
        run(&mut s, Command::TrackFocusNext);
        assert_eq!(
            s.selected_track,
            Some(ids[0]),
            "nothing focused: Down starts at the top"
        );
        run(&mut s, Command::TrackFocusNext);
        run(&mut s, Command::TrackFocusNext);
        run(&mut s, Command::TrackFocusNext);
        assert_eq!(s.selected_track, Some(ids[2]), "it stops at the last track");
        run(&mut s, Command::TrackFocusPrevious);
        assert_eq!(s.selected_track, Some(ids[1]));
        s.selected_track = None;
        run(&mut s, Command::TrackFocusPrevious);
        assert_eq!(
            s.selected_track,
            Some(ids[2]),
            "nothing focused: Up starts at the bottom"
        );
    }

    #[test]
    fn insert_time_opens_a_gap_at_the_cursor_as_one_undo_step() {
        let (mut s, [a, b, c]) = state_with_three_events();
        // The events start at 2, 10 and 14 seconds
        s.playhead = 5.0;
        let revision = s.history.revision();
        s.insert_time_at_cursor(3.0);
        assert_eq!(s.history.revision(), revision + 1);
        assert_eq!(
            geometry(&s, a.0, a.1).start,
            2.0,
            "an event before the cursor stays"
        );
        assert_eq!(geometry(&s, b.0, b.1).start, 13.0);
        assert_eq!(geometry(&s, c.0, c.1).start, 17.0);
        run(&mut s, Command::EditUndo);
        assert_eq!(geometry(&s, b.0, b.1).start, 10.0);
        // A length of zero is refused with a message and changes nothing
        let revision = s.history.revision();
        s.insert_time_at_cursor(0.0);
        assert_eq!(s.history.revision(), revision);
        assert!(s.status.contains("maior que zero"));
        // The command opens the dialog
        run(&mut s, Command::InsertTime);
        assert!(s.insert_time_dialog.is_some());
    }

    #[test]
    fn incremental_versions_count_up_from_the_first_free_number() {
        use std::path::{Path, PathBuf};
        let taken: Vec<PathBuf> = ["a/filme_0001.neoveg", "a/filme_0002.neoveg"]
            .iter()
            .map(PathBuf::from)
            .collect();
        let exists = |p: &Path| taken.iter().any(|t| t == p);
        assert_eq!(
            incremental_path(Path::new("a/filme.neoveg"), &exists),
            PathBuf::from("a/filme_0003.neoveg"),
            "0001 and 0002 are taken"
        );
        // A name that already ends in a number counts on from it
        assert_eq!(
            incremental_path(Path::new("a/filme_0007.neoveg"), &|_| false),
            PathBuf::from("a/filme_0008.neoveg")
        );
        assert_eq!(
            incremental_path(Path::new("a/cena_final.neoveg"), &|_| false),
            PathBuf::from("a/cena_final_0001.neoveg"),
            "a trailing word is not a number"
        );
    }

    #[test]
    fn a_still_or_generated_clip_can_be_dragged_out_past_its_first_length() {
        let mut s = AppState::new();
        s.add_track(TrackKind::Video);
        let track = s.selected_track.unwrap();
        for (id, path) in [(1, "cor.neoveggen"), (2, "foto.png"), (3, "filme.mp4")] {
            s.project.media_pool.push(MediaRef {
                subproject: false,
                format: None,
                id,
                path: path.into(),
                duration_seconds: 4.0,
            });
            let event = TrackEvent {
                id: s.timeline.alloc_id(),
                media_id: Some(id),
                start: id as f64 * 10.0,
                duration: 4.0,
                ..Default::default()
            };
            s.history.execute(
                &mut s.timeline,
                EditCommand::AddEvent {
                    track_id: track,
                    event,
                },
            );
        }
        let events: Vec<EventId> = s
            .timeline
            .track(track)
            .unwrap()
            .events
            .iter()
            .map(|e| e.id)
            .collect();
        assert_eq!(
            s.source_len(track, events[0]),
            None,
            "a generator has no end"
        );
        assert_eq!(s.source_len(track, events[1]), None, "neither has a still");
        assert_eq!(s.source_len(track, events[2]), Some(4.0), "a video does");
    }

    #[test]
    fn channel_modes_reach_the_mixer_and_undo() {
        use neoveg_common::channels::ChannelMode;
        let (mut s, track, ev) = state_with_clip();
        // A stereo source whose left channel is loud and whose right channel is silent
        let samples: Vec<f32> = (0..1000).flat_map(|_| [0.5_f32, 0.0]).collect();
        let mut media = LoadedMedia {
            path: PathBuf::from("x.wav"),
            name: "x.wav".into(),
            waveform: Arc::new(WaveformSource::from_interleaved_f32(100, 2, &samples).unwrap()),
            stereo: PcmStore::shared(samples),
            sample_rate: 100,
            channels: 2,
            duration: 10.0,
        };
        media.duration = 10.0;
        s.media.insert(1, media);
        s.selection.set([(track, ev)]);

        let first = |s: &AppState| s.build_clips().into_iter().next().unwrap();
        assert_eq!(first(&s).sample_at(3.0), (0.5, 0.0));
        s.set_event_channels(ChannelMode::Right);
        assert_eq!(
            first(&s).sample_at(3.0),
            (0.0, 0.0),
            "the right channel is silent"
        );
        s.set_event_channels(ChannelMode::Swap);
        assert_eq!(first(&s).sample_at(3.0), (0.0, 0.5));
        s.set_event_channels(ChannelMode::Combine);
        assert_eq!(first(&s).sample_at(3.0), (0.25, 0.25));
        run(&mut s, Command::EditUndo);
        assert_eq!(first(&s).sample_at(3.0), (0.0, 0.5), "back to Swap");
    }

    #[test]
    fn both_window_layouts_hold_every_panel() {
        for dock in [default_dock(), audio_dock()] {
            for tab in [
                Tab::Preview,
                Tab::Timeline,
                Tab::Media,
                Tab::MasterBus,
                Tab::Loudness,
                Tab::AudioFx,
                Tab::VideoFx,
                Tab::Transitions,
                Tab::Scopes,
            ] {
                assert!(dock.find_tab(&tab).is_some(), "{tab:?} is missing");
            }
        }
    }

    #[test]
    fn project_settings_from_the_properties_window_mark_the_project_changed() {
        use crate::project_dialog::ProjectAction;
        let mut s = AppState::new();
        assert!(!s.is_dirty());
        let mut settings = s.project.settings.clone();
        settings.width = 3840;
        settings.height = 2160;
        settings.fps = 25.0;
        s.run_project_actions(vec![ProjectAction::Apply {
            settings: Box::new(settings.clone()),
            start_all_new: false,
            close: true,
        }]);
        assert_eq!(
            (s.project.settings.width, s.project.settings.fps),
            (3840, 25.0)
        );
        assert!(
            s.is_dirty(),
            "the settings changed, so the project must be saved"
        );
        assert!(s.prefs.new_project_settings.is_none());

        // "Start all new projects with these settings" keeps them for the next project
        s.run_project_actions(vec![ProjectAction::Apply {
            settings: Box::new(settings.clone()),
            start_all_new: true,
            close: true,
        }]);
        assert_eq!(s.prefs.new_project_settings, Some(settings));
        s.new_tab();
        assert_eq!(
            s.project.settings.width, 3840,
            "a new project starts with them"
        );
        s.run_project_actions(vec![ProjectAction::Apply {
            settings: Box::new(s.project.settings.clone()),
            start_all_new: false,
            close: true,
        }]);
        assert!(
            s.prefs.new_project_settings.is_none(),
            "unchecking forgets them"
        );
    }

    #[test]
    fn user_templates_are_saved_and_replaced_by_name() {
        use crate::project_dialog::ProjectAction;
        use neoveg_common::presets::ProjectTemplate;
        let mut s = AppState::new();
        let make = |width| ProjectTemplate {
            name: "Meu formato".into(),
            settings: ProjectSettings {
                width,
                ..ProjectSettings::default()
            },
        };
        s.run_project_actions(vec![ProjectAction::SaveTemplate(make(1280))]);
        s.run_project_actions(vec![ProjectAction::SaveTemplate(make(1920))]);
        assert_eq!(s.prefs.custom_templates.len(), 1);
        assert_eq!(s.prefs.custom_templates[0].settings.width, 1920);
    }

    #[test]
    fn a_new_project_from_the_welcome_screen_gets_the_chosen_settings_in_its_own_tab() {
        let mut s = AppState::new();
        s.add_track(TrackKind::Audio);
        let settings = ProjectSettings {
            width: 1080,
            height: 1920,
            fps: 30.0,
            ..ProjectSettings::default()
        };
        s.new_project_with(settings);
        assert_eq!(
            (s.project.settings.width, s.project.settings.height),
            (1080, 1920)
        );
        assert!(s.timeline.tracks.is_empty(), "it is a project of its own");
        assert_eq!(s.tab_ids().len(), 2);
    }

    #[test]
    fn recent_projects_open_into_a_blank_project_and_missing_ones_are_forgotten() {
        let dir = std::env::temp_dir().join(format!("neoveg_recent_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("antigo.neoveg");
        let mut source = AppState::new();
        source.project.settings.width = 1280;
        source.project.settings.height = 720;
        project_file::save(&file, &source.project, &source.timeline).unwrap();

        let ctx = egui::Context::default();
        let mut s = AppState::new();
        assert!(s.is_blank_project());
        s.open_recent(&file, &ctx);
        assert_eq!(s.project_path.as_deref(), Some(file.as_path()));
        assert_eq!(s.project.settings.width, 1280);
        assert_eq!(
            s.prefs.recent_projects.len(),
            1,
            "opening puts it on the list"
        );
        assert!(!s.is_blank_project());

        // With a project in progress the file goes to a tab instead of replacing the work
        let second = dir.join("outro.neoveg");
        project_file::save(&second, &source.project, &source.timeline).unwrap();
        s.open_recent(&second, &ctx);
        assert_eq!(s.tabs.open_request.as_deref(), Some(second.as_path()));

        // A file that is gone is dropped from the list
        s.prefs.add_recent_project(&dir.join("sumiu.neoveg"));
        s.open_recent(&dir.join("sumiu.neoveg"), &ctx);
        assert!(s.status.contains("não encontrado"));
        assert!(!s
            .prefs
            .recent_projects
            .iter()
            .any(|p| p.ends_with("sumiu.neoveg")));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn project_notes_are_part_of_the_project_and_the_window_actions_mark_it_changed() {
        use crate::notes_window::{apply_note_action, NoteAction};
        let mut s = AppState::new();
        s.playhead = 6.0;
        assert!(!s.is_dirty());
        let effect = apply_note_action(
            &mut s.project.notes,
            NoteAction::Add {
                color: 1,
                label: "som".into(),
            },
            s.playhead,
        );
        assert!(effect.changed);
        assert_eq!(s.project.notes[0].time, 6.0);
        // The window loop marks the project changed when an action says so
        s.forced_dirty = effect.changed;
        assert!(s.is_dirty());
        // A note is saved with the project and comes back
        let dir = std::env::temp_dir().join(format!("neoveg_notes_ui_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("n.neoveg");
        project_file::save(&file, &s.project, &s.timeline).unwrap();
        let (info, _) = project_file::load(&file).unwrap();
        assert_eq!(info.notes, s.project.notes);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_edit_tool_cycles_with_d_and_shift_d_and_can_be_chosen_directly() {
        let mut s = AppState::new();
        assert_eq!(s.edit_tool, EditTool::Normal);
        run(&mut s, Command::EditToolNext);
        assert_eq!(s.edit_tool, EditTool::Slip);
        run(&mut s, Command::EditToolNext);
        assert_eq!(s.edit_tool, EditTool::Slide);
        run(&mut s, Command::EditToolNext);
        assert_eq!(s.edit_tool, EditTool::Shuffle);
        run(&mut s, Command::EditToolNext);
        assert_eq!(s.edit_tool, EditTool::Normal, "the cycle wraps around");
        run(&mut s, Command::EditToolPrevious);
        assert_eq!(s.edit_tool, EditTool::Shuffle);
        run(&mut s, Command::EditToolNormal);
        assert_eq!(s.edit_tool, EditTool::Normal);
        assert!(s.status.contains("Ferramenta de edição"));
    }

    #[test]
    fn dropping_an_event_with_the_shuffle_tool_reorders_the_track_in_one_undo_step() {
        let mut s = AppState::new();
        let mut track = s.timeline.new_track(TrackKind::Audio);
        let mut ids = Vec::new();
        for start in [0.0, 2.0, 4.0] {
            let event = TrackEvent {
                id: s.timeline.alloc_id(),
                start,
                duration: 2.0,
                media_id: Some(1),
                ..TrackEvent::default()
            };
            ids.push(event.id);
            track.events.push(event);
        }
        let track_id = track.id;
        s.timeline.tracks.push(track);
        s.edit_tool = EditTool::Shuffle;
        let origin = s.event_geometry(track_id, ids[0]).unwrap();
        s.begin_drag((track_id, ids[0], DragMode::Move, origin), 1.0);
        assert_eq!(s.drag.as_ref().unwrap().mode, DragMode::Shuffle);
        s.update_drag(5.5, Some(track_id), 0.0, false);
        s.finish_drag();
        let order: Vec<_> = s.timeline.tracks[0].events.iter().map(|e| e.id).collect();
        assert_eq!(order, vec![ids[1], ids[2], ids[0]]);
        s.undo();
        let order: Vec<_> = s.timeline.tracks[0].events.iter().map(|e| e.id).collect();
        assert_eq!(order, ids);
    }

    fn two_touching_events() -> (AppState, TrackId, Vec<EventId>) {
        let mut s = AppState::new();
        let mut track = s.timeline.new_track(TrackKind::Audio);
        let mut ids = Vec::new();
        for (start, trim_in) in [(0.0, 0.0), (4.0, 2.0)] {
            let event = TrackEvent {
                id: s.timeline.alloc_id(),
                start,
                duration: 4.0,
                trim_in,
                media_id: Some(1),
                ..TrackEvent::default()
            };
            ids.push(event.id);
            track.events.push(event);
        }
        let id = track.id;
        s.timeline.tracks.push(track);
        (s, id, ids)
    }

    #[test]
    fn a_rolling_trim_moves_the_cut_between_two_events_in_one_undo_step() {
        let (mut s, track, ids) = two_touching_events();
        let origin = s.event_geometry(track, ids[0]).unwrap();
        s.begin_drag((track, ids[0], DragMode::RollRight, origin), 4.0);
        assert_eq!(s.drag.as_ref().unwrap().mode, DragMode::RollRight);
        s.update_drag(5.0, Some(track), 0.0, false);
        s.finish_drag();
        let events = &s.timeline.tracks[0].events;
        // The cut lands on the project's frame grid, within one frame of the pointer
        let cut = events[0].duration;
        assert!((cut - 5.0).abs() < 0.04, "cut at {cut}");
        assert_eq!(events[0].start, 0.0);
        assert!(
            (events[1].start - cut).abs() < 1e-9,
            "the events still touch"
        );
        assert!(
            (events[1].duration - (8.0 - cut)).abs() < 1e-9,
            "total length kept"
        );
        assert!(
            (events[1].trim_in - (2.0 + cut - 4.0)).abs() < 1e-9,
            "source stays aligned"
        );
        s.undo();
        let events = &s.timeline.tracks[0].events;
        assert_eq!((events[0].duration, events[1].start), (4.0, 4.0));
    }

    #[test]
    fn auto_ripple_moves_the_following_events_when_an_edge_is_trimmed() {
        let (mut s, track, ids) = two_touching_events(); // 0-4 and 4-8
        let origin = s.event_geometry(track, ids[0]).unwrap();
        s.begin_drag((track, ids[0], DragMode::TrimRight, origin), 4.0);
        s.update_drag(2.0, Some(track), 0.0, false);
        s.finish_drag();
        let b = s.timeline.tracks[0].events[1].start;
        assert!((b - 4.0).abs() < 1e-9, "without auto ripple the gap stays");
        s.undo();

        s.auto_ripple = true;
        let origin = s.event_geometry(track, ids[0]).unwrap();
        s.begin_drag((track, ids[0], DragMode::TrimRight, origin), 4.0);
        s.update_drag(2.0, Some(track), 0.0, false);
        s.finish_drag();
        let (a, b) = (
            s.timeline.tracks[0].events[0].clone(),
            s.timeline.tracks[0].events[1].clone(),
        );
        assert!(
            (a.end() - b.start).abs() < 1e-9,
            "the gap closed behind the trim"
        );
        assert!(a.duration < 4.0 && (b.duration - 4.0).abs() < 1e-9);
        s.undo();
        let events = &s.timeline.tracks[0].events;
        assert_eq!(
            (events[0].duration, events[1].start),
            (4.0, 4.0),
            "one undo step"
        );
    }

    #[test]
    fn snap_source_commands_toggle_and_decide_what_a_drag_snaps_to() {
        let (mut s, track, ids) = two_touching_events(); // 0-4 and 4-8
        s.timeline.markers.push(neoveg_engine::Marker {
            id: 1,
            time: 10.0,
            name: "m".into(),
        });
        let ctx = egui::Context::default();
        assert!(s.snap_sources.markers && !s.snap_sources.grid);
        // Dragging the second event: its start lands on the marker at 10 s when markers snap
        let origin = s.event_geometry(track, ids[1]).unwrap();
        s.begin_drag((track, ids[1], DragMode::Move, origin), 5.0);
        s.update_drag(11.02, Some(track), 0.1, true);
        assert!(
            (s.drag.as_ref().unwrap().current.start - 10.0).abs() < 1e-9,
            "snapped to the marker"
        );
        s.drag = None;

        s.run_command(Command::OptionsSnapToMarkers, &ctx, 0.0);
        assert!(!s.snap_sources.markers && s.status.contains("desligado"));
        s.begin_drag((track, ids[1], DragMode::Move, origin), 5.0);
        s.update_drag(11.02, Some(track), 0.1, true);
        let start = s.drag.as_ref().unwrap().current.start;
        assert!(
            (start - 10.0).abs() > 1e-6,
            "no marker snap now, got {start}"
        );
        s.drag = None;

        s.run_command(Command::OptionsSnapToGrid, &ctx, 0.0);
        s.run_command(Command::OptionsSnapToAllEvents, &ctx, 0.0);
        assert!(s.snap_sources.grid && !s.snap_sources.all_events);
        s.run_command(Command::OptionsQuantizeToFrames, &ctx, 0.0);
        assert!(!s.quantize_frames);
    }

    #[test]
    fn shift_inverts_snapping_only_while_moving_and_alt_always_ignores_it() {
        use DragMode::*;
        assert!(effective_snap(true, Some(Move), false, false));
        assert!(
            !effective_snap(true, Some(Move), true, false),
            "Shift turns it off while moving"
        );
        assert!(
            effective_snap(false, Some(Move), true, false),
            "and on when it is off"
        );
        assert!(
            effective_snap(true, Some(TrimRight), true, false),
            "on an edge Shift is for groups"
        );
        assert!(!effective_snap(true, Some(Move), false, true));
        assert!(
            !effective_snap(false, Some(Move), true, true),
            "Alt wins over Shift"
        );
        assert!(effective_snap(true, None, true, false));
    }

    #[test]
    fn the_drag_reports_what_it_snapped_to_or_that_it_only_rounded() {
        let (mut s, track, ids) = two_touching_events();
        s.timeline.markers.push(neoveg_engine::Marker {
            id: 1,
            time: 10.0,
            name: "m".into(),
        });
        let origin = s.event_geometry(track, ids[1]).unwrap();
        s.begin_drag((track, ids[1], DragMode::Move, origin), 5.0);
        s.update_drag(11.02, Some(track), 0.1, true);
        assert_eq!(
            s.drag.as_ref().unwrap().snap_mark,
            Some(SnapMark::Target(10.0))
        );
        // Far from every target the position is only rounded onto a frame: a dashed mark
        s.update_drag(5.0 + 2.0123, Some(track), 0.001, true);
        let mark = s.drag.as_ref().unwrap().snap_mark;
        assert!(matches!(mark, Some(SnapMark::Rounded(_))), "got {mark:?}");
        // With no snapping and no rounding left there is nothing to show
        s.quantize_frames = false;
        s.update_drag(5.0 + 2.0123, Some(track), 0.0, false);
        assert_eq!(s.drag.as_ref().unwrap().snap_mark, None);
    }

    #[test]
    fn audio_only_edits_can_skip_the_frame_quantization() {
        let (mut s, track, ids) = two_touching_events(); // audio track
        let fps = s.project.settings.fps;
        let off_grid = |s: &mut AppState| {
            let origin = s.event_geometry(track, ids[1]).unwrap();
            s.begin_drag((track, ids[1], DragMode::Move, origin), 5.0);
            s.update_drag(5.0123 + 3.0, Some(track), 0.0, false);
            let start = s.drag.as_ref().unwrap().current.start;
            s.drag = None;
            ((start * fps).round() - start * fps).abs() > 1e-6
        };
        assert!(!off_grid(&mut s), "quantized by default");
        s.prefs.quantize_audio_edits = false;
        assert!(off_grid(&mut s), "left between frames for audio-only edits");
    }

    #[test]
    fn the_snap_switches_are_remembered_in_the_preferences() {
        let mut s = AppState::new();
        let ctx = egui::Context::default();
        s.run_command(Command::OptionsSnapToGrid, &ctx, 0.0);
        s.run_command(Command::OptionsSnapToMarkers, &ctx, 0.0);
        s.run_command(Command::OptionsQuantizeToFrames, &ctx, 0.0);
        assert!(s.prefs.snap_to_grid && !s.prefs.snap_to_markers && !s.prefs.quantize_to_frames);
        assert!(
            s.prefs.snap_to_all_events,
            "untouched switch keeps its value"
        );
    }

    #[test]
    fn switching_quantize_off_leaves_a_drag_between_frames() {
        let (mut s, track, ids) = two_touching_events();
        s.quantize_frames = false;
        let origin = s.event_geometry(track, ids[1]).unwrap();
        s.begin_drag((track, ids[1], DragMode::Move, origin), 5.0);
        s.update_drag(5.0123 + 3.0, Some(track), 0.0, false);
        let start = s.drag.as_ref().unwrap().current.start;
        assert!((start - 7.0123).abs() < 1e-9, "free time, got {start}");
        s.quantize_frames = true;
        s.update_drag(5.0123 + 3.0, Some(track), 0.0, false);
        let start = s.drag.as_ref().unwrap().current.start;
        let fps = s.project.settings.fps;
        assert!(
            ((start * fps).round() - start * fps).abs() < 1e-6,
            "on the frame grid"
        );
    }

    /// A real clip made by the `ffmpeg` tool goes through the whole path the user takes: import on
    /// the loader thread, the event on the video track, the frame request, the decoder thread and the
    /// texture of the preview. Skipped without the `ffmpeg` feature or the command-line tool.
    /// `NEOVEG_TEST_VIDEO=<file>` imports that file (read only) instead of the generated clip.
    #[cfg(feature = "ffmpeg")]
    #[test]
    fn an_imported_video_reaches_the_preview_texture() {
        use std::process::Command;
        use std::time::{Duration, Instant};
        let dir = std::env::temp_dir().join(format!("neoveg_preview_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let own = std::env::var_os("NEOVEG_TEST_VIDEO").map(PathBuf::from);
        let clip = own.clone().unwrap_or_else(|| dir.join("clip.mp4"));
        let made = own.is_some()
            || Command::new("ffmpeg")
                .args(["-y", "-v", "error", "-f", "lavfi", "-i"])
                .arg("testsrc=duration=2:size=320x240:rate=25")
                .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=2"])
                .args(["-c:v", "mpeg4", "-c:a", "aac", "-shortest"])
                .arg(&clip)
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
        if !made {
            let _ = std::fs::remove_dir_all(&dir);
            return;
        }

        let ctx = egui::Context::default();
        let mut s = AppState::new();
        s.start_frame_worker(&ctx);
        assert!(s.frame_worker.is_some(), "the decoder thread is running");

        // Import on the loader thread, as the Import media command does
        s.spawn_media(clip.clone(), None, &ctx);
        let deadline = Instant::now() + Duration::from_secs(20);
        while s.project.media_pool.is_empty() && Instant::now() < deadline {
            s.poll_loader();
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            !s.project.media_pool.is_empty(),
            "the clip was imported; status: {}",
            s.status
        );
        let video_events: usize = s
            .timeline
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Video)
            .map(|t| t.events.len())
            .sum();
        assert_eq!(video_events, 1, "one event on a video track");
        // The clip has a sound track: it lands on its own audio track and its samples are loaded
        let audio_events: usize = s
            .timeline
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Audio)
            .map(|t| t.events.len())
            .sum();
        println!(
            "audio events: {audio_events}, loaded audio: {}, status: {}",
            s.media.len(),
            s.status
        );
        assert_eq!(
            audio_events, 1,
            "the sound of the video is on an audio track"
        );
        assert!(
            s.media.values().all(|m| m.duration > 0.5),
            "the audio of the clip was decoded"
        );

        // Park the cursor inside the clip, ask for the picture and wait for the decoder thread
        s.playhead = 0.5;
        s.request_preview_frame(1.0);
        let deadline = Instant::now() + Duration::from_secs(30);
        while s.video_texture.is_none() && Instant::now() < deadline {
            s.poll_frames(&ctx);
            std::thread::sleep(Duration::from_millis(20));
        }
        let texture = s
            .video_texture
            .as_ref()
            .unwrap_or_else(|| panic!("no frame reached the preview; status: {}", s.status));
        let [w, h] = texture.size();
        assert!(w > 0 && h > 0, "the texture has a size");
        let info = s.video_info.values().next().expect("the clip was probed");
        let drift = (w * info.height as usize).abs_diff(h * info.width as usize);
        assert!(
            drift <= w.max(h),
            "the frame keeps the clip's aspect ratio ({w}x{h} for {}x{})",
            info.width,
            info.height
        );
        println!(
            "preview texture: {w}x{h} from a {}x{} clip",
            info.width, info.height
        );
        assert!(s.frame_scopes.is_some(), "the scopes read the same picture");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reduced_preview_qualities_show_their_pixels_and_the_others_stay_smooth() {
        let named = |name: &str| QUALITIES.iter().position(|(n, _)| *n == name).unwrap();
        assert_eq!(
            preview_texture_options(named("Draft")),
            egui::TextureOptions::NEAREST
        );
        assert_eq!(
            preview_texture_options(named("Preview")),
            egui::TextureOptions::NEAREST
        );
        for smooth in ["Good", "Best", "Auto"] {
            assert_eq!(
                preview_texture_options(named(smooth)),
                egui::TextureOptions::LINEAR,
                "{smooth}"
            );
        }
        assert_eq!(
            preview_texture_options(99),
            egui::TextureOptions::LINEAR,
            "unknown level"
        );
    }

    #[test]
    fn the_ripple_keys_ripple_the_last_edit_only_while_it_is_the_latest_change() {
        let (mut s, track, ids) = two_touching_events(); // 0-4 and 4-8
                                                         // Plain trim leaves a gap behind the first event
        let origin = s.event_geometry(track, ids[0]).unwrap();
        s.begin_drag((track, ids[0], DragMode::TrimRight, origin), 4.0);
        s.update_drag(2.0, Some(track), 0.0, false);
        s.finish_drag();
        let gap_start = s.timeline.tracks[0].events[1].start;
        assert!((gap_start - 4.0).abs() < 1e-9);
        s.run_command(
            Command::RippleAffectedTracks,
            &egui::Context::default(),
            0.0,
        );
        let (a, b) = (
            &s.timeline.tracks[0].events[0],
            &s.timeline.tracks[0].events[1],
        );
        assert!((a.end() - b.start).abs() < 1e-9, "F closed the gap");
        assert!(s.status.contains("Ripple aplicado"));
        s.undo();
        assert!(
            (s.timeline.tracks[0].events[1].start - 4.0).abs() < 1e-9,
            "one undo step"
        );

        // After another change the old edit is not rippled any more; the scope is still chosen
        s.redo();
        s.undo();
        s.undo();
        s.run_command(Command::RippleAll, &egui::Context::default(), 0.0);
        assert_eq!(s.ripple_scope, RippleScope::AllTracksAndMarks);
        assert!(!s.status.contains("Ripple aplicado"));
    }

    #[test]
    fn a_ripple_key_ignores_an_edit_that_is_no_longer_the_latest_change() {
        let (mut s, track, ids) = two_touching_events();
        let origin = s.event_geometry(track, ids[0]).unwrap();
        s.begin_drag((track, ids[0], DragMode::TrimRight, origin), 4.0);
        s.update_drag(2.0, Some(track), 0.0, false);
        s.finish_drag();
        s.add_track(TrackKind::Video); // another undo step on top of the trim
        s.run_command(
            Command::RippleAffectedTracks,
            &egui::Context::default(),
            0.0,
        );
        assert!(!s.status.contains("Ripple aplicado"));
        assert!(
            (s.timeline.tracks[0].events[1].start - 4.0).abs() < 1e-9,
            "the gap stays"
        );
    }

    #[test]
    fn a_plain_delete_can_be_rippled_afterwards_with_the_key() {
        let (mut s, track, ids) = two_touching_events();
        s.selection.select_only((track, ids[0]));
        s.delete_selection();
        assert_eq!(
            s.timeline.tracks[0].events[0].start, 4.0,
            "the gap is still there"
        );
        s.run_command(Command::RippleWithMarkers, &egui::Context::default(), 0.0);
        assert_eq!(s.timeline.tracks[0].events[0].start, 0.0);
    }

    #[test]
    fn trimming_a_grouped_event_edge_trims_the_members_that_share_it() {
        let (mut s, track, ids) = two_touching_events(); // 0-4 and 4-8 on one track
                                                         // Add a second track with an event that ends at the same time as the first (0-4)
        s.add_track(TrackKind::Audio);
        let other = s.selected_track.unwrap();
        let linked = add_event(&mut s, other, 0.0, 4.0);
        s.selection
            .select_many(vec![(track, ids[0]), (other, linked)], (track, ids[0]));
        s.group_selection();
        let origin = s.event_geometry(track, ids[0]).unwrap();
        s.begin_drag((track, ids[0], DragMode::TrimRight, origin), 4.0);
        s.update_drag(3.0, Some(track), 0.0, false);
        s.finish_drag();
        let a = s.event_geometry(track, ids[0]).unwrap();
        let b = s.event_geometry(other, linked).unwrap();
        assert!(
            a.duration < 4.0 && (a.duration - b.duration).abs() < 1e-9,
            "both trimmed"
        );
        s.undo();
        assert_eq!(
            s.event_geometry(other, linked).unwrap().duration,
            4.0,
            "one undo step"
        );

        // Shift (ignore the group): only the dragged event
        s.drag_ignores_group = true;
        let origin = s.event_geometry(track, ids[0]).unwrap();
        s.begin_drag((track, ids[0], DragMode::TrimRight, origin), 4.0);
        s.update_drag(3.0, Some(track), 0.0, false);
        s.finish_drag();
        assert!(s.event_geometry(track, ids[0]).unwrap().duration < 4.0);
        assert_eq!(s.event_geometry(other, linked).unwrap().duration, 4.0);
    }

    #[test]
    fn a_rolling_trim_without_a_touching_neighbour_is_a_plain_trim() {
        let (mut s, track, ids) = two_touching_events();
        s.timeline.tracks[0].events[1].start = 6.0;
        let origin = s.event_geometry(track, ids[0]).unwrap();
        s.begin_drag((track, ids[0], DragMode::RollRight, origin), 4.0);
        assert_eq!(s.drag.as_ref().unwrap().mode, DragMode::TrimRight);
    }

    #[test]
    fn a_locked_neighbour_blocks_the_rolling_trim() {
        let (mut s, track, ids) = two_touching_events();
        s.timeline.tracks[0].events[1].locked = true;
        let origin = s.event_geometry(track, ids[0]).unwrap();
        s.begin_drag((track, ids[0], DragMode::RollRight, origin), 4.0);
        assert!(s.drag.is_none());
        assert!(s.status.contains("bloqueado"));
    }

    #[test]
    fn dragging_with_the_slip_tool_moves_the_media_and_not_the_event() {
        let (mut s, track, ev) = state_with_clip();
        // The clip is 2..6 s of a 10 s source; give it a 1 s in point so it can slip both ways
        s.timeline.track_mut(track).unwrap().events[0].trim_in = 1.0;
        s.edit_tool = EditTool::Slip;
        let revision = s.history.revision();
        s.begin_drag(hit(&s, track, ev, DragMode::Move), 3.0);
        assert_eq!(s.drag.as_ref().unwrap().mode, DragMode::Slip);
        // Drag 2 s to the right: later media shows, the event stays put
        s.update_drag(5.0, None, 0.0, false);
        s.finish_drag();
        let g = geometry(&s, track, ev);
        // The drag is rounded to whole frames (60 frames of 29.97 fps is 2.002 s)
        assert_eq!((g.start, g.duration), (2.0, 4.0));
        assert!((g.trim_in - 3.0).abs() < 1.0 / 29.97, "{}", g.trim_in);
        assert_eq!(s.history.revision(), revision + 1, "one undo step");
        run(&mut s, Command::EditUndo);
        assert_eq!(geometry(&s, track, ev).trim_in, 1.0);
    }

    #[test]
    fn dragging_with_the_slide_tool_moves_the_event_and_keeps_the_media_in_place() {
        let (mut s, track, ev) = state_with_clip();
        s.timeline.track_mut(track).unwrap().events[0].trim_in = 1.0;
        s.edit_tool = EditTool::Slide;
        s.begin_drag(hit(&s, track, ev, DragMode::Move), 3.0);
        assert_eq!(s.drag.as_ref().unwrap().mode, DragMode::Slide);
        s.update_drag(5.0, None, 0.0, false);
        s.finish_drag();
        let g = geometry(&s, track, ev);
        // The event moved 2 s and its in point moved with it: the same media sits at the same time
        assert!((g.start - 4.0).abs() < 1.0 / 29.97 && g.duration == 4.0);
        assert!(
            ((g.trim_in - 1.0) - (g.start - 2.0)).abs() < 1e-9,
            "start and in point move together"
        );
    }

    #[test]
    fn slip_stops_at_the_end_of_the_media_and_a_locked_event_refuses_the_tool() {
        let (mut s, track, ev) = state_with_clip();
        s.edit_tool = EditTool::Slip;
        s.begin_drag(hit(&s, track, ev, DragMode::Move), 3.0);
        // 10 s of media, 4 s shown: the in point is at most 6 s however far the pointer goes
        s.update_drag(500.0, None, 0.0, false);
        s.finish_drag();
        assert_eq!(geometry(&s, track, ev).trim_in, 6.0);

        s.selection.set([(track, ev)]);
        run(&mut s, Command::EditSwitchLock);
        s.begin_drag(hit(&s, track, ev, DragMode::Move), 3.0);
        assert!(s.drag.is_none(), "a locked event cannot be slipped either");
    }

    #[test]
    fn the_normal_tool_still_moves_the_event_and_the_trim_edges_ignore_the_tool() {
        let (mut s, track, ev) = state_with_clip();
        s.edit_tool = EditTool::Slip;
        // The edges keep trimming whatever the tool is
        s.begin_drag(hit(&s, track, ev, DragMode::TrimRight), 6.0);
        assert_eq!(s.drag.as_ref().unwrap().mode, DragMode::TrimRight);
        s.drag = None;
        s.edit_tool = EditTool::Normal;
        s.begin_drag(hit(&s, track, ev, DragMode::Move), 3.0);
        assert_eq!(s.drag.as_ref().unwrap().mode, DragMode::Move);
    }

    fn write_png(path: &std::path::Path, width: u32, height: u32) {
        image::RgbaImage::from_pixel(width, height, image::Rgba([200, 30, 30, 255]))
            .save(path)
            .unwrap();
    }

    #[test]
    fn a_slideshow_is_laid_on_the_video_track_with_overlaps_as_one_undo_step() {
        use neoveg_engine::slideshow::{Slide, SlideshowSettings};
        let dir = std::env::temp_dir().join(format!("neoveg_show_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["a.png", "b.png", "c.png"] {
            write_png(&dir.join(name), 64, 36);
        }
        let mut slides: Vec<Slide> = ["a.png", "b.png", "c.png"]
            .iter()
            .map(|n| Slide::new(&dir.join(n).to_string_lossy(), 0))
            .collect();
        // A picture that is gone is skipped, not an error for the others
        slides.insert(1, Slide::new(&dir.join("sumiu.png").to_string_lossy(), 0));

        let mut s = AppState::new();
        // The track exists already, so what is measured is the show alone
        s.add_track(TrackKind::Video);
        let revision = s.history.revision();
        s.insert_slideshow(
            &slides,
            SlideshowSettings {
                start: 2.0,
                per_photo: 4.0,
                overlap: 1.0,
            },
        );
        let track = s
            .timeline
            .tracks
            .iter()
            .find(|t| t.kind == TrackKind::Video)
            .expect("a video track was made");
        let spans: Vec<(f64, f64)> = track.events.iter().map(|e| (e.start, e.end())).collect();
        // Each picture starts 3 s after the last (4 s long, 1 s overlap)
        assert_eq!(spans, vec![(2.0, 6.0), (5.0, 9.0), (8.0, 12.0)]);
        assert_eq!(s.project.media_pool.len(), 3);
        assert_eq!(s.history.revision(), revision + 1, "one undo step");
        assert!(s.status.contains("1 ilegíveis"));
        // The neighbours overlap, so with automatic crossfades they fade into each other
        assert_eq!(
            neoveg_engine::crossfade::find_crossfades(&track.events).len(),
            2
        );
        run(&mut s, Command::EditUndo);
        assert!(
            s.timeline.tracks.iter().all(|t| t.events.is_empty()),
            "undo takes the whole show away"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_first_media_does_not_overwrite_a_format_the_user_chose() {
        let dir = std::env::temp_dir().join(format!("neoveg_format_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("foto.png");
        write_png(&file, 640, 480);
        let slide = neoveg_engine::slideshow::Slide::new(&file.to_string_lossy(), 0);
        let settings = neoveg_engine::slideshow::SlideshowSettings {
            start: 0.0,
            per_photo: 3.0,
            overlap: 0.0,
        };

        // Nothing chosen: the first picture sets the frame size
        let mut open = AppState::new();
        open.insert_slideshow(std::slice::from_ref(&slide), settings);
        assert_eq!(
            (open.project.settings.width, open.project.settings.height),
            (640, 480)
        );

        // A project made with a format keeps it
        let mut chosen = AppState::new();
        chosen.new_project_with(ProjectSettings {
            width: 3840,
            height: 2160,
            ..ProjectSettings::default()
        });
        chosen.insert_slideshow(std::slice::from_ref(&slide), settings);
        assert_eq!(
            (
                chosen.project.settings.width,
                chosen.project.settings.height
            ),
            (3840, 2160)
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_stretched_event_refuses_the_loop_switch() {
        let (mut s, track, ev) = state_with_clip();
        s.timeline.track_mut(track).unwrap().events[0].playback_rate = 2.0;
        s.selection.set([(track, ev)]);
        run(&mut s, Command::EditSwitchLoop);
        assert!(!s.timeline.track(track).unwrap().events[0].looping);
    }

    #[test]
    fn normalize_measures_the_played_part_and_recalculate_follows_a_trim() {
        let (mut s, track, ev) = state_with_clip();
        // 10 s at 100 Hz: quiet (0.25) except for one loud frame (0.5) at 7 s
        let mut samples = vec![0.25_f32; 2_000];
        samples[1_400] = 0.5;
        let mut media = test_media();
        media.stereo = PcmStore::shared(samples);
        s.media.insert(1, media);
        s.selection.set([(track, ev)]);

        // The event plays 0..4 s of the source: peak 0.25, so a gain of 4
        run(&mut s, Command::EditSwitchNormalize);
        let gain = |s: &AppState| s.timeline.track(track).unwrap().events[0].normalize_gain;
        assert!((gain(&s) - 4.0).abs() < 1e-4, "{}", gain(&s));
        let clip = s.build_clips().into_iter().next().unwrap();
        assert!((clip.gain - 4.0).abs() < 1e-4);

        // Extend the event over the loud frame: the stored gain is stale until recalculated
        s.timeline.track_mut(track).unwrap().events[0].duration = 8.0;
        assert!((gain(&s) - 4.0).abs() < 1e-4);
        run(&mut s, Command::EditNormalizeRecalculate);
        assert!((gain(&s) - 2.0).abs() < 1e-4, "{}", gain(&s));

        run(&mut s, Command::EditUndo);
        assert!((gain(&s) - 4.0).abs() < 1e-4);
        run(&mut s, Command::EditSwitchNormalize);
        assert!(!s.timeline.track(track).unwrap().events[0].normalize);
        assert_eq!(gain(&s), 1.0);
    }

    #[test]
    fn normalize_without_loaded_media_does_nothing() {
        let (mut s, track, ev) = state_with_clip();
        s.selection.set([(track, ev)]);
        let revision = s.history.revision();
        run(&mut s, Command::EditSwitchNormalize);
        assert_eq!(s.history.revision(), revision);
        assert!(!s.timeline.track(track).unwrap().events[0].normalize);
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
        let c = s.snap_candidates(&[(track, ev)]);
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

    #[test]
    fn the_volume_envelope_of_an_event_reaches_the_clip_the_mixer_plays() {
        let (mut s, track, event) = state_with_clip(); // event 2..6
        add_event(&mut s, track, 8.0, 2.0);
        s.media.insert(1, test_media());
        assert!(s.build_clips().iter().all(|c| c.volume.is_none()));

        // -20 dB at the start of the event, ramping to 0 dB after 2 s
        let mut env = neoveg_engine::event_volume::new_envelope(-20.0);
        env.points.push(neoveg_engine::envelope::EnvelopePoint {
            time: 2.0,
            value: 0.0,
            shape: neoveg_engine::envelope::EnvelopeShape::Linear,
        });
        let cmd = neoveg_engine::event_volume::set(&s.timeline, track, event, Some(env)).unwrap();
        assert!(s.history.execute(&mut s.timeline, cmd));
        let clips = s.build_clips();
        let first = clips.iter().find(|c| c.start == 2.0).unwrap();
        let curve = first.volume.as_ref().expect("the first event has a curve");
        assert!((curve.gain_at(0.0) - 0.1).abs() < 1e-5, "-20 dB");
        assert!((curve.gain_at(2.0) - 1.0).abs() < 1e-6, "0 dB");
        assert_eq!(clips.iter().filter(|c| c.volume.is_some()).count(), 1);

        // Undo takes it away again
        assert!(s.history.undo(&mut s.timeline));
        assert!(s.build_clips().iter().all(|c| c.volume.is_none()));
    }

    fn test_media() -> LoadedMedia {
        let samples = vec![0.0f32; 200];
        LoadedMedia {
            path: PathBuf::from("x.wav"),
            name: "x.wav".into(),
            waveform: Arc::new(WaveformSource::from_interleaved_f32(100, 2, &samples).unwrap()),
            stereo: PcmStore::shared(samples),
            sample_rate: 100,
            channels: 2,
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
    fn bracket_keys_pick_an_edge_and_the_frame_keys_nudge_it_in_undoable_steps() {
        let (mut s, track, ev) = state_with_clip(); // event 2.0 - 6.0
        let fps = s.project.settings.fps;
        s.playhead = 0.0;
        run(&mut s, Command::EditTrimEdgeNext);
        let edge = s.trim_edge.expect("an edge is selected");
        assert_eq!(
            (edge.event, edge.side),
            (ev, neoveg_engine::keytrim::EdgeSide::Start)
        );
        assert_eq!(s.playhead, 2.0, "the cursor goes to the edge");
        run(&mut s, Command::EditTrimEdgeNext);
        assert_eq!(s.playhead, 6.0, "next edge is the end");

        // With an edge selected the frame keys trim it instead of moving the cursor
        run(&mut s, Command::CursorRightByFrame);
        let g = geometry(&s, track, ev);
        assert!(
            (g.duration - (4.0 + 1.0 / fps)).abs() < 1e-9,
            "grew one frame"
        );
        assert!(
            (s.playhead - g.end()).abs() < 1e-9,
            "the cursor follows the edge"
        );
        run(&mut s, Command::CursorLeftByFrame);
        assert!((geometry(&s, track, ev).duration - 4.0).abs() < 1e-9);
        s.undo();
        s.undo();
        assert_eq!(geometry(&s, track, ev).duration, 4.0);

        // Without an edge the same keys only move the cursor
        s.trim_edge = None;
        let before = geometry(&s, track, ev);
        run(&mut s, Command::CursorRightByFrame);
        assert_eq!(geometry(&s, track, ev), before);
    }

    #[test]
    fn the_modifier_commands_stretch_the_edge_or_roll_the_cut() {
        let (mut s, track, ids) = two_touching_events(); // 0-4 and 4-8, source for the second starts at 2
        s.playhead = 0.0;
        run(&mut s, Command::EditTrimEdgeNext); // from the cursor at 0: end of the first event
        assert_eq!(s.playhead, 4.0, "the cut between the two events");
        let fps = s.project.settings.fps;
        run(&mut s, Command::EditTrimEdgeStretchRight);
        let a = geometry(&s, track, ids[0]);
        assert!(
            (a.duration - (4.0 + 1.0 / fps)).abs() < 1e-9 && a.playback_rate < 1.0,
            "stretched"
        );
        s.undo();
        run(&mut s, Command::EditTrimEdgeRollRight);
        let (a, b) = (geometry(&s, track, ids[0]), geometry(&s, track, ids[1]));
        assert!(
            (a.end() - b.start).abs() < 1e-9 && a.duration > 4.0,
            "the cut moved for both"
        );
        assert!((a.duration + b.duration - 8.0).abs() < 1e-9);
        s.undo();
        assert_eq!(geometry(&s, track, ids[1]).start, 4.0, "one undo step");

        // Without a selected edge the modifier commands only explain what to do
        s.trim_edge = None;
        run(&mut s, Command::EditTrimEdgeStretchLeft);
        assert!(s.status.contains("[ ou ]"));
    }

    #[test]
    fn the_edit_tool_decides_what_the_frame_keys_do_to_the_edge() {
        let (mut s, track, ev) = state_with_clip();
        s.playhead = 0.0;
        run(&mut s, Command::EditTrimEdgeNext);
        s.edit_tool = EditTool::Slide;
        run(&mut s, Command::CursorRightByFrame);
        let g = geometry(&s, track, ev);
        assert!(
            g.start > 2.0 && (g.duration - 4.0).abs() < 1e-9,
            "slide moved the whole event"
        );
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
    fn number_keys_jump_to_the_nth_marker() {
        let (mut s, _, _) = state_with_clip();
        for t in [7.0, 3.0, 5.0] {
            s.playhead = t;
            run(&mut s, Command::InsertMarker);
            s.commit_name_edit();
        }
        run(&mut s, Command::CursorMarker1);
        assert_eq!(s.playhead, 3.0);
        run(&mut s, Command::CursorMarker3);
        assert_eq!(s.playhead, 7.0);
        run(&mut s, Command::CursorMarker2);
        assert_eq!(s.playhead, 5.0);
        run(&mut s, Command::CursorMarker4);
        assert_eq!(
            s.playhead, 5.0,
            "a missing marker leaves the cursor and says so"
        );
        assert!(s.status.contains('4'));
    }

    /// Two events on one track and one on a second audio track, none selected.
    fn state_with_three_events() -> (AppState, [EventRef; 3]) {
        let (mut s, track, ev) = state_with_clip();
        let b = add_event(&mut s, track, 10.0, 2.0);
        s.add_track(TrackKind::Audio);
        let other = s.timeline.tracks[1].id;
        let c = add_event(&mut s, other, 14.0, 2.0);
        s.selection.clear();
        (s, [(track, ev), (track, b), (other, c)])
    }

    #[test]
    fn group_and_ungroup_commands_are_one_undo_step_each() {
        let (mut s, [a, b, _]) = state_with_three_events();
        run(&mut s, Command::EditGroup);
        assert!(s.status.contains("dois"), "one event cannot be grouped");
        s.selection.set([a, b]);
        let revision = s.history.revision();
        run(&mut s, Command::EditGroup);
        assert_eq!(s.history.revision(), revision + 1);
        assert_eq!(groups::group_members(&s.timeline, a).len(), 2);

        run(&mut s, Command::EditUngroup);
        assert_eq!(groups::group_members(&s.timeline, a).len(), 1);
        run(&mut s, Command::EditUndo);
        assert_eq!(groups::group_members(&s.timeline, a).len(), 2);
    }

    #[test]
    fn clicking_a_grouped_event_selects_the_group_unless_grouping_is_ignored() {
        let (mut s, [a, b, c]) = state_with_three_events();
        s.selection.set([a, b]);
        run(&mut s, Command::EditGroup);
        s.selection.clear();

        s.click_select(a);
        assert_eq!(s.selection.len(), 2);
        s.click_select(c);
        assert_eq!(s.selection.refs(), vec![c]);

        s.click_toggle(b);
        assert_eq!(s.selection.len(), 3, "Ctrl-click adds the whole group");
        s.click_toggle(a);
        assert_eq!(s.selection.refs(), vec![c], "and removes it again");

        run(&mut s, Command::EditIgnoreGrouping);
        assert!(s.ignore_grouping);
        s.click_select(a);
        assert_eq!(s.selection.refs(), vec![a]);
    }

    #[test]
    fn dragging_one_selected_event_moves_the_whole_selection_in_one_undo_step() {
        let (mut s, [a, b, c]) = state_with_three_events();
        s.selection.set([a, b, c]);
        let hit = (a.0, a.1, DragMode::Move, geometry(&s, a.0, a.1));
        s.begin_drag(hit, 3.0);
        assert_eq!(s.drag.as_ref().unwrap().moving.len(), 3);
        s.update_drag(8.0, Some(a.0), 0.0, false);
        let revision = s.history.revision();
        s.finish_drag();

        let delta = 5.0;
        for (r, old) in [(a, 2.0), (b, 10.0), (c, 14.0)] {
            let start = geometry(&s, r.0, r.1).start;
            assert!(
                (start - (old + delta)).abs() < 0.05,
                "{start} vs {}",
                old + delta
            );
        }
        assert_eq!(s.history.revision(), revision + 1);
        assert_eq!(s.selection.len(), 3, "the selection survives the move");
        run(&mut s, Command::EditUndo);
        assert_eq!(geometry(&s, b.0, b.1).start, 10.0);
        assert_eq!(geometry(&s, c.0, c.1).start, 14.0);
    }

    fn flags(s: &AppState, r: EventRef) -> (bool, bool, bool) {
        let e = s
            .timeline
            .track(r.0)
            .unwrap()
            .events
            .iter()
            .find(|e| e.id == r.1)
            .unwrap();
        (e.muted, e.locked, e.invert_phase)
    }

    #[test]
    fn switch_commands_toggle_the_selection_as_one_undo_step() {
        let (mut s, [a, b, _]) = state_with_three_events();
        s.selection.set([a, b]);
        let revision = s.history.revision();
        run(&mut s, Command::EditSwitchMute);
        assert_eq!(s.history.revision(), revision + 1);
        assert!(flags(&s, a).0 && flags(&s, b).0);
        run(&mut s, Command::EditSwitchMute);
        assert!(!flags(&s, a).0, "a second toggle turns it off");
        run(&mut s, Command::EditUndo);
        assert!(flags(&s, a).0 && flags(&s, b).0);
    }

    #[test]
    fn a_locked_event_refuses_a_drag_and_a_selection_holding_one_too() {
        let (mut s, [a, b, _]) = state_with_three_events();
        s.selection.set([a]);
        run(&mut s, Command::EditSwitchLock);
        assert!(flags(&s, a).1);

        let hit = (a.0, a.1, DragMode::Move, geometry(&s, a.0, a.1));
        s.begin_drag(hit, 3.0);
        assert!(s.drag.is_none());

        s.selection.set([a, b]);
        let hit = (b.0, b.1, DragMode::Move, geometry(&s, b.0, b.1));
        s.begin_drag(hit, 11.0);
        assert!(s.drag.is_none(), "the whole gesture is refused");

        run(&mut s, Command::EditDelete);
        assert!(
            s.timeline
                .track(a.0)
                .unwrap()
                .events
                .iter()
                .any(|e| e.id == a.1),
            "Delete leaves the locked event"
        );
    }

    #[test]
    fn a_block_move_stops_when_the_earliest_event_reaches_zero() {
        let (mut s, [a, b, _]) = state_with_three_events();
        s.selection.set([a, b]);
        let hit = (a.0, a.1, DragMode::Move, geometry(&s, a.0, a.1));
        s.begin_drag(hit, 3.0);
        s.update_drag(-50.0, Some(a.0), 0.0, false);
        s.finish_drag();
        assert_eq!(geometry(&s, a.0, a.1).start, 0.0);
        assert!(
            (geometry(&s, b.0, b.1).start - 8.0).abs() < 1e-6,
            "distance kept"
        );
    }

    #[test]
    fn rectangular_selection_takes_the_touched_events_on_the_covered_rows() {
        let (mut s, [a, b, c]) = state_with_three_events();
        // First track only: times 0..11 touch a (2-6) and b (10-12)
        s.select_rect(0.0, 11.0, 0.2, 0.8, false);
        assert_eq!(s.selection.refs(), vec![a, b]);
        // Both tracks, later times
        s.select_rect(13.0, 20.0, 0.5, 1.5, false);
        assert_eq!(s.selection.refs(), vec![c]);
        // Ctrl keeps what was selected
        s.select_rect(0.0, 3.0, 0.5, 0.6, true);
        let mut got = s.selection.refs();
        got.sort();
        let mut want = vec![a, c];
        want.sort();
        assert_eq!(got, want);
        // Empty space clears
        s.select_rect(100.0, 120.0, 0.0, 1.0, false);
        assert!(s.selection.is_empty());
    }

    #[test]
    fn a_rectangle_that_touches_one_grouped_event_selects_the_whole_group() {
        let (mut s, [a, b, c]) = state_with_three_events();
        s.selection.set([a, c]);
        run(&mut s, Command::EditGroup);
        s.select_rect(0.0, 3.0, 0.1, 0.9, false);
        let mut got = s.selection.refs();
        got.sort();
        let mut want = vec![a, c];
        want.sort();
        assert_eq!(got, want);
        assert!(!got.contains(&b));
    }

    #[test]
    fn the_effects_panel_edits_the_selected_audio_track_or_the_master_bus() {
        let mut s = AppState::new();
        assert!(s.fx_target().is_none(), "no track selected");
        s.add_track(TrackKind::Video);
        assert!(
            s.fx_target().is_none(),
            "video tracks have no audio effects"
        );
        s.add_track(TrackKind::Audio);
        let audio = s.selected_track.unwrap();
        assert_eq!(s.fx_target().map(|t| t.0), Some(FxTarget::Track(audio)));

        s.fx.master = true;
        assert_eq!(s.fx_target().map(|t| t.0), Some(FxTarget::Master));
    }

    #[test]
    fn effect_actions_become_one_undo_step_each_and_reach_the_audio_plan() {
        let mut s = AppState::new();
        s.add_track(TrackKind::Audio);
        let (target, _) = s.fx_target().unwrap();
        let revision = s.history.revision();

        s.apply_fx_actions(
            target,
            vec![
                FxAction::Add("compressor"),
                FxAction::Add("simple_eq"),
                FxAction::SetParam {
                    index: 0,
                    param: "ratio".into(),
                    value: 8.0,
                },
            ],
        );
        assert_eq!(s.history.revision(), revision + 3);
        let plan = fx_panel::fx_plan(&s.timeline);
        assert_eq!(plan.tracks[0].len(), 2);
        assert_eq!(plan.tracks[0][0].params, vec![("ratio".to_string(), 8.0)]);

        s.apply_fx_actions(target, vec![FxAction::Bypass(0, true)]);
        assert_eq!(
            fx_panel::fx_plan(&s.timeline).tracks[0].len(),
            1,
            "bypassed effects are not played"
        );

        s.apply_fx_actions(
            target,
            vec![FxAction::Move { from: 0, to: 1 }, FxAction::Remove(0)],
        );
        run(&mut s, Command::EditUndo);
        run(&mut s, Command::EditUndo);
        run(&mut s, Command::EditUndo);
        assert_eq!(fx::chain(&s.timeline, target).unwrap().len(), 2);
        assert!(!fx::chain(&s.timeline, target).unwrap()[0].bypass);
    }

    #[test]
    fn actions_that_change_nothing_leave_the_history_alone() {
        let mut s = AppState::new();
        s.add_track(TrackKind::Audio);
        let (target, _) = s.fx_target().unwrap();
        let revision = s.history.revision();
        s.apply_fx_actions(
            target,
            vec![
                FxAction::Remove(3),
                FxAction::Bypass(0, true),
                FxAction::ResetParam {
                    index: 0,
                    param: "ratio".into(),
                },
            ],
        );
        assert_eq!(s.history.revision(), revision);
    }

    #[test]
    fn a_clip_carries_the_index_of_its_track_even_when_an_earlier_track_is_muted() {
        let (mut s, first, _) = state_with_clip();
        s.add_track(TrackKind::Audio);
        let second = s.timeline.tracks[1].id;
        add_event(&mut s, second, 0.0, 2.0);
        s.media.insert(1, test_media());
        s.timeline.track_mut(first).unwrap().muted = true;

        let clips = s.build_clips();
        assert_eq!(clips.len(), 1, "only the unmuted track plays");
        assert_eq!(
            clips[0].track, 1,
            "the effect chain of track 2 is found by this index"
        );
        assert_eq!(fx_panel::fx_plan(&s.timeline).tracks.len(), 2);
    }

    #[test]
    fn ducking_actions_edit_the_selected_track_and_reach_the_audio_plan() {
        let mut s = AppState::new();
        s.add_track(TrackKind::Audio);
        let voice = s.selected_track.unwrap();
        s.add_track(TrackKind::Audio);
        let music = s.selected_track.unwrap();
        let (target, _) = s.fx_target().unwrap();
        assert_eq!(target, FxTarget::Track(music));
        let revision = s.history.revision();

        s.apply_fx_actions(
            target,
            vec![
                FxAction::Duck(DuckAction::Enable(true)),
                FxAction::Duck(DuckAction::Controller(voice, true)),
                FxAction::Duck(DuckAction::Param(fx::DuckParam::ReduceDb, 20.0)),
            ],
        );
        assert_eq!(s.history.revision(), revision + 3, "one undo step each");
        let plan = fx_panel::fx_plan(&s.timeline);
        let spec = plan.duck[1].as_ref().unwrap();
        assert_eq!((spec.controllers.clone(), spec.reduce_db), (vec![0], 20.0));
        assert!(plan.duck[0].is_none());

        run(&mut s, Command::EditUndo);
        assert_eq!(
            s.timeline
                .track(music)
                .unwrap()
                .ducking
                .as_ref()
                .unwrap()
                .reduce_db,
            15.0
        );
        s.apply_fx_actions(target, vec![FxAction::Duck(DuckAction::Enable(false))]);
        assert!(s.timeline.track(music).unwrap().ducking.is_none());
    }

    #[test]
    fn ducking_is_a_track_setting_and_is_ignored_for_the_master_bus() {
        let mut s = AppState::new();
        s.add_track(TrackKind::Audio);
        let revision = s.history.revision();
        s.apply_fx_actions(
            FxTarget::Master,
            vec![FxAction::Duck(DuckAction::Enable(true))],
        );
        assert_eq!(s.history.revision(), revision);
    }

    #[test]
    fn subtitle_actions_are_one_undo_step_each() {
        let mut s = AppState::new();
        s.playhead = 3.0;
        let revision = s.history.revision();
        s.apply_caption_actions(vec![CaptionAction::Add], 0.0);
        assert_eq!(s.timeline.captions.len(), 1);
        let id = s.timeline.captions[0].id;
        assert_eq!(s.timeline.captions[0].start, 3.0);

        s.apply_caption_actions(
            vec![
                CaptionAction::SetText(id, "Olá".into()),
                CaptionAction::SetTimes(id, 4.0, 6.5),
            ],
            0.0,
        );
        assert_eq!(s.history.revision(), revision + 3);
        let c = &s.timeline.captions[0];
        assert_eq!((c.text.as_str(), c.start, c.end), ("Olá", 4.0, 6.5));
        assert_eq!(captions::active_at(&s.timeline, 5.0).len(), 1);

        s.apply_caption_actions(vec![CaptionAction::Seek(9.0)], 1.0);
        assert_eq!(s.playhead, 9.0);
        assert_eq!(
            s.history.revision(),
            revision + 3,
            "moving the cursor is not an edit"
        );

        run(&mut s, Command::EditUndo);
        run(&mut s, Command::EditUndo);
        assert_eq!(s.timeline.captions[0].text, "");
        s.apply_caption_actions(vec![CaptionAction::Remove(id)], 0.0);
        assert!(s.timeline.captions.is_empty());
        s.apply_caption_actions(vec![CaptionAction::Clear], 0.0);
        assert_eq!(
            s.history.revision(),
            revision + 6,
            "clearing nothing changes nothing"
        );

        let before_whisper = s.history.revision();
        s.apply_caption_actions(
            vec![CaptionAction::ApplyWhisper {
                segments: vec![
                    neoveg_audio::whisper::TranscriptSegment {
                        start_s: 0.5,
                        end_s: 2.0,
                        text: "Primeira fala".into(),
                    },
                    neoveg_audio::whisper::TranscriptSegment {
                        start_s: 2.2,
                        end_s: 4.0,
                        text: "Segunda fala".into(),
                    },
                ],
                replace_all: true,
            }],
            0.0,
        );
        assert_eq!(s.timeline.captions.len(), 2);
        assert_eq!(
            s.history.revision(),
            before_whisper + 1,
            "Whisper import must commit as one undo step"
        );
        run(&mut s, Command::EditUndo);
        assert!(s.timeline.captions.is_empty());
    }

    fn edl_event(reel: &str, clip: Option<&str>, file: Option<&str>) -> EdlEvent {
        EdlEvent {
            number: 1,
            reel: reel.into(),
            track: edl::EdlTrack::Video,
            transition: edl::Transition::Cut,
            source_in: 0.0,
            source_out: 1.0,
            record_in: 0.0,
            record_out: 1.0,
            clip_name: clip.map(str::to_owned),
            source_file: file.map(str::to_owned),
        }
    }

    fn pool() -> Vec<MediaRef> {
        vec![
            MediaRef {
                subproject: false,
                format: None,
                id: 1,
                path: r"C:\media\Entrevista 01.mp4".into(),
                duration_seconds: 60.0,
            },
            MediaRef {
                subproject: false,
                format: None,
                id: 2,
                path: "/home/u/cobertura.mov".into(),
                duration_seconds: 12.0,
            },
        ]
    }

    #[test]
    fn edl_events_find_their_media_by_file_name_clip_name_or_reel() {
        let pool = pool();
        let id = |e: &EdlEvent| match_edl_media(e, &pool).map(|m| m.media_id);
        // By the source file, in either path style and any letter case
        assert_eq!(
            id(&edl_event("X", None, Some(r"D:\outro\ENTREVISTA 01.MP4"))),
            Some(1)
        );
        assert_eq!(
            id(&edl_event("X", None, Some("/mnt/cobertura.mov"))),
            Some(2)
        );
        // By the clip name, with or without the extension
        assert_eq!(id(&edl_event("X", Some("Entrevista 01"), None)), Some(1));
        assert_eq!(id(&edl_event("X", Some("cobertura.mov"), None)), Some(2));
        // By the reel when nothing else says what it is
        assert_eq!(id(&edl_event("COBERTUR", None, None)), Some(2));
        // The reel alone is not enough once a clip name says it is something else
        assert_eq!(id(&edl_event("COBERTUR", Some("outro plano"), None)), None);
        assert_eq!(id(&edl_event("ZZZ", Some("nada"), Some("x.mp4"))), None);
        assert_eq!(
            match_edl_media(&edl_event("X", Some("Entrevista 01"), None), &pool)
                .unwrap()
                .duration,
            60.0
        );
    }

    #[test]
    fn pasting_attributes_copies_them_onto_the_selected_events_in_one_undo_step() {
        let (mut s, track, ev) = state_with_clip();
        // A second event on the same track, with attributes to lend
        let other = s.timeline.alloc_id();
        let source = TrackEvent {
            id: other,
            media_id: Some(1),
            start: 10.0,
            duration: 2.0,
            muted: true,
            looping: true,
            ..TrackEvent::default()
        };
        assert!(s.history.execute(
            &mut s.timeline,
            EditCommand::AddEvent {
                track_id: track,
                event: source
            }
        ));
        // Nothing on the clipboard: it says so and changes nothing
        s.selection.select_only((track, ev));
        let revision = s.history.revision();
        s.paste_attributes(Attributes::ALL);
        assert_eq!(s.history.revision(), revision);
        assert!(s.status.contains("vazia"));
        // Copy the source, paste its attributes onto the first event
        s.selection.select_only((track, other));
        run(&mut s, Command::EditCopy);
        s.selection.select_only((track, ev));
        assert!(s.command_enabled(Command::EditPasteAttributes));
        run(&mut s, Command::EditPasteAttributes);
        let first = |s: &AppState| {
            s.timeline
                .track(track)
                .unwrap()
                .events
                .iter()
                .find(|e| e.id == ev)
                .unwrap()
                .clone()
        };
        assert!(first(&s).muted && first(&s).looping);
        assert_eq!(first(&s).start, 2.0, "position stays");
        assert!(s.history.undo(&mut s.timeline));
        assert!(!first(&s).muted);
        // The selective command opens the dialog
        run(&mut s, Command::EditPasteAttributesSelective);
        assert_eq!(s.attributes_dialog, Some(Attributes::ALL));
    }

    /// Every kind of file goes through the import; what Neoveg cannot read directly is converted and
    /// the copy must then import. Files are made with `ffmpeg` (the test skips without it), and a
    /// format the local `ffmpeg` cannot write is reported as not made, not as a failure.
    #[test]
    fn every_media_format_imports_directly_or_after_the_conversion() {
        use std::process::{Command, Stdio};
        let dir = std::env::temp_dir().join("neoveg_all_formats");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ffmpeg_ok = Command::new("ffmpeg")
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        if !ffmpeg_ok
            || neoveg_video::render::encoder_for(neoveg_video::render::RenderFormat::Mp4H264)
                .is_err()
        {
            return;
        }
        const VIDEO_SRC: &str = "testsrc=size=320x240:rate=10:duration=1";
        const SOUND_SRC: &str = "sine=frequency=440:duration=1";
        // (file name, kind, extra ffmpeg output arguments)
        let video: &[(&str, &[&str])] = &[
            ("v.mp4", &["-c:v", "libx264", "-c:a", "aac"]),
            ("v.m4v", &["-c:v", "libx264", "-c:a", "aac"]),
            ("v.mov", &["-c:v", "mpeg4", "-c:a", "pcm_s16le"]),
            ("v.mkv", &["-c:v", "mpeg4", "-c:a", "libmp3lame"]),
            ("v.avi", &["-c:v", "mpeg4", "-c:a", "libmp3lame"]),
            ("v.webm", &["-c:v", "libvpx", "-c:a", "libvorbis"]),
            ("v.wmv", &["-c:v", "wmv2", "-c:a", "wmav2"]),
            (
                "v.flv",
                &["-c:v", "flv", "-c:a", "libmp3lame", "-ar", "44100"],
            ),
            ("v.mpg", &["-c:v", "mpeg2video", "-c:a", "mp2"]),
            ("v.ts", &["-c:v", "mpeg2video", "-c:a", "mp2"]),
            ("v.ogv", &["-c:v", "libtheora", "-c:a", "libvorbis"]),
            (
                "v.3gp",
                &[
                    "-c:v", "h263", "-s", "176x144", "-c:a", "aac", "-ar", "8000", "-ac", "1",
                ],
            ),
            ("v.mxf", &["-c:v", "mpeg2video", "-c:a", "pcm_s16le"]),
        ];
        let sound: &[(&str, &[&str])] = &[
            ("a.wav", &["-c:a", "pcm_s16le"]),
            ("a.mp3", &["-c:a", "libmp3lame"]),
            ("a.flac", &["-c:a", "flac"]),
            ("a.ogg", &["-c:a", "libvorbis"]),
            ("a.opus", &["-c:a", "libopus"]),
            ("a.m4a", &["-c:a", "aac"]),
            ("a.aac", &["-c:a", "aac"]),
            ("a.wma", &["-c:a", "wmav2"]),
            ("a.aiff", &["-c:a", "pcm_s16be"]),
            ("a.ac3", &["-c:a", "ac3"]),
            ("a.mka", &["-c:a", "libvorbis"]),
        ];
        let images: &[(&str, &[&str])] = &[
            ("i.png", &[]),
            ("i.jpg", &[]),
            ("i.bmp", &[]),
            ("i.tiff", &[]),
            ("i.webp", &[]),
            ("i.gif", &[]),
        ];
        let make =
            |name: &str, args: &[&str], with_video: bool, with_sound: bool, one_frame: bool| {
                let out = dir.join(name);
                let mut cmd = Command::new("ffmpeg");
                cmd.args(["-hide_banner", "-y"]);
                if with_video {
                    cmd.args(["-f", "lavfi", "-i", VIDEO_SRC]);
                }
                if with_sound {
                    cmd.args(["-f", "lavfi", "-i", SOUND_SRC]);
                }
                if !with_sound && with_video {
                    cmd.arg("-an");
                }
                if one_frame {
                    cmd.args(["-frames:v", "1"]);
                }
                cmd.args(args).arg("-shortest").arg(&out);
                let ok = cmd
                    .stdin(Stdio::null())
                    .output()
                    .is_ok_and(|o| o.status.success())
                    && out.metadata().is_ok_and(|m| m.len() > 0);
                ok.then_some(out)
            };
        let mut files = Vec::new();
        let mut not_made = Vec::new();
        for (name, args) in video {
            match make(name, args, true, true, false) {
                Some(f) => files.push(f),
                None => not_made.push(*name),
            }
        }
        for (name, args) in sound {
            match make(name, args, false, true, false) {
                Some(f) => files.push(f),
                None => not_made.push(*name),
            }
        }
        for (name, args) in images {
            match make(name, args, true, false, true) {
                Some(f) => files.push(f),
                None => not_made.push(*name),
            }
        }
        assert!(files.len() >= 10, "ffmpeg made only {} files", files.len());
        let mut direct = 0;
        let mut converted = 0;
        let mut broken = Vec::new();
        let mut no_video_decoder = Vec::new();
        for file in &files {
            let name = file.file_name().unwrap().to_string_lossy().into_owned();
            // Without the `ffmpeg` feature no container video opens, converted or not
            if is_video_path(file) && !neoveg_video::info::video_decoding_enabled() {
                no_video_decoder.push(name);
                continue;
            }
            match load_message(file.clone(), None, false) {
                LoadMsg::Failed { path, error, .. } => {
                    let target = neoveg_video::convert::target_for(&path);
                    let out =
                        neoveg_video::convert::output_path(&path, &dir.join("conv"), target, |p| {
                            p.exists()
                        });
                    match neoveg_video::convert::convert(&path, &out, target) {
                        Ok(())
                            if !matches!(load_message(out, None, true), LoadMsg::Failed { .. }) =>
                        {
                            converted += 1;
                            println!("{name:10} converte -> {} ({error})", target.label());
                        }
                        other => {
                            println!("{name:10} FALHOU mesmo convertendo: {other:?}");
                            broken.push(name);
                        }
                    }
                }
                _ => {
                    direct += 1;
                    println!("{name:10} importa direto");
                }
            }
        }
        println!(
            "direto {direct}, convertido {converted}, sem codificador {not_made:?}, vídeos sem decodificador nesta compilação {no_video_decoder:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            broken.is_empty(),
            "não importam nem convertidos: {broken:?}"
        );
    }

    #[test]
    fn a_failed_import_of_an_existing_file_offers_a_conversion_but_reloads_and_results_do_not() {
        let mut s = AppState::new();
        let file = std::env::temp_dir().join("neoveg_convert_offer.bin");
        std::fs::write(&file, b"not media").unwrap();
        let send = |s: &mut AppState, path: &std::path::Path, id: Option<u64>, converted: bool| {
            s.loader_tx
                .send(LoadMsg::Failed {
                    path: path.to_path_buf(),
                    error: "formato desconhecido".into(),
                    existing_id: id,
                    converted,
                })
                .unwrap();
            s.poll_loader();
        };
        // A fresh import that failed: the question appears
        send(&mut s, &file, None, false);
        assert!(s.status.contains("Falha ao importar"));
        assert_eq!(
            s.convert_offer.as_ref().map(|o| o.path.clone()),
            Some(file.clone())
        );
        s.convert_offer = None;
        // The reload of a project's media, the result of a conversion and a file that is gone do not ask
        send(&mut s, &file, Some(3), false);
        assert!(s.convert_offer.is_none());
        send(&mut s, &file, None, true);
        assert!(s.convert_offer.is_none());
        send(&mut s, std::path::Path::new("nao_existe.xyz"), None, false);
        assert!(s.convert_offer.is_none());
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn inserting_an_adjustment_event_needs_a_video_track_and_selects_the_new_event() {
        let mut s = AppState::new();
        // No track: it says what to do and changes nothing
        let revision = s.history.revision();
        assert!(!s.command_enabled(Command::InsertAdjustmentEvent));
        run(&mut s, Command::InsertAdjustmentEvent);
        assert_eq!(s.history.revision(), revision);
        assert!(s.status.contains("faixa de vídeo"));
        // An audio track is not enough
        run(&mut s, Command::InsertAudioTrack);
        assert!(!s.command_enabled(Command::InsertAdjustmentEvent));
        run(&mut s, Command::InsertVideoTrack);
        let video = s
            .timeline
            .tracks
            .iter()
            .find(|t| t.kind == TrackKind::Video)
            .unwrap()
            .id;
        s.selected_track = Some(video);
        s.playhead = 3.0;
        assert!(s.command_enabled(Command::InsertAdjustmentEvent));
        run(&mut s, Command::InsertAdjustmentEvent);
        let event = s.timeline.track(video).unwrap().events[0].clone();
        assert!(event.adjustment && event.start == 3.0);
        assert_eq!(s.selection.primary(), Some((video, event.id)));
        assert!(s.history.undo(&mut s.timeline));
        assert!(s.timeline.track(video).unwrap().events.is_empty());
    }

    /// A mono 16-bit WAV of `seconds` at 8 kHz (silence), written by hand.
    fn write_wav(path: &std::path::Path, seconds: u32) {
        let samples = 8000 * seconds;
        let data_len = samples * 2;
        let mut bytes = Vec::new();
        bytes.extend(b"RIFF");
        bytes.extend((36 + data_len).to_le_bytes());
        bytes.extend(b"WAVEfmt ");
        bytes.extend(16u32.to_le_bytes());
        bytes.extend(1u16.to_le_bytes());
        bytes.extend(1u16.to_le_bytes());
        bytes.extend(8000u32.to_le_bytes());
        bytes.extend(16000u32.to_le_bytes());
        bytes.extend(2u16.to_le_bytes());
        bytes.extend(16u16.to_le_bytes());
        bytes.extend(b"data");
        bytes.extend(data_len.to_le_bytes());
        bytes.extend(std::iter::repeat_n(0u8, data_len as usize));
        std::fs::write(path, bytes).unwrap();
    }

    /// Opens a project whose only media is missing and lets the background search finish.
    fn open_with_missing(
        tag: &str,
        recorded_seconds: f64,
        found_seconds: u32,
    ) -> (AppState, PathBuf) {
        let dir = std::env::temp_dir().join(format!("neoveg_missing_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        write_wav(&dir.join("voz.wav"), found_seconds);
        let mut saved = AppState::new();
        saved.project.media_pool.push(MediaRef {
            subproject: false,
            format: Some(neoveg_common::project::MediaFormat {
                channels: Some(1),
                fps: None,
            }),
            id: 7,
            path: "C:\\antigo\\voz.wav".into(),
            duration_seconds: recorded_seconds,
        });
        let file = dir.join("p.neoveg");
        project_file::save(&file, &saved.project, &saved.timeline).unwrap();
        let mut s = AppState::new();
        let ctx = egui::Context::default();
        assert!(s.load_project_file(&file, Some(file.clone()), &ctx));
        assert_eq!(
            s.missing.len(),
            1,
            "the media is not where the project says"
        );
        // The search runs on a worker thread: poll until it reports
        for _ in 0..200 {
            s.poll_loader();
            if s.missing.iter().all(|m| !m.searching) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        (s, dir)
    }

    #[test]
    fn missing_media_is_found_in_the_project_folder_when_it_matches_what_was_recorded() {
        let (s, dir) = open_with_missing("fits", 1.0, 1);
        assert!(s.missing.is_empty(), "relinked: {:?}", s.status);
        assert_eq!(
            std::path::Path::new(&s.project.media_pool[0].path),
            dir.join("voz.wav")
        );
        assert!(s.is_dirty(), "the project changed");
        assert!(!s.missing_open);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_with_another_length_is_turned_down_and_listed_in_the_missing_media_window() {
        let (mut s, dir) = open_with_missing("wrong", 5.0, 1);
        assert_eq!(s.missing.len(), 1);
        assert!(s.missing_open, "the window opens when something is left");
        let m = &s.missing[0];
        assert_eq!(m.name, "voz.wav");
        assert_eq!(m.rejected.len(), 1);
        assert!(
            m.rejected[0].reason.contains("duração"),
            "{}",
            m.rejected[0].reason
        );
        assert!(
            s.project.media_pool[0].path.contains("antigo"),
            "the entry still points where it did"
        );
        // Choosing it by hand points the entry there; the load then warns about the length
        let chosen = dir.join("voz.wav");
        assert!(relink::relink_to(&mut s.project.media_pool, 7, &chosen));
        s.note_loaded(7, Some(1), None, 1.0);
        assert!(s.status.contains("confira"), "{}", s.status);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_ignored_media_stays_offline_and_does_not_bring_the_window_back() {
        let (mut s, dir) = open_with_missing("ignore", 5.0, 1);
        assert!(s.missing_open, "something is waiting");
        s.missing_open = false;
        // Leave it offline: it stays in the list (the events stay marked) but nobody waits for it
        s.leave_offline(7, &dir.join("voz.wav"), "deixada offline");
        assert_eq!(s.missing.len(), 1);
        assert!(s.missing[0].ignored && !s.missing[0].searching);
        // A later search that finds nothing does not open the window again
        s.apply_relink(
            7,
            Outcome::Unresolved {
                rejected: Vec::new(),
            },
        );
        assert!(!s.missing_open);
        // Finding it later still relinks it and takes it off the list
        s.apply_relink(
            7,
            Outcome::Relinked {
                path: dir.join("voz.wav"),
                how: relink::Match::Exact,
                verified: true,
            },
        );
        assert!(s.missing.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_of_the_project_that_cannot_be_read_is_asked_about_once_and_can_be_ignored_for_good() {
        let mut s = AppState::new();
        let path = std::path::Path::new("C:\\midia\\quebrada.wav");
        // A failed reload of a project's media (an existing id) queues the question once
        s.loader_tx
            .send(LoadMsg::Failed {
                path: path.to_path_buf(),
                error: "formato inválido".into(),
                existing_id: Some(4),
                converted: false,
            })
            .unwrap();
        s.poll_loader();
        s.media_file_failed(4, path, "outra vez".into());
        assert_eq!(s.media_errors.len(), 1, "one question per file");
        assert_eq!(s.media_errors[0].id, 4);
        assert!(
            s.convert_offer.is_none(),
            "a reload is not offered a conversion"
        );
        // After "ignore all", the next unreadable files go offline without a question
        s.ignore_media_errors = true;
        s.media_file_failed(
            5,
            std::path::Path::new("C:\\midia\\outra.wav"),
            "ilegível".into(),
        );
        assert_eq!(s.media_errors.len(), 1);
        let offline = s.missing.iter().find(|m| m.id == 5).unwrap();
        assert!(offline.ignored);
        assert_eq!(offline.rejected[0].reason, "ilegível");
        // Leaving the first one offline too
        s.leave_offline(4, path, "formato inválido");
        assert_eq!(s.missing.len(), 2);
        // Opening another project starts clean
        s.media_errors.clear();
        s.ignore_media_errors = false;
    }

    #[test]
    fn a_found_media_outside_the_project_folder_can_be_copied_next_to_the_project() {
        let base = std::env::temp_dir().join(format!("neoveg_copy_found_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (project_dir, elsewhere) = (base.join("projeto"), base.join("antigo"));
        std::fs::create_dir_all(&project_dir).unwrap();
        std::fs::create_dir_all(&elsewhere).unwrap();
        write_wav(&elsewhere.join("voz.wav"), 1);
        write_wav(&project_dir.join("ja_aqui.wav"), 1);
        let mut s = AppState::new();
        s.project.media_pool.push(MediaRef {
            subproject: false,
            format: None,
            id: 7,
            path: "C:\\perdido\\voz.wav".into(),
            duration_seconds: 1.0,
        });
        // An unsaved project has no folder to copy to: nothing is offered
        s.offer_copy(7, &elsewhere.join("voz.wav"));
        assert!(s.found.is_empty() && !s.found_open);
        s.project_path = Some(project_dir.join("p.neoveg"));
        // A file already in the project's folder is not offered either
        s.offer_copy(7, &project_dir.join("ja_aqui.wav"));
        assert!(s.found.is_empty());
        // A relink to a file elsewhere offers the copy
        s.missing.push(MissingMedia {
            id: 7,
            name: "voz.wav".into(),
            old_path: "C:\\perdido\\voz.wav".into(),
            rejected: Vec::new(),
            searching: true,
            ignored: false,
        });
        s.apply_relink(
            7,
            Outcome::Relinked {
                path: elsewhere.join("voz.wav"),
                how: relink::Match::Exact,
                verified: true,
            },
        );
        assert!(s.found_open);
        assert_eq!(s.found.len(), 1);
        assert_eq!(s.found[0].path, elsewhere.join("voz.wav"));
        // Copy it: the project then points at the copy, the original stays
        let ctx = egui::Context::default();
        s.start_copy(vec![7], &ctx);
        assert!(s.found[0].copying);
        for _ in 0..300 {
            s.poll_loader();
            if s.found.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(s.found.is_empty(), "{}", s.status);
        assert_eq!(
            std::path::Path::new(&s.project.media_pool[0].path),
            project_dir.join("voz.wav")
        );
        assert!(project_dir.join("voz.wav").is_file());
        assert!(
            elsewhere.join("voz.wav").is_file(),
            "the original is not deleted"
        );
        assert!(s.is_dirty());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn buses_are_added_routed_renamed_and_removed_one_undo_step_each() {
        use crate::buses_panel::{BusAction, RouteAction};
        use neoveg_engine::{BusOutput, TrackKind};
        let mut s = AppState::new();
        run(&mut s, Command::InsertAudioTrack);
        let track = s
            .timeline
            .tracks
            .iter()
            .find(|t| t.kind == TrackKind::Audio)
            .unwrap()
            .id;
        // Ctrl+Alt+B (or the menu) adds a bus and shows it in the effects tab
        run(&mut s, Command::InsertAudioBus);
        run(&mut s, Command::InsertAudioBus);
        assert_eq!(s.timeline.buses.len(), 2);
        let (a, b) = (s.timeline.buses[0].id, s.timeline.buses[1].id);
        assert_eq!(s.fx.bus, Some(b), "the new bus is the one edited");
        assert_eq!(s.fx_target().map(|(t, _)| t), Some(FxTarget::Bus(b)));
        // The track goes to A and sends to B
        s.apply_route_actions(
            track,
            vec![
                RouteAction::Output(BusOutput::Bus(a)),
                RouteAction::AddSend(b),
                RouteAction::SendLevel(b, -12.0),
            ],
        );
        let t = s.timeline.track(track).unwrap();
        assert_eq!(t.output, BusOutput::Bus(a));
        assert_eq!(t.sends.len(), 1);
        assert_eq!(t.sends[0].level_db, -12.0);
        // Naming, the fader and a loop that is refused
        s.apply_bus_actions(vec![
            BusAction::Rename(a, "Vozes".into()),
            BusAction::Mix(a, -3.0, 0.5),
            BusAction::Output(a, BusOutput::Bus(b)),
        ]);
        assert_eq!(neoveg_engine::buses::label(&s.timeline, a), "Vozes");
        assert_eq!(s.timeline.buses[0].mix.volume_db, -3.0);
        assert_eq!(s.timeline.buses[0].output, BusOutput::Bus(b));
        let before = s.history.revision();
        s.apply_bus_actions(vec![BusAction::Output(b, BusOutput::Bus(a))]);
        assert_eq!(s.history.revision(), before, "B -> A would close a loop");
        // The effects of a bus are edited like those of a track
        let chain_cmd = fx::add_effect(&s.timeline, FxTarget::Bus(b), "gain").unwrap();
        assert!(s.history.execute(&mut s.timeline, chain_cmd));
        assert_eq!(s.timeline.buses[1].effects.len(), 1);
        // Removing B hands A (and the track's send) on; undo brings everything back
        let snapshot = s.timeline.clone();
        s.apply_bus_actions(vec![BusAction::Remove(b)]);
        assert_eq!(s.timeline.buses.len(), 1);
        assert_eq!(s.timeline.buses[0].output, BusOutput::Master);
        assert!(s.timeline.track(track).unwrap().sends.is_empty());
        assert!(s.history.undo(&mut s.timeline));
        assert_eq!(s.timeline, snapshot);
    }

    /// A project with a video track (a generated clip and a plain one) and an audio track.
    fn header_project(
        tag: &str,
    ) -> (
        AppState,
        TrackId,
        TrackId,
        EventId,
        EventId,
        EventId,
        PathBuf,
    ) {
        let dir = std::env::temp_dir().join(format!("neoveg_header_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest = dir.join("cor.neoveggen");
        let spec = neoveg_video::generators::GeneratorSpec {
            name: "Cor".into(),
            width: 64,
            height: 36,
            fps: 25.0,
            duration: 5.0,
            kind: neoveg_video::generators::GeneratorKind::SolidColor {
                rgba: [10, 20, 30, 255],
            },
        };
        spec.save_manifest(&manifest).unwrap();
        let mut s = AppState::new();
        run(&mut s, Command::InsertVideoTrack);
        run(&mut s, Command::InsertAudioTrack);
        let video = s
            .timeline
            .tracks
            .iter()
            .find(|t| t.kind == TrackKind::Video)
            .unwrap()
            .id;
        let audio = s
            .timeline
            .tracks
            .iter()
            .find(|t| t.kind == TrackKind::Audio)
            .unwrap()
            .id;
        for (id, path) in [
            (1, manifest.to_string_lossy().into_owned()),
            (2, "clip.mp4".into()),
        ] {
            s.project.media_pool.push(MediaRef {
                subproject: false,
                format: None,
                id,
                path,
                duration_seconds: 5.0,
            });
        }
        let mut ids = Vec::new();
        for (track, media, start) in [(video, 1, 0.0), (video, 2, 6.0), (audio, 2, 0.0)] {
            let id = s.timeline.alloc_id();
            let event = TrackEvent {
                id,
                media_id: Some(media),
                name: "ev".into(),
                start,
                duration: 5.0,
                ..TrackEvent::default()
            };
            assert!(s.history.execute(
                &mut s.timeline,
                EditCommand::AddEvent {
                    track_id: track,
                    event
                }
            ));
            ids.push(id);
        }
        s.pixels_per_second = 100.0;
        s.scroll_seconds = 0.0;
        (s, video, audio, ids[0], ids[1], ids[2], dir)
    }

    #[test]
    fn the_header_buttons_are_found_where_they_are_drawn_and_nowhere_else() {
        use event_header::HeaderButton;
        let (s, video, _audio, generated, plain, _sound, dir) = header_project("found");
        let lanes = Rect::from_min_size(pos2(100.0, 50.0), vec2(1200.0, 400.0));
        let lane_top = 60.0;
        let row_rect = |ev_start: f64, row: f32| {
            let x0 = lanes.left() + (ev_start * 100.0) as f32;
            Rect::from_min_max(
                pos2(x0, lane_top + row * TRACK_HEIGHT + 2.0),
                pos2(
                    x0 + 500.0,
                    lane_top + row * TRACK_HEIGHT + TRACK_HEIGHT - 2.0,
                ),
            )
        };
        // The generated clip has all four, the plain video clip three
        let buttons = event_header::buttons_for(TrackKind::Video, true, false);
        let placed = event_header::layout(row_rect(0.0, 0.0), &buttons, EVENT_HEADER_HEIGHT);
        assert_eq!(placed.len(), 4);
        for (button, rect) in &placed {
            let found = header_button_at(&s, lanes, lane_top, rect.center()).unwrap();
            assert_eq!((found.0, found.1, found.2), (video, generated, *button));
        }
        let plain_buttons = event_header::buttons_for(TrackKind::Video, false, false);
        let plain_placed =
            event_header::layout(row_rect(6.0, 0.0), &plain_buttons, EVENT_HEADER_HEIGHT);
        let first = header_button_at(&s, lanes, lane_top, plain_placed[0].1.center()).unwrap();
        assert_eq!((first.1, first.2), (plain, HeaderButton::PanCrop));
        // The body of the event and the empty lane are not buttons
        assert!(header_button_at(&s, lanes, lane_top, pos2(200.0, 100.0)).is_none());
        assert!(header_button_at(&s, lanes, lane_top, pos2(1100.0, 100.0)).is_none());
        // The audio event (the second row) has only FX and the menu
        let sound = event_header::buttons_for(TrackKind::Audio, false, false);
        let sound_placed = event_header::layout(row_rect(0.0, 1.0), &sound, EVENT_HEADER_HEIGHT);
        assert_eq!(sound_placed.len(), 2);
        let hit = header_button_at(&s, lanes, lane_top, sound_placed[0].1.center()).unwrap();
        assert_eq!(hit.2, HeaderButton::Fx);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn each_header_button_selects_the_event_and_does_its_job() {
        use event_header::HeaderButton;
        let (mut s, video, audio, generated, plain, sound, dir) = header_project("jobs");
        let at = pos2(300.0, 70.0);
        // FX of a video event: the video effects tab, on the Event scope
        s.header_button_clicked(video, plain, HeaderButton::Fx, at);
        assert_eq!(s.selection.primary(), Some((video, plain)));
        assert_eq!(s.vfx.scope, VfxScope::Event);
        assert_eq!(s.window_request, Some(WindowAction::Show(Tab::VideoFx)));
        // FX of an audio event: the effects of its track
        s.fx.master = true;
        s.header_button_clicked(audio, sound, HeaderButton::Fx, at);
        assert_eq!(s.selected_track, Some(audio));
        assert!(!s.fx.master && s.fx.bus.is_none());
        assert_eq!(s.window_request, Some(WindowAction::Show(Tab::AudioFx)));
        // The menu asks for a right click where the button is
        s.header_button_clicked(video, plain, HeaderButton::Menu, at);
        assert_eq!(s.pending_context_click, Some(at));
        // Generated media opens the dialog on its values
        s.header_button_clicked(video, generated, HeaderButton::GeneratedMedia, at);
        assert!(s.generator_dialog.open);
        assert_eq!(s.generator_dialog.editing.as_ref().map(|e| e.0), Some(1));
        assert_eq!(s.generator_dialog.solid_rgba, [10, 20, 30, 255]);
        // Applying it writes a new manifest, repoints the media and leaves the old file
        let old = s.project.media_pool[0].path.clone();
        let (_, spec) = s.generator_dialog.editing.clone().unwrap();
        let mut edited = spec.clone();
        edited.kind = neoveg_video::generators::GeneratorKind::SolidColor {
            rgba: [200, 0, 0, 255],
        };
        s.apply_generated_edit(1, edited, &egui::Context::default());
        let new = s.project.media_pool[0].path.clone();
        assert_ne!(new, old);
        assert!(
            std::path::Path::new(&old).is_file(),
            "the old manifest stays"
        );
        let saved =
            neoveg_video::generators::GeneratorSpec::load_manifest(std::path::Path::new(&new))
                .unwrap();
        assert_eq!(
            saved.kind,
            neoveg_video::generators::GeneratorKind::SolidColor {
                rgba: [200, 0, 0, 255]
            }
        );
        assert!(s.is_dirty());
        // A video event that is not generated has no such button
        assert!(!s.is_generated_event(
            s.timeline
                .track(video)
                .unwrap()
                .events
                .iter()
                .find(|e| e.id == plain)
                .unwrap()
        ));
        let _ = std::fs::remove_dir_all(&dir);
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

    #[test]
    fn mute_and_solo_shortcuts_act_on_the_selected_track_and_filter_build_clips() {
        let (mut s, first, _) = state_with_clip();
        s.add_track(TrackKind::Audio);
        let second = s.selected_track.unwrap();
        add_event(&mut s, second, 0.0, 2.0);
        s.media.insert(1, test_media());

        assert_eq!(s.build_clips().len(), 2);
        // Solo on `second`: only `second` plays
        run(&mut s, Command::TrackSolo);
        assert!(s.timeline.track(second).unwrap().solo);
        let clips = s.build_clips();
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].track, 1);

        // Exclusive solo on `first`: swaps solo in one undo step
        s.selected_track = Some(first);
        let rev = s.history.revision();
        run(&mut s, Command::TrackSoloExclusive);
        assert_eq!(s.history.revision(), rev + 1);
        assert!(s.timeline.track(first).unwrap().solo && !s.timeline.track(second).unwrap().solo);
        assert_eq!(s.build_clips()[0].track, 0);

        // Mute overrides solo on the same track
        run(&mut s, Command::TrackMute);
        assert!(s.build_clips().is_empty());

        // Exclusive mute on `second`: clears `first`'s mute and mutes `second`
        s.selected_track = Some(second);
        run(&mut s, Command::TrackMuteExclusive);
        assert!(!s.timeline.track(first).unwrap().muted && s.timeline.track(second).unwrap().muted);

        run(&mut s, Command::EditUndo);
        assert!(s.timeline.track(first).unwrap().muted && !s.timeline.track(second).unwrap().muted);
    }

    #[test]
    fn mixer_actions_update_track_fader_pan_phase_and_pan_law_with_undo() {
        let (mut s, track, _) = state_with_clip();
        let rev = s.history.revision();
        s.apply_mix_actions(
            track,
            vec![
                MixAction::Volume(-6.0),
                MixAction::Pan(0.4),
                MixAction::Phase(true),
                MixAction::PanLaw(neoveg_common::pan::PanLaw::ConstantPower),
            ],
        );
        assert_eq!(s.history.revision(), rev + 4);
        let plan = fx_panel::fx_plan(&s.timeline);
        assert_eq!((plan.mix[0].pan, plan.mix[0].invert), (0.4, true));
        assert_eq!(plan.pan_law, neoveg_common::pan::PanLaw::ConstantPower);

        run(&mut s, Command::EditUndo);
        assert_eq!(s.timeline.pan_law, neoveg_common::pan::PanLaw::Balance);
    }

    #[test]
    fn header_controls_hit_test_audio_and_video_rows() {
        let row = Rect::from_min_size(pos2(0.0, 100.0), vec2(HEADER_WIDTH, TRACK_HEIGHT));
        assert_eq!(
            hit_header_control(row, TrackKind::Audio, pos2(12.0, 142.0)),
            Some(HeaderControl::Mute)
        );
        assert_eq!(
            hit_header_control(row, TrackKind::Audio, pos2(35.0, 142.0)),
            Some(HeaderControl::Solo)
        );
        assert_eq!(
            hit_header_control(row, TrackKind::Audio, pos2(58.0, 142.0)),
            Some(HeaderControl::Phase)
        );
        assert_eq!(
            hit_header_control(row, TrackKind::Video, pos2(12.0, 142.0)),
            Some(HeaderControl::Mute)
        );
        assert_eq!(
            hit_header_control(row, TrackKind::Video, pos2(35.0, 142.0)),
            None,
            "video headers have no solo button"
        );
    }

    #[test]
    fn ctrl_edge_stretch_scales_playback_rate_and_builds_stretched_mix_clip_with_undo() {
        let (mut s, track, ev) = state_with_clip(); // 2.0 .. 6.0 (4 s)
        s.project.settings.fps = 25.0;
        s.media.insert(1, test_media());

        s.begin_drag(hit(&s, track, ev, DragMode::StretchRight), 6.0);
        // Stretch right edge from 6.0 to 10.0 -> 8.0 s duration -> 0.5x rate
        s.update_drag(10.0, None, 0.1, false);
        s.finish_drag();

        let g = geometry(&s, track, ev);
        assert!((g.duration - 8.0).abs() < 1e-6);
        assert!((g.playback_rate - 0.5).abs() < 1e-6);
        assert!(g.preserve_pitch);

        let clips = s.build_clips();
        assert_eq!(clips.len(), 1);
        assert!((clips[0].duration - 8.0).abs() < 1e-6);
        assert_eq!(clips[0].trim_in, 0.0);
        assert!(!clips[0].samples.is_empty());

        // Single undo restores 1.0x rate and 4.0 s duration
        s.undo();
        let restored = geometry(&s, track, ev);
        assert!((restored.duration - 4.0).abs() < 1e-6);
        assert!((restored.playback_rate - 1.0).abs() < 1e-6);
    }
}

#[cfg(test)]
mod ripple_tests {
    use super::*;

    fn state_with_three_events() -> (AppState, TrackId, Vec<EventId>) {
        let mut s = AppState::new();
        let mut track = s.timeline.new_track(TrackKind::Audio);
        let mut ids = Vec::new();
        for (start, duration) in [(0.0, 2.0), (2.0, 3.0), (6.0, 1.0)] {
            let event = TrackEvent {
                id: s.timeline.alloc_id(),
                start,
                duration,
                media_id: Some(1),
                ..TrackEvent::default()
            };
            ids.push(event.id);
            track.events.push(event);
        }
        let id = track.id;
        s.timeline.tracks.push(track);
        (s, id, ids)
    }

    fn starts(s: &AppState) -> Vec<f64> {
        s.timeline.tracks[0]
            .events
            .iter()
            .map(|e| e.start)
            .collect()
    }

    #[test]
    fn ripple_delete_closes_the_gap_in_one_undo_step() {
        let (mut s, track, ids) = state_with_three_events();
        s.selection.select_only((track, ids[1]));
        s.ripple_delete_selection();
        assert_eq!(starts(&s), vec![0.0, 3.0]);
        assert!(s.selection.is_empty());
        s.undo();
        assert_eq!(starts(&s), vec![0.0, 2.0, 6.0]);
        // Nothing selected: a hint, no change
        s.ripple_delete_selection();
        assert!(s.status.contains("Selecione"));
        assert_eq!(starts(&s), vec![0.0, 2.0, 6.0]);
    }

    #[test]
    fn plain_delete_only_ripples_while_auto_ripple_is_on() {
        let (mut s, track, ids) = state_with_three_events();
        s.selection.select_only((track, ids[0]));
        s.delete_selection();
        assert_eq!(starts(&s), vec![2.0, 6.0], "a plain delete leaves the gap");
        s.undo();
        s.auto_ripple = true;
        s.selection.select_only((track, ids[0]));
        s.delete_selection();
        assert_eq!(starts(&s), vec![0.0, 4.0], "auto ripple closes it");
    }

    #[test]
    fn ripple_scope_commands_choose_what_a_ripple_delete_carries() {
        let (mut s, track, ids) = state_with_three_events();
        s.timeline.markers.push(neoveg_engine::Marker {
            id: 1,
            time: 6.0,
            name: "m".into(),
        });
        assert_eq!(s.ripple_scope, RippleScope::AffectedTracks);
        s.selection.select_only((track, ids[1]));
        s.ripple_delete_selection();
        assert_eq!(
            s.timeline.markers[0].time, 6.0,
            "default scope leaves marks"
        );
        s.undo();

        s.set_ripple_scope(RippleScope::AffectedTracksAndMarks);
        assert!(s.status.contains("marcadores"));
        s.selection.select_only((track, ids[1]));
        s.ripple_delete_selection();
        assert_eq!(s.timeline.markers[0].time, 3.0, "marker followed the gap");
        s.undo();
        assert_eq!(s.timeline.markers[0].time, 6.0, "one undo step restores it");
    }
}
