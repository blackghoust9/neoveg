//! Video effect chains: the registry of effects, their parameters and the CPU reference
//! implementation.
//!
//! The engine stores a chain as registry ids plus the parameters the user changed
//! (`neoveg_engine::TrackEffect`). The worker receives [`VideoEffect`]s (bypassed ones already left
//! out), clamps them through [`VideoEffect::resolve`] and runs the chain, on the GPU when an adapter
//! exists ([`crate::vfx_gpu`]) and with the functions of this module otherwise. The CPU version is
//! the reference: the GPU path has to match it (a test compares them).
//!
//! **Resolution independence.** Frames reach the chain at the source size scaled by the preview
//! quality, and the track chain runs at the canvas size, so no parameter is in pixels: spatial
//! values are fractions of the frame height. The same chain looks the same in Draft, in Best and in
//! the render.
//!
//! **Alpha.** Neighbourhood effects work on premultiplied colour, so a keyed or faded edge does not
//! pick up dark or green fringes.
//!
//! The colour tools (colour balance, levels, gamma, posterize, extract, tint) use the formulas of
//! the FilmCraft effect stage (`render/src/gpufx.rs`, MIT OR Apache-2.0, ArtCraft Team and the
//! FilmCraft contributors), rewritten for the premultiplied encoded working buffer of this module
//! (see `third_party/filmcraft/README.md`).
//!
//! Effects and their parameters follow the reference manuals (Gaussian Blur: horizontal and vertical
//! range; Sharpen: none; Unsharp Mask: ratio; Vignette: outer radius, inner radius, aspect and
//! centre). Decisions of the project that the manuals do not state are in `docs/research/ui_audit.md`.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use crate::frame::FrameBuffer;

/// Longest chain the worker accepts (the same cap as the engine).
pub const MAX_CHAIN: usize = 32;

/// One adjustable number of an effect.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParamSpec {
    pub id: &'static str,
    /// Label in Brazilian Portuguese.
    pub label: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    /// Shown multiplied by 100 with a `%` sign (fractions of the frame height, centre positions).
    pub percent: bool,
}

/// An effect of the registry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectSpec {
    pub id: &'static str,
    /// Name in Brazilian Portuguese.
    pub label: &'static str,
    pub params: &'static [ParamSpec],
}

pub const GAUSSIAN_BLUR: &str = "gaussian_blur";
pub const SHARPEN: &str = "sharpen";
pub const UNSHARP_MASK: &str = "unsharp_mask";
pub const VIGNETTE: &str = "vignette";
pub const BLACK_AND_WHITE: &str = "black_and_white";
pub const INVERT: &str = "invert";
pub const BRIGHTNESS_CONTRAST: &str = "brightness_contrast";
pub const PIXELATE: &str = "pixelate";
pub const MIRROR: &str = "mirror";
pub const SEPIA: &str = "sepia";
pub const SWIRL: &str = "swirl";
pub const RADIAL_BLUR: &str = "radial_blur";
pub const MEDIAN: &str = "median";
pub const HSL_ADJUST: &str = "hsl_adjust";
pub const LINEAR_BLUR: &str = "linear_blur";
pub const COLOR_BALANCE: &str = "color_balance";
pub const LEVELS: &str = "levels";
pub const GAMMA_CORRECTION: &str = "gamma_correction";
pub const POSTERIZE: &str = "posterize";
pub const EXTRACT: &str = "extract";
pub const TINT: &str = "tint";
pub const CHANNEL_MIX: &str = "channel_mix";
pub const ASC_CDL: &str = "asc_cdl";
pub const LEAVE_COLOR: &str = "leave_color";
pub const CHANGE_TO_COLOR: &str = "change_to_color";
pub const COLOR_PASS: &str = "color_pass";
pub const ALPHA_ADJUST: &str = "alpha_adjust";
pub const HORIZONTAL_FLIP: &str = "horizontal_flip";
pub const VERTICAL_FLIP: &str = "vertical_flip";
pub const OFFSET: &str = "offset";
pub const SATURATION_ADJUST: &str = "saturation_adjust";

/// Every effect the video layer can run, in the order of the effect list.
pub const EFFECTS: &[EffectSpec] = &[
    EffectSpec {
        id: GAUSSIAN_BLUR,
        label: "Desfoque gaussiano",
        params: &[
            ParamSpec {
                id: "h_range",
                label: "Alcance horizontal",
                min: 0.0,
                max: 0.05,
                default: 0.005,
                percent: true,
            },
            ParamSpec {
                id: "v_range",
                label: "Alcance vertical",
                min: 0.0,
                max: 0.05,
                default: 0.005,
                percent: true,
            },
        ],
    },
    EffectSpec {
        id: SHARPEN,
        label: "Nitidez",
        params: &[],
    },
    EffectSpec {
        id: UNSHARP_MASK,
        label: "Máscara de nitidez",
        params: &[ParamSpec {
            id: "ratio",
            label: "Proporção",
            min: 0.0,
            max: 3.0,
            default: 1.0,
            percent: false,
        }],
    },
    EffectSpec {
        id: VIGNETTE,
        label: "Vinheta",
        params: &[
            ParamSpec {
                id: "outer_radius",
                label: "Raio externo",
                min: 0.0,
                max: 2.0,
                default: 1.0,
                percent: false,
            },
            ParamSpec {
                id: "inner_radius",
                label: "Raio interno",
                min: 0.0,
                max: 2.0,
                default: 0.5,
                percent: false,
            },
            ParamSpec {
                id: "aspect",
                label: "Aspecto",
                min: 0.25,
                max: 4.0,
                default: 1.0,
                percent: false,
            },
            ParamSpec {
                id: "center_x",
                label: "Centro X",
                min: 0.0,
                max: 1.0,
                default: 0.5,
                percent: true,
            },
            ParamSpec {
                id: "center_y",
                label: "Centro Y",
                min: 0.0,
                max: 1.0,
                default: 0.5,
                percent: true,
            },
        ],
    },
    EffectSpec {
        id: BLACK_AND_WHITE,
        label: "Preto e branco",
        params: &[ParamSpec {
            id: "amount",
            label: "Quantidade",
            min: 0.0,
            max: 1.0,
            default: 1.0,
            percent: true,
        }],
    },
    EffectSpec {
        id: INVERT,
        label: "Inverter",
        params: &[ParamSpec {
            id: "amount",
            label: "Quantidade",
            min: 0.0,
            max: 1.0,
            default: 1.0,
            percent: true,
        }],
    },
    EffectSpec {
        id: BRIGHTNESS_CONTRAST,
        label: "Brilho e contraste",
        params: &[
            ParamSpec {
                id: "brightness",
                label: "Brilho",
                min: -1.0,
                max: 1.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "contrast",
                label: "Contraste",
                min: -1.0,
                max: 1.0,
                default: 0.0,
                percent: true,
            },
        ],
    },
    EffectSpec {
        id: PIXELATE,
        label: "Pixelizar",
        params: &[
            ParamSpec {
                id: "h_size",
                label: "Pixelização horizontal",
                min: 0.0,
                max: 0.25,
                default: 0.02,
                percent: true,
            },
            ParamSpec {
                id: "v_size",
                label: "Pixelização vertical",
                min: 0.0,
                max: 0.25,
                default: 0.02,
                percent: true,
            },
        ],
    },
    EffectSpec {
        id: MIRROR,
        label: "Espelho",
        params: &[
            ParamSpec {
                id: "angle",
                label: "Ângulo",
                min: 0.0,
                max: 360.0,
                default: 0.0,
                percent: false,
            },
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
        ],
    },
    EffectSpec {
        id: SEPIA,
        label: "Sépia",
        params: &[
            ParamSpec {
                id: "red",
                label: "Vermelho do filtro",
                min: 0.0,
                max: 1.0,
                default: 0.70,
                percent: true,
            },
            ParamSpec {
                id: "green",
                label: "Verde do filtro",
                min: 0.0,
                max: 1.0,
                default: 0.50,
                percent: true,
            },
            ParamSpec {
                id: "blue",
                label: "Azul do filtro",
                min: 0.0,
                max: 1.0,
                default: 0.30,
                percent: true,
            },
            ParamSpec {
                id: "strength",
                label: "Intensidade",
                min: 0.0,
                max: 1.0,
                default: 1.0,
                percent: true,
            },
        ],
    },
    EffectSpec {
        id: SWIRL,
        label: "Redemoinho",
        params: &[
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
                id: "ratio",
                label: "Razão",
                min: -3.0,
                max: 3.0,
                default: 0.5,
                percent: false,
            },
            ParamSpec {
                id: "h_size",
                label: "Largura",
                min: 0.01,
                max: 2.0,
                default: 0.5,
                percent: true,
            },
            ParamSpec {
                id: "v_size",
                label: "Altura",
                min: 0.01,
                max: 2.0,
                default: 0.5,
                percent: true,
            },
        ],
    },
    EffectSpec {
        id: RADIAL_BLUR,
        label: "Desfoque radial",
        params: &[
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
                id: "level",
                label: "Nível",
                min: 0.0,
                max: 1.0,
                default: 0.1,
                percent: true,
            },
        ],
    },
    EffectSpec {
        id: MEDIAN,
        label: "Mediana",
        params: &[
            ParamSpec {
                id: "h_range",
                label: "Alcance horizontal",
                min: 0.0,
                max: 0.004,
                default: 0.002,
                percent: true,
            },
            ParamSpec {
                id: "v_range",
                label: "Alcance vertical",
                min: 0.0,
                max: 0.004,
                default: 0.002,
                percent: true,
            },
        ],
    },
    EffectSpec {
        id: HSL_ADJUST,
        label: "Ajuste HSL",
        params: &[
            ParamSpec {
                id: "add_to_hue",
                label: "Somar ao matiz",
                min: 0.0,
                max: 1.0,
                default: 0.0,
                percent: false,
            },
            ParamSpec {
                id: "saturation",
                label: "Saturação",
                min: 0.0,
                max: 2.0,
                default: 1.0,
                percent: true,
            },
            ParamSpec {
                id: "luminosity",
                label: "Luminosidade",
                min: 0.0,
                max: 2.0,
                default: 1.0,
                percent: true,
            },
        ],
    },
    EffectSpec {
        id: LINEAR_BLUR,
        label: "Desfoque linear",
        params: &[
            ParamSpec {
                id: "angle",
                label: "Ângulo",
                min: 0.0,
                max: 180.0,
                default: 0.0,
                percent: false,
            },
            ParamSpec {
                id: "ratio",
                label: "Razão",
                min: 0.0,
                max: 1.0,
                default: 0.1,
                percent: true,
            },
        ],
    },
    EffectSpec {
        id: COLOR_BALANCE,
        label: "Equilíbrio de cor",
        params: &[
            ParamSpec {
                id: "shadow_r",
                label: "Sombras, vermelho",
                min: -1.0,
                max: 1.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "shadow_g",
                label: "Sombras, verde",
                min: -1.0,
                max: 1.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "shadow_b",
                label: "Sombras, azul",
                min: -1.0,
                max: 1.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "mid_r",
                label: "Meios-tons, vermelho",
                min: -1.0,
                max: 1.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "mid_g",
                label: "Meios-tons, verde",
                min: -1.0,
                max: 1.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "mid_b",
                label: "Meios-tons, azul",
                min: -1.0,
                max: 1.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "hi_r",
                label: "Realces, vermelho",
                min: -1.0,
                max: 1.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "hi_g",
                label: "Realces, verde",
                min: -1.0,
                max: 1.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "hi_b",
                label: "Realces, azul",
                min: -1.0,
                max: 1.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "preserve",
                label: "Preservar a luminosidade (0 ou 1)",
                min: 0.0,
                max: 1.0,
                default: 0.0,
                percent: false,
            },
        ],
    },
    EffectSpec {
        id: LEVELS,
        label: "Níveis",
        params: &[
            ParamSpec {
                id: "in_black",
                label: "Preto de entrada",
                min: 0.0,
                max: 255.0,
                default: 0.0,
                percent: false,
            },
            ParamSpec {
                id: "in_white",
                label: "Branco de entrada",
                min: 0.0,
                max: 255.0,
                default: 255.0,
                percent: false,
            },
            ParamSpec {
                id: "gamma",
                label: "Gama",
                min: 10.0,
                max: 300.0,
                default: 100.0,
                percent: false,
            },
            ParamSpec {
                id: "out_black",
                label: "Preto de saída",
                min: 0.0,
                max: 255.0,
                default: 0.0,
                percent: false,
            },
            ParamSpec {
                id: "out_white",
                label: "Branco de saída",
                min: 0.0,
                max: 255.0,
                default: 255.0,
                percent: false,
            },
        ],
    },
    EffectSpec {
        id: GAMMA_CORRECTION,
        label: "Correção de gama",
        params: &[ParamSpec {
            id: "gamma",
            label: "Gama",
            min: 1.0,
            max: 28.0,
            default: 10.0,
            percent: false,
        }],
    },
    EffectSpec {
        id: POSTERIZE,
        label: "Posterizar",
        params: &[ParamSpec {
            id: "levels",
            label: "Níveis",
            min: 2.0,
            max: 256.0,
            default: 8.0,
            percent: false,
        }],
    },
    EffectSpec {
        id: EXTRACT,
        label: "Extrair",
        params: &[
            ParamSpec {
                id: "black",
                label: "Preto",
                min: 0.0,
                max: 255.0,
                default: 0.0,
                percent: false,
            },
            ParamSpec {
                id: "white",
                label: "Branco",
                min: 0.0,
                max: 255.0,
                default: 255.0,
                percent: false,
            },
            ParamSpec {
                id: "softness",
                label: "Suavidade",
                min: 0.0,
                max: 100.0,
                default: 0.0,
                percent: false,
            },
            ParamSpec {
                id: "invert",
                label: "Inverter (0 ou 1)",
                min: 0.0,
                max: 1.0,
                default: 0.0,
                percent: false,
            },
        ],
    },
    EffectSpec {
        id: TINT,
        label: "Matiz",
        params: &[
            ParamSpec {
                id: "black_r",
                label: "Preto mapeado, vermelho",
                min: 0.0,
                max: 1.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "black_g",
                label: "Preto mapeado, verde",
                min: 0.0,
                max: 1.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "black_b",
                label: "Preto mapeado, azul",
                min: 0.0,
                max: 1.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "white_r",
                label: "Branco mapeado, vermelho",
                min: 0.0,
                max: 1.0,
                default: 1.0,
                percent: true,
            },
            ParamSpec {
                id: "white_g",
                label: "Branco mapeado, verde",
                min: 0.0,
                max: 1.0,
                default: 1.0,
                percent: true,
            },
            ParamSpec {
                id: "white_b",
                label: "Branco mapeado, azul",
                min: 0.0,
                max: 1.0,
                default: 1.0,
                percent: true,
            },
            ParamSpec {
                id: "amount",
                label: "Quantidade",
                min: 0.0,
                max: 1.0,
                default: 1.0,
                percent: true,
            },
        ],
    },
    EffectSpec {
        id: CHANNEL_MIX,
        label: "Misturador de canais",
        params: &[
            ParamSpec {
                id: "rr",
                label: "Vermelho com vermelho",
                min: -2.0,
                max: 2.0,
                default: 1.0,
                percent: true,
            },
            ParamSpec {
                id: "rg",
                label: "Vermelho com verde",
                min: -2.0,
                max: 2.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "rb",
                label: "Vermelho com azul",
                min: -2.0,
                max: 2.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "rc",
                label: "Vermelho com constante",
                min: -2.0,
                max: 2.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "gr",
                label: "Verde com vermelho",
                min: -2.0,
                max: 2.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "gg",
                label: "Verde com verde",
                min: -2.0,
                max: 2.0,
                default: 1.0,
                percent: true,
            },
            ParamSpec {
                id: "gb",
                label: "Verde com azul",
                min: -2.0,
                max: 2.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "gc",
                label: "Verde com constante",
                min: -2.0,
                max: 2.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "br",
                label: "Azul com vermelho",
                min: -2.0,
                max: 2.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "bg",
                label: "Azul com verde",
                min: -2.0,
                max: 2.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "bb",
                label: "Azul com azul",
                min: -2.0,
                max: 2.0,
                default: 1.0,
                percent: true,
            },
            ParamSpec {
                id: "bc",
                label: "Azul com constante",
                min: -2.0,
                max: 2.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "monochrome",
                label: "Monocromático (0 ou 1)",
                min: 0.0,
                max: 1.0,
                default: 0.0,
                percent: false,
            },
        ],
    },
    EffectSpec {
        id: ASC_CDL,
        label: "ASC CDL",
        params: &[
            ParamSpec {
                id: "r_slope",
                label: "Inclinação, vermelho",
                min: 0.0,
                max: 10.0,
                default: 1.0,
                percent: false,
            },
            ParamSpec {
                id: "g_slope",
                label: "Inclinação, verde",
                min: 0.0,
                max: 10.0,
                default: 1.0,
                percent: false,
            },
            ParamSpec {
                id: "b_slope",
                label: "Inclinação, azul",
                min: 0.0,
                max: 10.0,
                default: 1.0,
                percent: false,
            },
            ParamSpec {
                id: "r_offset",
                label: "Deslocamento, vermelho",
                min: -1.0,
                max: 1.0,
                default: 0.0,
                percent: false,
            },
            ParamSpec {
                id: "g_offset",
                label: "Deslocamento, verde",
                min: -1.0,
                max: 1.0,
                default: 0.0,
                percent: false,
            },
            ParamSpec {
                id: "b_offset",
                label: "Deslocamento, azul",
                min: -1.0,
                max: 1.0,
                default: 0.0,
                percent: false,
            },
            ParamSpec {
                id: "r_power",
                label: "Potência, vermelho",
                min: 0.0,
                max: 10.0,
                default: 1.0,
                percent: false,
            },
            ParamSpec {
                id: "g_power",
                label: "Potência, verde",
                min: 0.0,
                max: 10.0,
                default: 1.0,
                percent: false,
            },
            ParamSpec {
                id: "b_power",
                label: "Potência, azul",
                min: 0.0,
                max: 10.0,
                default: 1.0,
                percent: false,
            },
            ParamSpec {
                id: "saturation",
                label: "Saturação",
                min: 0.0,
                max: 10.0,
                default: 1.0,
                percent: false,
            },
        ],
    },
    EffectSpec {
        id: LEAVE_COLOR,
        label: "Manter cor",
        params: &[
            ParamSpec {
                id: "amount",
                label: "Quantidade a descolorir",
                min: 0.0,
                max: 1.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "key_r",
                label: "Cor a manter, vermelho",
                min: 0.0,
                max: 1.0,
                default: 1.0,
                percent: true,
            },
            ParamSpec {
                id: "key_g",
                label: "Cor a manter, verde",
                min: 0.0,
                max: 1.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "key_b",
                label: "Cor a manter, azul",
                min: 0.0,
                max: 1.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "tolerance",
                label: "Tolerância",
                min: 0.0,
                max: 1.0,
                default: 0.15,
                percent: true,
            },
            ParamSpec {
                id: "softness",
                label: "Suavidade da borda",
                min: 0.0,
                max: 1.0,
                default: 0.0,
                percent: true,
            },
        ],
    },
    EffectSpec {
        id: CHANGE_TO_COLOR,
        label: "Trocar por cor",
        params: &[
            ParamSpec {
                id: "from_r",
                label: "De, vermelho",
                min: 0.0,
                max: 1.0,
                default: 1.0,
                percent: true,
            },
            ParamSpec {
                id: "from_g",
                label: "De, verde",
                min: 0.0,
                max: 1.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "from_b",
                label: "De, azul",
                min: 0.0,
                max: 1.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "to_r",
                label: "Para, vermelho",
                min: 0.0,
                max: 1.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "to_g",
                label: "Para, verde",
                min: 0.0,
                max: 1.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "to_b",
                label: "Para, azul",
                min: 0.0,
                max: 1.0,
                default: 1.0,
                percent: true,
            },
            ParamSpec {
                id: "hue_tol",
                label: "Tolerância de matiz",
                min: 0.0,
                max: 1.0,
                default: 0.05,
                percent: true,
            },
            ParamSpec {
                id: "softness",
                label: "Suavidade",
                min: 0.0,
                max: 1.0,
                default: 0.5,
                percent: true,
            },
        ],
    },
    EffectSpec {
        id: COLOR_PASS,
        label: "Passar cor",
        params: &[
            ParamSpec {
                id: "key_r",
                label: "Cor, vermelho",
                min: 0.0,
                max: 1.0,
                default: 1.0,
                percent: true,
            },
            ParamSpec {
                id: "key_g",
                label: "Cor, verde",
                min: 0.0,
                max: 1.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "key_b",
                label: "Cor, azul",
                min: 0.0,
                max: 1.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "similarity",
                label: "Semelhança",
                min: 0.0,
                max: 1.0,
                default: 0.1,
                percent: true,
            },
            ParamSpec {
                id: "reverse",
                label: "Inverter (0 ou 1)",
                min: 0.0,
                max: 1.0,
                default: 0.0,
                percent: false,
            },
        ],
    },
    EffectSpec {
        id: ALPHA_ADJUST,
        label: "Ajuste de alfa",
        params: &[
            ParamSpec {
                id: "opacity",
                label: "Opacidade",
                min: 0.0,
                max: 1.0,
                default: 1.0,
                percent: true,
            },
            ParamSpec {
                id: "ignore",
                label: "Ignorar o alfa (0 ou 1)",
                min: 0.0,
                max: 1.0,
                default: 0.0,
                percent: false,
            },
            ParamSpec {
                id: "invert",
                label: "Inverter o alfa (0 ou 1)",
                min: 0.0,
                max: 1.0,
                default: 0.0,
                percent: false,
            },
            ParamSpec {
                id: "mask_only",
                label: "Só a máscara (0 ou 1)",
                min: 0.0,
                max: 1.0,
                default: 0.0,
                percent: false,
            },
        ],
    },
    EffectSpec {
        id: HORIZONTAL_FLIP,
        label: "Espelhar na horizontal",
        params: &[],
    },
    EffectSpec {
        id: VERTICAL_FLIP,
        label: "Espelhar na vertical",
        params: &[],
    },
    EffectSpec {
        id: OFFSET,
        label: "Deslocar",
        params: &[
            ParamSpec {
                id: "shift_x",
                label: "Centro para, horizontal",
                min: 0.0,
                max: 1.0,
                default: 0.5,
                percent: true,
            },
            ParamSpec {
                id: "shift_y",
                label: "Centro para, vertical",
                min: 0.0,
                max: 1.0,
                default: 0.5,
                percent: true,
            },
            ParamSpec {
                id: "blend",
                label: "Mistura com o original",
                min: 0.0,
                max: 1.0,
                default: 0.0,
                percent: true,
            },
        ],
    },
    EffectSpec {
        id: SATURATION_ADJUST,
        label: "Ajuste de saturação",
        params: &[
            ParamSpec {
                id: "ratio",
                label: "Razão (dentro da faixa)",
                min: -1.0,
                max: 1.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "center",
                label: "Centro (matiz)",
                min: 0.0,
                max: 1.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "spread",
                label: "Largura (matizes)",
                min: 0.0,
                max: 1.0,
                default: 0.2,
                percent: true,
            },
            ParamSpec {
                id: "low",
                label: "Abaixo da faixa",
                min: -1.0,
                max: 1.0,
                default: 0.0,
                percent: true,
            },
            ParamSpec {
                id: "up",
                label: "Acima da faixa",
                min: -1.0,
                max: 1.0,
                default: 0.0,
                percent: true,
            },
        ],
    },
];

/// The registry entry of `id`.
pub fn spec(id: &str) -> Option<&'static EffectSpec> {
    EFFECTS.iter().find(|e| e.id == id)
}

impl EffectSpec {
    pub fn param(&self, id: &str) -> Option<&'static ParamSpec> {
        self.params.iter().find(|p| p.id == id)
    }
}

/// An effect as the worker receives it: the registry id and the values that differ from the
/// defaults. Unknown ids are skipped when the chain runs, unknown or non-finite values are ignored.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VideoEffect {
    pub kind: String,
    pub params: BTreeMap<String, f32>,
}

impl VideoEffect {
    pub fn new(kind: &str) -> Self {
        Self {
            kind: kind.to_owned(),
            params: BTreeMap::new(),
        }
    }

    pub fn with(mut self, param: &str, value: f32) -> Self {
        self.params.insert(param.to_owned(), value);
        self
    }

    /// The value of `param` clamped to its range, or its default.
    fn value(&self, spec: &EffectSpec, param: &str) -> f32 {
        let Some(p) = spec.param(param) else {
            return 0.0;
        };
        match self.params.get(param) {
            Some(v) if v.is_finite() => v.clamp(p.min, p.max),
            _ => p.default,
        }
    }

    /// The effect with every value clamped, or `None` for an unknown id.
    pub fn resolve(&self) -> Option<Resolved> {
        let spec = spec(&self.kind)?;
        Some(match spec.id {
            GAUSSIAN_BLUR => Resolved::GaussianBlur {
                h_range: self.value(spec, "h_range"),
                v_range: self.value(spec, "v_range"),
            },
            SHARPEN => Resolved::Sharpen,
            UNSHARP_MASK => Resolved::UnsharpMask {
                ratio: self.value(spec, "ratio"),
            },
            VIGNETTE => {
                let inner = self.value(spec, "inner_radius");
                // A hard edge when the radii are equal or reversed, never a division by zero.
                let outer = self.value(spec, "outer_radius").max(inner + 1e-3);
                Resolved::Vignette {
                    outer,
                    inner,
                    aspect: self.value(spec, "aspect"),
                    center: [self.value(spec, "center_x"), self.value(spec, "center_y")],
                }
            }
            BLACK_AND_WHITE => Resolved::BlackAndWhite {
                amount: self.value(spec, "amount"),
            },
            INVERT => Resolved::Invert {
                amount: self.value(spec, "amount"),
            },
            BRIGHTNESS_CONTRAST => Resolved::BrightnessContrast {
                brightness: self.value(spec, "brightness"),
                contrast: self.value(spec, "contrast"),
            },
            PIXELATE => Resolved::Pixelate {
                h_size: self.value(spec, "h_size"),
                v_size: self.value(spec, "v_size"),
            },
            MIRROR => Resolved::Mirror {
                angle: self.value(spec, "angle"),
                center: [self.value(spec, "center_x"), self.value(spec, "center_y")],
            },
            SEPIA => Resolved::Sepia {
                tint: [
                    self.value(spec, "red"),
                    self.value(spec, "green"),
                    self.value(spec, "blue"),
                ],
                strength: self.value(spec, "strength"),
            },
            SWIRL => Resolved::Swirl {
                center: [self.value(spec, "center_x"), self.value(spec, "center_y")],
                ratio: self.value(spec, "ratio"),
                size: [self.value(spec, "h_size"), self.value(spec, "v_size")],
            },
            RADIAL_BLUR => Resolved::RadialBlur {
                center: [self.value(spec, "center_x"), self.value(spec, "center_y")],
                level: self.value(spec, "level"),
            },
            MEDIAN => Resolved::Median {
                h_range: self.value(spec, "h_range"),
                v_range: self.value(spec, "v_range"),
            },
            HSL_ADJUST => Resolved::HslAdjust {
                add_to_hue: self.value(spec, "add_to_hue"),
                saturation: self.value(spec, "saturation"),
                luminosity: self.value(spec, "luminosity"),
            },
            LINEAR_BLUR => Resolved::LinearBlur {
                angle: self.value(spec, "angle"),
                ratio: self.value(spec, "ratio"),
            },
            COLOR_BALANCE => {
                let zone = |name: &str| {
                    [
                        self.value(spec, &format!("{name}_r")) * BALANCE_SCALE,
                        self.value(spec, &format!("{name}_g")) * BALANCE_SCALE,
                        self.value(spec, &format!("{name}_b")) * BALANCE_SCALE,
                    ]
                };
                Resolved::ColorBalance {
                    shadows: zone("shadow"),
                    mids: zone("mid"),
                    highlights: zone("hi"),
                    preserve: self.value(spec, "preserve") >= 0.5,
                }
            }
            LEVELS => {
                let ib = self.value(spec, "in_black") / 255.0;
                Resolved::Levels {
                    ib,
                    iw: (self.value(spec, "in_white") / 255.0).max(ib + 1e-3),
                    ob: self.value(spec, "out_black") / 255.0,
                    ow: self.value(spec, "out_white") / 255.0,
                    g: 100.0 / self.value(spec, "gamma").max(1.0),
                }
            }
            GAMMA_CORRECTION => Resolved::Gamma {
                g: self.value(spec, "gamma") / 10.0,
            },
            POSTERIZE => Resolved::Posterize {
                n: self.value(spec, "levels").round().max(2.0) - 1.0,
            },
            EXTRACT => {
                let softness = self.value(spec, "softness");
                Resolved::Extract {
                    lo: self.value(spec, "black") / 255.0,
                    hi: self.value(spec, "white") / 255.0,
                    soft: if softness <= 0.0 {
                        0.0
                    } else {
                        softness / 100.0 * 0.2 + 1e-4
                    },
                    invert: self.value(spec, "invert") >= 0.5,
                }
            }
            TINT => Resolved::Tint {
                black: [
                    self.value(spec, "black_r"),
                    self.value(spec, "black_g"),
                    self.value(spec, "black_b"),
                ],
                white: [
                    self.value(spec, "white_r"),
                    self.value(spec, "white_g"),
                    self.value(spec, "white_b"),
                ],
                amount: self.value(spec, "amount"),
            },
            CHANNEL_MIX => {
                let g = |id: &str| self.value(spec, id);
                let mut m = [
                    [g("rr"), g("rg"), g("rb"), g("rc")],
                    [g("gr"), g("gg"), g("gb"), g("gc")],
                    [g("br"), g("bg"), g("bb"), g("bc")],
                ];
                if g("monochrome") >= 0.5 {
                    m = [m[0]; 3];
                }
                Resolved::ChannelMix { m }
            }
            ASC_CDL => {
                let g = |id: &str| self.value(spec, id);
                Resolved::AscCdl {
                    slope: [g("r_slope"), g("g_slope"), g("b_slope")],
                    offset: [g("r_offset"), g("g_offset"), g("b_offset")],
                    power: [g("r_power"), g("g_power"), g("b_power")],
                    sat: g("saturation"),
                }
            }
            LEAVE_COLOR => Resolved::LeaveColor {
                amount: self.value(spec, "amount"),
                key_hue: hue_fraction([
                    self.value(spec, "key_r"),
                    self.value(spec, "key_g"),
                    self.value(spec, "key_b"),
                ]),
                tol: self.value(spec, "tolerance"),
                soft: self.value(spec, "softness") + 1e-4,
            },
            CHANGE_TO_COLOR => Resolved::ChangeToColor {
                from_hue: hue_fraction([
                    self.value(spec, "from_r"),
                    self.value(spec, "from_g"),
                    self.value(spec, "from_b"),
                ]),
                to_hue: hue_fraction([
                    self.value(spec, "to_r"),
                    self.value(spec, "to_g"),
                    self.value(spec, "to_b"),
                ]),
                tol: self.value(spec, "hue_tol"),
                soft: self.value(spec, "softness") * 0.3 + 1e-4,
            },
            COLOR_PASS => Resolved::ColorPass {
                key: [
                    self.value(spec, "key_r"),
                    self.value(spec, "key_g"),
                    self.value(spec, "key_b"),
                ],
                sim: self.value(spec, "similarity"),
                reverse: self.value(spec, "reverse") >= 0.5,
            },
            ALPHA_ADJUST => Resolved::AlphaAdjust {
                opacity: self.value(spec, "opacity"),
                ignore: self.value(spec, "ignore") >= 0.5,
                invert: self.value(spec, "invert") >= 0.5,
                mask_only: self.value(spec, "mask_only") >= 0.5,
            },
            HORIZONTAL_FLIP => Resolved::HFlip,
            VERTICAL_FLIP => Resolved::VFlip,
            OFFSET => Resolved::Offset {
                shift: [self.value(spec, "shift_x"), self.value(spec, "shift_y")],
                blend: self.value(spec, "blend"),
            },
            SATURATION_ADJUST => Resolved::SaturationAdjust {
                ratio: self.value(spec, "ratio"),
                center: self.value(spec, "center"),
                spread: self.value(spec, "spread"),
                low: self.value(spec, "low"),
                up: self.value(spec, "up"),
            },
            _ => return None,
        })
    }

    /// Feeds the effect into a hasher (the worker's frame cache key).
    pub fn hash_into(&self, h: &mut impl Hasher) {
        self.kind.hash(h);
        self.params.len().hash(h);
        for (k, v) in &self.params {
            k.hash(h);
            v.to_bits().hash(h);
        }
    }
}

/// An effect with its values clamped, ready to run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Resolved {
    /// Ranges are fractions of the frame height: the number of pixels blended to each side.
    GaussianBlur {
        h_range: f32,
        v_range: f32,
    },
    Sharpen,
    UnsharpMask {
        ratio: f32,
    },
    Vignette {
        outer: f32,
        inner: f32,
        aspect: f32,
        /// `0.0, 0.0` is the lower-left corner of the frame.
        center: [f32; 2],
    },
    /// Mixes the colour with its luma; `1.0` is full grey.
    BlackAndWhite {
        amount: f32,
    },
    /// Mixes the colour with its negative; `1.0` is the full negative.
    Invert {
        amount: f32,
    },
    BrightnessContrast {
        brightness: f32,
        contrast: f32,
    },
    /// Block sizes as fractions of the frame height.
    Pixelate {
        h_size: f32,
        v_size: f32,
    },
    /// Reflects one side of a line over the other (see [`mirror`]). The centre is in fractions of
    /// the frame, `0.0, 0.0` being the lower-left corner.
    Mirror {
        angle: f32,
        center: [f32; 2],
    },
    /// A filter colour on the luma of the picture, `strength` `0..=1` of the way.
    Sepia {
        tint: [f32; 3],
        strength: f32,
    },
    /// See [`swirl`]; the size is in fractions of the frame height.
    Swirl {
        center: [f32; 2],
        ratio: f32,
        size: [f32; 2],
    },
    /// See [`radial_blur`].
    RadialBlur {
        center: [f32; 2],
        level: f32,
    },
    /// Ranges are fractions of the frame height (see [`median_radius`]).
    Median {
        h_range: f32,
        v_range: f32,
    },
    /// See [`hsl_adjust_px`]; `add_to_hue` is a fraction of the colour circle.
    HslAdjust {
        add_to_hue: f32,
        saturation: f32,
        luminosity: f32,
    },
    /// See [`linear_blur`].
    LinearBlur {
        angle: f32,
        ratio: f32,
    },
    /// Colour offsets for the shadows, midtones and highlights (already scaled, see
    /// [`BALANCE_SCALE`]); `preserve` keeps the luminosity.
    ColorBalance {
        shadows: [f32; 3],
        mids: [f32; 3],
        highlights: [f32; 3],
        preserve: bool,
    },
    /// Input black and white, output black and white (`0..=1`) and the gamma exponent.
    Levels {
        ib: f32,
        iw: f32,
        ob: f32,
        ow: f32,
        g: f32,
    },
    /// `v^g` on each channel.
    Gamma {
        g: f32,
    },
    /// `n` is the number of steps between black and white.
    Posterize {
        n: f32,
    },
    /// A matte of the luma between two cuts (see [`extract_px`]).
    Extract {
        lo: f32,
        hi: f32,
        soft: f32,
        invert: bool,
    },
    /// The luma mapped between two colours and mixed with the picture by `amount`.
    Tint {
        black: [f32; 3],
        white: [f32; 3],
        amount: f32,
    },
    /// Rows `[red, green, blue, constant]` for each output channel.
    ChannelMix {
        m: [[f32; 4]; 3],
    },
    /// See [`cdl_px`].
    AscCdl {
        slope: [f32; 3],
        offset: [f32; 3],
        power: [f32; 3],
        sat: f32,
    },
    /// See [`leave_color_px`]; hues and `tol` are fractions of the colour circle.
    LeaveColor {
        amount: f32,
        key_hue: f32,
        tol: f32,
        soft: f32,
    },
    /// See [`change_to_color_px`].
    ChangeToColor {
        from_hue: f32,
        to_hue: f32,
        tol: f32,
        soft: f32,
    },
    /// Colours within `sim` of `key` stay (or, reversed, are the ones that go grey).
    ColorPass {
        key: [f32; 3],
        sim: f32,
        reverse: bool,
    },
    AlphaAdjust {
        opacity: f32,
        ignore: bool,
        invert: bool,
        mask_only: bool,
    },
    HFlip,
    VFlip,
    /// See [`offset_pixels`].
    Offset {
        shift: [f32; 2],
        blend: f32,
    },
    /// See [`saturation_gain`].
    SaturationAdjust {
        ratio: f32,
        center: f32,
        spread: f32,
        low: f32,
        up: f32,
    },
}

impl Resolved {
    /// `true` when the effect leaves the picture unchanged.
    pub fn is_neutral(&self) -> bool {
        match *self {
            Resolved::GaussianBlur { h_range, v_range } => h_range <= 0.0 && v_range <= 0.0,
            Resolved::Sharpen => false,
            Resolved::UnsharpMask { ratio } => ratio <= 0.0,
            Resolved::Vignette { .. } => false,
            Resolved::BlackAndWhite { amount } | Resolved::Invert { amount } => amount <= 0.0,
            Resolved::BrightnessContrast {
                brightness,
                contrast,
            } => brightness == 0.0 && contrast == 0.0,
            // Blocks of a single pixel are the picture itself
            Resolved::Pixelate { h_size, v_size } => h_size <= 0.0 && v_size <= 0.0,
            Resolved::Mirror { .. } => false,
            Resolved::Sepia { strength, .. } => strength <= 0.0,
            Resolved::Swirl { ratio, .. } => ratio == 0.0,
            Resolved::RadialBlur { level, .. } => level <= 0.0,
            Resolved::Median { h_range, v_range } => h_range <= 0.0 && v_range <= 0.0,
            Resolved::HslAdjust {
                add_to_hue,
                saturation,
                luminosity,
            } => hue_degrees(add_to_hue) % 360.0 == 0.0 && saturation == 1.0 && luminosity == 1.0,
            Resolved::LinearBlur { ratio, .. } => ratio <= 0.0,
            Resolved::ColorBalance {
                shadows,
                mids,
                highlights,
                ..
            } => shadows
                .iter()
                .chain(&mids)
                .chain(&highlights)
                .all(|v| *v == 0.0),
            Resolved::Levels { ib, iw, ob, ow, g } => {
                ib == 0.0 && iw == 1.0 && ob == 0.0 && ow == 1.0 && g == 1.0
            }
            Resolved::Gamma { g } => g == 1.0,
            Resolved::Posterize { n } => n >= 255.0,
            Resolved::Extract { .. } => false,
            Resolved::Tint { amount, .. } => amount <= 0.0,
            Resolved::ChannelMix { m } => {
                m == [
                    [1.0, 0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                ]
            }
            Resolved::AscCdl {
                slope,
                offset,
                power,
                sat,
            } => slope == [1.0; 3] && offset == [0.0; 3] && power == [1.0; 3] && sat == 1.0,
            Resolved::LeaveColor { amount, .. } => amount <= 0.0,
            Resolved::ChangeToColor {
                from_hue, to_hue, ..
            } => from_hue == to_hue,
            Resolved::ColorPass { .. } => false,
            Resolved::AlphaAdjust {
                opacity,
                ignore,
                invert,
                mask_only,
            } => opacity == 1.0 && !ignore && !invert && !mask_only,
            Resolved::HFlip | Resolved::VFlip => false,
            Resolved::Offset { shift, blend } => shift == [0.5, 0.5] || blend >= 1.0,
            Resolved::SaturationAdjust { ratio, low, up, .. } => {
                ratio == 0.0 && low == 0.0 && up == 0.0
            }
        }
    }
}

/// The chain resolved, with unknown and neutral effects left out.
pub fn resolve_chain(chain: &[VideoEffect]) -> Vec<Resolved> {
    chain
        .iter()
        .take(MAX_CHAIN)
        .filter_map(VideoEffect::resolve)
        .filter(|r| !r.is_neutral())
        .collect()
}

/// Feeds a whole chain into a hasher.
pub fn hash_chain(chain: &[VideoEffect], h: &mut impl Hasher) {
    chain.len().hash(h);
    for e in chain {
        e.hash_into(h);
    }
}

// ---- shared maths ----------------------------------------------------------------------------------

/// Pixels blended to each side for a range given as a fraction of the frame height.
pub fn range_pixels(range: f32, height: u32) -> f32 {
    range * height as f32
}

/// Longest half width of a blur kernel.
pub const MAX_KERNEL_HALF: usize = 192;

/// Normalised gaussian weights for offsets `-half..=half` (`2 * half + 1` values). `radius_px` is
/// the range in pixels; the standard deviation is half of it. `None` when it is under a quarter of
/// a pixel, which changes nothing.
pub fn gaussian_kernel(radius_px: f32) -> Option<Vec<f32>> {
    if !radius_px.is_finite() || radius_px < 0.5 {
        return None;
    }
    let sigma = radius_px / 2.0;
    let half = ((sigma * 3.0).ceil() as usize).clamp(1, MAX_KERNEL_HALF);
    let mut weights: Vec<f32> = (0..=2 * half)
        .map(|i| {
            let d = i as f32 - half as f32;
            (-(d * d) / (2.0 * sigma * sigma)).exp()
        })
        .collect();
    let sum: f32 = weights.iter().sum();
    for w in &mut weights {
        *w /= sum;
    }
    Some(weights)
}

/// Standard deviation, as a fraction of the frame height, of the blur the unsharp mask subtracts.
pub const UNSHARP_RADIUS: f32 = 0.006;

/// Centre-to-corner distance, in frame-height units, used to normalise the vignette radii.
pub fn vignette_corner(width: u32, height: u32) -> f32 {
    let ar = width as f32 / height.max(1) as f32;
    ((0.5 * ar).powi(2) + 0.25).sqrt()
}

/// The clamped numbers of a vignette.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VignetteShape {
    pub outer: f32,
    pub inner: f32,
    pub aspect: f32,
    /// `0.0, 0.0` is the lower-left corner of the frame.
    pub center: [f32; 2],
}

/// The darkening of the vignette at a pixel (`1.0` keeps it, `0.0` is black).
pub fn vignette_gain(px: u32, py: u32, width: u32, height: u32, shape: &VignetteShape) -> f32 {
    let VignetteShape {
        outer,
        inner,
        aspect,
        center,
    } = *shape;
    let ar = width as f32 / height.max(1) as f32;
    let nx = (px as f32 + 0.5) / width as f32 - center[0];
    // Frame coordinates grow downwards, the manual's centre grows upwards
    let ny = 1.0 - (py as f32 + 0.5) / height as f32 - center[1];
    let r = (((nx * ar) / aspect).powi(2) + ny * ny).sqrt() / vignette_corner(width, height);
    let t = ((r - inner) / (outer - inner)).clamp(0.0, 1.0);
    let smooth = t * t * (3.0 - 2.0 * t);
    1.0 - smooth
}

// ---- CPU reference ---------------------------------------------------------------------------------

/// A frame as premultiplied floats, the working format of the chain.
pub struct Work {
    pub width: usize,
    pub height: usize,
    pub px: Vec<[f32; 4]>,
}

impl Work {
    pub fn from_frame(frame: &FrameBuffer) -> Self {
        let px = frame
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| {
                let a = f32::from(p[3]) / 255.0;
                [
                    f32::from(p[0]) / 255.0 * a,
                    f32::from(p[1]) / 255.0 * a,
                    f32::from(p[2]) / 255.0 * a,
                    a,
                ]
            })
            .collect();
        Self {
            width: frame.width as usize,
            height: frame.height as usize,
            px,
        }
    }

    pub fn write_to(&self, frame: &mut FrameBuffer) {
        for (dst, p) in frame.rgba.as_chunks_mut::<4>().0.iter_mut().zip(&self.px) {
            let a = p[3].clamp(0.0, 1.0);
            // Under half a level of alpha the pixel is empty (the same cut as the transitions)
            if a * 255.0 < 0.5 {
                *dst = [0, 0, 0, 0];
                continue;
            }
            let inv = 1.0 / a;
            for c in 0..3 {
                dst[c] = ((p[c] * inv).clamp(0.0, 1.0) * 255.0).round() as u8;
            }
            dst[3] = (a * 255.0).round() as u8;
        }
    }
}

/// Runs `job` on bands of rows, in parallel when the picture is big enough.
fn par_rows(out: &mut [[f32; 4]], width: usize, job: impl Fn(usize, &mut [[f32; 4]]) + Sync) {
    let rows = out.len() / width.max(1);
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    if rows * width < 64 * 1024 || threads < 2 {
        job(0, out);
        return;
    }
    let band = rows.div_ceil(threads);
    std::thread::scope(|s| {
        for (i, chunk) in out.chunks_mut(band * width).enumerate() {
            let job = &job;
            s.spawn(move || job(i * band, chunk));
        }
    });
}

fn blur_pass(src: &Work, kernel: &[f32], horizontal: bool) -> Work {
    let (w, h) = (src.width, src.height);
    let half = (kernel.len() / 2) as isize;
    let mut out = vec![[0.0f32; 4]; w * h];
    par_rows(&mut out, w, |first_row, band| {
        for (ry, row) in band.chunks_mut(w).enumerate() {
            let y = first_row + ry;
            for (x, dst) in row.iter_mut().enumerate() {
                let mut acc = [0.0f32; 4];
                for (k, wt) in kernel.iter().enumerate() {
                    let d = k as isize - half;
                    let (sx, sy) = if horizontal {
                        ((x as isize + d).clamp(0, w as isize - 1) as usize, y)
                    } else {
                        (x, (y as isize + d).clamp(0, h as isize - 1) as usize)
                    };
                    let p = src.px[sy * w + sx];
                    for c in 0..4 {
                        acc[c] += p[c] * wt;
                    }
                }
                *dst = acc;
            }
        }
    });
    Work {
        width: w,
        height: h,
        px: out,
    }
}

fn gaussian_blur(work: &Work, h_range: f32, v_range: f32) -> Work {
    let mut cur = Work {
        width: work.width,
        height: work.height,
        px: work.px.clone(),
    };
    if let Some(k) = gaussian_kernel(range_pixels(h_range, work.height as u32)) {
        cur = blur_pass(&cur, &k, true);
    }
    if let Some(k) = gaussian_kernel(range_pixels(v_range, work.height as u32)) {
        cur = blur_pass(&cur, &k, false);
    }
    cur
}

/// Keeps colour inside what a premultiplied pixel with this alpha can hold.
fn clamp_premultiplied(p: [f32; 4]) -> [f32; 4] {
    let a = p[3].clamp(0.0, 1.0);
    [
        p[0].clamp(0.0, a),
        p[1].clamp(0.0, a),
        p[2].clamp(0.0, a),
        a,
    ]
}

fn sharpen(work: &Work) -> Work {
    let (w, h) = (work.width, work.height);
    let mut out = vec![[0.0f32; 4]; w * h];
    par_rows(&mut out, w, |first_row, band| {
        for (ry, row) in band.chunks_mut(w).enumerate() {
            let y = first_row + ry;
            for (x, dst) in row.iter_mut().enumerate() {
                let at = |dx: isize, dy: isize| {
                    let sx = (x as isize + dx).clamp(0, w as isize - 1) as usize;
                    let sy = (y as isize + dy).clamp(0, h as isize - 1) as usize;
                    work.px[sy * w + sx]
                };
                let (c, l, r, u, d) = (at(0, 0), at(-1, 0), at(1, 0), at(0, -1), at(0, 1));
                let mut p = [0.0f32; 4];
                for i in 0..4 {
                    p[i] = 5.0 * c[i] - l[i] - r[i] - u[i] - d[i];
                }
                *dst = clamp_premultiplied(p);
            }
        }
    });
    Work {
        width: w,
        height: h,
        px: out,
    }
}

fn unsharp(work: &Work, ratio: f32) -> Work {
    let radius = range_pixels(UNSHARP_RADIUS * 2.0, work.height as u32);
    let Some(k) = gaussian_kernel(radius) else {
        return Work {
            width: work.width,
            height: work.height,
            px: work.px.clone(),
        };
    };
    let blurred = blur_pass(&blur_pass(work, &k, true), &k, false);
    let px = work
        .px
        .iter()
        .zip(&blurred.px)
        .map(|(o, b)| {
            let mut p = [0.0f32; 4];
            for i in 0..4 {
                p[i] = o[i] + ratio * (o[i] - b[i]);
            }
            clamp_premultiplied([p[0], p[1], p[2], o[3]])
        })
        .collect();
    Work {
        width: work.width,
        height: work.height,
        px,
    }
}

fn vignette(work: &mut Work, outer: f32, inner: f32, aspect: f32, center: [f32; 2]) {
    let (w, h) = (work.width, work.height);
    let shape = VignetteShape {
        outer,
        inner,
        aspect,
        center,
    };
    for y in 0..h {
        for x in 0..w {
            let g = vignette_gain(x as u32, y as u32, w as u32, h as u32, &shape);
            let p = &mut work.px[y * w + x];
            p[0] *= g;
            p[1] *= g;
            p[2] *= g;
        }
    }
}

/// Rec. 709 luma weights, the same ones the compositor uses.
const LUMA: [f32; 3] = [0.2126, 0.7152, 0.0722];

/// The multiplier of the contrast control: `-1..0` flattens towards grey, `0..1` stretches (up to
/// four times).
pub fn contrast_gain(contrast: f32) -> f32 {
    if contrast >= 0.0 {
        1.0 + 3.0 * contrast
    } else {
        1.0 + contrast
    }
}

/// Brightness adds to each channel and contrast stretches around mid grey, on straight colour. A
/// pixel with no alpha stays as it is.
pub fn brightness_contrast_px(p: [f32; 4], brightness: f32, gain: f32) -> [f32; 4] {
    let a = p[3];
    if a <= 1e-6 {
        return p;
    }
    let mut out = [0.0, 0.0, 0.0, a];
    for c in 0..3 {
        let v = ((p[c] / a - 0.5) * gain + 0.5 + brightness).clamp(0.0, 1.0);
        out[c] = v * a;
    }
    out
}

/// The size in pixels of a pixelate block for a size given as a fraction of the frame height; never
/// under one pixel.
pub fn pixelate_block(size: f32, height: u32) -> u32 {
    (range_pixels(size, height).round() as u32).max(1)
}

fn black_and_white(work: &mut Work, amount: f32) {
    for p in &mut work.px {
        let l = p[0] * LUMA[0] + p[1] * LUMA[1] + p[2] * LUMA[2];
        for v in p.iter_mut().take(3) {
            *v += amount * (l - *v);
        }
    }
}

fn invert(work: &mut Work, amount: f32) {
    for p in &mut work.px {
        for c in 0..3 {
            p[c] += amount * ((p[3] - p[c]) - p[c]);
        }
    }
}

fn brightness_contrast(work: &mut Work, brightness: f32, contrast: f32) {
    let gain = contrast_gain(contrast);
    for p in &mut work.px {
        *p = brightness_contrast_px(*p, brightness, gain);
    }
}

/// Every block takes the pixel at its centre.
fn pixelate(work: &Work, h_size: f32, v_size: f32) -> Work {
    let (w, h) = (work.width, work.height);
    let bw = pixelate_block(h_size, h as u32) as usize;
    let bh = pixelate_block(v_size, h as u32) as usize;
    let mut px = Vec::with_capacity(w * h);
    for y in 0..h {
        let sy = (y / bh * bh + bh / 2).min(h - 1);
        for x in 0..w {
            let sx = (x / bw * bw + bw / 2).min(w - 1);
            px.push(work.px[sy * w + sx]);
        }
    }
    Work {
        width: w,
        height: h,
        px,
    }
}

/// Bilinear sample of the working buffer at pixel position `(x, y)` (pixel centres at `+0.5`).
/// Taps outside the picture are transparent.
fn sample_work(work: &Work, x: f32, y: f32) -> [f32; 4] {
    let (w, h) = (work.width as i64, work.height as i64);
    let (fx, fy) = (x - 0.5, y - 0.5);
    let (x0, y0) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - x0, fy - y0);
    let (x0, y0) = (x0 as i64, y0 as i64);
    let mut acc = [0.0f32; 4];
    for (dy, wy) in [(0, 1.0 - ty), (1, ty)] {
        for (dx, wx) in [(0, 1.0 - tx), (1, tx)] {
            let (px, py) = (x0 + dx, y0 + dy);
            if px < 0 || py < 0 || px >= w || py >= h {
                continue;
            }
            let p = work.px[py as usize * w as usize + px as usize];
            let weight = wx * wy;
            for c in 0..4 {
                acc[c] += p[c] * weight;
            }
        }
    }
    acc
}

/// The side of the mirror line that is kept: `angle` `0` keeps the top and reflects it onto the
/// bottom, `180` the other way round. Positions are in pixels with the origin at the lower-left
/// corner and `y` up, so the line has the angle it shows. What the reflection brings from outside the
/// frame is transparent.
fn mirror(work: &Work, angle: f32, center: [f32; 2]) -> Work {
    let (w, h) = (work.width, work.height);
    let (sin, cos) = angle.to_radians().sin_cos();
    let normal = (-sin, cos);
    let (cx, cy) = (center[0] * w as f32, center[1] * h as f32);
    let mut px = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let (qx, qy) = (x as f32 + 0.5, h as f32 - (y as f32 + 0.5));
            let side = (qx - cx) * normal.0 + (qy - cy) * normal.1;
            if side >= 0.0 {
                px.push(work.px[y * w + x]);
            } else {
                let (rx, ry) = (qx - 2.0 * side * normal.0, qy - 2.0 * side * normal.1);
                px.push(sample_work(work, rx, h as f32 - ry));
            }
        }
    }
    Work {
        width: w,
        height: h,
        px,
    }
}

/// The colour a grey of luma `l` takes with the sepia filter: black stays black, white stays white
/// and the middle is the filter colour.
pub fn sepia_tone(l: f32, tint: [f32; 3]) -> [f32; 3] {
    let mut out = [0.0; 3];
    for c in 0..3 {
        out[c] = if l < 0.5 {
            tint[c] * 2.0 * l
        } else {
            tint[c] + (1.0 - tint[c]) * (2.0 * l - 1.0)
        };
    }
    out
}

fn sepia(work: &mut Work, tint: [f32; 3], strength: f32) {
    for p in &mut work.px {
        let a = p[3];
        if a <= 1e-6 {
            continue;
        }
        let straight = [p[0] / a, p[1] / a, p[2] / a];
        let l = straight[0] * LUMA[0] + straight[1] * LUMA[1] + straight[2] * LUMA[2];
        let toned = sepia_tone(l, tint);
        for c in 0..3 {
            p[c] = (straight[c] + strength * (toned[c] - straight[c])) * a;
        }
    }
}

/// Twists the picture around `center` (fractions of the frame, lower-left origin). `size` is the
/// radius of the swirl on each axis as a fraction of the frame height; the turn at the centre is
/// `ratio` full turns and falls to nothing at the edge of that ellipse.
fn swirl(work: &Work, center: [f32; 2], ratio: f32, size: [f32; 2]) -> Work {
    let (w, h) = (work.width, work.height);
    let (cx, cy) = (center[0] * w as f32, (1.0 - center[1]) * h as f32);
    let (rx, ry) = (size[0] * h as f32, size[1] * h as f32);
    let mut px = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            let r = ((dx / rx).powi(2) + (dy / ry).powi(2)).sqrt();
            if r >= 1.0 {
                px.push(work.px[y * w + x]);
                continue;
            }
            let turn = ratio * std::f32::consts::TAU * (1.0 - r) * (1.0 - r);
            let (sin, cos) = turn.sin_cos();
            px.push(sample_work(
                work,
                cx + cos * dx - sin * dy,
                cy + sin * dx + cos * dy,
            ));
        }
    }
    Work {
        width: w,
        height: h,
        px,
    }
}

/// Taps along the ray of a radial blur.
pub const RADIAL_TAPS: u32 = 24;

/// How far toward the centre the last tap of a radial blur goes at level `1.0`.
pub const RADIAL_REACH: f32 = 0.4;

/// Longest half width of the median window, in pixels (the window is at most 9 by 9).
pub const MEDIAN_MAX_RADIUS: u32 = 4;

/// The half width in pixels of a median window for a range given as a fraction of the frame height.
pub fn median_radius(range: f32, height: u32) -> u32 {
    (range_pixels(range, height).round().max(0.0) as u32).min(MEDIAN_MAX_RADIUS)
}

/// A proportional blur: every pixel is the mean of taps on the line to the centre, the farther from
/// the centre the longer the line (a zoom blur). `level` `1.0` reaches [`RADIAL_REACH`] of the way.
fn radial_blur(work: &Work, center: [f32; 2], level: f32) -> Work {
    let (w, h) = (work.width, work.height);
    let (cx, cy) = (center[0] * w as f32, (1.0 - center[1]) * h as f32);
    let mut px = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let (qx, qy) = (x as f32 + 0.5, y as f32 + 0.5);
            let mut acc = [0.0f32; 4];
            for k in 0..RADIAL_TAPS {
                let s = 1.0 - level * RADIAL_REACH * k as f32 / (RADIAL_TAPS - 1) as f32;
                let t = sample_work(work, cx + (qx - cx) * s, cy + (qy - cy) * s);
                for c in 0..4 {
                    acc[c] += t[c];
                }
            }
            px.push(acc.map(|v| v / RADIAL_TAPS as f32));
        }
    }
    Work {
        width: w,
        height: h,
        px,
    }
}

/// Every channel becomes the median of the window around the pixel (edges repeat).
fn median(work: &Work, rx: u32, ry: u32) -> Work {
    let (w, h) = (work.width, work.height);
    let (rx, ry) = (rx as isize, ry as isize);
    let mut out = vec![[0.0f32; 4]; w * h];
    par_rows(&mut out, w, |first_row, band| {
        let mut window: Vec<f32> = Vec::with_capacity(81);
        for (ry_row, row) in band.chunks_mut(w).enumerate() {
            let y = (first_row + ry_row) as isize;
            for (x, dst) in row.iter_mut().enumerate() {
                for (c, value) in dst.iter_mut().enumerate() {
                    window.clear();
                    for dy in -ry..=ry {
                        for dx in -rx..=rx {
                            let sx = (x as isize + dx).clamp(0, w as isize - 1) as usize;
                            let sy = (y + dy).clamp(0, h as isize - 1) as usize;
                            window.push(work.px[sy * w + sx][c]);
                        }
                    }
                    window.sort_unstable_by(f32::total_cmp);
                    *value = window[window.len() / 2];
                }
            }
        }
    });
    Work {
        width: w,
        height: h,
        px: out,
    }
}

/// The hue offset in degrees of the "add to hue" slider: `0.0` and `1.0` both leave the colours as
/// they are.
pub fn hue_degrees(add_to_hue: f32) -> f32 {
    add_to_hue * 360.0
}

/// Hue, saturation and luminosity (HSL) of straight colour `c`: hue in degrees.
fn rgb_to_hsl(c: [f32; 3]) -> [f32; 3] {
    let mx = c[0].max(c[1]).max(c[2]);
    let mn = c[0].min(c[1]).min(c[2]);
    let l = (mx + mn) / 2.0;
    let d = mx - mn;
    if d < 1e-6 {
        return [0.0, 0.0, l];
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs()).max(1e-6);
    let h = if mx == c[0] {
        ((c[1] - c[2]) / d).rem_euclid(6.0)
    } else if mx == c[1] {
        (c[2] - c[0]) / d + 2.0
    } else {
        (c[0] - c[1]) / d + 4.0
    };
    [h * 60.0, s, l]
}

fn hsl_to_rgb(hsl: [f32; 3]) -> [f32; 3] {
    let (h, s, l) = (hsl[0].rem_euclid(360.0), hsl[1], hsl[2]);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - (hp.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    [r + m, g + m, b + m]
}

/// The HSL adjustment of one premultiplied pixel: the hue turns by `hue` degrees, the saturation
/// and the luminosity are multiplied.
pub fn hsl_adjust_px(p: [f32; 4], hue: f32, saturation: f32, luminosity: f32) -> [f32; 4] {
    let a = p[3];
    if a <= 1e-6 {
        return p;
    }
    let mut hsl = rgb_to_hsl([p[0] / a, p[1] / a, p[2] / a]);
    hsl[0] += hue;
    hsl[1] = (hsl[1] * saturation).clamp(0.0, 1.0);
    hsl[2] = (hsl[2] * luminosity).clamp(0.0, 1.0);
    let rgb = hsl_to_rgb(hsl);
    [
        rgb[0].clamp(0.0, 1.0) * a,
        rgb[1].clamp(0.0, 1.0) * a,
        rgb[2].clamp(0.0, 1.0) * a,
        a,
    ]
}

fn hsl_adjust(work: &mut Work, add_to_hue: f32, saturation: f32, luminosity: f32) {
    let hue = hue_degrees(add_to_hue);
    for p in &mut work.px {
        *p = hsl_adjust_px(*p, hue, saturation, luminosity);
    }
}

/// Taps on each side of the centre of a linear blur.
pub const LINEAR_TAPS: u32 = 16;

/// The longest half length of a linear blur (ratio `1.0`) as a fraction of the frame height.
pub const LINEAR_REACH: f32 = 0.05;

/// The unit vector of a linear blur at `angle` degrees (counterclockwise from the right, `y` down on
/// the screen) and its half length in pixels.
pub fn linear_blur_line(angle: f32, ratio: f32, height: u32) -> (f32, f32, f32) {
    let (sin, cos) = angle.to_radians().sin_cos();
    (cos, -sin, ratio * LINEAR_REACH * height as f32)
}

/// Blurs along a line: the mean of `2 * LINEAR_TAPS + 1` taps spread over the half length on each
/// side of the pixel, the taps outside the picture taking the edge.
fn linear_blur(work: &Work, angle: f32, ratio: f32) -> Work {
    let (dx, dy, half) = linear_blur_line(angle, ratio, work.height as u32);
    blur_along(work, dx, dy, half)
}

/// The blur of [`linear_blur`] along the unit vector `(dx, dy)` (`y` down) with a half length in
/// pixels.
pub fn blur_along(work: &Work, dx: f32, dy: f32, half: f32) -> Work {
    let (w, h) = (work.width, work.height);
    let (max_x, max_y) = (w as f32 - 0.5, h as f32 - 0.5);
    let mut px = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let (qx, qy) = (x as f32 + 0.5, y as f32 + 0.5);
            let mut acc = [0.0f32; 4];
            for k in 0..=(2 * LINEAR_TAPS) {
                let t = (k as f32 - LINEAR_TAPS as f32) / LINEAR_TAPS as f32 * half;
                let s = sample_work(
                    work,
                    (qx + dx * t).clamp(0.5, max_x),
                    (qy + dy * t).clamp(0.5, max_y),
                );
                for c in 0..4 {
                    acc[c] += s[c];
                }
            }
            px.push(acc.map(|v| v / (2 * LINEAR_TAPS + 1) as f32));
        }
    }
    Work {
        width: w,
        height: h,
        px,
    }
}

/// Runs `op` on the straight (not premultiplied) colour of every pixel that has any alpha, and
/// puts the clamped result back, premultiplied. Alpha is kept.
fn map_straight(work: &mut Work, op: impl Fn([f32; 3]) -> [f32; 3]) {
    for p in &mut work.px {
        let a = p[3];
        if a <= 1e-6 {
            continue;
        }
        let c = op([p[0] / a, p[1] / a, p[2] / a]);
        for k in 0..3 {
            p[k] = c[k].clamp(0.0, 1.0) * a;
        }
    }
}

/// How much of each colour offset a pixel of luma `l` takes from the shadows, the midtones and the
/// highlights (they add up to one).
fn balance_weights(l: f32) -> (f32, f32, f32) {
    let shadows = (1.0 - l) * (1.0 - l);
    let highlights = l * l;
    (shadows, (1.0 - shadows - highlights).max(0.0), highlights)
}

/// Offsets of the colour balance per zone are `value * BALANCE_SCALE`.
pub const BALANCE_SCALE: f32 = 0.25;

/// Colour balance of one straight pixel (see the project note on the source of these formulas).
pub fn color_balance_px(
    c: [f32; 3],
    shadows: [f32; 3],
    mids: [f32; 3],
    highlights: [f32; 3],
    preserve: bool,
) -> [f32; 3] {
    let l = LUMA[0] * c[0] + LUMA[1] * c[1] + LUMA[2] * c[2];
    let (ws, wm, wh) = balance_weights(l);
    let mut out = [0.0; 3];
    for k in 0..3 {
        out[k] = c[k] + shadows[k] * ws + mids[k] * wm + highlights[k] * wh;
    }
    if preserve {
        let l2 = LUMA[0] * out[0] + LUMA[1] * out[1] + LUMA[2] * out[2];
        let d = l - l2;
        out = out.map(|v| v + d);
    }
    out
}

/// The levels curve on one channel: input black/white, gamma exponent, output black/white.
pub fn levels_px(v: f32, ib: f32, iw: f32, ob: f32, ow: f32, g: f32) -> f32 {
    ob + ((v - ib) / (iw - ib)).clamp(0.0, 1.0).powf(g) * (ow - ob)
}

/// Grey that the extract effect gives a pixel of luma `l`: `1` inside `lo..hi`, `0` outside, with a
/// `soft` wide ramp at each cut when it is not zero.
pub fn extract_px(l: f32, lo: f32, hi: f32, soft: f32, invert: bool) -> f32 {
    let inside = if soft <= 0.0 {
        f32::from(l > lo && l < hi)
    } else {
        ((l - lo) / soft)
            .clamp(0.0, 1.0)
            .min(((hi - l) / soft).clamp(0.0, 1.0))
    };
    if invert {
        1.0 - inside
    } else {
        inside
    }
}

fn luma_of(c: [f32; 3]) -> f32 {
    LUMA[0] * c[0] + LUMA[1] * c[1] + LUMA[2] * c[2]
}

/// The hue of a straight colour as a fraction of the circle (`0.0..1.0`).
fn hue_fraction(c: [f32; 3]) -> f32 {
    rgb_to_hsl(c)[0] / 360.0
}

/// Distance between two hues on the circle (`0.0..=0.5`).
fn hue_distance(a: f32, b: f32) -> f32 {
    let d = (a - b).abs();
    d.min(1.0 - d)
}

/// The ASC CDL of one straight colour: slope and offset, a power curve, then saturation around the
/// luma.
pub fn cdl_px(
    c: [f32; 3],
    slope: [f32; 3],
    offset: [f32; 3],
    power: [f32; 3],
    sat: f32,
) -> [f32; 3] {
    let mut v = [0.0; 3];
    for k in 0..3 {
        v[k] = (c[k] * slope[k] + offset[k])
            .clamp(0.0, 1.0)
            .powf(power[k].max(0.0));
    }
    let l = luma_of(v);
    v.map(|q| (l + sat * (q - l)).clamp(0.0, 1.0))
}

/// Leave Color: the colours away from the key hue lose `amount` of their colour.
pub fn leave_color_px(c: [f32; 3], amount: f32, key_hue: f32, tol: f32, soft: f32) -> [f32; 3] {
    let d = hue_distance(hue_fraction(c), key_hue) * 2.0;
    let keep = 1.0 - ((d - tol) / soft).clamp(0.0, 1.0);
    let l = luma_of(c);
    let k = amount * (1.0 - keep);
    c.map(|v| v + (l - v) * k)
}

/// Change to Color: hues near `from_hue` move toward `to_hue`.
pub fn change_to_color_px(
    c: [f32; 3],
    from_hue: f32,
    to_hue: f32,
    tol: f32,
    soft: f32,
) -> [f32; 3] {
    let mut hsl = rgb_to_hsl(c);
    let h = hsl[0] / 360.0;
    let w = 1.0 - ((hue_distance(h, from_hue) - tol) / soft).clamp(0.0, 1.0);
    hsl[0] = (h + (to_hue - from_hue) * w).rem_euclid(1.0) * 360.0;
    hsl_to_rgb(hsl)
}

fn alpha_adjust(work: &mut Work, opacity: f32, ignore: bool, invert: bool, mask_only: bool) {
    for p in &mut work.px {
        let a0 = p[3];
        let straight = if a0 > 1e-6 {
            [p[0] / a0, p[1] / a0, p[2] / a0]
        } else {
            [0.0; 3]
        };
        let mut a = if ignore { 1.0 } else { a0 };
        if invert {
            a = 1.0 - a;
        }
        a = (a * opacity).clamp(0.0, 1.0);
        *p = if mask_only {
            [a, a, a, 1.0]
        } else {
            [straight[0] * a, straight[1] * a, straight[2] * a, a]
        };
    }
}

fn flip(work: &Work, horizontal: bool) -> Work {
    let (w, h) = (work.width, work.height);
    let mut px = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let (sx, sy) = if horizontal {
                (w - 1 - x, y)
            } else {
                (x, h - 1 - y)
            };
            px.push(work.px[sy * w + sx]);
        }
    }
    Work {
        width: w,
        height: h,
        px,
    }
}

/// How far the picture shifts so that its centre lands on `shift` (fractions of the frame, lower-left
/// origin), wrapped into one frame: `(dx, dy)` in pixels, `y` down.
pub fn offset_pixels(shift: [f32; 2], width: u32, height: u32) -> (f32, f32) {
    let (w, h) = (width.max(1) as f32, height.max(1) as f32);
    (
        ((shift[0] - 0.5) * w).rem_euclid(w),
        (((1.0 - shift[1]) - 0.5) * h).rem_euclid(h),
    )
}

/// The picture wrapped around by `(dx, dy)` pixels and mixed with the original by `blend`.
fn offset(work: &Work, dx: f32, dy: f32, blend: f32) -> Work {
    let (w, h) = (work.width, work.height);
    let (wf, hf) = (w as f32, h as f32);
    let mut px = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let u = (x as f32 + 0.5 - dx).rem_euclid(wf).clamp(0.5, wf - 0.5);
            let v = (y as f32 + 0.5 - dy).rem_euclid(hf).clamp(0.5, hf - 0.5);
            let shifted = sample_work(work, u, v);
            let original = work.px[y * w + x];
            let mut out = [0.0; 4];
            for c in 0..4 {
                out[c] = shifted[c] + (original[c] - shifted[c]) * blend;
            }
            px.push(out);
        }
    }
    Work {
        width: w,
        height: h,
        px,
    }
}

/// Saturation gain for a hue: `ratio` inside the range of `spread` around `center`, `low` for the
/// hues before it and `up` for the hues after it (hues, `center` and `spread` are fractions of the
/// colour circle), blended with a soft edge.
pub fn saturation_gain(hue: f32, ratio: f32, center: f32, spread: f32, low: f32, up: f32) -> f32 {
    let raw = hue - center;
    let offset = raw - (raw + 0.5).floor();
    let t = ((offset.abs() - spread * 0.5) / 0.05).clamp(0.0, 1.0);
    let inside = 1.0 - t * t * (3.0 - 2.0 * t);
    ratio * inside + if offset < 0.0 { low } else { up } * (1.0 - inside)
}

fn saturation_adjust(work: &mut Work, ratio: f32, center: f32, spread: f32, low: f32, up: f32) {
    for p in &mut work.px {
        let a = p[3];
        if a <= 1e-6 {
            continue;
        }
        let mut hsl = rgb_to_hsl([p[0] / a, p[1] / a, p[2] / a]);
        let gain = saturation_gain(hsl[0] / 360.0, ratio, center, spread, low, up);
        hsl[1] = (hsl[1] * (1.0 + gain)).clamp(0.0, 1.0);
        let rgb = hsl_to_rgb(hsl);
        for k in 0..3 {
            p[k] = rgb[k].clamp(0.0, 1.0) * a;
        }
    }
}

/// Runs one resolved effect on the working buffer.
pub fn apply_resolved(work: &mut Work, effect: &Resolved) {
    match *effect {
        Resolved::GaussianBlur { h_range, v_range } => {
            *work = gaussian_blur(work, h_range, v_range)
        }
        Resolved::Sharpen => *work = sharpen(work),
        Resolved::UnsharpMask { ratio } => *work = unsharp(work, ratio),
        Resolved::Vignette {
            outer,
            inner,
            aspect,
            center,
        } => vignette(work, outer, inner, aspect, center),
        Resolved::BlackAndWhite { amount } => black_and_white(work, amount),
        Resolved::Invert { amount } => invert(work, amount),
        Resolved::BrightnessContrast {
            brightness,
            contrast,
        } => brightness_contrast(work, brightness, contrast),
        Resolved::Pixelate { h_size, v_size } => *work = pixelate(work, h_size, v_size),
        Resolved::Mirror { angle, center } => *work = mirror(work, angle, center),
        Resolved::Sepia { tint, strength } => sepia(work, tint, strength),
        Resolved::Swirl {
            center,
            ratio,
            size,
        } => *work = swirl(work, center, ratio, size),
        Resolved::RadialBlur { center, level } => *work = radial_blur(work, center, level),
        Resolved::Median { h_range, v_range } => {
            let h = work.height as u32;
            *work = median(work, median_radius(h_range, h), median_radius(v_range, h));
        }
        Resolved::HslAdjust {
            add_to_hue,
            saturation,
            luminosity,
        } => hsl_adjust(work, add_to_hue, saturation, luminosity),
        Resolved::LinearBlur { angle, ratio } => *work = linear_blur(work, angle, ratio),
        Resolved::ChannelMix { m } => map_straight(work, |c| {
            [0, 1, 2].map(|k| m[k][0] * c[0] + m[k][1] * c[1] + m[k][2] * c[2] + m[k][3])
        }),
        Resolved::AscCdl {
            slope,
            offset,
            power,
            sat,
        } => map_straight(work, |c| cdl_px(c, slope, offset, power, sat)),
        Resolved::LeaveColor {
            amount,
            key_hue,
            tol,
            soft,
        } => map_straight(work, |c| leave_color_px(c, amount, key_hue, tol, soft)),
        Resolved::ChangeToColor {
            from_hue,
            to_hue,
            tol,
            soft,
        } => map_straight(work, |c| change_to_color_px(c, from_hue, to_hue, tol, soft)),
        Resolved::ColorPass { key, sim, reverse } => map_straight(work, |c| {
            let d = ((c[0] - key[0]).powi(2) + (c[1] - key[1]).powi(2) + (c[2] - key[2]).powi(2))
                .sqrt();
            if (d <= sim * 1.2) != reverse {
                c
            } else {
                [luma_of(c); 3]
            }
        }),
        Resolved::AlphaAdjust {
            opacity,
            ignore,
            invert,
            mask_only,
        } => alpha_adjust(work, opacity, ignore, invert, mask_only),
        Resolved::HFlip => *work = flip(work, true),
        Resolved::VFlip => *work = flip(work, false),
        Resolved::Offset { shift, blend } => {
            let (dx, dy) = offset_pixels(shift, work.width as u32, work.height as u32);
            *work = offset(work, dx, dy, blend);
        }
        Resolved::SaturationAdjust {
            ratio,
            center,
            spread,
            low,
            up,
        } => saturation_adjust(work, ratio, center, spread, low, up),
        Resolved::ColorBalance {
            shadows,
            mids,
            highlights,
            preserve,
        } => map_straight(work, |c| {
            color_balance_px(c, shadows, mids, highlights, preserve)
        }),
        Resolved::Levels { ib, iw, ob, ow, g } => {
            map_straight(work, |c| c.map(|v| levels_px(v, ib, iw, ob, ow, g)))
        }
        Resolved::Gamma { g } => map_straight(work, |c| c.map(|v| v.max(0.0).powf(g))),
        Resolved::Posterize { n } => map_straight(work, |c| c.map(|v| (v * n).round() / n)),
        Resolved::Extract {
            lo,
            hi,
            soft,
            invert,
        } => map_straight(work, |c| [extract_px(luma_of(c), lo, hi, soft, invert); 3]),
        Resolved::Tint {
            black,
            white,
            amount,
        } => map_straight(work, |c| {
            let l = luma_of(c);
            let mut out = [0.0; 3];
            for k in 0..3 {
                let target = black[k] + (white[k] - black[k]) * l;
                out[k] = c[k] + (target - c[k]) * amount;
            }
            out
        }),
    }
}

/// Runs the chain on the CPU. An empty (or all neutral) chain leaves the frame untouched.
pub fn apply_chain_cpu(frame: &mut FrameBuffer, chain: &[VideoEffect]) {
    let resolved = resolve_chain(chain);
    if resolved.is_empty() || frame.width == 0 || frame.height == 0 {
        return;
    }
    let mut work = Work::from_frame(frame);
    for effect in &resolved {
        apply_resolved(&mut work, effect);
    }
    work.write_to(frame);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generators::smpte_color_bars;
    use std::collections::hash_map::DefaultHasher;

    fn mean(frame: &FrameBuffer) -> [f64; 3] {
        let n = (frame.width * frame.height) as f64;
        let mut s = [0.0; 3];
        for p in frame.rgba.as_chunks::<4>().0.iter() {
            for c in 0..3 {
                s[c] += f64::from(p[c]);
            }
        }
        [s[0] / n, s[1] / n, s[2] / n]
    }

    fn hash(chain: &[VideoEffect]) -> u64 {
        let mut h = DefaultHasher::new();
        hash_chain(chain, &mut h);
        h.finish()
    }

    #[test]
    fn black_and_white_inverts_and_brightness_follow_their_definitions() {
        let mut gray = smpte_color_bars(64, 36);
        apply_chain_cpu(&mut gray, &[VideoEffect::new(BLACK_AND_WHITE)]);
        for p in gray.rgba.as_chunks::<4>().0.iter() {
            assert!(
                p[0].abs_diff(p[1]) <= 1 && p[1].abs_diff(p[2]) <= 1,
                "{p:?}"
            );
        }
        // Half of the amount sits half way between the colour and the grey
        let original = FrameBuffer {
            width: 1,
            height: 1,
            rgba: vec![200, 100, 0, 255],
        };
        let mut half = original.clone();
        apply_chain_cpu(
            &mut half,
            &[VideoEffect::new(BLACK_AND_WHITE).with("amount", 0.5)],
        );
        let luma = 200.0 * 0.2126 + 100.0 * 0.7152;
        assert!((f32::from(half.rgba[0]) - (200.0 + luma) / 2.0).abs() <= 1.0);
        // Invert twice gives the picture back, also with transparency
        let mut twice = FrameBuffer {
            width: 2,
            height: 1,
            rgba: vec![10, 200, 30, 255, 90, 40, 250, 128],
        };
        let before = twice.clone();
        apply_chain_cpu(&mut twice, &[VideoEffect::new(INVERT)]);
        assert_eq!(twice.rgba[..4], [245, 55, 225, 255]);
        assert_eq!(twice.rgba[7], 128, "alpha is kept");
        apply_chain_cpu(&mut twice, &[VideoEffect::new(INVERT)]);
        assert!(twice
            .rgba
            .iter()
            .zip(&before.rgba)
            .all(|(a, b)| a.abs_diff(*b) <= 2));
        // Brightness adds, contrast stretches around mid grey
        let mut bright = FrameBuffer {
            width: 1,
            height: 1,
            rgba: vec![100, 100, 100, 255],
        };
        apply_chain_cpu(
            &mut bright,
            &[VideoEffect::new(BRIGHTNESS_CONTRAST).with("brightness", 0.2)],
        );
        assert_eq!(bright.rgba[..3], [151, 151, 151]);
        let mut stretched = FrameBuffer {
            width: 1,
            height: 1,
            rgba: vec![191, 64, 128, 255],
        };
        apply_chain_cpu(
            &mut stretched,
            &[VideoEffect::new(BRIGHTNESS_CONTRAST).with("contrast", 0.5)],
        );
        assert!(stretched.rgba[0] > 191 && stretched.rgba[1] < 64);
        assert!(
            (i32::from(stretched.rgba[2]) - 128).abs() <= 2,
            "mid grey stays"
        );
        // Neutral values leave the chain empty
        assert!(resolve_chain(&[VideoEffect::new(BRIGHTNESS_CONTRAST)]).is_empty());
        assert!(resolve_chain(&[VideoEffect::new(INVERT).with("amount", 0.0)]).is_empty());
    }

    #[test]
    fn pixelate_makes_blocks_of_one_colour_and_a_tiny_size_changes_nothing() {
        let original = smpte_color_bars(120, 60);
        let mut blocky = original.clone();
        apply_chain_cpu(
            &mut blocky,
            &[VideoEffect::new(PIXELATE)
                .with("h_size", 0.2)
                .with("v_size", 0.1)],
        );
        // 12 x 6 pixel blocks: every pixel of a block equals the first one
        let px = |f: &FrameBuffer, x: usize, y: usize| {
            let i = (y * 120 + x) * 4;
            f.rgba[i..i + 4].to_vec()
        };
        for (bx, by) in [(0usize, 0usize), (5, 3), (9, 7)] {
            let first = px(&blocky, bx * 12, by * 6);
            for (dx, dy) in [(1, 1), (11, 5), (6, 3)] {
                assert_eq!(px(&blocky, bx * 12 + dx, by * 6 + dy), first);
            }
        }
        assert_ne!(blocky, original);
        let mut same = original.clone();
        apply_chain_cpu(
            &mut same,
            &[VideoEffect::new(PIXELATE)
                .with("h_size", 0.0)
                .with("v_size", 0.0)],
        );
        assert_eq!(same, original);
    }

    #[test]
    fn the_mirror_reflects_the_kept_side_and_the_other_angle_keeps_the_other_half() {
        // Left to right gradient with the red channel, 8 x 4
        let mut frame = FrameBuffer {
            width: 8,
            height: 4,
            rgba: vec![0; 8 * 4 * 4],
        };
        for y in 0..4usize {
            for x in 0..8usize {
                let i = (y * 8 + x) * 4;
                frame.rgba[i] = (x * 30 + y) as u8;
                frame.rgba[i + 3] = 255;
            }
        }
        // Angle 0 keeps the top and copies it, flipped, to the bottom
        let mut top = frame.clone();
        apply_chain_cpu(&mut top, &[VideoEffect::new(MIRROR)]);
        for x in 0..8usize {
            assert_eq!(top.rgba[x * 4], frame.rgba[x * 4], "row 0 kept");
            assert_eq!(top.rgba[(8 + x) * 4], frame.rgba[(8 + x) * 4], "row 1 kept");
            assert_eq!(
                top.rgba[(16 + x) * 4],
                frame.rgba[(8 + x) * 4],
                "row 2 is row 1"
            );
            assert_eq!(top.rgba[(24 + x) * 4], frame.rgba[x * 4], "row 3 is row 0");
        }
        // Angle 180 keeps the bottom
        let mut bottom = frame.clone();
        apply_chain_cpu(
            &mut bottom,
            &[VideoEffect::new(MIRROR).with("angle", 180.0)],
        );
        for x in 0..8usize {
            assert_eq!(
                bottom.rgba[x * 4],
                frame.rgba[(24 + x) * 4],
                "row 0 is row 3"
            );
            assert_eq!(
                bottom.rgba[(24 + x) * 4],
                frame.rgba[(24 + x) * 4],
                "row 3 kept"
            );
        }
        // A line near the top edge reflects a lot of nothing: transparent, never garbage
        let mut edge = frame.clone();
        apply_chain_cpu(
            &mut edge,
            &[VideoEffect::new(MIRROR)
                .with("angle", 180.0)
                .with("center_y", 1.0)],
        );
        assert!(edge
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| p[3] == 255 || p[3] == 0));
        assert_eq!(edge.rgba[3], 255, "the kept side is the one below the line");
    }

    #[test]
    fn sepia_tints_greys_and_keeps_black_white_and_alpha_and_the_swirl_leaves_the_far_corners() {
        let mut grey = FrameBuffer {
            width: 3,
            height: 1,
            rgba: vec![0, 0, 0, 255, 128, 128, 128, 255, 255, 255, 255, 128],
        };
        apply_chain_cpu(&mut grey, &[VideoEffect::new(SEPIA)]);
        assert_eq!(grey.rgba[..3], [0, 0, 0], "black stays black");
        let mid = &grey.rgba[4..7];
        assert!(
            mid[0] > mid[1] && mid[1] > mid[2],
            "the middle is warm: {mid:?}"
        );
        assert_eq!(
            grey.rgba[8..12],
            [255, 255, 255, 128],
            "white and alpha stay"
        );
        let mut off = grey.clone();
        apply_chain_cpu(&mut off, &[VideoEffect::new(SEPIA).with("strength", 0.0)]);
        assert_eq!(off, grey, "no strength, no change");
        // A swirl turns the middle and leaves what is outside its radius
        let original = smpte_color_bars(120, 60);
        let mut twisted = original.clone();
        apply_chain_cpu(
            &mut twisted,
            &[VideoEffect::new(SWIRL)
                .with("h_size", 0.4)
                .with("v_size", 0.4)],
        );
        assert_ne!(twisted, original);
        assert_eq!(
            twisted.rgba[..4],
            original.rgba[..4],
            "the corner is far away"
        );
        let mut none = original.clone();
        apply_chain_cpu(&mut none, &[VideoEffect::new(SWIRL).with("ratio", 0.0)]);
        assert_eq!(none, original);
    }

    #[test]
    fn the_radial_blur_spares_the_centre_and_the_median_removes_a_speck() {
        let original = smpte_color_bars(120, 60);
        let mut zoomed = original.clone();
        apply_chain_cpu(
            &mut zoomed,
            &[VideoEffect::new(RADIAL_BLUR).with("level", 1.0)],
        );
        let centre = (30 * 120 + 60) * 4;
        assert!(zoomed.rgba[centre..centre + 3]
            .iter()
            .zip(&original.rgba[centre..centre + 3])
            .all(|(a, b)| a.abs_diff(*b) <= 2));
        assert_ne!(zoomed, original, "the edges of the bars blur");
        assert!(resolve_chain(&[VideoEffect::new(RADIAL_BLUR).with("level", 0.0)]).is_empty());
        // A single bright pixel on a dark picture disappears
        let mut speck = FrameBuffer {
            width: 21,
            height: 300,
            rgba: [10u8, 10, 10, 255].repeat(21 * 300),
        };
        let at = (150 * 21 + 10) * 4;
        speck.rgba[at..at + 3].copy_from_slice(&[250, 250, 250]);
        let before = speck.clone();
        // 300 pixels tall: the largest range, 0.004 of the height, is one pixel on each side
        apply_chain_cpu(
            &mut speck,
            &[VideoEffect::new(MEDIAN)
                .with("h_range", 0.004)
                .with("v_range", 0.004)],
        );
        assert_ne!(speck, before);
        assert!(speck.rgba.as_chunks::<4>().0.iter().all(|p| p[0] == 10));
        assert_eq!(median_radius(0.5, 1080), MEDIAN_MAX_RADIUS);
        assert_eq!(median_radius(0.0, 1080), 0);
        assert!(resolve_chain(&[VideoEffect::new(MEDIAN)
            .with("h_range", 0.0)
            .with("v_range", 0.0)])
        .is_empty());
    }

    #[test]
    fn hsl_turns_the_hue_scales_the_saturation_and_the_linear_blur_follows_its_angle() {
        let red = |extra: &[(&str, f32)]| {
            let mut fx = VideoEffect::new(HSL_ADJUST);
            for (k, v) in extra {
                fx = fx.with(k, *v);
            }
            let mut f = FrameBuffer {
                width: 1,
                height: 1,
                rgba: vec![255, 0, 0, 200],
            };
            apply_chain_cpu(&mut f, &[fx]);
            f.rgba
        };
        let green = red(&[("add_to_hue", 1.0 / 3.0)]);
        assert!(
            green[0] <= 1 && green[1] >= 254 && green[2] <= 1,
            "{green:?}"
        );
        assert_eq!(green[3], 200, "alpha stays");
        let grey = red(&[("saturation", 0.0)]);
        assert!(grey[0] == grey[1] && grey[1] == grey[2], "{grey:?}");
        assert_eq!(red(&[("luminosity", 0.0)])[..3], [0, 0, 0]);
        assert!(resolve_chain(&[VideoEffect::new(HSL_ADJUST)]).is_empty());
        assert!(resolve_chain(&[VideoEffect::new(HSL_ADJUST).with("add_to_hue", 1.0)]).is_empty());
        // A thin vertical line: a horizontal blur spreads it, a vertical one leaves it alone
        let mut line = FrameBuffer {
            width: 100,
            height: 100,
            rgba: [0u8, 0, 0, 255].repeat(100 * 100),
        };
        for y in 0..100 {
            let i = (y * 100 + 50) * 4;
            line.rgba[i..i + 3].copy_from_slice(&[255, 255, 255]);
        }
        let at = |f: &FrameBuffer, x: usize| f.rgba[(40 * 100 + x) * 4];
        let mut across = line.clone();
        apply_chain_cpu(
            &mut across,
            &[VideoEffect::new(LINEAR_BLUR).with("ratio", 1.0)],
        );
        assert!(
            at(&across, 50) < 200 && at(&across, 48) > 10,
            "{} {}",
            at(&across, 50),
            at(&across, 48)
        );
        let mut along = line.clone();
        apply_chain_cpu(
            &mut along,
            &[VideoEffect::new(LINEAR_BLUR)
                .with("ratio", 1.0)
                .with("angle", 90.0)],
        );
        assert!(at(&along, 50) >= 253 && at(&along, 48) <= 2);
        assert!(resolve_chain(&[VideoEffect::new(LINEAR_BLUR).with("ratio", 0.0)]).is_empty());
    }

    #[test]
    fn the_colour_tools_do_what_their_formulas_say() {
        let px = |rgb: [u8; 3], fx: VideoEffect| {
            let mut f = FrameBuffer {
                width: 1,
                height: 1,
                rgba: vec![rgb[0], rgb[1], rgb[2], 255],
            };
            apply_chain_cpu(&mut f, &[fx]);
            [f.rgba[0], f.rgba[1], f.rgba[2]]
        };
        // Colour balance: red in the highlights lifts the bright pixel, hardly the dark one
        let balance = || VideoEffect::new(COLOR_BALANCE).with("hi_r", 1.0);
        let bright = px([200, 200, 200], balance());
        let dark = px([20, 20, 20], balance());
        assert!(bright[0] > 200 && bright[1] == 200, "{bright:?}");
        assert!(dark[0] <= 21, "{dark:?}");
        // Preserving the luminosity keeps the brightness while the colour moves
        let kept = px(
            [128, 128, 128],
            VideoEffect::new(COLOR_BALANCE)
                .with("mid_r", 1.0)
                .with("preserve", 1.0),
        );
        let luma =
            0.2126 * f32::from(kept[0]) + 0.7152 * f32::from(kept[1]) + 0.0722 * f32::from(kept[2]);
        assert!((luma - 128.0).abs() < 1.5 && kept[0] > kept[2], "{kept:?}");
        // Levels: a white point at 128 sends 128 to white, the output range squeezes the result
        assert_eq!(
            px(
                [128, 96, 0],
                VideoEffect::new(LEVELS).with("in_white", 128.0)
            ),
            [255, 191, 0]
        );
        assert_eq!(
            px(
                [255, 0, 128],
                VideoEffect::new(LEVELS).with("out_white", 100.0)
            ),
            [100, 0, 50]
        );
        // Gamma 20 (an exponent of 2) turns a half grey into a quarter
        assert_eq!(
            px(
                [128, 128, 128],
                VideoEffect::new(GAMMA_CORRECTION).with("gamma", 20.0)
            )[0],
            64
        );
        // Posterize to two levels leaves black and white only
        let two = px(
            [100, 200, 130],
            VideoEffect::new(POSTERIZE).with("levels", 2.0),
        );
        assert_eq!(two, [0, 255, 255]);
        // Extract keeps what is between the two cuts and drops the rest
        let extract = || {
            VideoEffect::new(EXTRACT)
                .with("black", 100.0)
                .with("white", 200.0)
        };
        assert_eq!(px([150, 150, 150], extract()), [255, 255, 255]);
        assert_eq!(px([50, 50, 50], extract()), [0, 0, 0]);
        assert_eq!(
            px([150, 150, 150], extract().with("invert", 1.0)),
            [0, 0, 0]
        );
        // Tint maps black and white to the two colours
        let tint = |rgb| {
            px(
                rgb,
                VideoEffect::new(TINT)
                    .with("white_g", 0.0)
                    .with("white_b", 0.0),
            )
        };
        assert_eq!(tint([255, 255, 255]), [255, 0, 0]);
        assert_eq!(tint([0, 0, 0]), [0, 0, 0]);
        assert_eq!(
            px([10, 20, 30], VideoEffect::new(TINT).with("amount", 0.0)),
            [10, 20, 30]
        );
        // Neutral settings are skipped
        for neutral in [COLOR_BALANCE, LEVELS] {
            assert!(
                resolve_chain(&[VideoEffect::new(neutral)]).is_empty(),
                "{neutral}"
            );
        }
        assert!(resolve_chain(&[VideoEffect::new(GAMMA_CORRECTION)]).is_empty());
        assert!(resolve_chain(&[VideoEffect::new(POSTERIZE).with("levels", 256.0)]).is_empty());
        assert!(resolve_chain(&[VideoEffect::new(TINT).with("amount", 0.0)]).is_empty());
    }

    #[test]
    fn the_second_batch_of_colour_and_image_tools_does_what_it_says() {
        let px = |rgb: [u8; 3], fx: VideoEffect| {
            let mut f = FrameBuffer {
                width: 1,
                height: 1,
                rgba: vec![rgb[0], rgb[1], rgb[2], 255],
            };
            apply_chain_cpu(&mut f, &[fx]);
            [f.rgba[0], f.rgba[1], f.rgba[2]]
        };
        // Channel mix: swapping red and blue; monochrome copies the red row to every channel
        let swap = VideoEffect::new(CHANNEL_MIX)
            .with("rr", 0.0)
            .with("rb", 1.0)
            .with("bb", 0.0)
            .with("br", 1.0);
        assert_eq!(px([10, 20, 30], swap), [30, 20, 10]);
        let mono = VideoEffect::new(CHANNEL_MIX).with("monochrome", 1.0);
        assert_eq!(px([10, 20, 30], mono), [10, 10, 10]);
        assert!(resolve_chain(&[VideoEffect::new(CHANNEL_MIX)]).is_empty());
        // ASC CDL: a slope of 2 doubles, an offset adds, saturation 0 is grey
        assert_eq!(
            px([50, 50, 50], VideoEffect::new(ASC_CDL).with("r_slope", 2.0))[0],
            100
        );
        let grey = px(
            [200, 100, 50],
            VideoEffect::new(ASC_CDL).with("saturation", 0.0),
        );
        assert!(grey[0] == grey[1] && grey[1] == grey[2]);
        assert!(resolve_chain(&[VideoEffect::new(ASC_CDL)]).is_empty());
        // Leave colour: red stays, blue loses its colour
        let leave = || VideoEffect::new(LEAVE_COLOR).with("amount", 1.0);
        assert_eq!(px([255, 0, 0], leave()), [255, 0, 0]);
        let blue = px([0, 0, 255], leave());
        assert!(blue[0] == blue[1] && blue[1] == blue[2], "{blue:?}");
        assert!(resolve_chain(&[VideoEffect::new(LEAVE_COLOR)]).is_empty());
        // Change to colour: red becomes blue, green is left alone
        let change = || VideoEffect::new(CHANGE_TO_COLOR);
        let turned = px([255, 0, 0], change());
        assert!(turned[2] > 240 && turned[0] < 15, "{turned:?}");
        assert_eq!(px([0, 255, 0], change()), [0, 255, 0]);
        // Colour pass: the key colour stays, everything else turns grey; reverse swaps them
        let pass = || VideoEffect::new(COLOR_PASS).with("similarity", 0.2);
        assert_eq!(px([255, 0, 0], pass()), [255, 0, 0]);
        let other = px([0, 0, 255], pass());
        assert!(other[0] == other[1] && other[1] == other[2]);
        assert_eq!(
            px([255, 0, 0], pass().with("reverse", 1.0))[1],
            px([255, 0, 0], pass().with("reverse", 1.0))[0]
        );
        // Alpha adjust: half opacity, inverted alpha, and the matte alone
        let rgba = |fx: VideoEffect| {
            let mut f = FrameBuffer {
                width: 1,
                height: 1,
                rgba: vec![200, 100, 50, 200],
            };
            apply_chain_cpu(&mut f, &[fx]);
            f.rgba
        };
        assert_eq!(
            rgba(VideoEffect::new(ALPHA_ADJUST).with("opacity", 0.5)),
            [200, 100, 50, 100]
        );
        assert_eq!(
            rgba(VideoEffect::new(ALPHA_ADJUST).with("invert", 1.0))[3],
            55
        );
        assert_eq!(
            rgba(VideoEffect::new(ALPHA_ADJUST).with("ignore", 1.0))[3],
            255
        );
        assert_eq!(
            rgba(VideoEffect::new(ALPHA_ADJUST).with("mask_only", 1.0)),
            [200, 200, 200, 255]
        );
        assert!(resolve_chain(&[VideoEffect::new(ALPHA_ADJUST)]).is_empty());
        // Flips and offset on a 4 by 2 picture whose red channel is the column
        let mut wide = FrameBuffer {
            width: 4,
            height: 2,
            rgba: vec![0; 4 * 2 * 4],
        };
        for y in 0..2 {
            for x in 0..4 {
                let i = (y * 4 + x) * 4;
                wide.rgba[i] = (x * 10 + y) as u8;
                wide.rgba[i + 3] = 255;
            }
        }
        let red = |f: &FrameBuffer| f.rgba.chunks(4).map(|p| p[0]).collect::<Vec<_>>();
        let mut h = wide.clone();
        apply_chain_cpu(&mut h, &[VideoEffect::new(HORIZONTAL_FLIP)]);
        assert_eq!(red(&h), [30, 20, 10, 0, 31, 21, 11, 1]);
        let mut v = wide.clone();
        apply_chain_cpu(&mut v, &[VideoEffect::new(VERTICAL_FLIP)]);
        assert_eq!(red(&v), [1, 11, 21, 31, 0, 10, 20, 30]);
        // The centre moved a quarter of the width to the right: the picture wraps by one column
        let mut shifted = wide.clone();
        apply_chain_cpu(
            &mut shifted,
            &[VideoEffect::new(OFFSET).with("shift_x", 0.75)],
        );
        assert_eq!(red(&shifted), [30, 0, 10, 20, 31, 1, 11, 21]);
        assert_eq!(offset_pixels([0.5, 0.5], 100, 50), (0.0, 0.0));
    }

    #[test]
    fn the_saturation_ranges_cut_or_boost_only_the_hues_they_name() {
        let px = |rgb: [u8; 3], fx: VideoEffect| {
            let mut f = FrameBuffer {
                width: 1,
                height: 1,
                rgba: vec![rgb[0], rgb[1], rgb[2], 255],
            };
            apply_chain_cpu(&mut f, &[fx]);
            [f.rgba[0], f.rgba[1], f.rgba[2]]
        };
        let grey = |c: [u8; 3]| c[0] == c[1] && c[1] == c[2];
        // Red is in the range around hue 0: cutting it by all turns it grey, green stays
        let inside = || {
            VideoEffect::new(SATURATION_ADJUST)
                .with("ratio", -1.0)
                .with("center", 0.0)
                .with("spread", 0.2)
        };
        assert!(grey(px([255, 0, 0], inside())));
        assert_eq!(px([0, 255, 0], inside()), [0, 255, 0]);
        // Below the range (green is 120 degrees on, blue 240, which is 120 before): the low side
        // holds blue, the up side holds green
        let low = || VideoEffect::new(SATURATION_ADJUST).with("low", -1.0);
        assert!(grey(px([0, 0, 255], low())));
        assert_eq!(px([0, 255, 0], low()), [0, 255, 0]);
        let up = || VideoEffect::new(SATURATION_ADJUST).with("up", -1.0);
        assert!(grey(px([0, 255, 0], up())));
        assert_eq!(px([0, 0, 255], up()), [0, 0, 255]);
        // Grey has no hue to change, and neutral values are skipped
        assert_eq!(px([90, 90, 90], inside()), [90, 90, 90]);
        assert!(resolve_chain(&[VideoEffect::new(SATURATION_ADJUST)]).is_empty());
        // The gain is continuous across the edge of the range
        let step = (0..200)
            .map(|i| saturation_gain(i as f32 / 200.0, -1.0, 0.0, 0.2, 0.0, 0.0))
            .collect::<Vec<_>>();
        assert!(step.windows(2).all(|w| (w[0] - w[1]).abs() < 0.3));
    }

    #[test]
    fn an_empty_or_neutral_chain_changes_nothing() {
        let original = smpte_color_bars(96, 54);
        let mut frame = original.clone();
        apply_chain_cpu(&mut frame, &[]);
        assert_eq!(frame, original);
        let neutral = [
            VideoEffect::new(GAUSSIAN_BLUR)
                .with("h_range", 0.0)
                .with("v_range", 0.0),
            VideoEffect::new(UNSHARP_MASK).with("ratio", 0.0),
            VideoEffect::new("not_an_effect"),
        ];
        apply_chain_cpu(&mut frame, &neutral);
        assert_eq!(frame, original);
    }

    #[test]
    fn values_are_clamped_and_bad_values_fall_back_to_the_default() {
        let fx = VideoEffect::new(GAUSSIAN_BLUR)
            .with("h_range", 9.0)
            .with("v_range", f32::NAN);
        assert_eq!(
            fx.resolve(),
            Some(Resolved::GaussianBlur {
                h_range: 0.05,
                v_range: 0.005
            })
        );
        let vg = VideoEffect::new(VIGNETTE)
            .with("inner_radius", 1.0)
            .with("outer_radius", 0.2);
        let Some(Resolved::Vignette { outer, inner, .. }) = vg.resolve() else {
            panic!("vignette");
        };
        assert!(outer > inner, "a reversed pair becomes a hard edge");
        assert!(VideoEffect::new("nope").resolve().is_none());
    }

    #[test]
    fn the_blur_keeps_the_average_and_softens_edges() {
        let original = smpte_color_bars(160, 90);
        let mut frame = original.clone();
        apply_chain_cpu(
            &mut frame,
            &[VideoEffect::new(GAUSSIAN_BLUR)
                .with("h_range", 0.04)
                .with("v_range", 0.0)],
        );
        let (a, b) = (mean(&original), mean(&frame));
        for c in 0..3 {
            assert!(
                (a[c] - b[c]).abs() < 2.0,
                "channel {c}: {} vs {}",
                a[c],
                b[c]
            );
        }
        assert_ne!(frame, original);
        // A hard vertical edge of the bars is now a ramp: neighbours differ less than before.
        let step = |f: &FrameBuffer| {
            (0..f.width - 1)
                .map(|x| {
                    let (p, q) = (f.pixel(x, 10), f.pixel(x + 1, 10));
                    (0..3)
                        .map(|c| i32::from(p[c]).abs_diff(i32::from(q[c])))
                        .sum::<u32>()
                })
                .max()
                .unwrap()
        };
        assert!(step(&frame) < step(&original));
    }

    #[test]
    fn the_blur_is_the_same_picture_at_any_resolution() {
        // The same range is the same share of the frame: a half-size frame blurs half as many pixels.
        let big = smpte_color_bars(320, 180);
        let small = smpte_color_bars(160, 90);
        let chain = [VideoEffect::new(GAUSSIAN_BLUR).with("h_range", 0.03)];
        let (mut a, mut b) = (big.clone(), small.clone());
        apply_chain_cpu(&mut a, &chain);
        apply_chain_cpu(&mut b, &chain);
        // Compare the first row sampled at the same relative positions.
        let mut worst = 0u32;
        for x in 0..160u32 {
            let p = a.pixel(x * 2, 20);
            let q = b.pixel(x, 10);
            for c in 0..3 {
                worst = worst.max(u32::from(p[c]).abs_diff(u32::from(q[c])));
            }
        }
        assert!(worst < 40, "worst difference {worst}");
    }

    #[test]
    fn blur_does_not_leave_dark_fringes_at_a_transparent_edge() {
        // A white block on a fully transparent background: premultiplied blur keeps the colour white.
        let mut frame = FrameBuffer::solid(64, 64, [255, 255, 255, 0]);
        for y in 20..44 {
            for x in 20..44 {
                let i = (y * 64 + x) * 4;
                frame.rgba[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
            }
        }
        apply_chain_cpu(
            &mut frame,
            &[VideoEffect::new(GAUSSIAN_BLUR)
                .with("h_range", 0.05)
                .with("v_range", 0.05)],
        );
        for p in frame.rgba.as_chunks::<4>().0.iter() {
            if p[3] > 20 {
                assert!(p[0] > 240 && p[1] > 240 && p[2] > 240, "fringe {p:?}");
            }
        }
    }

    #[test]
    fn sharpen_and_unsharp_raise_contrast_at_an_edge_and_leave_flat_areas() {
        // Tall enough for the unsharp radius (a share of the height) to be over a pixel
        let mut frame = FrameBuffer::solid(160, 200, [100, 100, 100, 255]);
        for y in 0..200 {
            for x in 80..160 {
                let i = (y * 160 + x) * 4;
                frame.rgba[i..i + 3].copy_from_slice(&[160, 160, 160]);
            }
        }
        for chain in [
            vec![VideoEffect::new(SHARPEN)],
            vec![VideoEffect::new(UNSHARP_MASK).with("ratio", 2.0)],
        ] {
            let mut out = frame.clone();
            apply_chain_cpu(&mut out, &chain);
            assert_eq!(out.pixel(5, 100), frame.pixel(5, 100), "flat dark area");
            assert_eq!(
                out.pixel(150, 100),
                frame.pixel(150, 100),
                "flat light area"
            );
            assert!(out.pixel(79, 100)[0] < 100, "undershoot on the dark side");
            assert!(out.pixel(80, 100)[0] > 160, "overshoot on the light side");
        }
    }

    #[test]
    fn the_vignette_darkens_the_corners_and_keeps_the_centre() {
        let mut frame = FrameBuffer::solid(160, 90, [200, 200, 200, 255]);
        apply_chain_cpu(&mut frame, &[VideoEffect::new(VIGNETTE)]);
        assert_eq!(
            frame.pixel(80, 45),
            [200, 200, 200, 255],
            "centre untouched"
        );
        assert!(frame.pixel(0, 0)[0] < 20, "corner is dark");
        assert!(frame.pixel(159, 89)[0] < 20);
        assert_eq!(frame.pixel(0, 0)[3], 255, "alpha is left alone");
        // Moving the centre to the left edge keeps the left edge bright
        let mut off = FrameBuffer::solid(160, 90, [200, 200, 200, 255]);
        apply_chain_cpu(
            &mut off,
            &[VideoEffect::new(VIGNETTE)
                .with("center_x", 0.0)
                .with("center_y", 0.5)],
        );
        assert!(off.pixel(0, 45)[0] > 190);
        assert!(off.pixel(159, 45)[0] < 40);
    }

    #[test]
    fn the_chain_order_matters_and_the_cache_key_follows_every_value() {
        let a = [
            VideoEffect::new(VIGNETTE),
            VideoEffect::new(GAUSSIAN_BLUR).with("h_range", 0.02),
        ];
        let b = [a[1].clone(), a[0].clone()];
        assert_ne!(hash(&a), hash(&b));
        let mut c = a.clone();
        c[1].params.insert("h_range".into(), 0.021);
        assert_ne!(hash(&a), hash(&c), "one changed value changes the key");
        assert_eq!(hash(&a), hash(&a.clone()));
        assert_ne!(hash(&[]), hash(&a));
    }

    #[test]
    fn the_chain_is_capped() {
        let chain: Vec<VideoEffect> = (0..40).map(|_| VideoEffect::new(SHARPEN)).collect();
        assert_eq!(resolve_chain(&chain).len(), MAX_CHAIN);
    }
}
