//! Exibir > Detalhes de edição: every event of the project in one table.
//!
//! A row shows the track, the name, the start, the length, the end, where the event begins in its
//! source, its speed and the switches that are on. A click selects the event and moves the cursor to
//! its start; *Copiar tabela* puts the whole table on the clipboard as tab-separated text.

use eframe::egui::{self, RichText};
use neoveg_common::timecode::seconds_to_timecode;
use neoveg_engine::details::DetailRow;
use neoveg_engine::selection::EventRef;

#[derive(Debug, Clone, PartialEq)]
pub enum DetailsAction {
    /// Select the event and move the cursor to its start.
    Go(EventRef, f64),
    /// Copy the table as tab-separated text.
    Copy,
}

/// Draws the window; returns what the user did.
pub fn show(
    ctx: &egui::Context,
    open: &mut bool,
    rows: &[DetailRow],
    selected: &[EventRef],
    fps: f64,
    drop_frame: bool,
) -> Vec<DetailsAction> {
    let mut actions = Vec::new();
    if !*open {
        return actions;
    }
    crate::dialogs::tool_window(ctx, "Detalhes de edição")
        .open(open)
        .default_width(760.0)
        .default_height(380.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("{} eventos", rows.len()));
                if ui.button("Copiar tabela").clicked() {
                    actions.push(DetailsAction::Copy);
                }
            });
            ui.separator();
            egui::ScrollArea::both().show(ui, |ui| {
                egui::Grid::new("edit_details")
                    .striped(true)
                    .num_columns(8)
                    .show(ui, |ui| {
                        for heading in [
                            "N",
                            "Faixa",
                            "Nome",
                            "Início",
                            "Duração",
                            "Fim",
                            "Fonte",
                            "Opções",
                        ] {
                            ui.label(RichText::new(heading).strong());
                        }
                        ui.end_row();
                        let tc = |t: f64| seconds_to_timecode(t, fps, drop_frame);
                        for (i, row) in rows.iter().enumerate() {
                            let on = selected.contains(&row.event);
                            ui.label((i + 1).to_string());
                            ui.label(format!("{} {}", row.track_number, row.track_name));
                            if ui.selectable_label(on, &row.name).clicked() {
                                actions.push(DetailsAction::Go(row.event, row.start));
                            }
                            ui.label(tc(row.start));
                            ui.label(tc(row.duration));
                            ui.label(tc(row.end));
                            ui.label(tc(row.trim_in));
                            ui.label(&row.flags);
                            ui.end_row();
                        }
                    });
            });
            ui.weak("Opções: M mudo, B bloqueado, L repetir, N normalizar, F fase invertida.");
        });
    actions
}
