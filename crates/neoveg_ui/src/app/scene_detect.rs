//! *Detectar cenas e dividir* (`Editar > Detectar cenas e dividir`): the primary selected video
//! event is looked at frame by frame on its own thread (the event alone, with its effects, at a
//! small size), `neoveg_video::scene` finds where the shot changes, and the event is split there in
//! one undo step (`neoveg_engine::ops::split_at_times`).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

use neoveg_engine::ops::{split_at_times, Scope};
use neoveg_engine::render_track::only_events;
use neoveg_video::frame::FrameBuffer;
use neoveg_video::scene::{self, SceneDetector, SceneOptions};
use neoveg_video::worker::OfflineFrameRenderer;

use super::composite::{composite_request, FrameSettings, Source};
use super::*;
use crate::scene_window::SceneAction;

/// Width of the pictures the detection looks at, pixels.
const PICTURE_WIDTH: u32 = 160;
/// Most frames looked at in one event; a longer event is looked at every few frames.
const MAX_SAMPLES: usize = 6000;
/// Resolution of the progress counter.
const PROGRESS_STEPS: u32 = 1000;

/// A detection that is running.
pub(super) struct SceneJob {
    event: EventRef,
    name: String,
    progress: Arc<AtomicU32>,
    cancel: Arc<AtomicBool>,
    result: Receiver<Result<Vec<f64>, String>>,
}

impl AppState {
    /// The command: opens the options on the primary selected event of a video track.
    pub(super) fn open_scene_detection(&mut self) {
        let event = self.selection.primary().filter(|(track, event)| {
            self.timeline.track(*track).is_some_and(|t| {
                t.kind == TrackKind::Video
                    && t.events
                        .iter()
                        .any(|e| e.id == *event && e.media_id.is_some() && !e.adjustment)
            })
        });
        if event.is_none() {
            self.status = "Selecione um evento de vídeo para detectar as cenas".into();
            return;
        }
        self.scene_dialog.open = true;
    }

    fn scene_target_name(&self) -> String {
        self.selection
            .primary()
            .and_then(|(track, event)| {
                self.timeline
                    .track(track)?
                    .events
                    .iter()
                    .find(|e| e.id == event)
                    .map(|e| e.name.clone())
            })
            .unwrap_or_default()
    }

    fn start_scene_detection(&mut self, options: SceneOptions, ctx: &egui::Context) {
        if self.scene_job.is_some() {
            return;
        }
        let Some(event_ref) = self.selection.primary() else {
            return;
        };
        let Some(event) = self
            .timeline
            .track(event_ref.0)
            .and_then(|t| t.events.iter().find(|e| e.id == event_ref.1))
            .cloned()
        else {
            return;
        };
        let fps = self.project.settings.fps.max(1.0);
        let settings = &self.project.settings;
        let aspect = settings.width.max(2) as f32 / settings.height.max(2) as f32;
        let canvas = (
            PICTURE_WIDTH,
            ((PICTURE_WIDTH as f32 / aspect).round() as u32).max(2),
        );
        let frame_settings = FrameSettings {
            fps,
            canvas,
            max_width: PICTURE_WIDTH,
            playing: false,
            auto_crossfade: false,
            resample_mode: settings.resample_mode,
            lut: None,
            grade: Default::default(),
            chroma_key: Default::default(),
            fx_draft: None,
            pan_crop_draft: None,
            velocity_draft: None,
        };
        let flat = self.flattened();
        let sources: HashMap<u64, Source> = self.render_sources(flat.as_deref());
        let solo = only_events(&self.timeline, &[event_ref]);
        let progress = Arc::new(AtomicU32::new(0));
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, result) = channel();
        let (job_progress, job_cancel, repaint) =
            (Arc::clone(&progress), Arc::clone(&cancel), ctx.clone());
        let (start, duration) = (event.start, event.duration);
        std::thread::spawn(move || {
            let outcome = detect(
                &solo,
                &sources,
                &frame_settings,
                (start, duration),
                fps,
                options,
                &job_progress,
                &job_cancel,
            );
            let _ = tx.send(outcome);
            repaint.request_repaint();
        });
        self.status = "Detectando cenas…".into();
        self.scene_job = Some(SceneJob {
            event: event_ref,
            name: event.name,
            progress,
            cancel,
            result,
        });
    }

    /// Every frame: draws the options and the progress, and when the detection is over splits the
    /// event.
    pub(super) fn run_scene_detection(&mut self, ctx: &egui::Context) {
        if let Some(job) = &self.scene_job {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
            if let Ok(outcome) = job.result.try_recv() {
                let event = job.event;
                self.scene_job = None;
                self.finish_scene_detection(event, outcome);
            }
        }
        if !self.scene_dialog.open {
            return;
        }
        let name = self
            .scene_job
            .as_ref()
            .map_or_else(|| self.scene_target_name(), |j| j.name.clone());
        let progress = self
            .scene_job
            .as_ref()
            .map(|j| j.progress.load(Ordering::Relaxed) as f32 / PROGRESS_STEPS as f32);
        match self.scene_dialog.show(ctx, &name, progress) {
            Some(SceneAction::Start(options)) => self.start_scene_detection(options, ctx),
            Some(SceneAction::Cancel) => {
                if let Some(job) = &self.scene_job {
                    job.cancel.store(true, Ordering::Relaxed);
                }
            }
            None => {}
        }
    }

    pub(super) fn finish_scene_detection(
        &mut self,
        event: EventRef,
        outcome: Result<Vec<f64>, String>,
    ) {
        let times = match outcome {
            Ok(times) => times,
            Err(error) => {
                self.status = tr_args("A detecção de cenas parou: {}", &[&error]);
                return;
            }
        };
        if times.is_empty() {
            self.status = "Nenhuma troca de cena encontrada".into();
            self.scene_dialog.open = false;
            return;
        }
        // The track is the scope, not the event: the halves a first cut makes are new events, and
        // the next cuts have to find them (the cuts are all inside this event, where the track
        // holds nothing else)
        let tracks = [event.0];
        let Some(outcome) = split_at_times(&mut self.timeline, &times, Scope::Tracks(&tracks))
        else {
            self.status = "O evento não pôde ser dividido (travado ou mudou de lugar)".into();
            return;
        };
        if self.history.execute(&mut self.timeline, outcome.command) {
            self.last_frame_key = None;
            self.status = tr_args("Evento dividido em {} cenas", &[&(times.len() + 1)]);
        }
        self.scene_dialog.open = false;
    }
}

/// Looks at the frames of the event and returns the timeline times where a new shot begins.
#[allow(clippy::too_many_arguments)]
fn detect(
    solo: &Timeline,
    sources: &HashMap<u64, Source>,
    settings: &FrameSettings,
    (start, duration): (f64, f64),
    fps: f64,
    options: SceneOptions,
    progress: &AtomicU32,
    cancel: &AtomicBool,
) -> Result<Vec<f64>, String> {
    let frames = (duration * fps).floor() as usize;
    let step = scene::sampling_step(frames, MAX_SAMPLES);
    let samples = frames / step;
    if samples < 2 {
        return Err("o evento é curto demais".into());
    }
    let resolve = |id: u64| sources.get(&id).cloned();
    let mut renderer = OfflineFrameRenderer::new();
    let black = FrameBuffer::solid(settings.canvas.0, settings.canvas.1, [0, 0, 0, 255]);
    let mut detector = SceneDetector::new();
    for i in 0..samples {
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelada".into());
        }
        // Half a frame inside the frame, so the time cannot fall in the one before it
        let time = start + (i * step) as f64 / fps + 0.5 / fps;
        let frame = match composite_request(solo, time, settings, &resolve) {
            Some(request) => renderer.render_frame(&request)?,
            None => black.clone(),
        };
        detector.push_frame(&frame);
        progress.store(
            (i as u64 * u64::from(PROGRESS_STEPS) / samples as u64) as u32,
            Ordering::Relaxed,
        );
    }
    let sampled = SceneOptions {
        min_shot_frames: options.min_shot_frames.div_ceil(step).max(1),
        ..options
    };
    Ok(scene::cut_times_at(
        &detector.cuts(&sampled),
        step,
        fps,
        start,
    ))
}
