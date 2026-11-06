//! "Inserir Mídia Gerada…" modal dialog (`Cor sólida`, `Padrão de teste`, `Gradiente`,
//! `Títulos e texto`).

use eframe::egui::{self, Color32};
use neoveg_common::i18n::tr;
use neoveg_common::i18n::tr_args;
use neoveg_common::title_style::{self, TitleStyle};
use neoveg_video::generators::{
    GeneratorKind, GeneratorSpec, GradientShape, TestPatternVariant, TitleAlign, TitleAnimation,
    TitleLayout, TitleSpan,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GeneratorTab {
    #[default]
    SolidColor,
    TestPattern,
    Gradient,
    Noise,
    Countdown,
    TitleText,
}

impl GeneratorTab {
    pub const ALL: [Self; 6] = [
        Self::SolidColor,
        Self::TestPattern,
        Self::Gradient,
        Self::Noise,
        Self::Countdown,
        Self::TitleText,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::SolidColor => tr("Cor sólida"),
            Self::TestPattern => tr("Padrão de teste"),
            Self::Gradient => tr("Gradiente"),
            Self::Noise => tr("Ruído"),
            Self::Countdown => tr("Contagem regressiva"),
            Self::TitleText => tr("Títulos e texto"),
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
    // Noise
    pub noise_colored: bool,
    pub noise_dark: [u8; 4],
    pub noise_light: [u8; 4],
    pub noise_grain: u32,
    pub noise_seed: u32,
    pub noise_animated: bool,
    // Countdown
    pub countdown_from: u32,
    pub countdown_text: [u8; 4],
    pub countdown_bg: [u8; 4],
    pub countdown_line: [u8; 4],
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
    /// Alignment, fixed width and styles of stretches of the title.
    pub title_layout: TitleLayout,
    /// The style the next stretch gets (*Aplicar ao trecho selecionado*).
    span_draft: TitleSpan,
    span_with_color: bool,
    /// The characters that were selected in the text box the last time (start, end).
    text_selection: Option<(usize, usize)>,
    /// Families and their styles, read from the text engine when the controls first need them.
    font_list: Vec<(String, Vec<String>)>,
    /// The name typed for a title style.
    style_name: String,
    /// The saved styles changed: the caller keeps the preferences.
    pub styles_dirty: bool,
    /// Title templates: values of the fields of the text and the library picker.
    pub template: crate::title_template_ui::TemplateUi,
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
            noise_colored: false,
            noise_dark: [0, 0, 0, 255],
            noise_light: [255, 255, 255, 255],
            noise_grain: 1,
            noise_seed: 1,
            noise_animated: true,
            countdown_from: 5,
            countdown_text: [255, 255, 255, 255],
            countdown_bg: [0, 0, 0, 255],
            countdown_line: [96, 96, 96, 255],
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
            title_layout: TitleLayout::default(),
            span_draft: TitleSpan::new(0, 0),
            span_with_color: false,
            text_selection: None,
            font_list: Vec::new(),
            style_name: String::new(),
            styles_dirty: false,
            template: Default::default(),
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

/// The stack of fills, outlines and shadows over the text: while it is empty the single ones above
/// are used; once it has a layer it replaces them.
fn stack_controls(ui: &mut egui::Ui, stack: &mut neoveg_text::title_paint::PaintStack) {
    use neoveg_text::title_paint::{FillLayer, OutlineLayer, OutlinePlacement, ShadowLayer};
    ui.collapsing(
        tr("Camadas empilhadas (preenchimentos, contornos e sombras)"),
        |ui| {
            ui.weak(tr(
                "Com ao menos uma camada, a pilha substitui o contorno e a sombra simples acima.",
            ));
            ui.horizontal(|ui| {
                if ui.button(tr("+ Preenchimento")).clicked() && stack.fills.len() < 8 {
                    stack.fills.push(FillLayer {
                        color: [255, 255, 255, 255],
                        offset: [0, 0],
                    });
                }
                if ui.button(tr("+ Contorno")).clicked() && stack.outlines.len() < 8 {
                    stack.outlines.push(OutlineLayer {
                        color: [0, 0, 0, 255],
                        width: 3.0,
                        placement: OutlinePlacement::Outer,
                    });
                }
                if ui.button(tr("+ Sombra")).clicked() && stack.shadows.len() < 8 {
                    stack.shadows.push(ShadowLayer {
                        color: [0, 0, 0, 180],
                        offset: [4, 4],
                        blur: 0.0,
                    });
                }
                if ui.button(tr("Limpar pilha")).clicked() {
                    *stack = Default::default();
                }
            });
            let mut remove: Option<(u8, usize)> = None;
            for (i, f) in stack.fills.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    edit_rgba(ui, tr("Preenchimento:"), &mut f.color);
                    ui.add(
                        egui::DragValue::new(&mut f.offset[0])
                            .range(-24..=24)
                            .prefix("ΔX: "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut f.offset[1])
                            .range(-24..=24)
                            .prefix("ΔY: "),
                    );
                    if ui.small_button("✕").clicked() {
                        remove = Some((0, i));
                    }
                });
            }
            for (i, o) in stack.outlines.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    edit_rgba(ui, tr("Contorno:"), &mut o.color);
                    ui.add(
                        egui::DragValue::new(&mut o.width)
                            .range(0.5..=24.0)
                            .prefix(tr("Espessura: "))
                            .suffix(" px"),
                    );
                    if ui.small_button("✕").clicked() {
                        remove = Some((1, i));
                    }
                });
            }
            for (i, sh) in stack.shadows.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    edit_rgba(ui, tr("Sombra:"), &mut sh.color);
                    ui.add(
                        egui::DragValue::new(&mut sh.offset[0])
                            .range(-48..=48)
                            .prefix("ΔX: "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut sh.offset[1])
                            .range(-48..=48)
                            .prefix("ΔY: "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut sh.blur)
                            .range(0.0..=24.0)
                            .prefix(tr("Desfoque: ")),
                    );
                    if ui.small_button("✕").clicked() {
                        remove = Some((2, i));
                    }
                });
            }
            match remove {
                Some((0, i)) => {
                    stack.fills.remove(i);
                }
                Some((1, i)) => {
                    stack.outlines.remove(i);
                }
                Some((_, i)) => {
                    stack.shadows.remove(i);
                }
                None => {}
            }
        },
    );
}

/// The animation with the snake-case id of a title style.
fn animation_of(id: &str) -> TitleAnimation {
    TitleAnimation::ALL
        .into_iter()
        .find(|a| {
            serde_json::to_value(a)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .as_deref()
                == Some(id)
        })
        .unwrap_or_default()
}

/// The alignment with the id of a title style.
fn align_of(id: &str) -> TitleAlign {
    TitleAlign::ALL
        .into_iter()
        .find(|a| align_id(*a) == id)
        .unwrap_or_default()
}

/// The id of `align` in a title style.
fn align_id(align: TitleAlign) -> String {
    match align {
        TitleAlign::Left => "left",
        TitleAlign::Center => "center",
        TitleAlign::Right => "right",
        TitleAlign::Justify => "justify",
    }
    .into()
}

/// The snake-case id of `animation`.
fn animation_id(animation: TitleAnimation) -> String {
    serde_json::to_value(animation)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "static".into())
}

impl GeneratorDialogState {
    /// The look of the title in the controls, under `name`.
    pub fn current_style(&self, name: &str) -> TitleStyle {
        TitleStyle {
            name: name.to_owned(),
            font: self.title_font.clone(),
            font_style: self.title_font_style.clone(),
            scale: self.title_scale,
            text_rgba: self.title_rgba,
            bg_rgba: self.title_bg_rgba,
            outline_rgba: self.title_outline_rgba,
            outline_px: self.title_outline_px,
            shadow_rgba: self.title_shadow_rgba,
            shadow_offset: self.title_shadow_offset,
            align_y: self.title_align_y,
            animation: animation_id(self.title_animation),
            align: align_id(self.title_layout.align),
            box_width: self.title_layout.box_width,
        }
    }

    /// Puts the look of `style` in the controls; the text of the title stays.
    pub fn apply_style(&mut self, style: &TitleStyle) {
        self.title_font = style.font.clone();
        self.title_font_style = style.font_style.clone();
        self.title_scale = style.scale;
        self.title_rgba = style.text_rgba;
        self.title_bg_rgba = style.bg_rgba;
        self.title_outline_rgba = style.outline_rgba;
        self.title_outline_px = style.outline_px;
        self.title_shadow_rgba = style.shadow_rgba;
        self.title_shadow_offset = style.shadow_offset;
        self.title_align_y = style.align_y;
        self.title_animation = animation_of(&style.animation);
        self.title_layout.align = align_of(&style.align);
        self.title_layout.box_width = style.box_width;
    }

    /// Alignment, fixed width and the styles of stretches of the title.
    fn paragraph_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(tr("Alinhamento:"));
            for align in TitleAlign::ALL {
                ui.selectable_value(&mut self.title_layout.align, align, align.label());
            }
        });
        ui.horizontal(|ui| {
            let mut percent = self.title_layout.box_width * 100.0;
            let slider = egui::Slider::new(&mut percent, 0.0..=100.0)
                .text(tr(
                    "Largura fixa do parágrafo (% da tela; 0 = linhas livres)",
                ))
                .fixed_decimals(0);
            if ui.add(slider).changed() {
                self.title_layout.box_width = (percent / 100.0).clamp(0.0, 1.0);
            }
        });
        if self.title_layout.align == TitleAlign::Justify && self.title_layout.box_width <= 0.0 {
            ui.weak(tr("Justificado precisa de uma largura fixa."));
        }
        ui.collapsing(tr("Estilos por trecho"), |ui| {
            let draft = &mut self.span_draft;
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut draft.bold, tr("Negrito"));
                ui.checkbox(&mut draft.italic, tr("Itálico"));
                ui.checkbox(&mut draft.underline, tr("Sublinhado"));
                ui.checkbox(&mut self.span_with_color, tr("Cor"));
                if self.span_with_color {
                    let mut rgba = draft.color.unwrap_or([255, 220, 60, 255]);
                    let mut color =
                        Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]);
                    if ui.color_edit_button_srgba(&mut color).changed() {
                        rgba = color.to_srgba_unmultiplied();
                    }
                    draft.color = Some(rgba);
                } else {
                    draft.color = None;
                }
                ui.add(
                    egui::DragValue::new(&mut draft.scale)
                        .range(0.25..=4.0)
                        .speed(0.02)
                        .prefix(tr("Tamanho ×")),
                );
            });
            let chars = self.title_text.chars().count();
            let selected = self
                .text_selection
                .map(|(a, b)| (a.min(chars), b.min(chars)))
                .filter(|(a, b)| a < b);
            if ui
                .add_enabled(
                    selected.is_some(),
                    egui::Button::new(tr("Aplicar ao trecho selecionado")),
                )
                .on_hover_text(tr("Selecione parte do texto acima e escolha o estilo"))
                .clicked()
            {
                if let Some((start, end)) = selected {
                    let mut span = self.span_draft.clone();
                    (span.start, span.end) = (start, end);
                    if self.title_layout.spans.len() < 32 {
                        self.title_layout.spans.push(span);
                    }
                }
            }
            let mut remove = None;
            for (index, span) in self.title_layout.spans.iter().enumerate() {
                ui.horizontal(|ui| {
                    if ui.small_button("✖").clicked() {
                        remove = Some(index);
                    }
                    let excerpt: String = self
                        .title_text
                        .chars()
                        .skip(span.start)
                        .take(span.end.saturating_sub(span.start).min(24))
                        .collect();
                    let mut flags = String::new();
                    for (on, mark) in [(span.bold, "N"), (span.italic, "I"), (span.underline, "S")]
                    {
                        if on {
                            flags.push_str(mark);
                        }
                    }
                    ui.label(format!("“{excerpt}”  {flags}"));
                    if let Some(c) = span.color {
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                        ui.painter().rect_filled(
                            rect,
                            2.0,
                            Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]),
                        );
                    }
                    if (span.scale - 1.0).abs() > 1e-3 {
                        ui.weak(format!("×{:.2}", span.scale));
                    }
                });
            }
            if let Some(index) = remove {
                self.title_layout.spans.remove(index);
            }
        });
    }

    /// The row of the styles: pick one to apply it, or name the look in the controls and keep it.
    fn style_row(&mut self, ui: &mut egui::Ui, saved: &mut Vec<TitleStyle>) {
        ui.horizontal(|ui| {
            ui.label(tr("Estilo:"));
            let mut picked: Option<TitleStyle> = None;
            egui::ComboBox::from_id_salt("gen_title_style")
                .selected_text(tr("Escolher…"))
                .width(130.0)
                .show_ui(ui, |ui| {
                    for style in title_style::all(saved) {
                        if ui.selectable_label(false, &style.name).clicked() {
                            picked = Some(style);
                        }
                    }
                });
            if let Some(style) = picked {
                self.style_name = style.name.clone();
                self.apply_style(&style);
            }
            ui.add(
                egui::TextEdit::singleline(&mut self.style_name)
                    .hint_text(tr("Nome"))
                    .desired_width(110.0),
            );
            let name = title_style::clean_name(&self.style_name);
            if ui
                .add_enabled(name.is_some(), egui::Button::new(tr("Salvar")))
                .on_hover_text(tr(
                    "Guarda a aparência do título (sem o texto) sob esse nome",
                ))
                .clicked()
            {
                if let Some(name) = &name {
                    let style = self.current_style(name);
                    self.styles_dirty |= title_style::save(saved, &style);
                }
            }
            let deletable = name
                .as_deref()
                .is_some_and(|n| saved.iter().any(|s| s.name == n));
            if ui
                .add_enabled(deletable, egui::Button::new(tr("Excluir")))
                .clicked()
            {
                if let Some(name) = &name {
                    self.styles_dirty |= title_style::remove(saved, name);
                }
            }
        });
    }

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
            GeneratorKind::Noise {
                colored,
                dark_rgba,
                light_rgba,
                grain,
                seed,
                animated,
            } => {
                self.tab = GeneratorTab::Noise;
                self.noise_colored = *colored;
                self.noise_dark = *dark_rgba;
                self.noise_light = *light_rgba;
                self.noise_grain = *grain;
                self.noise_seed = *seed;
                self.noise_animated = *animated;
            }
            GeneratorKind::Countdown {
                from,
                text_rgba,
                bg_rgba,
                line_rgba,
            } => {
                self.tab = GeneratorTab::Countdown;
                self.countdown_from = *from;
                self.countdown_text = *text_rgba;
                self.countdown_bg = *bg_rgba;
                self.countdown_line = *line_rgba;
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
                layout,
            } => {
                self.tab = GeneratorTab::TitleText;
                self.title_layout = layout.clone();
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

    /// The spec with the fields `{{name}}` of a title filled in from the template controls.
    pub fn build_spec(&self, width: u32, height: u32, fps: f64) -> GeneratorSpec {
        let spec = self.build_spec_raw(width, height, fps);
        if self.tab != GeneratorTab::TitleText {
            return spec;
        }
        let template = neoveg_video::title_template::TitleTemplate::new(&spec.name.clone(), spec);
        let mut filled = template.instantiate(&self.template.values);
        if let GeneratorKind::TitleText { text, .. } = &filled.kind {
            let first = text
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("Título")
                .trim();
            let short: String = first.chars().take(24).collect();
            filled.name = tr_args("Título: {}", &[&short]);
        }
        filled
    }

    /// The spec exactly as the controls hold it (markers of fields kept in the text).
    pub fn build_spec_raw(&self, width: u32, height: u32, fps: f64) -> GeneratorSpec {
        let (name, kind) = match self.tab {
            GeneratorTab::SolidColor => (
                tr("Cor sólida").to_string(),
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
                tr_args("Gradiente ({})", &[&self.gradient_shape.label()]),
                GeneratorKind::Gradient {
                    shape: self.gradient_shape,
                    start_rgba: self.grad_start,
                    end_rgba: self.grad_end,
                    angle_deg: self.grad_angle_deg,
                },
            ),
            GeneratorTab::Noise => (
                tr("Ruído").to_string(),
                GeneratorKind::Noise {
                    colored: self.noise_colored,
                    dark_rgba: self.noise_dark,
                    light_rgba: self.noise_light,
                    grain: self.noise_grain.clamp(1, 64),
                    seed: self.noise_seed,
                    animated: self.noise_animated,
                },
            ),
            GeneratorTab::Countdown => (
                tr("Contagem regressiva").to_string(),
                GeneratorKind::Countdown {
                    from: self.countdown_from.clamp(1, 99),
                    text_rgba: self.countdown_text,
                    bg_rgba: self.countdown_bg,
                    line_rgba: self.countdown_line,
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
                    tr_args("Título: {}", &[&short]),
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
                        layout: self.title_layout.clone(),
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
        styles: &mut Vec<TitleStyle>,
    ) -> Option<GeneratorSpec> {
        if !self.open {
            return None;
        }
        let mut open = self.open;
        let mut confirmed = false;
        let mut cancelled = false;

        let editing = self.editing.is_some();
        let title = if editing {
            tr("Editar mídia gerada")
        } else {
            tr("Geradores de Mídia")
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
                    GeneratorTab::Noise => {
                        ui.checkbox(
                            &mut self.noise_colored,
                            tr("Colorido (cada canal ao acaso)"),
                        );
                        ui.add_enabled_ui(!self.noise_colored, |ui| {
                            edit_rgba(ui, tr("Cor dos pontos mais escuros:"), &mut self.noise_dark);
                            edit_rgba(ui, tr("Cor dos pontos mais claros:"), &mut self.noise_light);
                        });
                        ui.horizontal(|ui| {
                            ui.label(tr("Tamanho do grão (pixels):"));
                            ui.add(egui::DragValue::new(&mut self.noise_grain).range(1..=64));
                        });
                        ui.horizontal(|ui| {
                            ui.label(tr("Semente:"));
                            ui.add(egui::DragValue::new(&mut self.noise_seed));
                        });
                        ui.checkbox(
                            &mut self.noise_animated,
                            tr("Animado (muda a cada quadro)"),
                        );
                    }
                    GeneratorTab::Countdown => {
                        ui.horizontal(|ui| {
                            ui.label(tr("Contar a partir de:"));
                            ui.add(egui::DragValue::new(&mut self.countdown_from).range(1..=99));
                        });
                        edit_rgba(ui, tr("Cor do número:"), &mut self.countdown_text);
                        edit_rgba(ui, tr("Cor do fundo:"), &mut self.countdown_bg);
                        edit_rgba(ui, tr("Cor do anel e da mira:"), &mut self.countdown_line);
                    }
                    GeneratorTab::TitleText => {
                        self.style_row(ui, styles);
                        crate::title_template_ui::template_row(ui, self);
                        ui.label(tr("Texto do título (suporta múltiplas linhas):"));
                        let edited = egui::TextEdit::multiline(&mut self.title_text)
                            .desired_rows(3)
                            .desired_width(f32::INFINITY)
                            .show(ui);
                        if let Some(range) = edited.cursor_range {
                            let (a, b) = (range.primary.ccursor.index, range.secondary.ccursor.index);
                            self.text_selection = Some((a.min(b), a.max(b)));
                        }
                        self.paragraph_controls(ui);
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
                                .button(tr("Fontes do sistema"))
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
                            edit_rgba(ui, tr("Texto:"), &mut self.title_rgba);
                            edit_rgba(ui, tr("Fundo:"), &mut self.title_bg_rgba);
                        });
                        ui.horizontal(|ui| {
                            edit_rgba(ui, tr("Contorno:"), &mut self.title_outline_rgba);
                            ui.add(
                                egui::DragValue::new(&mut self.title_outline_px)
                                    .range(0..=10)
                                    .prefix(tr("Espessura: "))
                                    .suffix(" px"),
                            );
                        });
                        ui.horizontal(|ui| {
                            edit_rgba(ui, tr("Sombra:"), &mut self.title_shadow_rgba);
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
                        stack_controls(ui, &mut self.title_layout.stack);
                    }
                }

                ui.separator();
                ui.horizontal(|ui| {
                    let label = if editing {
                        tr("Aplicar")
                    } else {
                        tr("Inserir na timeline")
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
