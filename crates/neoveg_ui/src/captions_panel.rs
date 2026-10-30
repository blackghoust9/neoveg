//! Subtitles: the "Legendas" tab of the media panel and the text drawn over the video preview.
//!
//! The panel lists the captions of the project, lets the user edit their text and times, add
//! them at the cursor, import or export SubRip / WebVTT files, transcribe timeline speech locally
//! with Whisper (`neoveg_audio::whisper`), and jump to one. It never touches the project: it
//! returns [`CaptionAction`]s and the application turns each one into one undoable command
//! (`neoveg_engine::captions`).

use neoveg_common::i18n::tr;
use neoveg_common::i18n::tr_args;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc};

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect};
use neoveg_audio::mixer::MixClip;
use neoveg_audio::whisper::{self, TranscriptSegment};
use neoveg_engine::{Caption, CaptionId};

use crate::icons::{self, Icon};

/// Shortest length a caption may be given by hand, seconds.
const MIN_LENGTH: f64 = 0.05;
/// Length of a caption added at the cursor, seconds.
pub const NEW_CAPTION_SECONDS: f64 = 2.0;
/// Characters per line in the preview before a long line is wrapped.
const WRAP_CHARS: usize = 42;

#[derive(Debug, Clone, PartialEq)]
pub enum CaptionAction {
    /// A new, empty caption at the cursor.
    Add,
    Remove(CaptionId),
    SetText(CaptionId, String),
    SetTimes(CaptionId, f64, f64),
    /// Move the cursor to a time.
    Seek(f64),
    Import,
    Export,
    Clear,
    /// Open a folder picker to select a local Whisper model folder.
    PickWhisperModel,
    /// Start background Whisper speech-to-subtitles transcription of the timeline audio.
    StartWhisper,
    /// Commit completed Whisper segments to the timeline as a single undoable command.
    ApplyWhisper {
        segments: Vec<TranscriptSegment>,
        replace_all: bool,
    },
}

struct WhisperJob {
    rx: mpsc::Receiver<Result<Vec<TranscriptSegment>, String>>,
    cancel: Arc<AtomicBool>,
    progress_bits: Arc<AtomicU32>,
    replace_all: bool,
}

impl std::fmt::Debug for WhisperJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WhisperJob")
            .field(
                "progress",
                &f32::from_bits(self.progress_bits.load(Ordering::Relaxed)),
            )
            .field("replace_all", &self.replace_all)
            .finish()
    }
}

#[derive(Debug)]
pub struct CaptionsPanelState {
    /// Text being edited, until the box loses focus.
    text_drafts: HashMap<CaptionId, String>,
    /// Start and end being dragged, until the drag ends.
    time_drafts: HashMap<CaptionId, (f64, f64)>,
    /// Selected Whisper model directory (`config.json`, `tokenizer.json`, `model.safetensors` / `.gguf`).
    pub whisper_model_dir: Option<PathBuf>,
    /// Whisper language code (`"pt"`, `"en"`, ..., or `"auto"`).
    pub whisper_language: String,
    /// Whether Whisper output replaces existing project captions instead of appending.
    pub whisper_replace_all: bool,
    /// Latest status or error message from Whisper transcription.
    pub whisper_status: Option<String>,
    whisper_job: Option<WhisperJob>,
}

impl Default for CaptionsPanelState {
    fn default() -> Self {
        Self {
            text_drafts: HashMap::new(),
            time_drafts: HashMap::new(),
            whisper_model_dir: default_whisper_model_dir(),
            whisper_language: "pt".into(),
            whisper_replace_all: false,
            whisper_status: None,
            whisper_job: None,
        }
    }
}

fn default_whisper_model_dir() -> Option<PathBuf> {
    if let Some(env_dir) = std::env::var_os("NEOVEG_WHISPER_MODEL") {
        let p = PathBuf::from(env_dir);
        if p.exists() {
            return Some(p);
        }
    }
    neoveg_common::paths::app_data_dir()
        .map(|d| d.join("models").join("whisper"))
        .filter(|d| d.join("config.json").is_file())
}

impl CaptionsPanelState {
    pub fn is_transcribing(&self) -> bool {
        self.whisper_job.is_some()
    }

    pub fn whisper_progress(&self) -> Option<f32> {
        self.whisper_job
            .as_ref()
            .map(|j| f32::from_bits(j.progress_bits.load(Ordering::Relaxed)).clamp(0.0, 1.0))
    }

    pub fn cancel_whisper(&mut self) {
        if let Some(job) = &self.whisper_job {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// Launches a background worker thread running pure-Rust Whisper over `clips` (`0..duration_s`).
    pub fn start_whisper(
        &mut self,
        clips: Vec<MixClip>,
        duration_s: f64,
        on_wakeup: impl Fn() + Send + Sync + 'static,
    ) -> Result<(), String> {
        if self.whisper_job.is_some() {
            return Err("Já existe uma transcrição Whisper em andamento.".into());
        }
        let Some(model_dir) = self.whisper_model_dir.clone() else {
            let msg = "Selecione primeiro uma pasta de modelo Whisper.".to_owned();
            self.whisper_status = Some(msg.clone());
            return Err(msg);
        };
        if clips.is_empty() || duration_s <= 0.0 {
            let msg = "Nenhum evento de áudio ativo na linha do tempo para transcrever.".to_owned();
            self.whisper_status = Some(msg.clone());
            return Err(msg);
        }

        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let progress_bits = Arc::new(AtomicU32::new(0.0_f32.to_bits()));
        let cancel_worker = Arc::clone(&cancel);
        let progress_worker = Arc::clone(&progress_bits);
        let lang = self.whisper_language.clone();
        let replace_all = self.whisper_replace_all;
        let wakeup = Arc::new(on_wakeup);

        let spawn_res = std::thread::Builder::new()
            .name("neoveg-whisper".into())
            .spawn(move || {
                let wakeup_progress = Arc::clone(&wakeup);
                let result = whisper::transcribe_clips(
                    &model_dir,
                    &clips,
                    duration_s,
                    Some(&lang),
                    &cancel_worker,
                    move |p| {
                        progress_worker.store(p.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
                        wakeup_progress();
                    },
                );
                let _ = tx.send(result);
                wakeup();
            });

        if let Err(e) = spawn_res {
            let msg = tr_args("Falha ao iniciar thread Whisper: {}", &[&e]);
            self.whisper_status = Some(msg.clone());
            return Err(msg);
        }

        self.whisper_status = Some("Transcrevendo áudio com Whisper…".into());
        self.whisper_job = Some(WhisperJob {
            rx,
            cancel,
            progress_bits,
            replace_all,
        });
        Ok(())
    }

    /// Polls the background Whisper worker without blocking; returns [`CaptionAction::ApplyWhisper`]
    /// when transcription succeeds.
    pub fn poll_whisper(&mut self) -> Option<CaptionAction> {
        let outcome = self.whisper_job.as_ref()?.rx.try_recv().ok()?;
        let replace_all = self
            .whisper_job
            .take()
            .map(|j| j.replace_all)
            .unwrap_or(false);

        match outcome {
            Ok(segments) => {
                if segments.is_empty() {
                    self.whisper_status = Some("Whisper concluído: nenhuma fala detectada.".into());
                    None
                } else {
                    self.whisper_status = Some(tr_args(
                        "Whisper: {} legenda(s) transcrita(s).",
                        &[&segments.len()],
                    ));
                    Some(CaptionAction::ApplyWhisper {
                        segments,
                        replace_all,
                    })
                }
            }
            Err(err) => {
                self.whisper_status = Some(err);
                None
            }
        }
    }
}

// ---- pure helpers -------------------------------------------------------------------------------

/// The text of a caption as shown: inline tags (`<i>`, `<b>`, `<v Name>`, `<00:01.000>`) removed and
/// the common character entities decoded.
pub fn plain_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_tag = false;
    for c in text.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// The lines to draw for a caption: its own lines, long ones wrapped at spaces.
pub fn display_lines(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for raw in plain_text(text).lines() {
        let mut line = String::new();
        for word in raw.split_whitespace() {
            if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > max_chars {
                lines.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        if !line.is_empty() {
            lines.push(line);
        }
    }
    lines
}

/// A start and end that make a valid caption: not before zero, at least [`MIN_LENGTH`] long.
pub fn valid_times(start: f64, end: f64) -> (f64, f64) {
    let start = start.max(0.0);
    (start, end.max(start + MIN_LENGTH))
}

/// Index of the caption showing at `time`, if any.
pub fn current_index(captions: &[Caption], time: f64) -> Option<usize> {
    captions
        .iter()
        .position(|c| time >= c.start && time < c.end)
}

// ---- drawing ------------------------------------------------------------------------------------------

/// Draws the captions showing at `time` over `frame` (the video picture): white text with a dark
/// outline, at the bottom, sized with the picture.
pub fn paint_overlay(painter: &egui::Painter, frame: Rect, active: &[&Caption]) {
    let size = (frame.height() * 0.052).clamp(10.0, 48.0);
    let font = FontId::proportional(size);
    let mut lines: Vec<String> = Vec::new();
    for caption in active {
        lines.extend(display_lines(&caption.text, WRAP_CHARS));
    }
    let line_height = size * 1.18;
    let mut y = frame.bottom() - frame.height() * 0.06 - line_height * lines.len() as f32;
    for line in lines {
        let at = pos2(frame.center().x, y);
        for (dx, dy) in [
            (-1.5, 0.0),
            (1.5, 0.0),
            (0.0, -1.5),
            (0.0, 1.5),
            (1.2, 1.2),
            (-1.2, -1.2),
        ] {
            painter.text(
                at + vec2(dx, dy),
                Align2::CENTER_TOP,
                &line,
                font.clone(),
                Color32::from_black_alpha(230),
            );
        }
        painter.text(at, Align2::CENTER_TOP, &line, font.clone(), Color32::WHITE);
        y += line_height;
    }
}

/// The captions tab. `playhead` highlights the caption under the cursor.
pub fn captions_ui(
    ui: &mut egui::Ui,
    panel: &mut CaptionsPanelState,
    captions: &[Caption],
    playhead: f64,
) -> Vec<CaptionAction> {
    let mut actions = Vec::new();
    if let Some(done_action) = panel.poll_whisper() {
        actions.push(done_action);
    }

    ui.horizontal(|ui| {
        if icons::icon_button(ui, Icon::Add, tr("Adicionar uma legenda no cursor"), false).clicked()
        {
            actions.push(CaptionAction::Add);
        }
        if icons::icon_button(
            ui,
            Icon::Import,
            tr("Importar legendas (SRT ou WebVTT)…"),
            false,
        )
        .clicked()
        {
            actions.push(CaptionAction::Import);
        }
        let any = !captions.is_empty();
        ui.add_enabled_ui(any, |ui| {
            if icons::icon_button(
                ui,
                Icon::Export,
                tr("Exportar legendas (SRT ou WebVTT)…"),
                false,
            )
            .clicked()
            {
                actions.push(CaptionAction::Export);
            }
            if icons::icon_button(ui, Icon::Remove, tr("Apagar todas as legendas"), false).clicked()
            {
                actions.push(CaptionAction::Clear);
            }
        });
        ui.weak(tr_args("{} legenda(s)", &[&captions.len()]));
    });

    ui.horizontal_wrapped(|ui| {
        let running = panel.is_transcribing();
        ui.add_enabled_ui(!running, |ui| {
            if icons::icon_button(
                ui,
                Icon::Import,
                tr("Selecionar pasta do modelo Whisper (config.json, tokenizer.json, model.safetensors/.gguf)…"),
                panel.whisper_model_dir.is_some(),
            )
            .clicked()
            {
                actions.push(CaptionAction::PickWhisperModel);
            }

            let selected_label = if panel.whisper_language == "auto" {
                "Auto".to_owned()
            } else {
                whisper::SUPPORTED_LANGUAGES
                    .iter()
                    .find(|(code, _)| *code == panel.whisper_language)
                    .map(|(code, name)| format!("{name} ({code})"))
                    .unwrap_or_else(|| panel.whisper_language.clone())
            };

            egui::ComboBox::from_id_salt("whisper_lang_combo")
                .width(122.0)
                .selected_text(selected_label)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut panel.whisper_language,
                        "auto".to_owned(),
                        tr("Detectar idioma (Auto)"),
                    );
                    for &(code, name) in whisper::SUPPORTED_LANGUAGES {
                        ui.selectable_value(
                            &mut panel.whisper_language,
                            code.to_owned(),
                            format!("{name} ({code})"),
                        );
                    }
                });

            ui.checkbox(&mut panel.whisper_replace_all, tr("Substituir"))
                .on_hover_text(tr("Substituir as legendas existentes ao concluir a transcrição"));
        });

        if let Some(progress) = panel.whisper_progress() {
            ui.add(
                egui::ProgressBar::new(progress)
                    .show_percentage()
                    .desired_width(105.0),
            );
            if icons::icon_button(ui, Icon::Stop, tr("Cancelar transcrição Whisper"), false).clicked()
            {
                panel.cancel_whisper();
            }
        } else {
            let has_model = panel.whisper_model_dir.is_some();
            ui.add_enabled_ui(has_model, |ui| {
                if icons::icon_button(
                    ui,
                    Icon::Record,
                    tr("Transcrever fala da linha do tempo com Whisper (Rust puro)"),
                    false,
                )
                .clicked()
                {
                    actions.push(CaptionAction::StartWhisper);
                }
            });
        }

        let model_hint = panel
            .whisper_model_dir
            .as_deref()
            .and_then(|p| p.file_name().or_else(|| p.as_os_str().into()))
            .map(|n| tr_args("Whisper: {}", &[&n.to_string_lossy()]))
            .unwrap_or_else(|| "Whisper: selecione a pasta do modelo".into());
        ui.weak(model_hint);
    });

    if let Some(status) = &panel.whisper_status {
        ui.weak(status);
    }

    ui.separator();
    if captions.is_empty() {
        ui.weak(tr("Nenhuma legenda. Adicione uma no cursor, importe SRT/WebVTT ou transcreva com Whisper."));
        return actions;
    }

    // Drafts of captions that no longer exist (removed, undone) are dropped
    panel
        .text_drafts
        .retain(|id, _| captions.iter().any(|c| c.id == *id));
    panel
        .time_drafts
        .retain(|id, _| captions.iter().any(|c| c.id == *id));
    let current = current_index(captions, playhead);

    egui::ScrollArea::vertical().show(ui, |ui| {
        for (i, caption) in captions.iter().enumerate() {
            caption_row(ui, panel, caption, current == Some(i), &mut actions);
            ui.separator();
        }
    });
    actions
}

fn caption_row(
    ui: &mut egui::Ui,
    panel: &mut CaptionsPanelState,
    caption: &Caption,
    current: bool,
    actions: &mut Vec<CaptionAction>,
) {
    let id = caption.id;
    ui.horizontal(|ui| {
        if icons::icon_button_sized(ui, Icon::Play, tr("Ir para esta legenda"), current, 22.0)
            .clicked()
        {
            actions.push(CaptionAction::Seek(caption.start));
        }
        let (mut start, mut end) = panel
            .time_drafts
            .get(&id)
            .copied()
            .unwrap_or((caption.start, caption.end));
        let mut finished = false;
        let mut changed = false;
        for (label, value) in [("de", &mut start), ("até", &mut end)] {
            ui.label(label);
            let response = ui.add(
                egui::DragValue::new(value)
                    .speed(0.02)
                    .range(0.0..=36_000.0)
                    .fixed_decimals(3)
                    .suffix(" s"),
            );
            changed |= response.changed();
            finished |= response.drag_stopped() || response.lost_focus();
        }
        if changed {
            panel.time_drafts.insert(id, (start, end));
        }
        if finished {
            panel.time_drafts.remove(&id);
            let (s, e) = valid_times(start, end);
            if (s - caption.start).abs() > 1e-6 || (e - caption.end).abs() > 1e-6 {
                actions.push(CaptionAction::SetTimes(id, s, e));
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icons::icon_button_sized(ui, Icon::Remove, tr("Apagar esta legenda"), false, 22.0)
                .clicked()
            {
                actions.push(CaptionAction::Remove(id));
            }
        });
    });
    let mut text = panel
        .text_drafts
        .get(&id)
        .cloned()
        .unwrap_or_else(|| caption.text.clone());
    let response = ui.add(
        egui::TextEdit::multiline(&mut text)
            .desired_rows(2)
            .desired_width(f32::INFINITY)
            .hint_text(tr("Texto da legenda")),
    );
    if response.changed() {
        panel.text_drafts.insert(id, text.clone());
    }
    if response.lost_focus() {
        panel.text_drafts.remove(&id);
        if text != caption.text {
            actions.push(CaptionAction::SetText(id, text));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caption(id: u64, start: f64, end: f64, text: &str) -> Caption {
        Caption {
            id,
            start,
            end,
            text: text.into(),
            speaker: None,
        }
    }

    #[test]
    fn markup_is_removed_and_entities_decoded() {
        assert_eq!(plain_text("<i>Olá</i>, <b>mundo</b>"), "Olá, mundo");
        assert_eq!(plain_text("<v Ana>Oi &amp; tchau"), "Oi & tchau");
        assert_eq!(plain_text("a &lt;b&gt; c&nbsp;d"), "a <b> c d");
        assert_eq!(plain_text("<00:01.000>palavra"), "palavra");
        assert_eq!(plain_text("sem marcação"), "sem marcação");
        // An unclosed tag swallows the rest instead of showing raw markup
        assert_eq!(plain_text("texto <i"), "texto ");
    }

    #[test]
    fn long_lines_wrap_at_spaces_and_own_lines_are_kept() {
        let lines = display_lines("um dois três quatro cinco seis sete", 15);
        assert_eq!(lines, ["um dois três", "quatro cinco", "seis sete"]);
        assert!(lines.iter().all(|l| l.chars().count() <= 15));
        assert_eq!(
            display_lines("primeira\nsegunda", 40),
            ["primeira", "segunda"]
        );
        assert!(display_lines("   \n  ", 40).is_empty());
        // A word longer than the limit stays whole
        assert_eq!(
            display_lines("supercalifragilístico", 5),
            ["supercalifragilístico"]
        );
    }

    #[test]
    fn hand_edited_times_stay_valid() {
        assert_eq!(valid_times(1.0, 3.0), (1.0, 3.0));
        assert_eq!(valid_times(-2.0, 1.0), (0.0, 1.0));
        let (s, e) = valid_times(5.0, 5.0);
        assert!(e - s >= MIN_LENGTH - 1e-9);
        let (s, e) = valid_times(5.0, 2.0);
        assert!(e > s);
    }

    #[test]
    fn the_caption_under_the_cursor_is_found() {
        let list = [caption(1, 1.0, 2.0, "a"), caption(2, 3.0, 4.0, "b")];
        assert_eq!(current_index(&list, 0.5), None);
        assert_eq!(current_index(&list, 1.0), Some(0));
        assert_eq!(current_index(&list, 2.0), None, "the end is exclusive");
        assert_eq!(current_index(&list, 3.5), Some(1));
    }

    #[test]
    fn the_overlay_draws_every_line_with_its_outline_and_nothing_without_captions() {
        let ctx = egui::Context::default();
        let frame = Rect::from_min_size(pos2(100.0, 50.0), vec2(640.0, 360.0));
        let first = caption(
            1,
            0.0,
            2.0,
            "primeira linha
segunda linha",
        );
        let second = caption(2, 0.0, 2.0, "<i>outra</i> legenda");
        let texts = |shown: &[&Caption]| {
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                let painter = ctx.layer_painter(egui::LayerId::background());
                paint_overlay(&painter, frame, shown);
            });
            output
                .shapes
                .iter()
                .filter(|s| matches!(s.shape, egui::Shape::Text(_)))
                .count()
        };
        // Three lines, each drawn six times for the outline and once for the white text
        assert_eq!(texts(&[&first, &second]), 3 * 7);
        assert_eq!(texts(&[]), 0);
    }

    #[test]
    fn whisper_panel_state_rejects_missing_model_and_delivers_completed_segments() {
        let mut state = CaptionsPanelState {
            whisper_model_dir: None,
            ..Default::default()
        };
        assert!(state.start_whisper(Vec::new(), 0.0, || {}).is_err());

        let (tx, rx) = mpsc::channel();
        state.whisper_job = Some(WhisperJob {
            rx,
            cancel: Arc::new(AtomicBool::new(false)),
            progress_bits: Arc::new(AtomicU32::new(0.5_f32.to_bits())),
            replace_all: true,
        });
        assert!(state.is_transcribing());
        assert!((state.whisper_progress().unwrap() - 0.5).abs() < 1e-6);

        tx.send(Ok(vec![TranscriptSegment {
            start_s: 1.0,
            end_s: 2.5,
            text: "Teste Whisper".into(),
        }]))
        .unwrap();

        let action = state.poll_whisper();
        assert!(!state.is_transcribing());
        assert_eq!(
            action,
            Some(CaptionAction::ApplyWhisper {
                segments: vec![TranscriptSegment {
                    start_s: 1.0,
                    end_s: 2.5,
                    text: "Teste Whisper".into(),
                }],
                replace_all: true,
            })
        );
    }
}
