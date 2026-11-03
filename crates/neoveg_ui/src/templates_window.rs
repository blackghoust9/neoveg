//! The list of the project templates the user saved (the resolution and frame rate presets of
//! the welcome screen), with a button to delete each one. The templates that ship with Neoveg are
//! only counted: they cannot be changed. New ones are made in the project properties, with the
//! *Salvar modelo* button.

use eframe::egui;
use neoveg_common::i18n::{tr, tr_args};
use neoveg_common::presets::{builtin_templates, ProjectTemplate};

/// What the user did in the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplatesAction {
    /// Forget the template with this name.
    Delete(String),
}

#[derive(Debug, Default)]
pub struct TemplatesWindow {
    pub open: bool,
}

/// `1920x1080 a 29,97 quadros/s`, the line under the name of a template.
pub fn summary(template: &ProjectTemplate) -> String {
    let s = &template.settings;
    tr_args(
        "{}x{} a {} quadros/s",
        &[
            &s.width.to_string(),
            &s.height.to_string(),
            &format!("{:.3}", s.fps)
                .trim_end_matches('0')
                .trim_end_matches('.')
                .replace('.', ","),
        ],
    )
}

impl TemplatesWindow {
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        custom: &[ProjectTemplate],
    ) -> Vec<TemplatesAction> {
        let mut actions = Vec::new();
        if !self.open {
            return actions;
        }
        let mut open = self.open;
        crate::dialogs::dialog(ctx, tr("Presets de resolução"))
            .open(&mut open)
            .default_width(420.0)
            .show(ctx, |ui| {
                ui.label(tr_args(
                    "{} modelos vêm com o Neoveg e não podem ser mudados.",
                    &[&builtin_templates().len().to_string()],
                ));
                ui.add_space(4.0);
                ui.heading(tr("Seus modelos"));
                if custom.is_empty() {
                    ui.weak(tr(
                        "Nenhum ainda. Em Propriedades do projeto, ajuste a resolução e a taxa de quadros e use Salvar modelo.",
                    ));
                }
                for template in custom {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.strong(&template.name);
                            ui.weak(summary(template));
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(tr("Excluir")).clicked() {
                                actions.push(TemplatesAction::Delete(template.name.clone()));
                            }
                        });
                    });
                    ui.separator();
                }
            });
        self.open = open;
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neoveg_common::project::ProjectSettings;

    #[test]
    fn the_summary_names_the_size_and_the_rate() {
        let template = ProjectTemplate {
            name: "meu".into(),
            settings: ProjectSettings {
                width: 1920,
                height: 1080,
                fps: 30_000.0 / 1001.0,
                ..ProjectSettings::default()
            },
        };
        assert_eq!(summary(&template), "1920x1080 a 29,97 quadros/s");
        let mut whole = template;
        whole.settings.fps = 24.0;
        assert_eq!(summary(&whole), "1920x1080 a 24 quadros/s");
    }
}
