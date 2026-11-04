//! The editor of the colour curves effect: a square from black to white where the master curve and
//! the red, green and blue curves are shaped by dragging their five control points.
//!
//! The effect keeps each point as two plain numbers (`m_x1`, `m_y1`, ...), see
//! `neoveg_video::vfx::COLOR_CURVES`. Dragging a point moves both at once, so the editor hands
//! the pair back to the panel ([`CurveOutput::Live`] while the mouse is down, [`CurveOutput::Commit`]
//! on release), and the panel makes the release one undo step. The rules that do not need a window
//! (which point is hit, how far a point may go) are plain functions with tests.

use eframe::egui::{self, pos2, vec2, Color32, Sense, Shape, Stroke};
use neoveg_common::i18n::tr;
use neoveg_engine::TrackEffect;
use neoveg_video::vfx::{self, CURVE_POINTS};

use crate::video_fx_panel::effective_value;

/// Parameter prefix and label of each curve, the master first.
pub const CHANNELS: [(&str, &str); 4] = [
    ("m", "Mestre"),
    ("r", "Vermelho"),
    ("g", "Verde"),
    ("b", "Azul"),
];

/// How close (in points of the screen) the pointer has to be to grab a control point.
const GRAB_RADIUS: f32 = 11.0;

/// Smallest distance in x between two neighbouring points.
const MIN_GAP: f32 = 0.01;

/// What the editor needs to remember between frames.
#[derive(Debug, Default)]
pub struct CurveState {
    /// The curve being edited (index into [`CHANNELS`]).
    pub channel: usize,
    /// The point under the mouse button: the curve and the point.
    drag: Option<(usize, usize)>,
    /// The point shown in the read-out.
    selected: Option<usize>,
}

/// What the user did in the editor.
#[derive(Debug, Clone, PartialEq)]
pub enum CurveOutput {
    /// A point is being dragged: show these values, change nothing yet.
    Live(Vec<(String, f32)>),
    /// The drag ended, or a point or a curve was restored: these values are the edit.
    Commit(Vec<(String, f32)>),
}

/// The id of the parameter holding `axis` (`'x'` or `'y'`) of `point` of `channel`.
pub fn param_id(channel: usize, point: usize, axis: char) -> String {
    format!("{}_{axis}{}", CHANNELS[channel].0, point + 1)
}

/// The value of parameter `id`: the dragged one when `draft` has it, otherwise the stored one.
fn value_of(effect: &TrackEffect, draft: &[(String, f32)], id: &str) -> f32 {
    if let Some((_, v)) = draft.iter().find(|(k, _)| k == id) {
        return *v;
    }
    vfx::spec(vfx::COLOR_CURVES)
        .and_then(|spec| spec.param(id))
        .map_or(0.0, |p| effective_value(effect, p))
}

/// The control points of `channel` as the effect runs them (`x0, y0, x1, y1, ...`).
pub fn read_points(
    effect: &TrackEffect,
    channel: usize,
    draft: &[(String, f32)],
) -> [f32; 2 * CURVE_POINTS] {
    let mut points = [0.0; 2 * CURVE_POINTS];
    for point in 0..CURVE_POINTS {
        points[2 * point] = value_of(effect, draft, &param_id(channel, point, 'x'));
        points[2 * point + 1] = value_of(effect, draft, &param_id(channel, point, 'y'));
    }
    points
}

/// The values that put one point of `channel` back where a straight curve has it.
pub fn default_point(channel: usize, point: usize) -> Vec<(String, f32)> {
    let straight = point as f32 / (CURVE_POINTS - 1) as f32;
    vec![
        (param_id(channel, point, 'x'), straight),
        (param_id(channel, point, 'y'), straight),
    ]
}

/// The values that put the whole curve back to a straight line.
pub fn default_curve(channel: usize) -> Vec<(String, f32)> {
    (0..CURVE_POINTS)
        .flat_map(|point| default_point(channel, point))
        .collect()
}

/// The point nearest to `at` (both in `0.0..=1.0`, y up) within `reach` (the grab radius as a
/// fraction of the square), if any.
pub fn nearest_point(
    points: &[f32; 2 * CURVE_POINTS],
    at: (f32, f32),
    reach: f32,
) -> Option<usize> {
    (0..CURVE_POINTS)
        .map(|i| {
            let (dx, dy) = (points[2 * i] - at.0, points[2 * i + 1] - at.1);
            (i, (dx * dx + dy * dy).sqrt())
        })
        .filter(|(_, d)| *d <= reach)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// Where point `i` ends up when dragged to `(x, y)`: inside the square and between its
/// neighbours, so the order of the points never changes under the mouse.
pub fn drag_point(points: &[f32; 2 * CURVE_POINTS], i: usize, x: f32, y: f32) -> (f32, f32) {
    let low = if i == 0 {
        0.0
    } else {
        points[2 * (i - 1)] + MIN_GAP
    };
    let high = if i + 1 == CURVE_POINTS {
        1.0
    } else {
        points[2 * (i + 1)] - MIN_GAP
    };
    (x.clamp(low.min(high), high), y.clamp(0.0, 1.0))
}

fn channel_color(ui: &egui::Ui, channel: usize) -> Color32 {
    match channel {
        1 => Color32::from_rgb(225, 70, 70),
        2 => Color32::from_rgb(70, 190, 90),
        3 => Color32::from_rgb(80, 130, 235),
        _ => ui.visuals().text_color(),
    }
}

/// Draws the editor for `effect` (the effect at the panel's `index`) and returns what the user did.
/// `draft` holds the values of a drag in progress.
pub fn curves_ui(
    ui: &mut egui::Ui,
    state: &mut CurveState,
    effect: &TrackEffect,
    draft: &[(String, f32)],
) -> Option<CurveOutput> {
    let mut output = None;
    state.channel = state.channel.min(CHANNELS.len() - 1);
    ui.horizontal(|ui| {
        for (i, (_, name)) in CHANNELS.iter().enumerate() {
            if ui
                .selectable_value(&mut state.channel, i, tr(name))
                .clicked()
            {
                state.selected = None;
            }
        }
    });
    let channel = state.channel;
    let points = read_points(effect, channel, draft);

    let side = ui.available_width().clamp(120.0, 260.0);
    let (rect, response) = ui.allocate_exact_size(vec2(side, side), Sense::click_and_drag());
    let to_screen = |x: f32, y: f32| pos2(rect.left() + x * side, rect.bottom() - y * side);
    let to_curve = |p: egui::Pos2| ((p.x - rect.left()) / side, (rect.bottom() - p.y) / side);
    let reach = GRAB_RADIUS / side;

    if response.drag_started() {
        if let Some(p) = response.interact_pointer_pos() {
            if let Some(i) = nearest_point(&points, to_curve(p), reach) {
                state.drag = Some((channel, i));
                state.selected = Some(i);
            }
        }
    }
    if response.clicked() {
        if let Some(p) = response.interact_pointer_pos() {
            state.selected = nearest_point(&points, to_curve(p), reach);
        }
    }
    if response.double_clicked() {
        if let Some(i) = response
            .interact_pointer_pos()
            .and_then(|p| nearest_point(&points, to_curve(p), reach))
        {
            output = Some(CurveOutput::Commit(default_point(channel, i)));
        }
    }
    let mut points = points;
    if let Some((drag_channel, i)) = state.drag.filter(|(c, _)| *c == channel) {
        if response.dragged() {
            if let Some(p) = response.interact_pointer_pos() {
                let (x, y) = to_curve(p);
                let (x, y) = drag_point(&points, i, x, y);
                points[2 * i] = x;
                points[2 * i + 1] = y;
                output = Some(CurveOutput::Live(vec![
                    (param_id(drag_channel, i, 'x'), x),
                    (param_id(drag_channel, i, 'y'), y),
                ]));
            }
        }
        if response.drag_stopped() {
            state.drag = None;
            output = Some(CurveOutput::Commit(vec![
                (param_id(drag_channel, i, 'x'), points[2 * i]),
                (param_id(drag_channel, i, 'y'), points[2 * i + 1]),
            ]));
        }
    } else if response.drag_stopped() {
        state.drag = None;
    }

    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect.expand(1.0));
        let grid = ui.visuals().widgets.noninteractive.bg_stroke.color;
        painter.rect_filled(rect, 2.0, ui.visuals().extreme_bg_color);
        for k in 1..4 {
            let t = k as f32 / 4.0;
            let faint = Stroke::new(1.0_f32, grid.gamma_multiply(0.5));
            painter.line_segment([to_screen(t, 0.0), to_screen(t, 1.0)], faint);
            painter.line_segment([to_screen(0.0, t), to_screen(1.0, t)], faint);
        }
        painter.line_segment(
            [to_screen(0.0, 0.0), to_screen(1.0, 1.0)],
            Stroke::new(1.0_f32, grid),
        );
        let color = channel_color(ui, channel);
        let table = vfx::curve_table(&points);
        let line: Vec<egui::Pos2> = table
            .iter()
            .enumerate()
            .map(|(i, y)| to_screen(i as f32 / (table.len() - 1) as f32, *y))
            .collect();
        painter.add(Shape::line(line, Stroke::new(2.0_f32, color)));
        for i in 0..CURVE_POINTS {
            let at = to_screen(points[2 * i], points[2 * i + 1]);
            let picked = state.selected == Some(i);
            painter.circle(
                at,
                if picked { 5.5 } else { 4.0 },
                if picked {
                    color
                } else {
                    ui.visuals().extreme_bg_color
                },
                Stroke::new(1.5_f32, color),
            );
        }
        painter.rect_stroke(rect, 2.0, Stroke::new(1.0_f32, grid));
    }

    ui.horizontal(|ui| {
        match state.selected.filter(|i| *i < CURVE_POINTS) {
            Some(i) => ui.weak(format!(
                "X {:.1} %   Y {:.1} %",
                points[2 * i] * 100.0,
                points[2 * i + 1] * 100.0
            )),
            None => ui.weak(tr("Arraste os pontos; duplo clique restaura um ponto.")),
        };
        if ui.button(tr("Restaurar curva")).clicked() {
            output = Some(CurveOutput::Commit(default_curve(channel)));
        }
    });
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    const STRAIGHT: [f32; 10] = [0.0, 0.0, 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1.0, 1.0];

    #[test]
    fn parameter_ids_follow_the_effect_registry() {
        let spec = vfx::spec(vfx::COLOR_CURVES).unwrap();
        for channel in 0..CHANNELS.len() {
            for point in 0..CURVE_POINTS {
                for axis in ['x', 'y'] {
                    assert!(
                        spec.param(&param_id(channel, point, axis)).is_some(),
                        "{channel} {point} {axis}"
                    );
                }
            }
        }
        assert_eq!(param_id(2, 3, 'y'), "g_y4");
    }

    #[test]
    fn a_stored_point_and_a_dragged_one_are_read_back() {
        let mut fx = TrackEffect::new(vfx::COLOR_CURVES);
        assert_eq!(read_points(&fx, 0, &[]), STRAIGHT);
        fx.params.insert("r_y3".into(), 0.8);
        assert_eq!(read_points(&fx, 1, &[])[5], 0.8);
        assert_eq!(read_points(&fx, 0, &[])[5], 0.5, "another curve");
        let drag = [("r_y3".to_string(), 0.9)];
        assert_eq!(read_points(&fx, 1, &drag)[5], 0.9, "the drag wins");
    }

    #[test]
    fn the_nearest_point_is_grabbed_only_within_reach() {
        assert_eq!(nearest_point(&STRAIGHT, (0.52, 0.49), 0.05), Some(2));
        assert_eq!(nearest_point(&STRAIGHT, (0.5, 0.9), 0.05), None);
        // Two points close to each other: the closer one wins
        let mut points = STRAIGHT;
        points[5] = 0.3; // point 2 at (0.5, 0.3)
        assert_eq!(nearest_point(&points, (0.26, 0.26), 0.2), Some(1));
    }

    #[test]
    fn a_dragged_point_stays_in_the_square_and_between_its_neighbours() {
        assert_eq!(drag_point(&STRAIGHT, 2, 0.6, 0.9), (0.6, 0.9));
        let (x, _) = drag_point(&STRAIGHT, 2, 0.9, 0.5);
        assert!(
            (x - (0.75 - MIN_GAP)).abs() < 1e-6,
            "not past the next point"
        );
        let (x, _) = drag_point(&STRAIGHT, 2, 0.1, 0.5);
        assert!(
            (x - (0.25 + MIN_GAP)).abs() < 1e-6,
            "not before the previous one"
        );
        assert_eq!(drag_point(&STRAIGHT, 2, 0.5, 1.7).1, 1.0);
        assert_eq!(drag_point(&STRAIGHT, 2, 0.5, -1.0).1, 0.0);
        // The ends can leave the corners along the edges but not the square
        assert_eq!(drag_point(&STRAIGHT, 0, -0.5, 0.4), (0.0, 0.4));
        assert_eq!(drag_point(&STRAIGHT, 4, 3.0, 0.6), (1.0, 0.6));
    }

    #[test]
    fn restoring_gives_the_straight_values_of_a_point_or_of_the_whole_curve() {
        assert_eq!(
            default_point(1, 3),
            vec![("r_x4".to_string(), 0.75), ("r_y4".to_string(), 0.75)]
        );
        let all = default_curve(3);
        assert_eq!(all.len(), 2 * CURVE_POINTS);
        let fx = TrackEffect::new(vfx::COLOR_CURVES);
        for (id, value) in all {
            let spec = vfx::spec(vfx::COLOR_CURVES).unwrap().param(&id).unwrap();
            assert_eq!(effective_value(&fx, spec), value, "{id}");
        }
    }
}
