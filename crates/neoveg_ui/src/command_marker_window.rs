//! Command Properties (manual v21, "Inserting a command marker"): the command, its parameter, a
//! comment and the position of a command marker, with templates that fill the boxes in. The dialog
//! edits a draft and reports [`CommandDialogAction`]s; the edits themselves are
//! `neoveg_engine::command_markers` and `neoveg_common::command_template`.

use eframe::egui;
use neoveg_common::command_template::{clean_name, CommandTemplate};
use neoveg_common::i18n::tr;
use neoveg_common::timecode::seconds_to_timecode;
use neoveg_engine::command_markers::{CommandId, CommandMarker, SUGGESTED};

use crate::media_properties_window::parse_start;

/// What the user asked for.
#[derive(Debug, Clone, PartialEq)]
pub enum CommandDialogAction {
    /// Put the marker in the project: a new one (`id` is `None`) or the changed one.
    Apply {
        id: Option<CommandId>,
        time: f64,
        command: String,
        parameter: String,
        comment: String,
    },
    SaveTemplate(CommandTemplate),
    DeleteTemplate(String),
}

struct Draft {
    id: Option<CommandId>,
    command: String,
    parameter: String,
    comment: String,
    /// The position as typed (a timecode).
    position: String,
    template: String,
    fps: f64,
}

#[derive(Default)]
pub struct CommandDialog {
    draft: Option<Draft>,
}

impl CommandDialog {
    #[cfg(test)]
    pub fn is_open(&self) -> bool {
        self.draft.is_some()
    }

    /// Opens the dialog for a new marker at `time`.
    pub fn open_new(&mut self, time: f64, fps: f64, drop_frame: bool) {
        self.draft = Some(Draft {
            id: None,
            command: SUGGESTED[0].to_owned(),
            parameter: String::new(),
            comment: String::new(),
            position: seconds_to_timecode(time, fps, drop_frame),
            template: String::new(),
            fps,
        });
    }

    /// Opens the dialog on an existing marker.
    pub fn open_edit(&mut self, marker: &CommandMarker, fps: f64, drop_frame: bool) {
        self.draft = Some(Draft {
            id: Some(marker.id),
            command: marker.command.clone(),
            parameter: marker.parameter.clone(),
            comment: marker.comment.clone(),
            position: seconds_to_timecode(marker.time, fps, drop_frame),
            template: String::new(),
            fps,
        });
    }

    /// Draws the dialog; `templates` are the saved ones.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        templates: &[CommandTemplate],
    ) -> Vec<CommandDialogAction> {
        let mut actions = Vec::new();
        let Some(mut draft) = self.draft.take() else {
            return actions;
        };
        let mut open = true;
        let mut done = false;
        crate::dialogs::dialog(ctx, tr("Propriedades do comando"))
            .open(&mut open)
            .resizable(false)
            .show(ctx, |ui| {
                egui::Grid::new("command_marker_grid")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        // Template: pick one to fill the boxes, or type a name and save the boxes
                        ui.label(tr("Modelo"));
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt("command_template_pick")
                                .selected_text(tr("Escolher…"))
                                .width(110.0)
                                .show_ui(ui, |ui| {
                                    for t in templates {
                                        if ui.selectable_label(false, &t.name).clicked() {
                                            draft.template = t.name.clone();
                                            draft.command = t.command.clone();
                                            draft.parameter = t.parameter.clone();
                                            draft.comment = t.comment.clone();
                                        }
                                    }
                                });
                            ui.add(
                                egui::TextEdit::singleline(&mut draft.template)
                                    .desired_width(120.0),
                            );
                            let name = clean_name(&draft.template);
                            if ui
                                .add_enabled(
                                    name.is_some() && !draft.command.trim().is_empty(),
                                    egui::Button::new(tr("Salvar")),
                                )
                                .on_hover_text(tr(
                                    "Guarda o comando, o parâmetro e o comentário sob esse nome",
                                ))
                                .clicked()
                            {
                                if let Some(name) = name {
                                    actions.push(CommandDialogAction::SaveTemplate(
                                        CommandTemplate {
                                            name,
                                            command: draft.command.clone(),
                                            parameter: draft.parameter.clone(),
                                            comment: draft.comment.clone(),
                                        },
                                    ));
                                }
                            }
                            let exists = templates.iter().any(|t| t.name == draft.template.trim());
                            if ui
                                .add_enabled(exists, egui::Button::new(tr("Excluir")))
                                .clicked()
                            {
                                actions.push(CommandDialogAction::DeleteTemplate(
                                    draft.template.trim().to_owned(),
                                ));
                                draft.template.clear();
                            }
                        });
                        ui.end_row();

                        ui.label(tr("Comando"));
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut draft.command).desired_width(180.0),
                            );
                            egui::ComboBox::from_id_salt("command_suggested")
                                .selected_text("▾")
                                .width(24.0)
                                .show_ui(ui, |ui| {
                                    for name in SUGGESTED {
                                        if ui
                                            .selectable_label(draft.command == name, name)
                                            .clicked()
                                        {
                                            draft.command = name.to_owned();
                                        }
                                    }
                                });
                        });
                        ui.end_row();

                        ui.label(tr("Parâmetro"));
                        ui.add(
                            egui::TextEdit::singleline(&mut draft.parameter).desired_width(260.0),
                        );
                        ui.end_row();

                        ui.label(tr("Comentário"));
                        ui.add(egui::TextEdit::singleline(&mut draft.comment).desired_width(260.0));
                        ui.end_row();

                        ui.label(tr("Posição"));
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut draft.position)
                                    .desired_width(110.0)
                                    .font(egui::TextStyle::Monospace),
                            );
                            if parse_start(&draft.position, draft.fps).is_none() {
                                ui.colored_label(
                                    ui.visuals().error_fg_color,
                                    tr("Timecode inválido (hh:mm:ss:qq)"),
                                );
                            }
                        });
                        ui.end_row();
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    let time = parse_start(&draft.position, draft.fps);
                    let ok = time.is_some() && !draft.command.trim().is_empty();
                    if ui
                        .add_enabled(ok, egui::Button::new(tr("Aplicar")))
                        .clicked()
                    {
                        if let Some(time) = time {
                            actions.push(CommandDialogAction::Apply {
                                id: draft.id,
                                time,
                                command: draft.command.clone(),
                                parameter: draft.parameter.clone(),
                                comment: draft.comment.clone(),
                            });
                            done = true;
                        }
                    }
                    if ui.button(tr("Cancelar")).clicked() {
                        done = true;
                    }
                });
            });
        if open && !done {
            self.draft = Some(draft);
        }
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dialog_opens_on_a_new_marker_at_the_cursor_or_on_an_existing_one() {
        let mut dialog = CommandDialog::default();
        assert!(!dialog.is_open());
        dialog.open_new(2.0, 25.0, false);
        let draft = dialog.draft.as_ref().unwrap();
        assert_eq!(draft.id, None);
        assert_eq!(draft.position, "00:00:02:00");
        assert_eq!(draft.command, SUGGESTED[0]);
        let marker = CommandMarker {
            id: 7,
            time: 61.0,
            command: "Text".into(),
            parameter: "Olá".into(),
            comment: "c".into(),
        };
        dialog.open_edit(&marker, 25.0, false);
        let draft = dialog.draft.as_ref().unwrap();
        assert_eq!(draft.id, Some(7));
        assert_eq!(draft.position, "00:01:01:00");
        assert_eq!(
            (draft.command.as_str(), draft.parameter.as_str()),
            ("Text", "Olá")
        );
        // The position that is typed reads back as the time of the marker
        assert_eq!(parse_start(&draft.position, draft.fps), Some(61.0));
    }
}
