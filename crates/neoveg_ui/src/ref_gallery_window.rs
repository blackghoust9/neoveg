//! Exibir > Galeria de quadros de referência: keeps the current preview frame (with its correction
//! applied), lists the kept frames and compares one of them with the current frame by a wipe or
//! side by side, inside the window.
//!
//! The state holds the logic (save, remove, compose) and is tested without a window.

use std::path::PathBuf;

use eframe::egui::{self, TextureHandle, TextureOptions};
use neoveg_common::i18n::tr;
use neoveg_video::frame::FrameBuffer;
use neoveg_video::ref_gallery::{compare, CompareMode, ReferenceGallery};

/// How the reference is shown against the current frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    /// Only the current frame.
    #[default]
    Off,
    WipeVertical,
    WipeHorizontal,
    SideBySide,
}

impl ViewMode {
    pub const ALL: [ViewMode; 4] = [
        ViewMode::Off,
        ViewMode::WipeVertical,
        ViewMode::WipeHorizontal,
        ViewMode::SideBySide,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Quadro atual",
            Self::WipeVertical => "Cortina vertical",
            Self::WipeHorizontal => "Cortina horizontal",
            Self::SideBySide => "Lado a lado",
        }
    }

    fn compare_mode(self, position: f32) -> Option<CompareMode> {
        match self {
            Self::Off => None,
            Self::WipeVertical => Some(CompareMode::WipeVertical { position }),
            Self::WipeHorizontal => Some(CompareMode::WipeHorizontal { position }),
            Self::SideBySide => Some(CompareMode::SideBySide),
        }
    }
}

#[derive(Default)]
pub struct RefGalleryWindow {
    pub open: bool,
    gallery: Option<ReferenceGallery>,
    selected: Option<u64>,
    reference: Option<(u64, FrameBuffer)>,
    mode: ViewMode,
    position: f32,
    name: String,
    pub status: String,
    texture: Option<TextureHandle>,
    shown_key: Option<(u64, u64, ViewMode, i32)>,
}

impl RefGalleryWindow {
    /// Opens the gallery folder (once); an error is kept as the status.
    pub fn ensure_open(&mut self, dir: impl Into<PathBuf>) {
        if self.gallery.is_some() {
            return;
        }
        match ReferenceGallery::open(dir.into()) {
            Ok(g) => self.gallery = Some(g),
            Err(e) => self.status = e.to_string(),
        }
    }

    #[allow(dead_code)]
    pub fn entry_count(&self) -> usize {
        self.gallery.as_ref().map_or(0, |g| g.entries().len())
    }

    /// Keeps `frame` under `name` (a default name when empty) and selects it.
    pub fn save_current(&mut self, frame: &FrameBuffer, name: &str) -> Result<u64, String> {
        let gallery = self
            .gallery
            .as_mut()
            .ok_or_else(|| tr("A galeria não está aberta.").to_string())?;
        let name = if name.trim().is_empty() {
            format!("{} {}", tr("Quadro"), gallery.entries().len() + 1)
        } else {
            name.trim().to_string()
        };
        let id = gallery.add(&name, frame).map_err(|e| e.to_string())?;
        self.select(Some(id));
        Ok(id)
    }

    pub fn select(&mut self, id: Option<u64>) {
        self.selected = id;
        self.reference = None;
        self.shown_key = None;
    }

    pub fn remove(&mut self, id: u64) -> Result<(), String> {
        let gallery = self
            .gallery
            .as_mut()
            .ok_or_else(|| tr("A galeria não está aberta.").to_string())?;
        gallery.remove(id).map_err(|e| e.to_string())?;
        if self.selected == Some(id) {
            self.select(None);
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn set_mode(&mut self, mode: ViewMode, position: f32) {
        self.mode = mode;
        self.position = position.clamp(0.0, 1.0);
    }

    /// The picture to show: the comparison of the selected reference with `current`, or `current`
    /// itself when there is no comparison to make.
    pub fn compose(&mut self, current: &FrameBuffer) -> FrameBuffer {
        let Some(id) = self.selected else {
            return current.clone();
        };
        let Some(mode) = self.mode.compare_mode(self.position) else {
            return current.clone();
        };
        if self.reference.as_ref().map(|r| r.0) != Some(id) {
            self.reference = self
                .gallery
                .as_ref()
                .and_then(|g| g.load(id).ok())
                .map(|f| (id, f));
        }
        match &self.reference {
            Some((_, reference)) => compare(current, reference, mode),
            None => current.clone(),
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, current: Option<&FrameBuffer>, version: u64) {
        let mut open = self.open;
        egui::Window::new(tr("Galeria de quadros de referência"))
            .id(egui::Id::new("Galeria de quadros de referência"))
            .open(&mut open)
            .default_width(420.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.name);
                    if ui
                        .add_enabled(
                            current.is_some(),
                            egui::Button::new(tr("Guardar quadro atual")),
                        )
                        .clicked()
                    {
                        if let Some(frame) = current {
                            let name = std::mem::take(&mut self.name);
                            self.status = match self.save_current(frame, &name) {
                                Ok(_) => tr("Quadro guardado na galeria.").to_string(),
                                Err(e) => e,
                            };
                        }
                    }
                });
                let entries: Vec<(u64, String)> = self
                    .gallery
                    .as_ref()
                    .map(|g| g.entries().iter().map(|e| (e.id, e.name.clone())).collect())
                    .unwrap_or_default();
                if entries.is_empty() {
                    ui.label(tr("Nenhum quadro guardado."));
                }
                let mut remove = None;
                for (id, name) in &entries {
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(self.selected == Some(*id), name)
                            .clicked()
                        {
                            self.select(Some(*id));
                        }
                        if ui.small_button(tr("Remover")).clicked() {
                            remove = Some(*id);
                        }
                    });
                }
                if let Some(id) = remove {
                    if let Err(e) = self.remove(id) {
                        self.status = e;
                    }
                }
                ui.separator();
                ui.horizontal(|ui| {
                    for mode in ViewMode::ALL {
                        ui.selectable_value(&mut self.mode, mode, tr(mode.label()));
                    }
                });
                if matches!(self.mode, ViewMode::WipeVertical | ViewMode::WipeHorizontal) {
                    ui.add(egui::Slider::new(&mut self.position, 0.0..=1.0).text(tr("Posição")));
                }
                if let Some(frame) = current {
                    let key = (
                        version,
                        self.selected.unwrap_or(0),
                        self.mode,
                        (self.position * 1000.0) as i32,
                    );
                    if self.shown_key != Some(key) || self.texture.is_none() {
                        let composed = self.compose(frame);
                        let image = egui::ColorImage::from_rgba_unmultiplied(
                            [composed.width as usize, composed.height as usize],
                            &composed.rgba,
                        );
                        self.texture =
                            Some(ctx.load_texture("ref_gallery", image, TextureOptions::LINEAR));
                        self.shown_key = Some(key);
                    }
                    if let Some(texture) = &self.texture {
                        let width = ui.available_width().clamp(160.0, 640.0);
                        let aspect = frame.height.max(1) as f32 / frame.width.max(1) as f32;
                        ui.image((texture.id(), egui::vec2(width, width * aspect)));
                    }
                } else {
                    ui.label(tr("A prévia ainda não tem quadro."));
                }
                if !self.status.is_empty() {
                    ui.label(&self.status);
                }
            });
        self.open = open;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("neoveg_gallery_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_saved_frame_is_listed_selected_and_compared_by_wipe_and_side_by_side() {
        let dir = temp_dir("a");
        let mut w = RefGalleryWindow::default();
        assert!(w
            .save_current(&FrameBuffer::solid(8, 8, [1, 1, 1, 255]), "x")
            .is_err());
        w.ensure_open(&dir);
        let red = FrameBuffer::solid(8, 8, [255, 0, 0, 255]);
        let blue = FrameBuffer::solid(8, 8, [0, 0, 255, 255]);
        let id = w.save_current(&red, "").unwrap();
        assert_eq!(w.entry_count(), 1);
        assert_eq!(w.selected, Some(id));
        // No comparison until a mode is chosen
        assert_eq!(w.compose(&blue), blue);
        w.set_mode(ViewMode::WipeVertical, 0.5);
        let wipe = w.compose(&blue);
        assert_eq!(&wipe.rgba[0..4], &[255, 0, 0, 255]);
        assert_eq!(&wipe.rgba[7 * 4..7 * 4 + 4], &[0, 0, 255, 255]);
        w.set_mode(ViewMode::SideBySide, 0.5);
        let side = w.compose(&blue);
        assert_eq!(&side.rgba[0..4], &[255, 0, 0, 255]);
        assert_eq!(&side.rgba[7 * 4..7 * 4 + 4], &[0, 0, 255, 255]);
        // The kept frame survives closing and opening the gallery
        let mut again = RefGalleryWindow::default();
        again.ensure_open(&dir);
        assert_eq!(again.entry_count(), 1);
        w.remove(id).unwrap();
        assert_eq!(w.entry_count(), 0);
        assert_eq!(w.compose(&blue), blue);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
