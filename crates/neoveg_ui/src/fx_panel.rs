//! Audio effects panel: the effect chain of the selected track or of the master bus.
//!
//! The effects come from `neoveg_audio::dsp::effects` (adapted from FilmCraft); the chain itself is
//! stored in the project (`neoveg_engine::effects`). The panel never touches the project: it
//! returns [`FxAction`]s and the application turns each one into one undoable command.

use eframe::egui::{self, Slider};
use neoveg_audio::dsp::effects::{self, EffectInfo};
use neoveg_audio::dsp::{ParamSpec, Unit};
use neoveg_audio::fxmixer::{EffectSpec, FxPlan};
use neoveg_engine::{Timeline, TrackEffect};

use crate::icons::{self, Icon};

/// What the user did in the panel.
#[derive(Debug, Clone, PartialEq)]
pub enum FxAction {
    Add(&'static str),
    Remove(usize),
    Move {
        from: usize,
        to: usize,
    },
    Bypass(usize, bool),
    SetParam {
        index: usize,
        param: String,
        value: f32,
    },
    ResetParam {
        index: usize,
        param: String,
    },
}

/// A slider value that is being dragged: the project changes only when the drag ends.
#[derive(Debug, Clone, PartialEq)]
struct Draft {
    index: usize,
    param: String,
    value: f32,
}

#[derive(Debug, Default)]
pub struct FxPanelState {
    /// Edit the master bus instead of the selected track.
    pub master: bool,
    draft: Option<Draft>,
}

// ---- pure helpers -------------------------------------------------------------------------------

/// The effects offered for a chain: the master bus also gets the ones that delay the signal.
pub fn available_effects(master: bool) -> Vec<&'static EffectInfo> {
    effects::effects()
        .iter()
        .filter(|e| master || !e.has_latency)
        .collect()
}

/// Value of `spec` in `effect`: what the user stored (kept inside the range) or the default.
pub fn effective_value(effect: &TrackEffect, spec: &ParamSpec) -> f32 {
    effect
        .params
        .get(spec.id)
        .map_or(spec.default, |v| spec.sanitize(*v))
}

/// A parameter value as shown next to its slider.
pub fn format_value(spec: &ParamSpec, value: f32) -> String {
    match spec.unit {
        Unit::Decibels => format!("{value:.1} dB"),
        Unit::Hertz if value >= 1000.0 => format!("{:.2} kHz", value / 1000.0),
        Unit::Hertz => format!("{value:.0} Hz"),
        Unit::Milliseconds if value >= 100.0 => format!("{value:.0} ms"),
        Unit::Milliseconds => format!("{value:.1} ms"),
        Unit::Seconds => format!("{value:.2} s"),
        Unit::Percent => format!("{value:.0} %"),
        Unit::Ratio => format!("{value:.1}:1"),
        Unit::Q => format!("{value:.2}"),
        Unit::Semitones => format!("{value:+.1} st"),
        Unit::Bpm => format!("{value:.0} BPM"),
        Unit::Pan => format!("{value:+.0}"),
        Unit::Toggle => if value >= 0.5 { "ligado" } else { "desligado" }.into(),
        Unit::Choice => spec
            .choices
            .get(value.round().max(0.0) as usize)
            .copied()
            .unwrap_or("")
            .into(),
        Unit::None => format!("{value:.2}"),
    }
}

/// Splits parameters into groups by the part of their id before the first `.` (`b3.freq` belongs
/// to `b3`); parameters without a prefix form the first group, named `None`.
pub fn group_params(
    params: &'static [ParamSpec],
) -> Vec<(Option<&'static str>, Vec<&'static ParamSpec>)> {
    let mut groups: Vec<(Option<&'static str>, Vec<&'static ParamSpec>)> = Vec::new();
    for spec in params {
        let prefix = spec.id.split_once('.').map(|(p, _)| p);
        match groups.iter_mut().find(|(p, _)| *p == prefix) {
            Some((_, list)) => list.push(spec),
            None => groups.push((prefix, vec![spec])),
        }
    }
    groups
}

fn spec_chain(chain: &[TrackEffect]) -> Vec<EffectSpec> {
    chain
        .iter()
        .filter(|e| !e.bypass)
        .map(|e| EffectSpec {
            kind: e.kind.clone(),
            params: e.params.iter().map(|(k, v)| (k.clone(), *v)).collect(),
        })
        .collect()
}

/// What the mixer needs to play the project: every track's chain (indexed like the tracks of the
/// timeline) and the master's, without the bypassed effects.
pub fn fx_plan(timeline: &Timeline) -> FxPlan {
    FxPlan {
        tracks: timeline
            .tracks
            .iter()
            .map(|t| spec_chain(&t.effects))
            .collect(),
        master: spec_chain(&timeline.master_effects),
    }
}

// ---- drawing ----------------------------------------------------------------------------------------

/// Draws the chain `chain` of the target called `title` and returns what the user did.
pub fn fx_ui(
    ui: &mut egui::Ui,
    panel: &mut FxPanelState,
    title: &str,
    chain: &[TrackEffect],
) -> Vec<FxAction> {
    let mut actions = Vec::new();
    ui.horizontal(|ui| {
        ui.selectable_value(&mut panel.master, false, "Faixa");
        ui.selectable_value(&mut panel.master, true, "Master");
        ui.label(title);
    });

    let full = chain.len() >= neoveg_engine::effects::MAX_EFFECTS;
    ui.add_enabled_ui(!full, |ui| {
        egui::ComboBox::from_id_salt("fx_add")
            .selected_text("Adicionar efeito…")
            .width(200.0)
            .show_ui(ui, |ui| {
                for category in [
                    effects::Category::Filter,
                    effects::Category::Dynamics,
                    effects::Category::Time,
                ] {
                    ui.weak(category.label());
                    for info in available_effects(panel.master)
                        .into_iter()
                        .filter(|e| e.category == category)
                    {
                        if ui.selectable_label(false, info.name).clicked() {
                            actions.push(FxAction::Add(info.id));
                        }
                    }
                }
            });
    });
    if chain.is_empty() {
        ui.weak("Sem efeitos. Os efeitos rodam na ordem da lista.");
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        for (index, effect) in chain.iter().enumerate() {
            ui.separator();
            effect_ui(ui, panel, index, chain.len(), effect, &mut actions);
        }
    });
    actions
}

fn effect_ui(
    ui: &mut egui::Ui,
    panel: &mut FxPanelState,
    index: usize,
    count: usize,
    effect: &TrackEffect,
    actions: &mut Vec<FxAction>,
) {
    let info = effects::effect_info(&effect.kind);
    ui.horizontal(|ui| {
        if icons::icon_button_sized(
            ui,
            Icon::Bypass,
            "Ligar ou desligar o efeito (bypass)",
            !effect.bypass,
            22.0,
        )
        .clicked()
        {
            actions.push(FxAction::Bypass(index, !effect.bypass));
        }
        let name = info.map_or(effect.kind.as_str(), |i| i.name);
        ui.strong(name);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icons::icon_button_sized(ui, Icon::Remove, "Remover o efeito", false, 22.0).clicked()
            {
                actions.push(FxAction::Remove(index));
            }
            ui.add_enabled_ui(index + 1 < count, |ui| {
                if icons::icon_button_sized(ui, Icon::MoveDown, "Mover para baixo", false, 22.0)
                    .clicked()
                {
                    actions.push(FxAction::Move {
                        from: index,
                        to: index + 1,
                    });
                }
            });
            ui.add_enabled_ui(index > 0, |ui| {
                if icons::icon_button_sized(ui, Icon::MoveUp, "Mover para cima", false, 22.0)
                    .clicked()
                {
                    actions.push(FxAction::Move {
                        from: index,
                        to: index - 1,
                    });
                }
            });
        });
    });
    let Some(info) = info else {
        ui.colored_label(
            egui::Color32::from_rgb(231, 76, 60),
            "Efeito desconhecido: ele é ignorado na reprodução.",
        );
        return;
    };
    if effect.bypass {
        ui.weak("Em bypass");
    }
    ui.add_enabled_ui(!effect.bypass, |ui| {
        for (prefix, specs) in group_params(info.params) {
            match prefix {
                None => {
                    for spec in specs {
                        param_row(ui, panel, index, effect, spec, actions);
                    }
                }
                Some(prefix) => {
                    egui::CollapsingHeader::new(prefix)
                        .id_salt((index, &effect.kind, prefix))
                        .default_open(false)
                        .show(ui, |ui| {
                            for spec in specs {
                                param_row(ui, panel, index, effect, spec, actions);
                            }
                        });
                }
            }
        }
    });
}

fn param_row(
    ui: &mut egui::Ui,
    panel: &mut FxPanelState,
    index: usize,
    effect: &TrackEffect,
    spec: &'static ParamSpec,
    actions: &mut Vec<FxAction>,
) {
    let current = effective_value(effect, spec);
    ui.horizontal(|ui| {
        ui.add_sized([120.0, 18.0], egui::Label::new(spec.name).truncate());
        let response = match spec.unit {
            Unit::Toggle => {
                let mut on = current >= 0.5;
                let response = ui.checkbox(&mut on, "");
                if response.changed() {
                    actions.push(FxAction::SetParam {
                        index,
                        param: spec.id.into(),
                        value: if on { 1.0 } else { 0.0 },
                    });
                }
                response
            }
            Unit::Choice => {
                let selected = current.round().max(0.0) as usize;
                let text = spec.choices.get(selected).copied().unwrap_or("");
                egui::ComboBox::from_id_salt((index, &effect.kind, spec.id))
                    .selected_text(text)
                    .show_ui(ui, |ui| {
                        for (i, choice) in spec.choices.iter().enumerate() {
                            if ui.selectable_label(i == selected, *choice).clicked() {
                                actions.push(FxAction::SetParam {
                                    index,
                                    param: spec.id.into(),
                                    value: i as f32,
                                });
                            }
                        }
                    })
                    .response
            }
            _ => slider(ui, panel, index, spec, current, actions),
        };
        response.context_menu(|ui| {
            if ui.button("Restaurar o padrão").clicked() {
                actions.push(FxAction::ResetParam {
                    index,
                    param: spec.id.into(),
                });
                ui.close_menu();
            }
        });
    });
}

fn slider(
    ui: &mut egui::Ui,
    panel: &mut FxPanelState,
    index: usize,
    spec: &'static ParamSpec,
    current: f32,
    actions: &mut Vec<FxAction>,
) -> egui::Response {
    // While a drag is in progress the slider follows the draft; the project changes on release
    let dragging = panel
        .draft
        .as_ref()
        .filter(|d| d.index == index && d.param == spec.id);
    let mut value = dragging.map_or(current, |d| d.value);
    let response = ui.add(
        Slider::new(&mut value, spec.min..=spec.max)
            .logarithmic(spec.log_scale)
            .custom_formatter(|v, _| format_value(spec, v as f32)),
    );
    if response.changed() {
        panel.draft = Some(Draft {
            index,
            param: spec.id.into(),
            value,
        });
    }
    let finished = response.drag_stopped()
        || response.lost_focus()
        || (response.changed() && !response.dragged() && !response.has_focus());
    if finished {
        if let Some(draft) = panel
            .draft
            .take_if(|d| d.index == index && d.param == spec.id)
        {
            if (draft.value - current).abs() > f32::EPSILON {
                actions.push(FxAction::SetParam {
                    index,
                    param: draft.param,
                    value: draft.value,
                });
            }
        }
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use neoveg_engine::TrackKind;

    fn spec(id: &str) -> &'static ParamSpec {
        effects::effects()
            .iter()
            .flat_map(|e| e.params)
            .find(|p| p.id == id)
            .unwrap()
    }

    #[test]
    fn tracks_are_not_offered_effects_that_delay_the_signal_but_the_master_is() {
        assert!(available_effects(false).iter().all(|e| !e.has_latency));
        assert!(available_effects(false).iter().all(|e| e.id != "limiter"));
        assert!(available_effects(true).iter().any(|e| e.id == "limiter"));
        assert!(available_effects(false).len() + 1 == available_effects(true).len());
    }

    #[test]
    fn a_parameter_shows_its_stored_value_inside_the_range_or_its_default() {
        let ratio = spec("ratio");
        let mut e = TrackEffect::new("compressor");
        assert_eq!(effective_value(&e, ratio), ratio.default);
        e.params.insert("ratio".into(), 8.0);
        assert_eq!(effective_value(&e, ratio), 8.0);
        e.params.insert("ratio".into(), 1000.0);
        assert_eq!(
            effective_value(&e, ratio),
            ratio.max,
            "a file with a wild value is clamped"
        );
        e.params.insert("ratio".into(), f32::NAN);
        assert_eq!(effective_value(&e, ratio), ratio.default);
    }

    #[test]
    fn values_are_shown_with_their_units() {
        let by_unit = |unit: Unit| {
            effects::effects()
                .iter()
                .flat_map(|e| e.params)
                .find(|p| p.unit == unit)
                .unwrap()
        };
        assert_eq!(format_value(by_unit(Unit::Decibels), -20.0), "-20.0 dB");
        assert_eq!(format_value(by_unit(Unit::Hertz), 250.0), "250 Hz");
        assert_eq!(format_value(by_unit(Unit::Hertz), 5000.0), "5.00 kHz");
        assert_eq!(format_value(by_unit(Unit::Milliseconds), 10.0), "10.0 ms");
        assert_eq!(format_value(by_unit(Unit::Percent), 30.0), "30 %");
        assert_eq!(format_value(by_unit(Unit::Ratio), 4.0), "4.0:1");
        assert_eq!(format_value(spec("detector"), 1.0), "RMS");
        assert_eq!(format_value(spec("true_peak"), 1.0), "ligado");
    }

    #[test]
    fn eq_bands_are_grouped_by_their_prefix_and_plain_effects_form_one_group() {
        let eq = effects::effect_info("parametric_eq").unwrap();
        let groups = group_params(eq.params);
        assert!(groups.len() > 2);
        assert!(groups
            .iter()
            .any(|(p, list)| *p == Some("b1") && list.len() == 5));
        // The output gain has no prefix
        assert!(groups.iter().any(|(p, _)| p.is_none()));

        let comp = effects::effect_info("compressor").unwrap();
        let groups = group_params(comp.params);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, None);
        assert_eq!(groups[0].1.len(), comp.params.len());
    }

    #[test]
    fn the_plan_follows_the_track_order_and_leaves_out_bypassed_effects() {
        let mut tl = Timeline::new();
        for _ in 0..3 {
            let track = tl.new_track(TrackKind::Audio);
            tl.tracks.push(track);
        }
        let mut compressor = TrackEffect::new("compressor");
        compressor.params.insert("ratio".into(), 8.0);
        let mut bypassed = TrackEffect::new("reverb");
        bypassed.bypass = true;
        tl.tracks[1].effects = vec![compressor, bypassed, TrackEffect::new("simple_eq")];
        tl.master_effects = vec![TrackEffect::new("limiter")];

        let plan = fx_plan(&tl);
        assert_eq!(plan.tracks.len(), 3);
        assert!(plan.tracks[0].is_empty() && plan.tracks[2].is_empty());
        let kinds: Vec<&str> = plan.tracks[1].iter().map(|e| e.kind.as_str()).collect();
        assert_eq!(kinds, ["compressor", "simple_eq"]);
        assert_eq!(plan.tracks[1][0].params, vec![("ratio".to_string(), 8.0)]);
        assert_eq!(plan.master.len(), 1);
        assert!(!plan.is_empty());
        assert!(fx_plan(&Timeline::new()).is_empty());
    }
}
