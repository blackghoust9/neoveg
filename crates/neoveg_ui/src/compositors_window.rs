//! Faixas > Painel de compositores: the compositing mode of each video track, that is how its
//! picture blends over the tracks below it (Source Alpha, Add, Multiply, ... and the three map
//! modes), with what each mode does to the pixels.
//!
//! The window only reports [`CompositorAction`]s; [`command_for`] turns one into the undoable edit,
//! so the logic is tested without a window.

use eframe::egui;
use neoveg_common::composite::CompositeMode;
use neoveg_common::i18n::tr;
use neoveg_engine::command::EditCommand;
use neoveg_engine::compositing;
use neoveg_engine::timeline::{Timeline, TrackId, TrackKind};
use neoveg_engine::track_tree;

#[derive(Debug, Clone, PartialEq)]
pub enum CompositorAction {
    /// This track blends with this mode.
    Set(TrackId, CompositeMode),
    /// Every video track blends with this mode.
    SetAll(CompositeMode),
    /// Move the track one level into the compositing tree (`true`: it becomes a child of the track
    /// above) or out of it (`false`).
    Nest(TrackId, bool),
}

#[derive(Debug, Clone, Default)]
pub struct CompositorsState {
    pub open: bool,
}

/// What a mode does, in terms of `dst` (what is already composed below) and `src` (this track), per
/// colour channel from 0 to 1. The map modes have no one-line formula and say it in words.
pub fn formula(mode: CompositeMode) -> &'static str {
    match mode {
        CompositeMode::SourceAlpha => "src over dst, by the alpha of src",
        CompositeMode::Add => "min(1, dst + src)",
        CompositeMode::Subtract => "max(0, dst - src)",
        CompositeMode::Multiply => "dst * src * alpha(src), and alpha(dst) * alpha(src)",
        CompositeMode::Screen => "1 - (1 - dst) * (1 - src)",
        CompositeMode::Overlay => "dst < 0.5: 2 * dst * src, else 1 - 2 * (1 - dst) * (1 - src)",
        CompositeMode::HardLight => "src < 0.5: 2 * dst * src, else 1 - 2 * (1 - dst) * (1 - src)",
        CompositeMode::Dodge => "min(1, dst / (1 - src))",
        CompositeMode::Burn => "max(0, 1 - (1 - dst) / src)",
        CompositeMode::Darken => "min(dst, src)",
        CompositeMode::Lighten => "max(dst, src)",
        CompositeMode::Difference => "abs(dst - src)",
        CompositeMode::DifferenceSquared => "(dst - src) ^ 2",
        CompositeMode::Cut => "src replaces dst, transparency included",
        CompositeMode::DisplacementMap => "",
        CompositeMode::HeightMap => "",
        CompositeMode::BumpMap => "",
    }
}

/// What the map modes do, in words (the others show their [`formula`]).
fn map_note(mode: CompositeMode) -> Option<&'static str> {
    match mode {
        CompositeMode::DisplacementMap => Some(tr(
            "O vermelho e o verde desta faixa deslocam os pixels das faixas de baixo",
        )),
        CompositeMode::HeightMap => Some(tr(
            "A luminosidade desta faixa vira relevo: refrata e ilumina as faixas de baixo",
        )),
        CompositeMode::BumpMap => Some(tr(
            "A luminosidade desta faixa vira relevo: sombreia as faixas de baixo como em alto-relevo",
        )),
        _ => None,
    }
}

/// The text under a mode: its formula, or the sentence about the map modes.
pub fn describe(mode: CompositeMode) -> &'static str {
    map_note(mode).unwrap_or_else(|| formula(mode))
}

/// The edit an action asks for; `None` when it changes nothing.
pub fn command_for(timeline: &Timeline, action: &CompositorAction) -> Option<EditCommand> {
    match action {
        CompositorAction::Set(track, mode) => compositing::set_mode(timeline, *track, *mode),
        CompositorAction::SetAll(mode) => compositing::set_mode_all(timeline, *mode),
        CompositorAction::Nest(track, true) => track_tree::make_child(timeline, *track, false),
        CompositorAction::Nest(track, false) => track_tree::make_parent(timeline, *track, false),
    }
}

fn mode_combo(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    current: CompositeMode,
) -> Option<CompositeMode> {
    let mut chosen = None;
    egui::ComboBox::from_id_salt(id)
        .selected_text(current.label())
        .width(220.0)
        .show_ui(ui, |ui| {
            for mode in CompositeMode::ALL {
                if ui
                    .selectable_label(mode == current, mode.label())
                    .on_hover_text(describe(mode))
                    .clicked()
                {
                    chosen = Some(mode);
                }
            }
        });
    chosen
}

impl CompositorsState {
    /// Draws the window. `selected` is the track selected on the timeline, shown in bold.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        timeline: &Timeline,
        selected: Option<TrackId>,
    ) -> Vec<CompositorAction> {
        let mut actions = Vec::new();
        if !self.open {
            return actions;
        }
        let mut open = self.open;
        crate::dialogs::dialog(ctx, tr("Painel de compositores"))
            .collapsible(false)
            .open(&mut open)
            .show(ctx, |ui| {
                let video: Vec<_> = timeline
                    .tracks
                    .iter()
                    .filter(|t| t.kind == TrackKind::Video)
                    .collect();
                if video.is_empty() {
                    ui.label(tr("Não há faixas de vídeo."));
                    return;
                }
                ui.weak(tr(
                    "Como a imagem de cada faixa se mistura com as faixas de baixo",
                ));
                let levels = track_tree::effective_levels(&timeline.tracks);
                egui::Grid::new("compositors_grid")
                    .num_columns(3)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        for track in &video {
                            let level = timeline
                                .tracks
                                .iter()
                                .position(|t| t.id == track.id)
                                .and_then(|i| levels.get(i))
                                .copied()
                                .unwrap_or(0);
                            let name = egui::RichText::new(format!(
                                "{}{}",
                                "    ".repeat(usize::from(level)),
                                track.name
                            ));
                            ui.label(if Some(track.id) == selected {
                                name.strong()
                            } else {
                                name
                            });
                            if track.adjustment {
                                ui.weak(tr("Faixa de ajuste: sem camada própria"));
                            } else if let Some(mode) =
                                mode_combo(ui, ("compositor", track.id), track.composite_mode)
                            {
                                actions.push(CompositorAction::Set(track.id, mode));
                            }
                            ui.horizontal(|ui| {
                                if ui
                                    .add_enabled(
                                        track_tree::can_make_child(timeline, track.id),
                                        egui::Button::new("→").small(),
                                    )
                                    .on_hover_text(tr("Tornar a faixa filha de composição"))
                                    .clicked()
                                {
                                    actions.push(CompositorAction::Nest(track.id, true));
                                }
                                if ui
                                    .add_enabled(
                                        track_tree::can_make_parent(timeline, track.id),
                                        egui::Button::new("←").small(),
                                    )
                                    .on_hover_text(tr("Tornar a faixa mãe de composição"))
                                    .clicked()
                                {
                                    actions.push(CompositorAction::Nest(track.id, false));
                                }
                            });
                            ui.end_row();
                        }
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(tr("Todas as faixas de vídeo:"));
                    let current = video
                        .iter()
                        .find(|t| !t.adjustment)
                        .map_or(CompositeMode::SourceAlpha, |t| t.composite_mode);
                    if let Some(mode) = mode_combo(ui, "compositor_all", current) {
                        actions.push(CompositorAction::SetAll(mode));
                    }
                });
                ui.separator();
                egui::CollapsingHeader::new(tr("O que cada modo faz"))
                    .default_open(false)
                    .show(ui, |ui| {
                        for mode in CompositeMode::ALL {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(egui::RichText::new(mode.label()).strong());
                                ui.label(egui::RichText::new(describe(mode)).monospace());
                            });
                        }
                    });
            });
        self.open = open;
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timeline() -> (Timeline, TrackId, TrackId) {
        let mut tl = Timeline::new();
        let video = tl.new_track(TrackKind::Video);
        let audio = tl.new_track(TrackKind::Audio);
        let (v, a) = (video.id, audio.id);
        tl.tracks.push(video);
        tl.tracks.push(audio);
        (tl, v, a)
    }

    #[test]
    fn the_arrows_of_the_panel_nest_a_track_under_the_one_above_and_back() {
        let mut tl = Timeline::new();
        let top = tl.new_track(TrackKind::Video);
        let below = tl.new_track(TrackKind::Video);
        let (a, b) = (top.id, below.id);
        tl.tracks.push(top);
        tl.tracks.push(below);
        // The first track has nothing above to be a child of, and the top level has nowhere out
        assert!(command_for(&tl, &CompositorAction::Nest(a, true)).is_none());
        assert!(command_for(&tl, &CompositorAction::Nest(b, false)).is_none());
        let command = command_for(&tl, &CompositorAction::Nest(b, true)).unwrap();
        let mut history = neoveg_engine::command::History::new();
        assert!(history.execute(&mut tl, command));
        assert_eq!(track_tree::parent_of(&tl, b), Some(a));
        let back = command_for(&tl, &CompositorAction::Nest(b, false)).unwrap();
        assert!(history.execute(&mut tl, back));
        assert_eq!(track_tree::parent_of(&tl, b), None);
    }

    #[test]
    fn every_mode_says_what_it_does() {
        for mode in CompositeMode::ALL {
            assert!(!describe(mode).is_empty(), "{mode:?}");
            // The three map modes use a sentence, the others a formula over dst and src
            assert_eq!(map_note(mode).is_some(), mode.is_spatial(), "{mode:?}");
            if !mode.is_spatial() {
                assert!(formula(mode).contains("src"), "{mode:?}");
            }
        }
    }

    #[test]
    fn an_action_becomes_the_edit_and_a_no_op_becomes_nothing() {
        let (tl, video, audio) = timeline();
        let set = CompositorAction::Set(video, CompositeMode::Overlay);
        assert!(matches!(
            command_for(&tl, &set),
            Some(EditCommand::SetCompositeMode {
                after: CompositeMode::Overlay,
                ..
            })
        ));
        assert!(command_for(
            &tl,
            &CompositorAction::Set(video, CompositeMode::SourceAlpha)
        )
        .is_none());
        assert!(command_for(&tl, &CompositorAction::Set(audio, CompositeMode::Add)).is_none());
        assert!(command_for(&tl, &CompositorAction::SetAll(CompositeMode::Add)).is_some());
        assert!(command_for(&tl, &CompositorAction::SetAll(CompositeMode::SourceAlpha)).is_none());
    }
}
