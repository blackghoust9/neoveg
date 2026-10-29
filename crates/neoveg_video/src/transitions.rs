//! Video transitions: how a layer appears (or disappears) instead of the plain dissolve.
//!
//! A transition is a function of a **progress** `0.0..=1.0`: at `0.0` the layer is not there, at
//! `1.0` it is whole and untouched. The progress is the gain of the event's fade (or of the overlap
//! with the event before it), so the length and the curve of a transition are the ones of the fade.
//! Out-transitions use the same functions with the progress going down from `1.0` to `0.0`.
//!
//! Wipes, iris, barn door and blinds multiply the alpha of the layer by a mask with a soft edge;
//! slide moves the layer in and grow scales it up from the centre. The registry (names, values,
//! ranges) is the one of [`crate::vfx`], so the interface treats effects and transitions alike.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use crate::frame::FrameBuffer;
use crate::pan_crop::{render, sample};
use crate::vfx::{EffectSpec, ParamSpec};

pub const DISSOLVE: &str = "dissolve";
pub const LINEAR_WIPE: &str = "linear_wipe";
pub const CLOCK_WIPE: &str = "clock_wipe";
pub const IRIS: &str = "iris";
pub const BARN_DOOR: &str = "barn_door";
pub const BLINDS: &str = "blinds";
pub const SLIDE: &str = "slide";
pub const GROW: &str = "grow";
pub const SQUEEZE: &str = "squeeze";
pub const SPIRAL: &str = "spiral";
pub const FLASH: &str = "flash";

const SOFTNESS: ParamSpec = ParamSpec {
    id: "softness",
    label: "Suavidade da borda",
    min: 0.0,
    max: 1.0,
    default: 0.1,
    percent: true,
};

fn angle(id: &'static str, label: &'static str, max: f32) -> ParamSpec {
    ParamSpec {
        id,
        label,
        min: 0.0,
        max,
        default: 0.0,
        percent: false,
    }
}

/// Every transition, in the order of the list. (`fn` because a `ParamSpec` list with computed
/// entries cannot be a `const`; the interface asks once per frame, which costs nothing.)
pub fn transitions() -> &'static [EffectSpec] {
    use std::sync::OnceLock;
    static LIST: OnceLock<Vec<EffectSpec>> = OnceLock::new();
    LIST.get_or_init(|| {
        fn leak(params: Vec<ParamSpec>) -> &'static [ParamSpec] {
            Box::leak(params.into_boxed_slice())
        }
        vec![
            EffectSpec {
                id: DISSOLVE,
                label: "Dissolver",
                params: &[],
            },
            EffectSpec {
                id: LINEAR_WIPE,
                label: "Cortina linear",
                params: leak(vec![angle("angle", "Ângulo", 360.0), SOFTNESS]),
            },
            EffectSpec {
                id: CLOCK_WIPE,
                label: "Cortina de relógio",
                params: leak(vec![
                    angle("start_angle", "Ângulo inicial", 360.0),
                    ParamSpec {
                        default: 0.05,
                        ..SOFTNESS
                    },
                ]),
            },
            EffectSpec {
                id: IRIS,
                label: "Íris",
                params: leak(vec![SOFTNESS]),
            },
            EffectSpec {
                id: BARN_DOOR,
                label: "Porta de celeiro",
                params: leak(vec![
                    angle("angle", "Orientação", 180.0),
                    ParamSpec {
                        default: 0.05,
                        ..SOFTNESS
                    },
                ]),
            },
            EffectSpec {
                id: BLINDS,
                label: "Persianas",
                params: leak(vec![
                    ParamSpec {
                        id: "count",
                        label: "Quantidade",
                        min: 2.0,
                        max: 50.0,
                        default: 8.0,
                        percent: false,
                    },
                    angle("angle", "Orientação", 180.0),
                    ParamSpec {
                        default: 0.05,
                        ..SOFTNESS
                    },
                ]),
            },
            EffectSpec {
                id: SLIDE,
                label: "Deslizar",
                params: leak(vec![angle("angle", "Direção de entrada", 360.0)]),
            },
            EffectSpec {
                id: GROW,
                label: "Crescer",
                params: &[],
            },
            EffectSpec {
                id: SQUEEZE,
                label: "Espremer",
                params: leak(vec![angle("angle", "Direção de entrada", 360.0)]),
            },
            EffectSpec {
                id: SPIRAL,
                label: "Espiral",
                params: leak(vec![
                    ParamSpec {
                        id: "turns",
                        label: "Voltas",
                        min: 0.0,
                        max: 8.0,
                        default: 1.0,
                        percent: false,
                    },
                    ParamSpec {
                        id: "zoom",
                        label: "Zoom inicial",
                        min: 0.0,
                        max: 1.0,
                        default: 0.0,
                        percent: true,
                    },
                    ParamSpec {
                        id: "clockwise",
                        label: "Sentido horário (0 ou 1)",
                        min: 0.0,
                        max: 1.0,
                        default: 1.0,
                        percent: false,
                    },
                ]),
            },
            EffectSpec {
                id: FLASH,
                label: "Flash",
                params: leak(vec![
                    ParamSpec {
                        id: "red",
                        label: "Vermelho",
                        min: 0.0,
                        max: 1.0,
                        default: 1.0,
                        percent: true,
                    },
                    ParamSpec {
                        id: "green",
                        label: "Verde",
                        min: 0.0,
                        max: 1.0,
                        default: 1.0,
                        percent: true,
                    },
                    ParamSpec {
                        id: "blue",
                        label: "Azul",
                        min: 0.0,
                        max: 1.0,
                        default: 1.0,
                        percent: true,
                    },
                ]),
            },
        ]
    })
}

/// The registry entry of `id`.
pub fn spec(id: &str) -> Option<&'static EffectSpec> {
    transitions().iter().find(|t| t.id == id)
}

/// A transition as the worker receives it: the id, the values that differ from the defaults and
/// the progress of this frame.
#[derive(Debug, Clone, PartialEq)]
pub struct TransitionRequest {
    pub kind: String,
    pub params: BTreeMap<String, f32>,
    pub progress: f32,
}

impl TransitionRequest {
    /// Feeds the transition into a hasher (the worker's frame cache key).
    pub fn hash_into(&self, h: &mut impl Hasher) {
        self.kind.hash(h);
        self.progress.to_bits().hash(h);
        for (k, v) in &self.params {
            k.hash(h);
            v.to_bits().hash(h);
        }
    }
}

/// A transition with its values clamped.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Resolved {
    Dissolve,
    LinearWipe {
        angle: f32,
        softness: f32,
    },
    ClockWipe {
        start: f32,
        softness: f32,
    },
    Iris {
        softness: f32,
    },
    BarnDoor {
        angle: f32,
        softness: f32,
    },
    Blinds {
        count: f32,
        angle: f32,
        softness: f32,
    },
    Slide {
        angle: f32,
    },
    Grow,
    /// The layer is compressed against the edge it enters from (see [`squeeze_edge`]).
    Squeeze {
        angle: f32,
    },
    /// The layer turns and grows from `zoom` to its full size (see [`spiral_transform`]).
    Spiral {
        turns: f32,
        zoom: f32,
        clockwise: bool,
    },
    /// A flash of `color` (`0..=1` per channel) covers the picture in the middle of the transition.
    Flash {
        color: [f32; 3],
    },
}

/// The transition with every value clamped to its range, or `None` for an unknown id.
pub fn resolve(kind: &str, params: &BTreeMap<String, f32>) -> Option<Resolved> {
    let spec = spec(kind)?;
    let v = |id: &str| -> f32 {
        let Some(p) = spec.param(id) else {
            return 0.0;
        };
        match params.get(id) {
            Some(x) if x.is_finite() => x.clamp(p.min, p.max),
            _ => p.default,
        }
    };
    Some(match spec.id {
        DISSOLVE => Resolved::Dissolve,
        LINEAR_WIPE => Resolved::LinearWipe {
            angle: v("angle"),
            softness: v("softness"),
        },
        CLOCK_WIPE => Resolved::ClockWipe {
            start: v("start_angle"),
            softness: v("softness"),
        },
        IRIS => Resolved::Iris {
            softness: v("softness"),
        },
        BARN_DOOR => Resolved::BarnDoor {
            angle: v("angle"),
            softness: v("softness"),
        },
        BLINDS => Resolved::Blinds {
            count: v("count").round(),
            angle: v("angle"),
            softness: v("softness"),
        },
        SLIDE => Resolved::Slide { angle: v("angle") },
        GROW => Resolved::Grow,
        SQUEEZE => Resolved::Squeeze { angle: v("angle") },
        SPIRAL => Resolved::Spiral {
            turns: v("turns"),
            zoom: v("zoom"),
            clockwise: v("clockwise") >= 0.5,
        },
        FLASH => Resolved::Flash {
            color: [v("red"), v("green"), v("blue")],
        },
        _ => return None,
    })
}

/// The edge a squeeze starts from, from its angle: `0` the left, `1` the top, `2` the right, `3`
/// the bottom (the nearest of the four, like the direction of a slide).
pub fn squeeze_edge(angle: f32) -> u32 {
    ((angle / 90.0).round() as i32).rem_euclid(4) as u32
}

/// What the spiral does to the layer at `progress`: the cosine and sine of the turn it still has to
/// make and its scale (`zoom` at the start, `1.0` at the end).
pub fn spiral_transform(turns: f32, zoom: f32, clockwise: bool, progress: f32) -> (f32, f32, f32) {
    let turn = (1.0 - progress) * turns * std::f32::consts::TAU;
    let (sin, cos) = (if clockwise { turn } else { -turn }).sin_cos();
    (cos, sin, zoom + (1.0 - zoom) * progress)
}

/// How strong the flash is at `progress`: nothing at either end, full in the middle.
pub fn flash_strength(progress: f32) -> f32 {
    1.0 - (2.0 * progress - 1.0).abs()
}

/// Opacity of a point whose place in the reveal order is `s` (`0.0` is revealed first, `1.0`
/// last) at `progress`, with an edge `soft` wide.
fn reveal(s: f32, progress: f32, soft: f32) -> f32 {
    let soft = soft.max(0.002);
    let t = ((progress * (1.0 + soft) - s) / soft).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Position of `(u, v)` (`0..=1` over the frame) along the direction `degrees`, as a share of the
/// frame's extent in that direction: `0.0` on the first corner to be passed, `1.0` on the last.
/// Distances are measured in pixels (the frame is `aspect` times as wide as it is high).
fn along(u: f32, v: f32, aspect: f32, degrees: f32) -> f32 {
    let (sin, cos) = degrees.to_radians().sin_cos();
    let proj = |x: f32, y: f32| x * aspect * cos + y * sin;
    let corners = [
        proj(0.0, 0.0),
        proj(1.0, 0.0),
        proj(0.0, 1.0),
        proj(1.0, 1.0),
    ];
    let (lo, hi) = corners
        .iter()
        .fold((f32::MAX, f32::MIN), |(lo, hi), c| (lo.min(*c), hi.max(*c)));
    if hi - lo < 1e-6 {
        return 0.0;
    }
    ((proj(u, v) - lo) / (hi - lo)).clamp(0.0, 1.0)
}

/// How much of the layer is visible at `(u, v)` for the mask transitions.
fn mask(t: &Resolved, u: f32, v: f32, aspect: f32, progress: f32) -> f32 {
    match *t {
        Resolved::Dissolve => progress,
        Resolved::LinearWipe { angle, softness } => {
            reveal(along(u, v, aspect, angle), progress, softness)
        }
        Resolved::ClockWipe { start, softness } => {
            let (dx, dy) = ((u - 0.5) * aspect, v - 0.5);
            // `0` at the top, growing clockwise, `1` after a whole turn
            let turn = dx.atan2(-dy) / std::f32::consts::TAU;
            let s = (turn - start / 360.0).rem_euclid(1.0);
            reveal(s, progress, softness)
        }
        Resolved::Iris { softness } => {
            let r = ((u - 0.5) * aspect).hypot(v - 0.5) / (aspect / 2.0).hypot(0.5);
            reveal(r, progress, softness)
        }
        Resolved::BarnDoor { angle, softness } => {
            // Opens from the middle line outward
            let s = (along(u, v, aspect, angle) - 0.5).abs() * 2.0;
            reveal(s, progress, softness)
        }
        Resolved::Blinds {
            count,
            angle,
            softness,
        } => {
            let s = (along(u, v, aspect, angle) * count).fract();
            reveal(s, progress, softness)
        }
        Resolved::Slide { .. }
        | Resolved::Grow
        | Resolved::Squeeze { .. }
        | Resolved::Spiral { .. }
        | Resolved::Flash { .. } => 1.0,
    }
}

/// The layer as it looks at `progress` of the transition `t`.
pub fn apply(layer: &FrameBuffer, t: &Resolved, progress: f32) -> FrameBuffer {
    let progress = if progress.is_finite() {
        progress.clamp(0.0, 1.0)
    } else {
        1.0
    };
    if layer.width == 0 || layer.height == 0 {
        return layer.clone();
    }
    if progress >= 1.0 {
        return layer.clone();
    }
    let (w, h) = (layer.width as f32, layer.height as f32);
    let aspect = w / h;
    match *t {
        Resolved::Slide { angle } => {
            let (sin, cos) = angle.to_radians().sin_cos();
            // Enters from the side the angle names: `0` from the left, `90` from the top
            let (ox, oy) = (-cos * (1.0 - progress) * w, -sin * (1.0 - progress) * h);
            render(layer.width, layer.height, |x, y| {
                sample(layer, x as f32 + 0.5 - ox, y as f32 + 0.5 - oy)
            })
        }
        Resolved::Grow => {
            if progress <= 0.001 {
                return render(layer.width, layer.height, |_, _| [0.0; 4]);
            }
            let (cx, cy) = (w / 2.0, h / 2.0);
            render(layer.width, layer.height, |x, y| {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                sample(layer, cx + (px - cx) / progress, cy + (py - cy) / progress)
            })
        }
        Resolved::Squeeze { angle } => {
            if progress <= 0.001 {
                return render(layer.width, layer.height, |_, _| [0.0; 4]);
            }
            let edge = squeeze_edge(angle);
            render(layer.width, layer.height, |x, y| {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                match edge {
                    0 => sample(layer, px / progress, py),
                    1 => sample(layer, px, py / progress),
                    2 => sample(layer, w - (w - px) / progress, py),
                    _ => sample(layer, px, h - (h - py) / progress),
                }
            })
        }
        Resolved::Spiral {
            turns,
            zoom,
            clockwise,
        } => {
            let (cos, sin, scale) = spiral_transform(turns, zoom, clockwise, progress);
            if scale <= 0.001 {
                return render(layer.width, layer.height, |_, _| [0.0; 4]);
            }
            let (cx, cy) = (w / 2.0, h / 2.0);
            render(layer.width, layer.height, |x, y| {
                let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                sample(
                    layer,
                    cx + (cos * dx + sin * dy) / scale,
                    cy + (cos * dy - sin * dx) / scale,
                )
            })
        }
        Resolved::Flash { color } => {
            let f = flash_strength(progress);
            render(layer.width, layer.height, |x, y| {
                let p = sample(layer, x as f32 + 0.5, y as f32 + 0.5);
                let keep = progress * (1.0 - f);
                [
                    color[0] * 255.0 * f + p[0] * keep,
                    color[1] * 255.0 * f + p[1] * keep,
                    color[2] * 255.0 * f + p[2] * keep,
                    255.0 * f + p[3] * keep,
                ]
            })
        }
        _ => render(layer.width, layer.height, |x, y| {
            let (u, v) = ((x as f32 + 0.5) / w, (y as f32 + 0.5) / h);
            let m = mask(t, u, v, aspect, progress);
            let p = sample(layer, x as f32 + 0.5, y as f32 + 0.5);
            [p[0] * m, p[1] * m, p[2] * m, p[3] * m]
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn squeeze_spiral_and_flash_start_empty_end_whole_and_do_what_they_say() {
        let layer = crate::generators::smpte_color_bars(80, 40);
        let none = BTreeMap::new();
        for id in [SQUEEZE, SPIRAL, FLASH] {
            let t = resolve(id, &none).expect("registered");
            assert_eq!(apply(&layer, &t, 1.0), layer, "{id} ends whole");
            let first = apply(&layer, &t, 0.0);
            assert!(
                first.rgba.as_chunks::<4>().0.iter().all(|p| p[3] == 0),
                "{id} starts empty"
            );
        }
        // The squeeze enters from the left: at a half the picture fills the left half, squeezed
        let t = resolve(SQUEEZE, &none).unwrap();
        let half = apply(&layer, &t, 0.5);
        let at = |f: &FrameBuffer, x: usize, y: usize| f.rgba[(y * 80 + x) * 4 + 3];
        assert_eq!(at(&half, 20, 10), 255);
        assert_eq!(at(&half, 60, 10), 0);
        let from_right = resolve(SQUEEZE, &BTreeMap::from([("angle".to_owned(), 180.0)])).unwrap();
        let half = apply(&layer, &from_right, 0.5);
        assert_eq!((at(&half, 20, 10), at(&half, 60, 10)), (0, 255));
        assert_eq!(squeeze_edge(0.0), 0);
        assert_eq!(squeeze_edge(100.0), 1);
        assert_eq!(squeeze_edge(359.0), 0);
        // The spiral at the middle is smaller than the frame; the flash covers it whole
        let spiral = resolve(SPIRAL, &none).unwrap();
        let mid = apply(&layer, &spiral, 0.5);
        assert_eq!(at(&mid, 0, 0), 0, "the corner is empty while it is smaller");
        let flash = resolve(FLASH, &BTreeMap::from([("blue".to_owned(), 0.0)])).unwrap();
        let peak = apply(&layer, &flash, 0.5);
        for p in peak.rgba.as_chunks::<4>().0.iter() {
            assert_eq!(*p, [255, 255, 0, 255]);
        }
        assert!((flash_strength(0.25) - 0.5).abs() < 1e-6);
    }

    /// A picture with colour in it so a wrong move shows.
    fn picture(w: u32, h: u32) -> FrameBuffer {
        let mut f = FrameBuffer::solid(w, h, [0, 0, 0, 255]);
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                f.rgba[i] = (x * 255 / w) as u8;
                f.rgba[i + 1] = (y * 255 / h) as u8;
                f.rgba[i + 2] = 180;
            }
        }
        f
    }

    fn all() -> Vec<Resolved> {
        transitions()
            .iter()
            .map(|t| resolve(t.id, &BTreeMap::new()).expect("a known transition"))
            .collect()
    }

    fn with(kind: &str, pairs: &[(&str, f32)]) -> Resolved {
        let params = pairs.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect();
        resolve(kind, &params).unwrap()
    }

    fn visible(f: &FrameBuffer) -> usize {
        f.rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] > 128)
            .count()
    }

    #[test]
    fn every_registry_default_is_inside_its_range_and_every_id_resolves() {
        assert_eq!(transitions().len(), 11);
        for t in transitions() {
            assert!(!t.label.is_empty());
            for p in t.params {
                assert!(
                    p.min <= p.default && p.default <= p.max,
                    "{}.{}",
                    t.id,
                    p.id
                );
            }
            assert!(resolve(t.id, &BTreeMap::new()).is_some(), "{}", t.id);
        }
        assert!(resolve("not_a_transition", &BTreeMap::new()).is_none());
        // Values are clamped and bad ones fall back to the default
        let params: BTreeMap<String, f32> = [
            ("count".to_owned(), 999.0),
            ("softness".to_owned(), f32::NAN),
        ]
        .into();
        assert_eq!(
            resolve(BLINDS, &params),
            Some(Resolved::Blinds {
                count: 50.0,
                angle: 0.0,
                softness: 0.05
            })
        );
    }

    #[test]
    fn at_the_end_every_transition_leaves_the_layer_untouched_and_at_the_start_nothing_shows() {
        let layer = picture(64, 36);
        for t in all() {
            assert_eq!(apply(&layer, &t, 1.0), layer, "{t:?} at 1.0");
            let none = apply(&layer, &t, 0.0);
            assert_eq!(
                visible(&none),
                0,
                "{t:?} at 0.0 shows {} pixels",
                visible(&none)
            );
        }
    }

    #[test]
    fn what_shows_grows_with_the_progress_and_never_exceeds_the_layer() {
        let layer = picture(64, 36);
        for t in all() {
            let mut last = 0usize;
            for step in 0..=10 {
                let shown = visible(&apply(&layer, &t, step as f32 / 10.0));
                assert!(
                    shown + 2 >= last,
                    "{t:?}: {last} then {shown} at step {step}"
                );
                last = shown;
            }
            assert_eq!(last, 64 * 36, "{t:?} ends whole");
        }
    }

    #[test]
    fn a_linear_wipe_reveals_from_the_left_with_a_hard_edge_when_the_softness_is_zero() {
        let layer = picture(100, 20);
        let half = apply(
            &layer,
            &with(LINEAR_WIPE, &[("angle", 0.0), ("softness", 0.0)]),
            0.5,
        );
        assert_eq!(half.pixel(10, 10)[3], 255);
        assert_eq!(half.pixel(90, 10)[3], 0);
        // From the right
        let from_right = apply(
            &layer,
            &with(LINEAR_WIPE, &[("angle", 180.0), ("softness", 0.0)]),
            0.5,
        );
        assert_eq!(from_right.pixel(10, 10)[3], 0);
        assert_eq!(from_right.pixel(90, 10)[3], 255);
        // Colour is the layer's own, not a blend
        assert_eq!(half.pixel(10, 10), layer.pixel(10, 10));
    }

    #[test]
    fn the_iris_the_barn_door_and_the_clock_open_where_they_should() {
        let layer = picture(80, 80);
        let iris = apply(&layer, &with(IRIS, &[("softness", 0.0)]), 0.4);
        assert_eq!(iris.pixel(40, 40)[3], 255, "the centre is first");
        assert_eq!(iris.pixel(2, 2)[3], 0, "the corner is last");

        let door = apply(
            &layer,
            &with(BARN_DOOR, &[("angle", 0.0), ("softness", 0.0)]),
            0.4,
        );
        assert_eq!(door.pixel(40, 5)[3], 255, "the middle column opens first");
        assert_eq!(door.pixel(3, 5)[3], 0);
        let door_v = apply(
            &layer,
            &with(BARN_DOOR, &[("angle", 90.0), ("softness", 0.0)]),
            0.4,
        );
        assert_eq!(
            door_v.pixel(5, 40)[3],
            255,
            "turned a quarter, the middle row opens first"
        );
        assert_eq!(door_v.pixel(5, 3)[3], 0);

        // A quarter of the way round, clockwise from the top: the upper right quadrant shows
        let clock = apply(
            &layer,
            &with(CLOCK_WIPE, &[("start_angle", 0.0), ("softness", 0.0)]),
            0.25,
        );
        assert_eq!(clock.pixel(60, 20)[3], 255);
        assert_eq!(clock.pixel(20, 20)[3], 0);
        assert_eq!(clock.pixel(60, 60)[3], 0);
    }

    #[test]
    fn blinds_open_in_the_number_of_strips_asked_for() {
        let layer = picture(100, 10);
        let t = with(BLINDS, &[("count", 4.0), ("angle", 0.0), ("softness", 0.0)]);
        let half = apply(&layer, &t, 0.5);
        // Each strip is 25 px wide and half open: its left half shows, its right half does not
        for strip in 0..4u32 {
            assert_eq!(half.pixel(strip * 25 + 5, 5)[3], 255, "strip {strip}");
            assert_eq!(half.pixel(strip * 25 + 20, 5)[3], 0, "strip {strip}");
        }
    }

    #[test]
    fn slide_and_grow_move_and_scale_the_layer_instead_of_cutting_it() {
        let layer = picture(80, 40);
        // Half way in from the left: the layer sits half its width to the left of its place, so the
        // left half of the frame shows the right half of the layer and the right half is still empty
        let slide = apply(&layer, &with(SLIDE, &[("angle", 0.0)]), 0.5);
        assert_eq!(
            slide.pixel(60, 20)[3],
            0,
            "nothing has reached the right half yet"
        );
        let moved = slide.pixel(10, 20);
        let source = layer.pixel(50, 20);
        assert!(
            (0..3).all(|c| moved[c].abs_diff(source[c]) <= 4),
            "{moved:?} vs {source:?}"
        );
        assert_eq!(moved[3], 255);
        // From the top: the top half of the frame is filled first
        let from_top = apply(&layer, &with(SLIDE, &[("angle", 90.0)]), 0.5);
        assert_eq!(from_top.pixel(40, 5)[3], 255);
        assert_eq!(from_top.pixel(40, 35)[3], 0);

        // Half grown: the middle is there, scaled to half size, and the edges are empty
        let grown = apply(&layer, &Resolved::Grow, 0.5);
        assert_eq!(grown.pixel(40, 20)[3], 255);
        assert_eq!(grown.pixel(3, 3)[3], 0);
        assert_eq!(visible(&grown), (40 * 20) as usize, "a quarter of the area");
    }

    #[test]
    fn a_dissolve_scales_the_alpha_and_a_bad_progress_is_safe() {
        let layer = FrameBuffer::solid(8, 8, [100, 100, 100, 255]);
        let half = apply(&layer, &Resolved::Dissolve, 0.5);
        assert!((i32::from(half.pixel(3, 3)[3]) - 128).abs() <= 1);
        assert_eq!(half.pixel(3, 3)[0], 100, "the colour is not darkened");
        assert_eq!(
            apply(&layer, &Resolved::Dissolve, f32::NAN),
            layer,
            "a bad progress shows the layer"
        );
        assert_eq!(apply(&layer, &Resolved::Dissolve, 7.0), layer);
        let empty = FrameBuffer {
            width: 0,
            height: 0,
            rgba: Vec::new(),
        };
        assert_eq!(apply(&empty, &Resolved::Grow, 0.5), empty);
    }

    #[test]
    fn the_request_key_follows_the_kind_the_values_and_the_progress() {
        use std::collections::hash_map::DefaultHasher;
        let key = |r: &TransitionRequest| {
            let mut h = DefaultHasher::new();
            r.hash_into(&mut h);
            h.finish()
        };
        let base = TransitionRequest {
            kind: IRIS.into(),
            params: BTreeMap::new(),
            progress: 0.5,
        };
        let mut other = base.clone();
        other.progress = 0.51;
        let mut third = base.clone();
        third.params.insert("softness".into(), 0.3);
        assert_ne!(key(&base), key(&other));
        assert_ne!(key(&base), key(&third));
        assert_eq!(key(&base), key(&base.clone()));
    }
}
