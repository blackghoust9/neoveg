//! An event envelope in the inspector (the volume of an audio event, the progress of the transition of
//! a video event): a list of keyframes (time in the event, value and the shape to the next one), a button that adds one at the cursor and one that
//! removes the envelope. Edits live in a draft while a value is being dragged and become one undo
//! step when the drag ends (or the field loses focus, or a button is pressed).

use eframe::egui;
use neoveg_common::i18n::tr;
use neoveg_engine::envelope::{EnvelopePoint, EnvelopeShape, TrackEnvelope};
use neoveg_engine::event_volume::{self, MAX_DB, MIN_DB};
use neoveg_engine::transition_progress;

/// What differs between the envelopes the panel edits.
pub struct PanelSpec {
    pub title: &'static str,
    pub create: &'static str,
    /// A new envelope; the argument is the length of the fade it replaces, when there is one.
    pub new_envelope: fn(f64) -> TrackEnvelope,
    /// The value of an envelope at a time, for *Ponto no cursor*.
    pub value_at: fn(&TrackEnvelope, f64) -> f32,
    pub min: f32,
    pub max: f32,
    /// Step of a drag, in the units shown.
    pub speed: f64,
    /// What the stored value is multiplied by to be shown (100 shows a fraction as a percentage).
    pub scale: f32,
    pub suffix: &'static str,
}

fn new_volume(_fade: f64) -> TrackEnvelope {
    event_volume::new_envelope(0.0)
}

fn volume_at(envelope: &TrackEnvelope, time: f64) -> f32 {
    event_volume::db_at(Some(envelope), time)
}

fn progress_now(envelope: &TrackEnvelope, time: f64) -> f32 {
    transition_progress::progress_at(envelope, time)
}

/// The volume of an audio event, in dB.
pub const VOLUME: PanelSpec = PanelSpec {
    title: "Volume do evento",
    create: "Criar envelope de volume",
    new_envelope: new_volume,
    value_at: volume_at,
    min: MIN_DB,
    max: MAX_DB,
    speed: 0.1,
    scale: 1.0,
    suffix: " dB",
};

/// The progress of the transition of a video event, as a percentage.
pub const PROGRESS: PanelSpec = PanelSpec {
    title: "Progresso da transição",
    create: "Criar envelope de progresso",
    new_envelope: transition_progress::new_envelope,
    value_at: progress_now,
    min: 0.0,
    max: 1.0,
    speed: 0.5,
    scale: 100.0,
    suffix: " %",
};

/// What is being edited: the envelope the draft started from, and the draft.
#[derive(Clone, Default)]
struct Draft {
    from: Option<TrackEnvelope>,
    work: Option<TrackEnvelope>,
}

/// Draws the section. `cursor` is the playhead in seconds from the start of the event and `duration`
/// the length of the event; `fade` is the length of the fade a new envelope replaces (`0.0` when
/// unknown). Returns what the user decided to store (`Some(None)` removes the envelope), or `None`
/// while nothing is to be stored.
pub fn show(
    ui: &mut egui::Ui,
    id: egui::Id,
    envelope: Option<&TrackEnvelope>,
    cursor: f64,
    duration: f64,
    fade: f64,
    spec: &PanelSpec,
) -> Option<Option<TrackEnvelope>> {
    ui.label(egui::RichText::new(tr(spec.title)).strong());
    let Some(source) = envelope else {
        if ui.button(tr(spec.create)).clicked() {
            return Some(Some((spec.new_envelope)(fade)));
        }
        return None;
    };
    // A draft only counts for the envelope it started from (an undo or another edit drops it)
    let mut work = ui
        .data(|d| d.get_temp::<Draft>(id))
        .filter(|draft| draft.from.as_ref() == Some(source))
        .and_then(|draft| draft.work)
        .unwrap_or_else(|| source.clone());
    let mut commit = false;
    let mut remove = false;

    ui.horizontal(|ui| {
        commit |= ui
            .checkbox(&mut work.enabled, tr("Ativo"))
            .on_hover_text(tr("Desligado, o evento toca sem o envelope"))
            .changed();
        if ui.button(tr("Remover envelope")).clicked() {
            remove = true;
        }
    });

    let count = work.points.len();
    let mut delete = None;
    for (i, point) in work.points.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            let time = ui.add(
                egui::DragValue::new(&mut point.time)
                    .range(0.0..=duration.max(0.0))
                    .speed(0.01)
                    .suffix(" s"),
            );
            let mut shown = point.value * spec.scale;
            let level = ui.add(
                egui::DragValue::new(&mut shown)
                    .range(spec.min * spec.scale..=spec.max * spec.scale)
                    .speed(spec.speed)
                    .suffix(spec.suffix),
            );
            if level.changed() {
                point.value = shown / spec.scale;
            }
            commit |= time.drag_stopped() || time.lost_focus();
            commit |= level.drag_stopped() || level.lost_focus();
            egui::ComboBox::from_id_salt((id, i))
                .selected_text(point.shape.label())
                .show_ui(ui, |ui| {
                    for shape in EnvelopeShape::ALL {
                        if ui
                            .selectable_value(&mut point.shape, shape, shape.label())
                            .clicked()
                        {
                            commit = true;
                        }
                    }
                });
            if ui
                .add_enabled(count > 1, egui::Button::new("✕"))
                .on_hover_text(tr("Remover este ponto"))
                .clicked()
            {
                delete = Some(i);
            }
        });
    }
    if let Some(i) = delete {
        work.points.remove(i);
        commit = true;
    }
    if ui
        .button(tr("Ponto no cursor"))
        .on_hover_text(tr(
            "Acrescenta um ponto onde o cursor está, com o nível de agora",
        ))
        .clicked()
    {
        let time = cursor.clamp(0.0, duration.max(0.0));
        let value = (spec.value_at)(&work, time);
        work.points.push(EnvelopePoint {
            time,
            value,
            shape: EnvelopeShape::Linear,
        });
        commit = true;
    }

    if remove {
        ui.data_mut(|d| d.remove_temp::<Draft>(id));
        return Some(None);
    }
    if commit {
        work.points.sort_by(|a, b| {
            a.time
                .partial_cmp(&b.time)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        ui.data_mut(|d| d.remove_temp::<Draft>(id));
        return Some(Some(work));
    }
    if work != *source {
        let draft = Draft {
            from: Some(source.clone()),
            work: Some(work),
        };
        ui.data_mut(|d| d.insert_temp(id, draft));
    } else {
        ui.data_mut(|d| d.remove_temp::<Draft>(id));
    }
    None
}
