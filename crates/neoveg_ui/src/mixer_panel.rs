//! Per-track mixer strip (mute, solo, phase, fader, pan) and the project pan law picker.
//!
//! The panel only returns [`MixAction`]s; `AppState` turns them into undoable commands. A fader
//! drag follows a draft and changes the project once, on release.

use eframe::egui::{self, vec2, RichText};
use neoveg_common::envelope::AutomationMode;
use neoveg_common::i18n::tr;
use neoveg_common::pan::PanLaw;
use neoveg_engine::{AutomationTarget, Track, TrackId, TrackKind, TrackMix};

use crate::icons::{self, Icon};
use crate::precision_slider::PrecisionSlider;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoControl {
    Volume,
    Pan,
    Mute,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MixAction {
    Mute(bool),
    MuteExclusive,
    Solo(bool),
    SoloExclusive,
    Phase(bool),
    Volume(f32),
    Pan(f32),
    PanLaw(PanLaw),
    Automation(AutomationMode),
    /// Commits or updates a single automation keyframe at cursor when stopped in `Touch`/`Latch`.
    AutoPoint(AutoControl, f32),
    /// Ends a live `Touch` pass during playback so accumulated samples become one undo step.
    FinishTouch(AutoControl),
    /// Transitions a live `Latch` gesture to holding `value` until playback stops.
    LatchHold(AutoControl, f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Control {
    Volume,
    Pan,
}

#[derive(Default)]
pub struct MixerPanelState {
    draft: Option<(TrackId, Control, f32)>,
    auto_draft: Option<(TrackId, AutoControl, f32)>,
}

impl MixerPanelState {
    /// Returns `track`'s mix with the in-progress fader or pan drag overlaid for live auditioning.
    pub fn preview_mix(&self, track: &Track) -> Option<TrackMix> {
        let (draft_track, control, value) = self.draft?;
        if draft_track != track.id {
            return None;
        }
        let mut mix = track.mix();
        match control {
            Control::Volume => mix.volume_db = value,
            Control::Pan => mix.pan = value,
        }
        Some(mix)
    }

    /// Returns the in-progress automation control drag on `track_id`, when active.
    pub fn active_auto_draft(&self, track_id: TrackId) -> Option<(AutoControl, f32)> {
        self.auto_draft
            .and_then(|(t, c, v)| (t == track_id).then_some((c, v)))
    }

    pub fn clear_auto_draft(&mut self) {
        self.auto_draft = None;
    }
}

/// `"-6.0 dB"`, `"-∞"` at the bottom of the fader.
pub fn format_db(db: f32) -> String {
    if db <= TrackMix::VOLUME_RANGE_DB.0 {
        "-∞ dB".into()
    } else {
        format!("{db:+.1} dB")
    }
}

/// `"C"`, `"30 E"` (left) or `"30 D"` (right): the pan in percent.
pub fn format_pan(pan: f32) -> String {
    let percent = (pan.abs() * 100.0).round() as i32;
    match percent {
        0 => "C".into(),
        _ if pan < 0.0 => format!("{percent} E"),
        _ => format!("{percent} D"),
    }
}

fn fader(
    ui: &mut egui::Ui,
    state: &mut MixerPanelState,
    track: TrackId,
    control: Control,
    current: f32,
    range: std::ops::RangeInclusive<f32>,
    out: &mut Vec<MixAction>,
) {
    let draft = state.draft.filter(|(t, c, _)| *t == track && *c == control);
    let mut value = draft.map_or(current, |(_, _, v)| v);
    // The readout after the fader (about 64 points) stays in view in a narrow panel
    let width = (crate::fx_panel::visible_width(ui) - 64.0).clamp(60.0, 150.0);
    let response = ui.add(PrecisionSlider::new(&mut value, range, 0.0).width(width));
    if response.changed() {
        state.draft = Some((track, control, value));
    }
    let finished = response.drag_stopped()
        || response.lost_focus()
        || (response.changed() && !response.dragged() && !response.has_focus());
    if finished {
        if let Some((_, _, v)) = state
            .draft
            .take_if(|(t, c, _)| *t == track && *c == control)
        {
            if (v - current).abs() > f32::EPSILON {
                out.push(match control {
                    Control::Volume => MixAction::Volume(v),
                    Control::Pan => MixAction::Pan(v),
                });
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn auto_fader(
    ui: &mut egui::Ui,
    state: &mut MixerPanelState,
    track: TrackId,
    mode: AutomationMode,
    playing: bool,
    control: AutoControl,
    current: f32,
    range: std::ops::RangeInclusive<f32>,
    out: &mut Vec<MixAction>,
) -> f32 {
    let draft = state
        .auto_draft
        .filter(|(t, c, _)| *t == track && *c == control);
    let mut value = draft.map_or(current, |(_, _, v)| v);
    let width = (crate::fx_panel::visible_width(ui) - 64.0).clamp(60.0, 150.0);
    let response = ui.add(PrecisionSlider::new(&mut value, range, 0.0).width(width));
    if response.changed() || response.dragged() {
        state.auto_draft = Some((track, control, value));
    }
    let finished = response.drag_stopped()
        || response.lost_focus()
        || (response.changed() && !response.dragged() && !response.has_focus());
    if finished {
        if let Some((_, _, v)) = state
            .auto_draft
            .take_if(|(t, c, _)| *t == track && *c == control)
        {
            if playing {
                match mode {
                    AutomationMode::Touch => out.push(MixAction::FinishTouch(control)),
                    AutomationMode::Latch => out.push(MixAction::LatchHold(control, v)),
                    _ => {}
                }
            } else {
                out.push(MixAction::AutoPoint(control, v));
            }
        }
    }
    value
}

/// The strip of `track` plus the project's pan law and track automation mode.
pub fn mixer_ui(
    ui: &mut egui::Ui,
    state: &mut MixerPanelState,
    track: &Track,
    law: PanLaw,
    playhead: f64,
    playing: bool,
) -> Vec<MixAction> {
    let mut out = Vec::new();
    let mix = track.mix();
    let mode = track.automation_mode;
    let recording_mode = mode.records();
    let mute_target = if track.kind == TrackKind::Video {
        AutomationTarget::VideoMute
    } else {
        AutomationTarget::AudioMute
    };
    let env_muted = mode.reads_envelopes()
        && track
            .envelopes
            .iter()
            .find(|e| e.enabled && e.target == mute_target)
            .is_some_and(|e| e.value_at(playhead, 0.0) >= 0.5);

    ui.horizontal_wrapped(|ui| {
        let exclusive = ui.input(|i| i.modifiers.command || i.modifiers.shift);
        let mute_active = if recording_mode { env_muted } else { mix.muted };
        if icons::icon_button(
            ui,
            Icon::Mute,
            tr("Mudo (Z; Ctrl+clique = só esta faixa)"),
            mute_active,
        )
        .clicked()
        {
            if recording_mode && !exclusive {
                let next_val = if env_muted { 0.0 } else { 1.0 };
                if playing && mode == AutomationMode::Latch {
                    out.push(MixAction::LatchHold(AutoControl::Mute, next_val));
                } else {
                    out.push(MixAction::AutoPoint(AutoControl::Mute, next_val));
                }
            } else {
                out.push(if exclusive {
                    MixAction::MuteExclusive
                } else {
                    MixAction::Mute(!mix.muted)
                });
            }
        }
        if icons::icon_button(
            ui,
            Icon::Solo,
            tr("Solo (X; Ctrl+clique = só esta faixa)"),
            mix.solo,
        )
        .clicked()
        {
            out.push(if exclusive {
                MixAction::SoloExclusive
            } else {
                MixAction::Solo(!mix.solo)
            });
        }
        if icons::icon_button(ui, Icon::Phase, tr("Inverter a fase"), mix.invert_phase).clicked() {
            out.push(MixAction::Phase(!mix.invert_phase));
        }
        ui.add_space(4.0);
        egui::ComboBox::from_id_salt(("auto_mode", track.id))
            .selected_text(tr(mode.label()))
            .width(142.0)
            .show_ui(ui, |ui| {
                for candidate in AutomationMode::ALL {
                    if ui
                        .selectable_label(mode == candidate, tr(candidate.label()))
                        .clicked()
                    {
                        out.push(MixAction::Automation(candidate));
                    }
                }
            })
            .response
            .on_hover_text(tr(
                "Modo de automação da faixa (Desligada ignora envelopes; Ler segue a curva; Toque grava enquanto arrasta; Trava mantém após soltar até parar)",
            ));
        egui::ComboBox::from_id_salt("pan_law")
            .selected_text(tr(law.label()))
            .width(142.0)
            .show_ui(ui, |ui| {
                for candidate in PanLaw::ALL {
                    if ui
                        .selectable_label(law == candidate, tr(candidate.label()))
                        .clicked()
                    {
                        out.push(MixAction::PanLaw(candidate));
                    }
                }
            })
            .response
            .on_hover_text(tr("Lei de pan do projeto"));
    });

    let vol_env = track
        .envelopes
        .iter()
        .find(|e| e.enabled && e.target == AutomationTarget::Volume)
        .map(|e| e.value_at(playhead, 0.0));
    let pan_env = track
        .envelopes
        .iter()
        .find(|e| e.enabled && e.target == AutomationTarget::Pan)
        .map(|e| e.value_at(playhead, 0.0));

    let (lo, hi) = TrackMix::VOLUME_RANGE_DB;
    if recording_mode && track.kind == TrackKind::Audio {
        let cur_vol = vol_env.unwrap_or(0.0);
        let cur_pan = pan_env.unwrap_or(0.0);
        ui.horizontal(|ui| {
            ui.allocate_ui(vec2(52.0, 18.0), |ui| {
                ui.colored_label(egui::Color32::from_rgb(0xE7, 0x4C, 0x3C), tr("Auto Vol"))
            });
            let shown = auto_fader(
                ui,
                state,
                track.id,
                mode,
                playing,
                AutoControl::Volume,
                cur_vol,
                lo..=hi,
                &mut out,
            );
            ui.monospace(format_db(shown));
        });
        ui.horizontal(|ui| {
            ui.allocate_ui(vec2(52.0, 18.0), |ui| {
                ui.colored_label(egui::Color32::from_rgb(0xE6, 0x7E, 0x22), tr("Auto Pan"))
            });
            let shown = auto_fader(
                ui,
                state,
                track.id,
                mode,
                playing,
                AutoControl::Pan,
                cur_pan,
                -1.0..=1.0,
                &mut out,
            );
            ui.monospace(format_pan(shown));
        });
        ui.weak(format!(
            "{}: {} · {}",
            tr("Trim fixo"),
            format_db(mix.volume_db),
            format_pan(mix.pan)
        ));
    } else {
        let volume = state
            .draft
            .filter(|(t, c, _)| *t == track.id && *c == Control::Volume)
            .map_or(mix.volume_db, |(_, _, v)| v);
        let pan = state
            .draft
            .filter(|(t, c, _)| *t == track.id && *c == Control::Pan)
            .map_or(mix.pan, |(_, _, v)| v);
        ui.horizontal(|ui| {
            ui.allocate_ui(vec2(52.0, 18.0), |ui| ui.label(tr("Volume")));
            fader(
                ui,
                state,
                track.id,
                Control::Volume,
                mix.volume_db,
                lo..=hi,
                &mut out,
            );
            ui.monospace(format_db(volume));
            if mode.reads_envelopes() {
                if let Some(ev_db) = vol_env {
                    ui.weak(format!("({}: {})", tr("Env"), format_db(ev_db)));
                }
            }
        });
        ui.horizontal(|ui| {
            ui.allocate_ui(vec2(52.0, 18.0), |ui| ui.label(tr("Pan")));
            fader(
                ui,
                state,
                track.id,
                Control::Pan,
                mix.pan,
                -1.0..=1.0,
                &mut out,
            );
            ui.monospace(format_pan(pan));
            if mode.reads_envelopes() {
                if let Some(ev_pan) = pan_env {
                    ui.weak(format!("({}: {})", tr("Env"), format_pan(ev_pan)));
                }
            }
        });
    }
    if mix.muted || env_muted {
        ui.label(RichText::new(tr("Faixa muda")).weak());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decibels_and_pan_are_formatted_for_the_strip() {
        assert_eq!(format_db(0.0), "+0.0 dB");
        assert_eq!(format_db(-6.0), "-6.0 dB");
        assert_eq!(format_db(-60.0), "-∞ dB");
        assert_eq!(format_pan(0.0), "C");
        assert_eq!(format_pan(-0.3), "30 E");
        assert_eq!(format_pan(1.0), "100 D");
    }
}
