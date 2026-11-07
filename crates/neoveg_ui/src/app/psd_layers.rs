//! *PSD em camadas*: every visible layer of a `.psd` becomes a picture file of its own and an event
//! on its own video track, stacked in the order of the document. The reading and the PNG files are
//! `neoveg_video::psd`; this module only places the media and the events.

use neoveg_common::i18n::tr_args;
use neoveg_engine::timeline::{TrackEvent, TrackKind};
use neoveg_video::psd::layers::{decode_layers_file, export_layers_png};

use super::*;

impl AppState {
    /// Asks for a `.psd` and opens its layers as tracks.
    pub(super) fn import_psd_layers(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Photoshop (*.psd)", &["psd"])
            .pick_file()
        else {
            return;
        };
        self.place_psd_layers(&path, ctx);
    }

    /// Writes the layers of `path` beside the project and puts one event per layer on a new video
    /// track, the last layer of the document on the top track. Returns how many layers went in.
    pub(super) fn place_psd_layers(
        &mut self,
        path: &std::path::Path,
        ctx: &egui::Context,
    ) -> usize {
        let canvas = match neoveg_video::psd::probe_file(path) {
            Ok(canvas) => canvas,
            Err(e) => {
                self.status = e;
                return 0;
            }
        };
        let layers = match decode_layers_file(path) {
            Ok(layers) => layers,
            Err(e) => {
                self.status = e;
                return 0;
            }
        };
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("psd")
            .to_owned();
        let dir = self.psd_layers_dir(&stem);
        let files = match export_layers_png(&layers, canvas, &dir) {
            Ok(files) => files,
            Err(e) => {
                self.status = e;
                return 0;
            }
        };
        if files.is_empty() {
            self.status = tr("O PSD não tem camadas visíveis").into();
            return 0;
        }
        let (start, duration) = match self.time_selection {
            Some(range) if range.end - range.start > TIME_EPSILON => {
                (range.start, range.end - range.start)
            }
            _ => (self.playhead, self.prefs.still_image_seconds.max(0.1)),
        };

        let mut commands = Vec::new();
        let mut first: Option<(TrackId, EventId)> = None;
        for (file, name) in &files {
            let media_id = self.timeline.alloc_id();
            self.project.media_pool.push(MediaRef {
                id: media_id,
                path: file.to_string_lossy().into_owned(),
                duration_seconds: duration,
                subproject: false,
                format: None,
            });
            let mut track = self.timeline.new_track(TrackKind::Video);
            track.name = name.clone();
            let track_id = track.id;
            let event = TrackEvent {
                id: self.timeline.alloc_id(),
                name: name.clone(),
                media_id: Some(media_id),
                start,
                duration,
                ..TrackEvent::default()
            };
            first.get_or_insert((track_id, event.id));
            commands.push(EditCommand::InsertTrack { track, index: 0 });
            commands.push(EditCommand::AddEvent { track_id, event });
            self.spawn_media(file.clone(), Some(media_id), ctx);
        }
        if !self
            .history
            .execute(&mut self.timeline, EditCommand::Batch(commands))
        {
            return 0;
        }
        if let Some(target) = first {
            self.selection.select_only(target);
        }
        self.last_frame_key = None;
        self.last_composite_key = None;
        self.status = tr_args(
            "{} camadas do PSD {} na linha do tempo",
            &[&files.len(), &stem],
        );
        files.len()
    }

    /// Where the PNGs of a document go: `camadas/<name>/` beside the project, or in the temporary
    /// folder while the project has no file of its own yet.
    fn psd_layers_dir(&self, stem: &str) -> std::path::PathBuf {
        let base = self
            .project_path
            .as_ref()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::env::temp_dir().join("neoveg_psd"));
        base.join("camadas").join(stem)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_layers_of_a_document_go_beside_the_project_folder() {
        let mut s = AppState::new();
        s.project_path = Some(std::path::PathBuf::from("C:/filmes/projeto.neoveg"));
        assert!(s
            .psd_layers_dir("capa")
            .ends_with(std::path::Path::new("camadas").join("capa")));
        assert!(s.psd_layers_dir("capa").starts_with("C:/filmes"));
    }
}
