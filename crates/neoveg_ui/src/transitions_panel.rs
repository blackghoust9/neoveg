//! The Transitions tab: the transition that reveals (or hides) the selected video event.
//!
//! A transition runs over the fade of the event (or the overlap with the event before it) and follows
//! its fade curve, so the tab only chooses *which* transition and its values; the length is the one
//! of the fade. Like the effects panels it returns actions and `AppState` runs each one as one undo
//! step; a slider commits when the drag ends.

use eframe::egui::{self, Slider};
use neoveg_common::i18n::tr;
use neoveg_engine::TrackEffect;
use neoveg_video::transitions::{self, TransitionRequest};
use neoveg_video::vfx::ParamSpec;

/// What the user did in the tab.
#[derive(Debug, Clone, PartialEq)]
pub enum TransitionAction {
    /// Use this transition (the registry id).
    Choose(&'static str),
    /// Back to the plain dissolve.
    Clear,
    SetParam {
        param: String,
        value: f32,
    },
    ResetParam(String),
}

#[derive(Debug, Clone, PartialEq)]
struct Draft {
    param: String,
    value: f32,
}

/// State of the tab.
#[derive(Debug, Default)]
pub struct TransitionsPanelState {
    /// Edit the transition that hides the event instead of the one that reveals it.
    pub outgoing: bool,
    draft: Option<Draft>,
}

/// What the tab shows about the event.
pub struct TransitionsView<'a> {
    pub event_name: &'a str,
    pub current: Option<&'a TrackEffect>,
    /// Seconds the transition runs: the fade, or for the entrance the longer of fade and overlap.
    pub length: f64,
    /// `true` when the entrance overlaps the event before it.
    pub overlaps: bool,
}

/// The worker's version of a stored transition at `progress` (the fade gain, `0..1`), or `None` for
/// a plain dissolve and for an id the registry does not know.
pub fn to_request(setting: Option<&TrackEffect>, progress: f32) -> Option<TransitionRequest> {
    let t = setting?;
    if t.kind == transitions::DISSOLVE || transitions::spec(&t.kind).is_none() {
        return None;
    }
    Some(TransitionRequest {
        kind: t.kind.clone(),
        params: t.params.clone(),
        progress,
        entering: false,
    })
}

fn effective(effect: &TrackEffect, spec: &ParamSpec) -> f32 {
    match effect.params.get(spec.id) {
        Some(v) if v.is_finite() => v.clamp(spec.min, spec.max),
        _ => spec.default,
    }
}

fn format_value(spec: &ParamSpec, value: f32) -> String {
    if spec.percent {
        format!("{:.0} %", value * 100.0)
    } else if spec.id.contains("angle") {
        format!("{value:.0} °")
    } else {
        format!("{value:.0}")
    }
}

impl TransitionsPanelState {
    /// Draws the tab and returns what the user did.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        view: Option<&TransitionsView>,
    ) -> Vec<TransitionAction> {
        let mut actions = Vec::new();
        let Some(view) = view else {
            ui.weak(tr(
                "Selecione um evento de uma faixa de vídeo para escolher a transição dele.",
            ));
            return actions;
        };
        ui.horizontal(|ui| {
            ui.strong(view.event_name);
            ui.separator();
            ui.selectable_value(&mut self.outgoing, false, "Entrada");
            ui.selectable_value(&mut self.outgoing, true, "Saída");
        });
        if view.length > 0.0 {
            ui.weak(format!(
                "Dura {:.2} s: {}. Segue a curva do fade.",
                view.length,
                match (self.outgoing, view.overlaps) {
                    (true, _) => "o fade de saída do evento",
                    (false, true) => "a sobreposição com o evento anterior",
                    (false, false) => "o fade de entrada do evento",
                }
            ));
        } else {
            ui.colored_label(
                egui::Color32::from_rgb(241, 196, 15),
                "O evento não tem fade neste lado: a transição só aparece com um fade (ou uma sobreposição). Ao escolher uma, o Neoveg dá 1 s de fade.",
            );
        }
        ui.separator();

        let current_kind = view
            .current
            .map_or(transitions::DISSOLVE, |t| t.kind.as_str());
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for spec in transitions::transitions() {
                    let selected = spec.id == current_kind;
                    if ui.selectable_label(selected, spec.label).clicked() && !selected {
                        if spec.id == transitions::DISSOLVE {
                            actions.push(TransitionAction::Clear);
                        } else {
                            actions.push(TransitionAction::Choose(spec.id));
                        }
                    }
                }
            });
            let (Some(effect), Some(spec)) = (
                view.current,
                view.current.and_then(|t| transitions::spec(&t.kind)),
            ) else {
                return;
            };
            ui.separator();
            if spec.params.is_empty() {
                ui.weak(tr("Sem parâmetros."));
            }
            for param in spec.params {
                self.param_row(ui, effect, param, &mut actions);
            }
        });
        actions
    }

    fn param_row(
        &mut self,
        ui: &mut egui::Ui,
        effect: &TrackEffect,
        spec: &'static ParamSpec,
        actions: &mut Vec<TransitionAction>,
    ) {
        let current = effective(effect, spec);
        ui.horizontal(|ui| {
            ui.add_sized([130.0, 18.0], egui::Label::new(spec.label).truncate());
            let dragging = self.draft.as_ref().filter(|d| d.param == spec.id);
            let mut value = dragging.map_or(current, |d| d.value);
            let response = ui.add(
                Slider::new(&mut value, spec.min..=spec.max)
                    .custom_formatter(|v, _| format_value(spec, v as f32)),
            );
            if response.changed() {
                self.draft = Some(Draft {
                    param: spec.id.into(),
                    value,
                });
            }
            let finished = response.drag_stopped()
                || response.lost_focus()
                || (response.changed() && !response.dragged() && !response.has_focus());
            if finished {
                if let Some(draft) = self.draft.take_if(|d| d.param == spec.id) {
                    if (draft.value - current).abs() > f32::EPSILON {
                        actions.push(TransitionAction::SetParam {
                            param: draft.param,
                            value: draft.value,
                        });
                    }
                }
            }
            response.context_menu(|ui| {
                if ui.button(tr("Restaurar o padrão")).clicked() {
                    actions.push(TransitionAction::ResetParam(spec.id.into()));
                    ui.close_menu();
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_dissolve_and_an_unknown_id_send_no_request() {
        assert!(to_request(None, 0.5).is_none());
        assert!(to_request(Some(&TrackEffect::new(transitions::DISSOLVE)), 0.5).is_none());
        assert!(to_request(Some(&TrackEffect::new("from_the_future")), 0.5).is_none());
    }

    #[test]
    fn a_stored_transition_becomes_a_request_with_its_values_and_the_progress() {
        let mut stored = TrackEffect::new(transitions::LINEAR_WIPE);
        stored.params.insert("angle".into(), 45.0);
        let r = to_request(Some(&stored), 0.25).unwrap();
        assert_eq!(r.kind, transitions::LINEAR_WIPE);
        assert_eq!(r.params["angle"], 45.0);
        assert_eq!(r.progress, 0.25);
    }

    #[test]
    fn shown_values_are_clamped_and_default_when_missing_or_bad() {
        let spec = transitions::spec(transitions::BLINDS)
            .and_then(|s| s.param("count"))
            .unwrap();
        let mut fx = TrackEffect::new(transitions::BLINDS);
        assert_eq!(effective(&fx, spec), spec.default);
        fx.params.insert("count".into(), 500.0);
        assert_eq!(effective(&fx, spec), spec.max);
        fx.params.insert("count".into(), f32::NAN);
        assert_eq!(effective(&fx, spec), spec.default);
    }

    #[test]
    fn angles_show_a_degree_sign_and_softness_a_percentage() {
        let wipe = transitions::spec(transitions::LINEAR_WIPE).unwrap();
        assert_eq!(format_value(wipe.param("angle").unwrap(), 90.0), "90 °");
        assert_eq!(format_value(wipe.param("softness").unwrap(), 0.1), "10 %");
    }
}
