//! Track groups window (Exibir > Grupos de faixas): create a group from ticked tracks, name it,
//! colour it, mute or solo it as one, and give single tracks a colour. The rules are in
//! `neoveg_engine::track_groups`; the window reports [`GroupAction`]s.

use std::collections::BTreeSet;

use eframe::egui;
use neoveg_common::i18n::tr;
use neoveg_engine::timeline::{Timeline, TrackId};
use neoveg_engine::track_groups::{GroupSwitch, TrackGroupId, TRACK_COLORS};

#[derive(Debug, Clone, PartialEq)]
pub enum GroupAction {
    Create { name: String, tracks: Vec<TrackId> },
    Rename(TrackGroupId, String),
    Ungroup(TrackGroupId),
    Collapse(TrackGroupId, bool),
    GroupColor(TrackGroupId, Option<[u8; 3]>),
    Mute(TrackGroupId, bool),
    Solo(TrackGroupId, bool),
    TrackColor(TrackId, Option<[u8; 3]>),
}

#[derive(Default)]
pub struct TrackGroupsWindow {
    pub open: bool,
    name: String,
    picked: BTreeSet<TrackId>,
}

fn swatch_row(ui: &mut egui::Ui, current: Option<[u8; 3]>) -> Option<Option<[u8; 3]>> {
    let mut chosen = None;
    if ui
        .selectable_label(current.is_none(), tr("Sem cor"))
        .clicked()
    {
        chosen = Some(None);
    }
    for rgb in TRACK_COLORS {
        let color = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
        let (rect, response) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
        ui.painter().rect_filled(rect, 2.0, color);
        if current == Some(rgb) {
            ui.painter().rect_stroke(
                rect,
                2.0,
                egui::Stroke::new(2.0_f32, ui.visuals().text_color()),
            );
        }
        if response.clicked() {
            chosen = Some(Some(rgb));
        }
    }
    chosen
}

impl TrackGroupsWindow {
    pub fn show(&mut self, ctx: &egui::Context, timeline: &Timeline) -> Vec<GroupAction> {
        let mut actions = Vec::new();
        if !self.open {
            return actions;
        }
        let mut open = self.open;
        let groups = &timeline.track_groups;
        crate::dialogs::dialog(ctx, tr("Grupos de faixas"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(420.0)
            .show(ctx, |ui| {
                ui.label(tr("Novo grupo: marque as faixas e dê um nome."));
                egui::ScrollArea::vertical()
                    .id_salt("track_groups_pick")
                    .max_height(140.0)
                    .show(ui, |ui| {
                        for (i, track) in timeline.tracks.iter().enumerate() {
                            let mut on = self.picked.contains(&track.id);
                            let tag = match groups.group_of(track.id).and_then(|g| groups.group(g))
                            {
                                Some(g) => format!("{}  [{}]", track.name, g.name),
                                None => track.name.clone(),
                            };
                            ui.horizontal(|ui| {
                                if ui.checkbox(&mut on, format!("{}  {tag}", i + 1)).changed() {
                                    if on {
                                        self.picked.insert(track.id);
                                    } else {
                                        self.picked.remove(&track.id);
                                    }
                                }
                                if let Some(rgb) = groups.color_of(track.id) {
                                    let (r, _) = ui.allocate_exact_size(
                                        egui::vec2(10.0, 10.0),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().rect_filled(
                                        r,
                                        2.0,
                                        egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]),
                                    );
                                }
                            });
                        }
                    });
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.name)
                            .hint_text(tr("Nome do grupo"))
                            .desired_width(180.0),
                    );
                    if ui
                        .add_enabled(
                            !self.picked.is_empty(),
                            egui::Button::new(tr("Criar grupo")),
                        )
                        .clicked()
                    {
                        actions.push(GroupAction::Create {
                            name: self.name.clone(),
                            tracks: timeline
                                .tracks
                                .iter()
                                .map(|t| t.id)
                                .filter(|id| self.picked.contains(id))
                                .collect(),
                        });
                        self.picked.clear();
                        self.name.clear();
                    }
                });
                if !self.picked.is_empty() {
                    ui.horizontal(|ui| {
                        ui.label(tr("Cor das faixas marcadas:"));
                        if let Some(color) = swatch_row(ui, None) {
                            for id in &self.picked {
                                actions.push(GroupAction::TrackColor(*id, color));
                            }
                        }
                    });
                }
                ui.separator();
                if groups.groups().is_empty() {
                    ui.weak(tr("Nenhum grupo."));
                }
                for group in groups.groups() {
                    ui.push_id(group.id, |ui| {
                        ui.horizontal(|ui| {
                            let mut name = group.name.clone();
                            let edit =
                                ui.add(egui::TextEdit::singleline(&mut name).desired_width(140.0));
                            if edit.lost_focus() && name != group.name {
                                actions.push(GroupAction::Rename(group.id, name));
                            }
                            let mut collapsed = group.collapsed;
                            if ui.checkbox(&mut collapsed, tr("Recolhido")).changed() {
                                actions.push(GroupAction::Collapse(group.id, collapsed));
                            }
                            let muted = groups.mute_state(timeline, group.id);
                            let solo = groups.solo_state(timeline, group.id);
                            if ui
                                .selectable_label(muted != GroupSwitch::None, tr("Mudo"))
                                .on_hover_text(tr(
                                    "Liga ou desliga o mudo de todas as faixas do grupo",
                                ))
                                .clicked()
                            {
                                actions
                                    .push(GroupAction::Mute(group.id, muted != GroupSwitch::All));
                            }
                            if ui
                                .selectable_label(solo != GroupSwitch::None, tr("Solo"))
                                .on_hover_text(tr(
                                    "Liga ou desliga o solo de todas as faixas de áudio do grupo",
                                ))
                                .clicked()
                            {
                                actions.push(GroupAction::Solo(group.id, solo != GroupSwitch::All));
                            }
                            if ui.button(tr("Desfazer grupo")).clicked() {
                                actions.push(GroupAction::Ungroup(group.id));
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label(tr("Cor:"));
                            if let Some(color) = swatch_row(ui, group.color) {
                                actions.push(GroupAction::GroupColor(group.id, color));
                            }
                        });
                    });
                    ui.separator();
                }
            });
        self.open = open;
        actions
    }
}
