//! The video effects panel: the chain of the selected event, of its media, of the video track or
//! of the whole project.
//!
//! Like the audio effects panel it only returns [`VfxAction`]s; `AppState` turns each one into one
//! undoable command. A slider commits when the drag ends, so one gesture is one undo step. The
//! effects themselves (names, parameters, ranges) come from the registry in `neoveg_video::vfx`.

use eframe::egui::{self, Slider};
use neoveg_common::effect_preset::{self, EffectPreset};
use neoveg_common::i18n::tr;
use neoveg_common::i18n::tr_text;
use neoveg_engine::{TrackEffect, VideoFxTarget};
use neoveg_video::vfx::{self, ParamSpec, VideoEffect};

use crate::curve_editor::{self, CurveOutput, CurveState};
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
    /// Several parameters of one effect as one edit (a point of the colour curves).
    SetParams {
        index: usize,
        values: Vec<(String, f32)>,
    },
    /// Keeps the values of the effect at `index` under `name` (a preset of that effect).
    SavePreset {
        index: usize,
        name: String,
    },
    DeletePreset {
        kind: String,
        name: String,
    },
    /// Writes the presets to a file the user picks / reads some from one.
    ExportPresets,
    ImportPresets,
    /// The color the video track fades to (track chain only).
    /// A color of the fade envelope: the bottom one when `bottom` is true.
    FadeColor {
        bottom: bool,
        color: [u8; 3],
    },
}

/// Colors offered for the fade to color of a video track.
pub const FADE_COLORS: [(&str, [u8; 3]); 8] = [
    ("Preto", [0, 0, 0]),
    ("Branco", [255, 255, 255]),
    ("Vermelho", [220, 40, 40]),
    ("Verde", [40, 170, 70]),
    ("Azul", [40, 90, 220]),
    ("Amarelo", [240, 200, 30]),
    ("Ciano", [30, 190, 210]),
    ("Magenta", [210, 50, 160]),
];

/// A slider value that is being dragged: the project changes only when the drag ends.
#[derive(Debug, Clone, PartialEq)]
struct Draft {
    index: usize,
    values: Vec<(String, f32)>,
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
    /// The editor of the colour curves.
    curves: CurveState,
    /// The name typed for a new preset.
    preset_name: String,
}

impl VfxPanelState {
    /// While the user drags a slider or a curve point: the effect index and the values being
    /// dragged.
    pub fn draft_view(&self) -> Option<(usize, &[(String, f32)])> {
        self.draft.as_ref().map(|d| (d.index, d.values.as_slice()))
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

/// Values that are being dragged, ready for the preview: the chain at `target` is shown with the
/// `values` of effect `index` put in, without touching the project.
#[derive(Debug, Clone, PartialEq)]
pub struct VfxDraft {
    pub target: VideoFxTarget,
    pub index: usize,
    pub values: Vec<(String, f32)>,
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
        for (param, value) in &d.values {
            param.hash(&mut h);
            value.to_bits().hash(&mut h);
        }
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
                for (param, value) in &d.values {
                    effect.params.insert(param.clone(), *value);
                }
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

/// The values of every parameter of `spec` when `preset` is applied: the ones it holds clamped to
/// their ranges, the defaults for the rest.
pub fn preset_values(spec: &vfx::EffectSpec, preset: &EffectPreset) -> Vec<(String, f32)> {
    spec.params
        .iter()
        .map(|p| {
            let value = match preset.params.get(p.id) {
                Some(v) if v.is_finite() => v.clamp(p.min, p.max),
                _ => p.default,
            };
            (p.id.to_owned(), value)
        })
        .collect()
}

/// A preset of `effect` under `name`: every parameter as it is shown.
pub fn preset_of(effect: &TrackEffect, name: &str) -> Option<EffectPreset> {
    let spec = vfx::spec(&effect.kind)?;
    Some(EffectPreset {
        kind: effect.kind.clone(),
        name: name.to_owned(),
        params: spec
            .params
            .iter()
            .map(|p| (p.id.to_owned(), effective_value(effect, p)))
            .collect(),
    })
}

/// Draws the panel and returns what the user did.
#[allow(clippy::too_many_arguments)]
pub fn vfx_ui(
    ui: &mut egui::Ui,
    panel: &mut VfxPanelState,
    title: &str,
    chain: &[TrackEffect],
    scopes: VfxScopes,
    fade_colors: Option<([u8; 3], [u8; 3])>,
    presets: &[EffectPreset],
) -> Vec<VfxAction> {
    let mut actions = Vec::new();
    panel.scope = scopes.resolve(panel.scope);
    ui.horizontal(|ui| {
        ui.add_enabled_ui(scopes.event, |ui| {
            ui.selectable_value(&mut panel.scope, VfxScope::Event, tr("Evento"));
        });
        ui.add_enabled_ui(scopes.media, |ui| {
            ui.selectable_value(&mut panel.scope, VfxScope::Media, tr("Mídia"));
        });
        ui.add_enabled_ui(scopes.track, |ui| {
            ui.selectable_value(&mut panel.scope, VfxScope::Track, tr("Faixa"));
        });
        ui.selectable_value(&mut panel.scope, VfxScope::Project, tr("Projeto"));
    });
    ui.label(title);

    let full = chain.len() >= neoveg_engine::video_effects::MAX_VIDEO_EFFECTS;
    ui.add_enabled_ui(!full, |ui| {
        egui::ComboBox::from_id_salt("vfx_add")
            .selected_text(tr("Adicionar efeito…"))
            .width(200.0)
            .show_ui(ui, |ui| {
                for spec in vfx::EFFECTS {
                    if ui.selectable_label(false, tr_text(spec.label)).clicked() {
                        actions.push(VfxAction::Add(spec.id));
                    }
                }
            });
    });
    ui.horizontal(|ui| {
        if ui
            .small_button(tr("Importar predefinições…"))
            .on_hover_text(tr("Lê de um arquivo as predefinições dos efeitos de vídeo"))
            .clicked()
        {
            actions.push(VfxAction::ImportPresets);
        }
        if ui
            .add_enabled(
                !presets.is_empty(),
                egui::Button::new(tr("Exportar predefinições…")).small(),
            )
            .clicked()
        {
            actions.push(VfxAction::ExportPresets);
        }
    });
    if let Some((top, bottom)) = fade_colors {
        for (label, current, is_bottom) in [
            (tr("Esmaecer para (acima de 0 %):"), top, false),
            (tr("Esmaecer para (abaixo de 0 %):"), bottom, true),
        ] {
            ui.horizontal(|ui| {
                ui.label(label);
                for (name, rgb) in FADE_COLORS {
                    let fill = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                    let stroke = if rgb == current {
                        egui::Stroke::new(2.0_f32, ui.visuals().text_color())
                    } else {
                        egui::Stroke::new(
                            1.0_f32,
                            ui.visuals().widgets.noninteractive.bg_stroke.color,
                        )
                    };
                    let button = egui::Button::new("")
                        .fill(fill)
                        .stroke(stroke)
                        .min_size(egui::vec2(22.0, 18.0));
                    if ui.add(button).on_hover_text(name).clicked() && rgb != current {
                        actions.push(VfxAction::FadeColor {
                            bottom: is_bottom,
                            color: rgb,
                        });
                    }
                }
            });
        }
        ui.weak(tr("As cores valem para o envelope Esmaecer para cor desta faixa (Faixas > Envelopes de vídeo): valores positivos vão para a primeira cor e negativos para a segunda."));
    }
    if chain.is_empty() {
        ui.weak(tr(
            "Sem efeitos. Os efeitos rodam na ordem da lista, antes da opacidade do evento.",
        ));
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        for (index, effect) in chain.iter().enumerate() {
            ui.separator();
            effect_ui(ui, panel, index, chain.len(), effect, presets, &mut actions);
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
    presets: &[EffectPreset],
    actions: &mut Vec<VfxAction>,
) {
    let spec = vfx::spec(&effect.kind);
    ui.horizontal(|ui| {
        if icons::icon_button_sized(
            ui,
            Icon::Bypass,
            tr("Ligar ou desligar o efeito (bypass)"),
            !effect.bypass,
            22.0,
        )
        .clicked()
        {
            actions.push(VfxAction::Bypass(index, !effect.bypass));
        }
        ui.strong(tr_text(spec.map_or(effect.kind.as_str(), |s| s.label)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icons::icon_button_sized(ui, Icon::Remove, tr("Remover o efeito"), false, 22.0)
                .clicked()
            {
                actions.push(VfxAction::Remove(index));
            }
            ui.add_enabled_ui(index + 1 < count, |ui| {
                if icons::icon_button_sized(ui, Icon::MoveDown, tr("Mover para baixo"), false, 22.0)
                    .clicked()
                {
                    actions.push(VfxAction::Move {
                        from: index,
                        to: index + 1,
                    });
                }
            });
            ui.add_enabled_ui(index > 0, |ui| {
                if icons::icon_button_sized(ui, Icon::MoveUp, tr("Mover para cima"), false, 22.0)
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
        ui.weak(tr("Em bypass"));
    }
    if spec.params.is_empty() {
        ui.weak(tr("Sem parâmetros."));
    }
    preset_row(ui, panel, index, spec, presets, actions);
    ui.add_enabled_ui(!effect.bypass, |ui| {
        if spec.id == vfx::COLOR_CURVES {
            curves_row(ui, panel, index, effect, actions);
            return;
        }
        for param in spec.params {
            param_row(ui, panel, index, effect, param, actions);
        }
    });
}

/// The presets of one effect: a list to apply one, and a name box to keep the values as they are.
fn preset_row(
    ui: &mut egui::Ui,
    panel: &mut VfxPanelState,
    index: usize,
    spec: &vfx::EffectSpec,
    presets: &[EffectPreset],
    actions: &mut Vec<VfxAction>,
) {
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt(("vfx_preset", index))
            .selected_text(tr("Predefinição…"))
            .width(130.0)
            .show_ui(ui, |ui| {
                let mut any = false;
                for preset in effect_preset::for_kind(presets, spec.id) {
                    any = true;
                    if ui.selectable_label(false, &preset.name).clicked() {
                        panel.preset_name = preset.name.clone();
                        actions.push(VfxAction::SetParams {
                            index,
                            values: preset_values(spec, preset),
                        });
                    }
                }
                if !any {
                    ui.weak(tr("Nenhuma predefinição salva"));
                }
            });
        ui.add(
            egui::TextEdit::singleline(&mut panel.preset_name)
                .hint_text(tr("Nome"))
                .desired_width(90.0),
        );
        let name = effect_preset::clean_name(&panel.preset_name);
        if ui
            .add_enabled(name.is_some(), egui::Button::new(tr("Salvar")).small())
            .on_hover_text(tr("Guarda os valores deste efeito sob esse nome"))
            .clicked()
        {
            if let Some(name) = name.clone() {
                actions.push(VfxAction::SavePreset { index, name });
            }
        }
        let exists = name
            .as_deref()
            .is_some_and(|n| effect_preset::for_kind(presets, spec.id).any(|p| p.name == n));
        if ui
            .add_enabled(exists, egui::Button::new(tr("Excluir")).small())
            .clicked()
        {
            if let Some(name) = name {
                actions.push(VfxAction::DeletePreset {
                    kind: spec.id.to_owned(),
                    name,
                });
            }
        }
    });
}

/// The curve editor in place of the sliders of the colour curves effect.
fn curves_row(
    ui: &mut egui::Ui,
    panel: &mut VfxPanelState,
    index: usize,
    effect: &TrackEffect,
    actions: &mut Vec<VfxAction>,
) {
    let dragged: Vec<(String, f32)> = panel
        .draft
        .as_ref()
        .filter(|d| d.index == index)
        .map(|d| d.values.clone())
        .unwrap_or_default();
    match curve_editor::curves_ui(ui, &mut panel.curves, effect, &dragged) {
        Some(CurveOutput::Live(values)) => panel.draft = Some(Draft { index, values }),
        Some(CurveOutput::Commit(values)) => {
            panel.draft = None;
            actions.push(VfxAction::SetParams { index, values });
        }
        None => {}
    }
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
        ui.add_sized(
            [120.0, 18.0],
            egui::Label::new(tr_text(spec.label)).truncate(),
        );
        // While a drag is in progress the slider follows the draft; the project changes on release
        let dragging = panel
            .draft
            .as_ref()
            .filter(|d| d.index == index && d.values.len() == 1 && d.values[0].0 == spec.id);
        let mut value = dragging.map_or(current, |d| d.values[0].1);
        let response = ui
            .scope(|ui| {
                crate::fx_panel::fit_slider(ui);
                ui.add(
                    Slider::new(&mut value, spec.min..=spec.max)
                        .custom_formatter(|v, _| format_value(spec, v as f32)),
                )
            })
            .inner;
        if response.changed() {
            panel.draft = Some(Draft {
                index,
                values: vec![(spec.id.into(), value)],
            });
        }
        let finished = response.drag_stopped()
            || response.lost_focus()
            || (response.changed() && !response.dragged() && !response.has_focus());
        if finished {
            if let Some(draft) = panel
                .draft
                .take_if(|d| d.index == index && d.values.len() == 1 && d.values[0].0 == spec.id)
            {
                let (param, value) = draft.values[0].clone();
                if (value - current).abs() > f32::EPSILON {
                    actions.push(VfxAction::SetParam {
                        index,
                        param,
                        value,
                    });
                }
            }
        }
        response.context_menu(|ui| {
            if ui.button(tr("Restaurar o padrão")).clicked() {
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
    fn a_preset_keeps_every_shown_value_and_applies_clamped_with_defaults_for_the_rest() {
        let mut effect = TrackEffect::new(vfx::GAUSSIAN_BLUR);
        effect.params.insert("h_range".into(), 0.03);
        let preset = preset_of(&effect, "Suave").unwrap();
        let spec = vfx::spec(vfx::GAUSSIAN_BLUR).unwrap();
        assert_eq!(preset.params.len(), spec.params.len());
        assert_eq!(preset.params["h_range"], 0.03);
        assert!(preset_of(&TrackEffect::new("nao-existe"), "x").is_none());
        // Applying: a value out of range is clamped, a missing one takes the default
        let mut odd = preset.clone();
        odd.params.insert("h_range".into(), 9.0);
        odd.params.remove("v_range");
        let values = preset_values(spec, &odd);
        let get = |id: &str| values.iter().find(|(p, _)| p == id).unwrap().1;
        assert_eq!(get("h_range"), spec.param("h_range").unwrap().max);
        assert_eq!(get("v_range"), spec.param("v_range").unwrap().default);
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
            values: vec![("h_range".into(), 0.04)],
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
            values: vec![("h_range".into(), 0.05)],
            ..draft.clone()
        }));
        assert_ne!(a, 0);
        assert_ne!(a, b);
        assert_eq!(a, VfxDraft::stamp(Some(&draft)));
    }

    #[test]
    fn a_draft_of_a_curve_point_puts_both_numbers_in_the_chain() {
        let target = VideoFxTarget::Project;
        let stored = [TrackEffect::new(vfx::COLOR_CURVES)];
        let draft = VfxDraft {
            target,
            index: 0,
            values: vec![("m_x2".into(), 0.3), ("m_y2".into(), 0.1)],
        };
        let live = effects_for(&stored, target, Some(&draft));
        assert_eq!((live[0].params["m_x2"], live[0].params["m_y2"]), (0.3, 0.1));
        let moved = VfxDraft::stamp(Some(&VfxDraft {
            values: vec![("m_x2".into(), 0.3), ("m_y2".into(), 0.2)],
            ..draft.clone()
        }));
        assert_ne!(VfxDraft::stamp(Some(&draft)), moved);
    }

    #[test]
    fn the_fade_palette_starts_with_black_and_has_no_repeated_color() {
        assert_eq!(
            FADE_COLORS[0].1,
            [0, 0, 0],
            "black is the default of a track"
        );
        for (i, (_, a)) in FADE_COLORS.iter().enumerate() {
            assert!(FADE_COLORS.iter().skip(i + 1).all(|(_, b)| a != b));
        }
    }
}
