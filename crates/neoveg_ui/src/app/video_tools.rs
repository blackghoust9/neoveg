//! Glue between the app and the video tools windows: the color correction of the selected event
//! (automatic color, export, import), the reference frame gallery and the lane mark for events
//! without a correction. The logic lives in `event_color_window` and `ref_gallery_window`.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use eframe::egui;
use neoveg_common::i18n::{tr, tr_args};
use neoveg_engine::{EventId, TrackId, TrackKind};
use neoveg_video::frame::FrameBuffer;

use super::AppState;
use crate::event_color_window::{self as color, ColorAction, ColorView, EventColorWindow};
use crate::ref_gallery_window::RefGalleryWindow;

/// A stabilization running on a worker thread.
pub(super) struct StabilizeJob {
    output: PathBuf,
    cancel: Arc<AtomicBool>,
    /// `(done, total)` and, once over, how it ended.
    state: Arc<Mutex<((u64, u64), Option<Result<(), String>>)>>,
}

/// State of the video tools windows.
#[derive(Default)]
pub(super) struct VideoTools {
    pub color: EventColorWindow,
    pub gallery: RefGalleryWindow,
    /// Exibir > Destacar eventos sem correção de cor.
    pub highlight_uncorrected: bool,
    frame: Option<FrameBuffer>,
    frame_version: u64,
    stabilize: Option<StabilizeJob>,
    pub storyboard: crate::storyboard_window::StoryboardWindow,
    /// Exibir > 3D estereoscópico: how the preview shows a left/right pair.
    pub stereo_layout: neoveg_video::stereo3d::StereoLayout,
    pub stereo_output: neoveg_video::stereo3d::StereoOutput,
    /// Exibir > desentrelaçar, girar e espelhar a prévia (see `view_mode_ui`).
    pub view_mode: neoveg_video::view_mode::ViewMode,
    /// Pulldown conversion running (see `pulldown_tools`).
    pub(super) pulldown: Option<super::pulldown_tools::PulldownJob>,
}

impl VideoTools {
    /// Keeps a copy of the newest preview frame, only while a window that reads it is open.
    pub fn keep_frame(&mut self, frame: &FrameBuffer, version: u64) {
        if self.color.open || self.gallery.open {
            self.frame = Some(frame.clone());
            self.frame_version = version;
        }
    }
}

impl AppState {
    /// The selected video event the color window works on.
    fn selected_video_event(&self) -> Option<(TrackId, EventId, String)> {
        let (track_id, event_id) = self.selection.primary()?;
        let track = self.timeline.track(track_id)?;
        if track.kind != TrackKind::Video {
            return None;
        }
        let event = track.events.iter().find(|e| e.id == event_id)?;
        Some((track_id, event_id, event.name.clone()))
    }

    /// Opens the color window and asks for a fresh preview frame.
    pub(super) fn open_event_color_window(&mut self) {
        self.video_tools.color.open = true;
        self.last_frame_key = None;
    }

    /// Opens the reference gallery and asks for a fresh preview frame.
    pub(super) fn open_ref_gallery_window(&mut self) {
        self.video_tools.gallery.open = true;
        self.last_frame_key = None;
    }

    pub(super) fn gallery_dir(&self) -> PathBuf {
        let base = self
            .project_path
            .as_ref()
            .and_then(|p| p.parent().map(PathBuf::from))
            .or_else(|| std::env::current_exe().ok().and_then(|e| e.parent().map(PathBuf::from)))
            .unwrap_or_default();
        base.join("quadros_de_referencia")
    }

    fn apply_color_edit(&mut self, result: Result<neoveg_engine::EditCommand, String>, done: &str) {
        match result {
            Ok(command) => {
                self.history.execute(&mut self.timeline, command);
                self.last_frame_key = None;
                self.status = done.to_string();
            }
            Err(e) => self.status = e,
        }
    }

    pub(super) fn run_video_tools(&mut self, ctx: &egui::Context) {
        if self.video_tools.color.open {
            let selected = self.selected_video_event();
            let has_correction = selected.as_ref().is_some_and(|(t, e, _)| {
                self.timeline
                    .track(*t)
                    .and_then(|tr| tr.events.iter().find(|ev| ev.id == *e))
                    .is_some_and(neoveg_engine::color_correction::has_color_correction)
            });
            let view = ColorView {
                event_name: selected.as_ref().map(|s| s.2.clone()),
                has_correction,
                has_frame: self.video_tools.frame.is_some(),
                stabilizing: self
                    .video_tools
                    .stabilize
                    .as_ref()
                    .and_then(|j| j.state.lock().ok().map(|s| s.0)),
            };
            let actions = self.video_tools.color.show(ctx, &view);
            if let Some((track_id, event_id, _)) = selected {
                for action in actions {
                    self.run_color_action(action, track_id, event_id);
                }
            }
        }
        self.poll_stabilize(ctx);
        self.poll_pulldown(ctx);
        if self.video_tools.storyboard.open {
            let track = self.selected_video_event().map(|s| s.0).or_else(|| {
                self.timeline
                    .tracks
                    .iter()
                    .find(|t| t.kind == TrackKind::Video)
                    .map(|t| t.id)
            });
            if let Some(edit) = self.video_tools.storyboard.show(ctx, &self.timeline, track) {
                if self.history.execute(&mut self.timeline, edit) {
                    self.last_frame_key = None;
                    self.status = tr("Ordem do storyboard enviada para a linha do tempo.").to_string();
                }
            }
        }
        if self.video_tools.gallery.open {
            let dir = self.gallery_dir();
            self.video_tools.gallery.ensure_open(dir);
            let tools = &mut self.video_tools;
            tools
                .gallery
                .show(ctx, tools.frame.as_ref(), tools.frame_version);
        }
    }

    fn run_color_action(&mut self, action: ColorAction, track_id: TrackId, event_id: EventId) {
        match action {
            ColorAction::Auto => {
                let Some(frame) = self.video_tools.frame.clone() else {
                    return;
                };
                let result =
                    color::auto_color_edit(&self.timeline, track_id, event_id, &frame, self.playhead);
                self.apply_color_edit(result, &tr("Cor automática aplicada ao evento."));
            }
            ColorAction::CancelStabilize => {
                if let Some(job) = &self.video_tools.stabilize {
                    job.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }
            ColorAction::Stabilize => self.start_stabilize(track_id, event_id),
            ColorAction::Reset => {
                let result = color::reset_edit(&self.timeline, track_id, event_id);
                self.apply_color_edit(result, &tr("Correção de cor do evento removida."));
            }
            ColorAction::Export => match color::export_text(&self.timeline, track_id, event_id) {
                Ok(text) => {
                    let Some(path) = rfd::FileDialog::new()
                        .add_filter(tr("Correção de cor"), &["json"])
                        .set_file_name("correcao_de_cor.json")
                        .save_file()
                    else {
                        return;
                    };
                    self.status = match std::fs::write(&path, text) {
                        Ok(()) => tr_args("Correção exportada: {}", &[&path.display().to_string()]),
                        Err(e) => tr_args("Falha ao exportar a correção: {}", &[&e.to_string()]),
                    };
                }
                Err(e) => self.status = e,
            },
            ColorAction::Import => {
                let Some(path) = rfd::FileDialog::new()
                    .add_filter(tr("Correção de cor"), &["json"])
                    .pick_file()
                else {
                    return;
                };
                let result = std::fs::read_to_string(&path)
                    .map_err(|e| tr_args("Falha ao ler a correção: {}", &[&e.to_string()]))
                    .and_then(|text| color::import_edit(&self.timeline, track_id, event_id, &text));
                self.apply_color_edit(result, &tr("Correção de cor importada."));
            }
        }
    }
}

impl AppState {
    #[cfg(feature = "ffmpeg")]
    fn start_stabilize(&mut self, track_id: TrackId, event_id: EventId) {
        use neoveg_video::render::RenderEngine;
        use neoveg_video::stabilize::StabilizeOptions;
        if self.video_tools.stabilize.is_some() {
            return;
        }
        let (media_id, in_seconds, duration) =
            match color::stabilize_plan(&self.timeline, track_id, event_id) {
                Ok(plan) => plan,
                Err(e) => {
                    self.status = e;
                    return;
                }
            };
        let Some(item) = self.project.media_pool.iter().find(|m| m.id == media_id) else {
            self.status = tr("A mídia do evento não está no projeto.").to_string();
            return;
        };
        let src = PathBuf::from(&item.path);
        let dir = self
            .gallery_dir()
            .with_file_name("estabilizados");
        if let Err(e) = std::fs::create_dir_all(&dir) {
            self.status = tr_args("Não foi possível criar a pasta {}: {}", &[&dir.display(), &e]);
            return;
        }
        let stem = src.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let output = (1..)
            .map(|n| dir.join(format!("{stem}_estabilizado_{n:03}.mp4")))
            .find(|p| !p.exists())
            .unwrap_or_else(|| dir.join("estabilizado.mp4"));
        let fps = self.project.settings.fps;
        let cancel = Arc::new(AtomicBool::new(false));
        let state = Arc::new(Mutex::new(((0, 1), None)));
        let (job_cancel, job_state, job_out) = (cancel.clone(), state.clone(), output.clone());
        std::thread::spawn(move || {
            let result = neoveg_video::stabilize_job::stabilize_file(
                &src,
                in_seconds,
                duration,
                fps,
                &job_out,
                StabilizeOptions::default(),
                RenderEngine::Auto,
                &job_cancel,
                |done, total| {
                    if let Ok(mut s) = job_state.lock() {
                        s.0 = (done, total);
                    }
                },
            );
            if let Ok(mut s) = job_state.lock() {
                s.1 = Some(result);
            }
        });
        self.status = tr("Estabilizando o evento…").to_string();
        self.video_tools.stabilize = Some(StabilizeJob { output, cancel, state });
    }

    #[cfg(not(feature = "ffmpeg"))]
    fn start_stabilize(&mut self, _track_id: TrackId, _event_id: EventId) {
        self.status = tr("A estabilização exige a build com FFmpeg.").to_string();
    }

    /// When the stabilization is over, imports the file it wrote (or reports why it failed).
    fn poll_stabilize(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.video_tools.stabilize else {
            return;
        };
        let outcome = match job.state.lock() {
            Ok(s) => s.1.clone(),
            Err(_) => Some(Err("estado da estabilização indisponível".into())),
        };
        let Some(outcome) = outcome else {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
            return;
        };
        let output = job.output.clone();
        self.video_tools.stabilize = None;
        match outcome {
            Ok(()) => {
                self.status = tr_args("Estabilizado: {} (importado na mídia do projeto)", &[&output.display()]);
                self.spawn_media(output, None, ctx);
            }
            Err(e) => self.status = tr_args("A estabilização falhou: {}", &[&e]),
        }
    }
}
