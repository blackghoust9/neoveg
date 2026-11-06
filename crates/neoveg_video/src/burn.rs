//! Burning subtitles into a picture: the text is set with the text engine (`neoveg_text`), in an
//! outline so it reads on any background, and drawn over the lower part of the frame. The export
//! uses it to put the captions of the project into the movie.

use neoveg_text::layout::{Align, ParagraphStyle, TextStyle};
use neoveg_text::mask::{stroke, StrokeKind};
use neoveg_text::raster::Mask;

use crate::frame::FrameBuffer;

/// How a burned-in subtitle looks. Sizes are fractions of the frame, so the same style suits any
/// resolution.
#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleStyle {
    pub family: String,
    pub style: String,
    /// Font size as a fraction of the frame height.
    pub size: f32,
    pub text_rgba: [u8; 4],
    pub outline_rgba: [u8; 4],
    /// Outline width as a fraction of the frame height.
    pub outline: f32,
    /// Distance from the bottom edge to the last line, as a fraction of the frame height.
    pub margin: f32,
    /// Width of the text box as a fraction of the frame width; longer lines wrap.
    pub box_width: f32,
}

impl Default for SubtitleStyle {
    fn default() -> Self {
        Self {
            family: "Inter".into(),
            style: "Bold".into(),
            size: 0.055,
            text_rgba: [255, 255, 255, 255],
            outline_rgba: [0, 0, 0, 255],
            outline: 0.006,
            margin: 0.07,
            box_width: 0.86,
        }
    }
}

impl From<&neoveg_common::project::CaptionStyle> for SubtitleStyle {
    /// The style of the project, with every value brought into range.
    fn from(style: &neoveg_common::project::CaptionStyle) -> Self {
        let s = style.sanitized();
        Self {
            family: s.family,
            style: s.style,
            size: s.size,
            text_rgba: s.text_rgba,
            outline_rgba: s.outline_rgba,
            outline: s.outline,
            margin: s.margin,
            box_width: s.box_width,
        }
    }
}

/// Draws `color` over the straight-alpha pixel `dst`, `coverage` (`0..=1`) of the way.
fn blend(dst: &mut [u8], color: [u8; 4], coverage: f32) {
    let sa = (f32::from(color[3]) / 255.0 * coverage).clamp(0.0, 1.0);
    if sa <= 0.0 {
        return;
    }
    let da = f32::from(dst[3]) / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 1e-6 {
        return;
    }
    for c in 0..3 {
        let sc = f32::from(color[c]) / 255.0;
        let dc = f32::from(dst[c]) / 255.0;
        let out = (sa * sc + (1.0 - sa) * da * dc) / out_a;
        dst[c] = (out.clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8;
    }
    dst[3] = (out_a.clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8;
}

fn blit(frame: &mut FrameBuffer, mask: &Mask, ox: i32, oy: i32, color: [u8; 4]) {
    let (w, h) = (frame.width as i32, frame.height as i32);
    for y in 0..mask.h {
        let dy = oy + y as i32;
        if dy < 0 || dy >= h {
            continue;
        }
        for x in 0..mask.w {
            let coverage = mask.a[y * mask.w + x];
            let dx = ox + x as i32;
            if coverage <= 0.003 || dx < 0 || dx >= w {
                continue;
            }
            let i = (dy as usize * w as usize + dx as usize) * 4;
            blend(&mut frame.rgba[i..i + 4], color, coverage.min(1.0));
        }
    }
}

/// Burns `text` into `frame`, centred near the bottom. An empty text changes nothing.
pub fn burn_subtitle(frame: &mut FrameBuffer, text: &str, style: &SubtitleStyle) {
    let text = text.trim();
    if text.is_empty() || frame.width == 0 || frame.height == 0 {
        return;
    }
    let (w, h) = (frame.width as f32, frame.height as f32);
    let text_style = TextStyle {
        family: style.family.clone(),
        style: style.style.clone(),
        size: (h * style.size).max(6.0),
        ..TextStyle::default()
    };
    let box_w = (w * style.box_width).max(8.0);
    let para = ParagraphStyle {
        align: Align::Center,
        width: Some(box_w),
        ..ParagraphStyle::default()
    };
    let laid = neoveg_text::layout(text, &text_style, &para);
    let block_h = laid.bounds[3] - laid.bounds[1];
    // Area text: the origin is the top-left corner of the box
    let origin = neoveg_text::render::at((w - box_w) / 2.0, h * (1.0 - style.margin) - block_h);
    let (glyphs, gx, gy) = neoveg_text::render::rasterize(&laid, &origin);

    let outline_px = (h * style.outline).round().max(0.0);
    let pad = outline_px as usize + 2;
    let mut padded = Mask::new(glyphs.w + 2 * pad, glyphs.h + 2 * pad);
    padded.add(&glyphs, pad as i32, pad as i32);
    let (mx, my) = (gx - pad as i32, gy - pad as i32);
    if outline_px > 0.0 && style.outline_rgba[3] > 0 {
        let ring = stroke(&padded, outline_px, StrokeKind::Outer);
        blit(frame, &ring, mx, my, style.outline_rgba);
    }
    blit(frame, &padded, mx, my, style.text_rgba);
}

/// Where a burned-in label sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Burns `text` (one or more lines) into a corner of `frame`, with the same outlined look as the
/// subtitles. `style.margin` is the distance from the top or bottom edge and `style.box_width`
/// the width of the text box; the distance from the side edge is the same as from the top or
/// bottom one.
pub fn burn_label(frame: &mut FrameBuffer, text: &str, corner: LabelCorner, style: &SubtitleStyle) {
    let text = text.trim();
    if text.is_empty() || frame.width == 0 || frame.height == 0 {
        return;
    }
    let (w, h) = (frame.width as f32, frame.height as f32);
    let text_style = TextStyle {
        family: style.family.clone(),
        style: style.style.clone(),
        size: (h * style.size).max(6.0),
        ..TextStyle::default()
    };
    let box_w = (w * style.box_width).max(8.0);
    let right = matches!(corner, LabelCorner::TopRight | LabelCorner::BottomRight);
    let para = ParagraphStyle {
        align: if right { Align::Right } else { Align::Left },
        width: Some(box_w),
        ..ParagraphStyle::default()
    };
    let laid = neoveg_text::layout(text, &text_style, &para);
    let block_h = laid.bounds[3] - laid.bounds[1];
    let gap = h * style.margin;
    let x = if right { w - gap - box_w } else { gap };
    let y = match corner {
        LabelCorner::TopLeft | LabelCorner::TopRight => gap,
        _ => h - gap - block_h,
    };
    let origin = neoveg_text::render::at(x, y);
    let (glyphs, gx, gy) = neoveg_text::render::rasterize(&laid, &origin);
    let outline_px = (h * style.outline).round().max(0.0);
    let pad = outline_px as usize + 2;
    let mut padded = Mask::new(glyphs.w + 2 * pad, glyphs.h + 2 * pad);
    padded.add(&glyphs, pad as i32, pad as i32);
    let (mx, my) = (gx - pad as i32, gy - pad as i32);
    if outline_px > 0.0 && style.outline_rgba[3] > 0 {
        let ring = stroke(&padded, outline_px, StrokeKind::Outer);
        blit(frame, &ring, mx, my, style.outline_rgba);
    }
    blit(frame, &padded, mx, my, style.text_rgba);
}

/// What to burn into the picture for review outside the editor.
#[derive(Debug, Clone, PartialEq)]
pub struct BurnIn {
    /// Burn the timecode of the project.
    pub timecode: bool,
    /// Burn the name of the event under the playhead.
    pub event_name: bool,
    pub timecode_corner: LabelCorner,
    pub name_corner: LabelCorner,
    pub drop_frame: bool,
    pub style: SubtitleStyle,
}

impl Default for BurnIn {
    fn default() -> Self {
        Self {
            timecode: true,
            event_name: true,
            timecode_corner: LabelCorner::BottomLeft,
            name_corner: LabelCorner::TopLeft,
            drop_frame: false,
            style: SubtitleStyle {
                family: "JetBrains Mono".into(),
                style: "Regular".into(),
                size: 0.04,
                box_width: 0.45,
                margin: 0.04,
                ..SubtitleStyle::default()
            },
        }
    }
}

impl BurnIn {
    /// Burns the timecode of `seconds` at `fps`, and `event_name` when asked for, into `frame`.
    pub fn apply(&self, frame: &mut FrameBuffer, seconds: f64, fps: f64, event_name: Option<&str>) {
        if self.timecode {
            let tc = neoveg_common::timecode::seconds_to_timecode(seconds, fps, self.drop_frame);
            burn_label(frame, &tc, self.timecode_corner, &self.style);
        }
        if self.event_name {
            if let Some(name) = event_name {
                burn_label(frame, name, self.name_corner, &self.style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit(frame: &FrameBuffer, rows: std::ops::Range<usize>) -> usize {
        let w = frame.width as usize;
        rows.map(|y| {
            frame.rgba[y * w * 4..(y + 1) * w * 4]
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|p| p[0] > 128)
                .count()
        })
        .sum()
    }

    #[test]
    fn a_subtitle_is_burned_into_the_lower_part_and_nowhere_else() {
        let blank = FrameBuffer::solid(640, 360, [0, 0, 0, 255]);
        let mut frame = blank.clone();
        burn_subtitle(&mut frame, "Olá, mundo!", &SubtitleStyle::default());
        assert_ne!(frame, blank);
        assert!(lit(&frame, 250..360) > 100, "white text near the bottom");
        assert_eq!(lit(&frame, 0..250), 0, "nothing above");
        // Centred: the lit pixels balance around the middle of the picture
        let (mut sum, mut n) = (0.0f64, 0.0f64);
        for (i, p) in frame.rgba.as_chunks::<4>().0.iter().enumerate() {
            if p[0] > 128 {
                sum += (i % 640) as f64;
                n += 1.0;
            }
        }
        assert!((sum / n - 320.0).abs() < 40.0, "{}", sum / n);
        // An empty or blank text changes nothing
        let mut same = blank.clone();
        burn_subtitle(&mut same, "  \n ", &SubtitleStyle::default());
        assert_eq!(same, blank);
    }

    #[test]
    fn the_project_style_becomes_the_burn_style_and_a_different_style_looks_different() {
        use neoveg_common::project::CaptionStyle;
        // The default of the project is the default of the burn
        assert_eq!(
            SubtitleStyle::from(&CaptionStyle::default()),
            SubtitleStyle::default()
        );
        // Hostile values are brought into range
        let wild = CaptionStyle {
            size: f32::NAN,
            outline: 9.0,
            margin: -3.0,
            box_width: 0.0,
            family: "  ".into(),
            ..CaptionStyle::default()
        };
        let s = SubtitleStyle::from(&wild);
        assert_eq!(
            (s.size, s.outline, s.margin, s.box_width),
            (0.055, 0.03, 0.0, 0.3)
        );
        assert_eq!(s.family, "Inter");
        // Colour, size and place all change the picture
        let draw = |style: &CaptionStyle| {
            let mut f = FrameBuffer::solid(640, 360, [0, 0, 0, 255]);
            burn_subtitle(&mut f, "Legenda", &SubtitleStyle::from(style));
            f
        };
        let base = draw(&CaptionStyle::default());
        let yellow = CaptionStyle {
            text_rgba: [255, 220, 0, 255],
            ..CaptionStyle::default()
        };
        let big = CaptionStyle {
            size: 0.1,
            ..CaptionStyle::default()
        };
        let high = CaptionStyle {
            margin: 0.4,
            ..CaptionStyle::default()
        };
        let mono = CaptionStyle {
            family: "JetBrains Mono".into(),
            style: "Regular".into(),
            ..CaptionStyle::default()
        };
        for other in [yellow.clone(), big, high, mono] {
            assert_ne!(draw(&other), base);
        }
        // The yellow text has no blue in it
        let yellow_frame = draw(&yellow);
        assert!(yellow_frame
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[0] > 200 && p[1] > 150 && p[2] < 60));
    }

    #[test]
    fn the_outline_keeps_the_text_readable_on_a_white_background() {
        let mut frame = FrameBuffer::solid(640, 360, [255, 255, 255, 255]);
        burn_subtitle(&mut frame, "Legenda", &SubtitleStyle::default());
        let dark = frame
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[0] < 100)
            .count();
        assert!(dark > 100, "the black outline shows on white: {dark}");
        // Long text wraps into more than one line instead of running off the sides
        let mut wrapped = FrameBuffer::solid(640, 360, [0, 0, 0, 255]);
        let long = "Esta é uma legenda bem comprida que precisa quebrar em duas linhas na tela";
        burn_subtitle(&mut wrapped, long, &SubtitleStyle::default());
        assert!(lit(&wrapped, 200..360) > 200);
        let (left, right) = (0..360).fold((false, false), |(l, r), y| {
            let row = &wrapped.rgba[y * 640 * 4..(y + 1) * 640 * 4];
            (
                l || row[..4 * 8].chunks(4).any(|p| p[0] > 128),
                r || row[(640 - 8) * 4..].chunks(4).any(|p| p[0] > 128),
            )
        });
        assert!(!left && !right, "the text stays inside the box");
    }

    fn lit_in(frame: &FrameBuffer, x: std::ops::Range<usize>, y: std::ops::Range<usize>) -> usize {
        let w = frame.width as usize;
        y.map(|yy| {
            x.clone()
                .filter(|&xx| frame.rgba[(yy * w + xx) * 4] > 128)
                .count()
        })
        .sum()
    }

    #[test]
    fn a_label_goes_to_the_corner_asked_for_and_nowhere_else() {
        let blank = FrameBuffer::solid(640, 360, [0, 0, 0, 255]);
        let style = BurnIn::default().style;
        for (corner, xs, ys) in [
            (LabelCorner::TopLeft, 0..320, 0..180),
            (LabelCorner::TopRight, 320..640, 0..180),
            (LabelCorner::BottomLeft, 0..320, 180..360),
            (LabelCorner::BottomRight, 320..640, 180..360),
        ] {
            let mut f = blank.clone();
            burn_label(&mut f, "01:02:03:04", corner, &style);
            let inside = lit_in(&f, xs, ys);
            let total = lit_in(&f, 0..640, 0..360);
            assert!(inside > 60, "{corner:?}: {inside}");
            assert_eq!(inside, total, "{corner:?} stays in its quadrant");
        }
        let mut same = blank.clone();
        burn_label(&mut same, " ", LabelCorner::TopLeft, &style);
        assert_eq!(same, blank);
    }

    #[test]
    fn the_burn_in_changes_with_the_time_and_follows_what_is_switched_on() {
        let blank = FrameBuffer::solid(640, 360, [0, 0, 0, 255]);
        let burn = BurnIn::default();
        let at = |t: f64, name: Option<&str>| {
            let mut f = blank.clone();
            burn.apply(&mut f, t, 25.0, name);
            f
        };
        let a = at(1.0, Some("Entrevista"));
        let b = at(2.0, Some("Entrevista"));
        assert_ne!(a, b, "another timecode, another picture");
        assert_eq!(a, at(1.0, Some("Entrevista")), "deterministic");
        // The name sits top-left, the timecode bottom-left
        assert!(lit_in(&a, 0..320, 0..180) > 60);
        assert!(lit_in(&a, 0..320, 180..360) > 60);
        // Without a name only the timecode is burned
        let tc_only = at(1.0, None);
        assert_eq!(lit_in(&tc_only, 0..640, 0..180), 0);
        let off = BurnIn {
            timecode: false,
            event_name: false,
            ..BurnIn::default()
        };
        let mut f = blank.clone();
        off.apply(&mut f, 1.0, 25.0, Some("x"));
        assert_eq!(f, blank);
    }
}
