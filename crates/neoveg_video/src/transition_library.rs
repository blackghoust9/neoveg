//! The library the Transitions tab shows: the categories the transitions are filed in, the presets
//! (a transition with values chosen for a purpose) and the small pictures that show what each one
//! does.

use std::collections::BTreeMap;

use crate::frame::FrameBuffer;
use crate::transitions::{self, apply2, resolve, spec};

/// Where a transition is filed in the tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    Dissolve,
    Wipes,
    Shapes,
    Motion,
    ThreeD,
    Effects,
}

impl Category {
    /// Every category, in the order of the tab.
    pub const ALL: [Category; 6] = [
        Category::Dissolve,
        Category::Wipes,
        Category::Shapes,
        Category::Motion,
        Category::ThreeD,
        Category::Effects,
    ];

    /// Name in Brazilian Portuguese.
    pub fn label(self) -> &'static str {
        match self {
            Category::Dissolve => "Dissolver",
            Category::Wipes => "Cortinas",
            Category::Shapes => "Formas",
            Category::Motion => "Movimento",
            Category::ThreeD => "3D",
            Category::Effects => "Efeitos",
        }
    }
}

/// The category of a transition (the registry id); `Effects` for an id this version does not know.
pub fn category(id: &str) -> Category {
    use transitions::*;
    match id {
        DISSOLVE => Category::Dissolve,
        LINEAR_WIPE | CLOCK_WIPE | BARN_DOOR | BLINDS | GRADIENT_WIPE => Category::Wipes,
        IRIS | GROW | SPIRAL | SPLIT => Category::Shapes,
        SLIDE | PUSH | SQUEEZE => Category::Motion,
        PAGE_ROLL | PAGE_PEEL | CUBE | FLIP => Category::ThreeD,
        _ => Category::Effects,
    }
}

/// The transitions of a category, in the order of the registry.
pub fn in_category(category_wanted: Category) -> impl Iterator<Item = &'static str> {
    transitions::transitions()
        .iter()
        .map(|t| t.id)
        .filter(move |id| category(id) == category_wanted)
}

/// A transition with values chosen for a purpose.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Preset {
    pub id: &'static str,
    /// Name in Brazilian Portuguese.
    pub label: &'static str,
    /// The registry id of the transition.
    pub kind: &'static str,
    /// The values that differ from the defaults.
    pub params: &'static [(&'static str, f32)],
}

impl Preset {
    pub fn values(&self) -> BTreeMap<String, f32> {
        self.params
            .iter()
            .map(|&(id, value)| (id.to_string(), value))
            .collect()
    }
}

macro_rules! preset {
    ($id:literal, $label:literal, $kind:expr, [$(($p:literal, $v:expr)),* $(,)?]) => {
        Preset { id: $id, label: $label, kind: $kind, params: &[$(($p, $v)),*] }
    };
}

use transitions::{
    BARN_DOOR, BLINDS, CLOCK_WIPE, CUBE, FLIP, IRIS, LINEAR_WIPE, PAGE_PEEL, PAGE_ROLL, PUSH,
    SLIDE, SPIRAL,
};

/// Every preset, in the order of the tab.
pub const PRESETS: &[Preset] = &[
    preset!(
        "wipe_left",
        "Cortina da esquerda",
        LINEAR_WIPE,
        [("angle", 0.0)]
    ),
    preset!(
        "wipe_right",
        "Cortina da direita",
        LINEAR_WIPE,
        [("angle", 180.0)]
    ),
    preset!(
        "wipe_top",
        "Cortina de cima",
        LINEAR_WIPE,
        [("angle", 90.0)]
    ),
    preset!(
        "wipe_bottom",
        "Cortina de baixo",
        LINEAR_WIPE,
        [("angle", 270.0)]
    ),
    preset!(
        "wipe_diagonal",
        "Cortina na diagonal",
        LINEAR_WIPE,
        [("angle", 45.0)]
    ),
    preset!(
        "wipe_soft",
        "Cortina de borda macia",
        LINEAR_WIPE,
        [("angle", 0.0), ("softness", 0.5)]
    ),
    preset!(
        "clock_hard",
        "Relógio de borda dura",
        CLOCK_WIPE,
        [("softness", 0.0)]
    ),
    preset!("iris_hard", "Íris de borda dura", IRIS, [("softness", 0.0)]),
    preset!(
        "iris_soft",
        "Íris de borda macia",
        IRIS,
        [("softness", 0.4)]
    ),
    preset!(
        "door_vertical",
        "Porta vertical",
        BARN_DOOR,
        [("angle", 0.0)]
    ),
    preset!(
        "door_horizontal",
        "Porta horizontal",
        BARN_DOOR,
        [("angle", 90.0)]
    ),
    preset!(
        "blinds_thin",
        "Persianas finas",
        BLINDS,
        [("count", 30.0), ("angle", 0.0)]
    ),
    preset!(
        "blinds_wide",
        "Persianas largas",
        BLINDS,
        [("count", 4.0), ("angle", 0.0)]
    ),
    preset!(
        "blinds_horizontal",
        "Persianas horizontais",
        BLINDS,
        [("count", 10.0), ("angle", 90.0)]
    ),
    preset!(
        "slide_left",
        "Deslizar da esquerda",
        SLIDE,
        [("angle", 0.0)]
    ),
    preset!(
        "slide_right",
        "Deslizar da direita",
        SLIDE,
        [("angle", 180.0)]
    ),
    preset!("slide_top", "Deslizar de cima", SLIDE, [("angle", 90.0)]),
    preset!(
        "slide_bottom",
        "Deslizar de baixo",
        SLIDE,
        [("angle", 270.0)]
    ),
    preset!(
        "push_left",
        "Empurrar para a esquerda",
        PUSH,
        [("angle", 180.0), ("push_off", 1.0)]
    ),
    preset!(
        "push_right",
        "Empurrar para a direita",
        PUSH,
        [("angle", 0.0), ("push_off", 1.0)]
    ),
    preset!("spiral_fast", "Espiral rápida", SPIRAL, [("turns", 3.0)]),
    preset!(
        "roll_left",
        "Página enrolada da direita",
        PAGE_ROLL,
        [("angle", 180.0)]
    ),
    preset!(
        "peel_corner",
        "Página virada da esquerda",
        PAGE_PEEL,
        [("angle", 0.0)]
    ),
    preset!(
        "cube_left",
        "Cubo para a esquerda",
        CUBE,
        [("angle", 180.0)]
    ),
    preset!("cube_right", "Cubo para a direita", CUBE, [("angle", 0.0)]),
    preset!("cube_up", "Cubo para cima", CUBE, [("angle", 270.0)]),
    preset!(
        "cube_strong",
        "Cubo de perspectiva forte",
        CUBE,
        [("angle", 180.0), ("perspective", 1.0)]
    ),
    preset!(
        "flip_horizontal",
        "Virar na horizontal",
        FLIP,
        [("angle", 0.0)]
    ),
    preset!(
        "flip_vertical",
        "Virar na vertical",
        FLIP,
        [("angle", 90.0)]
    ),
];

/// The presets of one transition.
pub fn presets_of(kind: &str) -> impl Iterator<Item = &'static Preset> + '_ {
    PRESETS.iter().filter(move |p| p.kind == kind)
}

/// The preset with this id.
pub fn preset(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id)
}

/// The preset whose transition and values are the ones given (the values that are missing count as
/// the defaults), when there is one.
pub fn matching_preset(kind: &str, params: &BTreeMap<String, f32>) -> Option<&'static Preset> {
    let spec = spec(kind)?;
    let value_of = |source: &BTreeMap<String, f32>, id: &str| {
        let p = spec.param(id)?;
        Some(match source.get(id) {
            Some(v) if v.is_finite() => v.clamp(p.min, p.max),
            _ => p.default,
        })
    };
    presets_of(kind).find(|preset| {
        let wanted = preset.values();
        spec.params.iter().all(|p| {
            let a = value_of(&wanted, p.id);
            let b = value_of(params, p.id);
            matches!((a, b), (Some(a), Some(b)) if (a - b).abs() < 1e-4)
        })
    })
}

/// A picture for the thumbnails: a diagonal gradient with a ring and a bar, of colours that tell
/// the two clips and the direction of a wipe apart.
fn test_card(width: u32, height: u32, hue: [f32; 3]) -> FrameBuffer {
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    let (w, h) = (width as f32, height as f32);
    for y in 0..height {
        for x in 0..width {
            let (u, v) = (x as f32 / w, y as f32 / h);
            let ramp = 0.35 + 0.45 * (0.5 * u + 0.5 * v);
            let (dx, dy) = ((u - 0.5) * w / h, v - 0.5);
            let ring = (dx.hypot(dy) - 0.28).abs() < 0.045;
            let bar = (u - 0.14).abs() < 0.03;
            let light = if ring || bar { 1.0 } else { ramp };
            for c in hue {
                rgba.push((c * light * 255.0).clamp(0.0, 255.0) as u8);
            }
            rgba.push(255);
        }
    }
    FrameBuffer {
        width,
        height,
        rgba,
    }
}

/// What the transition `kind` with `params` looks like at `progress` over two test pictures, `width`
/// by `height` pixels: the picture that goes (warm) and the one that comes (cool). `None` for an id
/// the registry does not know.
pub fn preview(
    kind: &str,
    params: &BTreeMap<String, f32>,
    width: u32,
    height: u32,
    progress: f32,
) -> Option<FrameBuffer> {
    let resolved = resolve(kind, params)?;
    let outgoing = test_card(width, height, [1.0, 0.55, 0.2]);
    let incoming = test_card(width, height, [0.25, 0.6, 1.0]);
    let layer = apply2(Some(&outgoing), &incoming, &resolved, progress);
    // Under the layer: the clip before, or, when the transition draws it itself, a dark ground
    let ground = if transitions::needs_outgoing(kind) {
        FrameBuffer::solid(width, height, [22, 24, 30, 255])
    } else {
        outgoing
    };
    let mut rgba = ground.rgba;
    for (out, top) in rgba
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(layer.rgba.as_chunks::<4>().0)
    {
        let a = u32::from(top[3]);
        for c in 0..3 {
            out[c] = ((u32::from(top[c]) * a + u32::from(out[c]) * (255 - a) + 127) / 255) as u8;
        }
        out[3] = 255;
    }
    Some(FrameBuffer {
        width,
        height,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_transition_is_in_exactly_one_category() {
        for t in transitions::transitions() {
            let owners: Vec<_> = Category::ALL
                .iter()
                .filter(|c| in_category(**c).any(|id| id == t.id))
                .collect();
            assert_eq!(owners.len(), 1, "{} is in {owners:?}", t.id);
            // Only the light and the optical-flow morph are in Effects
            assert_eq!(
                category(t.id) == Category::Effects,
                [transitions::FLASH, transitions::WARP_FLOW].contains(&t.id),
                "{} is in the wrong category",
                t.id
            );
        }
        assert_eq!(category("from_the_future"), Category::Effects);
    }

    #[test]
    fn every_preset_names_a_real_transition_and_values_inside_the_ranges() {
        let mut seen = std::collections::BTreeSet::new();
        for p in PRESETS {
            assert!(seen.insert(p.id), "{} is repeated", p.id);
            let spec = spec(p.kind).unwrap_or_else(|| panic!("{}: no such transition", p.id));
            for &(id, value) in p.params {
                let param = spec
                    .param(id)
                    .unwrap_or_else(|| panic!("{}: no such value {id}", p.id));
                assert!(
                    (param.min..=param.max).contains(&value),
                    "{}: {id} = {value} is out of range",
                    p.id
                );
            }
            assert!(resolve(p.kind, &p.values()).is_some());
            assert_eq!(preset(p.id), Some(p));
        }
    }

    #[test]
    fn a_preset_is_found_again_by_its_values() {
        let p = preset("wipe_top").unwrap();
        assert_eq!(matching_preset(p.kind, &p.values()), Some(p));
        // Values left at the default count: "wipe_left" is angle 0 and the softness is the default
        assert_eq!(
            matching_preset(LINEAR_WIPE, &BTreeMap::new()).map(|p| p.id),
            Some("wipe_left")
        );
        let changed = BTreeMap::from([("angle".to_string(), 33.0)]);
        assert!(matching_preset(LINEAR_WIPE, &changed).is_none());
    }

    #[test]
    fn a_thumbnail_shows_the_clip_that_goes_before_and_the_one_that_comes_after() {
        let none = BTreeMap::new();
        let (w, h) = (48, 32);
        let start = preview(LINEAR_WIPE, &none, w, h, 0.0).unwrap();
        let end = preview(LINEAR_WIPE, &none, w, h, 1.0).unwrap();
        assert_ne!(start, end);
        // Warm before, cool after
        let (a, b) = (start.pixel(30, 24), end.pixel(30, 24));
        assert!(a[0] > a[2] && b[2] > b[0], "{a:?} {b:?}");
        // Every transition draws a full, opaque picture half way
        for t in transitions::transitions() {
            let half = preview(t.id, &none, w, h, 0.5).unwrap();
            assert_eq!(half.rgba.len(), (w * h * 4) as usize);
            assert!(
                half.rgba.as_chunks::<4>().0.iter().all(|p| p[3] == 255),
                "{}",
                t.id
            );
        }
        assert!(preview("from_the_future", &none, w, h, 0.5).is_none());
    }

    /// Writes a sheet with every preset at 25, 50 and 75 %, to look at (`NEOVEG_THUMB_SHEET` names the
    /// PNG file): `cargo test -p neoveg_video --lib contact_sheet -- --ignored`.
    #[test]
    #[ignore = "writes a picture to look at"]
    fn contact_sheet() {
        let Some(path) = std::env::var_os("NEOVEG_THUMB_SHEET") else {
            return;
        };
        let (w, h) = (96u32, 54u32);
        let rows = PRESETS.len() as u32;
        let mut sheet = image::RgbaImage::new(w * 3, h * rows);
        for (row, p) in PRESETS.iter().enumerate() {
            for (col, progress) in [0.25f32, 0.5, 0.75].into_iter().enumerate() {
                let f = preview(p.kind, &p.values(), w, h, progress).unwrap();
                for y in 0..h {
                    for x in 0..w {
                        sheet.put_pixel(
                            col as u32 * w + x,
                            row as u32 * h + y,
                            image::Rgba(f.pixel(x, y)),
                        );
                    }
                }
            }
        }
        sheet.save(path).unwrap();
    }
}
