//! "Renderizar como… (*Make Movie*)" dialog and non-blocking background render job state.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui;
use neoveg_audio::readout::LoudnessSnapshot;
use neoveg_common::project::CaptionStyle;
use neoveg_video::frame::FrameBuffer;
use neoveg_video::render::{RenderEngine, RenderFormat};

#[derive(Debug, Clone)]
pub struct RenderJobStatus {
    pub phase: String,
    pub current_frame: u64,
    pub total_frames: u64,
    pub started_at: Instant,
    pub finished: bool,
    pub error: Option<String>,
    pub output_path: PathBuf,
    pub loudness_path: Option<PathBuf>,
    pub loudness_summary: Option<LoudnessSnapshot>,
}

pub struct ActiveRenderJob {
    pub status: Arc<Mutex<RenderJobStatus>>,
    pub cancel: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
pub struct RenderLaunchRequest {
    pub output_path: PathBuf,
    pub format: RenderFormat,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub crf: u8,
    pub engine: RenderEngine,
    /// Draw the captions of the project into the picture.
    pub burn_captions: bool,
    pub loop_region_only: bool,
    pub save_loudness_log: bool,
}

pub struct RenderDialogState {
    pub open: bool,
    pub format: RenderFormat,
    pub custom_resolution: bool,
    pub width: u32,
    pub height: u32,
    pub crf: u8,
    pub engine: RenderEngine,
    pub burn_captions: bool,
    pub loop_region_only: bool,
    pub save_loudness_log: bool,
    pub active_job: Option<ActiveRenderJob>,
    /// Families and their styles, read from the text engine when the style controls first open.
    font_list: Vec<(String, Vec<String>)>,
    /// The style the preview was drawn for, and the picture.
    preview_for: Option<CaptionStyle>,
    preview: Option<egui::TextureHandle>,
}

impl Default for RenderDialogState {
    fn default() -> Self {
        Self {
            open: false,
            format: RenderFormat::Mp4H264,
            custom_resolution: false,
            width: 1920,
            height: 1080,
            crf: 18,
            engine: RenderEngine::Auto,
            burn_captions: false,
            loop_region_only: false,
            save_loudness_log: true,
            active_job: None,
            font_list: Vec::new(),
            preview_for: None,
            preview: None,
        }
    }
}

impl RenderDialogState {
    pub fn is_running(&self) -> bool {
        self.active_job
            .as_ref()
            .and_then(|j| j.status.lock().ok())
            .is_some_and(|s| !s.finished)
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        project_width: u32,
        project_height: u32,
        project_fps: f64,
        has_loop_region: bool,
        timeline_duration: f64,
        caption_style: &mut CaptionStyle,
    ) -> Option<RenderLaunchRequest> {
        if !self.open {
            return None;
        }
        if !self.custom_resolution {
            self.width = project_width.max(2) & !1;
            self.height = project_height.max(2) & !1;
        }

        let mut open = self.open;
        let mut close_requested = false;
        let mut launch: Option<RenderLaunchRequest> = None;

        crate::dialogs::dialog(ctx, "Renderizar como… (Make Movie)")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(500.0)
            .show(ctx, |ui| {
                let running = self.is_running();
                ui.add_enabled_ui(!running, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Formato de saída:");
                        egui::ComboBox::from_id_salt("render_format_combo")
                            .selected_text(self.format.label())
                            .width(280.0)
                            .show_ui(ui, |ui| {
                                for candidate in RenderFormat::ALL {
                                    ui.selectable_value(
                                        &mut self.format,
                                        candidate,
                                        candidate.label(),
                                    );
                                }
                            });
                    });

                    if self.format.has_video() {
                        ui.horizontal(|ui| {
                            ui.label("Codificador:");
                            egui::ComboBox::from_id_salt("render_engine_combo")
                                .selected_text(self.engine.label())
                                .width(380.0)
                                .show_ui(ui, |ui| {
                                    for candidate in RenderEngine::ALL {
                                        ui.selectable_value(
                                            &mut self.engine,
                                            candidate,
                                            candidate.label(),
                                        );
                                    }
                                });
                        });
                        if !self.engine.can_write(self.format) {
                            ui.colored_label(
                                ui.visuals().warn_fg_color,
                                "O codificador interno só escreve MP4 H.264 + AAC: escolha o FFmpeg para este formato.",
                            );
                        } else if self.engine.resolve(self.format) == RenderEngine::Internal {
                            ui.weak("Codificador interno: H.264 + AAC em Rust, sem o FFmpeg.");
                        } else {
                            neoveg_video::render::probe_encoders_in_background();
                            match neoveg_video::render::cached_encoder_for(self.format) {
                                None => {
                                    ui.weak("Procurando codificadores do ffmpeg…");
                                    ui.ctx().request_repaint_after(
                                        std::time::Duration::from_millis(100),
                                    );
                                }
                                Some(Ok(encoder)) => {
                                    ui.weak(format!("Codificador do FFmpeg: {encoder}"));
                                }
                                Some(Err(why)) => {
                                    ui.colored_label(ui.visuals().warn_fg_color, why);
                                }
                            }
                        }
                        ui.checkbox(
                            &mut self.custom_resolution,
                            "Personalizar resolução de saída",
                        );
                        if self.custom_resolution {
                            ui.horizontal(|ui| {
                                ui.label("Dimensões:");
                                ui.add(
                                    egui::DragValue::new(&mut self.width)
                                        .range(160..=7680)
                                        .speed(2)
                                        .suffix(" px"),
                                );
                                ui.label("×");
                                ui.add(
                                    egui::DragValue::new(&mut self.height)
                                        .range(120..=4320)
                                        .speed(2)
                                        .suffix(" px"),
                                );
                            });
                        } else {
                            ui.weak(format!(
                                "Resolução do projeto: {}×{} @ {:.2} qps",
                                self.width, self.height, project_fps
                            ));
                        }

                        if matches!(
                            self.format,
                            RenderFormat::Mp4H264 | RenderFormat::Mp4Hevc | RenderFormat::WebmVp9
                        ) {
                            ui.add(
                                egui::Slider::new(&mut self.crf, 10..=34)
                                    .text("Qualidade CRF (menor = maior qualidade)"),
                            );
                        }
                        ui.checkbox(
                            &mut self.burn_captions,
                            "Queimar as legendas do projeto no vídeo",
                        );
                        if self.burn_captions {
                            egui::CollapsingHeader::new("Estilo das legendas")
                                .default_open(false)
                                .show(ui, |ui| {
                                    self.caption_style_ui(ui, caption_style);
                                });
                        }
                    }

                    ui.add_enabled(
                        has_loop_region,
                        egui::Checkbox::new(
                            &mut self.loop_region_only,
                            "Renderizar apenas a região de loop / seleção de tempo",
                        ),
                    );
                    if !has_loop_region {
                        self.loop_region_only = false;
                    }

                    ui.checkbox(
                        &mut self.save_loudness_log,
                        "Salvar relatório de loudness ao lado do arquivo (<nome>_loud.txt)",
                    );
                });

                ui.separator();

                if let Some(job) = &self.active_job {
                    if let Ok(st) = job.status.lock() {
                        ui.label(&st.phase);
                        let frac = if st.total_frames > 0 {
                            (st.current_frame as f32 / st.total_frames as f32).clamp(0.0, 1.0)
                        } else if st.finished {
                            1.0
                        } else {
                            0.0
                        };
                        ui.add(
                            egui::ProgressBar::new(frac)
                                .show_percentage()
                                .animate(!st.finished),
                        );
                        let elapsed = st.started_at.elapsed().as_secs_f64();
                        if !st.finished {
                            ctx.request_repaint_after(Duration::from_millis(80));
                            if st.current_frame > 0 && st.total_frames > st.current_frame {
                                let eta = elapsed
                                    * ((st.total_frames - st.current_frame) as f64
                                        / st.current_frame as f64);
                                ui.weak(format!(
                                    "Quadro {} / {} · Decorrido {:.1}s · Restante ~{:.1}s",
                                    st.current_frame, st.total_frames, elapsed, eta
                                ));
                            }
                            if ui.button("Cancelar renderização").clicked() {
                                job.cancel.store(true, Ordering::Relaxed);
                            }
                        } else if let Some(err) = &st.error {
                            ui.colored_label(egui::Color32::from_rgb(242, 100, 92), err);
                        } else {
                            ui.colored_label(
                                egui::Color32::from_rgb(102, 214, 132),
                                format!(
                                    "Concluído em {:.1}s: {}",
                                    elapsed,
                                    st.output_path.display()
                                ),
                            );
                            if let Some(loud) = &st.loudness_path {
                                ui.weak(format!(
                                    "Relatório BS.1770-4 salvo em: {}",
                                    loud.display()
                                ));
                            }
                        }
                    }
                    ui.separator();
                }

                ui.horizontal(|ui| {
                    let can_start = !running
                        && timeline_duration > 1e-3
                        && (!self.format.has_video() || self.engine.can_write(self.format));
                    if ui
                        .add_enabled(
                            can_start,
                            egui::Button::new("Escolher destino e renderizar…"),
                        )
                        .clicked()
                    {
                        let ext = self.format.default_extension();
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Renderizar projeto")
                            .set_file_name(format!("render.{ext}"))
                            .add_filter(self.format.label(), &[ext])
                            .save_file()
                        {
                            launch = Some(RenderLaunchRequest {
                                output_path: path,
                                format: self.format,
                                width: self.width.max(2) & !1,
                                height: self.height.max(2) & !1,
                                fps: project_fps,
                                crf: self.crf,
                                engine: self.engine,
                                burn_captions: self.burn_captions && self.format.has_video(),
                                loop_region_only: self.loop_region_only && has_loop_region,
                                save_loudness_log: self.save_loudness_log,
                            });
                        }
                    }
                    if !running && ui.button("Fechar").clicked() {
                        close_requested = true;
                    }
                });
                if timeline_duration <= 1e-3 {
                    ui.weak("Adicione eventos à timeline antes de renderizar.");
                }
            });

        self.open = open && !close_requested;
        launch
    }
}

impl RenderDialogState {
    /// The controls of the caption style, and a preview of what they give.
    fn caption_style_ui(&mut self, ui: &mut egui::Ui, style: &mut CaptionStyle) {
        if self.font_list.is_empty() {
            self.font_list = neoveg_text::fonts::families();
        }
        ui.horizontal(|ui| {
            ui.label("Fonte:");
            let mut family_pick: Option<String> = None;
            egui::ComboBox::from_id_salt("caption_font_family")
                .selected_text(style.family.clone())
                .show_ui(ui, |ui| {
                    for (family, _) in &self.font_list {
                        if ui
                            .selectable_label(style.family == *family, family)
                            .clicked()
                        {
                            family_pick = Some(family.clone());
                        }
                    }
                });
            if let Some(family) = family_pick {
                style.family = family;
                let styles = self
                    .font_list
                    .iter()
                    .find(|(f, _)| *f == style.family)
                    .map(|(_, s)| s.clone())
                    .unwrap_or_default();
                if !styles.contains(&style.style) {
                    style.style = styles
                        .iter()
                        .find(|s| s.as_str() == "Bold")
                        .or(styles.first())
                        .cloned()
                        .unwrap_or_else(|| "Regular".into());
                }
            }
            let styles = self
                .font_list
                .iter()
                .find(|(f, _)| *f == style.family)
                .map(|(_, s)| s.clone())
                .unwrap_or_default();
            egui::ComboBox::from_id_salt("caption_font_style")
                .selected_text(style.style.clone())
                .show_ui(ui, |ui| {
                    for s in &styles {
                        ui.selectable_value(&mut style.style, s.clone(), s);
                    }
                });
            if ui
                .button("Fontes do sistema")
                .on_hover_text(
                    "Procura as fontes instaladas no computador (pode levar um instante)",
                )
                .clicked()
            {
                neoveg_text::fonts::scan_system();
                self.font_list = neoveg_text::fonts::families();
            }
        });
        let mut size = style.size * 100.0;
        if ui
            .add(egui::Slider::new(&mut size, 2.0..=15.0).text("Tamanho (% da altura)"))
            .changed()
        {
            style.size = size / 100.0;
        }
        let mut outline = style.outline * 100.0;
        if ui
            .add(egui::Slider::new(&mut outline, 0.0..=3.0).text("Contorno (% da altura)"))
            .changed()
        {
            style.outline = outline / 100.0;
        }
        let mut margin = style.margin * 100.0;
        if ui
            .add(egui::Slider::new(&mut margin, 0.0..=40.0).text("Distância da base (% da altura)"))
            .changed()
        {
            style.margin = margin / 100.0;
        }
        let mut width = style.box_width * 100.0;
        if ui
            .add(
                egui::Slider::new(&mut width, 30.0..=100.0).text("Largura do texto (% da largura)"),
            )
            .changed()
        {
            style.box_width = width / 100.0;
        }
        ui.horizontal(|ui| {
            for (label, rgba) in [
                ("Texto:", &mut style.text_rgba),
                ("Contorno:", &mut style.outline_rgba),
            ] {
                ui.label(label);
                let mut color =
                    egui::Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]);
                if ui.color_edit_button_srgba(&mut color).changed() {
                    *rgba = color.to_srgba_unmultiplied();
                }
            }
            if ui.button("Restaurar o padrão").clicked() {
                *style = CaptionStyle::default();
            }
        });
        // A preview: the text on a dark picture, redrawn when the style changes
        if self.preview_for.as_ref() != Some(&*style) {
            let mut frame = FrameBuffer::solid(320, 180, [24, 32, 48, 255]);
            neoveg_video::burn::burn_subtitle(
                &mut frame,
                "Exemplo de legenda\nem duas linhas",
                &neoveg_video::burn::SubtitleStyle::from(&*style),
            );
            let image = egui::ColorImage::from_rgba_unmultiplied([320, 180], &frame.rgba);
            match &mut self.preview {
                Some(texture) => texture.set(image, egui::TextureOptions::LINEAR),
                None => {
                    self.preview = Some(ui.ctx().load_texture(
                        "caption_style_preview",
                        image,
                        egui::TextureOptions::LINEAR,
                    ));
                }
            }
            self.preview_for = Some(style.clone());
        }
        if let Some(texture) = &self.preview {
            ui.image((texture.id(), egui::vec2(320.0, 180.0)));
        }
    }
}
