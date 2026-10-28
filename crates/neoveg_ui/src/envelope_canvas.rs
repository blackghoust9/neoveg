//! Track automation envelope geometry, hit-testing, interactive drag state and timeline painting.

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, FontId, Pos2, Rect, Rounding, Shape, Stroke,
};
use neoveg_audio::clap_host;
use neoveg_audio::dsp::effects;
use neoveg_engine::{
    AutomationTarget, EnvelopePoint, EnvelopeShape, Track, TrackEffect, TrackEnvelope, TrackId,
};

use crate::fx_panel;

const MARGIN_Y: f32 = 6.0;
const NODE_HIT_RADIUS_PX: f32 = 7.0;
const LINE_HIT_PX: f32 = 5.5;
const CURVE_SUBSTEPS: usize = 20;

/// Live preview while the user drags an automation keyframe node.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvelopeDrag {
    pub track_id: TrackId,
    pub target: AutomationTarget,
    pub index: usize,
    pub time: f64,
    pub value: f32,
    pub bounds: (f32, f32),
}

/// Keyframe node or curve segment under the pointer.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvelopeHit {
    pub track_id: TrackId,
    pub target: AutomationTarget,
    pub point_index: Option<usize>,
    pub time: f64,
    pub value: f32,
    pub bounds: (f32, f32),
    pub default: f32,
}

/// Returns `(min, max, default)` for `target` on a track with `chain`.
pub fn target_bounds(chain: &[TrackEffect], target: &AutomationTarget) -> (f32, f32, f32) {
    match target {
        AutomationTarget::Volume
        | AutomationTarget::Pan
        | AutomationTarget::VideoLevel
        | AutomationTarget::VideoFade
        | AutomationTarget::VideoMute => target.default_bounds(),
        AutomationTarget::EffectParam {
            effect_index,
            param_id,
        } => {
            let Some(fx) = chain.get(*effect_index) else {
                return (0.0, 1.0, 0.0);
            };
            if let Some(spec) = effects::effect_info(&fx.kind)
                .and_then(|info| info.params.iter().find(|p| p.id == param_id))
            {
                return (spec.min, spec.max, spec.default);
            }
            if clap_host::is_clap_kind(&fx.kind) {
                if let Some(p) = clap_host::plugin_info(&fx.kind)
                    .and_then(|info| info.params.into_iter().find(|p| p.id == *param_id))
                {
                    return (p.min, p.max, p.default);
                }
            }
            (0.0, 1.0, 0.0)
        }
    }
}

/// Distinct color per automation target kind.
pub fn target_color(target: &AutomationTarget) -> Color32 {
    match target {
        AutomationTarget::Volume => Color32::from_rgb(0x34, 0x98, 0xDB),
        AutomationTarget::Pan => Color32::from_rgb(0xE6, 0x7E, 0x22),
        AutomationTarget::VideoLevel => Color32::from_rgb(0xF1, 0xC4, 0x0F),
        AutomationTarget::VideoFade => Color32::from_rgb(0xE9, 0x1E, 0x8C),
        AutomationTarget::VideoMute => Color32::from_rgb(0xE7, 0x4C, 0x3C),
        AutomationTarget::EffectParam { .. } => Color32::from_rgb(0x2E, 0xCC, 0x71),
    }
}

/// Human-readable pt-BR title of `target` (resolving effect parameter display names when known).
pub fn target_title(chain: &[TrackEffect], target: &AutomationTarget) -> String {
    match target {
        AutomationTarget::Volume => "Volume".into(),
        AutomationTarget::Pan => "Pan".into(),
        AutomationTarget::VideoLevel
        | AutomationTarget::VideoFade
        | AutomationTarget::VideoMute => target.label(),
        AutomationTarget::EffectParam {
            effect_index,
            param_id,
        } => {
            if let Some(fx) = chain.get(*effect_index) {
                if let Some(info) = effects::effect_info(&fx.kind) {
                    let pname = info
                        .params
                        .iter()
                        .find(|p| p.id == param_id)
                        .map_or(param_id.as_str(), |p| p.name);
                    return format!("{} · {pname}", info.name);
                }
                if clap_host::is_clap_kind(&fx.kind) {
                    if let Some(cinfo) = clap_host::plugin_info(&fx.kind) {
                        let pname = cinfo
                            .params
                            .iter()
                            .find(|p| p.id == *param_id)
                            .map_or(param_id.as_str(), |p| p.name.as_str());
                        return format!("{} · {pname}", cinfo.name);
                    }
                }
            }
            target.label()
        }
    }
}

/// Formats `value` with appropriate units (`dB`, `% L/R`, or the effect parameter's unit).
pub fn format_envelope_value(
    chain: &[TrackEffect],
    target: &AutomationTarget,
    value: f32,
) -> String {
    match target {
        AutomationTarget::Volume => {
            if value <= -60.0 {
                "-∞ dB".into()
            } else {
                format!("{value:+.1} dB")
            }
        }
        AutomationTarget::Pan => {
            if value.abs() < 0.005 {
                "Centro".into()
            } else if value < 0.0 {
                format!("{:.0}% E", -value * 100.0)
            } else {
                format!("{:.0}% D", value * 100.0)
            }
        }
        AutomationTarget::VideoLevel | AutomationTarget::VideoFade => {
            format!("{:.0} %", value * 100.0)
        }
        AutomationTarget::VideoMute => {
            if value >= 0.5 {
                "Oculta".into()
            } else {
                "Visível".into()
            }
        }
        AutomationTarget::EffectParam {
            effect_index,
            param_id,
        } => chain
            .get(*effect_index)
            .and_then(|fx| effects::effect_info(&fx.kind))
            .and_then(|info| info.params.iter().find(|p| p.id == param_id))
            .map_or_else(
                || format!("{value:.2}"),
                |spec| fx_panel::format_value(spec, value),
            ),
    }
}

/// Maps `value` in `min..=max` to a screen Y inside `[lane_y + MARGIN_Y .. lane_y + height - MARGIN_Y]`.
pub fn value_to_y(lane_y: f32, track_height: f32, value: f32, min: f32, max: f32) -> f32 {
    let top = lane_y + MARGIN_Y;
    let span_y = (track_height - 2.0 * MARGIN_Y).max(4.0);
    let span_v = (max - min).abs().max(1e-6);
    let norm = ((value - min) / span_v).clamp(0.0, 1.0);
    top + (1.0 - norm) * span_y
}

/// Maps a screen Y inside a track lane back to `min..=max`.
pub fn y_to_value(lane_y: f32, track_height: f32, y: f32, min: f32, max: f32) -> f32 {
    let top = lane_y + MARGIN_Y;
    let span_y = (track_height - 2.0 * MARGIN_Y).max(4.0);
    let norm = (1.0 - (y - top) / span_y).clamp(0.0, 1.0);
    min + norm * (max - min)
}

/// Hit-tests visible automation envelopes across `tracks` at pointer `pos`. Keyframe nodes take
/// priority over curve lines.
pub fn hit_test(
    tracks: &[Track],
    lanes_left: f32,
    lane_top: f32,
    track_height: f32,
    scroll_seconds: f64,
    pps: f64,
    pos: Pos2,
) -> Option<EnvelopeHit> {
    if pos.x < lanes_left || pos.y < lane_top || pps <= 0.0 {
        return None;
    }
    let row = ((pos.y - lane_top) / track_height).floor();
    if row < 0.0 {
        return None;
    }
    let row_idx = row as usize;
    let track = tracks.get(row_idx)?;
    let y0 = lane_top + row_idx as f32 * track_height;
    let ptr_time = (scroll_seconds + f64::from(pos.x - lanes_left) / pps).max(0.0);

    // 1. Check square keyframe nodes first (drawn last -> test in reverse order)
    for env in track.envelopes.iter().rev().filter(|e| e.visible) {
        let (min, max, default) = target_bounds(&track.effects, &env.target);
        for (idx, pt) in env.points.iter().enumerate() {
            let px = lanes_left + ((pt.time - scroll_seconds) * pps) as f32;
            let py = value_to_y(y0, track_height, pt.value, min, max);
            if (pos.x - px).abs() <= NODE_HIT_RADIUS_PX && (pos.y - py).abs() <= NODE_HIT_RADIUS_PX
            {
                return Some(EnvelopeHit {
                    track_id: track.id,
                    target: env.target.clone(),
                    point_index: Some(idx),
                    time: pt.time,
                    value: pt.value,
                    bounds: (min, max),
                    default,
                });
            }
        }
    }

    // 2. Check interpolated curve lines
    for env in track.envelopes.iter().rev().filter(|e| e.visible) {
        let (min, max, default) = target_bounds(&track.effects, &env.target);
        let curve_val = env.value_at(ptr_time, default);
        let cy = value_to_y(y0, track_height, curve_val, min, max);
        if (pos.y - cy).abs() <= LINE_HIT_PX {
            let v = y_to_value(y0, track_height, pos.y, min, max);
            return Some(EnvelopeHit {
                track_id: track.id,
                target: env.target.clone(),
                point_index: None,
                time: ptr_time,
                value: v,
                bounds: (min, max),
                default,
            });
        }
    }
    None
}

fn preview_envelope(
    env: &TrackEnvelope,
    drag: Option<&EnvelopeDrag>,
    track_id: TrackId,
) -> TrackEnvelope {
    let Some(d) = drag.filter(|d| d.track_id == track_id && d.target == env.target) else {
        return env.clone();
    };
    let mut copy = env.clone();
    copy.move_point(d.index, d.time, d.value, d.bounds.0, d.bounds.1);
    copy
}

/// Paints all visible automation envelopes on a `track` (audio or video) inside its lane row.
#[allow(clippy::too_many_arguments)]
pub fn paint_track_envelopes(
    painter: &egui::Painter,
    lanes: Rect,
    lane_y: f32,
    track_height: f32,
    scroll_seconds: f64,
    pps: f64,
    track: &Track,
    drag: Option<&EnvelopeDrag>,
) {
    if pps <= 0.0 {
        return;
    }
    let t_left = scroll_seconds;
    let t_right = scroll_seconds + f64::from(lanes.width()) / pps;
    for (vis_idx, raw_env) in track.envelopes.iter().filter(|e| e.visible).enumerate() {
        let env = preview_envelope(raw_env, drag, track.id);
        let (min, max, default) = target_bounds(&track.effects, &env.target);
        let base_color = target_color(&env.target);
        let color = if env.enabled {
            base_color
        } else {
            base_color.gamma_multiply(0.45)
        };
        let to_x = |t: f64| lanes.left() + ((t - scroll_seconds) * pps) as f32;
        let to_y = |v: f32| value_to_y(lane_y, track_height, v, min, max);

        // Build polyline across visible range
        let mut poly: Vec<Pos2> = Vec::new();
        if env.points.is_empty() {
            let y = to_y(default);
            poly.push(pos2(lanes.left(), y));
            poly.push(pos2(lanes.right(), y));
        } else {
            let first = env.points[0];
            if first.time > t_left {
                let y = to_y(first.value);
                poly.push(pos2(lanes.left(), y));
                poly.push(pos2(to_x(first.time).min(lanes.right()), y));
            }
            for pair in env.points.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                if b.time < t_left || a.time > t_right {
                    continue;
                }
                match a.shape {
                    EnvelopeShape::Hold => {
                        let ya = to_y(a.value);
                        let yb = to_y(b.value);
                        let xa = to_x(a.time).clamp(lanes.left() - 2.0, lanes.right() + 2.0);
                        let xb = to_x(b.time).clamp(lanes.left() - 2.0, lanes.right() + 2.0);
                        poly.push(pos2(xa, ya));
                        poly.push(pos2(xb, ya));
                        poly.push(pos2(xb, yb));
                    }
                    EnvelopeShape::Linear => {
                        poly.push(pos2(to_x(a.time), to_y(a.value)));
                        poly.push(pos2(to_x(b.time), to_y(b.value)));
                    }
                    shape => {
                        let span = (b.time - a.time).max(1e-6);
                        for step in 0..=CURVE_SUBSTEPS {
                            let u = step as f64 / CURVE_SUBSTEPS as f64;
                            let t = a.time + u * span;
                            if t < t_left - span / CURVE_SUBSTEPS as f64
                                || t > t_right + span / CURVE_SUBSTEPS as f64
                            {
                                continue;
                            }
                            let k = shape.eval(u) as f32;
                            let v = a.value + (b.value - a.value) * k;
                            poly.push(pos2(to_x(t), to_y(v)));
                        }
                    }
                }
            }
            if let Some(last) = env.points.last() {
                if last.time < t_right {
                    let y = to_y(last.value);
                    poly.push(pos2(to_x(last.time).max(lanes.left()), y));
                    poly.push(pos2(lanes.right(), y));
                }
            }
        }
        if poly.len() >= 2 {
            painter.add(Shape::line(poly, Stroke::new(1.6_f32, color)));
        }

        // Keyframe square handles
        for (idx, pt) in env.points.iter().enumerate() {
            let x = to_x(pt.time);
            if x < lanes.left() - 6.0 || x > lanes.right() + 6.0 {
                continue;
            }
            let y = to_y(pt.value);
            let is_active = drag.is_some_and(|d| {
                d.track_id == track.id && d.target == env.target && d.index == idx
            });
            let size = if is_active { 7.0 } else { 5.5 };
            let node_rect = Rect::from_center_size(pos2(x, y), vec2(size, size));
            painter.rect_filled(
                node_rect,
                Rounding::ZERO,
                if is_active { Color32::WHITE } else { color },
            );
            painter.rect_stroke(
                node_rect,
                Rounding::ZERO,
                Stroke::new(1.0_f32, Color32::from_black_alpha(200)),
            );
            if is_active {
                let text = format_envelope_value(&track.effects, &env.target, pt.value);
                painter.text(
                    pos2(
                        (x + 7.0).min(lanes.right() - 48.0),
                        (y - 12.0).max(lane_y + 2.0),
                    ),
                    Align2::LEFT_TOP,
                    text,
                    FontId::monospace(10.0),
                    Color32::WHITE,
                );
            }
        }

        // Compact badge on the left of the lane identifying the envelope
        let badge_y = lane_y + 3.0 + vis_idx as f32 * 12.0;
        if badge_y + 11.0 <= lane_y + track_height {
            let label = target_title(&track.effects, &env.target);
            painter.text(
                pos2(lanes.left() + 4.0, badge_y),
                Align2::LEFT_TOP,
                label,
                FontId::proportional(9.5),
                color,
            );
        }
    }
}

/// Builds a new keyframe [`EnvelopePoint`] inheriting the shape of the preceding keyframe.
pub fn new_point_on_curve(env: &TrackEnvelope, time: f64, value: f32) -> EnvelopePoint {
    let idx = env.points.partition_point(|p| p.time <= time);
    let shape = idx
        .checked_sub(1)
        .and_then(|i| env.points.get(i))
        .map_or(EnvelopeShape::Linear, |p| p.shape);
    EnvelopePoint { time, value, shape }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neoveg_engine::{Timeline, TrackKind};

    #[test]
    fn y_mapping_inverts_cleanly_and_hit_test_finds_nodes_before_lines() {
        let y = value_to_y(100.0, 74.0, -6.0, -60.0, 12.0);
        let back = y_to_value(100.0, 74.0, y, -60.0, 12.0);
        assert!((back - -6.0).abs() < 1e-4);

        let mut tl = Timeline::new();
        let mut t = tl.new_track(TrackKind::Audio);
        let mut env = TrackEnvelope::new(AutomationTarget::Volume, 0.0);
        env.upsert_point(1.0, -12.0, EnvelopeShape::Linear, -60.0, 12.0, 1e-4);
        t.envelopes.push(env);
        let tid = t.id;
        tl.tracks.push(t);

        // Node at t = 1.0 s -> x = 200 + 1.0 * 100 = 300
        let node_y = value_to_y(40.0, 74.0, -12.0, -60.0, 12.0);
        let hit = hit_test(
            &tl.tracks,
            200.0,
            40.0,
            74.0,
            0.0,
            100.0,
            pos2(300.0, node_y),
        )
        .unwrap();
        assert_eq!(hit.track_id, tid);
        assert_eq!(hit.point_index, Some(1));

        // Curve midpoint at t = 0.5 s -> value = -6.0 dB -> hits line (`point_index == None`)
        let mid_y = value_to_y(40.0, 74.0, -6.0, -60.0, 12.0);
        let line_hit = hit_test(
            &tl.tracks,
            200.0,
            40.0,
            74.0,
            0.0,
            100.0,
            pos2(250.0, mid_y),
        )
        .unwrap();
        assert_eq!(line_hit.point_index, None);
    }

    #[test]
    fn effect_param_bounds_and_titles_follow_the_effect_registry() {
        let chain = vec![TrackEffect::new("compressor")];
        let target = AutomationTarget::EffectParam {
            effect_index: 0,
            param_id: "ratio".into(),
        };
        let (min, max, def) = target_bounds(&chain, &target);
        assert!(min >= 1.0 && max >= 10.0 && def >= min);
        assert!(target_title(&chain, &target).contains("Compressor"));
        assert_eq!(format_envelope_value(&chain, &target, 4.0), "4.0:1");
    }
}
