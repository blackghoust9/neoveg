//! Procedural media generators.

use crate::frame::FrameBuffer;

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
}
