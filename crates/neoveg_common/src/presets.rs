//! Project format presets: frame sizes, frame rates, pixel aspect ratios, field orders, pixel
//! formats, sample rates and the ready-made templates built from them.
//!
//! The lists follow what the reference manuals describe for the Project Properties dialog and for
//! the new-project screen (template, width and height, field order, pixel aspect ratio, frame
//! rate, pixel format, output rotation, HDR mode, rendering quality, motion blur, deinterlacing,
//! resample mode, audio sample rate and bit depth). The names and the groupings are Neoveg's own.

use serde::{Deserialize, Serialize};

use crate::project::ProjectSettings;

/// A named frame size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolution {
    pub name: &'static str,
    pub width: u32,
    pub height: u32,
    /// Where the size is used, to group the list in a menu.
    pub group: &'static str,
}

/// Every frame size offered, from small web sizes to 8K, plus film, social and television sizes.
pub const RESOLUTIONS: &[Resolution] = &[
    Resolution {
        name: "360p",
        width: 640,
        height: 360,
        group: "Web",
    },
    Resolution {
        name: "480p (largo)",
        width: 854,
        height: 480,
        group: "Web",
    },
    Resolution {
        name: "540p (qHD)",
        width: 960,
        height: 540,
        group: "Web",
    },
    Resolution {
        name: "720p (HD)",
        width: 1280,
        height: 720,
        group: "HD",
    },
    Resolution {
        name: "900p (HD+)",
        width: 1600,
        height: 900,
        group: "HD",
    },
    Resolution {
        name: "1080p (Full HD)",
        width: 1920,
        height: 1080,
        group: "HD",
    },
    Resolution {
        name: "1440p (QHD)",
        width: 2560,
        height: 1440,
        group: "HD",
    },
    Resolution {
        name: "2160p (4K UHD)",
        width: 3840,
        height: 2160,
        group: "Ultra HD",
    },
    Resolution {
        name: "2880p (5K)",
        width: 5120,
        height: 2880,
        group: "Ultra HD",
    },
    Resolution {
        name: "3456p (6K)",
        width: 6144,
        height: 3456,
        group: "Ultra HD",
    },
    Resolution {
        name: "4320p (8K UHD)",
        width: 7680,
        height: 4320,
        group: "Ultra HD",
    },
    Resolution {
        name: "NTSC SD",
        width: 720,
        height: 480,
        group: "Televisão",
    },
    Resolution {
        name: "PAL SD",
        width: 720,
        height: 576,
        group: "Televisão",
    },
    Resolution {
        name: "HDV / anamórfico 1440x1080",
        width: 1440,
        height: 1080,
        group: "Televisão",
    },
    Resolution {
        name: "HD anamórfico 1280x1080",
        width: 1280,
        height: 1080,
        group: "Televisão",
    },
    Resolution {
        name: "2K DCI",
        width: 2048,
        height: 1080,
        group: "Cinema",
    },
    Resolution {
        name: "2K DCI Scope",
        width: 2048,
        height: 858,
        group: "Cinema",
    },
    Resolution {
        name: "4K DCI",
        width: 4096,
        height: 2160,
        group: "Cinema",
    },
    Resolution {
        name: "4K DCI Scope",
        width: 4096,
        height: 1716,
        group: "Cinema",
    },
    Resolution {
        name: "8K DCI",
        width: 8192,
        height: 4320,
        group: "Cinema",
    },
    Resolution {
        name: "Ultralargo 21:9 (2560x1080)",
        width: 2560,
        height: 1080,
        group: "Cinema",
    },
    Resolution {
        name: "Ultralargo 21:9 (3440x1440)",
        width: 3440,
        height: 1440,
        group: "Cinema",
    },
    Resolution {
        name: "Vertical 720x1280",
        width: 720,
        height: 1280,
        group: "Redes sociais",
    },
    Resolution {
        name: "Vertical 1080x1920 (9:16)",
        width: 1080,
        height: 1920,
        group: "Redes sociais",
    },
    Resolution {
        name: "Vertical 2160x3840",
        width: 2160,
        height: 3840,
        group: "Redes sociais",
    },
    Resolution {
        name: "Quadrado 1080x1080 (1:1)",
        width: 1080,
        height: 1080,
        group: "Redes sociais",
    },
    Resolution {
        name: "Retrato 1080x1350 (4:5)",
        width: 1080,
        height: 1350,
        group: "Redes sociais",
    },
];

/// A named frame rate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameRate {
    pub label: &'static str,
    pub fps: f64,
}

pub const FRAME_RATES: &[FrameRate] = &[
    FrameRate {
        label: "15",
        fps: 15.0,
    },
    FrameRate {
        label: "23,976 (cinema NTSC)",
        fps: 24_000.0 / 1001.0,
    },
    FrameRate {
        label: "24 (cinema)",
        fps: 24.0,
    },
    FrameRate {
        label: "25 (PAL)",
        fps: 25.0,
    },
    FrameRate {
        label: "29,97 (NTSC)",
        fps: 30_000.0 / 1001.0,
    },
    FrameRate {
        label: "30",
        fps: 30.0,
    },
    FrameRate {
        label: "47,952",
        fps: 48_000.0 / 1001.0,
    },
    FrameRate {
        label: "48",
        fps: 48.0,
    },
    FrameRate {
        label: "50 (PAL)",
        fps: 50.0,
    },
    FrameRate {
        label: "59,94 (NTSC)",
        fps: 60_000.0 / 1001.0,
    },
    FrameRate {
        label: "60",
        fps: 60.0,
    },
    FrameRate {
        label: "72",
        fps: 72.0,
    },
    FrameRate {
        label: "90",
        fps: 90.0,
    },
    FrameRate {
        label: "100",
        fps: 100.0,
    },
    FrameRate {
        label: "119,88",
        fps: 120_000.0 / 1001.0,
    },
    FrameRate {
        label: "120",
        fps: 120.0,
    },
    FrameRate {
        label: "240",
        fps: 240.0,
    },
];

/// Pixel aspect ratios: computers show square pixels, television formats do not.
pub const PIXEL_ASPECTS: &[(&str, f64)] = &[
    ("1,0000 (quadrado)", 1.0),
    ("0,9091 (NTSC DV 4:3)", 10.0 / 11.0),
    ("1,2121 (NTSC DV 16:9)", 40.0 / 33.0),
    ("1,0667 (PAL DV 4:3)", 16.0 / 15.0),
    ("1,4587 (PAL DV 16:9)", 1.4587),
    ("1,3333 (HDV, HD anamórfico)", 4.0 / 3.0),
    ("1,5000", 1.5),
    ("2,0000 (anamórfico 2x)", 2.0),
];

/// Sample rates of the project audio.
pub const SAMPLE_RATES: &[u32] = &[
    8_000, 11_025, 16_000, 22_050, 32_000, 44_100, 48_000, 88_200, 96_000, 176_400, 192_000,
];

/// Bit depths of the project audio (`32` is floating point).
pub const AUDIO_BIT_DEPTHS: &[u32] = &[8, 16, 24, 32];

/// Smallest and largest frame size accepted, per side.
pub const SIZE_RANGE: (u32, u32) = (16, 16_384);

/// Order of the two fields of an interlaced frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldOrder {
    /// No fields (progressive scan): the right choice for a computer screen.
    #[default]
    Progressive,
    UpperFirst,
    LowerFirst,
}

impl FieldOrder {
    pub const ALL: [FieldOrder; 3] = [
        FieldOrder::Progressive,
        FieldOrder::UpperFirst,
        FieldOrder::LowerFirst,
    ];

    pub fn label(self) -> &'static str {
        match self {
            FieldOrder::Progressive => "Nenhuma (varredura progressiva)",
            FieldOrder::UpperFirst => "Campo superior primeiro",
            FieldOrder::LowerFirst => "Campo inferior primeiro",
        }
    }
}

/// Arithmetic and colour range used to process the video.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PixelFormat {
    /// 8 bits per channel in the studio range (16-235).
    Legacy8Video,
    /// 8 bits per channel in the full range (0-255).
    #[default]
    Bit8Full,
    /// 32-bit floating point in the studio range.
    Float32Video,
    /// 32-bit floating point in the full range.
    Float32Full,
}

impl PixelFormat {
    pub const ALL: [PixelFormat; 4] = [
        PixelFormat::Legacy8Video,
        PixelFormat::Bit8Full,
        PixelFormat::Float32Video,
        PixelFormat::Float32Full,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PixelFormat::Legacy8Video => "8 bits (níveis de vídeo, legado)",
            PixelFormat::Bit8Full => "8 bits (faixa completa)",
            PixelFormat::Float32Video => "32 bits em ponto flutuante (níveis de vídeo)",
            PixelFormat::Float32Full => "32 bits em ponto flutuante (faixa completa)",
        }
    }

    /// Whether the compositing gamma can be chosen (only for 32-bit full range).
    pub fn has_gamma_choice(self) -> bool {
        self == PixelFormat::Float32Full
    }
}

/// Gamma in which layers are blended (32-bit full range only; 8-bit always uses 2.222).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositingGamma {
    Linear,
    #[default]
    Video,
}

impl CompositingGamma {
    pub const ALL: [CompositingGamma; 2] = [CompositingGamma::Linear, CompositingGamma::Video];

    pub fn label(self) -> &'static str {
        match self {
            CompositingGamma::Linear => "1,000 (linear)",
            CompositingGamma::Video => "2,222 (vídeo)",
        }
    }

    pub fn value(self) -> f64 {
        match self {
            CompositingGamma::Linear => 1.0,
            CompositingGamma::Video => 2.222,
        }
    }
}

/// Rotation of the whole output (portrait or upside-down projects).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputRotation {
    #[default]
    None,
    Clockwise90,
    CounterClockwise90,
    Upside180,
}

impl OutputRotation {
    pub const ALL: [OutputRotation; 4] = [
        OutputRotation::None,
        OutputRotation::Clockwise90,
        OutputRotation::CounterClockwise90,
        OutputRotation::Upside180,
    ];

    pub fn label(self) -> &'static str {
        match self {
            OutputRotation::None => "Nenhuma",
            OutputRotation::Clockwise90 => "90° no sentido horário",
            OutputRotation::CounterClockwise90 => "90° no sentido anti-horário",
            OutputRotation::Upside180 => "180°",
        }
    }
}

/// HDR mode of the project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HdrMode {
    #[default]
    Off,
    Hdr10,
    Hlg,
}

impl HdrMode {
    pub const ALL: [HdrMode; 3] = [HdrMode::Off, HdrMode::Hdr10, HdrMode::Hlg];

    pub fn label(self) -> &'static str {
        match self {
            HdrMode::Off => "Desligado",
            HdrMode::Hdr10 => "HDR10",
            HdrMode::Hlg => "HLG",
        }
    }
}

/// Quality of the final render: how the frames are scaled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderQuality {
    Draft,
    Preview,
    #[default]
    Good,
    Best,
}

impl RenderQuality {
    pub const ALL: [RenderQuality; 4] = [
        RenderQuality::Draft,
        RenderQuality::Preview,
        RenderQuality::Good,
        RenderQuality::Best,
    ];

    pub fn label(self) -> &'static str {
        match self {
            RenderQuality::Draft => "Rascunho (Draft)",
            RenderQuality::Preview => "Prévia (Preview)",
            RenderQuality::Good => "Boa (bilinear)",
            RenderQuality::Best => "Melhor (bicúbica com integração)",
        }
    }
}

/// Curve that weights the frames of a motion blur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MotionBlurType {
    #[default]
    Gaussian,
    Pyramid,
    Box,
    GaussianAsymmetric,
    PyramidAsymmetric,
    BoxAsymmetric,
}

impl MotionBlurType {
    pub const ALL: [MotionBlurType; 6] = [
        MotionBlurType::Gaussian,
        MotionBlurType::Pyramid,
        MotionBlurType::Box,
        MotionBlurType::GaussianAsymmetric,
        MotionBlurType::PyramidAsymmetric,
        MotionBlurType::BoxAsymmetric,
    ];

    pub fn label(self) -> &'static str {
        match self {
            MotionBlurType::Gaussian => "Gaussiana",
            MotionBlurType::Pyramid => "Pirâmide",
            MotionBlurType::Box => "Caixa (média igual)",
            MotionBlurType::GaussianAsymmetric => "Gaussiana (assimétrica)",
            MotionBlurType::PyramidAsymmetric => "Pirâmide (assimétrica)",
            MotionBlurType::BoxAsymmetric => "Caixa (assimétrica)",
        }
    }
}

/// How the two fields of interlaced footage are merged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Deinterlace {
    None,
    #[default]
    BlendFields,
    Interpolate,
    SmartAdaptive,
}

impl Deinterlace {
    pub const ALL: [Deinterlace; 4] = [
        Deinterlace::None,
        Deinterlace::BlendFields,
        Deinterlace::Interpolate,
        Deinterlace::SmartAdaptive,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Deinterlace::None => "Nenhum",
            Deinterlace::BlendFields => "Misturar campos",
            Deinterlace::Interpolate => "Interpolar",
            Deinterlace::SmartAdaptive => "Adaptativo inteligente (GPU)",
        }
    }
}

/// How frames are resampled when the media has fewer frames per second than the project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResampleMode {
    /// Resamples only when the rates differ.
    #[default]
    Smart,
    Force,
    Disable,
}

impl ResampleMode {
    pub const ALL: [ResampleMode; 3] = [
        ResampleMode::Smart,
        ResampleMode::Force,
        ResampleMode::Disable,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ResampleMode::Smart => "Inteligente (Smart)",
            ResampleMode::Force => "Forçar",
            ResampleMode::Disable => "Desativar",
        }
    }
}

/// Channel layout of the master bus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MasterBusMode {
    #[default]
    Stereo,
    Surround51,
}

impl MasterBusMode {
    pub const ALL: [MasterBusMode; 2] = [MasterBusMode::Stereo, MasterBusMode::Surround51];

    pub fn label(self) -> &'static str {
        match self {
            MasterBusMode::Stereo => "Estéreo",
            MasterBusMode::Surround51 => "Surround 5.1",
        }
    }
}

/// A ready-made set of project settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectTemplate {
    pub name: String,
    pub settings: ProjectSettings,
}

fn template(
    name: &str,
    (width, height): (u32, u32),
    fps: f64,
    pixel_aspect: f64,
    field_order: FieldOrder,
) -> ProjectTemplate {
    ProjectTemplate {
        name: name.to_owned(),
        settings: ProjectSettings {
            width,
            height,
            fps,
            pixel_aspect,
            field_order,
            ..ProjectSettings::default()
        },
    }
}

/// The templates that come with Neoveg, from standard definition to 8K.
pub fn builtin_templates() -> Vec<ProjectTemplate> {
    use FieldOrder::{LowerFirst, Progressive, UpperFirst};
    let (ntsc, pal) = (30_000.0 / 1001.0, 25.0);
    vec![
        template(
            "NTSC SD 4:3 (720x480, 29,97 entrelaçado)",
            (720, 480),
            ntsc,
            10.0 / 11.0,
            LowerFirst,
        ),
        template(
            "NTSC SD 16:9 (720x480, 29,97 entrelaçado)",
            (720, 480),
            ntsc,
            40.0 / 33.0,
            LowerFirst,
        ),
        template(
            "PAL SD 4:3 (720x576, 25 entrelaçado)",
            (720, 576),
            pal,
            16.0 / 15.0,
            UpperFirst,
        ),
        template(
            "PAL SD 16:9 (720x576, 25 entrelaçado)",
            (720, 576),
            pal,
            1.4587,
            UpperFirst,
        ),
        template(
            "HDV 1080i (1440x1080, 29,97)",
            (1440, 1080),
            ntsc,
            4.0 / 3.0,
            UpperFirst,
        ),
        template("HD 720p 24", (1280, 720), 24.0, 1.0, Progressive),
        template("HD 720p 25", (1280, 720), pal, 1.0, Progressive),
        template("HD 720p 29,97", (1280, 720), ntsc, 1.0, Progressive),
        template(
            "HD 720p 59,94",
            (1280, 720),
            60_000.0 / 1001.0,
            1.0,
            Progressive,
        ),
        template(
            "Full HD 1080p 23,976",
            (1920, 1080),
            24_000.0 / 1001.0,
            1.0,
            Progressive,
        ),
        template("Full HD 1080p 24", (1920, 1080), 24.0, 1.0, Progressive),
        template("Full HD 1080p 25", (1920, 1080), pal, 1.0, Progressive),
        template("Full HD 1080p 29,97", (1920, 1080), ntsc, 1.0, Progressive),
        template("Full HD 1080p 30", (1920, 1080), 30.0, 1.0, Progressive),
        template("Full HD 1080p 50", (1920, 1080), 50.0, 1.0, Progressive),
        template(
            "Full HD 1080p 59,94",
            (1920, 1080),
            60_000.0 / 1001.0,
            1.0,
            Progressive,
        ),
        template(
            "Full HD 1080i 29,97 (entrelaçado)",
            (1920, 1080),
            ntsc,
            1.0,
            UpperFirst,
        ),
        template("2K DCI 24", (2048, 1080), 24.0, 1.0, Progressive),
        template("QHD 1440p 30", (2560, 1440), 30.0, 1.0, Progressive),
        template(
            "4K UHD 23,976",
            (3840, 2160),
            24_000.0 / 1001.0,
            1.0,
            Progressive,
        ),
        template("4K UHD 25", (3840, 2160), pal, 1.0, Progressive),
        template("4K UHD 29,97", (3840, 2160), ntsc, 1.0, Progressive),
        template(
            "4K UHD 59,94",
            (3840, 2160),
            60_000.0 / 1001.0,
            1.0,
            Progressive,
        ),
        template("4K DCI 24", (4096, 2160), 24.0, 1.0, Progressive),
        template("8K UHD 24", (7680, 4320), 24.0, 1.0, Progressive),
        template("8K UHD 29,97", (7680, 4320), ntsc, 1.0, Progressive),
        template(
            "Vertical 1080x1920 30 (redes sociais)",
            (1080, 1920),
            30.0,
            1.0,
            Progressive,
        ),
        template(
            "Quadrado 1080x1080 30 (redes sociais)",
            (1080, 1080),
            30.0,
            1.0,
            Progressive,
        ),
    ]
}

/// The list entry whose size is `width` x `height`, if any.
pub fn resolution_named(width: u32, height: u32) -> Option<&'static Resolution> {
    RESOLUTIONS
        .iter()
        .find(|r| r.width == width && r.height == height)
}

/// The list entry closest to `fps` (within 0.005), if any.
pub fn frame_rate_of(fps: f64) -> Option<&'static FrameRate> {
    FRAME_RATES.iter().find(|r| (r.fps - fps).abs() < 0.005)
}

/// Display aspect ratio of a frame: width over height, stretched by the pixel aspect ratio.
pub fn display_aspect(width: u32, height: u32, pixel_aspect: f64) -> f64 {
    if height == 0 {
        return 0.0;
    }
    f64::from(width) / f64::from(height) * pixel_aspect
}

/// Keeps a frame size inside [`SIZE_RANGE`] and even, as the encoders need.
pub fn clamp_size(value: u32) -> u32 {
    value.clamp(SIZE_RANGE.0, SIZE_RANGE.1) & !1
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn resolutions_cover_the_range_from_360p_to_8k_without_repeats() {
        assert!(RESOLUTIONS
            .iter()
            .any(|r| (r.width, r.height) == (640, 360)));
        assert!(RESOLUTIONS
            .iter()
            .any(|r| (r.width, r.height) == (7680, 4320)));
        let sizes: HashSet<_> = RESOLUTIONS.iter().map(|r| (r.width, r.height)).collect();
        assert_eq!(sizes.len(), RESOLUTIONS.len(), "a size is listed twice");
        let names: HashSet<_> = RESOLUTIONS.iter().map(|r| r.name).collect();
        assert_eq!(names.len(), RESOLUTIONS.len());
        for r in RESOLUTIONS {
            assert!(
                r.width >= SIZE_RANGE.0 && r.width <= SIZE_RANGE.1,
                "{}",
                r.name
            );
            assert!(
                r.height >= SIZE_RANGE.0 && r.height <= SIZE_RANGE.1,
                "{}",
                r.name
            );
            assert!(r.width % 2 == 0 && r.height % 2 == 0, "{} is odd", r.name);
        }
    }

    #[test]
    fn frame_rates_are_sorted_and_include_the_broadcast_standards() {
        let rates: Vec<f64> = FRAME_RATES.iter().map(|r| r.fps).collect();
        assert!(rates.windows(2).all(|w| w[0] < w[1]), "not sorted");
        for wanted in [24.0, 25.0, 30.0, 50.0, 60.0] {
            assert!(frame_rate_of(wanted).is_some(), "{wanted}");
        }
        assert!((frame_rate_of(29.97).unwrap().fps - 30_000.0 / 1001.0).abs() < 1e-9);
        assert!(frame_rate_of(31.0).is_none());
    }

    #[test]
    fn templates_have_unique_names_and_valid_settings() {
        let templates = builtin_templates();
        assert!(templates.len() >= 25);
        let names: HashSet<_> = templates.iter().map(|t| t.name.clone()).collect();
        assert_eq!(names.len(), templates.len());
        for t in &templates {
            let s = &t.settings;
            assert_eq!(clamp_size(s.width), s.width, "{}", t.name);
            assert_eq!(clamp_size(s.height), s.height, "{}", t.name);
            assert!(s.fps > 1.0 && s.fps <= 240.0, "{}", t.name);
            assert!(s.pixel_aspect > 0.5 && s.pixel_aspect <= 2.0, "{}", t.name);
        }
        // Interlaced templates keep their field order, progressive ones have none
        assert!(templates
            .iter()
            .any(|t| t.settings.field_order == FieldOrder::UpperFirst));
        assert!(templates
            .iter()
            .any(|t| t.name.starts_with("Full HD 1080p 25")
                && t.settings.field_order == FieldOrder::Progressive));
    }

    #[test]
    fn display_aspect_stretches_by_the_pixel_aspect() {
        assert!((display_aspect(1920, 1080, 1.0) - 16.0 / 9.0).abs() < 1e-9);
        // NTSC DV widescreen: the 704x480 clean aperture with 40/33 pixels is 16:9
        assert!((display_aspect(704, 480, 40.0 / 33.0) - 16.0 / 9.0).abs() < 1e-3);
        assert_eq!(display_aspect(10, 0, 1.0), 0.0);
    }

    #[test]
    fn sizes_are_clamped_and_made_even() {
        assert_eq!(clamp_size(1), 16);
        assert_eq!(clamp_size(1921), 1920);
        assert_eq!(clamp_size(100_000), 16_384);
    }

    #[test]
    fn every_option_has_a_label_and_round_trips_through_json() {
        fn round<T: Serialize + for<'a> Deserialize<'a> + PartialEq + std::fmt::Debug + Copy>(
            all: &[T],
        ) {
            for v in all {
                let text = serde_json::to_string(v).unwrap();
                assert_eq!(&serde_json::from_str::<T>(&text).unwrap(), v);
            }
        }
        round(&FieldOrder::ALL);
        round(&PixelFormat::ALL);
        round(&CompositingGamma::ALL);
        round(&OutputRotation::ALL);
        round(&HdrMode::ALL);
        round(&RenderQuality::ALL);
        round(&MotionBlurType::ALL);
        round(&Deinterlace::ALL);
        round(&ResampleMode::ALL);
        round(&MasterBusMode::ALL);
        assert!(FieldOrder::ALL.iter().all(|v| !v.label().is_empty()));
        assert!(PixelFormat::ALL.iter().all(|v| !v.label().is_empty()));
        assert!(MotionBlurType::ALL.iter().all(|v| !v.label().is_empty()));
        assert!(
            PixelFormat::Float32Full.has_gamma_choice()
                && !PixelFormat::Bit8Full.has_gamma_choice()
        );
        assert!(SAMPLE_RATES.contains(&48_000) && AUDIO_BIT_DEPTHS.contains(&24));
    }
}
