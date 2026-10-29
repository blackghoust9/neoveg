//! Procedural media generators: Solid Color, Test Patterns (SMPTE bars, checkerboard, crosshair
//! grid), Linear/Radial Gradients, and Titles & Text (with outline, drop shadow, Portuguese Latin-1
//! glyphs, and animation presets), persisted as `.neoveggen` JSON manifests.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::frame::FrameBuffer;
use crate::info::{fit_size, VideoInfo};

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
            Self::SmpteBars => "Barras SMPTE (75%)",
            Self::Checkerboard => "Xadrez de transparência",
            Self::CrosshairGrid => "Grade de alinhamento e mira",
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
            Self::Linear => "Linear",
            Self::Radial => "Radial",
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
}

impl TitleAnimation {
    pub const ALL: [Self; 5] = [
        Self::Static,
        Self::FadeIn,
        Self::Typewriter,
        Self::SlideUp,
        Self::CreditRoll,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Static => "Estático",
            Self::FadeIn => "Surgir suave (Fade-In)",
            Self::Typewriter => "Máquina de escrever",
            Self::SlideUp => "Subir suave (Lower-Third)",
            Self::CreditRoll => "Rolagem de créditos",
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
    },
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
                    | TitleAnimation::CreditRoll,
                ..
            }
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
    seconds: f64,
    duration: f64,
) -> FrameBuffer {
    use neoveg_text::layout::{Align, ParagraphStyle, TextStyle};
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
    let para = ParagraphStyle {
        align: Align::Center,
        ..ParagraphStyle::default()
    };
    let laid = neoveg_text::layout(&visible_str, &style, &para);
    let [_, top, _, bottom] = laid.bounds;
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
    let origin = neoveg_text::render::at(w as f32 / 2.0, base_y as f32 - top);
    let (glyphs, gx, gy) = neoveg_text::render::rasterize(&laid, &origin);
    let pad = outline_px as usize + 2;
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
    blit(&mut frame, &padded, mx, my, text_rgba);
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
}
