//! The video effects panel: the chain of the selected event, of its media, of the video track or
//! of the whole project.
//!
//! Like the audio effects panel it only returns [`VfxAction`]s; `AppState` turns each one into one
//! undoable command. A slider commits when the drag ends, so one gesture is one undo step. The
//! effects themselves (names, parameters, ranges) come from the registry in `neoveg_video::vfx`.

use eframe::egui::{self, Slider};
use neoveg_engine::{TrackEffect, VideoFxTarget};
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

/// Which chain the panel edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VfxScope {
    /// The selected event.
    #[default]
    Event,
    /// The media pool entry of the selected event: every event that plays it.
    Media,
    /// The composited layer of the video track.
    Track,
    /// The whole composited picture.
    Project,
}

#[derive(Debug, Default)]
pub struct VfxPanelState {
    /// The chain the user chose; it falls back to one that exists (see [`VfxScopes::resolve`]).
    pub scope: VfxScope,
    draft: Option<Draft>,
}

impl VfxPanelState {
    /// While the user drags a slider: the effect index, the parameter and the value being dragged.
    pub fn draft_view(&self) -> Option<(usize, &str, f32)> {
        self.draft
            .as_ref()
            .map(|d| (d.index, d.param.as_str(), d.value))
    }
}

/// What the panel can edit right now. The project chain is always there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VfxScopes {
    /// A video event is selected.
    pub event: bool,
    /// The selected event plays a media pool entry.
    pub media: bool,
    /// A video track is selected, or the track of the selected event.
    pub track: bool,
}

impl VfxScopes {
    /// `wanted` when it can be edited, otherwise the nearest one that can: event, then track, then
    /// the project.
    pub fn resolve(self, wanted: VfxScope) -> VfxScope {
        let ok = |s: VfxScope| match s {
            VfxScope::Event => self.event,
            VfxScope::Media => self.media,
            VfxScope::Track => self.track,
            VfxScope::Project => true,
        };
        [wanted, VfxScope::Event, VfxScope::Track, VfxScope::Project]
            .into_iter()
            .find(|s| ok(*s))
            .unwrap_or(VfxScope::Project)
    }
}

/// A slider value that is being dragged, ready for the preview: the chain at `target` is shown
/// with `param` of effect `index` set to `value`, without touching the project.
#[derive(Debug, Clone, PartialEq)]
pub struct VfxDraft {
    pub target: VideoFxTarget,
    pub index: usize,
    pub param: String,
    pub value: f32,
}

impl VfxDraft {
    /// A number that changes whenever the draft does (part of the preview's frame key).
    pub fn stamp(draft: Option<&VfxDraft>) -> u64 {
        use std::hash::{Hash, Hasher};
        let Some(d) = draft else {
            return 0;
        };
        let mut h = std::collections::hash_map::DefaultHasher::new();
        match d.target {
            VideoFxTarget::Event { track_id, event_id } => (0u8, track_id, event_id).hash(&mut h),
            VideoFxTarget::Track(id) => (1u8, id).hash(&mut h),
            VideoFxTarget::Media(id) => (2u8, id).hash(&mut h),
            VideoFxTarget::Project => 3u8.hash(&mut h),
        }
        d.index.hash(&mut h);
        d.param.hash(&mut h);
        d.value.to_bits().hash(&mut h);
        h.finish() | 1
    }
}

/// The worker's chain for `target`: the stored chain with the dragged value of `draft` put in when
/// the draft belongs to this chain.
pub fn effects_for(
    stored: &[TrackEffect],
    target: VideoFxTarget,
    draft: Option<&VfxDraft>,
) -> Vec<VideoEffect> {
    match draft.filter(|d| d.target == target) {
        Some(d) => {
            let mut chain = stored.to_vec();
            if let Some(effect) = chain.get_mut(d.index) {
                effect.params.insert(d.param.clone(), d.value);
            }
            to_worker_chain(&chain)
        }
        None => to_worker_chain(stored),
    }
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
    panel.scope = scopes.resolve(panel.scope);
    ui.horizontal(|ui| {
        ui.add_enabled_ui(scopes.event, |ui| {
            ui.selectable_value(&mut panel.scope, VfxScope::Event, "Evento");
        });
        ui.add_enabled_ui(scopes.media, |ui| {
            ui.selectable_value(&mut panel.scope, VfxScope::Media, "Mídia");
        });
        ui.add_enabled_ui(scopes.track, |ui| {
            ui.selectable_value(&mut panel.scope, VfxScope::Track, "Faixa");
        });
        ui.selectable_value(&mut panel.scope, VfxScope::Project, "Projeto");
    });
    ui.label(title);

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

    #[test]
    fn a_scope_that_cannot_be_edited_falls_back_to_the_nearest_one() {
        let all = VfxScopes {
            event: true,
            media: true,
            track: true,
        };
        assert_eq!(all.resolve(VfxScope::Media), VfxScope::Media);
        let track_only = VfxScopes {
            event: false,
            media: false,
            track: true,
        };
        assert_eq!(track_only.resolve(VfxScope::Event), VfxScope::Track);
        assert_eq!(track_only.resolve(VfxScope::Media), VfxScope::Track);
        let nothing = VfxScopes {
            event: false,
            media: false,
            track: false,
        };
        assert_eq!(nothing.resolve(VfxScope::Event), VfxScope::Project);
        assert_eq!(nothing.resolve(VfxScope::Project), VfxScope::Project);
    }

    #[test]
    fn a_draft_changes_only_the_chain_it_belongs_to_and_the_stamp_follows_it() {
        let target = VideoFxTarget::Media(4);
        let other = VideoFxTarget::Project;
        let mut blur = TrackEffect::new(vfx::GAUSSIAN_BLUR);
        blur.params.insert("h_range".into(), 0.01);
        let stored = [blur];
        let draft = VfxDraft {
            target,
            index: 0,
            param: "h_range".into(),
            value: 0.04,
        };
        let live = effects_for(&stored, target, Some(&draft));
        assert_eq!(live[0].params["h_range"], 0.04);
        let untouched = effects_for(&stored, other, Some(&draft));
        assert_eq!(untouched[0].params["h_range"], 0.01);
        assert_eq!(
            stored[0].params["h_range"], 0.01,
            "the project is not changed"
        );
        // an index that no longer exists is ignored, not a panic
        let stale = VfxDraft {
            index: 9,
            ..draft.clone()
        };
        assert_eq!(effects_for(&stored, target, Some(&stale)).len(), 1);

        assert_eq!(VfxDraft::stamp(None), 0);
        let a = VfxDraft::stamp(Some(&draft));
        let b = VfxDraft::stamp(Some(&VfxDraft {
            value: 0.05,
            ..draft.clone()
        }));
        assert_ne!(a, 0);
        assert_ne!(a, b);
        assert_eq!(a, VfxDraft::stamp(Some(&draft)));
    }
}
