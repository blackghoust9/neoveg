//! Exibir > modos de visualização da prévia: desentrelaçar, girar e espelhar.

use eframe::egui;
use neoveg_common::i18n::tr;
use neoveg_common::presets::Deinterlace;
use neoveg_video::view_mode::{Rotation, ViewMode};

/// The controls over the preview. The deinterlace switch uses the mode and field order of the
/// project properties; it does nothing for a progressive project.
pub fn controls(ui: &mut egui::Ui, view: &mut ViewMode) {
    let mut deinterlace = view.deinterlace.is_some();
    if ui
        .checkbox(&mut deinterlace, tr("Desentrelaçar"))
        .on_hover_text(tr("Mostra a prévia sem entrelaçamento, com o modo das propriedades do projeto"))
        .changed()
    {
        // The mode itself comes from the project when the frame is shown
        view.deinterlace = deinterlace.then_some(Deinterlace::BlendFields);
    }
    egui::ComboBox::from_id_salt("preview_rotation")
        .selected_text(tr(view.rotation.label()))
        .show_ui(ui, |ui| {
            for rotation in Rotation::ALL {
                ui.selectable_value(&mut view.rotation, rotation, tr(rotation.label()));
            }
        });
    ui.checkbox(&mut view.flip_horizontal, tr("Espelhar na horizontal"));
    ui.checkbox(&mut view.flip_vertical, tr("Espelhar na vertical"));
}
