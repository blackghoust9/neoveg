//! File > Properties: the project's video and audio format.
//!
//! Follows the Project Properties dialog the reference manuals describe: a template list that fills
//! in the controls (with the user's own templates saved beside it and a button that copies the
//! settings of a media file), then the Video tab (frame size, field order, pixel aspect ratio, HDR
//! mode, output rotation, frame rate, pixel format, compositing gamma, render quality, motion
//! blur, deinterlacing, resampling) and the Audio tab (sample rate, bit depth, master bus).
//!
//! The window edits a draft and reports what the user asked for as [`ProjectAction`]s; the
//! application applies them. What the pipeline already honours (frame size, frame rate, sample
//! rate of the export) is told apart from what is only stored for now.

use eframe::egui::{self, Color32};
use neoveg_common::presets::{
    builtin_templates, clamp_size, display_aspect, frame_rate_of, resolution_named,
    CompositingGamma, Deinterlace, FieldOrder, HdrMode, MasterBusMode, MotionBlurType,
    OutputRotation, PixelFormat, ProjectTemplate, RenderQuality, ResampleMode, AUDIO_BIT_DEPTHS,
    FRAME_RATES, PIXEL_ASPECTS, RESOLUTIONS, SAMPLE_RATES, SIZE_RANGE,
};
use neoveg_common::project::ProjectSettings;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProjectTab {
    #[default]
    Video,
    Audio,
}

/// What the user asked for in the window.
#[derive(Debug, Clone, PartialEq)]
pub enum ProjectAction {
    /// OK or Apply: use these settings in the project. `start_all_new` carries the checkbox
    /// "start all new projects with these settings".
    Apply {
        settings: Box<ProjectSettings>,
        start_all_new: bool,
        close: bool,
    },
    /// Keep the draft as a template of the user's.
    SaveTemplate(ProjectTemplate),
    /// Choose a media file whose settings the draft should copy.
    MatchMedia,
}

#[derive(Debug, Clone, Default)]
pub struct ProjectDialogState {
    pub open: bool,
    pub tab: ProjectTab,
    pub draft: ProjectSettings,
    /// Name typed for "save template".
    pub template_name: String,
    pub start_all_new: bool,
    /// The template chosen last, to show in the list.
    pub chosen_template: Option<String>,
    pub note: String,
}

impl ProjectDialogState {
    /// Opens the window on a copy of the project's settings.
    pub fn open_with(&mut self, settings: &ProjectSettings, start_all_new: bool) {
        self.draft = settings.clone();
        self.start_all_new = start_all_new;
        self.chosen_template = None;
        self.note.clear();
        self.open = true;
    }

    /// Copies a template into the draft.
    pub fn use_template(&mut self, template: &ProjectTemplate) {
        // A template carries the picture format; the audio and the pipeline options stay as they are
        let keep = self.draft.clone();
        self.draft = ProjectSettings {
            width: template.settings.width,
            height: template.settings.height,
            fps: template.settings.fps,
            pixel_aspect: template.settings.pixel_aspect,
            field_order: template.settings.field_order,
            pixel_format: template.settings.pixel_format,
            compositing_gamma: template.settings.compositing_gamma,
            output_rotation: template.settings.output_rotation,
            hdr_mode: template.settings.hdr_mode,
            ..keep
        };
        self.chosen_template = Some(template.name.clone());
    }

    /// Copies a frame size and rate found in a media file into the draft.
    pub fn use_media(&mut self, width: u32, height: u32, fps: f64) {
        self.draft.width = clamp_size(width);
        self.draft.height = clamp_size(height);
        if fps.is_finite() && fps > 1.0 {
            self.draft.fps = fps.min(240.0);
        }
        self.chosen_template = None;
    }

    /// The draft with its values inside the accepted ranges.
    pub fn sanitized(&self) -> ProjectSettings {
        let mut s = self.draft.clone();
        s.width = clamp_size(s.width);
        s.height = clamp_size(s.height);
        s.fps = if s.fps.is_finite() {
            s.fps.clamp(1.0, 240.0)
        } else {
            29.97
        };
        s.pixel_aspect = if s.pixel_aspect.is_finite() {
            s.pixel_aspect.clamp(0.25, 4.0)
        } else {
            1.0
        };
        s.normalize_peak_db = if s.normalize_peak_db.is_finite() {
            s.normalize_peak_db.clamp(-60.0, 0.0)
        } else {
            0.0
        };
        s
    }

    /// Draws the window; returns what the user asked for.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        custom_templates: &[ProjectTemplate],
    ) -> Vec<ProjectAction> {
        let mut actions = Vec::new();
        if !self.open {
            return actions;
        }
        let mut open = self.open;
        let mut close = false;
        egui::Window::new("Propriedades do projeto")
            .open(&mut open)
            .collapsible(false)
            .default_width(520.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.tab, ProjectTab::Video, "Vídeo");
                    ui.selectable_value(&mut self.tab, ProjectTab::Audio, "Áudio");
                });
                ui.separator();
                match self.tab {
                    ProjectTab::Video => self.video_tab(ui, custom_templates, &mut actions),
                    ProjectTab::Audio => self.audio_tab(ui),
                }
                ui.separator();
                ui.checkbox(
                    &mut self.start_all_new,
                    "Iniciar todos os novos projetos com estas configurações",
                );
                if !self.note.is_empty() {
                    ui.colored_label(Color32::from_rgb(230, 160, 60), &self.note);
                }
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        actions.push(ProjectAction::Apply {
                            settings: Box::new(self.sanitized()),
                            start_all_new: self.start_all_new,
                            close: true,
                        });
                        close = true;
                    }
                    if ui.button("Aplicar").clicked() {
                        actions.push(ProjectAction::Apply {
                            settings: Box::new(self.sanitized()),
                            start_all_new: self.start_all_new,
                            close: false,
                        });
                    }
                    if ui.button("Cancelar").clicked() {
                        close = true;
                    }
                });
            });
        self.open = open && !close;
        actions
    }

    fn video_tab(
        &mut self,
        ui: &mut egui::Ui,
        custom_templates: &[ProjectTemplate],
        actions: &mut Vec<ProjectAction>,
    ) {
        let builtin = builtin_templates();
        ui.horizontal(|ui| {
            ui.label("Modelo:");
            let selected = self
                .chosen_template
                .clone()
                .unwrap_or_else(|| "(personalizado)".into());
            egui::ComboBox::from_id_salt("project_template")
                .selected_text(selected)
                .width(300.0)
                .show_ui(ui, |ui| {
                    for template in &builtin {
                        if ui
                            .selectable_label(
                                self.chosen_template.as_deref() == Some(template.name.as_str()),
                                &template.name,
                            )
                            .clicked()
                        {
                            self.use_template(template);
                        }
                    }
                    if !custom_templates.is_empty() {
                        ui.separator();
                        ui.weak("Seus modelos");
                        for template in custom_templates {
                            if ui.selectable_label(false, &template.name).clicked() {
                                self.use_template(template);
                            }
                        }
                    }
                });
            if ui.button("Combinar com mídia…").clicked() {
                actions.push(ProjectAction::MatchMedia);
            }
        });
        ui.horizontal(|ui| {
            ui.label("Salvar como modelo:");
            ui.add(egui::TextEdit::singleline(&mut self.template_name).desired_width(180.0));
            let name = self.template_name.trim().to_owned();
            if ui
                .add_enabled(!name.is_empty(), egui::Button::new("Salvar modelo"))
                .clicked()
            {
                actions.push(ProjectAction::SaveTemplate(ProjectTemplate {
                    name,
                    settings: self.sanitized(),
                }));
            }
        });
        ui.separator();

        // Frame size: a list of known sizes, then the two numbers
        let d = &mut self.draft;
        ui.horizontal(|ui| {
            ui.label("Resolução:");
            let selected = resolution_named(d.width, d.height)
                .map_or_else(|| "(personalizada)".to_owned(), |r| r.name.to_owned());
            egui::ComboBox::from_id_salt("project_resolution")
                .selected_text(selected)
                .width(260.0)
                .show_ui(ui, |ui| {
                    let mut group = "";
                    for r in RESOLUTIONS {
                        if r.group != group {
                            group = r.group;
                            ui.weak(group);
                        }
                        let on = (r.width, r.height) == (d.width, d.height);
                        if ui
                            .selectable_label(on, format!("{} — {}x{}", r.name, r.width, r.height))
                            .clicked()
                        {
                            d.width = r.width;
                            d.height = r.height;
                        }
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.label("Largura:");
            ui.add(egui::DragValue::new(&mut d.width).range(SIZE_RANGE.0..=SIZE_RANGE.1));
            ui.label("Altura:");
            ui.add(egui::DragValue::new(&mut d.height).range(SIZE_RANGE.0..=SIZE_RANGE.1));
            ui.weak(format!(
                "aspecto de tela {:.3}:1",
                display_aspect(d.width, d.height, d.pixel_aspect)
            ));
        });
        combo(
            ui,
            "Ordem dos campos",
            &mut d.field_order,
            &FieldOrder::ALL,
            FieldOrder::label,
        );
        ui.horizontal(|ui| {
            ui.label("Proporção do pixel:");
            let selected = PIXEL_ASPECTS
                .iter()
                .find(|(_, v)| (v - d.pixel_aspect).abs() < 1e-3)
                .map_or_else(
                    || format!("{:.4}", d.pixel_aspect),
                    |(l, _)| (*l).to_owned(),
                );
            egui::ComboBox::from_id_salt("project_par")
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    for (label, value) in PIXEL_ASPECTS {
                        ui.selectable_value(&mut d.pixel_aspect, *value, *label);
                    }
                });
        });
        combo(
            ui,
            "Modo HDR",
            &mut d.hdr_mode,
            &HdrMode::ALL,
            HdrMode::label,
        );
        combo(
            ui,
            "Rotação da saída",
            &mut d.output_rotation,
            &OutputRotation::ALL,
            OutputRotation::label,
        );
        ui.horizontal(|ui| {
            ui.label("Taxa de quadros:");
            let selected = frame_rate_of(d.fps)
                .map_or_else(|| format!("{:.3}", d.fps), |r| r.label.to_owned());
            egui::ComboBox::from_id_salt("project_fps")
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    for rate in FRAME_RATES {
                        if ui
                            .selectable_label((rate.fps - d.fps).abs() < 0.005, rate.label)
                            .clicked()
                        {
                            d.fps = rate.fps;
                        }
                    }
                });
            ui.add(
                egui::DragValue::new(&mut d.fps)
                    .range(1.0..=240.0)
                    .speed(0.01)
                    .suffix(" fps"),
            );
        });
        combo(
            ui,
            "Formato do pixel",
            &mut d.pixel_format,
            &PixelFormat::ALL,
            PixelFormat::label,
        );
        ui.add_enabled_ui(d.pixel_format.has_gamma_choice(), |ui| {
            combo(
                ui,
                "Gama de composição",
                &mut d.compositing_gamma,
                &CompositingGamma::ALL,
                CompositingGamma::label,
            );
        });
        combo(
            ui,
            "Qualidade da renderização",
            &mut d.render_quality,
            &RenderQuality::ALL,
            RenderQuality::label,
        );
        combo(
            ui,
            "Tipo de desfoque de movimento",
            &mut d.motion_blur,
            &MotionBlurType::ALL,
            MotionBlurType::label,
        );
        combo(
            ui,
            "Método de desentrelaçamento",
            &mut d.deinterlace,
            &Deinterlace::ALL,
            Deinterlace::label,
        );
        combo(
            ui,
            "Reamostragem",
            &mut d.resample_mode,
            &ResampleMode::ALL,
            ResampleMode::label,
        );
        ui.separator();
        ui.weak(
            "Já valem no programa: resolução, taxa de quadros e taxa de amostragem. As demais \
             opções ficam gravadas no projeto e serão aplicadas à renderização nas próximas versões.",
        );
    }

    fn audio_tab(&mut self, ui: &mut egui::Ui) {
        let d = &mut self.draft;
        ui.horizontal(|ui| {
            ui.label("Taxa de amostragem:");
            egui::ComboBox::from_id_salt("project_rate")
                .selected_text(format!("{} Hz", d.sample_rate))
                .show_ui(ui, |ui| {
                    for rate in SAMPLE_RATES {
                        ui.selectable_value(&mut d.sample_rate, *rate, format!("{rate} Hz"));
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.label("Profundidade de bits:");
            egui::ComboBox::from_id_salt("project_bits")
                .selected_text(bits_label(d.audio_bit_depth))
                .show_ui(ui, |ui| {
                    for bits in AUDIO_BIT_DEPTHS {
                        ui.selectable_value(&mut d.audio_bit_depth, *bits, bits_label(*bits));
                    }
                });
        });
        combo(
            ui,
            "Barramento master",
            &mut d.master_bus,
            &MasterBusMode::ALL,
            MasterBusMode::label,
        );
        ui.horizontal(|ui| {
            ui.label("Nível de pico da normalização:");
            ui.add(
                egui::DragValue::new(&mut d.normalize_peak_db)
                    .range(-60.0..=0.0)
                    .speed(0.1)
                    .suffix(" dBFS"),
            );
        });
        ui.separator();
        ui.weak(
            "A taxa de amostragem vale na exportação. A profundidade de bits e o barramento \
             surround ficam gravados no projeto; hoje o mixer trabalha em estéreo.",
        );
    }
}

fn bits_label(bits: u32) -> String {
    match bits {
        32 => "32 bits (ponto flutuante)".into(),
        n => format!("{n} bits"),
    }
}

/// A labelled drop-down over a fixed list of options.
fn combo<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut T,
    all: &[T],
    name: fn(T) -> &'static str,
) {
    ui.horizontal(|ui| {
        ui.label(format!("{label}:"));
        egui::ComboBox::from_id_salt(label)
            .selected_text(name(*value))
            .show_ui(ui, |ui| {
                for option in all {
                    ui.selectable_value(value, *option, name(*option));
                }
            });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> ProjectDialogState {
        let mut s = ProjectDialogState::default();
        s.open_with(&ProjectSettings::default(), false);
        s
    }

    #[test]
    fn a_template_sets_the_picture_format_and_keeps_the_audio_options() {
        let mut s = state();
        s.draft.sample_rate = 96_000;
        s.draft.audio_bit_depth = 32;
        s.draft.normalize_peak_db = -3.0;
        let template = builtin_templates()
            .into_iter()
            .find(|t| t.name.starts_with("4K UHD 25"))
            .unwrap();
        s.use_template(&template);
        assert_eq!((s.draft.width, s.draft.height), (3840, 2160));
        assert_eq!(s.draft.fps, 25.0);
        assert_eq!(
            s.draft.sample_rate, 96_000,
            "the sample rate is an audio setting"
        );
        assert_eq!(
            s.draft.audio_bit_depth, 32,
            "the audio depth is not a picture setting"
        );
        assert_eq!(s.draft.normalize_peak_db, -3.0);
        assert_eq!(s.chosen_template.as_deref(), Some(template.name.as_str()));
    }

    #[test]
    fn matching_a_media_file_copies_its_size_and_rate_and_forgets_the_template() {
        let mut s = state();
        s.chosen_template = Some("x".into());
        s.use_media(1921, 1081, 59.94);
        assert_eq!((s.draft.width, s.draft.height), (1920, 1080), "made even");
        assert!((s.draft.fps - 59.94).abs() < 1e-9);
        assert!(s.chosen_template.is_none());
        s.use_media(1280, 720, f64::NAN);
        assert!(
            (s.draft.fps - 59.94).abs() < 1e-9,
            "a bad rate changes nothing"
        );
    }

    #[test]
    fn the_draft_is_kept_inside_the_accepted_ranges_when_applied() {
        let mut s = state();
        s.draft.width = 3;
        s.draft.height = 99_999;
        s.draft.fps = 5_000.0;
        s.draft.pixel_aspect = f64::NAN;
        s.draft.normalize_peak_db = 12.0;
        let out = s.sanitized();
        assert_eq!((out.width, out.height), (SIZE_RANGE.0, SIZE_RANGE.1));
        assert_eq!(out.fps, 240.0);
        assert_eq!(out.pixel_aspect, 1.0);
        assert_eq!(out.normalize_peak_db, 0.0);
    }

    #[test]
    fn opening_copies_the_project_so_cancelling_changes_nothing() {
        let project = ProjectSettings::default();
        let mut s = ProjectDialogState::default();
        s.open_with(&project, true);
        s.draft.width = 640;
        assert_eq!(project.width, 1920, "the project is untouched until Apply");
        assert!(s.start_all_new && s.open);
    }
}
