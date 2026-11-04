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
    dragged: Option<(usize, f64, f32, f32)>,
}

const CANVAS_HEIGHT: f32 = 88.0;
const CANVAS_PAD_X: f32 = 6.0;
const CANVAS_PAD_Y: f32 = 6.0;
const HANDLE_HIT_PX: f32 = 7.0;
const CURVE_STEPS: usize = 20;

fn draw_curve_canvas(
    ui: &mut egui::Ui,
    id: egui::Id,
    work: &mut TrackEnvelope,
    dragged: &mut Option<(usize, f64, f32, f32)>,
    cursor: f64,
    duration: f64,
    spec: &PanelSpec,
) -> bool {
    let mut commit = false;
    let span_t = duration.max(0.01);
    let width = ui.available_width().max(120.0);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(width, CANVAS_HEIGHT),
        egui::Sense::click_and_drag(),
    );
    let painter = ui.painter_at(rect);
    let bg = ui.visuals().extreme_bg_color;
    let border = ui.visuals().widgets.noninteractive.bg_stroke;
    painter.rect_filled(rect, 3.0, bg);
    painter.rect_stroke(rect, 3.0, border);

    let plot = rect.shrink2(egui::vec2(CANVAS_PAD_X, CANVAS_PAD_Y));
    let span_v = (spec.max - spec.min).abs().max(1e-6);
    let to_x = |t: f64| plot.left() + ((t / span_t).clamp(0.0, 1.0) as f32) * plot.width();
    let to_y = |v: f32| {
        let norm = ((v - spec.min) / span_v).clamp(0.0, 1.0);
        plot.top() + (1.0 - norm) * plot.height()
    };
    let from_x = |x: f32| f64::from(((x - plot.left()) / plot.width().max(1.0)).clamp(0.0, 1.0)) * span_t;
    let from_y = |y: f32| {
        let norm = (1.0 - (y - plot.top()) / plot.height().max(1.0)).clamp(0.0, 1.0);
        spec.min + norm * span_v
    };

    // Reference horizontal line (0 dB for volume, 50 % for progress)
    let ref_val = if spec.min < 0.0 && spec.max > 0.0 {
        0.0
    } else {
        0.5 * (spec.min + spec.max)
    };
    let ref_y = to_y(ref_val);
    painter.line_segment(
        [egui::pos2(plot.left(), ref_y), egui::pos2(plot.right(), ref_y)],
        egui::Stroke::new(1.0, egui::Color32::from_gray(65)),
    );

    // Playhead line inside the event
    if (0.0..=span_t).contains(&cursor) {
        let cx = to_x(cursor);
        painter.line_segment(
            [egui::pos2(cx, rect.top() + 1.0), egui::pos2(cx, rect.bottom() - 1.0)],
            egui::Stroke::new(1.0, egui::Color32::from_rgb(0xF3, 0x9C, 0x12)),
        );
    }

    // Pointer interaction: hit-test handles, drag, double-click to insert, right-click menu
    let hovered_idx = response.hover_pos().and_then(|pos| {
        work.points.iter().enumerate().rev().find_map(|(idx, pt)| {
            let px = to_x(pt.time);
            let py = to_y(pt.value);
            ((pos.x - px).abs() <= HANDLE_HIT_PX && (pos.y - py).abs() <= HANDLE_HIT_PX)
                .then_some(idx)
        })
    });
    if dragged.is_some() || hovered_idx.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }

    if response.drag_started_by(egui::PointerButton::Primary) {
        if let Some(pos) = response.interact_pointer_pos() {
            if let Some(idx) = work.points.iter().enumerate().rev().find_map(|(idx, pt)| {
                let px = to_x(pt.time);
                let py = to_y(pt.value);
                ((pos.x - px).abs() <= HANDLE_HIT_PX && (pos.y - py).abs() <= HANDLE_HIT_PX)
                    .then_some(idx)
            }) {
                let pt = work.points[idx];
                *dragged = Some((idx, pt.time, pt.value, pos.y));
            }
        }
    }
    if response.dragged_by(egui::PointerButton::Primary) {
        if let (Some((idx, orig_t, orig_v, press_y)), Some(pos)) =
            (*dragged, response.interact_pointer_pos())
        {
            let mods = ui.input(|i| i.modifiers);
            let new_t = if mods.command && !mods.alt {
                orig_t
            } else {
                from_x(pos.x).clamp(0.0, span_t)
            };
            let new_v = if mods.alt && !mods.command {
                orig_v
            } else if mods.command && !mods.alt {
                let dy = press_y - pos.y;
                (orig_v + (dy / plot.height().max(1.0)) * span_v * 0.1).clamp(spec.min, spec.max)
            } else {
                from_y(pos.y)
            };
            work.move_point(idx, new_t, new_v, spec.min, spec.max);
        }
    }
    if response.drag_stopped() && dragged.take().is_some() {
        commit = true;
    }

    if response.double_clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let t = from_x(pos.x).clamp(0.0, span_t);
            let v = from_y(pos.y);
            let idx = work.points.partition_point(|p| p.time <= t);
            let shape = idx
                .checked_sub(1)
                .and_then(|i| work.points.get(i))
                .map_or(EnvelopeShape::Linear, |p| p.shape);
            work.upsert_point(t, v, shape, spec.min, spec.max, 1e-4);
            commit = true;
        }
    }

    // Context menu on canvas / handle
    let ctx_idx_id = id.with("ctx_pt");
    if response.secondary_clicked() {
        let hit = response.interact_pointer_pos().and_then(|pos| {
            work.points.iter().enumerate().rev().find_map(|(idx, pt)| {
                let px = to_x(pt.time);
                let py = to_y(pt.value);
                ((pos.x - px).abs() <= HANDLE_HIT_PX && (pos.y - py).abs() <= HANDLE_HIT_PX)
                    .then_some(idx)
            })
        });
        ui.data_mut(|d| d.insert_temp(ctx_idx_id, hit));
    }
    response.context_menu(|ui| {
        let hit = ui
            .data(|d| d.get_temp::<Option<usize>>(ctx_idx_id))
            .flatten();
        if let Some(idx) = hit.filter(|&i| i < work.points.len()) {
            let cur_shape = work.points[idx].shape;
            for shape in EnvelopeShape::ALL {
                if ui
                    .selectable_label(cur_shape == shape, tr(shape.label()))
                    .clicked()
                {
                    work.points[idx].shape = shape;
                    commit = true;
                    ui.close_menu();
                }
            }
            if work.points.len() > 1 {
                ui.separator();
                if ui.button(tr("Remover este ponto")).clicked() {
                    work.points.remove(idx);
                    commit = true;
                    ui.close_menu();
                }
            }
        } else if ui.button(tr("Ponto no cursor")).clicked() {
            let t = cursor.clamp(0.0, span_t);
            let v = (spec.value_at)(work, t);
            work.upsert_point(t, v, EnvelopeShape::Linear, spec.min, spec.max, 1e-4);
            commit = true;
            ui.close_menu();
        }
    });

    // Paint curve polyline
    let base_color = if spec.min < 0.0 {
        egui::Color32::from_rgb(0x34, 0x98, 0xDB)
    } else {
        egui::Color32::from_rgb(0xF1, 0xC4, 0x0F)
    };
    let color = if work.enabled {
        base_color
    } else {
        base_color.gamma_multiply(0.4)
    };
    let mut poly: Vec<egui::Pos2> = Vec::new();
    if let Some(first) = work.points.first() {
        if first.time > 0.0 {
            let y = to_y(first.value);
            poly.push(egui::pos2(plot.left(), y));
            poly.push(egui::pos2(to_x(first.time), y));
        }
        for pair in work.points.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            match a.shape {
                EnvelopeShape::Hold => {
                    let ya = to_y(a.value);
                    let yb = to_y(b.value);
                    let xa = to_x(a.time);
                    let xb = to_x(b.time);
                    poly.push(egui::pos2(xa, ya));
                    poly.push(egui::pos2(xb, ya));
                    poly.push(egui::pos2(xb, yb));
                }
                EnvelopeShape::Linear => {
                    poly.push(egui::pos2(to_x(a.time), to_y(a.value)));
                    poly.push(egui::pos2(to_x(b.time), to_y(b.value)));
                }
                shape => {
                    let span = (b.time - a.time).max(1e-6);
                    for step in 0..=CURVE_STEPS {
                        let u = step as f64 / CURVE_STEPS as f64;
                        let t = a.time + u * span;
                        let k = shape.eval(u) as f32;
                        let v = a.value + (b.value - a.value) * k;
                        poly.push(egui::pos2(to_x(t), to_y(v)));
                    }
                }
            }
        }
        if let Some(last) = work.points.last() {
            if last.time < span_t {
                let y = to_y(last.value);
                poly.push(egui::pos2(to_x(last.time), y));
                poly.push(egui::pos2(plot.right(), y));
            }
        }
    }
    if poly.len() >= 2 {
        painter.add(egui::Shape::line(poly, egui::Stroke::new(1.6, color)));
    }

    // Paint keyframe handles
    for (idx, pt) in work.points.iter().enumerate() {
        let active = dragged.is_some_and(|(d, ..)| d == idx) || hovered_idx == Some(idx);
        let sz = if active { 7.0 } else { 5.5 };
        let hrect = egui::Rect::from_center_size(
            egui::pos2(to_x(pt.time), to_y(pt.value)),
            egui::vec2(sz, sz),
        );
        painter.rect_filled(
            hrect,
            0.0,
            if active { egui::Color32::WHITE } else { color },
        );
        painter.rect_stroke(
            hrect,
            0.0,
            egui::Stroke::new(1.0, egui::Color32::from_black_alpha(200)),
        );
    }

    response.on_hover_text(tr(
        "Clique duplo na curva adiciona um ponto; arraste pontos (Ctrl = ajuste fino, Alt = só tempo).",
    ));
    commit
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
    let stored_draft = ui
        .data(|d| d.get_temp::<Draft>(id))
        .filter(|draft| draft.from.as_ref() == Some(source));
    let mut dragged = stored_draft.as_ref().and_then(|d| d.dragged);
    let mut work = stored_draft
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

    commit |= draw_curve_canvas(ui, id, &mut work, &mut dragged, cursor, duration, spec);

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
    if work != *source || dragged.is_some() {
        let draft = Draft {
            from: Some(source.clone()),
            work: Some(work),
            dragged,
        };
        ui.data_mut(|d| d.insert_temp(id, draft));
    } else {
        ui.data_mut(|d| d.remove_temp::<Draft>(id));
    }
    None
}
