//! Appearance of a title beyond one fill, one outline and one shadow: a stack of fills, outlines
//! and shadows over the same glyphs, plus the motion of a scrolling title.
//!
//! Everything here is pure: it works on coverage masks and numbers, so the layers (and their
//! order) can be tested without a frame.

use serde::{Deserialize, Serialize};

use crate::mask::{StrokeKind, blur, offset, stroke};
use crate::raster::Mask;

/// A fill of the glyphs, optionally shifted (a second, shifted fill makes a bevel-like edge).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FillLayer {
    pub color: [u8; 4],
    #[serde(default)]
    pub offset: [i32; 2],
}

/// Where an outline sits against the edge of the glyphs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutlinePlacement {
    #[default]
    Outer,
    Center,
    Inner,
}

impl OutlinePlacement {
    fn kind(self) -> StrokeKind {
        match self {
            Self::Outer => StrokeKind::Outer,
            Self::Center => StrokeKind::Center,
            Self::Inner => StrokeKind::Inner,
        }
    }
}

/// An outline of the glyphs.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct OutlineLayer {
    pub color: [u8; 4],
    pub width: f32,
    #[serde(default)]
    pub placement: OutlinePlacement,
}

/// A shadow: the glyphs shifted, optionally blurred.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShadowLayer {
    pub color: [u8; 4],
    pub offset: [i32; 2],
    #[serde(default)]
    pub blur: f32,
}

/// Several fills, outlines and shadows over the same text. Drawn from the bottom up: the shadows
/// (the first listed is the lowest), then the outlines (the widest lowest, so a thin one stays
/// visible over a wide one), then the fills (the first listed is the lowest).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PaintStack {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fills: Vec<FillLayer>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outlines: Vec<OutlineLayer>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shadows: Vec<ShadowLayer>,
}

impl PaintStack {
    /// The classic single fill, outline and shadow, as a stack. Layers with no effect (zero width
    /// or zero alpha) are left out.
    pub fn single(
        fill: [u8; 4],
        outline: [u8; 4],
        outline_px: u32,
        shadow: [u8; 4],
        shadow_offset: [i32; 2],
    ) -> Self {
        let mut stack = Self {
            fills: vec![FillLayer {
                color: fill,
                offset: [0, 0],
            }],
            ..Self::default()
        };
        if outline_px > 0 && outline[3] > 0 {
            stack.outlines.push(OutlineLayer {
                color: outline,
                width: outline_px as f32,
                placement: OutlinePlacement::Outer,
            });
        }
        if shadow[3] > 0 && shadow_offset != [0, 0] {
            stack.shadows.push(ShadowLayer {
                color: shadow,
                offset: shadow_offset,
                blur: 0.0,
            });
        }
        stack
    }

    pub fn is_empty(&self) -> bool {
        self.fills.is_empty() && self.outlines.is_empty() && self.shadows.is_empty()
    }

    /// Pixels the layers reach beyond the glyphs on any side: the room the glyph mask needs around
    /// it before [`PaintStack::layers`].
    pub fn margin(&self) -> usize {
        let shift = |o: [i32; 2]| o[0].unsigned_abs().max(o[1].unsigned_abs()) as usize;
        let fills = self
            .fills
            .iter()
            .map(|f| shift(f.offset))
            .max()
            .unwrap_or(0);
        let outlines = self
            .outlines
            .iter()
            .map(|o| o.width.max(0.0).ceil() as usize)
            .max()
            .unwrap_or(0);
        let shadows = self
            .shadows
            .iter()
            .map(|s| shift(s.offset) + (s.blur.max(0.0) * 3.0).ceil() as usize)
            .max()
            .unwrap_or(0);
        fills.max(outlines).max(shadows) + 2
    }

    /// The masks to paint over a frame, bottom first, each with its colour. `glyphs` must have at
    /// least [`PaintStack::margin`] pixels of room around the text.
    pub fn layers(&self, glyphs: &Mask) -> Vec<(Mask, [u8; 4])> {
        let mut out = Vec::new();
        for s in &self.shadows {
            let moved = offset(glyphs, s.offset[0], s.offset[1]);
            let mask = if s.blur > 0.0 {
                blur(&moved, s.blur)
            } else {
                moved
            };
            out.push((mask, s.color));
        }
        let mut outlines: Vec<&OutlineLayer> =
            self.outlines.iter().filter(|o| o.width > 0.0).collect();
        outlines.sort_by(|a, b| b.width.total_cmp(&a.width));
        for o in outlines {
            out.push((stroke(glyphs, o.width, o.placement.kind()), o.color));
        }
        for f in &self.fills {
            out.push((offset(glyphs, f.offset[0], f.offset[1]), f.color));
        }
        out
    }
}

/// Where a title scrolls from and to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollDirection {
    /// Credits: enters at the bottom, leaves at the top.
    Up,
    Down,
    /// Ticker: enters at the right, leaves at the left.
    Left,
    Right,
}

impl ScrollDirection {
    pub fn is_horizontal(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
}

/// Position of the leading edge (the top or the left of the content) along the scroll axis at
/// `progress` (`0.0..=1.0`): the content starts completely outside the frame and ends completely
/// outside on the other side, at a constant speed.
pub fn scroll_position(
    direction: ScrollDirection,
    progress: f32,
    content_extent: f32,
    frame_extent: f32,
) -> f32 {
    let p = progress.clamp(0.0, 1.0);
    let (from, to) = match direction {
        ScrollDirection::Up | ScrollDirection::Left => (frame_extent, -content_extent),
        ScrollDirection::Down | ScrollDirection::Right => (-content_extent, frame_extent),
    };
    from + (to - from) * p
}

/// Speed of a scroll in pixels per second over `duration` seconds.
pub fn scroll_speed(content_extent: f32, frame_extent: f32, duration: f64) -> f32 {
    (content_extent + frame_extent) / duration.max(0.001) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(size: usize, pad: usize) -> Mask {
        let mut m = Mask::new(size + 2 * pad, size + 2 * pad);
        for y in pad..pad + size {
            for x in pad..pad + size {
                m.a[y * m.w + x] = 1.0;
            }
        }
        m
    }

    #[test]
    fn horizontal_scroll_enters_and_leaves_through_the_sides() {
        use ScrollDirection::*;
        // Left: starts at the right edge of the frame, ends fully past the left edge
        assert_eq!(scroll_position(Left, 0.0, 300.0, 1000.0), 1000.0);
        assert_eq!(scroll_position(Left, 1.0, 300.0, 1000.0), -300.0);
        assert_eq!(scroll_position(Left, 0.5, 300.0, 1000.0), 350.0);
        assert_eq!(scroll_position(Right, 0.0, 300.0, 1000.0), -300.0);
        assert_eq!(scroll_position(Right, 1.0, 300.0, 1000.0), 1000.0);
        assert_eq!(scroll_position(Up, 0.0, 50.0, 100.0), 100.0);
        assert_eq!(scroll_position(Down, 1.0, 50.0, 100.0), 100.0);
        // Out of range progress is held at the ends
        assert_eq!(scroll_position(Left, 2.0, 300.0, 1000.0), -300.0);
        assert!(Left.is_horizontal() && !Up.is_horizontal());
        assert!((scroll_speed(300.0, 1000.0, 2.0) - 650.0).abs() < 1e-3);
    }

    #[test]
    fn the_layers_come_bottom_up_with_the_widest_outline_lowest() {
        let stack = PaintStack {
            fills: vec![
                FillLayer {
                    color: [1, 0, 0, 255],
                    offset: [0, 0],
                },
                FillLayer {
                    color: [2, 0, 0, 255],
                    offset: [1, 1],
                },
            ],
            outlines: vec![
                OutlineLayer {
                    color: [0, 1, 0, 255],
                    width: 2.0,
                    placement: OutlinePlacement::Outer,
                },
                OutlineLayer {
                    color: [0, 2, 0, 255],
                    width: 5.0,
                    placement: OutlinePlacement::Outer,
                },
            ],
            shadows: vec![ShadowLayer {
                color: [0, 0, 1, 255],
                offset: [4, 4],
                blur: 0.0,
            }],
        };
        let glyphs = square(10, stack.margin());
        let colors: Vec<u8> = stack
            .layers(&glyphs)
            .iter()
            .map(|(_, c)| c[0] + c[1] * 10 + c[2] * 100)
            .collect();
        // shadow, wide outline (5 -> 20), thin outline (2 -> 10), fill 1, fill 2
        assert_eq!(colors, vec![100, 20, 10, 1, 2]);
        assert!(stack.margin() >= 6, "room for the shadow shift");
    }

    #[test]
    fn a_wider_outline_covers_more_than_a_narrow_one_and_shadows_shift() {
        let glyphs = square(12, 10);
        let narrow = PaintStack {
            outlines: vec![OutlineLayer {
                color: [0, 0, 0, 255],
                width: 1.0,
                placement: OutlinePlacement::Outer,
            }],
            ..PaintStack::default()
        };
        let wide = PaintStack {
            outlines: vec![OutlineLayer {
                width: 4.0,
                ..narrow.outlines[0]
            }],
            ..PaintStack::default()
        };
        let area = |s: &PaintStack| s.layers(&glyphs)[0].0.a.iter().sum::<f32>();
        assert!(area(&wide) > area(&narrow) * 1.5);
        let shadow = PaintStack {
            shadows: vec![ShadowLayer {
                color: [0, 0, 0, 255],
                offset: [3, 0],
                blur: 0.0,
            }],
            ..PaintStack::default()
        };
        let m = &shadow.layers(&glyphs)[0].0;
        // The square starts at column 10; its shadow starts at column 13
        let row = 10 * m.w;
        assert_eq!(m.a[row + 10], 0.0);
        assert_eq!(m.a[row + 13], 1.0);
    }

    #[test]
    fn the_single_stack_drops_the_layers_with_no_effect_and_round_trips() {
        let none = PaintStack::single([255; 4], [0; 4], 0, [0; 4], [0, 0]);
        assert_eq!(none.fills.len(), 1);
        assert!(none.outlines.is_empty() && none.shadows.is_empty());
        let full = PaintStack::single([255; 4], [9, 9, 9, 255], 2, [0, 0, 0, 180], [4, 4]);
        assert_eq!((full.outlines.len(), full.shadows.len()), (1, 1));
        let json = serde_json::to_string(&full).unwrap();
        assert_eq!(serde_json::from_str::<PaintStack>(&json).unwrap(), full);
        assert!(PaintStack::default().is_empty() && !full.is_empty());
    }
}
