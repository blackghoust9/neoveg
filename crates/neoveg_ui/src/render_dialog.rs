//! "Renderizar como… (*Make Movie*)" dialog and non-blocking background render job state.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui;
use neoveg_audio::readout::LoudnessSnapshot;
use neoveg_video::render::RenderFormat;

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
    pub loop_region_only: bool,
    pub save_loudness_log: bool,
    pub active_job: Option<ActiveRenderJob>,
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
            loop_region_only: false,
            save_loudness_log: true,
            active_job: None,
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
                        neoveg_video::render::probe_encoders_in_background();
                        match neoveg_video::render::cached_encoder_for(self.format) {
                            None => {
                                ui.weak("Procurando codificadores do ffmpeg…");
                                ui.ctx()
                                    .request_repaint_after(std::time::Duration::from_millis(100));
                            }
                            Some(Ok(encoder)) => {
                                ui.weak(format!("Codificador: {encoder}"));
                            }
                            Some(Err(why)) => {
                                ui.colored_label(ui.visuals().warn_fg_color, why);
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
                    let can_start = !running && timeline_duration > 1e-3;
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
