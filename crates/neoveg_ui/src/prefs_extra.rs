//! The extra preference keys in the preferences window (quick fade, thumbnails, safe areas...).

use eframe::egui;
use neoveg_common::i18n::tr;
use neoveg_common::preferences::Preferences;

/// The label of a `thumbnails_mode` value.
pub fn thumbnail_mode_label(mode: &str) -> &'static str {
    match mode {
        "first" => "Só o primeiro quadro",
        "none" => "Sem miniaturas",
        _ => "Todas as miniaturas",
    }
}

/// Draws the controls of the extra keys inside the General tab.
pub fn extra_keys_ui(ui: &mut egui::Ui, prefs: &mut Preferences) {
    ui.separator();
    ui.strong(tr("Edição e exibição"));
    ui.horizontal(|ui| {
        ui.label(tr("Fade rápido (milissegundos)"));
        ui.add(egui::DragValue::new(&mut prefs.quick_fade_ms).range(0.0..=500.0));
    });
    ui.checkbox(
        &mut prefs.cut_to_overlap,
        tr("Cortar até a sobreposição (o evento de baixo é aparado, não coberto)"),
    );
    ui.checkbox(
        &mut prefs.envelope_fade,
        tr("Pontos novos de envelope usam curva de fade"),
    );
    ui.checkbox(
        &mut prefs.preserve_pitch_on_stretch,
        tr("Manter o tom ao esticar eventos no tempo"),
    );
    ui.checkbox(
        &mut prefs.create_fx_undos,
        tr("Cada mudança de parâmetro de efeito é um passo de desfazer"),
    );
    let current = prefs.thumbnails();
    egui::ComboBox::from_label(tr("Miniaturas nos eventos"))
        .selected_text(tr(thumbnail_mode_label(current)))
        .show_ui(ui, |ui| {
            for mode in neoveg_common::preferences::THUMBNAIL_MODES {
                if ui
                    .selectable_label(current == mode, tr(thumbnail_mode_label(mode)))
                    .clicked()
                {
                    prefs.thumbnails_mode = mode.to_string();
                }
            }
        });
    ui.horizontal(|ui| {
        ui.label(tr("Suavidade padrão de keyframes"));
        ui.add(egui::DragValue::new(&mut prefs.keyframe_smoothness).range(-1.0..=1.0).speed(0.05));
    });
    ui.checkbox(
        &mut prefs.show_safe_areas,
        tr("Mostrar áreas seguras sobre a prévia"),
    );
    ui.horizontal(|ui| {
        ui.label(tr("Área segura de ação / de título (%)"));
        ui.add(egui::DragValue::new(&mut prefs.safe_area_action_percent).range(50.0..=100.0));
        ui.add(egui::DragValue::new(&mut prefs.safe_area_title_percent).range(50.0..=100.0));
    });
    ui.horizontal(|ui| {
        ui.label(tr("Divisões da grade sobre a prévia"));
        ui.add(egui::DragValue::new(&mut prefs.grid_divisions).range(1..=16));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mode_has_its_own_label() {
        let labels: Vec<_> = neoveg_common::preferences::THUMBNAIL_MODES
            .iter()
            .map(|m| thumbnail_mode_label(m))
            .collect();
        assert_eq!(labels.len(), 3);
        assert!(labels[0] != labels[1] && labels[1] != labels[2] && labels[0] != labels[2]);
    }
}
