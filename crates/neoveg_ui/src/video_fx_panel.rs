//! The video effects panel: the chain of the selected event or of the selected video track.
//!
//! Like the audio effects panel it only returns [`VfxAction`]s; `AppState` turns each one into one
//! undoable command. A slider commits when the drag ends, so one gesture is one undo step. The
//! effects themselves (names, parameters, ranges) come from the registry in `neoveg_video::vfx`.

use eframe::egui::{self, Slider};
use neoveg_engine::TrackEffect;
use neoveg_video::vfx::{self, ParamSpec, VideoEffect};

use crate::icons::{self, Icon};

/// What the user did in the panel.
#[derive(Debug, Clone, PartialEq)]
pub enum VfxAction {
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
pub struct VfxPanelState {
    /// Edit the chain of the whole track instead of the selected event.
    pub track_scope: bool,
    draft: Option<Draft>,
}

/// What the panel can edit right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VfxScopes {
    /// A video event is selected.
    pub event: bool,
    /// A video track is selected, or the track of the selected event.
    pub track: bool,
}

/// The worker's version of a stored chain: bypassed effects and unknown ids are left out, values
/// stay as stored (the video layer clamps them).
pub fn to_worker_chain(chain: &[TrackEffect]) -> Vec<VideoEffect> {
    chain
        .iter()
        .filter(|e| !e.bypass && vfx::spec(&e.kind).is_some())
        .map(|e| VideoEffect {
            kind: e.kind.clone(),
            params: e.params.clone(),
        })
        .collect()
}

/// The value shown for a parameter: the stored one clamped to its range, or the default.
pub fn effective_value(effect: &TrackEffect, spec: &ParamSpec) -> f32 {
    match effect.params.get(spec.id) {
        Some(v) if v.is_finite() => v.clamp(spec.min, spec.max),
        _ => spec.default,
    }
}

pub fn format_value(spec: &ParamSpec, value: f32) -> String {
    if spec.percent {
        format!("{:.1} %", value * 100.0)
    } else {
        format!("{value:.2}")
    }
}

/// Draws the panel and returns what the user did.
pub fn vfx_ui(
    ui: &mut egui::Ui,
    panel: &mut VfxPanelState,
    title: &str,
    chain: &[TrackEffect],
    scopes: VfxScopes,
) -> Vec<VfxAction> {
    let mut actions = Vec::new();
    if !scopes.track {
        panel.track_scope = false;
    } else if !scopes.event {
        panel.track_scope = true;
    }
    ui.horizontal(|ui| {
        ui.add_enabled_ui(scopes.event, |ui| {
            ui.selectable_value(&mut panel.track_scope, false, "Evento");
        });
        ui.add_enabled_ui(scopes.track, |ui| {
            ui.selectable_value(&mut panel.track_scope, true, "Faixa");
        });
        ui.label(title);
    });
    if !scopes.event && !scopes.track {
        ui.weak("Selecione um evento ou uma faixa de vídeo para ver os efeitos.");
        return actions;
    }

    let full = chain.len() >= neoveg_engine::video_effects::MAX_VIDEO_EFFECTS;
    ui.add_enabled_ui(!full, |ui| {
        egui::ComboBox::from_id_salt("vfx_add")
            .selected_text("Adicionar efeito…")
            .width(200.0)
            .show_ui(ui, |ui| {
                for spec in vfx::EFFECTS {
                    if ui.selectable_label(false, spec.label).clicked() {
                        actions.push(VfxAction::Add(spec.id));
                    }
                }
            });
    });
    if chain.is_empty() {
        ui.weak("Sem efeitos. Os efeitos rodam na ordem da lista, antes da opacidade do evento.");
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
    panel: &mut VfxPanelState,
    index: usize,
    count: usize,
    effect: &TrackEffect,
    actions: &mut Vec<VfxAction>,
) {
    let spec = vfx::spec(&effect.kind);
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
            actions.push(VfxAction::Bypass(index, !effect.bypass));
        }
        ui.strong(spec.map_or(effect.kind.as_str(), |s| s.label));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icons::icon_button_sized(ui, Icon::Remove, "Remover o efeito", false, 22.0).clicked()
            {
                actions.push(VfxAction::Remove(index));
            }
            ui.add_enabled_ui(index + 1 < count, |ui| {
                if icons::icon_button_sized(ui, Icon::MoveDown, "Mover para baixo", false, 22.0)
                    .clicked()
                {
                    actions.push(VfxAction::Move {
                        from: index,
                        to: index + 1,
                    });
                }
            });
            ui.add_enabled_ui(index > 0, |ui| {
                if icons::icon_button_sized(ui, Icon::MoveUp, "Mover para cima", false, 22.0)
                    .clicked()
                {
                    actions.push(VfxAction::Move {
                        from: index,
                        to: index - 1,
                    });
                }
            });
        });
    });
    let Some(spec) = spec else {
        ui.colored_label(
            egui::Color32::from_rgb(231, 76, 60),
            "Efeito desconhecido: ele é ignorado na prévia e na renderização.",
        );
        return;
    };
    if effect.bypass {
        ui.weak("Em bypass");
    }
    if spec.params.is_empty() {
        ui.weak("Sem parâmetros.");
    }
    ui.add_enabled_ui(!effect.bypass, |ui| {
        for param in spec.params {
            param_row(ui, panel, index, effect, param, actions);
        }
    });
}

fn param_row(
    ui: &mut egui::Ui,
    panel: &mut VfxPanelState,
    index: usize,
    effect: &TrackEffect,
    spec: &'static ParamSpec,
    actions: &mut Vec<VfxAction>,
) {
    let current = effective_value(effect, spec);
    ui.horizontal(|ui| {
        ui.add_sized([120.0, 18.0], egui::Label::new(spec.label).truncate());
        // While a drag is in progress the slider follows the draft; the project changes on release
        let dragging = panel
            .draft
            .as_ref()
            .filter(|d| d.index == index && d.param == spec.id);
        let mut value = dragging.map_or(current, |d| d.value);
        let response = ui.add(
            Slider::new(&mut value, spec.min..=spec.max)
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
                    actions.push(VfxAction::SetParam {
                        index,
                        param: draft.param,
                        value: draft.value,
                    });
                }
            }
        }
        response.context_menu(|ui| {
            if ui.button("Restaurar o padrão").clicked() {
                actions.push(VfxAction::ResetParam {
                    index,
                    param: spec.id.into(),
                });
                ui.close_menu();
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_worker_chain_leaves_out_bypassed_and_unknown_effects_and_keeps_the_order() {
        let mut blur = TrackEffect::new(vfx::GAUSSIAN_BLUR);
        blur.params.insert("h_range".into(), 0.02);
        let mut off = TrackEffect::new(vfx::SHARPEN);
        off.bypass = true;
        let chain = [
            blur,
            off,
            TrackEffect::new("from_the_future"),
            TrackEffect::new(vfx::VIGNETTE),
        ];
        let worker = to_worker_chain(&chain);
        assert_eq!(
            worker.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(),
            [vfx::GAUSSIAN_BLUR, vfx::VIGNETTE]
        );
        assert_eq!(worker[0].params["h_range"], 0.02);
    }

    #[test]
    fn shown_values_are_clamped_and_default_when_missing_or_bad() {
        let spec = vfx::spec(vfx::GAUSSIAN_BLUR)
            .and_then(|s| s.param("h_range"))
            .unwrap();
        let mut fx = TrackEffect::new(vfx::GAUSSIAN_BLUR);
        assert_eq!(effective_value(&fx, spec), spec.default);
        fx.params.insert("h_range".into(), 5.0);
        assert_eq!(effective_value(&fx, spec), spec.max);
        fx.params.insert("h_range".into(), f32::NAN);
        assert_eq!(effective_value(&fx, spec), spec.default);
        assert_eq!(format_value(spec, 0.025), "2.5 %");
    }

    #[test]
    fn every_registry_default_is_inside_its_range() {
        for effect in vfx::EFFECTS {
            for p in effect.params {
                assert!(
                    p.min <= p.default && p.default <= p.max,
                    "{}.{}",
                    effect.id,
                    p.id
                );
                assert!(!p.label.is_empty());
            }
            assert!(!effect.label.is_empty());
        }
    }
}
