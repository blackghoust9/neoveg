//! The trim monitor (*Exibir > Monitor de aparo*): finds the cut, asks a worker thread for the two
//! pictures next to it, shows them, and turns the buttons into undoable edits
//! (`neoveg_engine::trim_monitor`). The pictures are made off the interface thread, each of the
//! event alone (over black), with the effects of its track, at a small size.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::mpsc::{channel, Receiver, Sender};

use neoveg_engine::render_track::only_events;
use neoveg_engine::trim_monitor::{self, TrimMode};
use neoveg_video::frame::FrameBuffer;
use neoveg_video::worker::OfflineFrameRenderer;

use super::composite::{composite_request, FrameSettings, Source};
use super::*;
use crate::trim_monitor_window::{TrimAction, TrimView};

/// Widest picture asked of the decoders, pixels.
const PICTURE_WIDTH: u32 = 640;

struct TrimJob {
    key: u64,
    sources: HashMap<u64, Source>,
    settings: FrameSettings,
    /// The solo timelines of the two events and the time to draw in each.
    sides: [(Timeline, f64); 2],
}

struct TrimPictures {
    key: u64,
    frames: [Option<FrameBuffer>; 2],
}

/// The thread that draws the pictures; a request that is not the last one is skipped.
pub(super) struct TrimWorker {
    jobs: Sender<TrimJob>,
    pictures: Receiver<TrimPictures>,
}

impl TrimWorker {
    fn start(ctx: egui::Context) -> Self {
        let (jobs, job_rx) = channel::<TrimJob>();
        let (picture_tx, pictures) = channel();
        std::thread::spawn(move || {
            let mut renderer = OfflineFrameRenderer::new();
            while let Ok(mut job) = job_rx.recv() {
                while let Ok(newer) = job_rx.try_recv() {
                    job = newer;
                }
                let resolve = |id: u64| job.sources.get(&id).cloned();
                let mut frames: [Option<FrameBuffer>; 2] = [None, None];
                for (slot, (timeline, time)) in frames.iter_mut().zip(&job.sides) {
                    *slot = composite_request(timeline, *time, &job.settings, &resolve)
                        .and_then(|request| renderer.render_frame(&request).ok());
                }
                if picture_tx
                    .send(TrimPictures {
                        key: job.key,
                        frames,
                    })
                    .is_err()
                {
                    break;
                }
                ctx.request_repaint();
            }
        });
        Self { jobs, pictures }
    }
}

fn texture(ctx: &egui::Context, name: &str, frame: &FrameBuffer) -> egui::TextureHandle {
    let image = egui::ColorImage::from_rgba_unmultiplied(
        [frame.width as usize, frame.height as usize],
        &frame.rgba,
    );
    ctx.load_texture(name, image, egui::TextureOptions::LINEAR)
}

impl AppState {
    /// *Exibir > Monitor de aparo*: opens the window on the cut at the cursor, or on the two selected
    /// events that meet.
    pub(super) fn open_trim_monitor(&mut self) {
        self.trim_monitor.open = true;
        self.retarget_trim_monitor();
    }

    fn retarget_trim_monitor(&mut self) {
        self.trim_monitor.pair = self.smooth_cut_target();
        self.trim_monitor.shown_key = None;
        self.trim_monitor.requested_key = None;
        if self.trim_monitor.pair.is_none() {
            self.trim_monitor.left = None;
            self.trim_monitor.right = None;
        }
    }

    /// What the pictures depend on: the cut and the media of the two events.
    fn trim_key(&self, track: TrackId, left: EventId, right: EventId) -> Option<u64> {
        let t = self.timeline.track(track)?;
        let (l, r) = (
            t.events.iter().find(|e| e.id == left)?,
            t.events.iter().find(|e| e.id == right)?,
        );
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for e in [l, r] {
            (e.id, e.media_id, e.start.to_bits(), e.duration.to_bits()).hash(&mut h);
            (e.trim_in.to_bits(), e.playback_rate.to_bits()).hash(&mut h);
        }
        Some(h.finish())
    }

    /// Every frame: sends the pictures that are wanted to the worker, takes in the ones that come
    /// back, draws the window and runs what it asked for.
    pub(super) fn run_trim_monitor(&mut self, ctx: &egui::Context) {
        if !self.trim_monitor.open {
            return;
        }
        let fps = self.project.settings.fps.max(1.0);
        let pair = self.trim_monitor.pair.filter(|(track, left, right)| {
            trim_monitor::edges(&self.timeline, *track, *left, *right).is_some()
        });
        if pair.is_none() {
            self.trim_monitor.pair = None;
        }
        if let Some((track, left, right)) = pair {
            self.request_trim_pictures(ctx, track, left, right, fps);
        }
        if let Some(worker) = &self.trim_worker {
            while let Ok(done) = worker.pictures.try_recv() {
                if Some(done.key) == self.trim_monitor.requested_key {
                    let [l, r] = &done.frames;
                    self.trim_monitor.left = l.as_ref().map(|f| texture(ctx, "trim_left", f));
                    self.trim_monitor.right = r.as_ref().map(|f| texture(ctx, "trim_right", f));
                    self.trim_monitor.shown_key = Some(done.key);
                }
            }
        }
        let view = pair.and_then(|(track, left, right)| self.trim_view(track, left, right, fps));
        let actions = self.trim_monitor.show(ctx, view.as_ref());
        for action in actions {
            match action {
                TrimAction::Retarget => self.retarget_trim_monitor(),
                TrimAction::Nudge(frames) => {
                    if let Some((track, left, right)) = pair {
                        self.nudge_cut(track, left, right, frames);
                    }
                }
            }
        }
    }

    fn trim_view(
        &self,
        track: TrackId,
        left: EventId,
        right: EventId,
        fps: f64,
    ) -> Option<TrimView> {
        let (out_end, in_start) = trim_monitor::edges(&self.timeline, track, left, right)?;
        let t = self.timeline.track(track)?;
        let name = |id: EventId| {
            t.events
                .iter()
                .find(|e| e.id == id)
                .map(|e| e.name.clone())
                .unwrap_or_default()
        };
        let drop_frame = self.drop_frame();
        let last = (out_end - 1.0 / fps).max(0.0);
        let gap = ((in_start - out_end) * fps).round() as i64;
        let settings = &self.project.settings;
        Some(TrimView {
            left_name: name(left),
            right_name: name(right),
            left_time: seconds_to_timecode(last, fps, drop_frame),
            right_time: seconds_to_timecode(in_start, fps, drop_frame),
            gap_frames: (gap != 0).then_some(gap),
            aspect: settings.width as f32 / settings.height.max(1) as f32,
        })
    }

    fn request_trim_pictures(
        &mut self,
        ctx: &egui::Context,
        track: TrackId,
        left: EventId,
        right: EventId,
        fps: f64,
    ) {
        let Some(key) = self.trim_key(track, left, right) else {
            return;
        };
        if self.trim_monitor.requested_key == Some(key) {
            return;
        }
        let Some((out_end, in_start)) = trim_monitor::edges(&self.timeline, track, left, right)
        else {
            return;
        };
        let settings = &self.project.settings;
        let (width, height) = (settings.width.max(2), settings.height.max(2));
        let frame_settings = FrameSettings {
            fps,
            canvas: (width, height),
            max_width: PICTURE_WIDTH,
            playing: false,
            auto_crossfade: settings.auto_crossfade,
            resample_mode: settings.resample_mode,
            lut: self.current_lut(),
            grade: self.effective_grade(),
            chroma_key: self.effective_chroma(),
            fx_draft: None,
            pan_crop_draft: None,
            velocity_draft: None,
        };
        let flat = self.flattened();
        let sources = self.render_sources(flat.as_deref());
        // Half a frame inside the frame, so the time cannot fall in the one next to it
        let side =
            |event: EventId, time: f64| (only_events(&self.timeline, &[(track, event)]), time);
        let job = TrimJob {
            key,
            sources,
            settings: frame_settings,
            sides: [
                side(left, (out_end - 0.5 / fps).max(0.0)),
                side(right, in_start + 0.5 / fps),
            ],
        };
        let worker = self
            .trim_worker
            .get_or_insert_with(|| TrimWorker::start(ctx.clone()));
        if worker.jobs.send(job).is_ok() {
            self.trim_monitor.requested_key = Some(key);
        }
    }

    /// The buttons of the window: moves the cut (or the edge of the mode) and puts the cursor on it.
    pub(super) fn nudge_cut(&mut self, track: TrackId, left: EventId, right: EventId, frames: i32) {
        let fps = self.project.settings.fps.max(1.0);
        let source_len = self.source_len(track, left);
        let Some(command) = trim_monitor::nudge(
            &self.timeline,
            track,
            left,
            right,
            self.trim_monitor.mode,
            frames,
            fps,
            source_len,
        ) else {
            self.status = "O corte não pode ir mais longe: acabou a mídia de um dos lados".into();
            return;
        };
        if self.history.execute(&mut self.timeline, command) {
            self.last_frame_key = None;
            if let Some((out_end, in_start)) =
                trim_monitor::edges(&self.timeline, track, left, right)
            {
                self.playhead = match self.trim_monitor.mode {
                    TrimMode::In => in_start,
                    _ => out_end,
                };
            }
        }
    }
}
