//! The glue of the export extras: *Exportação rápida*, *Arquivar projeto*, *Consolidar mídia* and
//! *Reenquadrar para vertical*. What each one decides (recipes, copy plan, reframe boxes) is in
//! `neoveg_common::quick_export`, `neoveg_engine::archive` and `neoveg_engine::vertical_reframe`;
//! here the project is read, a folder is asked for when needed, and the result is reported in the
//! status bar.

use std::path::Path;

use neoveg_common::i18n::tr_args;
use neoveg_common::quick_export::{output_path, output_size, Container, QuickExportTarget};
use neoveg_engine::archive::{self, ArchiveOptions};
use neoveg_video::render::{RenderEngine, RenderFormat};

use super::*;
use crate::render_dialog::RenderLaunchRequest;

/// Margin kept before and after the used part of each file by *Consolidar mídia*, in seconds.
const CONSOLIDATE_MARGIN: f64 = 2.0;

/// Cuts `source` between `start` and `end` seconds into `dest` with `ffmpeg`. Plain sound files
/// are cut without re-encoding; pictures are re-encoded so the cut starts exactly at `start`.
fn ffmpeg_cut(source: &Path, dest: &Path, start: f64, end: f64) -> Result<(), String> {
    let ext = source
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let mut cmd = std::process::Command::new("ffmpeg");
    cmd.args(["-y", "-v", "error", "-ss"])
        .arg(format!("{start:.3}"))
        .arg("-to")
        .arg(format!("{end:.3}"))
        .arg("-i")
        .arg(source);
    if ["wav", "flac", "aiff", "aif"].contains(&ext.as_str()) {
        cmd.args(["-c", "copy"]);
    } else {
        cmd.args(["-c:v", "libx264", "-crf", "14", "-preset", "fast", "-c:a", "aac"]);
    }
    let status = cmd
        .arg(dest)
        .status()
        .map_err(|e| format!("ffmpeg indisponível: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("ffmpeg terminou com {status}"))
    }
}

impl AppState {
    /// The three quick exports: the project is rendered with the recipe of `target`, next to the
    /// project file, with no dialog.
    pub(super) fn quick_export(&mut self, target: QuickExportTarget, ctx: &egui::Context) {
        if self.timeline.duration() <= 1e-3 {
            self.status = tr("Adicione eventos à timeline antes de renderizar.").to_string();
            return;
        }
        if self.render_dialog.is_running() {
            self.status = tr("Já existe uma renderização em andamento").to_string();
            return;
        }
        let recipe = target.recipe();
        let format = match recipe.container {
            Container::Mp4H264 => RenderFormat::Mp4H264,
            Container::WavPcm16 => RenderFormat::WavPcm16,
        };
        let engine = RenderEngine::Auto;
        if format.has_video() && !engine.can_write(format) {
            self.status = tr("Nenhum codificador de vídeo disponível para a exportação rápida").to_string();
            return;
        }
        let fallback = neoveg_common::paths::app_data_dir()
            .map(|d| d.join("renderizados"))
            .unwrap_or_default();
        let dir = self
            .project_path
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf);
        let path = output_path(target, &self.project.name, dir.as_deref(), &fallback, |p| {
            p.exists()
        });
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let (width, height) = output_size(
            &recipe,
            self.project.settings.width,
            self.project.settings.height,
        );
        let request = RenderLaunchRequest {
            output_path: path,
            format,
            width,
            height,
            fps: self.project.settings.fps,
            crf: recipe.crf,
            engine,
            burn_captions: false,
            loop_region_only: false,
            save_loudness_log: true,
        };
        self.start_render(request, ctx);
    }

    /// *Arquivar projeto…* (`consolidate` false: whole files) and *Consolidar mídia…* (`consolidate`
    /// true: only the used part of each file, with a margin).
    pub(super) fn archive_to_folder(&mut self, consolidate: bool) {
        let Some(folder) = rfd::FileDialog::new()
            .set_title(if consolidate {
                "Consolidar mídia em uma pasta"
            } else {
                "Arquivar projeto em uma pasta"
            })
            .pick_folder()
        else {
            return;
        };
        let options = ArchiveOptions {
            only_used: consolidate,
            margin: CONSOLIDATE_MARGIN,
        };
        let result = archive::archive_project(
            &self.project,
            &self.timeline,
            &folder,
            options,
            &mut ffmpeg_cut,
        );
        self.status = match result {
            Ok(r) => tr_args(
                "Projeto arquivado em {}: {} mídias copiadas, {} recortadas, {} ausentes",
                &[
                    &folder.display(),
                    &r.copied.len(),
                    &r.cut.len(),
                    &r.missing.len(),
                ],
            ),
            Err(e) => tr_args("Não foi possível arquivar o projeto: {}", &[&e]),
        };
    }

    /// *Reenquadrar para vertical*: saves a 9:16 copy of the project beside the original, every
    /// video event with a Pan/Crop box of the portrait shape (centered; adjust each event by hand
    /// afterwards). The open project is not changed.
    pub(super) fn save_vertical_copy(&mut self) {
        let (info, timeline) = neoveg_engine::vertical_reframe::reframe_project(
            &self.project,
            &self.timeline,
            |_| Vec::new(),
            |_| None,
        );
        let dir = self
            .project_path
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf);
        let Some(dir) = dir.or_else(|| rfd::FileDialog::new().pick_folder()) else {
            return;
        };
        let file = dir.join(format!(
            "{}.{}",
            archive::sanitize_file_name(&info.name),
            project_file::EXTENSION
        ));
        self.status = match project_file::save(&file, &info, &timeline) {
            Ok(()) => tr_args("Cópia vertical (9:16) salva em {}", &[&file.display()]),
            Err(e) => tr_args("Não foi possível salvar a cópia vertical: {}", &[&e]),
        };
    }
}

/// The window of *Renomear eventos por metadados*.
pub(super) struct RenameDialog {
    pub open: bool,
    template: String,
    start: u32,
}

impl Default for RenameDialog {
    fn default() -> Self {
        Self {
            open: false,
            template: "{data}_{n:3}".into(),
            start: 1,
        }
    }
}

impl AppState {
    /// The metadata of the file behind an event: its modification time (UTC); the camera is not
    /// read yet.
    fn event_metadata(&self, event: &EventRef) -> neoveg_common::rename_pattern::ItemMetadata {
        let secs = self
            .timeline
            .track(event.0)
            .and_then(|t| t.events.iter().find(|e| e.id == event.1))
            .and_then(|e| e.media_id)
            .and_then(|id| self.project.media_pool.iter().find(|m| m.id == id))
            .and_then(|m| std::fs::metadata(&m.path).ok())
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64);
        match secs {
            Some(secs) => neoveg_common::rename_pattern::ItemMetadata::from_unix("", secs, None),
            None => neoveg_common::rename_pattern::ItemMetadata::default(),
        }
    }

    /// Draws the rename window and applies it as one undo step.
    pub(super) fn show_rename_dialog(&mut self, ctx: &egui::Context) {
        if !self.rename_dialog.open {
            return;
        }
        let mut open = true;
        let mut apply = false;
        let template_error = neoveg_common::rename_pattern::validate(&self.rename_dialog.template).err();
        crate::dialogs::dialog(ctx, tr("Renomear eventos por metadados"))
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(tr(
                    "Modelo (campos: {data} {hora} {camera} {nome} {n} {n:3}):",
                ));
                ui.text_edit_singleline(&mut self.rename_dialog.template);
                ui.horizontal(|ui| {
                    ui.label(tr("Primeiro número:"));
                    ui.add(egui::DragValue::new(&mut self.rename_dialog.start).range(0..=99_999));
                });
                if let Some(error) = &template_error {
                    ui.colored_label(egui::Color32::from_rgb(242, 100, 92), error);
                }
                if ui
                    .add_enabled(template_error.is_none(), egui::Button::new(tr("Renomear")))
                    .clicked()
                {
                    apply = true;
                }
            });
        if apply {
            let refs = self.selection.refs();
            let result = neoveg_engine::event_rename::rename_command(
                &self.timeline,
                &refs,
                &self.rename_dialog.template,
                self.rename_dialog.start,
                |r| self.event_metadata(r),
            );
            match result {
                Ok(Some(command)) => {
                    let count = match &command {
                        EditCommand::RenameEvents { changes } => changes.len(),
                        _ => 0,
                    };
                    self.execute_timeline(command);
                    self.status = tr_args("{} eventos renomeados", &[&count]);
                    open = false;
                }
                Ok(None) => self.status = tr("Os nomes já estão assim").to_string(),
                Err(e) => self.status = e,
            }
        }
        self.rename_dialog.open = open;
    }
}
