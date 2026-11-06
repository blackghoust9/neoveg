//! *Criar subclipe do evento*, *Criar subclipe revertido* and *Reverter evento*: the part of its media
//! the selected event plays is written, on a worker thread, as a file of its own in the `subclipes`
//! folder beside the project (backwards for the reverse ones); when the file is read it joins the media
//! pool and, for *Reverter evento*, the events that played that part move to it in one undo step. What
//! is pure is in `neoveg_engine::subclip`; the files are written by `neoveg_audio::subclip` (sound) and
//! `neoveg_video::subclip` (files with a picture).

use neoveg_common::i18n::tr_args;
use neoveg_engine::subclip as sub;

use super::*;

/// A subclip being written, or read back.
pub(super) struct PendingSubclip {
    pub(super) path: PathBuf,
    /// The events that move to the new file when it is ready (none: it only joins the pool).
    pub(super) replace: Vec<EventRef>,
    pub(super) reverse: bool,
    /// The file has the effects of the event written into it (*efeito fora de tempo real*): the
    /// event moves to it and its own effects come off.
    pub(super) bake: bool,
}

impl AppState {
    /// Where the subclips go: beside the project, or in the data folder of an unsaved one.
    pub(super) fn subclip_dir(&self) -> Option<PathBuf> {
        match self
            .project_path
            .as_deref()
            .and_then(std::path::Path::parent)
        {
            Some(parent) => Some(parent.join("subclipes")),
            None => neoveg_common::paths::app_data_dir().map(|d| d.join("subclipes")),
        }
    }

    /// The primary selected event, the part of its media it plays and the media file.
    pub(super) fn subclip_source(&self) -> Option<(EventRef, sub::Section, PathBuf)> {
        let (track_id, event_id) = self.selection.primary()?;
        let event = self
            .timeline
            .track(track_id)?
            .events
            .iter()
            .find(|e| e.id == event_id)?;
        let section = sub::section_of(event)?;
        if self.is_generated_event(event) {
            return None;
        }
        let media_id = event.media_id?;
        let media = self
            .project
            .media_pool
            .iter()
            .find(|m| m.id == media_id && !m.subproject)?;
        Some(((track_id, event_id), section, PathBuf::from(&media.path)))
    }

    /// Whether the subclip commands can run now (`replace`: *Reverter evento*, which also needs an
    /// event that is not locked).
    pub(super) fn can_subclip(&self, replace: bool) -> bool {
        if !self.pending_subclips.is_empty() {
            return false;
        }
        let Some((event, ..)) = self.subclip_source() else {
            return false;
        };
        !replace
            || sub::move_to_media(
                &self.timeline,
                &sub::same_section(&self.timeline, &self.selection.refs(), event),
                0,
            )
            .is_some()
    }

    /// The commands: writes the part the primary selected event plays (backwards when `reverse`)
    /// and, when `replace`, puts the events that played it on the new file.
    pub(super) fn begin_subclip(&mut self, reverse: bool, replace: bool, ctx: &egui::Context) {
        if !self.pending_subclips.is_empty() {
            self.status = tr("Já há um subclipe sendo feito").into();
            return;
        }
        let Some((event, section, input)) = self.subclip_source() else {
            self.status = tr("Selecione um evento com mídia para cortar").into();
            return;
        };
        let refs = if replace {
            sub::same_section(&self.timeline, &self.selection.refs(), event)
        } else {
            Vec::new()
        };
        if replace && sub::move_to_media(&self.timeline, &refs, 0).is_none() {
            self.status = tr("O evento está bloqueado").into();
            return;
        }
        let video = is_video_path(&input);
        if video && reverse && section.length > sub::MAX_REVERSE_VIDEO_SECONDS {
            self.status = tr_args(
                "Vídeo só reverte até {} s por vez (o trecho tem {} s): divida o evento",
                &[
                    &sub::MAX_REVERSE_VIDEO_SECONDS,
                    &format!("{:.1}", section.length),
                ],
            );
            return;
        }
        let Some(dir) = self.subclip_dir() else {
            self.status = tr("Não há pasta para guardar o subclipe").into();
            return;
        };
        let output = sub::output_path(
            &input,
            &dir,
            section,
            reverse,
            if video { "mp4" } else { "wav" },
            |p| p.exists() || self.pending_subclips.iter().any(|s| s.path == p),
        );
        let has_audio = self
            .timeline
            .track(event.0)
            .and_then(|t| t.events.iter().find(|e| e.id == event.1))
            .and_then(|e| e.media_id)
            .is_some_and(|id| self.media.contains_key(&id));
        self.status = tr_args(
            if reverse {
                "Criando o subclipe revertido {}…"
            } else {
                "Criando o subclipe {}…"
            },
            &[&output.file_name().map_or_else(
                || output.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            )],
        );
        self.pending_subclips.push(PendingSubclip {
            path: output.clone(),
            replace: refs,
            reverse,
            bake: false,
        });
        let (tx, ctx) = (self.loader_tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let result = if video {
                neoveg_video::subclip::write(
                    &input,
                    &output,
                    section.start,
                    section.length,
                    reverse,
                    has_audio,
                )
            } else {
                neoveg_audio::subclip::write_wav(
                    &input,
                    &output,
                    section.start,
                    section.length,
                    reverse,
                )
                .map(|_| ())
            };
            let msg = match result {
                Ok(()) => load_message(output, None, true),
                Err(error) => LoadMsg::Failed {
                    path: output,
                    error,
                    existing_id: None,
                    converted: true,
                },
            };
            let _ = tx.send(msg);
            ctx.request_repaint();
        });
    }

    /// Whether `path` is a subclip that was being written and is now read.
    pub(super) fn takes_subclip(&self, path: &std::path::Path) -> bool {
        self.pending_subclips.iter().any(|s| s.path == path)
    }

    /// The file could not be written or read: nothing changes.
    pub(super) fn subclip_failed(&mut self, path: &std::path::Path) {
        self.pending_subclips.retain(|s| s.path != path);
    }

    /// Adds the subclip to the media pool and, for *Reverter evento*, moves the events to it.
    pub(super) fn place_subclip(
        &mut self,
        path: PathBuf,
        duration: f64,
        info: Option<VideoInfo>,
        audio: Option<LoadedMedia>,
    ) {
        let Some(at) = self.pending_subclips.iter().position(|s| s.path == path) else {
            return;
        };
        let pending = self.pending_subclips.remove(at);
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
        if let Some(sound) = audio {
            self.media.insert(media_id, sound);
        }
        if let Some(info) = info {
            self.video_info.insert(media_id, info);
        }
        if pending.replace.is_empty() {
            self.status = tr_args("Subclipe na mídia do projeto: {}", &[&name]);
            return;
        }
        let command = if pending.bake {
            sub::bake_effects_to_media(&self.timeline, &pending.replace, media_id)
        } else {
            sub::move_to_media(&self.timeline, &pending.replace, media_id)
        };
        let moved =
            command.is_some_and(|command| self.history.execute(&mut self.timeline, command));
        if !moved {
            self.status = tr_args("Subclipe na mídia do projeto: {}", &[&name]);
            return;
        }
        self.last_frame_key = None;
        self.last_composite_key = None;
        self.restart_audio_if_playing();
        self.status = tr_args(
            if pending.bake {
                "Efeitos gravados no áudio do evento: agora toca {}"
            } else if pending.reverse {
                "Evento revertido: agora toca {}"
            } else {
                "Evento trocado pelo subclipe {}"
            },
            &[&name],
        );
    }
}
