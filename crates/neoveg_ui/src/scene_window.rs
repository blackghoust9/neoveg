//! *Detectar cenas e dividir*: the options of the scene detection and its progress. It only draws
//! and reports a [`SceneAction`]; the detection runs on its own thread (`app/scene_detect.rs`).

use eframe::egui;
use neoveg_common::i18n::{tr, tr_args};
use neoveg_video::scene::SceneOptions;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SceneAction {
    Start(SceneOptions),
    Cancel,
}

#[derive(Default)]
pub struct SceneDialog {
    pub open: bool,
    pub options: SceneOptions,
}

impl SceneDialog {
    /// Draws the window. `progress` is `Some(0..=1)` while a detection runs on the event called
    /// `event_name`.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        event_name: &str,
        progress: Option<f32>,
    ) -> Option<SceneAction> {
        if !self.open {
            return None;
        }
        let mut action = None;
        let mut open = true;
        crate::dialogs::dialog(ctx, tr("Detectar cenas e dividir"))
            .open(&mut open)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(tr_args("Evento: {}", &[&event_name]));
                ui.add_enabled_ui(progress.is_none(), |ui| {
                    ui.add(
                        egui::Slider::new(&mut self.options.sensitivity, 0.0..=100.0)
                            .text(tr("Sensibilidade"))
                            .fixed_decimals(0),
                    )
                    .on_hover_text(tr(
                        "Baixa: só os cortes óbvios. Alta: também as trocas sutis de plano.",
                    ));
                    ui.add(
                        egui::DragValue::new(&mut self.options.min_shot_frames)
                            .range(1..=600)
                            .prefix(tr("Plano mínimo (quadros): ")),
                    );
                });
                ui.separator();
                match progress {
                    Some(fraction) => {
                        ui.add(egui::ProgressBar::new(fraction).show_percentage());
                        if ui.button(tr("Cancelar")).clicked() {
                            action = Some(SceneAction::Cancel);
                        }
                    }
                    None => {
                        ui.horizontal(|ui| {
                            if ui.button(tr("Detectar e dividir")).clicked() {
                                action = Some(SceneAction::Start(self.options));
                            }
                            if ui.button(tr("Fechar")).clicked() {
                                self.open = false;
                            }
                        });
                    }
                }
            });
        if !open {
            self.open = false;
            if progress.is_some() {
                action = Some(SceneAction::Cancel);
            }
        }
        action
    }
}
