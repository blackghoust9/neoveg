//! The velocity editor of a video event: a small graph of the speed over the length of the event.
//!
//! The horizontal axis is the time inside the event, the vertical one the speed from `-100 %`
//! (backwards) to `300 %`, with the lines of `0 %` (freeze) and `100 %` (normal) marked. Points are
//! dragged, added with a double click and removed or set to a preset from their context menu. The
//! geometry is made of pure functions; the widget returns [`VelocityAction`]s: `Draft` while a point
//! is dragged (the preview shows it, the project is not touched) and `Commit` when the gesture ends,
//! which `AppState` turns into one undo step.

use eframe::egui::{self, pos2, vec2, Color32, Pos2, Rect, Sense, Shape, Stroke};
use neoveg_common::i18n::tr;
use neoveg_engine::velocity::{self, MAX_SPEED, MIN_SPEED, PRESETS};
use neoveg_engine::{EnvelopeShape, EventId, TrackEnvelope, TrackId};

/// How close (in pixels) the pointer has to be to grab a point.
const GRAB_RADIUS: f32 = 8.0;
/// Room above and below the range so the lines of the extremes are not on the border.
const MARGIN: f32 = 8.0;

/// What the editor asks of the application.
#[derive(Debug, Clone, PartialEq)]
pub enum VelocityAction {
    /// Show this envelope in the preview now; the project is not changed.
    Draft(Option<TrackEnvelope>),
    /// The gesture ended: make this the velocity envelope of the event (one undo step).
    Commit(Option<TrackEnvelope>),
    /// Freeze the picture at the cursor (a keyframe pair, see `velocity::freeze_at`).
    Freeze,
}

/// A live velocity envelope that the preview must show instead of the project's.
#[derive(Debug, Clone, PartialEq)]
pub struct VelocityDraft {
    pub track_id: TrackId,
    pub event_id: EventId,
    pub envelope: Option<TrackEnvelope>,
}

impl VelocityDraft {
    /// A number that changes whenever the draft does (part of the preview's frame key).
    pub fn stamp(draft: Option<&VelocityDraft>) -> u64 {
        use std::hash::{Hash, Hasher};
        let Some(d) = draft else {
            return 0;
        };
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (d.track_id, d.event_id, d.envelope.is_some()).hash(&mut h);
        if let Some(env) = &d.envelope {
            env.enabled.hash(&mut h);
            for p in &env.points {
                p.time.to_bits().hash(&mut h);
                p.value.to_bits().hash(&mut h);
                (p.shape as u8).hash(&mut h);
            }
        }
        h.finish() | 1
    }
}

// ---- geometry ----------------------------------------------------------------------------------

fn inner(rect: Rect) -> Rect {
    rect.shrink2(vec2(0.0, MARGIN))
}

/// Screen x of `time` seconds into an event of `duration` seconds.
pub fn x_of(rect: Rect, duration: f64, time: f64) -> f32 {
    rect.left() + (time / duration.max(1e-6)).clamp(0.0, 1.0) as f32 * rect.width()
}

/// Screen y of `speed`.
pub fn y_of(rect: Rect, speed: f32) -> f32 {
    let r = inner(rect);
    let u = (speed.clamp(MIN_SPEED, MAX_SPEED) - MIN_SPEED) / (MAX_SPEED - MIN_SPEED);
    r.bottom() - u * r.height()
}

/// Time at screen x.
pub fn time_of(rect: Rect, duration: f64, x: f32) -> f64 {
    f64::from(((x - rect.left()) / rect.width().max(1.0)).clamp(0.0, 1.0)) * duration
}

/// Speed at screen y, snapped to the lines of `0 %` and `100 %` when within a few pixels.
pub fn speed_of(rect: Rect, y: f32, snap: bool) -> f32 {
    let r = inner(rect);
    let u = ((r.bottom() - y) / r.height().max(1.0)).clamp(0.0, 1.0);
    let speed = MIN_SPEED + u * (MAX_SPEED - MIN_SPEED);
    if snap {
        for line in [0.0_f32, 1.0, -1.0, 2.0, 3.0] {
            if (y_of(rect, line) - y).abs() <= 4.0 {
                return line;
            }
        }
    }
    speed
}

/// Index of the point under `pos`, the nearest one when several are in reach.
pub fn hit_point(env: &TrackEnvelope, rect: Rect, duration: f64, pos: Pos2) -> Option<usize> {
    env.points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let at = pos2(x_of(rect, duration, p.time), y_of(rect, p.value));
            (i, at.distance(pos))
        })
        .filter(|(_, d)| *d <= GRAB_RADIUS)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// The envelope with point `index` dragged to `pos`; it never crosses its neighbours.
pub fn with_point_moved(
    env: &TrackEnvelope,
    index: usize,
    rect: Rect,
    duration: f64,
    pos: Pos2,
    snap: bool,
) -> TrackEnvelope {
    let mut next = env.clone();
    let time = time_of(rect, duration, pos.x);
    let speed = speed_of(rect, pos.y, snap);
    // The first point stays at the start of the event
    let time = if index == 0 { 0.0 } else { time };
    next.move_point(index, time, speed, MIN_SPEED, MAX_SPEED);
    next
}

/// The envelope with a point added where the pointer is.
pub fn with_point_added(
    env: &TrackEnvelope,
    rect: Rect,
    duration: f64,
    pos: Pos2,
) -> TrackEnvelope {
    let mut next = env.clone();
    next.upsert_point(
        time_of(rect, duration, pos.x),
        speed_of(rect, pos.y, true),
        EnvelopeShape::Linear,
        MIN_SPEED,
        MAX_SPEED,
        duration / 200.0,
    );
    next
}

/// The envelope without point `index`; `None` when it was the last one (no envelope left).
pub fn with_point_removed(env: &TrackEnvelope, index: usize) -> Option<TrackEnvelope> {
    let mut next = env.clone();
    next.remove_point(index);
    (!next.points.is_empty()).then_some(next)
}

/// The envelope with point `index` set to `speed`.
pub fn with_point_speed(env: &TrackEnvelope, index: usize, speed: f32) -> TrackEnvelope {
    let mut next = env.clone();
    if let Some(p) = next.points.get_mut(index) {
        p.value = speed.clamp(MIN_SPEED, MAX_SPEED);
    }
    next
}

/// An envelope that holds `speed` for the whole event.
pub fn constant(speed: f32) -> TrackEnvelope {
    velocity::new_envelope(speed)
}

// ---- the widget --------------------------------------------------------------------------------

/// What the widget needs to know about the event.
pub struct VelocityView<'a> {
    pub envelope: Option<&'a TrackEnvelope>,
    pub duration: f64,
    /// Seconds from the start of the event to the cursor, clamped to the event.
    pub time: f64,
}

#[derive(Debug, Default)]
pub struct VelocityEditor {
    /// The event this state belongs to: another event starts from a clean slate.
    owner: Option<(TrackId, EventId)>,
    selected: Option<usize>,
    dragging: Option<usize>,
    draft: Option<TrackEnvelope>,
    /// Where the context menu was opened (the pointer moves while the menu is up).
    menu_at: Option<Pos2>,
}

fn percent(speed: f32) -> String {
    format!("{:.0} %", speed * 100.0)
}

impl VelocityEditor {
    /// Forgets the selected point, a drag in progress and the draft (the section went out of sight,
    /// or another event was chosen).
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Makes `owner` the event this editor works on; `true` when that was a change (state dropped).
    fn follow(&mut self, owner: (TrackId, EventId)) -> bool {
        if self.owner == Some(owner) {
            return false;
        }
        self.reset();
        self.owner = Some(owner);
        true
    }

    /// Draws the editor of the velocity of event `owner` and returns what the user did.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        owner: (TrackId, EventId),
        view: &VelocityView,
    ) -> Vec<VelocityAction> {
        self.follow(owner);
        let mut actions = Vec::new();
        let shown = self.draft.clone().or_else(|| view.envelope.cloned());
        match &shown {
            None => {
                ui.weak(tr("Sem envelope: o evento toca na velocidade normal (100 %). A duração do evento não muda."));
                ui.horizontal_wrapped(|ui| {
                    for (label, speed) in PRESETS {
                        if ui.button(label).clicked() {
                            actions.push(VelocityAction::Commit(Some(constant(speed))));
                        }
                    }
                });
                if ui.button(tr("Congelar quadro no cursor")).clicked() {
                    actions.push(VelocityAction::Freeze);
                }
            }
            Some(env) => self.editor(ui, view, env, &mut actions),
        }
        actions
    }

    fn editor(
        &mut self,
        ui: &mut egui::Ui,
        view: &VelocityView,
        env: &TrackEnvelope,
        actions: &mut Vec<VelocityAction>,
    ) {
        ui.horizontal_wrapped(|ui| {
            let mut enabled = env.enabled;
            if ui.checkbox(&mut enabled, tr("Ligado")).changed() {
                let mut next = env.clone();
                next.enabled = enabled;
                actions.push(VelocityAction::Commit(Some(next)));
            }
            if ui.button(tr("Congelar quadro no cursor")).clicked() {
                actions.push(VelocityAction::Freeze);
            }
            if ui.button(tr("Remover envelope")).clicked() {
                self.selected = None;
                actions.push(VelocityAction::Commit(None));
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(tr("Ponto selecionado:"));
            for (label, speed) in PRESETS {
                let on = self.selected.is_some();
                if ui.add_enabled(on, egui::Button::new(label)).clicked() {
                    if let Some(i) = self.selected {
                        actions.push(VelocityAction::Commit(Some(with_point_speed(
                            env, i, speed,
                        ))));
                    }
                }
            }
        });

        let width = ui.available_width().max(180.0);
        let (response, painter) = ui.allocate_painter(vec2(width, 150.0), Sense::click_and_drag());
        let rect = response.rect;
        painter.rect_filled(rect, 0.0, Color32::from_gray(24));
        let accent = Color32::from_rgb(0xF1, 0xC4, 0x0F);

        // The lines of the main speeds
        for (speed, label) in [
            (-1.0_f32, "-100 %"),
            (0.0, "0 %"),
            (1.0, "100 %"),
            (2.0, "200 %"),
            (3.0, "300 %"),
        ] {
            let y = y_of(rect, speed);
            let strong = speed == 0.0 || speed == 1.0;
            painter.line_segment(
                [pos2(rect.left(), y), pos2(rect.right(), y)],
                Stroke::new(1.0_f32, Color32::from_gray(if strong { 70 } else { 40 })),
            );
            painter.text(
                pos2(rect.left() + 3.0, y - 1.0),
                egui::Align2::LEFT_BOTTOM,
                label,
                egui::FontId::proportional(10.0),
                Color32::from_gray(120),
            );
        }
        // The curve
        let steps = (rect.width() / 3.0).max(16.0) as usize;
        let curve: Vec<Pos2> = (0..=steps)
            .map(|i| {
                let t = view.duration * i as f64 / steps as f64;
                let speed = velocity::rate_at(Some(env), t) as f32;
                pos2(x_of(rect, view.duration, t), y_of(rect, speed))
            })
            .collect();
        let line = if env.enabled {
            accent
        } else {
            accent.gamma_multiply(0.4)
        };
        painter.add(Shape::line(curve, Stroke::new(2.0_f32, line)));
        for (i, p) in env.points.iter().enumerate() {
            let at = pos2(x_of(rect, view.duration, p.time), y_of(rect, p.value));
            let on = self.selected == Some(i);
            painter.rect_filled(
                Rect::from_center_size(at, vec2(8.0, 8.0)),
                0.0,
                if on { Color32::WHITE } else { line },
            );
        }
        // The cursor
        let cursor = x_of(rect, view.duration, view.time);
        painter.line_segment(
            [pos2(cursor, rect.top()), pos2(cursor, rect.bottom())],
            Stroke::new(1.0_f32, Color32::from_rgb(0x29, 0x80, 0xB9)),
        );

        // Readout: the speed and the place in the source at the cursor
        let speed_now = velocity::rate_at(Some(env), view.time) as f32;
        let offset = velocity::source_offset(Some(env), view.time);
        ui.weak(format!(
            "No cursor: velocidade {}, {:.2} s da origem tocados até aqui",
            percent(speed_now),
            offset
        ));

        // Mouse
        let snap = ui.input(|i| !i.modifiers.alt);
        if let Some(pos) = response.hover_pos() {
            if hit_point(env, rect, view.duration, pos).is_some() {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grab);
            }
        }
        if response.drag_started() {
            if let Some(press) = ui.input(|i| i.pointer.press_origin()) {
                if let Some(i) = hit_point(env, rect, view.duration, press) {
                    self.dragging = Some(i);
                    self.selected = Some(i);
                }
            }
        }
        if let (Some(i), Some(pos)) = (self.dragging, response.interact_pointer_pos()) {
            let next = with_point_moved(env, i, rect, view.duration, pos, snap);
            if response.drag_stopped() {
                self.draft = None;
                actions.push(VelocityAction::Commit(Some(next)));
            } else {
                self.draft = Some(next.clone());
                actions.push(VelocityAction::Draft(Some(next)));
            }
        }
        if response.drag_stopped() {
            if self.dragging.take().is_some() && self.draft.is_some() {
                // The release had no pointer position: end the gesture with what was drafted
                if let Some(d) = self.draft.take() {
                    actions.push(VelocityAction::Commit(Some(d)));
                }
            }
            actions.push(VelocityAction::Draft(None));
        }
        if response.double_clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                if hit_point(env, rect, view.duration, pos).is_none() {
                    actions.push(VelocityAction::Commit(Some(with_point_added(
                        env,
                        rect,
                        view.duration,
                        pos,
                    ))));
                }
            }
        } else if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                self.selected = hit_point(env, rect, view.duration, pos);
            }
        }
        if response.secondary_clicked() {
            self.menu_at = response.interact_pointer_pos();
        }
        let menu_at = self.menu_at.unwrap_or_else(|| rect.center());
        response.context_menu(|ui| {
            let pos = menu_at;
            match hit_point(env, rect, view.duration, pos) {
                Some(i) => {
                    for (label, speed) in PRESETS {
                        if ui.button(label).clicked() {
                            actions.push(VelocityAction::Commit(Some(with_point_speed(
                                env, i, speed,
                            ))));
                            ui.close_menu();
                        }
                    }
                    ui.separator();
                    for shape in EnvelopeShape::ALL {
                        if ui.button(format!("Curva: {}", shape.label())).clicked() {
                            let mut next = env.clone();
                            next.points[i].shape = shape;
                            actions.push(VelocityAction::Commit(Some(next)));
                            ui.close_menu();
                        }
                    }
                    ui.separator();
                    if ui.button(tr("Remover ponto")).clicked() {
                        self.selected = None;
                        actions.push(VelocityAction::Commit(with_point_removed(env, i)));
                        ui.close_menu();
                    }
                }
                None => {
                    if ui.button(tr("Adicionar ponto aqui")).clicked() {
                        actions.push(VelocityAction::Commit(Some(with_point_added(
                            env,
                            rect,
                            view.duration,
                            pos,
                        ))));
                        ui.close_menu();
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neoveg_engine::EnvelopePoint;

    fn plot() -> Rect {
        Rect::from_min_size(pos2(100.0, 50.0), vec2(400.0, 150.0))
    }

    fn env(points: &[(f64, f32)]) -> TrackEnvelope {
        let mut e = velocity::new_envelope(1.0);
        e.points = points
            .iter()
            .map(|&(time, value)| EnvelopePoint {
                time,
                value,
                shape: EnvelopeShape::Linear,
            })
            .collect();
        e
    }

    #[test]
    fn the_axes_map_time_and_speed_and_back() {
        let r = plot();
        assert_eq!(x_of(r, 10.0, 0.0), 100.0);
        assert_eq!(x_of(r, 10.0, 10.0), 500.0);
        assert_eq!(x_of(r, 10.0, 25.0), 500.0, "past the end stays on the edge");
        // 100 % sits half way up the range from -100 % to 300 %
        let y = y_of(r, 1.0);
        assert!((y - (inner(r).top() + inner(r).height() / 2.0)).abs() < 1e-3);
        assert!(
            y_of(r, 3.0) < y_of(r, 0.0) && y_of(r, 0.0) < y_of(r, -1.0),
            "up is faster"
        );
        assert!((time_of(r, 10.0, 300.0) - 5.0).abs() < 1e-9);
        for speed in [-1.0, 0.0, 0.7, 2.0, 3.0] {
            let back = speed_of(r, y_of(r, speed), false);
            assert!((back - speed).abs() < 1e-4, "{speed} came back as {back}");
        }
        assert_eq!(
            speed_of(r, y_of(r, 1.0) + 3.0, true),
            1.0,
            "snaps to the 100 % line"
        );
        assert_ne!(speed_of(r, y_of(r, 1.0) + 3.0, false), 1.0);
    }

    #[test]
    fn the_pointer_finds_the_nearest_point() {
        let r = plot();
        let e = env(&[(0.0, 1.0), (5.0, 2.0)]);
        let second = pos2(x_of(r, 10.0, 5.0), y_of(r, 2.0));
        assert_eq!(hit_point(&e, r, 10.0, second + vec2(3.0, 2.0)), Some(1));
        assert_eq!(hit_point(&e, r, 10.0, pos2(300.0, 60.0)), None);
        assert_eq!(
            hit_point(&e, r, 10.0, pos2(x_of(r, 10.0, 0.0), y_of(r, 1.0))),
            Some(0)
        );
    }

    #[test]
    fn dragging_moves_a_point_inside_its_neighbours_and_the_first_one_stays_at_zero() {
        let r = plot();
        let e = env(&[(0.0, 1.0), (4.0, 2.0), (8.0, 0.5)]);
        let moved = with_point_moved(
            &e,
            1,
            r,
            10.0,
            pos2(x_of(r, 10.0, 5.0), y_of(r, 3.0)),
            false,
        );
        assert!((moved.points[1].time - 5.0).abs() < 1e-3);
        assert!((moved.points[1].value - 3.0).abs() < 1e-3);
        // Past the next point it stops just short of it
        let stuck = with_point_moved(
            &e,
            1,
            r,
            10.0,
            pos2(x_of(r, 10.0, 9.5), y_of(r, 2.0)),
            false,
        );
        assert!(stuck.points[1].time < 8.0);
        // The first point cannot leave the start
        let first = with_point_moved(
            &e,
            0,
            r,
            10.0,
            pos2(x_of(r, 10.0, 3.0), y_of(r, 0.0)),
            false,
        );
        assert_eq!(first.points[0].time, 0.0);
        assert!(first.points[0].value.abs() < 1e-3);
        // Values stay inside the speed range even with the pointer far away
        let wild = with_point_moved(&e, 2, r, 10.0, pos2(9999.0, -9999.0), false);
        assert!(wild.points[2].value <= MAX_SPEED);
    }

    #[test]
    fn a_point_is_added_where_clicked_and_removed_on_request_and_the_last_one_ends_the_envelope() {
        let r = plot();
        let e = env(&[(0.0, 1.0), (8.0, 1.0)]);
        let added = with_point_added(&e, r, 10.0, pos2(x_of(r, 10.0, 4.0), y_of(r, 2.0)));
        assert_eq!(added.points.len(), 3);
        assert!((added.points[1].time - 4.0).abs() < 0.05);
        assert!(
            (added.points[1].value - 2.0).abs() < 0.01,
            "snapped to the 200 % line"
        );
        let removed = with_point_removed(&added, 1).unwrap();
        assert_eq!(removed.points.len(), 2);
        let single = env(&[(0.0, 1.0)]);
        assert!(with_point_removed(&single, 0).is_none());
    }

    #[test]
    fn a_preset_changes_the_speed_of_a_point_and_a_constant_envelope_holds_it() {
        let e = env(&[(0.0, 1.0), (4.0, 1.0)]);
        let set = with_point_speed(&e, 1, -1.0);
        assert_eq!(set.points[1].value, -1.0);
        assert_eq!(
            with_point_speed(&e, 9, 2.0),
            e,
            "a missing point changes nothing"
        );
        let c = constant(0.5);
        assert_eq!(c.points.len(), 1);
        assert_eq!(velocity::rate_at(Some(&c), 7.0), 0.5);
        assert_eq!(PRESETS.len(), 6);
    }

    #[test]
    fn another_event_starts_with_no_selection_no_drag_and_no_draft() {
        let mut editor = VelocityEditor::default();
        assert!(editor.follow((1, 2)), "the first event is a change");
        assert!(!editor.follow((1, 2)), "the same event keeps its state");
        editor.selected = Some(1);
        editor.dragging = Some(1);
        editor.draft = Some(env(&[(0.0, 2.0)]));
        editor.menu_at = Some(pos2(5.0, 5.0));
        assert!(!editor.follow((1, 2)));
        assert_eq!(editor.selected, Some(1));
        assert!(editor.follow((1, 3)), "another event");
        assert_eq!(editor.selected, None);
        assert_eq!(editor.dragging, None);
        assert_eq!(editor.draft, None);
        assert_eq!(editor.menu_at, None);
        editor.selected = Some(0);
        editor.reset();
        assert_eq!(editor.selected, None);
        assert_eq!(editor.owner, None);
    }

    #[test]
    fn the_draft_stamp_follows_every_number_of_the_envelope() {
        let d = |v: f32| VelocityDraft {
            track_id: 1,
            event_id: 2,
            envelope: Some(env(&[(0.0, 1.0), (3.0, v)])),
        };
        assert_eq!(VelocityDraft::stamp(None), 0);
        assert_ne!(
            VelocityDraft::stamp(Some(&d(2.0))),
            VelocityDraft::stamp(Some(&d(2.01)))
        );
        assert_eq!(
            VelocityDraft::stamp(Some(&d(2.0))),
            VelocityDraft::stamp(Some(&d(2.0)))
        );
    }
}
