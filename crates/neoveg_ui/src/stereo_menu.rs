//! Exibir > 3D estereoscópico: how the preview shows a frame that carries a left/right pair.

use eframe::egui;
use neoveg_common::i18n::tr;
use neoveg_video::stereo3d::{StereoLayout, StereoOutput};

/// The submenu with the pair layout of the source and the way the preview shows it.
pub fn stereo_menu(ui: &mut egui::Ui, layout: &mut StereoLayout, output: &mut StereoOutput) {
    ui.menu_button(tr("3D estereoscópico"), |ui| {
        ui.label(tr("Formato da fonte"));
        for l in StereoLayout::ALL {
            ui.selectable_value(layout, l, tr(l.label()));
        }
        ui.separator();
        ui.label(tr("Exibição na prévia"));
        for o in StereoOutput::ALL {
            ui.selectable_value(output, o, tr(o.label()));
        }
    });
}
