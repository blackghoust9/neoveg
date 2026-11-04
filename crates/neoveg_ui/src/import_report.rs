//! The report of an import of an exchange file (XML, FCPXML, OpenTimelineIO, AAF, EDL): which clips
//! could not be placed and why, and what the reader had to leave out. It opens by itself when
//! something was left out, and its text can be copied.

use eframe::egui;
use neoveg_common::i18n::{tr, tr_args};

/// What an import did and did not do.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImportReportView {
    /// The kind of file, for the title ("AAF", "EDL"...).
    pub format: String,
    /// Clips or events placed on the timeline.
    pub added: usize,
    /// The clips that were not placed, each with the reason.
    pub skipped: Vec<String>,
    /// What the reader left out of the file or approximated (effects, speeds...).
    pub warnings: Vec<String>,
}

impl ImportReportView {
    /// `true` when nothing was left out: no window is needed.
    pub fn is_clean(&self) -> bool {
        self.skipped.is_empty() && self.warnings.is_empty()
    }

    /// The report as plain text, for the clipboard.
    pub fn text(&self) -> String {
        let mut out = format!(
            "{}: {} / {}\n",
            self.format,
            tr_args("{} colocado(s)", &[&self.added]),
            tr_args("{} sem mídia no projeto", &[&self.skipped.len()]),
        );
        if !self.skipped.is_empty() {
            out.push_str(&format!("\n{}\n", tr("Não colocados")));
            for line in &self.skipped {
                out.push_str(&format!("  - {line}\n"));
            }
        }
        if !self.warnings.is_empty() {
            out.push_str(&format!("\n{}\n", tr("Avisos da leitura")));
            for line in &self.warnings {
                out.push_str(&format!("  - {line}\n"));
            }
        }
        out
    }
}

/// Draws the window while `report` holds one; closing it (or the button) empties it.
pub fn show(ctx: &egui::Context, report: &mut Option<ImportReportView>) {
    let Some(current) = report.as_ref() else {
        return;
    };
    let mut open = true;
    let mut close = false;
    crate::dialogs::dialog(
        ctx,
        &tr_args("Relatório da importação ({})", &[&current.format]),
    )
    .open(&mut open)
    .default_width(520.0)
    .default_height(360.0)
    .show(ctx, |ui| {
        ui.label(tr_args(
            "{} colocado(s); {} não colocado(s); {} aviso(s)",
            &[
                &current.added,
                &current.skipped.len(),
                &current.warnings.len(),
            ],
        ));
        ui.separator();
        egui::ScrollArea::vertical()
            .max_height(260.0)
            .show(ui, |ui| {
                if !current.skipped.is_empty() {
                    egui::CollapsingHeader::new(tr_args(
                        "Não colocados ({})",
                        &[&current.skipped.len()],
                    ))
                    .default_open(true)
                    .show(ui, |ui| {
                        for line in &current.skipped {
                            ui.label(line);
                        }
                    });
                    ui.weak(tr(
                        "Importe a mídia que falta para o projeto e importe o arquivo de novo.",
                    ));
                }
                if !current.warnings.is_empty() {
                    egui::CollapsingHeader::new(tr_args(
                        "Avisos da leitura ({})",
                        &[&current.warnings.len()],
                    ))
                    .default_open(true)
                    .show(ui, |ui| {
                        for line in &current.warnings {
                            ui.label(line);
                        }
                    });
                }
            });
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button(tr("Copiar relatório")).clicked() {
                ui.ctx().copy_text(current.text());
            }
            if ui.button(tr("Fechar")).clicked() {
                close = true;
            }
        });
    });
    if !open || close {
        *report = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_with_nothing_left_out_is_clean_and_the_text_lists_what_was() {
        let mut report = ImportReportView {
            format: "EDL".into(),
            added: 3,
            ..ImportReportView::default()
        };
        assert!(report.is_clean());
        report
            .skipped
            .push("\"cena.mov\": mídia não encontrada no projeto".into());
        report.warnings.push("transição desconhecida".into());
        assert!(!report.is_clean());
        let text = report.text();
        assert!(text.starts_with("EDL: "), "{text}");
        assert!(text.contains("cena.mov"), "{text}");
        assert!(text.contains("transição desconhecida"), "{text}");
    }
}
