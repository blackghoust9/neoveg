//! The drawing of audio buses and sends: the strip of one bus (name, fader, pan, mute, phase, where
//! it goes, delete) and the routing of one audio track (its output and its sends). Nothing here
//! changes the project: every control returns an action and `AppState` turns it into a command.

use eframe::egui;
use neoveg_engine::buses::{self, MAX_SEND_DB, MIN_SEND_DB};
use neoveg_engine::{AudioBus, BusId, BusOutput, Timeline, Track, TrackSend};

/// What the user did on the strip of a bus.
#[derive(Debug, Clone, PartialEq)]
pub enum BusAction {
    Rename(BusId, String),
    /// A fader or pan being dragged: the sound follows, the project does not change yet.
    Draft(BusId, f32, f32),
    /// The fader and the pan, as they ended up (one undo step).
    Mix(BusId, f32, f32),
    Mute(BusId, bool),
    Phase(BusId, bool),
    Output(BusId, BusOutput),
    Remove(BusId),
}

/// What the user did on the routing of a track.
#[derive(Debug, Clone, PartialEq)]
pub enum RouteAction {
    Output(BusOutput),
    AddSend(BusId),
    RemoveSend(BusId),
    /// A send level being dragged.
    SendDraft(BusId, f32),
    SendLevel(BusId, f32),
    SendPost(BusId, bool),
    SendLinkPan(BusId, bool),
}

/// The name of an output for a menu.
fn output_label(timeline: &Timeline, output: BusOutput) -> String {
    match output {
        BusOutput::Master => "Master".into(),
        BusOutput::Bus(id) => buses::label(timeline, id),
    }
}

/// A slider that keeps the value being dragged between frames (the project only has the old value
/// until the drag ends). Returns the value shown and what happened: `(value, dragging, ended)`.
fn draft_slider(
    ui: &mut egui::Ui,
    key: egui::Id,
    current: f32,
    range: std::ops::RangeInclusive<f32>,
    suffix: &str,
) -> (f32, bool, bool) {
    let mut value = ui.data(|d| d.get_temp::<f32>(key)).unwrap_or(current);
    let response = ui.add(egui::Slider::new(&mut value, range).suffix(suffix));
    let mut dragging = false;
    let mut ended = false;
    if response.changed() {
        ui.data_mut(|d| d.insert_temp(key, value));
        dragging = true;
    }
    if response.drag_stopped()
        || (response.lost_focus() && ui.data(|d| d.get_temp::<f32>(key).is_some()))
    {
        ui.data_mut(|d| d.remove::<f32>(key));
        ended = true;
    }
    (value, dragging, ended)
}

/// The strip of one bus: what it is called, its fader and pan, mute and phase, and where it goes.
pub fn bus_strip(ui: &mut egui::Ui, timeline: &Timeline, bus: &AudioBus) -> Vec<BusAction> {
    let mut out = Vec::new();
    let id = bus.id;
    ui.horizontal(|ui| {
        ui.label("Nome");
        let mut name = bus.name.clone();
        // The default name is the letter of the bus's place in the list
        let index = timeline.buses.iter().position(|b| b.id == id).unwrap_or(0);
        ui.add(
            egui::TextEdit::singleline(&mut name)
                .hint_text(format!("Bus {}", buses::letter(index)))
                .desired_width(140.0),
        )
        .on_hover_text("Deixe em branco para voltar ao nome padrão (a letra do barramento)");
        if name != bus.name {
            out.push(BusAction::Rename(id, name));
        }
        if ui.button("Excluir barramento").clicked() {
            out.push(BusAction::Remove(id));
        }
    });
    ui.horizontal(|ui| {
        if ui.selectable_label(bus.mix.muted, "Mudo").clicked() {
            out.push(BusAction::Mute(id, !bus.mix.muted));
        }
        if ui.selectable_label(bus.mix.invert_phase, "Fase").clicked() {
            out.push(BusAction::Phase(id, !bus.mix.invert_phase));
        }
    });
    let key = ui.id().with(("bus_mix", id));
    ui.horizontal(|ui| {
        ui.allocate_ui(egui::vec2(52.0, 18.0), |ui| ui.label("Volume"));
        let (db, dragging, ended) =
            draft_slider(ui, key.with("vol"), bus.mix.volume_db, -60.0..=12.0, " dB");
        let pan = ui
            .data(|d| d.get_temp::<f32>(key.with("pan")))
            .unwrap_or(bus.mix.pan);
        if dragging {
            out.push(BusAction::Draft(id, db, pan));
        }
        if ended {
            out.push(BusAction::Mix(id, db, pan));
        }
    });
    ui.horizontal(|ui| {
        ui.allocate_ui(egui::vec2(52.0, 18.0), |ui| ui.label("Pan"));
        let (pan, dragging, ended) = draft_slider(ui, key.with("pan"), bus.mix.pan, -1.0..=1.0, "");
        let db = ui
            .data(|d| d.get_temp::<f32>(key.with("vol")))
            .unwrap_or(bus.mix.volume_db);
        if dragging {
            out.push(BusAction::Draft(id, db, pan));
        }
        if ended {
            out.push(BusAction::Mix(id, db, pan));
        }
    });
    ui.horizontal(|ui| {
        ui.label("Saída");
        egui::ComboBox::from_id_salt(("bus_output", id))
            .selected_text(output_label(timeline, bus.output))
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(bus.output == BusOutput::Master, "Master")
                    .clicked()
                {
                    out.push(BusAction::Output(id, BusOutput::Master));
                }
                for other in &timeline.buses {
                    // A bus never goes to itself or to one that leads back to it
                    let allowed = buses::can_route_bus(timeline, id, BusOutput::Bus(other.id));
                    let label = buses::label(timeline, other.id);
                    if ui
                        .add_enabled(
                            allowed,
                            egui::SelectableLabel::new(
                                bus.output == BusOutput::Bus(other.id),
                                label,
                            ),
                        )
                        .clicked()
                    {
                        out.push(BusAction::Output(id, BusOutput::Bus(other.id)));
                    }
                }
            });
    });
    out
}

/// Where an audio track goes and its sends.
pub fn route_ui(ui: &mut egui::Ui, timeline: &Timeline, track: &Track) -> Vec<RouteAction> {
    let mut out = Vec::new();
    if timeline.buses.is_empty() {
        ui.weak("Sem barramentos: Faixas > Inserir barramento de áudio cria um.");
        return out;
    }
    ui.horizontal(|ui| {
        ui.label("Saída");
        egui::ComboBox::from_id_salt(("track_output", track.id))
            .selected_text(output_label(timeline, track.output))
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(track.output == BusOutput::Master, "Master")
                    .clicked()
                {
                    out.push(RouteAction::Output(BusOutput::Master));
                }
                for bus in &timeline.buses {
                    if ui
                        .selectable_label(
                            track.output == BusOutput::Bus(bus.id),
                            buses::label(timeline, bus.id),
                        )
                        .clicked()
                    {
                        out.push(RouteAction::Output(BusOutput::Bus(bus.id)));
                    }
                }
            });
    });
    for send in &track.sends {
        ui.horizontal(|ui| {
            ui.label(format!("Send: {}", buses::label(timeline, send.bus)));
            let key = ui.id().with(("send_level", track.id, send.bus));
            let (db, dragging, ended) =
                draft_slider(ui, key, send.level_db, MIN_SEND_DB..=MAX_SEND_DB, " dB");
            if dragging {
                out.push(RouteAction::SendDraft(send.bus, db));
            }
            if ended {
                out.push(RouteAction::SendLevel(send.bus, db));
            }
            if ui
                .selectable_label(send.post_volume, "Pós")
                .on_hover_text(
                    "Depois do fader da faixa (desligado: antes do fader, independente dele)",
                )
                .clicked()
            {
                out.push(RouteAction::SendPost(send.bus, !send.post_volume));
            }
            if ui
                .selectable_label(send.link_pan, "Pan")
                .on_hover_text("Segue o pan da faixa (desligado: estéreo centralizado)")
                .clicked()
            {
                out.push(RouteAction::SendLinkPan(send.bus, !send.link_pan));
            }
            if ui.button("x").on_hover_text("Remover o send").clicked() {
                out.push(RouteAction::RemoveSend(send.bus));
            }
        });
    }
    let free: Vec<&AudioBus> = timeline
        .buses
        .iter()
        .filter(|b| !track.sends.iter().any(|s| s.bus == b.id))
        .collect();
    if !free.is_empty() {
        egui::ComboBox::from_id_salt(("add_send", track.id))
            .selected_text("Adicionar send…")
            .show_ui(ui, |ui| {
                for bus in free {
                    if ui
                        .selectable_label(false, buses::label(timeline, bus.id))
                        .clicked()
                    {
                        out.push(RouteAction::AddSend(bus.id));
                    }
                }
            });
    }
    out
}

/// A new send to `bus` as the interface adds it: unity level, before the fader.
pub fn new_send(bus: BusId) -> TrackSend {
    TrackSend {
        bus,
        level_db: 0.0,
        post_volume: false,
        link_pan: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_send_is_at_unity_before_the_fader_with_a_centred_signal() {
        let send = new_send(7);
        assert_eq!(send.bus, 7);
        assert_eq!(send.level_db, 0.0);
        assert!(!send.post_volume && !send.link_pan);
    }

    #[test]
    fn the_send_range_is_the_one_the_engine_clamps_to() {
        assert_eq!(MIN_SEND_DB, -60.0);
        assert_eq!(MAX_SEND_DB, 12.0);
    }
}
