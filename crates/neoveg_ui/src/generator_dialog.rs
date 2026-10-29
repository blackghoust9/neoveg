//! "Inserir Mídia Gerada…" modal dialog (`Cor sólida`, `Padrão de teste`, `Gradiente`,
//! `Títulos e texto`).

use eframe::egui::{self, Color32};
use neoveg_common::i18n::tr;
use neoveg_video::generators::{
    GeneratorKind, GeneratorSpec, GradientShape, TestPatternVariant, TitleAnimation,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GeneratorTab {
    #[default]
    SolidColor,
    TestPattern,
    Gradient,
    TitleText,
}

impl GeneratorTab {
    pub const ALL: [Self; 4] = [
        Self::SolidColor,
        Self::TestPattern,
        Self::Gradient,
        Self::TitleText,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::SolidColor => "Cor sólida",
            Self::TestPattern => "Padrão de teste",
            Self::Gradient => "Gradiente",
            Self::TitleText => "Títulos e texto",
        }
    }
}

pub struct GeneratorDialogState {
    pub open: bool,
    /// The media pool entry being edited and the spec it had (`None` when a new one is made).
    pub editing: Option<(u64, GeneratorSpec)>,
    pub tab: GeneratorTab,
    pub duration_seconds: f64,
    // Solid color
    pub solid_rgba: [u8; 4],
    // Test pattern
    pub pattern: TestPatternVariant,
    // Gradient
    pub gradient_shape: GradientShape,
    pub grad_start: [u8; 4],
    pub grad_end: [u8; 4],
    pub grad_angle_deg: f32,
    // Title & text
    pub title_text: String,
    pub title_scale: u32,
    pub title_rgba: [u8; 4],
    pub title_bg_rgba: [u8; 4],
    pub title_outline_rgba: [u8; 4],
    pub title_outline_px: u32,
    pub title_shadow_rgba: [u8; 4],
    pub title_shadow_offset: [i32; 2],
    pub title_align_y: f32,
    pub title_animation: TitleAnimation,
    pub title_font: String,
    pub title_font_style: String,
    /// Families and their styles, read from the text engine when the controls first need them.
    font_list: Vec<(String, Vec<String>)>,
}

impl Default for GeneratorDialogState {
    fn default() -> Self {
        Self {
            open: false,
            editing: None,
            tab: GeneratorTab::TitleText,
            duration_seconds: 5.0,
            solid_rgba: [28, 110, 216, 255],
            pattern: TestPatternVariant::SmpteBars,
            gradient_shape: GradientShape::Linear,
            grad_start: [18, 28, 54, 255],
            grad_end: [232, 118, 52, 255],
            grad_angle_deg: 35.0,
            title_text: "NEOVEG STUDIO".into(),
            title_scale: 6,
            title_rgba: [255, 255, 255, 255],
            title_bg_rgba: [0, 0, 0, 0],
            title_outline_rgba: [14, 16, 22, 255],
            title_outline_px: 2,
            title_shadow_rgba: [0, 0, 0, 180],
            title_shadow_offset: [4, 4],
            title_align_y: 0.5,
            title_animation: TitleAnimation::Static,
            title_font: "Inter".into(),
            title_font_style: "Bold".into(),
            font_list: Vec::new(),
        }
    }
}

fn edit_rgba(ui: &mut egui::Ui, label: &str, rgba: &mut [u8; 4]) {
    let mut color = Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]);
    ui.horizontal(|ui| {
        ui.label(label);
        if ui.color_edit_button_srgba(&mut color).changed() {
            *rgba = color.to_srgba_unmultiplied();
        }
    });
}

impl GeneratorDialogState {
    /// Opens the dialog on an existing generated media, with its values in the controls. Applying it
    /// gives back a spec of the same size and rate for the media `media_id`.
    pub fn edit(&mut self, media_id: u64, spec: &GeneratorSpec) {
        self.duration_seconds = spec.duration;
        match &spec.kind {
            GeneratorKind::SolidColor { rgba } => {
                self.tab = GeneratorTab::SolidColor;
                self.solid_rgba = *rgba;
            }
            GeneratorKind::TestPattern { variant } => {
                self.tab = GeneratorTab::TestPattern;
                self.pattern = *variant;
            }
            GeneratorKind::Gradient {
                shape,
                start_rgba,
                end_rgba,
                angle_deg,
            } => {
                self.tab = GeneratorTab::Gradient;
                self.gradient_shape = *shape;
                self.grad_start = *start_rgba;
                self.grad_end = *end_rgba;
                self.grad_angle_deg = *angle_deg;
            }
            GeneratorKind::TitleText {
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
            } => {
                self.tab = GeneratorTab::TitleText;
                self.title_font = font.clone();
                self.title_font_style = font_style.clone();
                self.title_text = text.clone();
                self.title_scale = *scale;
                self.title_rgba = *text_rgba;
                self.title_bg_rgba = *bg_rgba;
                self.title_outline_rgba = *outline_rgba;
                self.title_outline_px = *outline_px;
                self.title_shadow_rgba = *shadow_rgba;
                self.title_shadow_offset = *shadow_offset;
                self.title_align_y = *align_y;
                self.title_animation = *animation;
            }
        }
        self.editing = Some((media_id, spec.clone()));
        self.open = true;
    }

    pub fn build_spec(&self, width: u32, height: u32, fps: f64) -> GeneratorSpec {
        let (name, kind) = match self.tab {
            GeneratorTab::SolidColor => (
                "Cor sólida".to_string(),
                GeneratorKind::SolidColor {
                    rgba: self.solid_rgba,
                },
            ),
            GeneratorTab::TestPattern => (
                self.pattern.label().to_string(),
                GeneratorKind::TestPattern {
                    variant: self.pattern,
                },
            ),
            GeneratorTab::Gradient => (
                format!("Gradiente ({})", self.gradient_shape.label()),
                GeneratorKind::Gradient {
                    shape: self.gradient_shape,
                    start_rgba: self.grad_start,
                    end_rgba: self.grad_end,
                    angle_deg: self.grad_angle_deg,
                },
            ),
            GeneratorTab::TitleText => {
                let first_line = self
                    .title_text
                    .lines()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("Título")
                    .trim();
                let short: String = first_line.chars().take(24).collect();
                (
                    format!("Título: {short}"),
                    GeneratorKind::TitleText {
                        text: self.title_text.clone(),
                        scale: self.title_scale.clamp(1, 20),
                        text_rgba: self.title_rgba,
                        bg_rgba: self.title_bg_rgba,
                        outline_rgba: self.title_outline_rgba,
                        outline_px: self.title_outline_px.min(12),
                        shadow_rgba: self.title_shadow_rgba,
                        shadow_offset: self.title_shadow_offset,
                        align_y: self.title_align_y.clamp(0.05, 0.95),
                        animation: self.title_animation,
                        font: self.title_font.clone(),
                        font_style: self.title_font_style.clone(),
                    },
                )
            }
        };

        GeneratorSpec {
            name,
            width: width.max(2),
            height: height.max(2),
            fps: if fps.is_finite() && fps > 0.0 {
                fps
            } else {
                29.97
            },
            duration: self.duration_seconds.clamp(0.2, 3600.0),
            kind,
        }
    }

    /// Renders the modal dialog; returns `Some(GeneratorSpec)` when the user clicks **Inserir na timeline**.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        project_width: u32,
        project_height: u32,
        project_fps: f64,
    ) -> Option<GeneratorSpec> {
        if !self.open {
            return None;
        }
        let mut open = self.open;
        let mut confirmed = false;
        let mut cancelled = false;

        let editing = self.editing.is_some();
        let title = if editing {
            "Editar mídia gerada"
        } else {
            "Geradores de Mídia"
        };
        crate::dialogs::dialog(ctx, title)
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(480.0)
            .show(ctx, |ui| {
                // The kind of a media that exists does not change, only its values
                ui.horizontal_wrapped(|ui| {
                    for candidate in GeneratorTab::ALL {
                        if editing && candidate != self.tab {
                            continue;
                        }
                        ui.selectable_value(&mut self.tab, candidate, candidate.label());
                    }
                });
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label(tr("Duração inicial (s):"));
                    ui.add(
                        egui::DragValue::new(&mut self.duration_seconds)
                            .range(0.2..=600.0)
                            .speed(0.25),
                    );
                });
                ui.add_space(4.0);

                match self.tab {
                    GeneratorTab::SolidColor => {
                        edit_rgba(ui, "Cor de preenchimento:", &mut self.solid_rgba);
                    }
                    GeneratorTab::TestPattern => {
                        ui.horizontal(|ui| {
                            ui.label(tr("Padrão:"));
                            egui::ComboBox::from_id_salt("gen_test_pattern")
                                .selected_text(self.pattern.label())
                                .show_ui(ui, |ui| {
                                    for p in TestPatternVariant::ALL {
                                        ui.selectable_value(&mut self.pattern, p, p.label());
                                    }
                                });
                        });
                    }
                    GeneratorTab::Gradient => {
                        ui.horizontal(|ui| {
                            ui.label(tr("Formato:"));
                            for sh in GradientShape::ALL {
                                ui.selectable_value(&mut self.gradient_shape, sh, sh.label());
                            }
                        });
                        edit_rgba(ui, "Cor inicial:", &mut self.grad_start);
                        edit_rgba(ui, "Cor final:", &mut self.grad_end);
                        if self.gradient_shape == GradientShape::Linear {
                            ui.add(
                                egui::Slider::new(&mut self.grad_angle_deg, -180.0..=180.0)
                                    .text(tr("Ângulo (°)")),
                            );
                        }
                    }
                    GeneratorTab::TitleText => {
                        ui.label(tr("Texto do título (suporta múltiplas linhas):"));
                        ui.add(
                            egui::TextEdit::multiline(&mut self.title_text)
                                .desired_rows(3)
                                .desired_width(f32::INFINITY),
                        );
                        ui.horizontal(|ui| {
                            ui.label(tr("Fonte:"));
                            if self.font_list.is_empty() {
                                self.font_list = neoveg_text::fonts::families();
                            }
                            let mut family_pick: Option<String> = None;
                            egui::ComboBox::from_id_salt("gen_title_font")
                                .selected_text(self.title_font.clone())
                                .show_ui(ui, |ui| {
                                    for (family, _) in &self.font_list {
                                        if ui
                                            .selectable_label(self.title_font == *family, family)
                                            .clicked()
                                        {
                                            family_pick = Some(family.clone());
                                        }
                                    }
                                });
                            let styles: Vec<String> = self
                                .font_list
                                .iter()
                                .find(|(family, _)| *family == self.title_font)
                                .map(|(_, styles)| styles.clone())
                                .unwrap_or_default();
                            if let Some(family) = family_pick {
                                self.title_font = family;
                                let next = self
                                    .font_list
                                    .iter()
                                    .find(|(f, _)| *f == self.title_font)
                                    .map(|(_, styles)| styles.clone())
                                    .unwrap_or_default();
                                if !next.contains(&self.title_font_style) {
                                    self.title_font_style = next
                                        .iter()
                                        .find(|s| s.as_str() == "Bold")
                                        .or(next.first())
                                        .cloned()
                                        .unwrap_or_else(|| "Regular".into());
                                }
                            }
                            egui::ComboBox::from_id_salt("gen_title_font_style")
                                .selected_text(self.title_font_style.clone())
                                .show_ui(ui, |ui| {
                                    for style in &styles {
                                        ui.selectable_value(
                                            &mut self.title_font_style,
                                            style.clone(),
                                            style,
                                        );
                                    }
                                });
                            if ui
                                .button("Fontes do sistema")
                                .on_hover_text(
                                    tr("Procura as fontes instaladas no computador (pode levar um instante)"),
                                )
                                .clicked()
                            {
                                neoveg_text::fonts::scan_system();
                                self.font_list = neoveg_text::fonts::families();
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label(tr("Animação:"));
                            egui::ComboBox::from_id_salt("gen_title_anim")
                                .selected_text(self.title_animation.label())
                                .show_ui(ui, |ui| {
                                    for a in TitleAnimation::ALL {
                                        ui.selectable_value(
                                            &mut self.title_animation,
                                            a,
                                            a.label(),
                                        );
                                    }
                                });
                        });
                        ui.add(
                            egui::Slider::new(&mut self.title_scale, 2..=16)
                                .text(tr("Escala da fonte")),
                        );
                        ui.add(
                            egui::Slider::new(&mut self.title_align_y, 0.10..=0.90)
                                .text(tr("Posição vertical (0.5 = centro, 0.82 = terço inferior)")),
                        );
                        ui.horizontal(|ui| {
                            edit_rgba(ui, "Texto:", &mut self.title_rgba);
                            edit_rgba(ui, "Fundo:", &mut self.title_bg_rgba);
                        });
                        ui.horizontal(|ui| {
                            edit_rgba(ui, "Contorno:", &mut self.title_outline_rgba);
                            ui.add(
                                egui::DragValue::new(&mut self.title_outline_px)
                                    .range(0..=10)
                                    .prefix("Espessura: ")
                                    .suffix(" px"),
                            );
                        });
                        ui.horizontal(|ui| {
                            edit_rgba(ui, "Sombra:", &mut self.title_shadow_rgba);
                            ui.add(
                                egui::DragValue::new(&mut self.title_shadow_offset[0])
                                    .range(-24..=24)
                                    .prefix("ΔX: "),
                            );
                            ui.add(
                                egui::DragValue::new(&mut self.title_shadow_offset[1])
                                    .range(-24..=24)
                                    .prefix("ΔY: "),
                            );
                        });
                    }
                }

                ui.separator();
                ui.horizontal(|ui| {
                    let label = if editing {
                        "Aplicar"
                    } else {
                        "Inserir na timeline"
                    };
                    if ui.button(label).clicked() {
                        confirmed = true;
                    }
                    if ui.button(tr("Cancelar")).clicked() {
                        cancelled = true;
                    }
                });
            });

        self.open = open && !confirmed && !cancelled;
        if !self.open && !confirmed {
            self.editing = None;
        }
        // Editing keeps the size and the rate the media was made with
        let (width, height, fps) = match &self.editing {
            Some((_, spec)) => (spec.width, spec.height, spec.fps),
            None => (project_width, project_height, project_fps),
        };
        confirmed.then(|| self.build_spec(width, height, fps))
    }
}
