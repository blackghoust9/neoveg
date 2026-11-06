//! Title templates in the generator dialog: a library of `.neovegtitulo` files, a picker, and the
//! fields (`{{name}}`) of the text filled in before the title is inserted.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use eframe::egui;
use neoveg_common::i18n::{tr, tr_args};
use neoveg_video::generators::GeneratorKind;
use neoveg_video::title_template::{TitleTemplate, TEMPLATE_EXTENSION};

use crate::generator_dialog::GeneratorDialogState;

/// State of the template controls (kept inside the generator dialog).
#[derive(Default)]
pub struct TemplateUi {
    /// Values typed for the fields of the text, by field name.
    pub values: BTreeMap<String, String>,
    /// The name typed for saving the current title as a template.
    pub name: String,
    /// Last result shown under the controls.
    pub message: String,
    /// Library files found the last time the folder was read.
    library: Vec<PathBuf>,
    library_read: bool,
}

/// Folder where the templates of the user are kept.
pub fn library_dir() -> Option<PathBuf> {
    neoveg_common::paths::app_data_dir().map(|d| d.join("titulos"))
}

/// The `.neovegtitulo` files of `dir`, sorted by name.
pub fn list_templates(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|e| e == TEMPLATE_EXTENSION))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

/// The current title of the dialog as a template named `name`.
pub fn current_template(dialog: &GeneratorDialogState, name: &str) -> TitleTemplate {
    let mut spec = dialog.build_spec_raw(1280, 720, 29.97);
    spec.name = name.to_string();
    let mut template = TitleTemplate::new(name, spec);
    template.defaults = dialog.template.values.clone();
    template
}

/// Puts a template in the controls of the dialog: look, text with its markers, default values.
pub fn load_template(dialog: &mut GeneratorDialogState, template: &TitleTemplate) -> bool {
    let GeneratorKind::TitleText {
        text,
        scale,
        text_rgba,
        bg_rgba,
        outline_rgba,
        outline_px,
        shadow_rgba,
        shadow_offset,
        align_y,
        animation,
        font,
        font_style,
        layout,
    } = &template.spec.kind
    else {
        return false;
    };
    dialog.title_text = text.clone();
    dialog.title_scale = *scale;
    dialog.title_rgba = *text_rgba;
    dialog.title_bg_rgba = *bg_rgba;
    dialog.title_outline_rgba = *outline_rgba;
    dialog.title_outline_px = *outline_px;
    dialog.title_shadow_rgba = *shadow_rgba;
    dialog.title_shadow_offset = *shadow_offset;
    dialog.title_align_y = *align_y;
    dialog.title_animation = *animation;
    dialog.title_font = font.clone();
    dialog.title_font_style = font_style.clone();
    dialog.title_layout = layout.clone();
    dialog.duration_seconds = template.spec.duration;
    dialog.template.values = template.defaults.clone();
    dialog.template.name = template.name.clone();
    true
}

fn file_stem_name(name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
        .collect();
    clean.trim().to_string()
}

/// The template row of the title tab: picker, save, open, and one box for each field of the text.
pub fn template_row(ui: &mut egui::Ui, dialog: &mut GeneratorDialogState) {
    let dir = library_dir();
    if !dialog.template.library_read {
        if let Some(d) = &dir {
            dialog.template.library = list_templates(d);
        }
        dialog.template.library_read = true;
    }
    egui::CollapsingHeader::new(tr("Modelos de título (campos {{nome}})"))
        .id_salt("gen_title_templates")
        .default_open(false)
        .show(ui, |ui| {
            let mut picked: Option<PathBuf> = None;
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("gen_title_template_pick")
                    .selected_text(tr("Escolher modelo…"))
                    .show_ui(ui, |ui| {
                        for p in &dialog.template.library {
                            let label = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                            if ui.selectable_label(false, label).clicked() {
                                picked = Some(p.clone());
                            }
                        }
                    });
                if ui.button(tr("Abrir arquivo…")).clicked() {
                    picked = rfd::FileDialog::new()
                        .add_filter(tr("Modelo de título"), &[TEMPLATE_EXTENSION])
                        .pick_file();
                }
            });
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut dialog.template.name)
                        .hint_text(tr("Nome do modelo"))
                        .desired_width(160.0),
                );
                let named = !dialog.template.name.trim().is_empty();
                if ui.add_enabled(named, egui::Button::new(tr("Salvar modelo"))).clicked() {
                    let name = dialog.template.name.trim().to_string();
                    let template = current_template(dialog, &name);
                    let result = match &dir {
                        Some(d) => std::fs::create_dir_all(d)
                            .map_err(|e| e.to_string())
                            .and_then(|()| {
                                let path = d.join(format!("{}.{TEMPLATE_EXTENSION}", file_stem_name(&name)));
                                template.save(&path).map(|()| path)
                            }),
                        None => Err(tr("pasta de dados indisponível").to_string()),
                    };
                    dialog.template.message = match result {
                        Ok(path) => {
                            dialog.template.library_read = false;
                            tr_args("Modelo salvo: {}", &[&path.display().to_string()])
                        }
                        Err(e) => tr_args("Falha ao salvar o modelo: {}", &[&e]),
                    };
                }
                if ui.add_enabled(named, egui::Button::new(tr("Salvar como…"))).clicked() {
                    let name = dialog.template.name.trim().to_string();
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter(tr("Modelo de título"), &[TEMPLATE_EXTENSION])
                        .set_file_name(format!("{}.{TEMPLATE_EXTENSION}", file_stem_name(&name)))
                        .save_file()
                    {
                        dialog.template.message = match current_template(dialog, &name).save(&path) {
                            Ok(()) => tr_args("Modelo salvo: {}", &[&path.display().to_string()]),
                            Err(e) => tr_args("Falha ao salvar o modelo: {}", &[&e]),
                        };
                    }
                }
            });
            if let Some(path) = picked {
                dialog.template.message = match TitleTemplate::load(&path) {
                    Ok(t) if load_template(dialog, &t) => tr_args("Modelo aberto: {}", &[&t.name]),
                    Ok(_) => tr("O arquivo não é um modelo de título.").to_string(),
                    Err(e) => e,
                };
            }

            let fields = neoveg_video::title_template::field_names(&dialog.title_text);
            if fields.is_empty() {
                ui.weak(tr("Escreva {{nome}} no texto para criar um campo preenchível."));
            } else {
                ui.label(tr("Campos do texto:"));
                dialog.template.values.retain(|k, _| fields.contains(k));
                for name in fields {
                    ui.horizontal(|ui| {
                        ui.label(&name);
                        let value = dialog.template.values.entry(name).or_default();
                        ui.add(egui::TextEdit::singleline(value).desired_width(220.0));
                    });
                }
            }
            if !dialog.template.message.is_empty() {
                ui.weak(&dialog.template.message);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_template_round_trips_through_the_dialog_and_fills_its_fields() {
        let mut dialog = GeneratorDialogState::default();
        dialog.title_text = "Olá {{nome}}, {{cargo}}".into();
        dialog.title_scale = 9;
        dialog.template.values.insert("cargo".into(), "diretor".into());
        let template = current_template(&dialog, "Apresentação");

        let mut other = GeneratorDialogState::default();
        assert!(load_template(&mut other, &template));
        assert_eq!(other.title_scale, 9);
        assert_eq!(other.title_text, "Olá {{nome}}, {{cargo}}");
        other.template.values.insert("nome".into(), "Ana".into());
        // The spec made by the dialog has the fields filled in
        match other.build_spec(640, 360, 25.0).kind {
            GeneratorKind::TitleText { text, .. } => assert_eq!(text, "Olá Ana, diretor"),
            _ => panic!("expected a title"),
        }
    }

    #[test]
    fn templates_saved_in_a_folder_are_listed() {
        let dir = std::env::temp_dir().join(format!("neoveg_tpl_ui_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut dialog = GeneratorDialogState::default();
        dialog.title_text = "{{a}}".into();
        let template = current_template(&dialog, "Um");
        template.save(&dir.join(format!("um.{TEMPLATE_EXTENSION}"))).unwrap();
        std::fs::write(dir.join("outro.txt"), "x").unwrap();
        let found = list_templates(&dir);
        assert_eq!(found.len(), 1);
        let loaded = TitleTemplate::load(&found[0]).unwrap();
        assert_eq!(loaded.fields().len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
