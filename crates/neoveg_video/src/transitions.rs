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
pub const CROSS_DISSOLVE: &str = "cross_dissolve";
pub const LINEAR_WIPE: &str = "linear_wipe";
pub const CLOCK_WIPE: &str = "clock_wipe";
pub const RADIAL_WIPE: &str = "radial_wipe";
pub const IRIS: &str = "iris";
pub const BARN_DOOR: &str = "barn_door";
pub const BLINDS: &str = "blinds";
pub const SLIDE: &str = "slide";
pub const GROW: &str = "grow";
pub const ZOOM: &str = "zoom";
pub const SQUEEZE: &str = "squeeze";
pub const SPIRAL: &str = "spiral";
pub const FLASH: &str = "flash";
pub const SPLIT: &str = "split";
pub const PUSH: &str = "push";
pub const PAGE_ROLL: &str = "page_roll";
pub const PAGE_PEEL: &str = "page_peel";
pub const GRADIENT_WIPE: &str = "gradient_wipe";
pub const WARP_FLOW: &str = "warp_flow";
pub const CUBE: &str = "cube";
pub const FLIP: &str = "flip";
pub const FLY_IN: &str = "fly_in";
pub const THREE_D_FLY_IN: &str = "3d_fly_in";
pub const SHUFFLE: &str = "shuffle";
pub const THREE_D_SHUFFLE: &str = "3d_shuffle";

/// The 6 standard categories for video transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TransitionCategory {
    Dissolve,
    Wipes,
    Shapes,
    Motion,
    ThreeD,
    Effects,
}

impl TransitionCategory {
    pub const ALL: [TransitionCategory; 6] = [
        TransitionCategory::Dissolve,
        TransitionCategory::Wipes,
        TransitionCategory::Shapes,
        TransitionCategory::Motion,
        TransitionCategory::ThreeD,
        TransitionCategory::Effects,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TransitionCategory::Dissolve => "Dissolver",
            TransitionCategory::Wipes => "Cortinas",
            TransitionCategory::Shapes => "Formas",
            TransitionCategory::Motion => "Movimento",
            TransitionCategory::ThreeD => "3D",
            TransitionCategory::Effects => "Efeitos",
        }
    }
}

/// A transition preset definition with its parameter assignments.
#[derive(Debug, Clone, PartialEq)]
pub struct TransitionPreset {
    pub id: &'static str,
    pub category: TransitionCategory,
    pub name_key: &'static str,
    pub params: BTreeMap<String, f32>,
}

/// 3D spatial transformation parameters for perspective transitions.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreeDTransform {
    pub rotation_x_deg: f32,
    pub rotation_y_deg: f32,
    pub rotation_z_deg: f32,
    pub depth_z: f32,
    pub specular_light: f32,
}

impl Default for ThreeDTransform {
    fn default() -> Self {
        Self {
            rotation_x_deg: 0.0,
            rotation_y_deg: 0.0,
            rotation_z_deg: 0.0,
            depth_z: 0.0,
            specular_light: 0.0,
        }
    }
}

impl ThreeDTransform {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_rotations(mut self, rx: f32, ry: f32, rz: f32) -> Self {
        self.rotation_x_deg = rx;
        self.rotation_y_deg = ry;
        self.rotation_z_deg = rz;
        self
    }

    pub fn with_depth(mut self, depth: f32) -> Self {
        self.depth_z = depth;
        self
    }

    pub fn with_specular(mut self, specular: f32) -> Self {
        self.specular_light = specular.clamp(0.0, 1.0);
        self
    }

    /// Computes rotation matrix R = Rz * Ry * Rx in row-major order.
    pub fn rotation_matrix(&self) -> [[f32; 3]; 3] {
        let (sx, cx) = self.rotation_x_deg.to_radians().sin_cos();
        let (sy, cy) = self.rotation_y_deg.to_radians().sin_cos();
        let (sz, cz) = self.rotation_z_deg.to_radians().sin_cos();

        [
            [cy * cz, cz * sx * sy - cx * sz, cx * cz * sy + sx * sz],
            [cy * sz, cx * cz + sx * sy * sz, -cz * sx + cx * sy * sz],
            [-sy, cy * sx, cx * cy],
        ]
    }
}

/// Trait implemented by transition renderers with thumbnail support.
pub trait TransitionRenderer: Send + Sync {
    fn render(
        &self,
        source: &FrameBuffer,
        target: &FrameBuffer,
        progress: f32,
        params: &BTreeMap<String, f32>,
    ) -> Result<FrameBuffer, String>;

    fn render_thumbnail(
        &self,
        source: &FrameBuffer,
        target: &FrameBuffer,
        params: &BTreeMap<String, f32>,
    ) -> FrameBuffer {
        self.render(source, target, 0.5, params).unwrap_or_else(|_| source.clone())
    }
}

/// Keyframe in a transition progress envelope curve.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TransitionEnvelopePoint {
    pub time: f64,
    pub progress: f32,
    pub shape: TransitionEnvelopeShape,
}

/// Curve interpolation shapes for transition progress automation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum TransitionEnvelopeShape {
    #[default]
    Linear,
    Fast,
    Slow,
    Smooth,
    Sharp,
    Hold,
}

impl TransitionEnvelopeShape {
    pub fn eval(self, u: f32) -> f32 {
        let u = u.clamp(0.0, 1.0);
        match self {
            TransitionEnvelopeShape::Linear => u,
            TransitionEnvelopeShape::Fast => u.sqrt(),
            TransitionEnvelopeShape::Slow => u * u,
            TransitionEnvelopeShape::Smooth => u * u * (3.0 - 2.0 * u),
            TransitionEnvelopeShape::Sharp => {
                if u < 0.5 {
                    2.0 * u * u
                } else {
                    1.0 - 2.0 * (1.0 - u) * (1.0 - u)
                }
            }
            TransitionEnvelopeShape::Hold => {
                if u >= 1.0 {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }
}

/// Automation envelope controlling custom transition progress curves over time, clamped to 0.0..=1.0.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TransitionProgressEnvelope {
    pub points: Vec<TransitionEnvelopePoint>,
    pub enabled: bool,
}

impl Default for TransitionProgressEnvelope {
    fn default() -> Self {
        Self::new(1.0)
    }
}

impl TransitionProgressEnvelope {
    pub fn new(ramp_seconds: f64) -> Self {
        let ramp = if ramp_seconds.is_finite() && ramp_seconds > 1e-6 {
            ramp_seconds
        } else {
            1.0
        };
        Self {
            points: vec![
                TransitionEnvelopePoint {
                    time: 0.0,
                    progress: 0.0,
                    shape: TransitionEnvelopeShape::Linear,
                },
                TransitionEnvelopePoint {
                    time: ramp,
                    progress: 1.0,
                    shape: TransitionEnvelopeShape::Linear,
                },
            ],
            enabled: true,
        }
    }

    pub fn add_point(&mut self, time: f64, progress: f32, shape: TransitionEnvelopeShape) {
        let p = progress.clamp(0.0, 1.0);
        if let Some(pos) = self.points.iter().position(|pt| (pt.time - time).abs() < 1e-6) {
            self.points[pos].progress = p;
            self.points[pos].shape = shape;
        } else {
            let pos = self.points.partition_point(|pt| pt.time < time);
            self.points.insert(
                pos,
                TransitionEnvelopePoint {
                    time,
                    progress: p,
                    shape,
                },
            );
        }
    }

    pub fn progress_at(&self, time: f64) -> f32 {
        if !self.enabled || self.points.is_empty() || time.is_nan() {
            return 1.0;
        }

        if time <= self.points[0].time {
            return self.points[0].progress.clamp(0.0, 1.0);
        }

        let last = self.points.last().unwrap();
        if time >= last.time {
            return last.progress.clamp(0.0, 1.0);
        }

        let idx = self.points.partition_point(|pt| pt.time < time);
        let p1 = &self.points[idx];
        let p0 = &self.points[idx - 1];

        let dt = p1.time - p0.time;
        if dt.abs() < 1e-6 {
            return p0.progress.clamp(0.0, 1.0);
        }

        let u = ((time - p0.time) / dt) as f32;
        let factor = p0.shape.eval(u);
        let value = p0.progress + (p1.progress - p0.progress) * factor;
        value.clamp(0.0, 1.0)
    }

    pub fn is_active(&self) -> bool {
        self.enabled && !self.points.is_empty()
    }
}

/// Looks up a transition preset by its identifier or name key.
pub fn get_preset(id: &str) -> Option<TransitionPreset> {
    let lower = id.to_lowercase().replace('-', "_").replace(' ', "_");
    match lower.as_str() {
        "cross_dissolve" | "dissolve" => Some(TransitionPreset {
            id: "cross_dissolve",
            category: TransitionCategory::Dissolve,
            name_key: "Cross Dissolve",
            params: BTreeMap::new(),
        }),
        "linear_wipe" | "wipe" => Some(TransitionPreset {
            id: "linear_wipe",
            category: TransitionCategory::Wipes,
            name_key: "Linear Wipe",
            params: BTreeMap::from([("angle".into(), 0.0), ("softness".into(), 0.1)]),
        }),
        "radial_wipe" | "clock_wipe" => Some(TransitionPreset {
            id: "radial_wipe",
            category: TransitionCategory::Wipes,
            name_key: "Radial Wipe",
            params: BTreeMap::from([("start_angle".into(), 0.0), ("softness".into(), 0.05)]),
        }),
        "iris" => Some(TransitionPreset {
            id: "iris",
            category: TransitionCategory::Shapes,
            name_key: "Iris",
            params: BTreeMap::from([("softness".into(), 0.1)]),
        }),
        "push" => Some(TransitionPreset {
            id: "push",
            category: TransitionCategory::Motion,
            name_key: "Push",
            params: BTreeMap::from([("angle".into(), 0.0), ("push_off".into(), 1.0)]),
        }),
        "slide" => Some(TransitionPreset {
            id: "slide",
            category: TransitionCategory::Motion,
            name_key: "Slide",
            params: BTreeMap::from([("angle".into(), 0.0)]),
        }),
        "zoom" | "grow" => Some(TransitionPreset {
            id: "zoom",
            category: TransitionCategory::Shapes,
            name_key: "Zoom",
            params: BTreeMap::new(),
        }),
        "fly_in" | "3d_fly_in" => Some(TransitionPreset {
            id: "3d_fly_in",
            category: TransitionCategory::ThreeD,
            name_key: "3D Fly In",
            params: BTreeMap::from([
                ("far_z".into(), 4.0),
                ("rotation_x".into(), 25.0),
                ("rotation_y".into(), 35.0),
                ("rotation_z".into(), 10.0),
                ("specular_light".into(), 0.5),
            ]),
        }),
        "page_peel" => Some(TransitionPreset {
            id: "page_peel",
            category: TransitionCategory::ThreeD,
            name_key: "Page Peel",
            params: BTreeMap::from([("angle".into(), 0.0)]),
        }),
        "page_roll" => Some(TransitionPreset {
            id: "page_roll",
            category: TransitionCategory::ThreeD,
            name_key: "Page Roll",
            params: BTreeMap::from([("angle".into(), 0.0), ("radius".into(), 0.08)]),
        }),
        "cube" => Some(TransitionPreset {
            id: "cube",
            category: TransitionCategory::ThreeD,
            name_key: "Cube",
            params: BTreeMap::from([("angle".into(), 0.0), ("perspective".into(), 0.5)]),
        }),
        "shuffle" | "3d_shuffle" => Some(TransitionPreset {
            id: "shuffle",
            category: TransitionCategory::ThreeD,
            name_key: "Shuffle",
            params: BTreeMap::from([
                ("extra_spins".into(), 0.0),
                ("stagger".into(), 0.2),
                ("specular_light".into(), 0.4),
            ]),
        }),
        other => {
            let p = crate::transition_library::preset(other)?;
            Some(TransitionPreset {
                id: p.id,
                category: crate::transition_library::category(p.kind),
                name_key: p.label,
                params: p.values(),
            })
        }
    }
}

impl TransitionRenderer for TransitionPreset {
    fn render(
        &self,
        source: &FrameBuffer,
        target: &FrameBuffer,
        progress: f32,
        params: &BTreeMap<String, f32>,
    ) -> Result<FrameBuffer, String> {
        let mut merged = self.params.clone();
        for (k, v) in params {
            merged.insert(k.clone(), *v);
        }
        let kind = match self.id {
            "cross_dissolve" => DISSOLVE,
            "linear_wipe" => LINEAR_WIPE,
            "radial_wipe" => CLOCK_WIPE,
            "iris" => IRIS,
            "push" => PUSH,
            "slide" => SLIDE,
            "zoom" => GROW,
            "3d_fly_in" | "fly_in" => FLY_IN,
            "page_peel" => PAGE_PEEL,
            "page_roll" => PAGE_ROLL,
            "cube" => CUBE,
            "shuffle" | "3d_shuffle" => SHUFFLE,
            other => other,
        };
        let resolved = resolve(kind, &merged).ok_or_else(|| format!("Unknown transition preset kind: {kind}"))?;
        Ok(apply2(Some(source), target, &resolved, progress))
    }
}

impl TransitionRenderer for &str {
    fn render(
        &self,
        source: &FrameBuffer,
        target: &FrameBuffer,
        progress: f32,
        params: &BTreeMap<String, f32>,
    ) -> Result<FrameBuffer, String> {
        let resolved = resolve(self, params).ok_or_else(|| format!("Unknown transition kind: {self}"))?;
        Ok(apply2(Some(source), target, &resolved, progress))
    }
}

impl TransitionRenderer for String {
    fn render(
        &self,
        source: &FrameBuffer,
        target: &FrameBuffer,
        progress: f32,
        params: &BTreeMap<String, f32>,
    ) -> Result<FrameBuffer, String> {
        self.as_str().render(source, target, progress, params)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct StandardTransitionRenderer;

impl TransitionRenderer for StandardTransitionRenderer {
    fn render(
        &self,
        source: &FrameBuffer,
        target: &FrameBuffer,
        progress: f32,
        params: &BTreeMap<String, f32>,
    ) -> Result<FrameBuffer, String> {
        let kind = params
            .get("transition_id")
            .map(|_| "custom")
            .unwrap_or(DISSOLVE);
        let resolved = resolve(kind, params).ok_or_else(|| format!("Unknown transition: {kind}"))?;
        Ok(apply2(Some(source), target, &resolved, progress))
    }
}

const SOFTNESS: ParamSpec = ParamSpec {
    id: "softness",
    label: "Suavidade da borda",
    min: 0.0,
    max: 1.0,
    default: 0.1,
    percent: true,
};

/// How strong the perspective of the 3D transitions is.
const PERSPECTIVE: ParamSpec = ParamSpec {
    id: "perspective",
    label: "Perspectiva",
    min: 0.0,
    max: 1.0,
    default: 0.5,
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
                    ParamSpec {
                        id: "h_diffusion",
                        label: "Difusão horizontal",
                        min: 0.0,
                        max: 1.0,
                        default: 0.0,
                        percent: true,
                    },
                    ParamSpec {
                        id: "v_diffusion",
                        label: "Difusão vertical",
                        min: 0.0,
                        max: 1.0,
                        default: 0.0,
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
            EffectSpec {
                id: WARP_FLOW,
                label: "Fluxo de deformação",
                params: &[],
            },
            EffectSpec {
                id: CUBE,
                label: "Cubo 3D",
                params: leak(vec![
                    angle("angle", "Direção de entrada", 360.0),
                    PERSPECTIVE,
                ]),
            },
            EffectSpec {
                id: FLIP,
                label: "Virar 3D",
                params: leak(vec![
                    angle("angle", "Direção de entrada", 360.0),
                    PERSPECTIVE,
                ]),
            },
            EffectSpec {
                id: FLY_IN,
                label: "Voo 3D",
                params: leak(vec![
                    ParamSpec {
                        id: "far_z",
                        label: "Distância inicial Z",
                        min: 0.0,
                        max: 10.0,
                        default: 4.0,
                        percent: false,
                    },
                    ParamSpec {
                        id: "rotation_x",
                        label: "Rotação X",
                        min: -180.0,
                        max: 180.0,
                        default: 25.0,
                        percent: false,
                    },
                    ParamSpec {
                        id: "rotation_y",
                        label: "Rotação Y",
                        min: -180.0,
                        max: 180.0,
                        default: 35.0,
                        percent: false,
                    },
                    ParamSpec {
                        id: "rotation_z",
                        label: "Rotação Z",
                        min: -180.0,
                        max: 180.0,
                        default: 10.0,
                        percent: false,
                    },
                    ParamSpec {
                        id: "specular_light",
                        label: "Luz especular",
                        min: 0.0,
                        max: 1.0,
                        default: 0.5,
                        percent: true,
                    },
                ]),
            },
            EffectSpec {
                id: SHUFFLE,
                label: "Embaralhar 3D",
                params: leak(vec![
                    ParamSpec {
                        id: "extra_spins",
                        label: "Giros extras",
                        min: 0.0,
                        max: 5.0,
                        default: 0.0,
                        percent: false,
                    },
                    ParamSpec {
                        id: "stagger",
                        label: "Atraso lateral",
                        min: 0.0,
                        max: 1.0,
                        default: 0.2,
                        percent: true,
                    },
                    ParamSpec {
                        id: "specular_light",
                        label: "Luz especular",
                        min: 0.0,
                        max: 1.0,
                        default: 0.4,
                        percent: true,
                    },
                ]),
            },
        ]
    })
}

/// The registry entry of `id`.
pub fn spec(id: &str) -> Option<&'static EffectSpec> {
    let canonical = match id {
        CROSS_DISSOLVE => DISSOLVE,
        RADIAL_WIPE => CLOCK_WIPE,
        ZOOM => GROW,
        THREE_D_FLY_IN => FLY_IN,
        THREE_D_SHUFFLE => SHUFFLE,
        other => other,
    };
    transitions().iter().find(|t| t.id == canonical)
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
        /// How far the flash blurs the picture along each axis (see [`flash_blur_half`]).
        h_diffusion: f32,
        v_diffusion: f32,
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
    /// Motion-compensated optical-flow morph between the outgoing clip and the incoming clip.
    WarpFlow,
    /// The clip before and the one that enters are two faces of a cube that turns (see
    /// [`apply_3d`]); `angle` names the edge the new one comes from.
    Cube {
        angle: f32,
        perspective: f32,
    },
    /// A card that turns over: the clip before on one side, the new one on the other.
    Flip {
        angle: f32,
        perspective: f32,
    },
    /// 3D perspective fly-in transition with 3-axis rotation and specular highlight.
    FlyIn {
        far_z: f32,
        rotation_x: f32,
        rotation_y: f32,
        rotation_z: f32,
        specular_light: f32,
    },
    /// 3D perspective shuffle transition with depth exchange and specular highlight.
    Shuffle {
        extra_spins: f32,
        stagger: f32,
        specular_light: f32,
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
            h_diffusion: v("h_diffusion"),
            v_diffusion: v("v_diffusion"),
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
        WARP_FLOW => Resolved::WarpFlow,
        CUBE => Resolved::Cube {
            angle: v("angle"),
            perspective: v("perspective"),
        },
        FLIP => Resolved::Flip {
            angle: v("angle"),
            perspective: v("perspective"),
        },
        FLY_IN | THREE_D_FLY_IN => Resolved::FlyIn {
            far_z: v("far_z"),
            rotation_x: v("rotation_x"),
            rotation_y: v("rotation_y"),
            rotation_z: v("rotation_z"),
            specular_light: v("specular_light"),
        },
        SHUFFLE | THREE_D_SHUFFLE => Resolved::Shuffle {
            extra_spins: v("extra_spins"),
            stagger: v("stagger"),
            specular_light: v("specular_light"),
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

/// Half length in pixels of the horizontal or vertical blur the flash puts on the picture: it grows
/// with the strength of the flash and the diffusion asked for, over a frame `size` pixels long.
pub fn flash_blur_half(diffusion: f32, strength: f32, size: f32) -> f32 {
    diffusion * strength * 0.1 * size
}

/// The picture with the flash's diffusion on it, or `None` when it is too small to show.
fn diffuse(layer: &FrameBuffer, half_h: f32, half_v: f32) -> Option<FrameBuffer> {
    if half_h <= 0.25 && half_v <= 0.25 {
        return None;
    }
    let mut work = crate::vfx::Work::from_frame(layer);
    if half_h > 0.25 {
        work = crate::vfx::blur_along(&work, 1.0, 0.0, half_h);
    }
    if half_v > 0.25 {
        work = crate::vfx::blur_along(&work, 0.0, 1.0, half_v);
    }
    let mut out = layer.clone();
    work.write_to(&mut out);
    Some(out)
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
    matches!(
        kind,
        PUSH | PAGE_ROLL
            | PAGE_PEEL
            | WARP_FLOW
            | CUBE
            | FLIP
            | FLY_IN
            | THREE_D_FLY_IN
            | SHUFFLE
            | THREE_D_SHUFFLE
    )
}

/// `true` for the transitions only the CPU draws: the GPU refuses them and the worker falls back.
pub fn cpu_only(t: &Resolved) -> bool {
    matches!(
        t,
        Resolved::WarpFlow
            | Resolved::Cube { .. }
            | Resolved::Flip { .. }
            | Resolved::FlyIn { .. }
            | Resolved::Shuffle { .. }
    )
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
    if matches!(t, Resolved::WarpFlow) {
        return match outgoing {
            Some(out) => crate::optical_flow::interpolate_frames(out, incoming, progress),
            None => apply(incoming, t, progress),
        };
    }
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
        Resolved::Cube { angle, perspective } => apply_3d(
            outgoing,
            incoming,
            true,
            squeeze_edge(angle),
            perspective,
            progress,
        ),
        Resolved::Flip { angle, perspective } => apply_3d(
            outgoing,
            incoming,
            false,
            squeeze_edge(angle),
            perspective,
            progress,
        ),
        Resolved::FlyIn {
            far_z,
            rotation_x,
            rotation_y,
            rotation_z,
            specular_light,
        } => apply_3d_fly_in(
            outgoing,
            incoming,
            far_z,
            rotation_x,
            rotation_y,
            rotation_z,
            specular_light,
            progress,
        ),
        Resolved::Shuffle {
            extra_spins,
            stagger,
            specular_light,
        } => apply_3d_shuffle(
            outgoing,
            incoming,
            extra_spins,
            stagger,
            specular_light,
            progress,
        ),
        _ => apply(incoming, t, progress),
    }
}

fn shade_lit(px: [f32; 4], n: [f32; 3], q: [f32; 3], specular: f32, d_cam: f32) -> [f32; 4] {
    let lx = 0.28f32;
    let ly = -0.35f32;
    let lz = 0.89f32;
    let dot = (n[0] * lx + n[1] * ly + n[2] * lz).abs();
    let diff = 0.75 + 0.25 * dot;

    let vx = -q[0];
    let vy = -q[1];
    let vz = d_cam - q[2];
    let v_len = (vx * vx + vy * vy + vz * vz).sqrt().max(1e-6);
    let vx = vx / v_len;
    let vy = vy / v_len;
    let vz = vz / v_len;

    let hx = lx + vx;
    let hy = ly + vy;
    let hz = lz + vz;
    let h_len = (hx * hx + hy * hy + hz * hz).sqrt().max(1e-6);
    let s_dot = (n[0] * (hx / h_len) + n[1] * (hy / h_len) + n[2] * (hz / h_len)).abs();
    let spec = specular.clamp(0.0, 1.0) * s_dot.powi(12);

    let a = px[3];
    let ka = a / 255.0;
    [
        (px[0] * diff + spec * 255.0 * ka).min(a),
        (px[1] * diff + spec * 255.0 * ka).min(a),
        (px[2] * diff + spec * 255.0 * ka).min(a),
        a,
    ]
}

fn ray_intersect_plane(
    sx: f32,
    sy: f32,
    d_cam: f32,
    cx: f32,
    cy: f32,
    cz: f32,
    rot: &[[f32; 3]; 3],
) -> Option<(f32, f32, f32, [f32; 3], [f32; 3])> {
    let nx = rot[0][2];
    let ny = rot[1][2];
    let nz = rot[2][2];

    let denom = nx * sx + ny * sy - nz * d_cam;
    if denom.abs() < 1e-6 {
        return None;
    }

    let t = (nx * cx + ny * cy + nz * (cz - d_cam)) / denom;
    if t <= 0.001 || t >= 2.5 {
        return None;
    }

    let qx = t * sx;
    let qy = t * sy;
    let qz = d_cam * (1.0 - t);

    let dx = qx - cx;
    let dy = qy - cy;
    let dz = qz - cz;

    let u = rot[0][0] * dx + rot[1][0] * dy + rot[2][0] * dz;
    let v = rot[0][1] * dx + rot[1][1] * dy + rot[2][1] * dz;

    if (-1.0..=1.0).contains(&u) && (-1.0..=1.0).contains(&v) {
        Some((qz, u, v, [nx, ny, nz], [qx, qy, qz]))
    } else {
        None
    }
}

fn apply_3d_fly_in(
    outgoing: Option<&FrameBuffer>,
    incoming: &FrameBuffer,
    far_z: f32,
    rotation_x: f32,
    rotation_y: f32,
    rotation_z: f32,
    specular_light: f32,
    progress: f32,
) -> FrameBuffer {
    let progress = if progress.is_finite() {
        progress.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let (width, height) = (incoming.width, incoming.height);
    if width == 0 || height == 0 {
        return incoming.clone();
    }
    if progress >= 1.0 {
        return incoming.clone();
    }
    if progress <= 0.0 {
        return outgoing.map_or_else(|| FrameBuffer::solid(width, height, [0, 0, 0, 0]), |o| o.clone());
    }

    let d_cam = 5.0;
    let (w, h) = (width as f32, height as f32);

    let t_inc = 1.0 - progress;
    let far = far_z.clamp(0.0, 10.0);
    let inc_z = -far * t_inc;
    let inc_transform = ThreeDTransform {
        rotation_x_deg: rotation_x * t_inc,
        rotation_y_deg: rotation_y * t_inc,
        rotation_z_deg: rotation_z * t_inc,
        depth_z: inc_z,
        specular_light: specular_light * (1.0 - (2.0 * progress - 1.0).abs()),
    };
    let rot_inc = inc_transform.rotation_matrix();

    let out_transform = ThreeDTransform {
        rotation_x_deg: -progress * 15.0,
        rotation_y_deg: -progress * 25.0,
        rotation_z_deg: -progress * 5.0,
        depth_z: -progress * 2.0,
        specular_light: specular_light * 0.3 * (1.0 - progress),
    };
    let rot_out = out_transform.rotation_matrix();

    render(width, height, |x, y| {
        let sx = (x as f32 + 0.5) / w * 2.0 - 1.0;
        let sy = (y as f32 + 0.5) / h * 2.0 - 1.0;

        let hit_inc = ray_intersect_plane(sx, sy, d_cam, 0.0, 0.0, inc_z, &rot_inc).map(
            |(qz, u, v, n, q)| {
                let tx = (u + 1.0) * 0.5 * w;
                let ty = (v + 1.0) * 0.5 * h;
                let px = sample(incoming, tx, ty);
                (qz, shade_lit(px, n, q, inc_transform.specular_light, d_cam))
            },
        );

        let hit_out = outgoing.and_then(|out_buf| {
            ray_intersect_plane(sx, sy, d_cam, 0.0, 0.0, out_transform.depth_z, &rot_out).map(
                |(qz, u, v, n, q)| {
                    let tx = (u + 1.0) * 0.5 * w;
                    let ty = (v + 1.0) * 0.5 * h;
                    let px = sample(out_buf, tx, ty);
                    (qz, shade_lit(px, n, q, out_transform.specular_light, d_cam))
                },
            )
        });

        match (hit_inc, hit_out) {
            (Some((qz_inc, px_inc)), Some((qz_out, px_out))) => {
                if qz_inc >= qz_out {
                    over(px_inc, px_out)
                } else {
                    over(px_out, px_inc)
                }
            }
            (Some((_, px_inc)), None) => px_inc,
            (None, Some((_, px_out))) => px_out,
            (None, None) => [0.0; 4],
        }
    })
}

fn apply_3d_shuffle(
    outgoing: Option<&FrameBuffer>,
    incoming: &FrameBuffer,
    extra_spins: f32,
    stagger: f32,
    specular_light: f32,
    progress: f32,
) -> FrameBuffer {
    let progress = if progress.is_finite() {
        progress.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let (width, height) = (incoming.width, incoming.height);
    if width == 0 || height == 0 {
        return incoming.clone();
    }
    if progress >= 1.0 {
        return incoming.clone();
    }
    if progress <= 0.0 {
        return outgoing.map_or_else(|| FrameBuffer::solid(width, height, [0, 0, 0, 0]), |o| o.clone());
    }

    let d_cam = 5.0;
    let (w, h) = (width as f32, height as f32);

    let spin = extra_spins * 360.0 * progress;
    let out_cx = progress * (1.2 + stagger);
    let out_cz = -progress * 2.5;
    let out_transform = ThreeDTransform {
        rotation_x_deg: progress * 10.0,
        rotation_y_deg: -progress * 35.0 + spin,
        rotation_z_deg: progress * 5.0,
        depth_z: out_cz,
        specular_light: specular_light * (1.0 - progress),
    };
    let rot_out = out_transform.rotation_matrix();

    let t_inc = 1.0 - progress;
    let inc_cx = -t_inc * (1.2 + stagger);
    let inc_cz = -t_inc * 2.5;
    let inc_transform = ThreeDTransform {
        rotation_x_deg: -t_inc * 10.0,
        rotation_y_deg: t_inc * 35.0 - spin,
        rotation_z_deg: -t_inc * 5.0,
        depth_z: inc_cz,
        specular_light: specular_light * progress,
    };
    let rot_inc = inc_transform.rotation_matrix();

    render(width, height, |x, y| {
        let sx = (x as f32 + 0.5) / w * 2.0 - 1.0;
        let sy = (y as f32 + 0.5) / h * 2.0 - 1.0;

        let hit_inc = ray_intersect_plane(sx, sy, d_cam, inc_cx, 0.0, inc_cz, &rot_inc).map(
            |(qz, u, v, n, q)| {
                let tx = (u + 1.0) * 0.5 * w;
                let ty = (v + 1.0) * 0.5 * h;
                let px = sample(incoming, tx, ty);
                (qz, shade_lit(px, n, q, inc_transform.specular_light, d_cam))
            },
        );

        let hit_out = outgoing.and_then(|out_buf| {
            ray_intersect_plane(sx, sy, d_cam, out_cx, 0.0, out_cz, &rot_out).map(
                |(qz, u, v, n, q)| {
                    let tx = (u + 1.0) * 0.5 * w;
                    let ty = (v + 1.0) * 0.5 * h;
                    let px = sample(out_buf, tx, ty);
                    (qz, shade_lit(px, n, q, out_transform.specular_light, d_cam))
                },
            )
        });

        match (hit_inc, hit_out) {
            (Some((qz_inc, px_inc)), Some((qz_out, px_out))) => {
                if qz_inc >= qz_out {
                    over(px_inc, px_out)
                } else {
                    over(px_out, px_inc)
                }
            }
            (Some((_, px_inc)), None) => px_inc,
            (None, Some((_, px_out))) => px_out,
            (None, None) => [0.0; 4],
        }
    })
}

/// The 3D transitions. `cube`: the clip before is the front face of a cube and the new one the face
/// on the side it enters from, and the cube turns a quarter. Otherwise a card turns half a turn,
/// the clip before on its front and the new one on its back. The scene is seen from a camera at a
/// distance from the middle (a strong perspective is a near camera); outside the faces the layer is
/// transparent, so what is below shows.
///
/// The picture is found by going from each output pixel back to the face: a face is a line
/// `(X, Z) = (c·s + d, a·s + b)` in the plane of the turn, and the line of sight of the pixel,
/// `sx·(camera − Z) = k·X`, is solved for the position `s` along the face.
fn apply_3d(
    outgoing: Option<&FrameBuffer>,
    incoming: &FrameBuffer,
    cube: bool,
    edge: u32,
    perspective: f32,
    progress: f32,
) -> FrameBuffer {
    let (width, height) = (incoming.width, incoming.height);
    let (w, h) = (width as f32, height as f32);
    let camera = 8.0 - 6.0 * perspective.clamp(0.0, 1.0);
    // A face at the front fills the frame
    let k = camera - 1.0;
    let turn = if cube {
        progress * std::f32::consts::FRAC_PI_2
    } else {
        progress * std::f32::consts::PI
    };
    let (sin, cos) = turn.sin_cos();
    // Each face as `(a, b, c, d, is_the_clip_before)`
    let faces: Vec<(f32, f32, f32, f32, bool)> = if cube {
        vec![
            // (s, 1): X = s cos - sin, Z = s sin + cos
            (sin, cos, cos, -sin, true),
            // (1, u): X = cos - u sin, Z = sin + u cos
            (cos, sin, -sin, cos, false),
        ]
    } else {
        // (s, 0) turned by the angle: X = s cos, Z = s sin
        vec![(sin, 0.0, cos, 0.0, turn < std::f32::consts::FRAC_PI_2)]
    };
    // The coordinates along the turn and across it, `-1..=1`, from a pixel and back
    let to_scene = |px: f32, py: f32| {
        let (xn, yn) = (px / w * 2.0 - 1.0, py / h * 2.0 - 1.0);
        match edge {
            0 => (-xn, yn),
            1 => (-yn, xn),
            2 => (xn, yn),
            _ => (yn, xn),
        }
    };
    let to_pixel = |an: f32, cn: f32| {
        let (xn, yn) = match edge {
            0 => (-an, cn),
            1 => (cn, -an),
            2 => (an, cn),
            _ => (cn, an),
        };
        ((xn + 1.0) * 0.5 * w, (yn + 1.0) * 0.5 * h)
    };
    render(width, height, |x, y| {
        let (sx, sy) = to_scene(x as f32 + 0.5, y as f32 + 0.5);
        let mut best: Option<(f32, [f32; 4])> = None;
        for (index, &(a, b, c, d, is_outgoing)) in faces.iter().enumerate() {
            let denom = k * c + sx * a;
            if denom.abs() < 1e-6 {
                continue;
            }
            let s = (sx * (camera - b) - k * d) / denom;
            if !(-1.0..=1.0).contains(&s) {
                continue;
            }
            let z = a * s + b;
            let depth = camera - z;
            if depth <= 0.0 {
                continue;
            }
            let t = sy * depth / k;
            if !(-1.0..=1.0).contains(&t) {
                continue;
            }
            // Where the point is on the picture of the face, along the turn: the face that turns
            // away is seen from behind, so its picture runs the other way
            let along_tex = if is_outgoing { s } else { -s };
            let (tx, ty) = to_pixel(along_tex, t);
            let pixel = if is_outgoing {
                outgoing.map_or([0.0; 4], |o| sample(o, tx, ty))
            } else {
                sample(incoming, tx, ty)
            };
            // The faces that lean away from the camera are darker
            let lean = if cube {
                if index == 0 {
                    cos
                } else {
                    sin
                }
            } else {
                cos.abs()
            };
            let pixel = shade(pixel, 0.55 + 0.45 * lean.clamp(0.0, 1.0));
            if best.is_none_or(|(front, _)| z > front) {
                best = Some((z, pixel));
            }
        }
        best.map_or([0.0; 4], |(_, p)| p)
    })
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
        Resolved::Dissolve | Resolved::WarpFlow => progress,
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
        | Resolved::PagePeel { .. }
        | Resolved::Cube { .. }
        | Resolved::Flip { .. }
        | Resolved::FlyIn { .. }
        | Resolved::Shuffle { .. } => 1.0,
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
        Resolved::Push { .. }
            | Resolved::PageRoll { .. }
            | Resolved::PagePeel { .. }
            | Resolved::Cube { .. }
            | Resolved::Flip { .. }
            | Resolved::FlyIn { .. }
            | Resolved::Shuffle { .. }
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
        Resolved::Flash {
            color,
            h_diffusion,
            v_diffusion,
        } => {
            let f = flash_strength(progress);
            let soft = diffuse(
                layer,
                flash_blur_half(h_diffusion, f, w),
                flash_blur_half(v_diffusion, f, h),
            );
            let layer = soft.as_ref().unwrap_or(layer);
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
    fn the_flash_diffusion_blurs_the_picture_under_the_light() {
        // A chessboard has edges in both directions
        let mut layer = FrameBuffer::solid(80, 40, [0, 0, 0, 255]);
        for y in 0..40usize {
            for x in 0..80usize {
                if (x / 10 + y / 10) % 2 == 0 {
                    let i = (y * 80 + x) * 4;
                    layer.rgba[i..i + 3].copy_from_slice(&[255, 255, 255]);
                }
            }
        }
        let flash = |h: f32, v: f32| {
            resolve(
                FLASH,
                &BTreeMap::from([("h_diffusion".to_owned(), h), ("v_diffusion".to_owned(), v)]),
            )
            .unwrap()
        };
        let sharp = apply(&layer, &flash(0.0, 0.0), 0.25);
        let soft = apply(&layer, &flash(1.0, 0.0), 0.25);
        let softer = apply(&layer, &flash(1.0, 1.0), 0.25);
        assert_ne!(sharp, soft);
        assert_ne!(soft, softer);
        // No diffusion at the ends, where there is no flash
        assert_eq!(apply(&layer, &flash(1.0, 1.0), 1.0), layer);
        assert_eq!(flash_blur_half(0.0, 1.0, 100.0), 0.0);
        assert!((flash_blur_half(1.0, 0.5, 100.0) - 5.0).abs() < 1e-6);
    }

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
        assert_eq!(transitions().len(), 21);
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

    #[test]
    fn warp_flow_morphs_between_outgoing_and_incoming_with_optical_flow() {
        assert!(needs_outgoing(WARP_FLOW));
        assert_eq!(spec(WARP_FLOW).unwrap().params.len(), 0);
        let warp = resolve(WARP_FLOW, &BTreeMap::new()).unwrap();
        assert_eq!(warp, Resolved::WarpFlow);

        let paint_disk = |cx: i32, cy: i32| {
            let (w, h) = (64usize, 48usize);
            let mut f = FrameBuffer::solid(w as u32, h as u32, [20, 24, 30, 255]);
            for y in 0..h {
                for x in 0..w {
                    let dx = x as i32 - cx;
                    let dy = y as i32 - cy;
                    if dx * dx + dy * dy <= 64 {
                        let i = (y * w + x) * 4;
                        f.rgba[i..i + 4].copy_from_slice(&[240, 220, 180, 255]);
                    }
                }
            }
            f
        };
        let out = paint_disk(20, 24);
        let inc = paint_disk(28, 24);

        assert_eq!(apply2(Some(&out), &inc, &warp, 0.0), out);
        assert_eq!(apply2(Some(&out), &inc, &warp, 1.0), inc);

        // At midpoint, optical flow warps the disk toward x = 24 instead of leaving two half-dim ghosts
        let mid = apply2(Some(&out), &inc, &warp, 0.5);
        assert!(
            mid.pixel(24, 24)[0] > 200,
            "midpoint should morph the bright subject to the intermediate position: {:?}",
            mid.pixel(24, 24)
        );

        // Without an outgoing clip, WarpFlow falls back cleanly to a dissolve fade
        let solo = apply2(None, &inc, &warp, 0.5);
        assert!((i32::from(solo.pixel(28, 24)[3]) - 128).abs() <= 1);
    }

    #[test]
    fn the_cube_and_the_card_turn_from_the_clip_before_to_the_new_one() {
        let (w, h) = (64u32, 48u32);
        let red = FrameBuffer::solid(w, h, [255, 0, 0, 255]);
        let blue = FrameBuffer::solid(w, h, [0, 0, 255, 255]);
        let default = BTreeMap::new();
        for kind in [CUBE, FLIP] {
            assert!(needs_outgoing(kind));
            let t = resolve(kind, &default).unwrap();
            assert!(cpu_only(&t));
            // Just started: the clip before fills the middle (a little darker with the turn)
            let early = apply2(Some(&red), &blue, &t, 0.02);
            let mid = early.pixel(w / 2, h / 2);
            assert!(mid[0] > 200 && mid[2] < 40, "{kind}: {mid:?}");
            // Almost done: the new clip does
            let late = apply2(Some(&red), &blue, &t, 0.98);
            let mid = late.pixel(w / 2, h / 2);
            assert!(mid[2] > 200 && mid[0] < 40, "{kind}: {mid:?}");
            // The finished transition is the new clip untouched
            assert_eq!(apply2(Some(&red), &blue, &t, 1.0), blue);
        }
        // Half way the cube is two faces, and the corners of the frame are empty. The default
        // comes in from the left, as a slide does
        let cube = resolve(CUBE, &default).unwrap();
        let half = apply2(Some(&red), &blue, &cube, 0.5);
        assert_eq!(half.pixel(0, 0)[3], 0, "the corner is outside the cube");
        let (left, right) = (half.pixel(w / 4, h / 2), half.pixel(3 * w / 4, h / 2));
        assert!(left[2] > left[0], "the new clip is on the left: {left:?}");
        assert!(
            right[0] > right[2],
            "the clip before is on the right: {right:?}"
        );
        // Entering from the right turns the other way round
        let from_right = resolve(CUBE, &BTreeMap::from([("angle".to_string(), 180.0)])).unwrap();
        let half = apply2(Some(&red), &blue, &from_right, 0.5);
        let (left, right) = (half.pixel(w / 4, h / 2), half.pixel(3 * w / 4, h / 2));
        assert!(
            left[0] > left[2] && right[2] > right[0],
            "{left:?} {right:?}"
        );
        // With no clip before, only the new face shows
        let alone = apply2(None, &blue, &cube, 0.5);
        assert_eq!(alone.pixel(3 * w / 4, h / 2)[3], 0);
        assert_eq!(alone.pixel(w / 4, h / 2)[3], 255);
    }

    #[test]
    fn three_d_transform_orthogonality_and_lighting() {
        let t = ThreeDTransform::new()
            .with_rotations(30.0, 45.0, 60.0)
            .with_depth(3.5)
            .with_specular(0.8);
        assert_eq!(t.depth_z, 3.5);
        assert_eq!(t.specular_light, 0.8);

        let r = t.rotation_matrix();
        // Check R * R^T = I
        for i in 0..3 {
            for j in 0..3 {
                let mut dot = 0.0f32;
                for k in 0..3 {
                    dot += r[i][k] * r[j][k];
                }
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!((dot - expected).abs() < 1e-4, "orthogonality at {i},{j}: {dot}");
            }
        }
    }

    #[test]
    fn three_d_fly_in_and_shuffle_render_cpu_fallback() {
        let (w, h) = (48u32, 32u32);
        let red = FrameBuffer::solid(w, h, [255, 0, 0, 255]);
        let blue = FrameBuffer::solid(w, h, [0, 0, 255, 255]);
        let default_params = BTreeMap::new();

        for kind in [FLY_IN, SHUFFLE] {
            assert!(needs_outgoing(kind));
            let t = resolve(kind, &default_params).unwrap();
            assert!(cpu_only(&t));

            // Start matches outgoing frame
            let start = apply2(Some(&red), &blue, &t, 0.0);
            assert_eq!(start, red, "{kind} at 0.0");

            // End matches incoming frame
            let end = apply2(Some(&red), &blue, &t, 1.0);
            assert_eq!(end, blue, "{kind} at 1.0");

            // Midpoint renders composite frame with valid dimensions and non-zero pixels
            let mid = apply2(Some(&red), &blue, &t, 0.5);
            assert_eq!(mid.width, w);
            assert_eq!(mid.height, h);
            assert_eq!(mid.rgba.len(), (w * h * 4) as usize);

            let has_red = mid.rgba.chunks_exact(4).any(|p| p[0] > 100);
            let has_blue = mid.rgba.chunks_exact(4).any(|p| p[2] > 100);
            assert!(has_red || has_blue, "{kind} has visible content");

            // Without outgoing frame, incoming frame is rendered cleanly without panic
            let solo = apply2(None, &blue, &t, 0.5);
            assert_eq!(solo.width, w);
            assert_eq!(solo.height, h);
        }
    }

    #[test]
    fn transition_progress_envelope_curves_and_clamping() {
        let env = TransitionProgressEnvelope::new(2.0);
        assert!(env.is_active());

        // Default linear ramp: 0.0 at t=0, 0.5 at t=1.0, 1.0 at t=2.0
        assert_eq!(env.progress_at(0.0), 0.0);
        assert!((env.progress_at(1.0) - 0.5).abs() < 1e-4);
        assert_eq!(env.progress_at(2.0), 1.0);

        // Clamping before start and past end
        assert_eq!(env.progress_at(-10.0), 0.0);
        assert_eq!(env.progress_at(10.0), 1.0);
        assert_eq!(env.progress_at(f64::NAN), 1.0);

        // Multi-point shaping with Fast / Slow / Smooth
        let mut shaped = TransitionProgressEnvelope::new(1.0);
        shaped.add_point(0.5, 0.25, TransitionEnvelopeShape::Smooth);
        let mid = shaped.progress_at(0.5);
        assert!((mid - 0.25).abs() < 1e-4);

        // Disabled envelope returns 1.0
        shaped.enabled = false;
        assert_eq!(shaped.progress_at(0.5), 1.0);
    }

    #[test]
    fn transition_renderer_trait_and_presets() {
        let (w, h) = (32u32, 24u32);
        let src = FrameBuffer::solid(w, h, [200, 50, 50, 255]);
        let tgt = FrameBuffer::solid(w, h, [50, 50, 200, 255]);

        // Preset lookup
        let preset_cross = get_preset("cross_dissolve").expect("cross_dissolve preset");
        assert_eq!(preset_cross.category, TransitionCategory::Dissolve);

        // render via TransitionRenderer trait
        let params = BTreeMap::new();
        let rendered = preset_cross
            .render(&src, &tgt, 0.5, &params)
            .expect("render succeeds");
        assert_eq!(rendered.width, w);
        assert_eq!(rendered.height, h);

        // render_thumbnail generates at progress 0.5
        let thumb = preset_cross.render_thumbnail(&src, &tgt, &params);
        assert_eq!(thumb, rendered);

        // All 12 standard presets exist
        let all_12 = [
            "cross_dissolve",
            "linear_wipe",
            "radial_wipe",
            "iris",
            "push",
            "slide",
            "zoom",
            "3d_fly_in",
            "page_peel",
            "page_roll",
            "cube",
            "shuffle",
        ];
        for id in all_12 {
            let p = get_preset(id).unwrap_or_else(|| panic!("missing preset {id}"));
            assert!(!p.name_key.is_empty());
            let thumb = p.render_thumbnail(&src, &tgt, &BTreeMap::new());
            assert_eq!(thumb.width, w);
        }
    }
}
