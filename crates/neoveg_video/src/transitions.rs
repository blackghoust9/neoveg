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
pub const SPLIT: &str = "split";
pub const PUSH: &str = "push";
pub const PAGE_ROLL: &str = "page_roll";
pub const PAGE_PEEL: &str = "page_peel";
pub const GRADIENT_WIPE: &str = "gradient_wipe";

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
            EffectSpec {
                id: SPLIT,
                label: "Dividir",
                params: leak(vec![
                    ParamSpec {
                        id: "center_x",
                        label: "Centro horizontal",
                        min: 0.0,
                        max: 1.0,
                        default: 0.5,
                        percent: true,
                    },
                    ParamSpec {
                        id: "center_y",
                        label: "Centro vertical",
                        min: 0.0,
                        max: 1.0,
                        default: 0.5,
                        percent: true,
                    },
                    ParamSpec {
                        id: "mode",
                        label: "Modo (0 cortina, 1 empurrar)",
                        min: 0.0,
                        max: 1.0,
                        default: 0.0,
                        percent: false,
                    },
                    ParamSpec {
                        default: 0.05,
                        ..SOFTNESS
                    },
                ]),
            },
            EffectSpec {
                id: PUSH,
                label: "Empurrar",
                params: leak(vec![
                    ParamSpec {
                        id: "angle",
                        label: "Direção de entrada",
                        min: 0.0,
                        max: 360.0,
                        default: 0.0,
                        percent: false,
                    },
                    ParamSpec {
                        id: "push_off",
                        label: "Empurrar o anterior (0 ou 1)",
                        min: 0.0,
                        max: 1.0,
                        default: 1.0,
                        percent: false,
                    },
                    ParamSpec {
                        id: "border",
                        label: "Tamanho da borda",
                        min: 0.0,
                        max: 0.2,
                        default: 0.0,
                        percent: true,
                    },
                    ParamSpec {
                        id: "border_feather",
                        label: "Suavidade da borda",
                        min: 0.0,
                        max: 1.0,
                        default: 0.0,
                        percent: true,
                    },
                    ParamSpec {
                        id: "border_r",
                        label: "Borda, vermelho",
                        min: 0.0,
                        max: 1.0,
                        default: 1.0,
                        percent: true,
                    },
                    ParamSpec {
                        id: "border_g",
                        label: "Borda, verde",
                        min: 0.0,
                        max: 1.0,
                        default: 1.0,
                        percent: true,
                    },
                    ParamSpec {
                        id: "border_b",
                        label: "Borda, azul",
                        min: 0.0,
                        max: 1.0,
                        default: 1.0,
                        percent: true,
                    },
                ]),
            },
            EffectSpec {
                id: PAGE_ROLL,
                label: "Rolar página",
                params: leak(vec![
                    ParamSpec {
                        id: "angle",
                        label: "Lado de onde enrola",
                        min: 0.0,
                        max: 360.0,
                        default: 0.0,
                        percent: false,
                    },
                    ParamSpec {
                        id: "radius",
                        label: "Raio do rolo",
                        min: 0.02,
                        max: 0.3,
                        default: 0.08,
                        percent: true,
                    },
                ]),
            },
            EffectSpec {
                id: PAGE_PEEL,
                label: "Descolar página",
                params: leak(vec![ParamSpec {
                    id: "angle",
                    label: "Lado de onde descola",
                    min: 0.0,
                    max: 360.0,
                    default: 0.0,
                    percent: false,
                }]),
            },
            EffectSpec {
                id: GRADIENT_WIPE,
                label: "Cortina de gradiente",
                params: leak(vec![
                    ParamSpec {
                        id: "pattern",
                        label: "Padrão (0 a 6)",
                        min: 0.0,
                        max: 6.0,
                        default: 0.0,
                        percent: false,
                    },
                    ParamSpec {
                        id: "softness",
                        label: "Suavidade (desfoque do gradiente)",
                        min: 0.0,
                        max: 1.0,
                        default: 0.1,
                        percent: true,
                    },
                    ParamSpec {
                        id: "invert",
                        label: "Inverter (0 ou 1)",
                        min: 0.0,
                        max: 1.0,
                        default: 0.0,
                        percent: false,
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
    /// `true` for the transition that reveals the event (its fade-in or the overlap with the clip
    /// before it), `false` for the one that hides it.
    pub entering: bool,
}

impl TransitionRequest {
    /// Feeds the transition into a hasher (the worker's frame cache key).
    pub fn hash_into(&self, h: &mut impl Hasher) {
        self.kind.hash(h);
        self.progress.to_bits().hash(h);
        self.entering.hash(h);
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
    /// The picture opens from `center` (fractions of the frame, lower-left origin): as a
    /// rectangular wipe, or, with `push`, as four quarters that slide in from the corners.
    Split {
        center: [f32; 2],
        push: bool,
        softness: f32,
    },
    /// The clip before is pushed off by the one that enters from `angle` (see [`squeeze_edge`]);
    /// with `push_off` false it stays and the new one slides over it. The border (a fraction of the
    /// frame height) lies at the leading edge.
    Push {
        angle: f32,
        push_off: bool,
        border: f32,
        feather: f32,
        color: [f32; 3],
    },
    /// The clip before rolls up from `angle` and shows the one under it; `radius` is a fraction of
    /// the frame along the way.
    PageRoll {
        angle: f32,
        radius: f32,
    },
    /// The clip before is peeled back from `angle` and folded over itself.
    PagePeel {
        angle: f32,
    },
    /// A wipe that follows a built-in gradient (see [`gradient_value`]).
    GradientWipe {
        pattern: u32,
        softness: f32,
        invert: bool,
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
        SPLIT => Resolved::Split {
            center: [v("center_x"), v("center_y")],
            push: v("mode") >= 0.5,
            softness: v("softness"),
        },
        PUSH => Resolved::Push {
            angle: v("angle"),
            push_off: v("push_off") >= 0.5,
            border: v("border"),
            feather: v("border_feather"),
            color: [v("border_r"), v("border_g"), v("border_b")],
        },
        PAGE_ROLL => Resolved::PageRoll {
            angle: v("angle"),
            radius: v("radius"),
        },
        PAGE_PEEL => Resolved::PagePeel { angle: v("angle") },
        GRADIENT_WIPE => Resolved::GradientWipe {
            pattern: v("pattern").round() as u32,
            softness: v("softness"),
            invert: v("invert") >= 0.5,
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

/// Distance from `center` to the farther edge of a frame `size` long (pixels).
pub fn split_extent(center: f32, size: f32) -> f32 {
    center.max(size - center)
}

/// `true` for the transitions that draw the clip before the one they reveal (the outgoing layer).
/// With no clip before, they run on the incoming layer alone, as if the outgoing one were empty.
pub fn needs_outgoing(kind: &str) -> bool {
    matches!(kind, PUSH | PAGE_ROLL | PAGE_PEEL)
}

/// Where a point is along the way of a transition that enters from `edge` (see [`squeeze_edge`]):
/// the distance from that side and the position across it.
fn travel(edge: u32, x: f32, y: f32, w: f32, h: f32) -> (f32, f32) {
    match edge {
        0 => (x, y),
        1 => (y, x),
        2 => (w - x, y),
        _ => (h - y, x),
    }
}

/// The point of the frame at distance `s` from `edge` and `lat` across (the opposite of [`travel`]).
fn point_at(edge: u32, s: f32, lat: f32, w: f32, h: f32) -> (f32, f32) {
    match edge {
        0 => (s, lat),
        1 => (lat, s),
        2 => (w - s, lat),
        _ => (lat, h - s),
    }
}

/// Length of the frame along the way of a transition that enters from `edge`.
fn extent(edge: u32, w: f32, h: f32) -> f32 {
    if edge.is_multiple_of(2) {
        w
    } else {
        h
    }
}

/// `top` drawn over `bottom` (premultiplied, `0..=255`).
fn over(top: [f32; 4], bottom: [f32; 4]) -> [f32; 4] {
    let keep = 1.0 - top[3] / 255.0;
    [
        top[0] + bottom[0] * keep,
        top[1] + bottom[1] * keep,
        top[2] + bottom[2] * keep,
        top[3] + bottom[3] * keep,
    ]
}

fn shade(p: [f32; 4], k: f32) -> [f32; 4] {
    [p[0] * k, p[1] * k, p[2] * k, p[3]]
}

/// How far the front of a page roll has gone for a roll of radius `r` over a frame `e` long, and
/// the radius the roll has by then (it grows from nothing until it is round).
pub fn roll_front(e: f32, r: f32, progress: f32) -> (f32, f32) {
    let total = e + (1.5 * std::f32::consts::PI + 1.0) * r;
    let front = progress * total;
    (front, r.min(front / (1.5 * std::f32::consts::PI)))
}

/// The value `0..1` a point `(u, v)` (`0..1` over the frame) has in the gradient `pattern`: the
/// transition reveals the low values first.
pub fn gradient_value(pattern: u32, u: f32, v: f32, aspect: f32) -> f32 {
    match pattern {
        0 => u,
        1 => v,
        2 => (u * aspect + v) / (aspect + 1.0),
        3 => ((u - 0.5) * aspect).hypot(v - 0.5) / (aspect / 2.0).hypot(0.5),
        4 => 1.0 - gradient_value(3, u, v, aspect),
        5 => {
            let turn = ((u - 0.5) * aspect).atan2(-(v - 0.5)) / std::f32::consts::TAU;
            turn - turn.floor()
        }
        _ => (((u - 0.5).abs() * aspect) + (v - 0.5).abs()) / (aspect / 2.0 + 0.5),
    }
}

/// The picture at `progress` of a transition that sees both layers: `outgoing` is the clip before
/// (`None` when there is none), `incoming` the clip it reveals. Both have the size of the canvas.
pub fn apply2(
    outgoing: Option<&FrameBuffer>,
    incoming: &FrameBuffer,
    t: &Resolved,
    progress: f32,
) -> FrameBuffer {
    let progress = if progress.is_finite() {
        progress.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let (width, height) = (incoming.width, incoming.height);
    if width == 0 || height == 0 || progress >= 1.0 {
        return incoming.clone();
    }
    let outgoing = outgoing.filter(|o| o.width == width && o.height == height);
    // A roll or a peel with no clip before still uncovers the picture the way a page would: the
    // page is taken as an opaque sheet and only what it leaves free shows
    let ghost_page =
        outgoing.is_none() && matches!(t, Resolved::PageRoll { .. } | Resolved::PagePeel { .. });
    let sheet = FrameBuffer::solid(width, height, [0, 0, 0, 255]);
    let outgoing = if ghost_page { Some(&sheet) } else { outgoing };
    let out_at = |x: f32, y: f32| outgoing.map_or([0.0; 4], |o| sample(o, x, y));
    let finish = |page: [f32; 4], inc: [f32; 4]| {
        if ghost_page {
            inc.map(|v| v * (1.0 - page[3] / 255.0))
        } else {
            over(page, inc)
        }
    };
    let (w, h) = (width as f32, height as f32);
    match *t {
        Resolved::Push {
            angle,
            push_off,
            border,
            feather,
            color,
        } => {
            let edge = squeeze_edge(angle);
            let e = extent(edge, w, h);
            let (dx, dy) = [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)][edge as usize];
            let lead = progress * e;
            let thick = border * h;
            render(width, height, |x, y| {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let entering = (1.0 - progress) * e;
                let inc = sample(incoming, px + dx * entering, py + dy * entering);
                let out = if push_off {
                    out_at(px - dx * lead, py - dy * lead)
                } else {
                    out_at(px, py)
                };
                let mut below = out;
                let (s, _) = travel(edge, px, py, w, h);
                if thick > 0.0 && progress > 0.0 && s >= lead && s < lead + thick {
                    let ramp = feather * thick;
                    let a = if ramp <= 0.0 {
                        1.0
                    } else {
                        ((lead + thick - s) / ramp).clamp(0.0, 1.0)
                    };
                    let strip = [
                        color[0] * 255.0 * a,
                        color[1] * 255.0 * a,
                        color[2] * 255.0 * a,
                        255.0 * a,
                    ];
                    below = over(strip, out);
                }
                over(inc, below)
            })
        }
        Resolved::PageRoll { angle, radius } => {
            let edge = squeeze_edge(angle);
            let e = extent(edge, w, h);
            let (front, r) = roll_front(e, radius * e, progress);
            render(width, height, |x, y| {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let (s, lat) = travel(edge, px, py, w, h);
                let inc = sample(incoming, px, py);
                let flat = if s >= front { out_at(px, py) } else { [0.0; 4] };
                let page = if r > 0.0 && (s - front).abs() <= r {
                    let u = ((front - s) / r).clamp(-1.0, 1.0);
                    let arc = r * (std::f32::consts::PI - u.asin());
                    let source = front - arc;
                    if source >= 0.0 {
                        let (sx, sy) = point_at(edge, source, lat, w, h);
                        over(
                            shade(out_at(sx, sy), 0.35 + 0.65 * (1.0 - u * u).sqrt()),
                            flat,
                        )
                    } else {
                        flat
                    }
                } else {
                    flat
                };
                finish(page, inc)
            })
        }
        Resolved::PagePeel { angle } => {
            let edge = squeeze_edge(angle);
            let e = extent(edge, w, h);
            let front = progress * e;
            render(width, height, |x, y| {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let (s, lat) = travel(edge, px, py, w, h);
                let inc = sample(incoming, px, py);
                if s < front {
                    return inc;
                }
                let flat = out_at(px, py);
                let page = if s <= 2.0 * front {
                    let (sx, sy) = point_at(edge, 2.0 * front - s, lat, w, h);
                    over(shade(out_at(sx, sy), 0.6), flat)
                } else {
                    flat
                };
                finish(page, inc)
            })
        }
        _ => apply(incoming, t, progress),
    }
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
        | Resolved::Flash { .. }
        | Resolved::Split { push: true, .. }
        | Resolved::Push { .. }
        | Resolved::PageRoll { .. }
        | Resolved::PagePeel { .. } => 1.0,
        Resolved::GradientWipe {
            pattern,
            softness,
            invert,
        } => {
            let g = gradient_value(pattern, u, v, aspect);
            reveal(if invert { 1.0 - g } else { g }, progress, softness)
        }
        Resolved::Split {
            center, softness, ..
        } => {
            let (cx, cy) = (center[0], 1.0 - center[1]);
            let (ex, ey) = (cx.max(1.0 - cx) * aspect, cy.max(1.0 - cy));
            let s = (((u - cx).abs() * aspect) / ex).max((v - cy).abs() / ey);
            reveal(s, progress, softness)
        }
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
    if matches!(
        t,
        Resolved::Push { .. } | Resolved::PageRoll { .. } | Resolved::PagePeel { .. }
    ) {
        // No clip before: the outgoing layer is empty
        return apply2(None, layer, t, progress);
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
        Resolved::Split {
            center, push: true, ..
        } => {
            let (cx, cy) = (center[0] * w, (1.0 - center[1]) * h);
            let (ex, ey) = (split_extent(cx, w), split_extent(cy, h));
            let (off_x, off_y) = ((1.0 - progress) * ex, (1.0 - progress) * ey);
            render(layer.width, layer.height, |x, y| {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let (sx, sy) = (
                    if px >= cx { 1.0 } else { -1.0 },
                    if py >= cy { 1.0 } else { -1.0 },
                );
                // The quadrant moved outward; this pixel shows what was `off` closer to the centre
                let (qx, qy) = (px - sx * off_x, py - sy * off_y);
                if (qx >= cx) != (sx > 0.0) || (qy >= cy) != (sy > 0.0) {
                    [0.0; 4]
                } else {
                    sample(layer, qx, qy)
                }
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
    fn the_transitions_that_see_the_outgoing_layer_move_both_pictures() {
        // Outgoing: solid red. Incoming: solid blue. 80 by 40 pixels
        let out = FrameBuffer::solid(80, 40, [255, 0, 0, 255]);
        let inc = FrameBuffer::solid(80, 40, [0, 0, 255, 255]);
        let at = |f: &FrameBuffer, x: usize, y: usize| {
            let i = (y * 80 + x) * 4;
            [f.rgba[i], f.rgba[i + 1], f.rgba[i + 2], f.rgba[i + 3]]
        };
        let none = BTreeMap::new();
        for kind in [PUSH, PAGE_ROLL, PAGE_PEEL] {
            assert!(needs_outgoing(kind));
            let t = resolve(kind, &none).unwrap();
            // The end is the incoming layer, the start is the outgoing one, untouched
            assert_eq!(apply2(Some(&out), &inc, &t, 1.0), inc, "{kind} ends");
            assert_eq!(apply2(Some(&out), &inc, &t, 0.0), out, "{kind} starts");
        }
        assert!(!needs_outgoing(SLIDE) && !needs_outgoing(GRADIENT_WIPE));
        // Push from the left at a half: the incoming fills the left half, the outgoing was pushed
        // to the right half
        let push = resolve(PUSH, &none).unwrap();
        let half = apply2(Some(&out), &inc, &push, 0.5);
        assert_eq!(
            (at(&half, 10, 20), at(&half, 70, 20)),
            ([0, 0, 255, 255], [255, 0, 0, 255])
        );
        // Without push-off the outgoing stays where it was and the incoming slides over it
        let over_it = resolve(PUSH, &BTreeMap::from([("push_off".to_owned(), 0.0)])).unwrap();
        let half = apply2(Some(&out), &inc, &over_it, 0.5);
        assert_eq!(
            (at(&half, 10, 20), at(&half, 70, 20)),
            ([0, 0, 255, 255], [255, 0, 0, 255])
        );
        // A border: a green strip at the leading edge
        let bordered = resolve(
            PUSH,
            &BTreeMap::from([
                ("border".to_owned(), 0.25),
                ("border_r".to_owned(), 0.0),
                ("border_b".to_owned(), 0.0),
            ]),
        )
        .unwrap();
        let half = apply2(Some(&out), &inc, &bordered, 0.5);
        assert_eq!(at(&half, 45, 20), [0, 255, 0, 255], "inside the strip");
        assert_eq!(at(&half, 70, 20), [255, 0, 0, 255], "beyond it");
        // Without an outgoing layer the push is a slide in: nothing on the far side
        let alone = apply2(None, &inc, &push, 0.5);
        assert_eq!((at(&alone, 10, 20)[3], at(&alone, 70, 20)[3]), (255, 0));
        // The page roll: the incoming shows where the page has gone, the flat page stays beyond
        let roll = resolve(PAGE_ROLL, &none).unwrap();
        let mid = apply2(Some(&out), &inc, &roll, 0.5);
        assert_eq!(at(&mid, 2, 20), [0, 0, 255, 255]);
        assert_eq!(at(&mid, 78, 20), [255, 0, 0, 255]);
        // The peel: the incoming on the side that was peeled, the flap (darker red) next to it
        let peel = resolve(PAGE_PEEL, &none).unwrap();
        let mid = apply2(Some(&out), &inc, &peel, 0.25);
        assert_eq!(at(&mid, 5, 20), [0, 0, 255, 255]);
        let flap = at(&mid, 30, 20);
        assert!(flap[0] > 100 && flap[0] < 255 && flap[2] == 0, "{flap:?}");
        assert_eq!(at(&mid, 70, 20), [255, 0, 0, 255]);
    }

    #[test]
    fn the_gradient_wipe_reveals_by_the_pattern_and_can_be_inverted() {
        let layer = FrameBuffer::solid(80, 40, [255, 255, 255, 255]);
        let alpha = |f: &FrameBuffer, x: usize, y: usize| f.rgba[(y * 80 + x) * 4 + 3];
        let wipe = |pattern: f32, invert: f32| {
            resolve(
                GRADIENT_WIPE,
                &BTreeMap::from([
                    ("pattern".to_owned(), pattern),
                    ("invert".to_owned(), invert),
                    ("softness".to_owned(), 0.0),
                ]),
            )
            .unwrap()
        };
        // Pattern 0 is left to right
        let half = apply(&layer, &wipe(0.0, 0.0), 0.5);
        assert_eq!((alpha(&half, 10, 20), alpha(&half, 70, 20)), (255, 0));
        let half = apply(&layer, &wipe(0.0, 1.0), 0.5);
        assert_eq!((alpha(&half, 10, 20), alpha(&half, 70, 20)), (0, 255));
        // The radial pattern opens from the centre
        let half = apply(&layer, &wipe(3.0, 0.0), 0.5);
        assert_eq!((alpha(&half, 40, 20), alpha(&half, 1, 1)), (255, 0));
        // Every pattern is a value between 0 and 1 and the ends are empty and whole
        for pattern in 0..7 {
            for (u, v) in [(0.0, 0.0), (0.5, 0.5), (1.0, 1.0), (0.2, 0.9)] {
                let g = gradient_value(pattern, u, v, 2.0);
                assert!((0.0..=1.0).contains(&g), "{pattern} at {u},{v}: {g}");
            }
            let t = wipe(pattern as f32, 0.0);
            assert_eq!(apply(&layer, &t, 1.0), layer);
            assert!(apply(&layer, &t, 0.0)
                .rgba
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| p[3] == 0));
        }
    }

    #[test]
    fn split_opens_from_the_centre_and_pushes_the_quarters_in_from_the_corners() {
        let layer = crate::generators::smpte_color_bars(80, 40);
        let alpha = |f: &FrameBuffer, x: usize, y: usize| f.rgba[(y * 80 + x) * 4 + 3];
        let wipe = resolve(SPLIT, &BTreeMap::new()).unwrap();
        assert_eq!(apply(&layer, &wipe, 1.0), layer);
        assert!(apply(&layer, &wipe, 0.0)
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| p[3] == 0));
        let half = apply(&layer, &wipe, 0.5);
        assert_eq!(
            (
                alpha(&half, 40, 20),
                alpha(&half, 0, 0),
                alpha(&half, 79, 39)
            ),
            (255, 0, 0)
        );
        let push = resolve(SPLIT, &BTreeMap::from([("mode".to_owned(), 1.0)])).unwrap();
        assert_eq!(apply(&layer, &push, 1.0), layer);
        assert!(apply(&layer, &push, 0.0)
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| p[3] == 0));
        let half = apply(&layer, &push, 0.5);
        assert_eq!(
            (
                alpha(&half, 40, 20),
                alpha(&half, 0, 0),
                alpha(&half, 79, 39)
            ),
            (0, 255, 255)
        );
        // The centre can move: the farthest corner is the one that is reached last
        let off = resolve(
            SPLIT,
            &BTreeMap::from([("center_x".to_owned(), 0.0), ("center_y".to_owned(), 1.0)]),
        )
        .unwrap();
        let half = apply(&layer, &off, 0.5);
        assert_eq!((alpha(&half, 0, 0), alpha(&half, 79, 39)), (255, 0));
    }

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
        assert_eq!(transitions().len(), 16);
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
            entering: true,
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
