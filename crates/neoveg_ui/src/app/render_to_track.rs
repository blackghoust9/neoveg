//! *Renderizar para nova faixa* (`Ctrl+M`) and *Renderizar e substituir*: the selected events are
//! rendered, alone, to a file in the render folder; when the file is done it is imported and laid
//! on new tracks, or in place of the events. The pieces that decide what is rendered and what the
//! edit is are in `neoveg_engine::render_track`; this is the glue with the render job and the
//! media loader.

use neoveg_common::i18n::tr_args;
use neoveg_engine::render_track;
use neoveg_video::render::{RenderEngine, RenderFormat};

use super::*;
use crate::render_dialog::RenderLaunchRequest;

/// A render of the selection that is running, or whose file is being loaded.
pub(super) struct PendingRender {
    path: PathBuf,
    start: f64,
    replace: bool,
    refs: Vec<EventRef>,
    selected: render_track::Selected,
    /// The render is over and the file is being read.
    loading: bool,
}

impl AppState {
    /// Where the rendered files go: beside the project, or in the data folder of an unsaved one.
    fn render_dir(&self) -> Option<PathBuf> {
        match self
            .project_path
            .as_deref()
            .and_then(std::path::Path::parent)
        {
            Some(parent) => Some(parent.join("renderizados")),
            None => neoveg_common::paths::app_data_dir().map(|d| d.join("renderizados")),
        }
    }

    /// The command: renders the selected events and, when the file is ready, lays it on new tracks
    /// (`replace` false) or in place of the events.
    pub(super) fn begin_render_to_track(&mut self, replace: bool, ctx: &egui::Context) {
        if self.render_dialog.is_running() || self.pending_render.is_some() {
            self.status = "Já há uma renderização em andamento".into();
            return;
        }
        let refs = self.selection.refs();
        let Some(selected) = render_track::describe(&self.timeline, &refs) else {
            self.status = "Selecione os eventos que serão renderizados".into();
            return;
        };
        let Some(dir) = self.render_dir() else {
            self.status = "Não há pasta para guardar a renderização".into();
            return;
        };
        if let Err(e) = std::fs::create_dir_all(&dir) {
            self.status = tr_args(
                "Não foi possível criar a pasta {}: {}",
                &[&dir.display(), &e],
            );
            return;
        }
        let (format, extension) = if selected.has_video {
            (RenderFormat::Mp4H264, "mp4")
        } else {
            (RenderFormat::WavPcm16, "wav")
        };
        let path = (1..)
            .map(|n| dir.join(format!("render_{n:04}.{extension}")))
            .find(|p| !p.exists())
            .unwrap_or_else(|| dir.join(format!("render.{extension}")));
        let settings = &self.project.settings;
        let request = RenderLaunchRequest {
            output_path: path.clone(),
            format,
            width: settings.width,
            height: settings.height,
            fps: settings.fps,
            crf: 18,
            engine: RenderEngine::Auto,
            burn_captions: false,
            loop_region_only: false,
            save_loudness_log: false,
        };
        self.pending_render = Some(PendingRender {
            path,
            start: selected.span.0,
            replace,
            refs: refs.clone(),
            selected,
            loading: false,
        });
        self.start_render_with(request, selected.span, Some(refs), ctx);
    }

    /// Every frame: when the render of the selection is over, reads the file it made.
    pub(super) fn poll_render_to_track(&mut self, ctx: &egui::Context) {
        let Some(pending) = &mut self.pending_render else {
            return;
        };
        if pending.loading {
            return;
        }
        let Some(job) = &self.render_dialog.active_job else {
            self.pending_render = None;
            return;
        };
        let (finished, error) = match job.status.lock() {
            Ok(s) => (s.finished, s.error.clone()),
            Err(_) => (true, Some("estado da renderização indisponível".into())),
        };
        if !finished {
            return;
        }
        if let Some(error) = error {
            self.status = tr_args("A renderização falhou: {}", &[&error]);
            self.pending_render = None;
            return;
        }
        pending.loading = true;
        let path = pending.path.clone();
        self.spawn_media(path, None, ctx);
    }

    /// Whether `path` is the file the render of the selection made (and is being loaded).
    pub(super) fn takes_rendered(&self, path: &std::path::Path) -> bool {
        self.pending_render
            .as_ref()
            .is_some_and(|p| p.loading && p.path == path)
    }

    /// The file could not be read: nothing is placed.
    pub(super) fn rendered_failed(&mut self, path: &std::path::Path) {
        if self.takes_rendered(path) {
            self.pending_render = None;
        }
    }

    /// Lays the rendered file in the project as one undo step.
    pub(super) fn place_rendered(
        &mut self,
        path: PathBuf,
        duration: f64,
        info: Option<VideoInfo>,
        audio: Option<LoadedMedia>,
    ) {
        let Some(pending) = self.pending_render.take() else {
            return;
        };
        let media_id = self.timeline.alloc_id();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.project.media_pool.push(MediaRef {
            subproject: false,
            format: Some(neoveg_common::project::MediaFormat {
                channels: audio.as_ref().map(|a| a.channels),
                fps: info.as_ref().map(|i| i.fps),
            }),
            id: media_id,
            path: path.to_string_lossy().into_owned(),
            duration_seconds: duration,
        });
        let mut added = Vec::new();
        if let Some(info) = info.as_ref().filter(|_| pending.selected.has_video) {
            let track = self.render_target(
                pending.replace,
                pending.selected.video_track,
                TrackKind::Video,
            );
            added.push((
                track,
                self.rendered_event(&name, media_id, pending.start, info.duration),
            ));
        }
        if let Some(sound) = audio
            .as_ref()
            .filter(|_| pending.selected.has_audio || info.is_none())
        {
            let track = self.render_target(
                pending.replace,
                pending.selected.audio_track,
                TrackKind::Audio,
            );
            added.push((
                track,
                self.rendered_event(&name, media_id, pending.start, sound.duration),
            ));
        }
        if let Some(sound) = audio {
            self.media.insert(media_id, sound);
        }
        if let Some(info) = info {
            self.video_info.insert(media_id, info);
        }
        let placed = added.len();
        if let Some(command) =
            render_track::placement(&self.timeline, &pending.refs, added, pending.replace)
        {
            self.history.execute(&mut self.timeline, command);
        }
        if pending.replace {
            self.selection.clear();
        }
        self.last_frame_key = None;
        self.status = match (pending.replace, placed) {
            (_, 0) => "A renderização não tem imagem nem som para colocar".into(),
            (true, _) => tr_args("Eventos substituídos por {}", &[&name]),
            (false, _) => tr_args("Renderizado para nova faixa: {}", &[&name]),
        };
    }

    /// The track the rendered event goes to: the one of the events it replaces, or a new one.
    fn render_target(
        &mut self,
        replace: bool,
        original: Option<TrackId>,
        kind: TrackKind,
    ) -> TrackId {
        match original.filter(|_| replace) {
            Some(id) => id,
            None => {
                self.add_track(kind);
                self.selected_track.unwrap_or_default()
            }
        }
    }

    fn rendered_event(
        &mut self,
        name: &str,
        media_id: u64,
        start: f64,
        duration: f64,
    ) -> TrackEvent {
        TrackEvent {
            id: self.timeline.alloc_id(),
            name: name.to_owned(),
            media_id: Some(media_id),
            start,
            duration,
            ..Default::default()
        }
    }
}
