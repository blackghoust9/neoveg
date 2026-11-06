//! View modes of the preview: deinterlacing, rotation and mirroring of the picture shown.
//!
//! These change only what the preview shows (the frame is transformed after it is composited),
//! never the project or the render.

use crate::frame::FrameBuffer;
use neoveg_common::presets::{Deinterlace, FieldOrder};

/// A quarter-turn rotation of the picture shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Rotation {
    #[default]
    None,
    /// 90 degrees clockwise.
    Cw90,
    Half,
    /// 90 degrees counter-clockwise.
    Ccw90,
}

impl Rotation {
    pub const ALL: [Rotation; 4] = [
        Rotation::None,
        Rotation::Cw90,
        Rotation::Half,
        Rotation::Ccw90,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Rotation::None => "Sem rotação",
            Rotation::Cw90 => "90° horário",
            Rotation::Half => "180°",
            Rotation::Ccw90 => "90° anti-horário",
        }
    }
}

/// What the preview does to the frame before it is shown.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ViewMode {
    /// Deinterlace the preview. `None` leaves the frame as it is; the other modes only act when
    /// the project is interlaced (`field_order` is not progressive).
    pub deinterlace: Option<Deinterlace>,
    pub field_order: Option<FieldOrder>,
    pub rotation: Rotation,
    pub flip_horizontal: bool,
    pub flip_vertical: bool,
}

impl ViewMode {
    /// `true` when the frame comes out as it went in.
    pub fn is_identity(&self) -> bool {
        self.deinterlace_active().is_none()
            && self.rotation == Rotation::None
            && !self.flip_horizontal
            && !self.flip_vertical
    }

    /// The deinterlace mode that really runs: only for an interlaced project and a mode that
    /// does something.
    fn deinterlace_active(&self) -> Option<(Deinterlace, FieldOrder)> {
        let order = self.field_order?;
        let mode = self.deinterlace?;
        (order != FieldOrder::Progressive && mode != Deinterlace::None).then_some((mode, order))
    }

    /// The frame as the preview shows it.
    pub fn apply(&self, frame: &FrameBuffer) -> FrameBuffer {
        let mut out = match self.deinterlace_active() {
            Some((mode, order)) => deinterlace(frame, mode, order),
            None => frame.clone(),
        };
        if self.flip_horizontal {
            out = flip_horizontal(&out);
        }
        if self.flip_vertical {
            out = flip_vertical(&out);
        }
        rotate(&out, self.rotation)
    }
}

/// Which rows belong to the field drawn first (the dominant one): the even rows for upper-first.
fn dominant_parity(order: FieldOrder) -> usize {
    match order {
        FieldOrder::LowerFirst => 1,
        _ => 0,
    }
}

/// One row of `frame` as a slice.
fn row(frame: &FrameBuffer, y: usize) -> &[u8] {
    let stride = frame.width as usize * 4;
    &frame.rgba[y * stride..(y + 1) * stride]
}

/// Summed difference (over R, G, B) from the line average from which a pixel counts as combed.
const COMB_THRESHOLD: u32 = 48;

/// Removes the combing of an interlaced frame.
///
/// * `BlendFields`: every row is mixed with its neighbours (1-2-1), so the two fields blend.
/// * `Interpolate`: the rows of the second field are rebuilt from the first field's rows.
/// * `SmartAdaptive`: like `Interpolate`, but only where the two fields disagree (motion); still
///   areas keep their full resolution.
pub fn deinterlace(frame: &FrameBuffer, mode: Deinterlace, order: FieldOrder) -> FrameBuffer {
    let (w, h) = (frame.width as usize, frame.height as usize);
    if mode == Deinterlace::None || order == FieldOrder::Progressive || h < 2 || w == 0 {
        return frame.clone();
    }
    let keep = dominant_parity(order);
    let mut out = frame.clone();
    let stride = w * 4;
    for y in 0..h {
        // At the first and last row the missing neighbour is the one on the other side
        let above = row(frame, if y > 0 { y - 1 } else { 1 });
        let below = row(frame, if y + 1 < h { y + 1 } else { h - 2 });
        let here = row(frame, y);
        let dst = &mut out.rgba[y * stride..(y + 1) * stride];
        match mode {
            Deinterlace::BlendFields => {
                for i in 0..stride {
                    if i % 4 == 3 {
                        continue;
                    }
                    let sum = u32::from(above[i]) + 2 * u32::from(here[i]) + u32::from(below[i]);
                    dst[i] = ((sum + 2) / 4) as u8;
                }
            }
            Deinterlace::Interpolate | Deinterlace::SmartAdaptive => {
                if y % 2 == keep {
                    continue;
                }
                for px in 0..w {
                    let at = px * 4;
                    let mut diff = 0u32;
                    for c in 0..3 {
                        let mean = (u32::from(above[at + c]) + u32::from(below[at + c])) / 2;
                        diff += mean.abs_diff(u32::from(here[at + c]));
                    }
                    // A still pixel agrees with the lines around it
                    if mode == Deinterlace::SmartAdaptive && diff < COMB_THRESHOLD {
                        continue;
                    }
                    for c in 0..3 {
                        dst[at + c] =
                            ((u32::from(above[at + c]) + u32::from(below[at + c])) / 2) as u8;
                    }
                }
            }
            Deinterlace::None => {}
        }
    }
    out
}

pub fn flip_horizontal(frame: &FrameBuffer) -> FrameBuffer {
    let (w, h) = (frame.width as usize, frame.height as usize);
    let mut out = frame.clone();
    for y in 0..h {
        for x in 0..w {
            let from = (y * w + (w - 1 - x)) * 4;
            let to = (y * w + x) * 4;
            out.rgba[to..to + 4].copy_from_slice(&frame.rgba[from..from + 4]);
        }
    }
    out
}

pub fn flip_vertical(frame: &FrameBuffer) -> FrameBuffer {
    let (w, h) = (frame.width as usize, frame.height as usize);
    let mut out = frame.clone();
    let stride = w * 4;
    for y in 0..h {
        out.rgba[y * stride..(y + 1) * stride].copy_from_slice(row(frame, h - 1 - y));
    }
    out
}

/// The frame turned by `rotation` (a quarter turn swaps width and height).
pub fn rotate(frame: &FrameBuffer, rotation: Rotation) -> FrameBuffer {
    let (w, h) = (frame.width as usize, frame.height as usize);
    match rotation {
        Rotation::None => frame.clone(),
        Rotation::Half => flip_vertical(&flip_horizontal(frame)),
        Rotation::Cw90 | Rotation::Ccw90 => {
            let mut rgba = vec![0u8; w * h * 4];
            // The result is `h` wide and `w` tall
            for y in 0..h {
                for x in 0..w {
                    let (nx, ny) = if rotation == Rotation::Cw90 {
                        (h - 1 - y, x)
                    } else {
                        (y, w - 1 - x)
                    };
                    let from = (y * w + x) * 4;
                    let to = (ny * h + nx) * 4;
                    rgba[to..to + 4].copy_from_slice(&frame.rgba[from..from + 4]);
                }
            }
            FrameBuffer {
                width: frame.height,
                height: frame.width,
                rgba,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rows alternate black and white: the worst combing there is.
    fn combed(w: u32, h: u32) -> FrameBuffer {
        let mut f = FrameBuffer::solid(w, h, [0, 0, 0, 255]);
        for y in (1..h).step_by(2) {
            for x in 0..w {
                let at = ((y * w + x) * 4) as usize;
                f.rgba[at..at + 3].copy_from_slice(&[255, 255, 255]);
            }
        }
        f
    }

    fn r(f: &FrameBuffer, y: u32) -> u8 {
        f.pixel(0, y)[0]
    }

    #[test]
    fn interpolate_rebuilds_the_second_field_from_the_first() {
        let out = deinterlace(
            &combed(2, 6),
            Deinterlace::Interpolate,
            FieldOrder::UpperFirst,
        );
        assert!(
            (0..6).all(|y| r(&out, y) == 0),
            "every row follows the black field"
        );
        let out = deinterlace(
            &combed(2, 6),
            Deinterlace::Interpolate,
            FieldOrder::LowerFirst,
        );
        // Lower-first keeps the odd (white) rows and the even ones are rebuilt from them
        assert!((1..6).all(|y| r(&out, y) == 255));
        assert_eq!(r(&out, 0), 255);
    }

    #[test]
    fn blend_fields_mixes_neighbouring_rows() {
        let out = deinterlace(
            &combed(1, 6),
            Deinterlace::BlendFields,
            FieldOrder::UpperFirst,
        );
        // Row 2 is black between two white rows: (255 + 0 + 255 + 2) / 4; row 1 the reverse
        assert_eq!(r(&out, 2), 128);
        assert_eq!(r(&out, 1), 128);
    }

    #[test]
    fn the_adaptive_mode_leaves_still_areas_alone_and_fixes_motion() {
        let still = FrameBuffer::solid(2, 6, [90, 90, 90, 255]);
        assert_eq!(
            deinterlace(&still, Deinterlace::SmartAdaptive, FieldOrder::UpperFirst),
            still
        );
        let out = deinterlace(
            &combed(2, 6),
            Deinterlace::SmartAdaptive,
            FieldOrder::UpperFirst,
        );
        assert!((0..6).all(|y| r(&out, y) == 0));
    }

    #[test]
    fn progressive_or_none_changes_nothing() {
        let f = combed(2, 6);
        assert_eq!(
            deinterlace(&f, Deinterlace::Interpolate, FieldOrder::Progressive),
            f
        );
        assert_eq!(
            deinterlace(&f, Deinterlace::None, FieldOrder::UpperFirst),
            f
        );
        let mode = ViewMode {
            deinterlace: Some(Deinterlace::Interpolate),
            field_order: Some(FieldOrder::Progressive),
            ..ViewMode::default()
        };
        assert!(mode.is_identity() && mode.apply(&f) == f);
    }

    fn numbered(w: u32, h: u32) -> FrameBuffer {
        let mut f = FrameBuffer::solid(w, h, [0, 0, 0, 255]);
        for i in 0..(w * h) as usize {
            f.rgba[i * 4] = i as u8;
        }
        f
    }

    #[test]
    fn a_quarter_turn_swaps_the_sides_and_four_turns_come_back() {
        let f = numbered(3, 2);
        let cw = rotate(&f, Rotation::Cw90);
        assert_eq!((cw.width, cw.height), (2, 3));
        // The top-left pixel goes to the top-right
        assert_eq!(cw.pixel(1, 0)[0], f.pixel(0, 0)[0]);
        // The top-right pixel goes to the bottom-right
        assert_eq!(cw.pixel(1, 2)[0], f.pixel(2, 0)[0]);
        let ccw = rotate(&f, Rotation::Ccw90);
        assert_eq!(ccw.pixel(0, 2)[0], f.pixel(0, 0)[0]);
        assert_eq!(rotate(&cw, Rotation::Ccw90), f);
        let mut back = f.clone();
        for _ in 0..4 {
            back = rotate(&back, Rotation::Cw90);
        }
        assert_eq!(back, f);
        assert_eq!(rotate(&rotate(&f, Rotation::Half), Rotation::Half), f);
    }

    #[test]
    fn flips_mirror_the_picture() {
        let f = numbered(3, 2);
        assert_eq!(flip_horizontal(&f).pixel(0, 0)[0], f.pixel(2, 0)[0]);
        assert_eq!(flip_vertical(&f).pixel(0, 0)[0], f.pixel(0, 1)[0]);
        let mode = ViewMode {
            flip_horizontal: true,
            flip_vertical: true,
            ..ViewMode::default()
        };
        assert!(!mode.is_identity());
        assert_eq!(mode.apply(&f), rotate(&f, Rotation::Half));
    }
}
