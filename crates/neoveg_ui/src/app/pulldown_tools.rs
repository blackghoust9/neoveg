//! Glue for the 3:2 pulldown buttons of the Media Properties window: runs the conversion of the
//! media file on a worker thread (like the stabilization) and imports the file it wrote. The frame
//! logic lives in `neoveg_video::pulldown_job`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use eframe::egui;
use neoveg_common::i18n::{tr, tr_args};
use neoveg_video::pulldown_job::PulldownMode;

use super::AppState;

type PulldownJobState = Arc<Mutex<((u64, u64), Option<Result<(), String>>)>>;

/// A pulldown conversion running on a worker thread.
pub(super) struct PulldownJob {
    output: PathBuf,
    /// `(done, total)` and, once over, how it ended.
    state: PulldownJobState,
}

/// File name of the converted copy of `stem`: `stem_sem_pulldown_001.mp4`, the first number free.
#[allow(dead_code)]
pub(super) fn output_name(stem: &str, mode: PulldownMode, taken: impl Fn(&str) -> bool) -> String {
    (1..)
        .map(|n| format!("{stem}_{}_{n:03}.mp4", mode.suffix()))
        .find(|name| !taken(name))
        .unwrap_or_else(|| format!("{stem}_{}.mp4", mode.suffix()))
}

impl AppState {
    /// Starts the conversion the Media Properties window asked for.
    #[cfg(feature = "ffmpeg")]
    pub(super) fn start_pulldown(&mut self, media_id: u64, mode: PulldownMode) {
        use neoveg_video::render::RenderEngine;
        if self.video_tools.pulldown.is_some() {
            self.status = tr("Já há uma conversão de pulldown em andamento.").to_string();
            return;
        }
        let Some(item) = self.project.media_pool.iter().find(|m| m.id == media_id) else {
            self.status = tr("A mídia do evento não está no projeto.").to_string();
            return;
        };
        let src = PathBuf::from(&item.path);
        let dir = self.gallery_dir().with_file_name("pulldown");
        if let Err(e) = std::fs::create_dir_all(&dir) {
            self.status = tr_args(
                "Não foi possível criar a pasta {}: {}",
                &[&dir.display(), &e],
            );
            return;
        }
        let stem = src
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let output = dir.join(output_name(&stem, mode, |n| dir.join(n).exists()));
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let state = Arc::new(Mutex::new(((0, 1), None)));
        let (job_state, job_out) = (state.clone(), output.clone());
        std::thread::spawn(move || {
            let result = neoveg_video::pulldown_job::convert_file(
                &src,
                mode,
                &job_out,
                RenderEngine::Auto,
                &cancel,
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
        self.status = tr("Convertendo o pulldown da mídia…").to_string();
        self.video_tools.pulldown = Some(PulldownJob { output, state });
    }

    #[cfg(not(feature = "ffmpeg"))]
    pub(super) fn start_pulldown(&mut self, _media_id: u64, _mode: PulldownMode) {
        self.status = tr("A conversão de pulldown exige a build com FFmpeg.").to_string();
    }

    /// When the conversion is over, imports the file it wrote (or reports why it failed).
    pub(super) fn poll_pulldown(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.video_tools.pulldown else {
            return;
        };
        let (progress, outcome) = match job.state.lock() {
            Ok(s) => (s.0, s.1.clone()),
            Err(_) => ((0, 1), Some(Err("estado da conversão indisponível".into()))),
        };
        let Some(outcome) = outcome else {
            self.status = tr_args(
                "Convertendo o pulldown: quadro {} de {}",
                &[&progress.0, &progress.1],
            );
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
            return;
        };
        let output = job.output.clone();
        self.video_tools.pulldown = None;
        match outcome {
            Ok(()) => {
                self.status = tr_args(
                    "Pulldown convertido: {} (importado na mídia do projeto)",
                    &[&output.display()],
                );
                self.spawn_media(output, None, ctx);
            }
            Err(e) => self.status = tr_args("A conversão de pulldown falhou: {}", &[&e]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_converted_copy_never_overwrites_an_earlier_one() {
        let taken = ["clip_sem_pulldown_001.mp4", "clip_sem_pulldown_002.mp4"];
        let name = output_name("clip", PulldownMode::Remove, |n| taken.contains(&n));
        assert_eq!(name, "clip_sem_pulldown_003.mp4");
        assert_eq!(
            output_name("clip", PulldownMode::Apply, |_| false),
            "clip_com_pulldown_001.mp4"
        );
    }
}
