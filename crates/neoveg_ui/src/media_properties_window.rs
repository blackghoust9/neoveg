//! Media Properties: what the user says about a media file that the file does not say itself (manual
//! v21 p.172-177): a start timecode of the file's own or a custom one, a substitute frame rate, the
//! field order, the pixel aspect ratio, how to read the alpha channel and a background colour.
//!
//! The dialog edits a draft and hands the finished [`MediaProps`] back as an action; the edit
//! itself (one undo step) is `neoveg_engine::media_props::set`.

use eframe::egui;
use neoveg_common::i18n::tr;
use neoveg_common::media_props::{
    AlphaMode, FieldOrder, MediaProps, PIXEL_ASPECTS, PIXEL_ASPECT_RANGE, SUBSTITUTE_FPS,
};
use neoveg_common::timecode::{parse_timecode, seconds_to_timecode};

/// The user pressed *Aplicar*: these are the properties of this media now.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaPropsAction {
    pub media_id: u64,
    pub props: MediaProps,
}

/// What the dialog is showing.
struct Target {
    media_id: u64,
    name: String,
    path: String,
    /// Size, frame rate and length of the file, as one line.
    summary: String,
    /// Picture settings (field order, pixel aspect, alpha, background) only mean something for video.
    is_video: bool,
    /// Frame rate the timecode is counted at.
    timecode_fps: f64,
    drop_frame: bool,
    draft: MediaProps,
    /// The custom start timecode as typed.
    timecode_text: String,
}

#[derive(Default)]
pub struct MediaPropsDialog {
    target: Option<Target>,
    /// A pulldown conversion the user asked for: the media and the direction.
    pulldown_request: Option<(u64, neoveg_video::pulldown_job::PulldownMode)>,
}

/// The start timecode `text` (`hh:mm:ss:ff`) in seconds, counted at `fps`; `None` when it is not one.
pub fn parse_start(text: &str, fps: f64) -> Option<f64> {
    let (frame, _) = parse_timecode(text, fps)?;
    (fps > 0.0).then(|| frame as f64 / fps)
}

impl MediaPropsDialog {
    #[cfg(test)]
    pub fn is_open(&self) -> bool {
        self.target.is_some()
    }

    /// Opens the dialog on a media file with the properties it has now.
    #[allow(clippy::too_many_arguments)]
    pub fn open(
        &mut self,
        media_id: u64,
        name: String,
        path: String,
        summary: String,
        is_video: bool,
        timecode_fps: f64,
        drop_frame: bool,
        props: MediaProps,
    ) {
        let timecode_text = seconds_to_timecode(
            props.timecode_start.unwrap_or(0.0),
            timecode_fps,
            drop_frame,
        );
        self.target = Some(Target {
            media_id,
            name,
            path,
            summary,
            is_video,
            timecode_fps,
            drop_frame,
            draft: props,
            timecode_text,
        });
    }

    /// The pulldown conversion asked for since the last call, if any.
    pub fn take_pulldown_request(
        &mut self,
    ) -> Option<(u64, neoveg_video::pulldown_job::PulldownMode)> {
        self.pulldown_request.take()
    }

    /// Draws the dialog; the action is returned the moment *Aplicar* is pressed.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<MediaPropsAction> {
        let mut target = self.target.take()?;
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        crate::dialogs::dialog(ctx, tr("Propriedades da mídia"))
            .collapsible(false)
            .open(&mut open)
            .show(ctx, |ui| {
                apply = body(ui, &mut target);
                if target.is_video {
                    ui.separator();
                    ui.horizontal(|ui| {
                        use neoveg_video::pulldown_job::PulldownMode;
                        ui.label(tr("Pulldown 3:2"));
                        if ui
                            .button(tr("Remover pulldown…"))
                            .on_hover_text(tr(
                                "Reconstrói os quadros de filme (29,97 para 23,976 fps) num arquivo novo",
                            ))
                            .clicked()
                        {
                            self.pulldown_request = Some((target.media_id, PulldownMode::Remove));
                        }
                        if ui
                            .button(tr("Aplicar pulldown…"))
                            .on_hover_text(tr(
                                "Leva material de filme (23,976 para 29,97 fps) ao ritmo 2:3 num arquivo novo",
                            ))
                            .clicked()
                        {
                            self.pulldown_request = Some((target.media_id, PulldownMode::Apply));
                        }
                    });
                }
                ui.separator();
                ui.horizontal(|ui| {
                    let valid = target.draft.timecode_start.is_none()
                        || parse_start(&target.timecode_text, target.timecode_fps).is_some();
                    if ui
                        .add_enabled(valid, egui::Button::new(tr("Aplicar")))
                        .clicked()
                    {
                        apply = true;
                    }
                    if ui.button(tr("Cancelar")).clicked() {
                        cancel = true;
                    }
                    if ui.button(tr("Restaurar padrão")).clicked() {
                        target.draft = MediaProps::default();
                        target.timecode_text =
                            seconds_to_timecode(0.0, target.timecode_fps, target.drop_frame);
                    }
                });
            });
        // A custom timecode that does not read is not applied (Enter included)
        let valid = target.draft.timecode_start.is_none()
            || parse_start(&target.timecode_text, target.timecode_fps).is_some();
        if apply && valid {
            let mut props = target.draft.clone();
            if props.timecode_start.is_some() {
                props.timecode_start = parse_start(&target.timecode_text, target.timecode_fps);
            }
            return Some(MediaPropsAction {
                media_id: target.media_id,
                props,
            });
        }
        if open && !cancel {
            self.target = Some(target);
        }
        None
    }
}

fn field_order_label(order: FieldOrder) -> &'static str {
    match order {
        FieldOrder::Progressive => tr("Nenhuma (varredura progressiva)"),
        FieldOrder::UpperFirst => tr("Campo superior primeiro"),
        FieldOrder::LowerFirst => tr("Campo inferior primeiro"),
    }
}

fn alpha_label(mode: AlphaMode) -> &'static str {
    match mode {
        AlphaMode::Auto => tr("Detectado no arquivo"),
        AlphaMode::Undefined => tr("Indefinido (ignorar o alfa)"),
        AlphaMode::None => tr("Nenhum (opaco)"),
        AlphaMode::Straight => tr("Direto (sem pré-multiplicação)"),
        AlphaMode::Premultiplied => tr("Pré-multiplicado"),
        AlphaMode::PremultipliedDirty => tr("Pré-multiplicado (sujo)"),
    }
}

/// The name of a pixel aspect ratio: the preset it matches, or its number.
pub fn pixel_aspect_label(value: f64) -> String {
    if (value - 1.0).abs() < 1e-4 {
        return tr("1,0000 (quadrado)").to_owned();
    }
    PIXEL_ASPECTS
        .iter()
        .find(|(v, _)| (v - value).abs() < 1e-4)
        .map_or_else(
            || format!("{value:.4}"),
            |(v, name)| format!("{v:.4} ({name})"),
        )
}

/// The fields of the dialog; `true` when Enter asked to apply.
fn body(ui: &mut egui::Ui, t: &mut Target) -> bool {
    ui.label(egui::RichText::new(&t.name).strong());
    ui.weak(&t.path);
    if !t.summary.is_empty() {
        ui.weak(&t.summary);
    }
    ui.separator();
    let mut enter = false;
    egui::Grid::new("media_props_grid")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            // Timecode
            ui.label(tr("Timecode"));
            ui.vertical(|ui| {
                let custom = t.draft.timecode_start.is_some();
                if ui
                    .radio(!custom, tr("Usar o timecode do arquivo"))
                    .clicked()
                {
                    t.draft.timecode_start = None;
                }
                ui.horizontal(|ui| {
                    if ui
                        .radio(custom, tr("Usar timecode personalizado"))
                        .clicked()
                        && !custom
                    {
                        t.draft.timecode_start =
                            Some(parse_start(&t.timecode_text, t.timecode_fps).unwrap_or(0.0));
                    }
                    let response = ui.add_enabled(
                        custom,
                        egui::TextEdit::singleline(&mut t.timecode_text)
                            .desired_width(110.0)
                            .font(egui::TextStyle::Monospace),
                    );
                    enter |= response.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter));
                });
                if custom && parse_start(&t.timecode_text, t.timecode_fps).is_none() {
                    ui.colored_label(
                        ui.visuals().error_fg_color,
                        tr("Timecode inválido (hh:mm:ss:qq)"),
                    );
                }
            });
            ui.end_row();

            if t.is_video {
                // Substitute frame rate
                ui.label(tr("Taxa de quadros substituta"));
                ui.horizontal(|ui| {
                    let mut on = t.draft.substitute_fps.is_some();
                    if ui.checkbox(&mut on, tr("Reproduzir a")).changed() {
                        t.draft.substitute_fps = on.then(|| {
                            SUBSTITUTE_FPS
                                .iter()
                                .copied()
                                .find(|f| *f > 0.0)
                                .unwrap_or(24.0)
                        });
                    }
                    if let Some(fps) = &mut t.draft.substitute_fps {
                        ui.add(
                            egui::DragValue::new(fps)
                                .range(1.0..=240.0)
                                .speed(0.01)
                                .max_decimals(3)
                                .suffix(" fps"),
                        );
                        egui::ComboBox::from_id_salt("media_props_fps")
                            .selected_text(tr("Escolher…"))
                            .show_ui(ui, |ui| {
                                for preset in SUBSTITUTE_FPS {
                                    if ui.selectable_label(false, format!("{preset}")).clicked() {
                                        *fps = preset;
                                    }
                                }
                            });
                    }
                });
                ui.end_row();

                // Field order
                ui.label(tr("Ordem dos campos"));
                egui::ComboBox::from_id_salt("media_props_fields")
                    .selected_text(field_order_label(t.draft.field_order))
                    .show_ui(ui, |ui| {
                        for order in FieldOrder::ALL {
                            ui.selectable_value(
                                &mut t.draft.field_order,
                                order,
                                field_order_label(order),
                            );
                        }
                    })
                    .response
                    .on_hover_text(tr(
                        "A ordem dos campos fica guardada no projeto; a imagem ainda não é desentrelaçada",
                    ));
                ui.end_row();

                // Pixel aspect ratio
                ui.label(tr("Proporção do pixel"));
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("media_props_par")
                        .selected_text(pixel_aspect_label(t.draft.pixel_aspect))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut t.draft.pixel_aspect,
                                1.0,
                                pixel_aspect_label(1.0),
                            );
                            for (value, _) in PIXEL_ASPECTS {
                                ui.selectable_value(
                                    &mut t.draft.pixel_aspect,
                                    value,
                                    pixel_aspect_label(value),
                                );
                            }
                        });
                    ui.add(
                        egui::DragValue::new(&mut t.draft.pixel_aspect)
                            .range(PIXEL_ASPECT_RANGE.0..=PIXEL_ASPECT_RANGE.1)
                            .speed(0.001)
                            .max_decimals(4),
                    );
                });
                ui.end_row();

                // Alpha channel
                ui.label(tr("Canal alfa"));
                egui::ComboBox::from_id_salt("media_props_alpha")
                    .selected_text(alpha_label(t.draft.alpha))
                    .show_ui(ui, |ui| {
                        for mode in AlphaMode::ALL {
                            ui.selectable_value(&mut t.draft.alpha, mode, alpha_label(mode));
                        }
                    });
                ui.end_row();

                // Background colour
                ui.label(tr("Cor de fundo"));
                ui.horizontal(|ui| {
                    let mut on = t.draft.background.is_some();
                    if ui
                        .checkbox(&mut on, tr("Preencher as partes transparentes"))
                        .changed()
                    {
                        t.draft.background = on.then_some([0, 0, 0]);
                    }
                    if let Some(color) = &mut t.draft.background {
                        ui.color_edit_button_srgb(color);
                    }
                });
                ui.end_row();
            }
        });
    enter
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_typed_start_timecode_becomes_seconds_and_a_wrong_one_is_refused() {
        assert_eq!(parse_start("00:00:10:00", 25.0), Some(10.0));
        assert_eq!(parse_start("01:00:00:00", 25.0), Some(3600.0));
        assert_eq!(parse_start("00:00:01:12", 24.0), Some(1.5));
        for bad in ["", "10", "aa:bb:cc:dd", "00:61:00:00", "1:2:3"] {
            assert_eq!(parse_start(bad, 25.0), None, "{bad:?}");
        }
        assert_eq!(parse_start("00:00:01:00", 0.0), None, "no rate, no seconds");
    }

    #[test]
    fn pixel_aspects_are_named_by_their_preset_or_their_number() {
        assert_eq!(pixel_aspect_label(1.0), tr("1,0000 (quadrado)"));
        assert_eq!(pixel_aspect_label(0.9091), "0.9091 (NTSC DV)");
        assert_eq!(pixel_aspect_label(1.4587), "1.4587 (PAL DV 16:9)");
        assert_eq!(pixel_aspect_label(1.5), "1.5000");
    }

    #[test]
    fn a_pulldown_request_is_handed_over_once() {
        use neoveg_video::pulldown_job::PulldownMode;
        let mut dialog = MediaPropsDialog::default();
        assert_eq!(dialog.take_pulldown_request(), None);
        dialog.pulldown_request = Some((3, PulldownMode::Remove));
        assert_eq!(
            dialog.take_pulldown_request(),
            Some((3, PulldownMode::Remove))
        );
        assert_eq!(dialog.take_pulldown_request(), None);
    }

    #[test]
    fn the_dialog_opens_on_the_properties_the_media_has() {
        let mut dialog = MediaPropsDialog::default();
        assert!(!dialog.is_open());
        let props = MediaProps {
            timecode_start: Some(3600.0),
            ..MediaProps::default()
        };
        dialog.open(
            4,
            "clip.mov".into(),
            "C:\\media\\clip.mov".into(),
            "1920x1080".into(),
            true,
            25.0,
            false,
            props,
        );
        assert!(dialog.is_open());
        let target = dialog.target.as_ref().unwrap();
        assert_eq!(target.media_id, 4);
        assert_eq!(target.timecode_text, "01:00:00:00");
        assert_eq!(target.draft.timecode_start, Some(3600.0));
    }
}
