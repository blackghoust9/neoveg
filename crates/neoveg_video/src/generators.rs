//! Procedural media generators: Solid Color, Test Patterns (SMPTE bars, checkerboard, crosshair
//! grid), Linear/Radial Gradients, and Titles & Text (with outline, drop shadow, Portuguese Latin-1
//! glyphs, and animation presets), persisted as `.neoveggen` JSON manifests.

use neoveg_common::i18n::tr;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::frame::FrameBuffer;
use crate::info::{fit_size, VideoInfo};
use neoveg_text::title_paint::{scroll_position, ScrollDirection};

/// File extension for procedural media generator manifests in the media pool.
pub const GENERATOR_EXTENSION: &str = "neoveggen";

pub fn is_generator_manifest_path(path: &Path) -> bool {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .is_some_and(|e| e == GENERATOR_EXTENSION)
}

/// 75% SMPTE-style color bars: white, yellow, cyan, green, magenta, red, blue.
const BARS: [[u8; 3]; 7] = [
    [191, 191, 191],
    [191, 191, 0],
    [0, 191, 191],
    [0, 191, 0],
    [191, 0, 191],
    [191, 0, 0],
    [0, 0, 191],
];

pub fn smpte_color_bars(width: u32, height: u32) -> FrameBuffer {
    let mut frame = FrameBuffer::solid(width, height, [0, 0, 0, 255]);
    for y in 0..height {
        for x in 0..width {
            let bar = ((x as u64 * 7) / width.max(1) as u64) as usize;
            let [r, g, b] = BARS[bar.min(6)];
            let i = (y as usize * width as usize + x as usize) * 4;
            frame.rgba[i..i + 4].copy_from_slice(&[r, g, b, 255]);
        }
    }
    frame
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestPatternVariant {
    #[default]
    SmpteBars,
    Checkerboard,
    CrosshairGrid,
}

impl TestPatternVariant {
    pub const ALL: [Self; 3] = [Self::SmpteBars, Self::Checkerboard, Self::CrosshairGrid];

    pub fn label(self) -> &'static str {
        match self {
            Self::SmpteBars => tr("Barras SMPTE (75%)"),
            Self::Checkerboard => tr("Xadrez de transparência"),
            Self::CrosshairGrid => tr("Grade de alinhamento e mira"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GradientShape {
    #[default]
    Linear,
    Radial,
}

impl GradientShape {
    pub const ALL: [Self; 2] = [Self::Linear, Self::Radial];

    pub fn label(self) -> &'static str {
        match self {
            Self::Linear => tr("Linear"),
            Self::Radial => tr("Radial"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TitleAnimation {
    #[default]
    Static,
    FadeIn,
    Typewriter,
    SlideUp,
    CreditRoll,
    /// Ticker: crosses the frame from the right edge to the left one.
    ScrollLeft,
    /// Crosses the frame from the left edge to the right one.
    ScrollRight,
}

impl TitleAnimation {
    pub const ALL: [Self; 7] = [
        Self::Static,
        Self::FadeIn,
        Self::Typewriter,
        Self::SlideUp,
        Self::CreditRoll,
        Self::ScrollLeft,
        Self::ScrollRight,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Static => tr("Estático"),
            Self::FadeIn => tr("Surgir suave (Fade-In)"),
            Self::Typewriter => tr("Máquina de escrever"),
            Self::SlideUp => tr("Subir suave (Lower-Third)"),
            Self::CreditRoll => tr("Rolagem de créditos"),
            Self::ScrollLeft => tr("Rolagem horizontal (direita para esquerda)"),
            Self::ScrollRight => tr("Rolagem horizontal (esquerda para direita)"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GeneratorKind {
    SolidColor {
        rgba: [u8; 4],
    },
    TestPattern {
        variant: TestPatternVariant,
    },
    Gradient {
        shape: GradientShape,
        start_rgba: [u8; 4],
        end_rgba: [u8; 4],
        angle_deg: f32,
    },
    /// A texture of random pixels: grey noise between two colours, or a random colour in every
    /// cell. It depends only on the cell, the frame and the seed, so it is the same on every
    /// machine and at every preview size.
    Noise {
        /// Every channel at random (the two colours are not used).
        colored: bool,
        /// The colour of the darkest cells and of the lightest ones.
        dark_rgba: [u8; 4],
        light_rgba: [u8; 4],
        /// Side of a cell of noise, in pixels of the generator's own size.
        grain: u32,
        seed: u32,
        /// A new noise at every frame (film grain) instead of one that stays.
        animated: bool,
    },
    /// A leader countdown: a big number that goes from `from` down to 1, each number lasting an
    /// equal share of the duration, with a ring, a cross and a sweeping hand that turns once per
    /// number.
    Countdown {
        from: u32,
        text_rgba: [u8; 4],
        bg_rgba: [u8; 4],
        line_rgba: [u8; 4],
    },
    TitleText {
        text: String,
        scale: u32,
        text_rgba: [u8; 4],
        bg_rgba: [u8; 4],
        outline_rgba: [u8; 4],
        outline_px: u32,
        shadow_rgba: [u8; 4],
        shadow_offset: [i32; 2],
        /// Normalized vertical anchor (`0.5` = centered, `0.82` = lower-third).
        align_y: f32,
        animation: TitleAnimation,
        /// Font family and style: the bundled Inter, JetBrains Mono and Noto Serif, or a font
        /// installed on the system.
        #[serde(default = "default_title_font")]
        font: String,
        #[serde(default = "default_title_style")]
        font_style: String,
        /// How the paragraphs are set: alignment, a fixed width and the styles of stretches of the
        /// text. Left out of the manifest while it is the plain centred title.
        #[serde(default, skip_serializing_if = "TitleLayout::is_default")]
        layout: TitleLayout,
    },
}

/// Alignment of the lines of a title.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TitleAlign {
    Left,
    #[default]
    Center,
    Right,
    /// Both edges straight; needs a fixed width, or the lines have no edge to fill.
    Justify,
}

impl TitleAlign {
    pub const ALL: [Self; 4] = [Self::Left, Self::Center, Self::Right, Self::Justify];

    pub fn label(self) -> &'static str {
        match self {
            Self::Left => tr("À esquerda"),
            Self::Center => tr("Centralizado"),
            Self::Right => tr("À direita"),
            Self::Justify => tr("Justificado"),
        }
    }

    fn text_align(self) -> neoveg_text::layout::Align {
        use neoveg_text::layout::Align;
        match self {
            Self::Left => Align::Left,
            Self::Center => Align::Center,
            Self::Right => Align::Right,
            Self::Justify => Align::Justify,
        }
    }
}

/// A stretch of the text of a title with a style of its own (characters `start..end`, counted in
/// characters of the text).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TitleSpan {
    pub start: usize,
    pub end: usize,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub underline: bool,
    /// Its own colour; the colour of the title when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 4]>,
    /// Size of the stretch against the size of the title.
    #[serde(default = "unit_scale", skip_serializing_if = "is_unit_scale")]
    pub scale: f32,
}

fn unit_scale() -> f32 {
    1.0
}

fn is_unit_scale(v: &f32) -> bool {
    (*v - 1.0).abs() < 1e-6
}

impl TitleSpan {
    pub fn new(start: usize, end: usize) -> Self {
        Self {
            start,
            end,
            bold: false,
            italic: false,
            underline: false,
            color: None,
            scale: 1.0,
        }
    }
}

/// Paragraph setting of a title (`TitleText::layout`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TitleLayout {
    #[serde(default)]
    pub align: TitleAlign,
    /// Width of the paragraphs as a fraction of the frame width (`0` sets the lines free, each as
    /// long as it is).
    #[serde(default)]
    pub box_width: f32,
    #[serde(default)]
    pub spans: Vec<TitleSpan>,
    /// Several fills, outlines and shadows over the text. While empty, the title uses its single
    /// fill, outline and shadow.
    #[serde(
        default,
        skip_serializing_if = "neoveg_text::title_paint::PaintStack::is_empty"
    )]
    pub stack: neoveg_text::title_paint::PaintStack,
}

impl TitleLayout {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// The width of the box in pixels for a frame `frame_width` wide, or `None` for free lines.
    pub fn box_pixels(&self, frame_width: u32) -> Option<f32> {
        (self.box_width.is_finite() && self.box_width > 0.0)
            .then(|| frame_width as f32 * self.box_width.clamp(0.1, 1.0))
    }
}

/// The style runs of the visible part of a title: one for each span that falls on the text, in the
/// style of the title changed as the span says, with the colour the span asks for. Characters are
/// counted as `char`s, so a span stays on the same letters whatever they weigh in bytes.
pub fn title_runs(
    visible: &str,
    spans: &[TitleSpan],
    base: &neoveg_text::layout::TextStyle,
) -> Vec<(neoveg_text::layout::StyleRun, Option<[u8; 4]>)> {
    let bytes: Vec<usize> = visible
        .char_indices()
        .map(|(i, _)| i)
        .chain(std::iter::once(visible.len()))
        .collect();
    let chars = bytes.len() - 1;
    spans
        .iter()
        .filter_map(|span| {
            let (start, end) = (span.start.min(chars), span.end.min(chars));
            (start < end).then(|| {
                let mut style = base.clone();
                if span.bold {
                    style.style = "Bold".into();
                }
                style.faux_italic |= span.italic;
                style.underline |= span.underline;
                if span.scale.is_finite() && span.scale > 0.0 {
                    style.size = base.size * span.scale.clamp(0.25, 4.0);
                }
                (
                    neoveg_text::layout::StyleRun {
                        range: bytes[start]..bytes[end],
                        style,
                    },
                    span.color,
                )
            })
        })
        .collect()
}

fn default_title_font() -> String {
    "Inter".into()
}

fn default_title_style() -> String {
    "Bold".into()
}

impl GeneratorKind {
    pub fn is_time_varying(&self) -> bool {
        matches!(
            self,
            Self::TitleText {
                animation: TitleAnimation::FadeIn
                    | TitleAnimation::Typewriter
                    | TitleAnimation::SlideUp
                    | TitleAnimation::CreditRoll
                    | TitleAnimation::ScrollLeft
                    | TitleAnimation::ScrollRight,
                ..
            } | Self::Noise { animated: true, .. }
                | Self::Countdown { .. }
        )
    }

    pub fn default_title() -> Self {
        Self::TitleText {
            text: "NEOVEG STUDIO".into(),
            scale: 6,
            text_rgba: [255, 255, 255, 255],
            bg_rgba: [0, 0, 0, 0],
            outline_rgba: [15, 18, 24, 255],
            outline_px: 2,
            shadow_rgba: [0, 0, 0, 180],
            shadow_offset: [4, 4],
            align_y: 0.5,
            animation: TitleAnimation::Static,
            font: default_title_font(),
            font_style: default_title_style(),
            layout: TitleLayout::default(),
        }
    }
}

/// Complete specification of a procedural media generator saved in a `.neoveggen` manifest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeneratorSpec {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration: f64,
    pub kind: GeneratorKind,
}

impl GeneratorSpec {
    pub fn video_info(&self) -> VideoInfo {
        VideoInfo {
            width: self.width.max(2),
            height: self.height.max(2),
            fps: if self.fps.is_finite() && self.fps > 0.0 {
                self.fps
            } else {
                29.97
            },
            duration: if self.duration.is_finite() && self.duration > 0.0 {
                self.duration
            } else {
                5.0
            },
        }
    }

    pub fn save_manifest(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("falha ao criar pasta {}: {e}", parent.display()))?;
            }
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("falha ao serializar gerador: {e}"))?;
        std::fs::write(path, json)
            .map_err(|e| format!("falha ao gravar gerador {}: {e}", path.display()))
    }

    pub fn load_manifest(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("falha ao ler gerador {}: {e}", path.display()))?;
        serde_json::from_str(&text)
            .map_err(|e| format!("gerador de mídia inválido {}: {e}", path.display()))
    }

    /// Renders this generator at `seconds` (`0.0..=duration`) into an RGBA8 `FrameBuffer` scaled
    /// to `max_width` (`0` = native generator dimensions).
    pub fn render(&self, seconds: f64, max_width: u32) -> FrameBuffer {
        let (w, h) = fit_size(self.width.max(2), self.height.max(2), max_width);
        let scale_ratio = w as f32 / self.width.max(2) as f32;
        match &self.kind {
            GeneratorKind::SolidColor { rgba } => FrameBuffer::solid(w, h, *rgba),
            GeneratorKind::TestPattern { variant } => render_test_pattern(w, h, *variant),
            GeneratorKind::Gradient {
                shape,
                start_rgba,
                end_rgba,
                angle_deg,
            } => render_gradient(w, h, *shape, *start_rgba, *end_rgba, *angle_deg),
            GeneratorKind::Noise {
                colored,
                dark_rgba,
                light_rgba,
                grain,
                seed,
                animated,
            } => {
                let frame = if *animated {
                    (seconds.max(0.0) * self.video_info().fps).floor() as u32
                } else {
                    0
                };
                let look = NoiseLook {
                    colored: *colored,
                    dark: *dark_rgba,
                    light: *light_rgba,
                    grain: (*grain).clamp(1, 256),
                    seed: *seed,
                    frame,
                };
                render_noise(w, h, scale_ratio, &look)
            }
            GeneratorKind::Countdown {
                from,
                text_rgba,
                bg_rgba,
                line_rgba,
            } => render_countdown(
                w,
                h,
                *from,
                [*text_rgba, *bg_rgba, *line_rgba],
                seconds,
                self.duration.max(0.1),
            ),
            GeneratorKind::TitleText {
                text,
                scale,
                text_rgba,
                bg_rgba,
                outline_rgba,
                outline_px,
                shadow_rgba,
                shadow_offset,
                align_y,
                animation,
                font,
                font_style,
                layout,
            } => {
                let eff_scale = ((*scale as f32 * scale_ratio).round() as u32).max(1);
                let eff_outline = ((*outline_px as f32 * scale_ratio).round() as u32)
                    .max(u32::from(*outline_px > 0));
                let eff_shadow = [
                    (shadow_offset[0] as f32 * scale_ratio).round() as i32,
                    (shadow_offset[1] as f32 * scale_ratio).round() as i32,
                ];
                render_title_text(
                    w,
                    h,
                    text,
                    eff_scale,
                    (font, font_style),
                    *text_rgba,
                    *bg_rgba,
                    *outline_rgba,
                    eff_outline,
                    *shadow_rgba,
                    eff_shadow,
                    *align_y,
                    *animation,
                    layout,
                    seconds,
                    self.duration.max(0.1),
                )
            }
        }
    }
}

pub fn probe_generator_manifest(path: &Path) -> Result<VideoInfo, String> {
    Ok(GeneratorSpec::load_manifest(path)?.video_info())
}

pub fn decode_generator_at(
    path: &Path,
    seconds: f64,
    max_width: u32,
) -> Result<FrameBuffer, String> {
    let spec = GeneratorSpec::load_manifest(path)?;
    Ok(spec.render(seconds, max_width))
}

pub fn is_time_varying_generator_path(path: &Path) -> bool {
    is_generator_manifest_path(path)
        && GeneratorSpec::load_manifest(path).is_ok_and(|s| s.kind.is_time_varying())
}

fn render_test_pattern(w: u32, h: u32, variant: TestPatternVariant) -> FrameBuffer {
    match variant {
        TestPatternVariant::SmpteBars => smpte_color_bars(w, h),
        TestPatternVariant::Checkerboard => {
            let cell = (w.min(h) / 12).max(4);
            let mut f = FrameBuffer::solid(w, h, [48, 52, 58, 255]);
            for y in 0..h {
                for x in 0..w {
                    if ((x / cell) + (y / cell)).is_multiple_of(2) {
                        let i = (y as usize * w as usize + x as usize) * 4;
                        f.rgba[i..i + 4].copy_from_slice(&[196, 200, 208, 255]);
                    }
                }
            }
            f
        }
        TestPatternVariant::CrosshairGrid => {
            let mut f = FrameBuffer::solid(w, h, [18, 22, 28, 255]);
            let step = (w.min(h) / 8).max(8);
            let cx = w as i32 / 2;
            let cy = h as i32 / 2;
            let r_circle = (w.min(h) as f32 * 0.35).max(4.0);
            for y in 0..h {
                for x in 0..w {
                    let dx = x as i32 - cx;
                    let dy = y as i32 - cy;
                    let dist = (dx as f32).hypot(dy as f32);
                    let on_center = dx.abs() <= 1 || dy.abs() <= 1;
                    let on_circle = (dist - r_circle).abs() <= 1.2;
                    let on_grid = x.is_multiple_of(step) || y.is_multiple_of(step);
                    let color = if on_center {
                        [245, 195, 66, 255]
                    } else if on_circle {
                        [76, 194, 255, 255]
                    } else if on_grid {
                        [58, 66, 78, 255]
                    } else {
                        continue;
                    };
                    let i = (y as usize * w as usize + x as usize) * 4;
                    f.rgba[i..i + 4].copy_from_slice(&color);
                }
            }
            f
        }
    }
}

/// What a frame of noise looks like.
struct NoiseLook {
    colored: bool,
    dark: [u8; 4],
    light: [u8; 4],
    grain: u32,
    seed: u32,
    frame: u32,
}

/// A random 32-bit number for one lane (a channel) of one cell of one frame, made by mixing the
/// numbers with integer arithmetic only, so it is the same on every machine.
fn noise_hash(cx: u32, cy: u32, frame: u32, seed: u32, lane: u32) -> u32 {
    let mut h = seed.wrapping_mul(0x9E37_79B1)
        ^ cx.wrapping_mul(0x85EB_CA6B)
        ^ cy.wrapping_mul(0xC2B2_AE35)
        ^ frame.wrapping_mul(0x27D4_EB2F)
        ^ lane.wrapping_mul(0x1656_67B1);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^ (h >> 15)
}

/// The noise of `look` at `w` by `h` pixels. `scale` is the size of the picture over the size of
/// the generator: cells are counted in the generator's own pixels, so a small preview shows the
/// same cells as the full picture.
fn render_noise(w: u32, h: u32, scale: f32, look: &NoiseLook) -> FrameBuffer {
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let to_source = 1.0 / scale.max(1e-3);
    // The cell of each column, worked out once
    let columns: Vec<u32> = (0..w)
        .map(|x| ((x as f32 + 0.5) * to_source) as u32 / look.grain)
        .collect();
    for y in 0..h {
        let cy = ((y as f32 + 0.5) * to_source) as u32 / look.grain;
        for (x, cx) in columns.iter().enumerate() {
            let at = (y as usize * w as usize + x) * 4;
            let px = &mut rgba[at..at + 4];
            if look.colored {
                for (lane, c) in px.iter_mut().take(3).enumerate() {
                    *c = (noise_hash(*cx, cy, look.frame, look.seed, lane as u32) >> 24) as u8;
                }
                px[3] = 255;
            } else {
                let t = (noise_hash(*cx, cy, look.frame, look.seed, 0) >> 24) as f32 / 255.0;
                for ((out, d), l) in px.iter_mut().zip(look.dark).zip(look.light) {
                    let (d, l) = (f32::from(d), f32::from(l));
                    *out = (d + (l - d) * t + 0.5).floor().clamp(0.0, 255.0) as u8;
                }
            }
        }
    }
    FrameBuffer {
        width: w,
        height: h,
        rgba,
    }
}

fn render_gradient(
    w: u32,
    h: u32,
    shape: GradientShape,
    start: [u8; 4],
    end: [u8; 4],
    angle_deg: f32,
) -> FrameBuffer {
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let rad = angle_deg.to_radians();
    let dir_x = rad.cos();
    let dir_y = rad.sin();
    let inv_w = 1.0 / (w.saturating_sub(1).max(1) as f32);
    let inv_h = 1.0 / (h.saturating_sub(1).max(1) as f32);

    for y in 0..h {
        let ny = y as f32 * inv_h - 0.5;
        for x in 0..w {
            let nx = x as f32 * inv_w - 0.5;
            let t = match shape {
                GradientShape::Linear => {
                    let span = (dir_x.abs() + dir_y.abs()).max(1e-4) * 0.5;
                    ((nx * dir_x + ny * dir_y) / (2.0 * span) + 0.5).clamp(0.0, 1.0)
                }
                GradientShape::Radial => (nx.hypot(ny) * std::f32::consts::SQRT_2).clamp(0.0, 1.0),
            };
            let i = (y as usize * w as usize + x as usize) * 4;
            for c in 0..4 {
                let s = start[c] as f32;
                let e = end[c] as f32;
                rgba[i + c] = (s + (e - s) * t + 0.5).floor().clamp(0.0, 255.0) as u8;
            }
        }
    }
    FrameBuffer {
        width: w,
        height: h,
        rgba,
    }
}

/// The number shown by a countdown of `from` numbers over `duration` seconds at `seconds`, and the
/// fraction (`0.0..1.0`) of that number already elapsed. The last number stays on at the very end.
pub fn countdown_state(from: u32, duration: f64, seconds: f64) -> (u32, f32) {
    let from = from.max(1);
    let step = duration.max(0.001) / from as f64;
    let t = seconds.max(0.0) / step;
    let index = (t.floor() as u32).min(from - 1);
    let frac = if t >= from as f64 {
        0.999
    } else {
        (t - index as f64).clamp(0.0, 0.999) as f32
    };
    (from - index, frac)
}

fn render_countdown(
    w: u32,
    h: u32,
    from: u32,
    colors: [[u8; 4]; 3],
    seconds: f64,
    duration: f64,
) -> FrameBuffer {
    let [text_rgba, bg_rgba, line_rgba] = colors;
    let (number, frac) = countdown_state(from, duration, seconds);
    let mut frame = FrameBuffer::solid(w, h, bg_rgba);
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let radius = h.min(w) as f32 * 0.38;
    let thick = (h as f32 / 200.0).max(1.0);
    let put = |frame: &mut FrameBuffer, x: i32, y: i32| {
        if x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h {
            let i = (y as usize * w as usize + x as usize) * 4;
            blend_over_px(&mut frame.rgba[i..i + 4], line_rgba, 1.0);
        }
    };
    // Ring and cross
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            let d = (dx * dx + dy * dy).sqrt();
            if (d - radius).abs() <= thick / 2.0 + 0.5
                || dx.abs() <= thick / 2.0
                || dy.abs() <= thick / 2.0
            {
                put(&mut frame, x, y);
            }
        }
    }
    // The hand turns once per number, starting at the top and going clockwise
    let angle = frac * std::f32::consts::TAU;
    let steps = (radius * 2.0) as i32;
    for k in 0..=steps {
        let r = radius * k as f32 / steps.max(1) as f32;
        let (px, py) = (cx + r * angle.sin(), cy - r * angle.cos());
        for oy in 0..thick.ceil() as i32 {
            for ox in 0..thick.ceil() as i32 {
                put(&mut frame, px as i32 + ox, py as i32 + oy);
            }
        }
    }
    let digits = render_title_text(
        w,
        h,
        &number.to_string(),
        ((h as f32 / 14.0).round() as u32).max(1),
        ("Inter", "Bold"),
        text_rgba,
        [0, 0, 0, 0],
        [0, 0, 0, 0],
        0,
        [0, 0, 0, 0],
        [0, 0],
        0.5,
        TitleAnimation::Static,
        &TitleLayout::default(),
        0.0,
        1.0,
    );
    for (dst, src) in frame
        .rgba
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(digits.rgba.as_chunks::<4>().0.iter())
    {
        if src[3] > 0 {
            blend_over_px(dst, [src[0], src[1], src[2], src[3]], 1.0);
        }
    }
    frame
}

fn blend_over_px(dst: &mut [u8], src: [u8; 4], alpha_mul: f32) {
    let sa = (src[3] as f32 / 255.0 * alpha_mul).clamp(0.0, 1.0);
    if sa <= 0.0 {
        return;
    }
    let da = dst[3] as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 1e-6 {
        return;
    }
    for c in 0..3 {
        let sc = src[c] as f32 / 255.0;
        let dc = dst[c] as f32 / 255.0;
        let out_c = (sa * sc + (1.0 - sa) * da * dc) / out_a;
        dst[c] = (out_c.clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8;
    }
    dst[3] = (out_a.clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8;
}

#[allow(clippy::too_many_arguments)]
fn render_title_text(
    w: u32,
    h: u32,
    text: &str,
    scale: u32,
    font: (&str, &str),
    text_rgba: [u8; 4],
    bg_rgba: [u8; 4],
    outline_rgba: [u8; 4],
    outline_px: u32,
    shadow_rgba: [u8; 4],
    shadow_offset: [i32; 2],
    align_y: f32,
    animation: TitleAnimation,
    look: &TitleLayout,
    seconds: f64,
    duration: f64,
) -> FrameBuffer {
    use neoveg_text::layout::{ParagraphStyle, TextStyle};
    use neoveg_text::mask::{stroke, StrokeKind};
    use neoveg_text::raster::Mask;

    let mut frame = FrameBuffer::solid(w, h, bg_rgba);
    let progress = (seconds.max(0.0) / duration.max(0.1)).clamp(0.0, 1.0) as f32;
    let alpha_mul = match animation {
        TitleAnimation::FadeIn => (progress / 0.25).clamp(0.0, 1.0),
        _ => 1.0,
    };

    let all_chars: Vec<char> = text.chars().collect();
    let visible_chars = match animation {
        TitleAnimation::Typewriter => ((all_chars.len() as f32 * (progress / 0.75).clamp(0.0, 1.0))
            .ceil() as usize)
            .min(all_chars.len()),
        _ => all_chars.len(),
    };
    let visible_str: String = all_chars[..visible_chars].iter().collect();
    if visible_str.trim().is_empty() {
        return frame;
    }

    // The text engine (FilmCraft) shapes, kerns and breaks the lines with real fonts. The size keeps
    // the meaning the old bitmap scale had: a line is ten steps of the scale tall.
    let style = TextStyle {
        family: font.0.into(),
        style: font.1.into(),
        size: scale.max(1) as f32 * 10.0,
        ..TextStyle::default()
    };
    let box_width = look.box_pixels(w);
    let para = ParagraphStyle {
        align: look.align.text_align(),
        width: box_width,
        ..ParagraphStyle::default()
    };
    // The styles of stretches of the text, each with the colour it asks for (or the title's)
    let runs = title_runs(&visible_str, &look.spans, &style);
    let runs_own: Vec<Option<[u8; 4]>> = runs.iter().map(|(_, c)| *c).collect();
    let run_colors: Vec<[u8; 4]> = runs
        .iter()
        .map(|(_, color)| color.unwrap_or(text_rgba))
        .collect();
    let style_runs: Vec<neoveg_text::layout::StyleRun> =
        runs.into_iter().map(|(run, _)| run).collect();
    let laid = neoveg_text::layout_rich(&visible_str, &style, &style_runs, &para);
    let [left, top, right, bottom] = laid.bounds;
    let total_h = (bottom - top).ceil().max(1.0) as i32;

    let base_y = match animation {
        TitleAnimation::SlideUp => {
            let target = (h as f32 * align_y.clamp(0.05, 0.95)) as i32 - total_h / 2;
            let u = (progress / 0.30).clamp(0.0, 1.0);
            let smooth = 1.0 - (1.0 - u) * (1.0 - u);
            let start_y = target + (h as f32 * 0.16) as i32;
            start_y + ((target - start_y) as f32 * smooth).round() as i32
        }
        TitleAnimation::CreditRoll => {
            let start_y = h as i32;
            let end_y = -total_h;
            start_y + ((end_y - start_y) as f32 * progress).round() as i32
        }
        _ => (h as f32 * align_y.clamp(0.05, 0.95)) as i32 - total_h / 2,
    };

    // The glyphs go on a mask with room around them for the outline; the origin of a centred point
    // text is the middle of the first baseline.
    // A fixed width puts its box in the middle of the frame (the lines are aligned inside it); free
    // lines put the block in the middle, which for centred lines is the old place of the origin.
    let origin_x = match box_width {
        Some(width) => (w as f32 - width) / 2.0,
        None if look.align == TitleAlign::Center => w as f32 / 2.0,
        None => w as f32 / 2.0 - (left + right) / 2.0,
    };
    // A horizontal scroll moves the whole block along x, from outside one edge to outside the other
    let origin_x = match animation {
        TitleAnimation::ScrollLeft | TitleAnimation::ScrollRight => {
            let direction = if animation == TitleAnimation::ScrollLeft {
                ScrollDirection::Left
            } else {
                ScrollDirection::Right
            };
            let content = (right - left).max(1.0);
            scroll_position(direction, progress, content, w as f32) - left
        }
        _ => origin_x,
    };
    let origin = neoveg_text::render::at(origin_x, base_y as f32 - top);
    let (glyphs, gx, gy) = neoveg_text::render::rasterize(&laid, &origin);
    // A stack without a fill still shows the text in the title's colour
    let mut stack = look.stack.clone();
    if !stack.is_empty() && stack.fills.is_empty() {
        stack.fills.push(neoveg_text::title_paint::FillLayer {
            color: text_rgba,
            offset: [0, 0],
        });
    }
    let pad = if stack.is_empty() {
        outline_px as usize + 2
    } else {
        stack.margin()
    };
    let mut padded = Mask::new(glyphs.w + 2 * pad, glyphs.h + 2 * pad);
    padded.add(&glyphs, pad as i32, pad as i32);
    let (mx, my) = (gx - pad as i32, gy - pad as i32);

    let blit = |frame: &mut FrameBuffer, mask: &Mask, ox: i32, oy: i32, color: [u8; 4]| {
        for y in 0..mask.h {
            let dy = oy + y as i32;
            if dy < 0 || dy >= h as i32 {
                continue;
            }
            for x in 0..mask.w {
                let coverage = mask.a[y * mask.w + x];
                let dx = ox + x as i32;
                if coverage <= 0.003 || dx < 0 || dx >= w as i32 {
                    continue;
                }
                let i = (dy as usize * w as usize + dx as usize) * 4;
                blend_over_px(
                    &mut frame.rgba[i..i + 4],
                    color,
                    alpha_mul * coverage.min(1.0),
                );
            }
        }
    };

    // A stack of fills, outlines and shadows replaces the single ones; stretches with a colour of
    // their own are drawn over it
    if !stack.is_empty() {
        for (mask, color) in stack.layers(&padded) {
            blit(&mut frame, &mask, mx, my, color);
        }
        for (index, own) in runs_own.iter().enumerate() {
            if let Some(color) = own {
                let mut own_mask = Mask::new(padded.w, padded.h);
                let run = u16::try_from(index + 1).unwrap_or(u16::MAX);
                neoveg_text::render::draw_run(&laid, &origin, &mut own_mask, (mx, my), Some(run));
                blit(&mut frame, &own_mask, mx, my, *color);
            }
        }
        return frame;
    }
    // 1. Drop shadow, 2. outline, 3. the glyphs themselves.
    if shadow_rgba[3] > 0 && (shadow_offset[0] != 0 || shadow_offset[1] != 0) {
        blit(
            &mut frame,
            &padded,
            mx + shadow_offset[0],
            my + shadow_offset[1],
            shadow_rgba,
        );
    }
    if outline_px > 0 && outline_rgba[3] > 0 {
        let ring = stroke(&padded, outline_px as f32, StrokeKind::Outer);
        blit(&mut frame, &ring, mx, my, outline_rgba);
    }
    if style_runs.is_empty() {
        blit(&mut frame, &padded, mx, my, text_rgba);
    } else {
        // Each style is drawn in its own colour: the base style first, then the runs in order
        let mut base = Mask::new(padded.w, padded.h);
        neoveg_text::render::draw_run(&laid, &origin, &mut base, (mx, my), Some(0));
        blit(&mut frame, &base, mx, my, text_rgba);
        for (index, color) in run_colors.iter().enumerate() {
            let mut own = Mask::new(padded.w, padded.h);
            let run = u16::try_from(index + 1).unwrap_or(u16::MAX);
            neoveg_text::render::draw_run(&laid, &origin, &mut own, (mx, my), Some(run));
            blit(&mut frame, &own, mx, my, *color);
        }
    }
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bars_cover_the_frame_left_to_right() {
        let f = smpte_color_bars(70, 10);
        assert_eq!(f.pixel(0, 0), [191, 191, 191, 255]);
        assert_eq!(f.pixel(69, 9), [0, 0, 191, 255]);
        assert_eq!(f.rgba.len(), 70 * 10 * 4);
    }

    fn noise(colored: bool, grain: u32, seed: u32, animated: bool) -> GeneratorSpec {
        GeneratorSpec {
            name: "Ruído".into(),
            width: 64,
            height: 48,
            fps: 25.0,
            duration: 4.0,
            kind: GeneratorKind::Noise {
                colored,
                dark_rgba: [0, 0, 0, 255],
                light_rgba: [255, 255, 255, 255],
                grain,
                seed,
                animated,
            },
        }
    }

    #[test]
    fn noise_is_the_same_every_time_and_a_new_seed_is_a_new_noise() {
        let a = noise(false, 1, 7, false).render(0.0, 0);
        assert_eq!(
            a,
            noise(false, 1, 7, false).render(2.0, 0),
            "it stays when not animated"
        );
        assert_ne!(a, noise(false, 1, 8, false).render(0.0, 0));
        assert_eq!((a.width, a.height), (64, 48));
    }

    #[test]
    fn grey_noise_spreads_over_the_whole_range_around_the_middle() {
        let f = noise(false, 1, 3, false).render(0.0, 0);
        let greys: Vec<u8> = f.rgba.as_chunks::<4>().0.iter().map(|p| p[0]).collect();
        assert!(f
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| p[0] == p[1] && p[1] == p[2] && p[3] == 255));
        let mean = greys.iter().map(|&g| f64::from(g)).sum::<f64>() / greys.len() as f64;
        assert!((mean - 127.5).abs() < 8.0, "mean {mean}");
        assert!(*greys.iter().min().unwrap() < 12 && *greys.iter().max().unwrap() > 243);
    }

    #[test]
    fn coloured_noise_is_random_in_each_channel() {
        let f = noise(true, 1, 3, false).render(0.0, 0);
        let differs = f
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[0] != p[1] || p[1] != p[2])
            .count();
        assert!(
            differs > f.rgba.len() / 4 * 9 / 10,
            "{differs} coloured pixels"
        );
        assert!(f.rgba.as_chunks::<4>().0.iter().all(|p| p[3] == 255));
    }

    #[test]
    fn a_grain_makes_blocks_of_one_value() {
        let f = noise(false, 4, 5, false).render(0.0, 0);
        for (bx, by) in [(0, 0), (3, 2), (10, 7)] {
            let first = f.pixel(bx * 4, by * 4);
            for (dx, dy) in [(1, 0), (3, 3), (0, 2)] {
                assert_eq!(f.pixel(bx * 4 + dx, by * 4 + dy), first);
            }
        }
        assert_ne!(
            f.pixel(0, 0),
            f.pixel(4, 0),
            "the next block is another value"
        );
    }

    #[test]
    fn animated_noise_changes_at_each_frame_and_not_within_one() {
        let spec = noise(false, 1, 9, true);
        assert!(spec.kind.is_time_varying());
        assert!(!noise(false, 1, 9, false).kind.is_time_varying());
        let first = spec.render(0.0, 0);
        assert_eq!(
            first,
            spec.render(0.039, 0),
            "still the first frame at 25 fps"
        );
        assert_ne!(first, spec.render(0.041, 0), "the second frame");
    }

    #[test]
    fn a_small_preview_shows_the_cells_of_the_full_picture() {
        let spec = noise(false, 4, 11, false);
        let full = spec.render(0.0, 0);
        let half = spec.render(0.0, 32);
        assert_eq!((half.width, half.height), (32, 24));
        // The cell of a preview pixel is the cell of the pixels it stands for
        for (x, y) in [(2, 2), (9, 5), (20, 17)] {
            assert_eq!(half.pixel(x, y), full.pixel(x * 2, y * 2), "{x},{y}");
        }
    }

    #[test]
    fn solid_fills_every_pixel() {
        let f = FrameBuffer::solid(3, 2, [1, 2, 3, 4]);
        assert_eq!(f.pixel(2, 1), [1, 2, 3, 4]);
    }

    #[test]
    fn titles_are_set_in_real_fonts_with_smooth_edges_and_accents() {
        let make = |text: &str, font: &str, style: &str| GeneratorSpec {
            name: "Título".into(),
            width: 400,
            height: 160,
            fps: 25.0,
            duration: 4.0,
            kind: GeneratorKind::TitleText {
                text: text.into(),
                scale: 6,
                text_rgba: [255, 255, 255, 255],
                bg_rgba: [0, 0, 0, 0],
                outline_rgba: [0, 0, 0, 255],
                outline_px: 0,
                shadow_rgba: [0, 0, 0, 0],
                shadow_offset: [0, 0],
                align_y: 0.5,
                animation: TitleAnimation::Static,
                font: font.into(),
                font_style: style.into(),
                layout: TitleLayout::default(),
            },
        };
        let lit = |f: &FrameBuffer| {
            f.rgba
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|p| p[3] > 0)
                .count()
        };
        let plain = make("Hello, Ação", "Inter", "Bold").render(0.0, 0);
        assert!(lit(&plain) > 500, "the text is drawn");
        // Anti-aliased edges: partly covered pixels, not only 0 and 255
        assert!(plain
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[3] > 20 && p[3] < 235));
        // Another font, another picture; the accent and the cedilla are real glyphs
        assert_ne!(
            plain,
            make("Hello, Ação", "JetBrains Mono", "Regular").render(0.0, 0)
        );
        assert_ne!(
            make("Acao", "Inter", "Bold").render(0.0, 0),
            make("Ação", "Inter", "Bold").render(0.0, 0)
        );
        // The text is centred on the picture
        let centre = |f: &FrameBuffer| {
            let (mut sum, mut n) = (0.0f64, 0.0f64);
            for (i, p) in f.rgba.as_chunks::<4>().0.iter().enumerate() {
                if p[3] > 0 {
                    sum += (i % 400) as f64;
                    n += 1.0;
                }
            }
            sum / n
        };
        assert!((centre(&plain) - 200.0).abs() < 30.0, "{}", centre(&plain));
        // Old manifests without a font still load (Inter Bold)
        let json = serde_json::to_string(&make("x", "Inter", "Bold")).unwrap();
        let old = json
            .replace(",\"font\":\"Inter\"", "")
            .replace(",\"font_style\":\"Bold\"", "");
        assert!(!old.contains("font"));
        let back: GeneratorSpec = serde_json::from_str(&old).unwrap();
        assert_eq!(back, make("x", "Inter", "Bold"));
    }

    #[test]
    fn gradient_and_animated_title_manifest_round_trip_and_render_alpha_overlay() {
        let dir = std::env::temp_dir().join(format!("neoveg_gen_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let grad = GeneratorSpec {
            name: "Gradiente".into(),
            width: 120,
            height: 80,
            fps: 30.0,
            duration: 5.0,
            kind: GeneratorKind::Gradient {
                shape: GradientShape::Linear,
                start_rgba: [10, 20, 30, 255],
                end_rgba: [210, 220, 230, 255],
                angle_deg: 0.0,
            },
        };
        let gf = grad.render(0.0, 0);
        assert!(gf.pixel(0, 40)[0] < 30 && gf.pixel(119, 40)[0] > 190);

        let title = GeneratorSpec {
            name: "Título".into(),
            width: 320,
            height: 180,
            fps: 30.0,
            duration: 4.0,
            kind: GeneratorKind::TitleText {
                text: "PRODUÇÃO ÁUDIO".into(),
                scale: 4,
                text_rgba: [255, 240, 80, 255],
                bg_rgba: [0, 0, 0, 0],
                outline_rgba: [0, 0, 0, 255],
                outline_px: 2,
                shadow_rgba: [0, 0, 0, 180],
                shadow_offset: [3, 3],
                align_y: 0.5,
                animation: TitleAnimation::Typewriter,
                font: "Inter".into(),
                font_style: "Bold".into(),
                layout: TitleLayout::default(),
            },
        };
        let path = dir.join("titulo.neoveggen");
        title.save_manifest(&path).unwrap();
        assert!(is_generator_manifest_path(&path));
        assert!(is_time_varying_generator_path(&path));

        let early = decode_generator_at(&path, 0.0, 0).unwrap();
        let late = decode_generator_at(&path, 3.5, 0).unwrap();
        // Corners remain transparent so underlying video tracks show through.
        assert_eq!(late.pixel(0, 0)[3], 0);
        // Typewriter reveals substantially more lit pixels at t=3.5s than t=0.0s.
        let lit_early = early
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] > 0)
            .count();
        let lit_late = late
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] > 0)
            .count();
        assert!(
            lit_late > lit_early,
            "lit_late={lit_late}, lit_early={lit_early}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn title_with(text: &str, layout: TitleLayout) -> FrameBuffer {
        GeneratorSpec {
            name: "T".into(),
            width: 400,
            height: 160,
            fps: 25.0,
            duration: 4.0,
            kind: GeneratorKind::TitleText {
                text: text.into(),
                scale: 4,
                text_rgba: [255, 255, 255, 255],
                bg_rgba: [0, 0, 0, 0],
                outline_rgba: [0, 0, 0, 255],
                outline_px: 0,
                shadow_rgba: [0, 0, 0, 0],
                shadow_offset: [0, 0],
                align_y: 0.5,
                animation: TitleAnimation::Static,
                font: "Inter".into(),
                font_style: "Bold".into(),
                layout,
            },
        }
        .render(0.0, 0)
    }

    /// The columns of the picture that hold text: first and last.
    fn text_columns(f: &FrameBuffer) -> (usize, usize) {
        let (mut first, mut last) = (usize::MAX, 0);
        for (i, p) in f.rgba.as_chunks::<4>().0.iter().enumerate() {
            if p[3] > 40 {
                first = first.min(i % f.width as usize);
                last = last.max(i % f.width as usize);
            }
        }
        (first, last)
    }

    #[test]
    fn a_fixed_width_wraps_the_lines_and_the_alignment_puts_them_in_the_box() {
        let text = "uma frase bem longa que não cabe numa linha só";
        let free = title_with(text, TitleLayout::default());
        let boxed = |align| {
            title_with(
                text,
                TitleLayout {
                    align,
                    box_width: 0.5,
                    spans: Vec::new(),
                    ..TitleLayout::default()
                },
            )
        };
        // Free, the line is as wide as it is; boxed, no line is wider than half of the frame
        let (a, b) = text_columns(&free);
        let (c, d) = text_columns(&boxed(TitleAlign::Left));
        assert!(b - a > 200, "the free line is wide: {a} {b}");
        assert!(d - c <= 205, "the boxed lines stay in 200 px: {c} {d}");
        // Left lines start at the left edge of the box, right lines end at its right edge
        let (l0, _) = text_columns(&boxed(TitleAlign::Left));
        let (_, r1) = text_columns(&boxed(TitleAlign::Right));
        assert!((l0 as i32 - 100).abs() <= 12, "left edge of the box: {l0}");
        assert!((r1 as i32 - 300).abs() <= 12, "right edge of the box: {r1}");
        assert_ne!(boxed(TitleAlign::Left), boxed(TitleAlign::Center));
        assert_ne!(boxed(TitleAlign::Left), boxed(TitleAlign::Justify));
    }

    #[test]
    fn a_stretch_of_the_text_has_its_own_colour_size_and_weight() {
        let plain = title_with("ABCD", TitleLayout::default());
        let red = |f: &FrameBuffer| {
            f.rgba
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|p| p[3] > 200 && p[0] > 200 && p[1] < 60)
                .count()
        };
        assert_eq!(red(&plain), 0);
        let mut span = TitleSpan::new(1, 3);
        span.color = Some([255, 0, 0, 255]);
        let coloured = title_with(
            "ABCD",
            TitleLayout {
                spans: vec![span.clone()],
                ..TitleLayout::default()
            },
        );
        assert!(red(&coloured) > 100, "the middle letters are red");
        // A bigger stretch makes the picture wider
        span.color = None;
        span.scale = 1.6;
        let bigger = title_with(
            "ABCD",
            TitleLayout {
                spans: vec![span],
                ..TitleLayout::default()
            },
        );
        let (a, b) = text_columns(&plain);
        let (c, d) = text_columns(&bigger);
        assert!(d - c > b - a, "{c}-{d} against {a}-{b}");
    }

    #[test]
    fn the_runs_follow_the_characters_and_the_old_manifests_load() {
        let base = neoveg_text::layout::TextStyle::default();
        // "ação": the span 1..3 covers the two letters "çã" whatever their bytes
        let mut bold = TitleSpan::new(1, 3);
        bold.bold = true;
        bold.scale = 2.0;
        let runs = title_runs(
            "ação",
            &[bold, TitleSpan::new(9, 12), TitleSpan::new(2, 2)],
            &base,
        );
        assert_eq!(
            runs.len(),
            1,
            "a span outside the text or empty is left out"
        );
        let (run, color) = &runs[0];
        assert_eq!(&"ação"[run.range.clone()], "çã");
        assert_eq!(run.style.style, "Bold");
        assert_eq!(run.style.size, base.size * 2.0);
        assert_eq!(*color, None);
        // The layout is left out of the manifest while it is the plain one, and spans round trip
        assert!(TitleLayout::default().is_default());
        let layout = TitleLayout {
            align: TitleAlign::Justify,
            box_width: 0.6,
            spans: vec![TitleSpan::new(0, 2)],
            ..TitleLayout::default()
        };
        let json = serde_json::to_string(&layout).unwrap();
        assert_eq!(serde_json::from_str::<TitleLayout>(&json).unwrap(), layout);
        assert_eq!(layout.box_pixels(1000), Some(600.0));
        assert_eq!(TitleLayout::default().box_pixels(1000), None);
        let old = serde_json::to_string(&title_spec_without_layout()).unwrap();
        assert!(!old.contains("layout"));
        assert!(serde_json::from_str::<GeneratorSpec>(&old).is_ok());
    }

    fn scrolling_title(animation: TitleAnimation, layout: TitleLayout) -> GeneratorSpec {
        let mut spec = title_spec_without_layout();
        spec.width = 320;
        spec.height = 160;
        spec.duration = 4.0;
        spec.kind = GeneratorKind::TitleText {
            text: "ROLAGEM".into(),
            scale: 4,
            text_rgba: [255, 255, 255, 255],
            bg_rgba: [0, 0, 0, 0],
            outline_rgba: [0, 0, 0, 255],
            outline_px: 0,
            shadow_rgba: [0, 0, 0, 0],
            shadow_offset: [0, 0],
            align_y: 0.5,
            animation,
            font: "Inter".into(),
            font_style: "Bold".into(),
            layout,
        };
        spec
    }

    #[test]
    fn a_horizontal_scroll_crosses_the_frame_in_each_direction() {
        let left = scrolling_title(TitleAnimation::ScrollLeft, TitleLayout::default());
        assert!(left.kind.is_time_varying());
        // Out of the frame at both ends, in view and moving left in between
        assert_eq!(text_columns(&left.render(0.0, 0)).0, usize::MAX);
        assert_eq!(text_columns(&left.render(4.0, 0)).0, usize::MAX);
        let early = text_columns(&left.render(1.0, 0));
        let late = text_columns(&left.render(2.0, 0));
        assert!(early.0 != usize::MAX && late.0 != usize::MAX);
        assert!(late.0 < early.0, "moves to the left: {early:?} -> {late:?}");
        let right = scrolling_title(TitleAnimation::ScrollRight, TitleLayout::default());
        let (a, b) = (
            text_columns(&right.render(1.0, 0)),
            text_columns(&right.render(2.0, 0)),
        );
        assert!(b.0 > a.0, "moves to the right: {a:?} -> {b:?}");
        // The vertical roll keeps its own axis
        let credits = scrolling_title(TitleAnimation::CreditRoll, TitleLayout::default());
        let mid = text_columns(&credits.render(2.0, 0));
        assert!(
            mid.0 != usize::MAX && mid.0 > 20,
            "centred, not shifted: {mid:?}"
        );
    }

    #[test]
    fn a_stack_of_outlines_and_shadows_shows_every_layer() {
        use neoveg_text::title_paint::{OutlineLayer, OutlinePlacement, PaintStack, ShadowLayer};
        let plain = scrolling_title(TitleAnimation::Static, TitleLayout::default()).render(0.0, 0);
        let mut stack = PaintStack::single([255, 255, 255, 255], [0; 4], 0, [0; 4], [0, 0]);
        stack.outlines = vec![
            OutlineLayer {
                color: [255, 0, 0, 255],
                width: 8.0,
                placement: OutlinePlacement::Outer,
            },
            OutlineLayer {
                color: [0, 0, 255, 255],
                width: 3.0,
                placement: OutlinePlacement::Outer,
            },
        ];
        stack.shadows = vec![ShadowLayer {
            color: [0, 255, 0, 255],
            offset: [10, 10],
            blur: 0.0,
        }];
        let layout = TitleLayout {
            stack: stack.clone(),
            ..TitleLayout::default()
        };
        let stacked = scrolling_title(TitleAnimation::Static, layout.clone()).render(0.0, 0);
        let count = |f: &FrameBuffer, pred: fn(&[u8]) -> bool| {
            f.rgba
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|p| pred(*p))
                .count()
        };
        assert_eq!(count(&plain, |p| p[0] > 200 && p[1] < 60 && p[3] > 200), 0);
        assert!(
            count(&stacked, |p| p[0] > 200
                && p[1] < 60
                && p[2] < 60
                && p[3] > 200)
                > 50,
            "wide red outline"
        );
        assert!(
            count(&stacked, |p| p[2] > 200 && p[0] < 60 && p[3] > 200) > 50,
            "thin blue outline over it"
        );
        assert!(
            count(&stacked, |p| p[1] > 200
                && p[0] < 60
                && p[2] < 60
                && p[3] > 200)
                > 20,
            "green shadow"
        );
        // The blue ring is inside the red one: the red shows around it
        let (a, b) = (text_columns(&plain), text_columns(&stacked));
        assert!(b.0 < a.0 && b.1 > a.1, "the layers widen the picture");
        // A stack with an outline and no fill still draws the text in its own colour
        let outline_only = TitleLayout {
            stack: PaintStack {
                outlines: vec![stack.outlines[0]],
                ..PaintStack::default()
            },
            ..TitleLayout::default()
        };
        let only = scrolling_title(TitleAnimation::Static, outline_only).render(0.0, 0);
        assert!(
            count(&only, |p| p[0] > 240
                && p[1] > 240
                && p[2] > 240
                && p[3] > 240)
                > 50
        );
        // A stack is saved in the manifest and an empty one is left out
        let json = serde_json::to_string(&scrolling_title(TitleAnimation::Static, layout)).unwrap();
        assert!(json.contains("outlines"));
        let old = serde_json::to_string(&title_spec_without_layout()).unwrap();
        assert!(!old.contains("stack"));
    }

    #[test]
    fn the_countdown_counts_down_and_draws_each_number() {
        // 5 numbers over 5 seconds: one per second, the last one stays to the end
        assert_eq!(countdown_state(5, 5.0, 0.0).0, 5);
        assert_eq!(countdown_state(5, 5.0, 0.99).0, 5);
        assert_eq!(countdown_state(5, 5.0, 1.0).0, 4);
        assert_eq!(countdown_state(5, 5.0, 4.5).0, 1);
        assert_eq!(countdown_state(5, 5.0, 99.0).0, 1);
        let (_, frac) = countdown_state(5, 5.0, 2.5);
        assert!((frac - 0.5).abs() < 1e-4);
        let spec = GeneratorSpec {
            name: "C".into(),
            width: 160,
            height: 90,
            fps: 25.0,
            duration: 3.0,
            kind: GeneratorKind::Countdown {
                from: 3,
                text_rgba: [255, 255, 255, 255],
                bg_rgba: [0, 0, 0, 255],
                line_rgba: [90, 90, 90, 255],
            },
        };
        assert!(spec.kind.is_time_varying());
        let (a, b) = (spec.render(0.5, 0), spec.render(1.5, 0));
        assert_ne!(a.rgba, b.rgba, "another number, another picture");
        // The same instant renders the same and the manifest round trips
        assert_eq!(a.rgba, spec.render(0.5, 0).rgba);
        let json = serde_json::to_string(&spec).unwrap();
        assert_eq!(serde_json::from_str::<GeneratorSpec>(&json).unwrap(), spec);
        // Bright digit pixels exist around the middle
        assert!(a.rgba.as_chunks::<4>().0.iter().any(|p| p[0] > 200));
    }

    fn title_spec_without_layout() -> GeneratorSpec {
        GeneratorSpec {
            name: "T".into(),
            width: 64,
            height: 36,
            fps: 25.0,
            duration: 1.0,
            kind: GeneratorKind::default_title(),
        }
    }
}
