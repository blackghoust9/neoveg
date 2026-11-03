// Adapted from `filmcraft-render` (`crates/render/src/scene.rs`) of
// https://github.com/storytold/filmcraft (commit 57dc82e).
// Copyright (c) 2026 ArtCraft Team and the FilmCraft contributors. MIT OR Apache-2.0; Neoveg uses
// it under the MIT terms, see third_party/filmcraft/LICENSE-MIT.
// Changes: adapted to edition 2021, wired to `FrameBuffer` and `f64`-seconds timeline timestamps.

//! Pure-Rust scene cut detection for splitting continuous clips into shot events.
//!
//! Each frame is summarized into a [`Signature`] combining a 32×18 BT.709 mean-luma spatial grid
//! with a 16-bin normalized histogram per RGB channel. [`difference`] blends the normalized L1
//! histogram distance with the spatial grid delta so camera pans (changing the grid while keeping
//! the histogram) and gradual lighting shifts (moving slowly relative to local activity) are not
//! misclassified as hard cuts.

use crate::frame::FrameBuffer;

const GW: usize = 32;
const GH: usize = 18;
const BINS: usize = 16;

/// Compact spatial + colour signature of one video frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Signature {
    grid: Vec<f32>,
    hist: [f32; 3 * BINS],
}

/// Computes the [`Signature`] of an RGBA8 buffer (`w`×`h`, row-major, 4 bytes per pixel).
pub fn signature(rgba: &[u8], w: usize, h: usize) -> Signature {
    let mut grid = vec![0.0f32; GW * GH];
    let mut count = vec![0u32; GW * GH];
    let mut hist = [0.0f32; 3 * BINS];
    if w == 0 || h == 0 || rgba.len() < w.saturating_mul(h).saturating_mul(4) {
        return Signature { grid, hist };
    }
    // Subsample at most ~256×144 points so 1080p/4K frames remain cheap on CPU.
    let step_x = (w / 256).max(1);
    let step_y = (h / 144).max(1);
    let mut n = 0u32;
    for y in (0..h).step_by(step_y) {
        let gy = y * GH / h;
        for x in (0..w).step_by(step_x) {
            let i = (y * w + x) * 4;
            let (r, g, b) = (rgba[i] as f32, rgba[i + 1] as f32, rgba[i + 2] as f32);
            let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
            let gi = gy * GW + x * GW / w;
            grid[gi] += luma;
            count[gi] += 1;
            hist[rgba[i] as usize * BINS / 256] += 1.0;
            hist[BINS + rgba[i + 1] as usize * BINS / 256] += 1.0;
            hist[2 * BINS + rgba[i + 2] as usize * BINS / 256] += 1.0;
            n += 1;
        }
    }
    for (g, c) in grid.iter_mut().zip(&count) {
        *g = if *c > 0 { *g / *c as f32 / 255.0 } else { 0.0 };
    }
    let inv = 1.0 / n.max(1) as f32;
    for v in &mut hist {
        *v *= inv;
    }
    Signature { grid, hist }
}

/// Computes the [`Signature`] of a [`FrameBuffer`].
pub fn signature_frame(frame: &FrameBuffer) -> Signature {
    signature(&frame.rgba, frame.width as usize, frame.height as usize)
}

/// Perceptual distance in `0.0..=1.0` between two frame signatures (`0.0` = identical).
pub fn difference(a: &Signature, b: &Signature) -> f32 {
    let h: f32 = a
        .hist
        .iter()
        .zip(&b.hist)
        .map(|(x, y)| (x - y).abs())
        .sum::<f32>()
        / 6.0;
    let g: f32 = a
        .grid
        .iter()
        .zip(&b.grid)
        .map(|(x, y)| (x - y).abs())
        .sum::<f32>()
        / (GW * GH) as f32;
    (0.5 * h + 0.5 * (2.0 * g).min(1.0)).clamp(0.0, 1.0)
}

/// Scene cut detection parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneOptions {
    /// `0.0` (only obvious cuts) to `100.0` (includes subtle cuts). Default `50.0`.
    pub sensitivity: f32,
    /// Minimum shot length in frames; cuts closer than this to the previous cut are ignored.
    pub min_shot_frames: usize,
}

impl Default for SceneOptions {
    fn default() -> Self {
        Self {
            sensitivity: 50.0,
            min_shot_frames: 6,
        }
    }
}

impl SceneOptions {
    /// Absolute difference floor derived from `sensitivity`.
    pub fn threshold(&self) -> f32 {
        let s = self.sensitivity.clamp(0.0, 100.0) / 100.0;
        0.45 - 0.33 * s
    }
}

/// Returns frame indices `i >= 1` that begin a new shot. `diffs[i]` is the difference between
/// frame `i - 1` and frame `i` (`diffs[0]` is ignored).
pub fn detect_cuts(diffs: &[f32], opt: &SceneOptions) -> Vec<usize> {
    const WINDOW: usize = 5;
    let thr = opt.threshold();
    let mut cuts: Vec<usize> = Vec::new();
    for i in 1..diffs.len() {
        let d = diffs[i];
        if d < thr {
            continue;
        }
        let lo = i.saturating_sub(WINDOW).max(1);
        let hi = (i + WINDOW + 1).min(diffs.len());
        let mut around: Vec<f32> = (lo..hi).filter(|&j| j != i).map(|j| diffs[j]).collect();
        let med = if around.is_empty() {
            0.0
        } else {
            around.sort_by(f32::total_cmp);
            around[around.len() / 2]
        };
        if d < 2.5 * med + 0.05 {
            continue;
        }
        if diffs.get(i + 1).is_some_and(|n| *n > d)
            || (i > 1 && diffs[i - 1] > d && cuts.last() != Some(&(i - 1)))
        {
            continue;
        }
        let since = cuts.last().copied().unwrap_or(0);
        if i - since < opt.min_shot_frames.max(1) {
            continue;
        }
        cuts.push(i);
    }
    cuts
}

/// Streaming scene cut detector fed sequentially with decoded video frames.
#[derive(Clone, Debug, Default)]
pub struct SceneDetector {
    prev: Option<Signature>,
    diffs: Vec<f32>,
}

impl SceneDetector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends an RGBA8 frame (`w`×`h`) and returns its difference against the previous frame.
    pub fn push_rgba(&mut self, rgba: &[u8], w: usize, h: usize) -> f32 {
        self.push(signature(rgba, w, h))
    }

    /// Appends a [`FrameBuffer`] and returns its difference against the previous frame.
    pub fn push_frame(&mut self, frame: &FrameBuffer) -> f32 {
        self.push(signature_frame(frame))
    }

    /// Appends a precomputed [`Signature`] and returns its difference against the previous frame.
    pub fn push(&mut self, sig: Signature) -> f32 {
        let d = self
            .prev
            .as_ref()
            .map(|p| difference(p, &sig))
            .unwrap_or(0.0);
        self.diffs.push(d);
        self.prev = Some(sig);
        d
    }

    pub fn diffs(&self) -> &[f32] {
        &self.diffs
    }

    /// Returns detected cut frame indices (`1..len`).
    pub fn cuts(&self, opt: &SceneOptions) -> Vec<usize> {
        detect_cuts(&self.diffs, opt)
    }

    /// Returns detected cut timestamps in seconds (`frame_index / fps`).
    pub fn cut_times(&self, fps: f64, opt: &SceneOptions) -> Vec<f64> {
        let rate = if fps.is_finite() && fps > 0.0 {
            fps
        } else {
            29.97
        };
        self.cuts(opt)
            .into_iter()
            .map(|i| i as f64 / rate)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_frame(shot: u32, t: u32, w: usize, h: usize) -> Vec<u8> {
        let mut v = vec![0u8; w * h * 4];
        let mut seed = shot.wrapping_mul(2_654_435_761) ^ t.wrapping_mul(40_503);
        for y in 0..h {
            for x in 0..w {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                let noise = ((seed >> 16) % 9) as i32 - 4;
                let xx = (x as u32 + 2 * t) as f32;
                let (r, g, b) = match shot % 4 {
                    0 => (40.0 + 150.0 * (xx * 0.05).sin().abs(), 90.0, 160.0),
                    1 => (
                        200.0,
                        180.0 - (y as f32) * 0.5,
                        40.0 + (xx * 0.1).cos().abs() * 60.0,
                    ),
                    2 => (
                        if ((xx as usize / 8) + (y / 8)).is_multiple_of(2) {
                            230.0
                        } else {
                            20.0
                        },
                        120.0,
                        60.0,
                    ),
                    _ => (
                        30.0,
                        30.0 + (y as f32 * 1.5).min(200.0),
                        30.0 + (xx * 0.03).sin().abs() * 200.0,
                    ),
                };
                let i = (y * w + x) * 4;
                v[i] = (r as i32 + noise).clamp(0, 255) as u8;
                v[i + 1] = (g as i32 + noise).clamp(0, 255) as u8;
                v[i + 2] = (b as i32 + noise).clamp(0, 255) as u8;
                v[i + 3] = 255;
            }
        }
        v
    }

    fn run(shots: &[(u32, u32)], opt: &SceneOptions) -> Vec<usize> {
        let mut d = SceneDetector::new();
        let mut t = 0;
        for &(shot, len) in shots {
            for _ in 0..len {
                let fb = FrameBuffer {
                    width: 96,
                    height: 54,
                    rgba: synthetic_frame(shot, t, 96, 54),
                };
                d.push_frame(&fb);
                t += 1;
            }
        }
        d.cuts(opt)
    }

    #[test]
    fn finds_hard_cuts_and_ignores_motion() {
        let cuts = run(
            &[(0, 20), (1, 15), (2, 25), (3, 12), (0, 10)],
            &SceneOptions::default(),
        );
        assert_eq!(cuts, vec![20, 35, 60, 72]);
        for s in [0.0, 50.0, 100.0] {
            assert!(
                run(
                    &[(2, 60)],
                    &SceneOptions {
                        sensitivity: s,
                        ..Default::default()
                    }
                )
                .is_empty(),
                "sensitivity {s}"
            );
        }
    }

    #[test]
    fn min_shot_length_merges_flashes_and_computes_cut_times() {
        let mut det = SceneDetector::new();
        let mut t = 0;
        for &(shot, len) in &[(0, 24), (1, 2), (0, 24)] {
            for _ in 0..len {
                det.push_rgba(&synthetic_frame(shot, t, 96, 54), 96, 54);
                t += 1;
            }
        }
        assert_eq!(det.cuts(&SceneOptions::default()), vec![24]);
        let times = det.cut_times(24.0, &SceneOptions::default());
        assert_eq!(times.len(), 1);
        assert!((times[0] - 1.0).abs() < 1e-9);
        assert_eq!(
            det.cuts(&SceneOptions {
                min_shot_frames: 1,
                ..Default::default()
            }),
            vec![24, 26]
        );
    }

    #[test]
    fn sensitivity_controls_subtle_cuts() {
        let mk = |k: i32| {
            let mut f = synthetic_frame(0, 0, 64, 36);
            for p in f.as_chunks_mut::<4>().0 {
                for v in &mut p[..3] {
                    *v = (*v as i32 + k).clamp(0, 255) as u8;
                }
            }
            signature(&f, 64, 36)
        };
        let mut d = SceneDetector::new();
        for i in 0..30 {
            d.push(mk(if i < 15 { 0 } else { 12 }));
        }
        assert!(d
            .cuts(&SceneOptions {
                sensitivity: 0.0,
                ..Default::default()
            })
            .is_empty());
        assert_eq!(
            d.cuts(&SceneOptions {
                sensitivity: 100.0,
                ..Default::default()
            }),
            vec![15]
        );
    }

    #[test]
    fn degenerate_inputs_are_safe() {
        assert!(detect_cuts(&[], &SceneOptions::default()).is_empty());
        assert!(detect_cuts(&[0.0], &SceneOptions::default()).is_empty());
        let s = signature(&[], 0, 0);
        assert_eq!(difference(&s, &s), 0.0);
    }
}
