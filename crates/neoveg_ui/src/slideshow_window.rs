//! Faixas > Inserir apresentação de slides: choose still pictures, put them in order, give them a
//! length and the overlap that makes the transition, and lay them on a video track.
//!
//! The window follows the slideshow creator the reference manuals describe: add pictures, remove
//! the selected ones, move them one place, sort from A to Z or Z to A, from new to old or old to
//! new, shuffle, then choose where the show starts (at the cursor or at an exact time) and how long
//! each picture stays (or how long the whole show lasts). The rules are in
//! `neoveg_engine::slideshow`; this file is only the screen.

use eframe::egui;
use neoveg_engine::slideshow::{
    layout, move_selected, per_photo_for_total, sort_slides, total_length, Slide, SlideOrder,
    SlideshowSettings,
};

/// What the user asked for when pressing Insert.
#[derive(Debug, Clone, PartialEq)]
pub struct SlideshowRequest {
    pub slides: Vec<Slide>,
    pub settings: SlideshowSettings,
}

#[derive(Debug, Clone)]
pub struct SlideshowState {
    pub open: bool,
    pub slides: Vec<Slide>,
    pub selected: Vec<usize>,
    pub per_photo: f64,
    /// Set the length of the whole show instead of each picture.
    pub use_total: bool,
    pub total: f64,
    pub overlap: f64,
    pub at_cursor: bool,
    pub start: f64,
    shuffles: u64,
}

impl Default for SlideshowState {
    fn default() -> Self {
        Self {
            open: false,
            slides: Vec::new(),
            selected: Vec::new(),
            per_photo: 5.0,
            use_total: false,
            total: 30.0,
            overlap: 1.0,
            at_cursor: true,
            start: 0.0,
            shuffles: 0,
        }
    }
}

/// The modification time of a file in seconds since the Unix epoch (`0` when unknown).
pub fn modified_seconds(path: &std::path::Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}

impl SlideshowState {
    /// The settings the window describes, for a cursor at `playhead`.
    pub fn settings(&self, playhead: f64) -> SlideshowSettings {
        let overlap = self.overlap.max(0.0);
        let per_photo = if self.use_total {
            per_photo_for_total(self.slides.len(), self.total, overlap)
        } else {
            self.per_photo
        };
        SlideshowSettings {
            start: if self.at_cursor { playhead } else { self.start },
            per_photo,
            overlap,
        }
    }

    /// Adds pictures at the end of the list.
    pub fn add_pictures(&mut self, paths: Vec<std::path::PathBuf>) {
        for path in paths {
            let text = path.to_string_lossy().into_owned();
            if self.slides.iter().any(|s| s.path == text) {
                continue;
            }
            self.slides.push(Slide::new(&text, modified_seconds(&path)));
        }
    }

    pub fn remove_selected(&mut self) {
        let mut i = 0;
        self.slides.retain(|_| {
            let keep = !self.selected.contains(&i);
            i += 1;
            keep
        });
        self.selected.clear();
    }

    pub fn show(&mut self, ctx: &egui::Context, playhead: f64) -> Option<SlideshowRequest> {
        if !self.open {
            return None;
        }
        let mut open = self.open;
        let mut request = None;
        crate::dialogs::dialog(ctx, "Apresentação de slides")
            .open(&mut open)
            .collapsible(false)
            .default_width(560.0)
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Adicionar imagens…").clicked() {
                        if let Some(paths) = rfd::FileDialog::new()
                            .add_filter("Imagens", &neoveg_video::still::IMAGE_EXTENSIONS)
                            .pick_files()
                        {
                            self.add_pictures(paths);
                        }
                    }
                    let has_selection = !self.selected.is_empty();
                    ui.add_enabled_ui(has_selection, |ui| {
                        if ui.button("Remover").clicked() {
                            self.remove_selected();
                        }
                        if ui.button("Mover −1").clicked() {
                            self.selected = move_selected(&mut self.slides, &self.selected, -1);
                        }
                        if ui.button("Mover +1").clicked() {
                            self.selected = move_selected(&mut self.slides, &self.selected, 1);
                        }
                    });
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("Ordenar:");
                    for (label, order) in [
                        ("A a Z", SlideOrder::NameAscending),
                        ("Z a A", SlideOrder::NameDescending),
                        ("Novo para antigo", SlideOrder::NewestFirst),
                        ("Antigo para novo", SlideOrder::OldestFirst),
                    ] {
                        if ui.button(label).clicked() {
                            sort_slides(&mut self.slides, order);
                            self.selected.clear();
                        }
                    }
                    if ui.button("Embaralhar").clicked() {
                        self.shuffles += 1;
                        let seed = (ui.input(|i| i.time) * 1000.0) as u64 ^ (self.shuffles << 32);
                        sort_slides(&mut self.slides, SlideOrder::Shuffle(seed));
                        self.selected.clear();
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .show(ui, |ui| {
                        if self.slides.is_empty() {
                            ui.weak("Nenhuma imagem. Use Adicionar imagens…");
                        }
                        let additive = ui.input(|i| i.modifiers.command);
                        for (i, slide) in self.slides.iter().enumerate() {
                            let on = self.selected.contains(&i);
                            if ui
                                .selectable_label(on, format!("{:>3}  {}", i + 1, slide.name))
                                .on_hover_text(&slide.path)
                                .clicked()
                            {
                                if additive {
                                    if on {
                                        self.selected.retain(|&s| s != i);
                                    } else {
                                        self.selected.push(i);
                                    }
                                } else {
                                    self.selected = vec![i];
                                }
                            }
                        }
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.at_cursor, true, "Começar no cursor");
                    ui.radio_value(&mut self.at_cursor, false, "Começar em (s):");
                    ui.add_enabled(
                        !self.at_cursor,
                        egui::DragValue::new(&mut self.start)
                            .range(0.0..=36_000.0)
                            .speed(0.1),
                    );
                });
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.use_total, false, "Duração de cada imagem (s):");
                    ui.add_enabled(
                        !self.use_total,
                        egui::DragValue::new(&mut self.per_photo)
                            .range(0.1..=3_600.0)
                            .speed(0.1),
                    );
                    ui.radio_value(&mut self.use_total, true, "Duração total (s):");
                    ui.add_enabled(
                        self.use_total,
                        egui::DragValue::new(&mut self.total)
                            .range(0.1..=36_000.0)
                            .speed(0.5),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Sobreposição (transição, s):");
                    ui.add(
                        egui::DragValue::new(&mut self.overlap)
                            .range(0.0..=30.0)
                            .speed(0.05),
                    );
                    let settings = self.settings(playhead);
                    ui.weak(format!(
                        "{} imagens · {:.1} s por imagem · {:.1} s no total",
                        self.slides.len(),
                        settings.per_photo,
                        total_length(self.slides.len(), &settings)
                    ));
                });
                ui.horizontal(|ui| {
                    let ready = !self.slides.is_empty();
                    if ui
                        .add_enabled(ready, egui::Button::new("Inserir na linha do tempo"))
                        .clicked()
                    {
                        let settings = self.settings(playhead);
                        let starts = layout(self.slides.len(), &settings);
                        if !starts.is_empty() {
                            request = Some(SlideshowRequest {
                                slides: self.slides.clone(),
                                settings,
                            });
                        }
                    }
                    ui.weak("A sobreposição só vira crossfade com o crossfade automático ligado.");
                });
            });
        self.open = open && request.is_none();
        request
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pictures(names: &[&str]) -> SlideshowState {
        let mut s = SlideshowState::default();
        s.add_pictures(
            names
                .iter()
                .map(|n| std::path::PathBuf::from(format!("/fotos/{n}")))
                .collect(),
        );
        s
    }

    #[test]
    fn a_picture_added_twice_is_listed_once_and_removal_takes_the_selected_ones() {
        let mut s = pictures(&["a.png", "b.png", "c.png"]);
        s.add_pictures(vec![std::path::PathBuf::from("/fotos/a.png")]);
        assert_eq!(s.slides.len(), 3);
        s.selected = vec![0, 2];
        s.remove_selected();
        assert_eq!(
            s.slides.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            vec!["b.png"]
        );
        assert!(s.selected.is_empty());
    }

    #[test]
    fn the_settings_follow_the_cursor_or_an_exact_start_and_each_picture_or_the_total() {
        let mut s = pictures(&["a.png", "b.png", "c.png", "d.png"]);
        let each = s.settings(12.0);
        assert_eq!((each.start, each.per_photo, each.overlap), (12.0, 5.0, 1.0));
        s.at_cursor = false;
        s.start = 3.0;
        s.use_total = true;
        s.total = 30.0;
        let whole = s.settings(12.0);
        assert_eq!(whole.start, 3.0);
        assert!(
            (total_length(4, &whole) - 30.0).abs() < 1e-9,
            "the show lasts what was asked"
        );
    }
}
