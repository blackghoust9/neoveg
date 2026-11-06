//! Stereoscopic 3D viewing: a frame that carries a left-eye and a right-eye picture (side by side
//! or one over the other) is split into the two views and shown as a red-cyan anaglyph, as
//! interleaved rows (for passive displays) or as a side-by-side pair.

use crate::frame::FrameBuffer;

/// How the two eyes are packed in the source frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StereoLayout {
    /// Left eye in the left half, right eye in the right half (each squeezed horizontally).
    #[default]
    SideBySide,
    /// Left eye in the upper half, right eye in the lower half (each squeezed vertically).
    TopBottom,
}

impl StereoLayout {
    pub const ALL: [Self; 2] = [Self::SideBySide, Self::TopBottom];

    pub fn label(self) -> &'static str {
        match self {
            Self::SideBySide => "Lado a lado",
            Self::TopBottom => "Em cima e embaixo",
        }
    }
}

/// How the pair is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StereoOutput {
    /// The frame as it is.
    #[default]
    Off,
    /// The two views next to each other, squeezed to the frame size.
    SideBySide,
    /// Red (left eye) and cyan (right eye) in one picture, for coloured glasses.
    Anaglyph,
    /// Even rows from the left eye, odd rows from the right (row-interleaved, passive displays).
    RowInterleaved,
}

impl StereoOutput {
    pub const ALL: [Self; 4] = [
        Self::Off,
        Self::SideBySide,
        Self::Anaglyph,
        Self::RowInterleaved,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Desligado",
            Self::SideBySide => "Lado a lado",
            Self::Anaglyph => "Anaglífico (vermelho e ciano)",
            Self::RowInterleaved => "Linha alternada",
        }
    }
}

/// Splits `frame` into its `(left, right)` views, each stretched back to the size of the frame.
pub fn split_eyes(frame: &FrameBuffer, layout: StereoLayout) -> (FrameBuffer, FrameBuffer) {
    let (w, h) = (frame.width as usize, frame.height as usize);
    let mut left = frame.clone();
    let mut right = frame.clone();
    if w < 2 || h < 2 {
        return (left, right);
    }
    for y in 0..h {
        for x in 0..w {
            let at = (y * w + x) * 4;
            let (l_src, r_src) = match layout {
                StereoLayout::SideBySide => {
                    let sx = (x / 2).min(w / 2 - 1);
                    ((y * w + sx) * 4, (y * w + w / 2 + sx) * 4)
                }
                StereoLayout::TopBottom => {
                    let sy = (y / 2).min(h / 2 - 1);
                    ((sy * w + x) * 4, ((h / 2 + sy) * w + x) * 4)
                }
            };
            left.rgba[at..at + 4].copy_from_slice(&frame.rgba[l_src..l_src + 4]);
            right.rgba[at..at + 4].copy_from_slice(&frame.rgba[r_src..r_src + 4]);
        }
    }
    (left, right)
}

/// Red channel from the left view, green and blue from the right one.
pub fn anaglyph(left: &FrameBuffer, right: &FrameBuffer) -> FrameBuffer {
    let mut out = right.clone();
    if left.width == right.width && left.height == right.height {
        for (o, l) in out.rgba.chunks_exact_mut(4).zip(left.rgba.chunks_exact(4)) {
            o[0] = l[0];
        }
    }
    out
}

/// Even rows from the left view, odd rows from the right one.
pub fn row_interleaved(left: &FrameBuffer, right: &FrameBuffer) -> FrameBuffer {
    let mut out = left.clone();
    if left.width == right.width && left.height == right.height {
        let rb = left.width as usize * 4;
        for y in (1..left.height as usize).step_by(2) {
            out.rgba[y * rb..(y + 1) * rb].copy_from_slice(&right.rgba[y * rb..(y + 1) * rb]);
        }
    }
    out
}

/// The two views next to each other, each squeezed to half the width.
pub fn side_by_side(left: &FrameBuffer, right: &FrameBuffer) -> FrameBuffer {
    let mut out = left.clone();
    if left.width != right.width || left.height != right.height || left.width < 2 {
        return out;
    }
    let w = left.width as usize;
    for y in 0..left.height as usize {
        for x in 0..w {
            let src = if x < w / 2 { left } else { right };
            let sx = (if x < w / 2 { x } else { x - w / 2 } * 2).min(w - 1);
            let from = (y * w + sx) * 4;
            out.rgba[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&src.rgba[from..from + 4]);
        }
    }
    out
}

/// The frame as the chosen stereo output shows it.
pub fn stereo_view(frame: &FrameBuffer, layout: StereoLayout, output: StereoOutput) -> FrameBuffer {
    if output == StereoOutput::Off {
        return frame.clone();
    }
    let (left, right) = split_eyes(frame, layout);
    match output {
        StereoOutput::Off => frame.clone(),
        StereoOutput::SideBySide => side_by_side(&left, &right),
        StereoOutput::Anaglyph => anaglyph(&left, &right),
        StereoOutput::RowInterleaved => row_interleaved(&left, &right),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 8x4 pair: the left half is pure red-ish (200, 10, 10), the right half blue-ish (10, 20, 220).
    fn pair(layout: StereoLayout) -> FrameBuffer {
        let mut f = FrameBuffer::solid(8, 4, [0, 0, 0, 255]);
        for y in 0..4u32 {
            for x in 0..8u32 {
                let first = match layout {
                    StereoLayout::SideBySide => x < 4,
                    StereoLayout::TopBottom => y < 2,
                };
                let p = ((y * 8 + x) * 4) as usize;
                let c = if first { [200, 10, 10] } else { [10, 20, 220] };
                f.rgba[p..p + 3].copy_from_slice(&c);
            }
        }
        f
    }

    #[test]
    fn the_eyes_are_split_back_to_the_full_frame_size_for_both_layouts() {
        for layout in StereoLayout::ALL {
            let (l, r) = split_eyes(&pair(layout), layout);
            assert_eq!((l.width, l.height), (8, 4));
            assert!(l.rgba.chunks_exact(4).all(|p| p[..3] == [200, 10, 10]), "{layout:?}");
            assert!(r.rgba.chunks_exact(4).all(|p| p[..3] == [10, 20, 220]), "{layout:?}");
        }
    }

    #[test]
    fn an_anaglyph_takes_red_from_the_left_eye_and_the_rest_from_the_right() {
        let out = stereo_view(&pair(StereoLayout::SideBySide), StereoLayout::SideBySide, StereoOutput::Anaglyph);
        assert!(out.rgba.chunks_exact(4).all(|p| p == [200, 20, 220, 255]));
    }

    #[test]
    fn interleaved_rows_alternate_the_eyes() {
        let out = stereo_view(&pair(StereoLayout::TopBottom), StereoLayout::TopBottom, StereoOutput::RowInterleaved);
        for y in 0..4 {
            let want = if y % 2 == 0 { [200, 10, 10] } else { [10, 20, 220] };
            assert_eq!(out.pixel(3, y)[..3], want, "row {y}");
        }
    }

    #[test]
    fn side_by_side_output_converts_a_top_bottom_source() {
        let out = stereo_view(&pair(StereoLayout::TopBottom), StereoLayout::TopBottom, StereoOutput::SideBySide);
        assert_eq!(out, pair(StereoLayout::SideBySide));
    }

    #[test]
    fn off_leaves_the_frame_alone() {
        let f = pair(StereoLayout::SideBySide);
        assert_eq!(stereo_view(&f, StereoLayout::SideBySide, StereoOutput::Off), f);
    }
}
