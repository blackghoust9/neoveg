//! Precision NLE slider: flat square thumb, double-click reset, Ctrl for 1/10 sensitivity and a
//! right-click menu with "Restaurar Padrão" and "Digitar Valor...".

use std::ops::RangeInclusive;

use eframe::egui::{
    pos2, vec2, Color32, Key, Response, Rect, Rounding, Sense, TextEdit, Ui, Widget,
};

const THUMB_WIDTH: f32 = 8.0;
const HEIGHT: f32 = 18.0;
const FINE_SENSITIVITY: f32 = 0.1;

pub struct PrecisionSlider<'a> {
    value: &'a mut f32,
    min: f32,
    max: f32,
    default: f32,
    width: f32,
}

impl<'a> PrecisionSlider<'a> {
    pub fn new(value: &'a mut f32, range: RangeInclusive<f32>, default: f32) -> Self {
        Self { value, min: *range.start(), max: *range.end(), default, width: 160.0 }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
}

impl Widget for PrecisionSlider<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let Self { value, min, max, default, width } = self;
        let (rect, mut response) = ui.allocate_exact_size(vec2(width, HEIGHT), Sense::click_and_drag());
        let typed_id = response.id.with("typed_value");

        // "Digitar Valor..." replaces the slider with a text box until Enter / Esc / focus loss
        if let Some(mut text) = ui.data_mut(|d| d.get_temp::<String>(typed_id)) {
            let edit = ui.put(rect, TextEdit::singleline(&mut text).desired_width(width));
            if !edit.has_focus() && !edit.lost_focus() {
                edit.request_focus();
            }
            if edit.lost_focus() {
                if ui.input(|i| i.key_pressed(Key::Enter)) {
                    if let Ok(v) = text.trim().replace(',', ".").parse::<f32>() {
                        *value = v.clamp(min, max);
                        response.mark_changed();
                    }
                }
                ui.data_mut(|d| d.remove::<String>(typed_id));
            } else {
                ui.data_mut(|d| d.insert_temp(typed_id, text));
            }
            return response;
        }

        let span = (max - min).max(f32::EPSILON);
        let travel = (rect.width() - THUMB_WIDTH).max(1.0);
        let fine = ui.input(|i| i.modifiers.ctrl);
        let mut changed = false;

        if response.double_clicked() {
            *value = default;
            changed = true;
        } else if response.drag_started() && !fine {
            if let Some(p) = response.interact_pointer_pos() {
                let t = ((p.x - rect.left() - THUMB_WIDTH / 2.0) / travel).clamp(0.0, 1.0);
                *value = min + t * span;
                changed = true;
            }
        } else if response.dragged() {
            let sensitivity = if fine { FINE_SENSITIVITY } else { 1.0 };
            *value = (*value + response.drag_delta().x / travel * span * sensitivity).clamp(min, max);
            changed = true;
        }

        response.context_menu(|ui| {
            if ui.button("Restaurar Padrão").clicked() {
                *value = default;
                changed = true;
                ui.close_menu();
            }
            if ui.button("Digitar Valor...").clicked() {
                let current = format!("{:.3}", *value);
                ui.data_mut(|d| d.insert_temp(typed_id, current));
                ui.close_menu();
            }
        });

        if changed {
            response.mark_changed();
        }

        let t = ((*value - min) / span).clamp(0.0, 1.0);
        let x = rect.left() + THUMB_WIDTH / 2.0 + t * travel;
        let painter = ui.painter();
        let track = Rect::from_center_size(rect.center(), vec2(rect.width(), 4.0));
        painter.rect_filled(track, Rounding::ZERO, Color32::from_gray(38));
        painter.rect_filled(
            Rect::from_min_max(track.min, pos2(x, track.max.y)),
            Rounding::ZERO,
            Color32::from_rgb(41, 128, 185),
        );
        let thumb_color = if response.hovered() || response.dragged() {
            Color32::from_gray(235)
        } else {
            Color32::from_gray(200)
        };
        let thumb = Rect::from_center_size(pos2(x, rect.center().y), vec2(THUMB_WIDTH, rect.height() - 2.0));
        painter.rect_filled(thumb, Rounding::ZERO, thumb_color);

        response.on_hover_text("Duplo clique: padrão · Ctrl: ajuste fino · Botão direito: opções")
    }
}
