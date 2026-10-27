//! Exibir > Notas do projeto: notes pinned to moments of the timeline.
//!
//! As the reference manuals describe them, notes let an editor leave reminders in a project or
//! comments for a collaborator at exact points. Each one has a timecode, a colour and a label of its
//! own, new notes start from a default colour and label, the timecode can be typed or taken from the
//! cursor, a button jumps the cursor to it, and a note is marked resolved when it has been dealt
//! with (resolved notes can be hidden).
//!
//! The window only reports [`NoteAction`]s; [`apply_note_action`] is the whole of the logic, so it
//! is tested without a window.

use std::collections::HashMap;

use eframe::egui::{self, Color32, RichText};
use neoveg_common::project::{next_note_id, notes_in_order, ProjectNote, NOTE_COLORS};
use neoveg_common::timecode::seconds_to_timecode;
use neoveg_engine::navigation;

#[derive(Debug, Clone, PartialEq)]
pub enum NoteAction {
    /// A new note at the cursor with the default colour and label.
    Add {
        color: u8,
        label: String,
    },
    /// Move the cursor to the note.
    Jump(u64),
    /// The note's time is the cursor's now.
    UpdateToCursor(u64),
    SetTime {
        id: u64,
        time: f64,
    },
    SetText {
        id: u64,
        text: String,
    },
    SetLabel {
        id: u64,
        label: String,
    },
    SetColor {
        id: u64,
        color: u8,
    },
    ToggleResolved(u64),
    Delete(u64),
}

#[derive(Debug, Clone, Default)]
pub struct NotesState {
    pub open: bool,
    pub default_color: u8,
    pub default_label: String,
    pub hide_resolved: bool,
    /// The timecode text being typed for a note, until Enter.
    time_edits: HashMap<u64, String>,
}

/// What [`apply_note_action`] changed in the application besides the notes.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct NoteEffect {
    /// Put the cursor here.
    pub jump_to: Option<f64>,
    /// The notes changed (the project is no longer the saved one).
    pub changed: bool,
}

/// Carries out one action on the notes. `playhead` is the cursor.
pub fn apply_note_action(
    notes: &mut Vec<ProjectNote>,
    action: NoteAction,
    playhead: f64,
) -> NoteEffect {
    let mut effect = NoteEffect::default();
    let find = |notes: &mut Vec<ProjectNote>, id: u64| notes.iter().position(|n| n.id == id);
    match action {
        NoteAction::Add { color, label } => {
            let id = next_note_id(notes);
            notes.push(ProjectNote {
                id,
                time: playhead.max(0.0),
                text: String::new(),
                label,
                color: color.min(NOTE_COLORS.len() as u8 - 1),
                resolved: false,
            });
            effect.changed = true;
        }
        NoteAction::Jump(id) => {
            effect.jump_to = find(notes, id).map(|i| notes[i].time);
        }
        NoteAction::UpdateToCursor(id) => {
            if let Some(i) = find(notes, id) {
                notes[i].time = playhead.max(0.0);
                effect.changed = true;
            }
        }
        NoteAction::SetTime { id, time } => {
            if let (Some(i), true) = (find(notes, id), time.is_finite()) {
                notes[i].time = time.max(0.0);
                effect.changed = true;
            }
        }
        NoteAction::SetText { id, text } => {
            if let Some(i) = find(notes, id) {
                if notes[i].text != text {
                    notes[i].text = text;
                    effect.changed = true;
                }
            }
        }
        NoteAction::SetLabel { id, label } => {
            if let Some(i) = find(notes, id) {
                if notes[i].label != label {
                    notes[i].label = label;
                    effect.changed = true;
                }
            }
        }
        NoteAction::SetColor { id, color } => {
            if let Some(i) = find(notes, id) {
                notes[i].color = color.min(NOTE_COLORS.len() as u8 - 1);
                effect.changed = true;
            }
        }
        NoteAction::ToggleResolved(id) => {
            if let Some(i) = find(notes, id) {
                notes[i].resolved = !notes[i].resolved;
                effect.changed = true;
            }
        }
        NoteAction::Delete(id) => {
            let before = notes.len();
            notes.retain(|n| n.id != id);
            effect.changed = notes.len() != before;
        }
    }
    effect
}

fn color32(note_color: [u8; 3]) -> Color32 {
    Color32::from_rgb(note_color[0], note_color[1], note_color[2])
}

impl NotesState {
    /// Draws the window and returns what the user did.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        notes: &[ProjectNote],
        fps: f64,
        drop_frame: bool,
    ) -> Vec<NoteAction> {
        let mut actions = Vec::new();
        if !self.open {
            return actions;
        }
        let mut open = self.open;
        egui::Window::new("Notas do projeto")
            .open(&mut open)
            .default_width(520.0)
            .default_height(420.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Cor padrão:");
                    for (i, (name, rgb)) in NOTE_COLORS.iter().enumerate() {
                        let selected = usize::from(self.default_color) == i;
                        let button =
                            egui::Button::new("  ")
                                .fill(color32(*rgb))
                                .stroke(if selected {
                                    egui::Stroke::new(2.0_f32, ui.visuals().text_color())
                                } else {
                                    egui::Stroke::NONE
                                });
                        if ui.add(button).on_hover_text(*name).clicked() {
                            self.default_color = i as u8;
                        }
                    }
                    ui.label("Etiqueta padrão:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.default_label).desired_width(120.0),
                    );
                    if ui.button("Adicionar nota").clicked() {
                        actions.push(NoteAction::Add {
                            color: self.default_color,
                            label: self.default_label.clone(),
                        });
                    }
                });
                ui.checkbox(&mut self.hide_resolved, "Ocultar notas resolvidas");
                ui.separator();
                let list = notes_in_order(notes, self.hide_resolved);
                if list.is_empty() {
                    ui.weak("Nenhuma nota. Posicione o cursor e use Adicionar nota.");
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for note in list {
                        self.note_row(ui, note, fps, drop_frame, &mut actions);
                        ui.separator();
                    }
                });
            });
        self.open = open;
        actions
    }

    fn note_row(
        &mut self,
        ui: &mut egui::Ui,
        note: &ProjectNote,
        fps: f64,
        drop_frame: bool,
        actions: &mut Vec<NoteAction>,
    ) {
        let id = note.id;
        ui.horizontal(|ui| {
            // Colour: a swatch that cycles through the palette
            let swatch = egui::Button::new("  ").fill(color32(note.rgb()));
            if ui.add(swatch).on_hover_text("Mudar a cor").clicked() {
                actions.push(NoteAction::SetColor {
                    id,
                    color: (note.color + 1) % NOTE_COLORS.len() as u8,
                });
            }
            // Timecode: typed and confirmed with Enter
            let shown = seconds_to_timecode(note.time, fps, drop_frame);
            let text = self.time_edits.entry(id).or_insert_with(|| shown.clone());
            let edit = ui.add(egui::TextEdit::singleline(text).desired_width(96.0));
            if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if let Some(time) = navigation::parse_time(text, fps) {
                    actions.push(NoteAction::SetTime { id, time });
                }
                self.time_edits.remove(&id);
            } else if !edit.has_focus() && *text != shown {
                // Show the stored time again when the note moved elsewhere
                self.time_edits.remove(&id);
            }
            if ui
                .small_button("Ir")
                .on_hover_text("Mover o cursor para a nota")
                .clicked()
            {
                actions.push(NoteAction::Jump(id));
            }
            if ui
                .small_button("Cursor")
                .on_hover_text("Usar o tempo atual do cursor")
                .clicked()
            {
                actions.push(NoteAction::UpdateToCursor(id));
            }
            let label = if note.resolved { "Reabrir" } else { "Resolver" };
            if ui.small_button(label).clicked() {
                actions.push(NoteAction::ToggleResolved(id));
            }
            if ui
                .small_button("x")
                .on_hover_text("Excluir a nota")
                .clicked()
            {
                actions.push(NoteAction::Delete(id));
            }
        });
        ui.horizontal(|ui| {
            let mut label = note.label.clone();
            if ui
                .add(
                    egui::TextEdit::singleline(&mut label)
                        .hint_text("etiqueta")
                        .desired_width(110.0),
                )
                .changed()
            {
                actions.push(NoteAction::SetLabel { id, label });
            }
            let mut text = note.text.clone();
            let body = egui::TextEdit::multiline(&mut text)
                .hint_text("anotação")
                .desired_rows(2)
                .desired_width(f32::INFINITY);
            if ui.add(body).changed() {
                actions.push(NoteAction::SetText { id, text });
            }
        });
        if note.resolved {
            ui.label(RichText::new("resolvida").weak());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add(notes: &mut Vec<ProjectNote>, at: f64) -> u64 {
        apply_note_action(
            notes,
            NoteAction::Add {
                color: 2,
                label: "revisar".into(),
            },
            at,
        );
        notes.last().unwrap().id
    }

    #[test]
    fn a_new_note_takes_the_cursor_time_and_the_defaults_and_gets_its_own_id() {
        let mut notes = Vec::new();
        let a = add(&mut notes, 4.5);
        let b = add(&mut notes, 9.0);
        assert_ne!(a, b);
        assert_eq!(
            (notes[0].time, notes[0].color, notes[0].label.as_str()),
            (4.5, 2, "revisar")
        );
        assert!(!notes[0].resolved);
        // A colour outside the palette is brought back inside it
        apply_note_action(
            &mut notes,
            NoteAction::Add {
                color: 99,
                label: String::new(),
            },
            0.0,
        );
        assert_eq!(
            usize::from(notes.last().unwrap().color),
            NOTE_COLORS.len() - 1
        );
    }

    #[test]
    fn jump_reports_the_time_and_changes_nothing() {
        let mut notes = Vec::new();
        let id = add(&mut notes, 12.0);
        let effect = apply_note_action(&mut notes, NoteAction::Jump(id), 0.0);
        assert_eq!(
            effect,
            NoteEffect {
                jump_to: Some(12.0),
                changed: false
            }
        );
        let effect = apply_note_action(&mut notes, NoteAction::Jump(999), 0.0);
        assert_eq!(effect.jump_to, None);
    }

    #[test]
    fn the_time_can_be_taken_from_the_cursor_or_typed() {
        let mut notes = Vec::new();
        let id = add(&mut notes, 1.0);
        apply_note_action(&mut notes, NoteAction::UpdateToCursor(id), 30.0);
        assert_eq!(notes[0].time, 30.0);
        apply_note_action(&mut notes, NoteAction::SetTime { id, time: 7.25 }, 0.0);
        assert_eq!(notes[0].time, 7.25);
        // A time that is not a number, or is negative, is kept sane
        apply_note_action(&mut notes, NoteAction::SetTime { id, time: f64::NAN }, 0.0);
        assert_eq!(notes[0].time, 7.25);
        apply_note_action(&mut notes, NoteAction::SetTime { id, time: -3.0 }, 0.0);
        assert_eq!(notes[0].time, 0.0);
    }

    #[test]
    fn resolving_toggles_and_deleting_removes_only_that_note() {
        let mut notes = Vec::new();
        let a = add(&mut notes, 1.0);
        let b = add(&mut notes, 2.0);
        apply_note_action(&mut notes, NoteAction::ToggleResolved(a), 0.0);
        assert!(notes[0].resolved);
        apply_note_action(&mut notes, NoteAction::ToggleResolved(a), 0.0);
        assert!(!notes[0].resolved, "the button is a toggle");
        let effect = apply_note_action(&mut notes, NoteAction::Delete(a), 0.0);
        assert!(effect.changed);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].id, b);
        assert!(!apply_note_action(&mut notes, NoteAction::Delete(a), 0.0).changed);
    }

    #[test]
    fn editing_the_text_or_label_to_the_same_value_is_not_a_change() {
        let mut notes = Vec::new();
        let id = add(&mut notes, 1.0);
        let same = apply_note_action(
            &mut notes,
            NoteAction::SetText {
                id,
                text: String::new(),
            },
            0.0,
        );
        assert!(!same.changed);
        let changed = apply_note_action(
            &mut notes,
            NoteAction::SetText {
                id,
                text: "cortar aqui".into(),
            },
            0.0,
        );
        assert!(changed.changed && notes[0].text == "cortar aqui");
    }
}
