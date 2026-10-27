//! Video scopes computed from a frame: histogram, luma waveform, RGB parade and vectorscope.
//!
//! Everything is a plain function over a [`FrameBuffer`], so the scopes are testable without a
//! window. Frames are sampled (at most [`MAX_SAMPLES`] pixels) so a scope costs about the same for
//! a thumbnail and for a 4K frame.

use crate::frame::FrameBuffer;

/// Size of the square grids (columns x levels) of the waveform, parade and vectorscope.
pub const GRID: usize = 256;
/// Most pixels looked at per scope.
pub const MAX_SAMPLES: usize = 160_000;

/// A `GRID x GRID` table of hit counts. `at(x, y)` has `y = 0` at the **bottom** (level 0).
#[derive(Debug, Clone, PartialEq)]
pub struct Grid {
    pub width: usize,
    pub height: usize,
    pub counts: Vec<u32>,
    /// Highest count in the grid (for scaling).
    pub peak: u32,
}

impl Grid {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            counts: vec![0; width * height],
            peak: 0,
        }
    }

    fn hit(&mut self, x: usize, y: usize) {
        let i = y.min(self.height - 1) * self.width + x.min(self.width - 1);
        self.counts[i] += 1;
        self.peak = self.peak.max(self.counts[i]);
    }

    pub fn at(&self, x: usize, y: usize) -> u32 {
        self.counts.get(y * self.width + x).copied().unwrap_or(0)
    }

    /// Brightness `0.0..=1.0` of a cell with a logarithmic curve, so single hits stay visible next to
    /// a large flat area.
    pub fn intensity(&self, x: usize, y: usize) -> f32 {
        let c = self.at(x, y);
        if c == 0 || self.peak == 0 {
            return 0.0;
        }
        ((c as f32).ln_1p() / (self.peak as f32).ln_1p()).clamp(0.0, 1.0)
    }
}

/// Every scope of one frame.
#[derive(Debug, Clone, PartialEq)]
pub struct Scopes {
    /// Number of pixels sampled.
    pub samples: usize,
    /// Histogram of R, G, B and luma: 256 bins each.
    pub histogram: [[u32; 256]; 4],
    /// Luma against the horizontal position of the pixel.
    pub waveform: Grid,
    /// R, G and B side by side, each over the full picture width.
    pub parade: [Grid; 3],
    /// Chroma (Cb horizontal, Cr vertical, Rec. 709), centre = no color.
    pub vectorscope: Grid,
}

/// Rec. 709 luma and chroma of an 8-bit color: `(y, cb, cr)`, each `0..=255` (chroma centred on 128).
pub fn ycbcr(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let (r, g, b) = (f32::from(r), f32::from(g), f32::from(b));
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let cb = 128.0 + (b - y) / 1.8556;
    let cr = 128.0 + (r - y) / 1.5748;
    let q = |v: f32| v.round().clamp(0.0, 255.0) as u8;
    (q(y), q(cb), q(cr))
}

impl Scopes {
    /// Computes every scope of `frame`. An empty frame gives empty scopes.
    pub fn from_frame(frame: &FrameBuffer) -> Self {
        let (w, h) = (frame.width as usize, frame.height as usize);
        let mut scopes = Self {
            samples: 0,
            histogram: [[0; 256]; 4],
            waveform: Grid::new(GRID, GRID),
            parade: [
                Grid::new(GRID, GRID),
                Grid::new(GRID, GRID),
                Grid::new(GRID, GRID),
            ],
            vectorscope: Grid::new(GRID, GRID),
        };
        if w == 0 || h == 0 || frame.rgba.len() < w * h * 4 {
            return scopes;
        }
        // Skip pixels evenly so a big frame costs no more than MAX_SAMPLES
        let step = ((w * h) as f64 / MAX_SAMPLES as f64).sqrt().ceil().max(1.0) as usize;
        for y in (0..h).step_by(step) {
            for x in (0..w).step_by(step) {
                let i = (y * w + x) * 4;
                let (r, g, b) = (frame.rgba[i], frame.rgba[i + 1], frame.rgba[i + 2]);
                let (luma, cb, cr) = ycbcr(r, g, b);
                scopes.samples += 1;
                scopes.histogram[0][r as usize] += 1;
                scopes.histogram[1][g as usize] += 1;
                scopes.histogram[2][b as usize] += 1;
                scopes.histogram[3][luma as usize] += 1;
                let col = x * GRID / w;
                scopes.waveform.hit(col, luma as usize);
                for (grid, level) in scopes.parade.iter_mut().zip([r, g, b]) {
                    grid.hit(col, level as usize);
                }
                scopes.vectorscope.hit(cb as usize, cr as usize);
            }
        }
        scopes
    }

    /// Highest bin of one histogram channel (`0` = R, `1` = G, `2` = B, `3` = luma).
    pub fn histogram_peak(&self, channel: usize) -> u32 {
        self.histogram
            .get(channel)
            .map_or(0, |h| h.iter().copied().max().unwrap_or(0))
    }

    /// Share of sampled pixels that are clipped to full white in any channel (`0.0..=1.0`).
    pub fn clipped_fraction(&self) -> f32 {
        if self.samples == 0 {
            return 0.0;
        }
        let clipped = (0..3)
            .map(|c| self.histogram[c][255] as usize)
            .max()
            .unwrap_or(0);
        clipped as f32 / self.samples as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generators::smpte_color_bars;

    #[test]
    fn luma_and_chroma_follow_rec_709() {
        assert_eq!(ycbcr(0, 0, 0), (0, 128, 128));
        assert_eq!(ycbcr(255, 255, 255), (255, 128, 128));
        let (_, cb, cr) = ycbcr(255, 0, 0);
        assert!(cr > 250 && cb < 128, "red leans to Cr: {cb} {cr}");
        let (_, cb, cr) = ycbcr(0, 0, 255);
        assert!(cb > 250 && cr < 128, "blue leans to Cb: {cb} {cr}");
        assert_eq!(ycbcr(128, 128, 128).0, 128);
    }

    #[test]
    fn a_flat_grey_frame_is_one_histogram_spike_and_one_vectorscope_dot() {
        let frame = FrameBuffer::solid(64, 36, [100, 100, 100, 255]);
        let s = Scopes::from_frame(&frame);
        assert_eq!(s.samples, 64 * 36);
        for channel in 0..4 {
            assert_eq!(s.histogram[channel][100], s.samples as u32);
            assert_eq!(s.histogram_peak(channel), s.samples as u32);
        }
        // Centre of the vectorscope, no color
        assert_eq!(s.vectorscope.at(128, 128), s.samples as u32);
        assert_eq!(s.vectorscope.peak, s.samples as u32);
        // Waveform: every column has its hits on level 100
        for col in [0, 100, 252] {
            assert!(s.waveform.at(col, 100) > 0);
            assert_eq!(s.waveform.at(col, 101), 0);
        }
        assert_eq!(s.clipped_fraction(), 0.0);
    }

    #[test]
    fn color_bars_show_seven_steps_in_the_parade_and_seven_dots_on_the_vectorscope() {
        let s = Scopes::from_frame(&smpte_color_bars(700, 40));
        // 75 % bars: the left bar is white and the fourth one is green
        assert!(s.parade[0].at(10, 191) > 0, "white bar, red channel");
        assert!(
            s.parade[0].at(3 * 256 / 7 + 10, 0) > 0,
            "green bar has no red"
        );
        assert!(
            s.parade[1].at(3 * 256 / 7 + 10, 191) > 0,
            "green bar, green channel"
        );
        // Seven bars: white plus six chromatic bars, each one a point on the vectorscope
        let lit: usize = s.vectorscope.counts.iter().filter(|c| **c > 0).count();
        assert_eq!(lit, 7);
        assert!(s.vectorscope.at(128, 128) > 0);
    }

    #[test]
    fn clipping_is_measured_and_intensity_is_logarithmic() {
        let mut frame = FrameBuffer::solid(10, 10, [10, 10, 10, 255]);
        for px in frame.rgba.chunks_mut(4).take(25) {
            px[0] = 255;
        }
        let s = Scopes::from_frame(&frame);
        assert!((s.clipped_fraction() - 0.25).abs() < 1e-6);
        let g = &s.waveform;
        assert_eq!(g.intensity(0, 0), 0.0);
        let lit = (0..GRID * GRID)
            .filter(|i| g.counts[*i] > 0)
            .map(|i| g.intensity(i % GRID, i / GRID))
            .fold(0.0f32, f32::max);
        assert!((lit - 1.0).abs() < 1e-6);
    }

    #[test]
    fn big_frames_are_sampled_and_bad_frames_do_not_panic() {
        let big = FrameBuffer::solid(3840, 2160, [30, 60, 90, 255]);
        let s = Scopes::from_frame(&big);
        assert!(s.samples <= MAX_SAMPLES + MAX_SAMPLES / 4, "{}", s.samples);
        assert!(s.samples > 10_000);
        let empty = FrameBuffer {
            width: 0,
            height: 0,
            rgba: Vec::new(),
        };
        assert_eq!(Scopes::from_frame(&empty).samples, 0);
        let short = FrameBuffer {
            width: 10,
            height: 10,
            rgba: vec![0; 8],
        };
        assert_eq!(Scopes::from_frame(&short).samples, 0);
    }
}
