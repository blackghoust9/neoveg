//! *Make Movie*: the export that runs on its own thread while the application stays usable.
//!
//! The job mixes the project's audio offline into a 48 kHz stereo 16-bit WAV (every track effect,
//! automation curve, ducking and the master chain run, as in playback) and measures its loudness
//! at the same time, then streams the composited video frames into `ffmpeg` together with that
//! mixdown. The audio-only format stops after the first step and needs no `ffmpeg`. The frames
//! come from the same request builder as the preview, so the file shows what the preview showed.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use neoveg_audio::fxmixer::{BlockMixer, FxPlan};
use neoveg_audio::loudness_report::{
    render_mixer_to_wav_pcm16, save_loudness_log, sidecar_loudness_path,
};
use neoveg_audio::mixer::MixClip;
use neoveg_engine::project_manager::Flattened;
use neoveg_video::frame::FrameBuffer;
use neoveg_video::render::{encode_video_with_ffmpeg, VideoEncodeSpec};
use neoveg_video::worker::OfflineFrameRenderer;

use super::composite::{composite_request, FrameSettings, Source};
use super::*;
use crate::render_dialog::{ActiveRenderJob, RenderJobStatus, RenderLaunchRequest};

/// Everything the job needs, copied out of the application so that the thread owns it and later
/// edits do not change a render that is already running.
pub(super) struct RenderPlan {
    pub request: RenderLaunchRequest,
    pub timeline: Timeline,
    pub clips: Vec<MixClip>,
    pub fx: FxPlan,
    pub sources: HashMap<u64, Source>,
    pub auto_crossfade: bool,
    pub lut: Option<Arc<Lut>>,
    pub grade: Grade,
    pub chroma_key: ChromaKey,
    /// The part of the timeline to render, in seconds.
    pub range: (f64, f64),
    pub master_gain: f32,
    /// Sample rate of the exported audio: the project's.
    pub sample_rate: u32,
    pub title: String,
}

fn update(status: &Mutex<RenderJobStatus>, change: impl FnOnce(&mut RenderJobStatus)) {
    if let Ok(mut s) = status.lock() {
        change(&mut s);
    }
}

/// Starts the job; the returned handle carries its live status and the cancel flag.
pub(super) fn spawn(plan: RenderPlan, ctx: egui::Context) -> ActiveRenderJob {
    let status = Arc::new(Mutex::new(RenderJobStatus {
        phase: "Preparando".into(),
        current_frame: 0,
        total_frames: 0,
        started_at: Instant::now(),
        finished: false,
        error: None,
        output_path: plan.request.output_path.clone(),
        loudness_path: None,
        loudness_summary: None,
    }));
    let cancel = Arc::new(AtomicBool::new(false));
    let (job_status, job_cancel) = (Arc::clone(&status), Arc::clone(&cancel));
    std::thread::spawn(move || {
        let result = run(&plan, &job_status, &job_cancel, &ctx);
        update(&job_status, |s| {
            s.finished = true;
            if let Err(e) = result {
                tracing::error!("[ERROR] render failed: {e}");
                s.error = Some(e);
            }
        });
        ctx.request_repaint();
    });
    ActiveRenderJob { status, cancel }
}

fn run(
    plan: &RenderPlan,
    status: &Arc<Mutex<RenderJobStatus>>,
    cancel: &AtomicBool,
    ctx: &egui::Context,
) -> Result<(), String> {
    let request = &plan.request;
    let (t0, t1) = plan.range;
    if t1 - t0 <= 1e-3 {
        return Err("não há nada a renderizar neste intervalo".into());
    }

    // 1. The audio mixdown (and its loudness)
    update(status, |s| s.phase = "Mixagem de áudio".into());
    ctx.request_repaint();
    let wav_path = if request.format.has_video() {
        request.output_path.with_extension("mixagem.wav")
    } else {
        request.output_path.clone()
    };
    let rate = plan.sample_rate.clamp(8_000, 192_000);
    let mut mixer = BlockMixer::new(plan.clips.clone(), &plan.fx, rate as f32);
    let report = render_mixer_to_wav_pcm16(&mut mixer, &wav_path, rate, t0, t1, plan.master_gain)?;
    update(status, |s| s.loudness_summary = Some(report.summary));
    if request.save_loudness_log {
        let log = sidecar_loudness_path(&request.output_path);
        save_loudness_log(&log, &plan.title, &report)?;
        update(status, |s| s.loudness_path = Some(log));
    }
    if cancel.load(Ordering::Relaxed) {
        let _ = std::fs::remove_file(&wav_path);
        return Err("renderização cancelada pelo usuário".into());
    }
    if !request.format.has_video() {
        return Ok(());
    }

    // 2. The video, composited frame by frame and streamed into ffmpeg with the mixdown
    let fps = request.fps.max(1.0);
    let total_frames = ((t1 - t0) * fps).ceil().max(1.0) as u64;
    update(status, |s| {
        s.phase = "Codificação de vídeo".into();
        s.total_frames = total_frames;
    });
    let settings = FrameSettings {
        fps,
        canvas: (request.width, request.height),
        max_width: 0,
        playing: true,
        auto_crossfade: plan.auto_crossfade,
        lut: plan.lut.clone(),
        grade: plan.grade,
        chroma_key: plan.chroma_key,
    };
    let resolve = |id: u64| plan.sources.get(&id).cloned();
    let mut renderer = OfflineFrameRenderer::new();
    let black = FrameBuffer::solid(request.width, request.height, [0, 0, 0, 255]);
    let spec = VideoEncodeSpec {
        format: request.format,
        width: request.width,
        height: request.height,
        fps,
        crf: request.crf,
        total_frames,
        audio_wav: Some(&wav_path),
        output: &request.output_path,
    };
    let result = encode_video_with_ffmpeg(
        &spec,
        cancel,
        |done, _| {
            update(status, |s| s.current_frame = done);
            ctx.request_repaint();
        },
        |index| match composite_request(
            &plan.timeline,
            t0 + index as f64 / fps,
            &settings,
            &resolve,
        ) {
            Some(frame_request) => renderer.render_frame(&frame_request),
            None => Ok(black.clone()),
        },
    );
    let _ = std::fs::remove_file(&wav_path);
    result
}

impl AppState {
    /// Copies what a render needs out of the project and starts it.
    pub(super) fn start_render(&mut self, request: RenderLaunchRequest, ctx: &egui::Context) {
        let range = match (request.loop_region_only, self.loop_region) {
            (true, Some(region)) => (region.start, region.end),
            _ => (0.0, self.timeline.duration()),
        };
        // A sub-project plays through the expansion of its events
        let flat = self.flattened();
        let timeline = flat
            .as_deref()
            .map_or_else(|| self.timeline.clone(), |f| f.timeline.clone());
        let clips = self.build_clips();
        let sources = self.render_sources(flat.as_deref());
        let title = request
            .output_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.project.name.clone());
        let plan = RenderPlan {
            fx: fx_panel::fx_plan(&timeline),
            lut: self.current_lut(),
            grade: self.effective_grade(),
            chroma_key: self.effective_chroma(),
            auto_crossfade: self.project.settings.auto_crossfade,
            master_gain: self.master_volume,
            sample_rate: self.project.settings.sample_rate,
            request,
            timeline,
            clips,
            sources,
            range,
            title,
        };
        self.status = format!("Renderizando {}…", plan.title);
        self.render_dialog.active_job = Some(spawn(plan, ctx.clone()));
    }

    /// Where every video media of the project (and of its sub-projects) is on disk.
    pub(super) fn render_sources(&self, flat: Option<&Flattened>) -> HashMap<u64, Source> {
        let pool = self
            .project
            .media_pool
            .iter()
            .chain(flat.iter().flat_map(|f| f.extra_media.iter()));
        pool.map(|m| {
            let info = self.video_info.get(&m.id);
            (
                m.id,
                Source {
                    path: PathBuf::from(&m.path),
                    fps: info.map_or(0.0, |i| i.fps),
                    duration: info.map_or(m.duration_seconds, |i| i.duration),
                },
            )
        })
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_audio_only_render_writes_the_wav_and_the_loudness_log() {
        let dir = std::env::temp_dir().join(format!("neoveg_render_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = AppState::new();
        s.add_track(TrackKind::Audio);
        let track = s.selected_track.unwrap();
        // 2 s of a steady tone, so the mixdown is not silent
        let samples: Vec<f32> = (0..200).flat_map(|_| [0.25_f32; 2]).collect();
        let media = LoadedMedia {
            path: PathBuf::from("x.wav"),
            name: "x.wav".into(),
            waveform: Arc::new(WaveformSource::from_interleaved_f32(100, 2, &samples).unwrap()),
            stereo: Arc::new(samples),
            sample_rate: 100,
            duration: 2.0,
        };
        s.media.insert(1, media);
        s.project.media_pool.push(MediaRef {
            subproject: false,
            id: 1,
            path: "x.wav".into(),
            duration_seconds: 2.0,
        });
        let event = TrackEvent {
            id: s.timeline.alloc_id(),
            media_id: Some(1),
            start: 0.0,
            duration: 2.0,
            ..Default::default()
        };
        s.history.execute(
            &mut s.timeline,
            EditCommand::AddEvent {
                track_id: track,
                event,
            },
        );

        let output = dir.join("saida.wav");
        s.start_render(
            RenderLaunchRequest {
                output_path: output.clone(),
                format: neoveg_video::render::RenderFormat::WavPcm16,
                width: 640,
                height: 360,
                fps: 25.0,
                crf: 18,
                loop_region_only: false,
                save_loudness_log: true,
            },
            &egui::Context::default(),
        );
        let job = s.render_dialog.active_job.as_ref().expect("a job started");
        for _ in 0..500 {
            if job.status.lock().unwrap().finished {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let status = job.status.lock().unwrap().clone();
        assert!(status.finished);
        assert_eq!(status.error, None);
        assert!(status.loudness_summary.is_some());
        assert_eq!(&std::fs::read(&output).unwrap()[..4], b"RIFF");
        // 2 s of 16-bit stereo at 48 kHz plus the 44-byte header
        assert_eq!(
            std::fs::metadata(&output).unwrap().len(),
            2 * 48_000 * 4 + 44
        );
        let log = std::fs::read_to_string(dir.join("saida_loud.txt")).unwrap();
        assert!(log.contains("BS.1770-4"));
        let _ = std::fs::remove_dir_all(dir);
    }

    fn ffmpeg_available() -> bool {
        std::process::Command::new("ffmpeg")
            .arg("-version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }

    /// The whole Make Movie path for video: a generated clip is composited, encoded by `ffmpeg`
    /// together with the audio mixdown, and the result is a playable file. Skips without `ffmpeg`.
    #[test]
    fn a_video_render_encodes_the_composited_frames_when_ffmpeg_is_installed() {
        if !ffmpeg_available() {
            eprintln!("skipped: ffmpeg is not installed");
            return;
        }
        let dir = std::env::temp_dir().join(format!("neoveg_render_video_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = AppState::new();
        s.project_path = Some(dir.join("projeto.neoveg"));
        let ctx = egui::Context::default();
        s.insert_generated(
            neoveg_video::generators::GeneratorSpec {
                name: "cor".into(),
                width: 160,
                height: 90,
                fps: 25.0,
                duration: 1.0,
                kind: neoveg_video::generators::GeneratorKind::SolidColor {
                    rgba: [200, 40, 40, 255],
                },
            },
            &ctx,
        );
        for _ in 0..300 {
            s.poll_loader();
            if s.timeline.tracks.iter().any(|t| !t.events.is_empty()) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(s.timeline.tracks.iter().any(|t| !t.events.is_empty()));

        let output = dir.join("filme.mp4");
        s.start_render(
            RenderLaunchRequest {
                output_path: output.clone(),
                format: neoveg_video::render::RenderFormat::Mp4H264,
                width: 160,
                height: 90,
                fps: 25.0,
                crf: 30,
                loop_region_only: false,
                save_loudness_log: false,
            },
            &ctx,
        );
        let job = s.render_dialog.active_job.as_ref().expect("a job started");
        for _ in 0..3000 {
            if job.status.lock().unwrap().finished {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let status = job.status.lock().unwrap().clone();
        assert!(status.finished, "the render did not finish: {status:?}");
        assert_eq!(status.error, None);
        assert_eq!(status.total_frames, 25);
        assert_eq!(status.current_frame, 25);
        let size = std::fs::metadata(&output).expect("the movie exists").len();
        assert!(size > 500, "the movie is {size} bytes");
        // The temporary mixdown is removed
        assert!(!dir.join("filme.mixagem.wav").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_export_uses_the_sample_rate_of_the_project() {
        let dir = std::env::temp_dir().join(format!("neoveg_render_rate_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = AppState::new();
        s.project.settings.sample_rate = 44_100;
        s.add_track(TrackKind::Audio);
        let track = s.selected_track.unwrap();
        let samples: Vec<f32> = (0..100).flat_map(|_| [0.25_f32; 2]).collect();
        s.media.insert(
            1,
            LoadedMedia {
                path: PathBuf::from("x.wav"),
                name: "x.wav".into(),
                waveform: Arc::new(WaveformSource::from_interleaved_f32(100, 2, &samples).unwrap()),
                stereo: Arc::new(samples),
                sample_rate: 100,
                duration: 1.0,
            },
        );
        let event = TrackEvent {
            id: s.timeline.alloc_id(),
            media_id: Some(1),
            start: 0.0,
            duration: 1.0,
            ..Default::default()
        };
        s.history.execute(
            &mut s.timeline,
            EditCommand::AddEvent {
                track_id: track,
                event,
            },
        );
        let output = dir.join("saida.wav");
        s.start_render(
            RenderLaunchRequest {
                output_path: output.clone(),
                format: neoveg_video::render::RenderFormat::WavPcm16,
                width: 640,
                height: 360,
                fps: 25.0,
                crf: 18,
                loop_region_only: false,
                save_loudness_log: false,
            },
            &egui::Context::default(),
        );
        let job = s.render_dialog.active_job.as_ref().unwrap();
        for _ in 0..500 {
            if job.status.lock().unwrap().finished {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(job.status.lock().unwrap().error, None);
        // 1 s of 16-bit stereo at 44.1 kHz plus the header
        assert_eq!(std::fs::metadata(&output).unwrap().len(), 44_100 * 4 + 44);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_empty_range_is_an_error_and_not_a_file() {
        let dir = std::env::temp_dir().join(format!("neoveg_render_empty_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut s = AppState::new();
        let output = dir.join("vazio.wav");
        s.start_render(
            RenderLaunchRequest {
                output_path: output.clone(),
                format: neoveg_video::render::RenderFormat::WavPcm16,
                width: 640,
                height: 360,
                fps: 25.0,
                crf: 18,
                loop_region_only: false,
                save_loudness_log: false,
            },
            &egui::Context::default(),
        );
        let job = s.render_dialog.active_job.as_ref().unwrap();
        for _ in 0..200 {
            if job.status.lock().unwrap().finished {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let status = job.status.lock().unwrap().clone();
        assert!(status.error.is_some());
        assert!(!output.exists());
    }
}
