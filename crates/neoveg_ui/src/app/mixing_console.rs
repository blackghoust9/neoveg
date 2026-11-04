//! *Console de mixagem* (`Exibir > Console de mixagem`, `Ctrl+Alt+5`, manual v21 *Mixing Console*):
//! one strip for every audio track, every bus and the master bus side by side in one window. A strip
//! has what the mixer panels have apart from each other: mute, solo, phase, fader and pan, where the
//! track goes and its sends, and the inserts (the effect chain) with a switch for each.
//!
//! Nothing here is new editing: every control is the one of the mixer panel, the bus strip, the
//! routing or the effects panel, and what it asks for goes through the same `apply_*` functions
//! (so the undo steps and the live audition are the ones those panels have).

use neoveg_common::i18n::{tr, tr_text};

use super::*;
use crate::fx_panel::{insert_name, FxAction};

/// Width of a strip, in points.
const STRIP_WIDTH: f32 = 236.0;

impl AppState {
    /// Every frame while the console is open.
    pub(super) fn run_mixing_console(&mut self, ctx: &egui::Context) {
        if !self.mixing_console_open {
            return;
        }
        let mut open = true;
        crate::dialogs::tool_window(ctx, tr("Console de mixagem"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([980.0, 560.0])
            .show(ctx, |ui| self.console_body(ui));
        self.mixing_console_open = open;
    }

    fn console_body(&mut self, ui: &mut egui::Ui) {
        let tracks: Vec<TrackId> = self
            .timeline
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Audio)
            .map(|t| t.id)
            .collect();
        let buses: Vec<neoveg_engine::BusId> = self.timeline.buses.iter().map(|b| b.id).collect();
        if tracks.is_empty() && buses.is_empty() {
            ui.weak(tr("Não há faixas de áudio nem barramentos para mixar."));
        }
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    for id in tracks {
                        self.console_track_strip(ui, id);
                    }
                    for id in buses {
                        self.console_bus_strip(ui, id);
                    }
                    self.console_master_strip(ui);
                });
            });
    }

    /// The frame of a strip: a group with a fixed width and the name on top.
    fn console_frame(
        ui: &mut egui::Ui,
        title: &str,
        selected: bool,
        add: impl FnOnce(&mut egui::Ui),
    ) -> egui::Response {
        let stroke = if selected {
            egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(0x3A, 0x8F, 0xE0))
        } else {
            ui.visuals().widgets.noninteractive.bg_stroke
        };
        egui::Frame::group(ui.style())
            .stroke(stroke)
            .show(ui, |ui| {
                ui.set_width(STRIP_WIDTH);
                ui.vertical(|ui| {
                    ui.strong(title);
                    ui.separator();
                    add(ui);
                });
            })
            .response
    }

    fn console_track_strip(&mut self, ui: &mut egui::Ui, id: TrackId) {
        let Some(track) = self.timeline.track(id).cloned() else {
            return;
        };
        let selected = self.selected_track == Some(id);
        let response = Self::console_frame(ui, &track.name, selected, |ui| {
            let mix_actions = mixer_panel::mixer_ui(
                ui,
                &mut self.mixer,
                &track,
                self.timeline.pan_law,
                self.playhead,
                self.playing,
            );
            // The sound follows a fader that is being dragged
            if let (Some(active), Some(preview)) =
                (&self.active_player, self.mixer.preview_mix(&track))
            {
                if let Some(index) = self.timeline.tracks.iter().position(|t| t.id == id) {
                    active.player.set_track_mix(
                        index,
                        neoveg_audio::fxmixer::TrackMixSpec {
                            gain: preview.gain(),
                            pan: preview.pan,
                            invert: preview.invert_phase,
                        },
                    );
                }
            }
            if self.playing && track.automation_mode.records() {
                if let Some((control, value)) = self.mixer.active_auto_draft(id) {
                    self.record_automation_sample(id, control, value);
                }
            }
            self.apply_mix_actions(id, mix_actions);
            ui.separator();
            let route = crate::buses_panel::route_ui(ui, &self.timeline, &track);
            self.apply_route_actions(id, route);
            ui.separator();
            self.console_inserts(ui, FxTarget::Track(id));
        });
        if response.clicked() {
            self.selected_track = Some(id);
        }
    }

    fn console_bus_strip(&mut self, ui: &mut egui::Ui, id: neoveg_engine::BusId) {
        let Some(bus) = self.timeline.buses.iter().find(|b| b.id == id).cloned() else {
            return;
        };
        let title = neoveg_engine::buses::label(&self.timeline, id);
        let selected = self.fx.bus == Some(id);
        Self::console_frame(ui, &title, selected, |ui| {
            let actions = crate::buses_panel::bus_strip(ui, &self.timeline, &bus);
            self.apply_bus_actions(actions);
            ui.separator();
            self.console_inserts(ui, FxTarget::Bus(id));
        });
    }

    fn console_master_strip(&mut self, ui: &mut egui::Ui) {
        Self::console_frame(ui, tr("Master"), self.fx.master, |ui| {
            let db = 20.0 * self.master_volume.max(1e-6).log10();
            ui.monospace(format!("{db:+.1} dB"));
            ui.add(PrecisionSlider::new(&mut self.master_volume, 0.0..=2.0, 1.0).width(150.0));
            if let Some(active) = &self.active_player {
                active.player.set_volume(self.master_volume);
            }
            ui.separator();
            self.console_inserts(ui, FxTarget::Master);
        });
    }

    /// The inserts of a chain: a switch and the name of each effect, and a button that opens the
    /// effects panel on that chain.
    fn console_inserts(&mut self, ui: &mut egui::Ui, target: FxTarget) {
        let chain = fx::chain(&self.timeline, target)
            .map(<[_]>::to_vec)
            .unwrap_or_default();
        ui.horizontal(|ui| {
            ui.strong(tr("Inserts"));
            if ui
                .small_button(tr("Efeitos…"))
                .on_hover_text(tr("Abre o painel de efeitos de áudio nesta cadeia"))
                .clicked()
            {
                match target {
                    FxTarget::Track(id) => {
                        self.selected_track = Some(id);
                        self.fx.master = false;
                        self.fx.bus = None;
                    }
                    FxTarget::Bus(id) => {
                        self.fx.master = false;
                        self.fx.bus = Some(id);
                    }
                    FxTarget::Master => {
                        self.fx.master = true;
                        self.fx.bus = None;
                    }
                }
                self.window_request = Some(WindowAction::Show(Tab::AudioFx));
            }
        });
        if chain.is_empty() {
            ui.weak(tr("Sem efeitos"));
        }
        for (index, effect) in chain.iter().enumerate() {
            let mut on = !effect.bypass;
            if ui
                .checkbox(&mut on, tr_text(&insert_name(effect)))
                .on_hover_text(tr("Ligar ou desligar o efeito (bypass)"))
                .changed()
            {
                self.apply_fx_actions(target, vec![FxAction::Bypass(index, !on)]);
            }
        }
    }
}
