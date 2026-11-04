//! The trim monitor: the last frame of the event that ends at a cut next to the first frame of the
//! event that starts there, with buttons that move the cut by one or five frames. It only draws
//! and reports what the user asked for ([`TrimAction`]); finding the frames and the edits are in
//! `app/trim_monitor.rs` and `neoveg_engine::trim_monitor`.

use eframe::egui;
use neoveg_common::i18n::tr;
use neoveg_engine::timeline::{EventId, TrackId};
use neoveg_engine::trim_monitor::TrimMode;

/// What the user did in the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrimAction {
    /// Move the cut (or the edge of the mode) by this many frames.
    Nudge(i32),
    /// Take the cut under the cursor of the timeline as the one to trim.
    Retarget,
}

/// What the window shows about the cut.
pub struct TrimView {
    pub left_name: String,
    pub right_name: String,
    /// Timecode of the last frame of the first event / of the first frame of the second.
    pub left_time: String,
    pub right_time: String,
    /// The edges apart (`+3` frames of gap, `-2` of overlap), or `None` when they meet.
    pub gap_frames: Option<i64>,
    /// Width over height of the pictures.
    pub aspect: f32,
}

#[derive(Default)]
pub struct TrimMonitorState {
    pub open: bool,
    pub mode: TrimMode,
    /// The cut being trimmed: the track and the two events.
    pub pair: Option<(TrackId, EventId, EventId)>,
    pub left: Option<egui::TextureHandle>,
    pub right: Option<egui::TextureHandle>,
    /// What the pictures on show were made for (see `app/trim_monitor.rs`).
    pub shown_key: Option<u64>,
    /// What was last asked of the worker.
    pub requested_key: Option<u64>,
}

/// One side of the cut: the title, the picture (black when there is none), the name and the time.
fn picture(
    ui: &mut egui::Ui,
    title: &str,
    (name, time): (&str, &str),
    texture: Option<&egui::TextureHandle>,
    size: egui::Vec2,
) {
    ui.strong(title);
    match texture {
        Some(texture) => {
            ui.add(egui::Image::new((texture.id(), size)));
        }
        None => {
            let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
            ui.painter()
                .rect_filled(rect, egui::Rounding::ZERO, egui::Color32::BLACK);
        }
    }
    ui.label(name);
    ui.monospace(time);
}

impl TrimMonitorState {
    /// Draws the window; `view` is `None` while there is no cut to show.
    pub fn show(&mut self, ctx: &egui::Context, view: Option<&TrimView>) -> Vec<TrimAction> {
        let mut actions = Vec::new();
        if !self.open {
            return actions;
        }
        let mut open = true;
        crate::dialogs::tool_window(ctx, tr("Monitor de aparo"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(560.0)
            .show(ctx, |ui| {
                let Some(view) = view else {
                    ui.label(tr(
                        "Ponha o cursor num corte entre dois eventos de vídeo, ou selecione os dois",
                    ));
                    if ui.button(tr("Usar o corte no cursor")).clicked() {
                        actions.push(TrimAction::Retarget);
                    }
                    return;
                };
                let column = ((ui.available_width() - 16.0) / 2.0).clamp(120.0, 420.0);
                let size = egui::vec2(column, column / view.aspect.max(0.1));
                ui.columns(2, |cols| {
                    picture(
                        &mut cols[0],
                        tr("Saída (último quadro)"),
                        (&view.left_name, &view.left_time),
                        self.left.as_ref(),
                        size,
                    );
                    picture(
                        &mut cols[1],
                        tr("Entrada (primeiro quadro)"),
                        (&view.right_name, &view.right_time),
                        self.right.as_ref(),
                        size,
                    );
                });
                if let Some(gap) = view.gap_frames {
                    let text = if gap > 0 {
                        format!("{} {gap}", tr("Vão entre os eventos (quadros):"))
                    } else {
                        format!("{} {}", tr("Sobreposição (quadros):"), -gap)
                    };
                    ui.weak(text);
                }
                ui.separator();
                ui.horizontal(|ui| {
                    for mode in TrimMode::ALL {
                        ui.selectable_value(&mut self.mode, mode, tr(mode.label()));
                    }
                });
                ui.horizontal(|ui| {
                    for frames in [-5, -1, 1, 5] {
                        let label = if frames < 0 {
                            format!("◀ {}", -frames)
                        } else {
                            format!("{frames} ▶")
                        };
                        if ui
                            .button(label)
                            .on_hover_text(tr("Move o corte esta quantidade de quadros"))
                            .clicked()
                        {
                            actions.push(TrimAction::Nudge(frames));
                        }
                    }
                    ui.separator();
                    if ui.button(tr("Usar o corte no cursor")).clicked() {
                        actions.push(TrimAction::Retarget);
                    }
                });
            });
        self.open = open;
        actions
    }
}
