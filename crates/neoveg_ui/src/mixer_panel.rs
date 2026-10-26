//! Per-track mixer strip (mute, solo, phase, fader, pan) and the project pan law picker.
//!
//! The panel only returns [`MixAction`]s; `AppState` turns them into undoable commands. A fader
//! drag follows a draft and changes the project once, on release.

use eframe::egui::{self, vec2, RichText};
use neoveg_common::pan::PanLaw;
use neoveg_engine::{Track, TrackId, TrackMix};

use crate::icons::{self, Icon};
use crate::precision_slider::PrecisionSlider;

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
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Control {
    Volume,
    Pan,
}

#[derive(Default)]
pub struct MixerPanelState {
    draft: Option<(TrackId, Control, f32)>,
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
    let response = ui.add(PrecisionSlider::new(&mut value, range, 0.0).width(150.0));
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

/// The strip of `track` plus the project's pan law.
pub fn mixer_ui(
    ui: &mut egui::Ui,
    state: &mut MixerPanelState,
    track: &Track,
    law: PanLaw,
) -> Vec<MixAction> {
    let mut out = Vec::new();
    let mix = track.mix();
    ui.horizontal(|ui| {
        let exclusive = ui.input(|i| i.modifiers.command || i.modifiers.shift);
        if icons::icon_button(
            ui,
            Icon::Mute,
            "Mudo (Z; Ctrl+clique = só esta faixa)",
            mix.muted,
        )
        .clicked()
        {
            out.push(if exclusive {
                MixAction::MuteExclusive
            } else {
                MixAction::Mute(!mix.muted)
            });
        }
        if icons::icon_button(
            ui,
            Icon::Solo,
            "Solo (X; Ctrl+clique = só esta faixa)",
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
        if icons::icon_button(ui, Icon::Phase, "Inverter a fase", mix.invert_phase).clicked() {
            out.push(MixAction::Phase(!mix.invert_phase));
        }
        ui.add_space(8.0);
        egui::ComboBox::from_id_salt("pan_law")
            .selected_text(law.label())
            .width(150.0)
            .show_ui(ui, |ui| {
                for candidate in PanLaw::ALL {
                    if ui
                        .selectable_label(law == candidate, candidate.label())
                        .clicked()
                    {
                        out.push(MixAction::PanLaw(candidate));
                    }
                }
            })
            .response
            .on_hover_text("Lei de pan do projeto");
    });
    let volume = state
        .draft
        .filter(|(t, c, _)| *t == track.id && *c == Control::Volume)
        .map_or(mix.volume_db, |(_, _, v)| v);
    let pan = state
        .draft
        .filter(|(t, c, _)| *t == track.id && *c == Control::Pan)
        .map_or(mix.pan, |(_, _, v)| v);
    let (lo, hi) = TrackMix::VOLUME_RANGE_DB;
    ui.horizontal(|ui| {
        ui.allocate_ui(vec2(52.0, 18.0), |ui| ui.label("Volume"));
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
    });
    ui.horizontal(|ui| {
        ui.allocate_ui(vec2(52.0, 18.0), |ui| ui.label("Pan"));
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
    });
    if mix.muted {
        ui.label(RichText::new("Faixa muda").weak());
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
