//! Video effect chains: the registry of effects, their parameters and the CPU reference
//! implementation.
//!
//! The engine stores a chain as registry ids plus the parameters the user changed
//! (`neoveg_engine::TrackEffect`). The worker receives [`VideoEffect`]s (bypassed ones already left
//! out), clamps them through [`VideoEffect::resolve`] and runs the chain, on the GPU when an adapter
//! exists ([`crate::vfx_gpu`]) and with the functions of this module otherwise. The CPU version is
//! the reference: the GPU path has to match it (a test compares them).
//!
//! **Resolution independence.** Frames reach the chain at the source size scaled by the preview
//! quality, and the track chain runs at the canvas size, so no parameter is in pixels: spatial
//! values are fractions of the frame height. The same chain looks the same in Draft, in Best and in
//! the render.
//!
//! **Alpha.** Neighbourhood effects work on premultiplied colour, so a keyed or faded edge does not
//! pick up dark or green fringes.
//!
//! Effects and their parameters follow the reference manuals (Gaussian Blur: horizontal and vertical
//! range; Sharpen: none; Unsharp Mask: ratio; Vignette: outer radius, inner radius, aspect and
//! centre). Decisions of the project that the manuals do not state are in `docs/research/ui_audit.md`.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use crate::frame::FrameBuffer;

/// Longest chain the worker accepts (the same cap as the engine).
pub const MAX_CHAIN: usize = 32;

/// One adjustable number of an effect.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParamSpec {
    pub id: &'static str,
    /// Label in Brazilian Portuguese.
    pub label: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    /// Shown multiplied by 100 with a `%` sign (fractions of the frame height, centre positions).
    pub percent: bool,
}

/// An effect of the registry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectSpec {
    pub id: &'static str,
    /// Name in Brazilian Portuguese.
    pub label: &'static str,
    pub params: &'static [ParamSpec],
}

pub const GAUSSIAN_BLUR: &str = "gaussian_blur";
pub const SHARPEN: &str = "sharpen";
pub const UNSHARP_MASK: &str = "unsharp_mask";
pub const VIGNETTE: &str = "vignette";

/// Every effect the video layer can run, in the order of the effect list.
pub const EFFECTS: &[EffectSpec] = &[
    EffectSpec {
        id: GAUSSIAN_BLUR,
        label: "Desfoque gaussiano",
        params: &[
            ParamSpec {
                id: "h_range",
                label: "Alcance horizontal",
                min: 0.0,
                max: 0.05,
                default: 0.005,
                percent: true,
            },
            ParamSpec {
                id: "v_range",
                label: "Alcance vertical",
                min: 0.0,
                max: 0.05,
                default: 0.005,
                percent: true,
            },
        ],
    },
    EffectSpec {
        id: SHARPEN,
        label: "Nitidez",
        params: &[],
    },
    EffectSpec {
        id: UNSHARP_MASK,
        label: "Máscara de nitidez",
        params: &[ParamSpec {
            id: "ratio",
            label: "Proporção",
            min: 0.0,
            max: 3.0,
            default: 1.0,
            percent: false,
        }],
    },
    EffectSpec {
        id: VIGNETTE,
        label: "Vinheta",
        params: &[
            ParamSpec {
                id: "outer_radius",
                label: "Raio externo",
                min: 0.0,
                max: 2.0,
                default: 1.0,
                percent: false,
            },
            ParamSpec {
                id: "inner_radius",
                label: "Raio interno",
                min: 0.0,
                max: 2.0,
                default: 0.5,
                percent: false,
            },
            ParamSpec {
                id: "aspect",
                label: "Aspecto",
                min: 0.25,
                max: 4.0,
                default: 1.0,
                percent: false,
            },
            ParamSpec {
                id: "center_x",
                label: "Centro X",
                min: 0.0,
                max: 1.0,
                default: 0.5,
                percent: true,
            },
            ParamSpec {
                id: "center_y",
                label: "Centro Y",
                min: 0.0,
                max: 1.0,
                default: 0.5,
                percent: true,
            },
        ],
    },
];

/// The registry entry of `id`.
pub fn spec(id: &str) -> Option<&'static EffectSpec> {
    EFFECTS.iter().find(|e| e.id == id)
}

impl EffectSpec {
    pub fn param(&self, id: &str) -> Option<&'static ParamSpec> {
        self.params.iter().find(|p| p.id == id)
    }
}

/// An effect as the worker receives it: the registry id and the values that differ from the
/// defaults. Unknown ids are skipped when the chain runs, unknown or non-finite values are ignored.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VideoEffect {
    pub kind: String,
    pub params: BTreeMap<String, f32>,
}

impl VideoEffect {
    pub fn new(kind: &str) -> Self {
        Self {
            kind: kind.to_owned(),
            params: BTreeMap::new(),
        }
    }

    pub fn with(mut self, param: &str, value: f32) -> Self {
        self.params.insert(param.to_owned(), value);
        self
    }

    /// The value of `param` clamped to its range, or its default.
    fn value(&self, spec: &EffectSpec, param: &str) -> f32 {
        let Some(p) = spec.param(param) else {
            return 0.0;
        };
        match self.params.get(param) {
            Some(v) if v.is_finite() => v.clamp(p.min, p.max),
            _ => p.default,
        }
    }

    /// The effect with every value clamped, or `None` for an unknown id.
    pub fn resolve(&self) -> Option<Resolved> {
        let spec = spec(&self.kind)?;
        Some(match spec.id {
            GAUSSIAN_BLUR => Resolved::GaussianBlur {
                h_range: self.value(spec, "h_range"),
                v_range: self.value(spec, "v_range"),
            },
            SHARPEN => Resolved::Sharpen,
            UNSHARP_MASK => Resolved::UnsharpMask {
                ratio: self.value(spec, "ratio"),
            },
            VIGNETTE => {
                let inner = self.value(spec, "inner_radius");
                // A hard edge when the radii are equal or reversed, never a division by zero.
                let outer = self.value(spec, "outer_radius").max(inner + 1e-3);
                Resolved::Vignette {
                    outer,
                    inner,
                    aspect: self.value(spec, "aspect"),
                    center: [self.value(spec, "center_x"), self.value(spec, "center_y")],
                }
            }
            _ => return None,
        })
    }

    /// Feeds the effect into a hasher (the worker's frame cache key).
    pub fn hash_into(&self, h: &mut impl Hasher) {
        self.kind.hash(h);
        self.params.len().hash(h);
        for (k, v) in &self.params {
            k.hash(h);
            v.to_bits().hash(h);
        }
    }
}

/// An effect with its values clamped, ready to run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Resolved {
    /// Ranges are fractions of the frame height: the number of pixels blended to each side.
    GaussianBlur {
        h_range: f32,
        v_range: f32,
    },
    Sharpen,
    UnsharpMask {
        ratio: f32,
    },
    Vignette {
        outer: f32,
        inner: f32,
        aspect: f32,
        /// `0.0, 0.0` is the lower-left corner of the frame.
        center: [f32; 2],
    },
}

impl Resolved {
    /// `true` when the effect leaves the picture unchanged.
    pub fn is_neutral(&self) -> bool {
        match *self {
            Resolved::GaussianBlur { h_range, v_range } => h_range <= 0.0 && v_range <= 0.0,
            Resolved::Sharpen => false,
            Resolved::UnsharpMask { ratio } => ratio <= 0.0,
            Resolved::Vignette { .. } => false,
        }
    }
}

/// The chain resolved, with unknown and neutral effects left out.
pub fn resolve_chain(chain: &[VideoEffect]) -> Vec<Resolved> {
    chain
        .iter()
        .take(MAX_CHAIN)
        .filter_map(VideoEffect::resolve)
        .filter(|r| !r.is_neutral())
        .collect()
}

/// Feeds a whole chain into a hasher.
pub fn hash_chain(chain: &[VideoEffect], h: &mut impl Hasher) {
    chain.len().hash(h);
    for e in chain {
        e.hash_into(h);
    }
}

// ---- shared maths ----------------------------------------------------------------------------------

/// Pixels blended to each side for a range given as a fraction of the frame height.
pub fn range_pixels(range: f32, height: u32) -> f32 {
    range * height as f32
}

/// Longest half width of a blur kernel.
pub const MAX_KERNEL_HALF: usize = 192;

/// Normalised gaussian weights for offsets `-half..=half` (`2 * half + 1` values). `radius_px` is
/// the range in pixels; the standard deviation is half of it. `None` when it is under a quarter of
/// a pixel, which changes nothing.
pub fn gaussian_kernel(radius_px: f32) -> Option<Vec<f32>> {
    if !radius_px.is_finite() || radius_px < 0.5 {
        return None;
    }
    let sigma = radius_px / 2.0;
    let half = ((sigma * 3.0).ceil() as usize).clamp(1, MAX_KERNEL_HALF);
    let mut weights: Vec<f32> = (0..=2 * half)
        .map(|i| {
            let d = i as f32 - half as f32;
            (-(d * d) / (2.0 * sigma * sigma)).exp()
        })
        .collect();
    let sum: f32 = weights.iter().sum();
    for w in &mut weights {
        *w /= sum;
    }
    Some(weights)
}

/// Standard deviation, as a fraction of the frame height, of the blur the unsharp mask subtracts.
pub const UNSHARP_RADIUS: f32 = 0.006;

/// Centre-to-corner distance, in frame-height units, used to normalise the vignette radii.
pub fn vignette_corner(width: u32, height: u32) -> f32 {
    let ar = width as f32 / height.max(1) as f32;
    ((0.5 * ar).powi(2) + 0.25).sqrt()
}

/// The clamped numbers of a vignette.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VignetteShape {
    pub outer: f32,
    pub inner: f32,
    pub aspect: f32,
    /// `0.0, 0.0` is the lower-left corner of the frame.
    pub center: [f32; 2],
}

/// The darkening of the vignette at a pixel (`1.0` keeps it, `0.0` is black).
pub fn vignette_gain(px: u32, py: u32, width: u32, height: u32, shape: &VignetteShape) -> f32 {
    let VignetteShape {
        outer,
        inner,
        aspect,
        center,
    } = *shape;
    let ar = width as f32 / height.max(1) as f32;
    let nx = (px as f32 + 0.5) / width as f32 - center[0];
    // Frame coordinates grow downwards, the manual's centre grows upwards
    let ny = 1.0 - (py as f32 + 0.5) / height as f32 - center[1];
    let r = (((nx * ar) / aspect).powi(2) + ny * ny).sqrt() / vignette_corner(width, height);
    let t = ((r - inner) / (outer - inner)).clamp(0.0, 1.0);
    let smooth = t * t * (3.0 - 2.0 * t);
    1.0 - smooth
}

// ---- CPU reference ---------------------------------------------------------------------------------

/// A frame as premultiplied floats, the working format of the chain.
pub struct Work {
    pub width: usize,
    pub height: usize,
    pub px: Vec<[f32; 4]>,
}

impl Work {
    pub fn from_frame(frame: &FrameBuffer) -> Self {
        let px = frame
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| {
                let a = f32::from(p[3]) / 255.0;
                [
                    f32::from(p[0]) / 255.0 * a,
                    f32::from(p[1]) / 255.0 * a,
                    f32::from(p[2]) / 255.0 * a,
                    a,
                ]
            })
            .collect();
        Self {
            width: frame.width as usize,
            height: frame.height as usize,
            px,
        }
    }

    pub fn write_to(&self, frame: &mut FrameBuffer) {
        for (dst, p) in frame.rgba.as_chunks_mut::<4>().0.iter_mut().zip(&self.px) {
            let a = p[3].clamp(0.0, 1.0);
            let inv = if a > 1e-6 { 1.0 / a } else { 0.0 };
            for c in 0..3 {
                dst[c] = ((p[c] * inv).clamp(0.0, 1.0) * 255.0).round() as u8;
            }
            dst[3] = (a * 255.0).round() as u8;
        }
    }
}

/// Runs `job` on bands of rows, in parallel when the picture is big enough.
fn par_rows(out: &mut [[f32; 4]], width: usize, job: impl Fn(usize, &mut [[f32; 4]]) + Sync) {
    let rows = out.len() / width.max(1);
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    if rows * width < 64 * 1024 || threads < 2 {
        job(0, out);
        return;
    }
    let band = rows.div_ceil(threads);
    std::thread::scope(|s| {
        for (i, chunk) in out.chunks_mut(band * width).enumerate() {
            let job = &job;
            s.spawn(move || job(i * band, chunk));
        }
    });
}

fn blur_pass(src: &Work, kernel: &[f32], horizontal: bool) -> Work {
    let (w, h) = (src.width, src.height);
    let half = (kernel.len() / 2) as isize;
    let mut out = vec![[0.0f32; 4]; w * h];
    par_rows(&mut out, w, |first_row, band| {
        for (ry, row) in band.chunks_mut(w).enumerate() {
            let y = first_row + ry;
            for (x, dst) in row.iter_mut().enumerate() {
                let mut acc = [0.0f32; 4];
                for (k, wt) in kernel.iter().enumerate() {
                    let d = k as isize - half;
                    let (sx, sy) = if horizontal {
                        ((x as isize + d).clamp(0, w as isize - 1) as usize, y)
                    } else {
                        (x, (y as isize + d).clamp(0, h as isize - 1) as usize)
                    };
                    let p = src.px[sy * w + sx];
                    for c in 0..4 {
                        acc[c] += p[c] * wt;
                    }
                }
                *dst = acc;
            }
        }
    });
    Work {
        width: w,
        height: h,
        px: out,
    }
}

fn gaussian_blur(work: &Work, h_range: f32, v_range: f32) -> Work {
    let mut cur = Work {
        width: work.width,
        height: work.height,
        px: work.px.clone(),
    };
    if let Some(k) = gaussian_kernel(range_pixels(h_range, work.height as u32)) {
        cur = blur_pass(&cur, &k, true);
    }
    if let Some(k) = gaussian_kernel(range_pixels(v_range, work.height as u32)) {
        cur = blur_pass(&cur, &k, false);
    }
    cur
}

/// Keeps colour inside what a premultiplied pixel with this alpha can hold.
fn clamp_premultiplied(p: [f32; 4]) -> [f32; 4] {
    let a = p[3].clamp(0.0, 1.0);
    [
        p[0].clamp(0.0, a),
        p[1].clamp(0.0, a),
        p[2].clamp(0.0, a),
        a,
    ]
}

fn sharpen(work: &Work) -> Work {
    let (w, h) = (work.width, work.height);
    let mut out = vec![[0.0f32; 4]; w * h];
    par_rows(&mut out, w, |first_row, band| {
        for (ry, row) in band.chunks_mut(w).enumerate() {
            let y = first_row + ry;
            for (x, dst) in row.iter_mut().enumerate() {
                let at = |dx: isize, dy: isize| {
                    let sx = (x as isize + dx).clamp(0, w as isize - 1) as usize;
                    let sy = (y as isize + dy).clamp(0, h as isize - 1) as usize;
                    work.px[sy * w + sx]
                };
                let (c, l, r, u, d) = (at(0, 0), at(-1, 0), at(1, 0), at(0, -1), at(0, 1));
                let mut p = [0.0f32; 4];
                for i in 0..4 {
                    p[i] = 5.0 * c[i] - l[i] - r[i] - u[i] - d[i];
                }
                *dst = clamp_premultiplied(p);
            }
        }
    });
    Work {
        width: w,
        height: h,
        px: out,
    }
}

fn unsharp(work: &Work, ratio: f32) -> Work {
    let radius = range_pixels(UNSHARP_RADIUS * 2.0, work.height as u32);
    let Some(k) = gaussian_kernel(radius) else {
        return Work {
            width: work.width,
            height: work.height,
            px: work.px.clone(),
        };
    };
    let blurred = blur_pass(&blur_pass(work, &k, true), &k, false);
    let px = work
        .px
        .iter()
        .zip(&blurred.px)
        .map(|(o, b)| {
            let mut p = [0.0f32; 4];
            for i in 0..4 {
                p[i] = o[i] + ratio * (o[i] - b[i]);
            }
            clamp_premultiplied([p[0], p[1], p[2], o[3]])
        })
        .collect();
    Work {
        width: work.width,
        height: work.height,
        px,
    }
}

fn vignette(work: &mut Work, outer: f32, inner: f32, aspect: f32, center: [f32; 2]) {
    let (w, h) = (work.width, work.height);
    let shape = VignetteShape {
        outer,
        inner,
        aspect,
        center,
    };
    for y in 0..h {
        for x in 0..w {
            let g = vignette_gain(x as u32, y as u32, w as u32, h as u32, &shape);
            let p = &mut work.px[y * w + x];
            p[0] *= g;
            p[1] *= g;
            p[2] *= g;
        }
    }
}

/// Runs one resolved effect on the working buffer.
pub fn apply_resolved(work: &mut Work, effect: &Resolved) {
    match *effect {
        Resolved::GaussianBlur { h_range, v_range } => {
            *work = gaussian_blur(work, h_range, v_range)
        }
        Resolved::Sharpen => *work = sharpen(work),
        Resolved::UnsharpMask { ratio } => *work = unsharp(work, ratio),
        Resolved::Vignette {
            outer,
            inner,
            aspect,
            center,
        } => vignette(work, outer, inner, aspect, center),
    }
}

/// Runs the chain on the CPU. An empty (or all neutral) chain leaves the frame untouched.
pub fn apply_chain_cpu(frame: &mut FrameBuffer, chain: &[VideoEffect]) {
    let resolved = resolve_chain(chain);
    if resolved.is_empty() || frame.width == 0 || frame.height == 0 {
        return;
    }
    let mut work = Work::from_frame(frame);
    for effect in &resolved {
        apply_resolved(&mut work, effect);
    }
    work.write_to(frame);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generators::smpte_color_bars;
    use std::collections::hash_map::DefaultHasher;

    fn mean(frame: &FrameBuffer) -> [f64; 3] {
        let n = (frame.width * frame.height) as f64;
        let mut s = [0.0; 3];
        for p in frame.rgba.as_chunks::<4>().0.iter() {
            for c in 0..3 {
                s[c] += f64::from(p[c]);
            }
        }
        [s[0] / n, s[1] / n, s[2] / n]
    }

    fn hash(chain: &[VideoEffect]) -> u64 {
        let mut h = DefaultHasher::new();
        hash_chain(chain, &mut h);
        h.finish()
    }

    #[test]
    fn an_empty_or_neutral_chain_changes_nothing() {
        let original = smpte_color_bars(96, 54);
        let mut frame = original.clone();
        apply_chain_cpu(&mut frame, &[]);
        assert_eq!(frame, original);
        let neutral = [
            VideoEffect::new(GAUSSIAN_BLUR)
                .with("h_range", 0.0)
                .with("v_range", 0.0),
            VideoEffect::new(UNSHARP_MASK).with("ratio", 0.0),
            VideoEffect::new("not_an_effect"),
        ];
        apply_chain_cpu(&mut frame, &neutral);
        assert_eq!(frame, original);
    }

    #[test]
    fn values_are_clamped_and_bad_values_fall_back_to_the_default() {
        let fx = VideoEffect::new(GAUSSIAN_BLUR)
            .with("h_range", 9.0)
            .with("v_range", f32::NAN);
        assert_eq!(
            fx.resolve(),
            Some(Resolved::GaussianBlur {
                h_range: 0.05,
                v_range: 0.005
            })
        );
        let vg = VideoEffect::new(VIGNETTE)
            .with("inner_radius", 1.0)
            .with("outer_radius", 0.2);
        let Some(Resolved::Vignette { outer, inner, .. }) = vg.resolve() else {
            panic!("vignette");
        };
        assert!(outer > inner, "a reversed pair becomes a hard edge");
        assert!(VideoEffect::new("nope").resolve().is_none());
    }

    #[test]
    fn the_blur_keeps_the_average_and_softens_edges() {
        let original = smpte_color_bars(160, 90);
        let mut frame = original.clone();
        apply_chain_cpu(
            &mut frame,
            &[VideoEffect::new(GAUSSIAN_BLUR)
                .with("h_range", 0.04)
                .with("v_range", 0.0)],
        );
        let (a, b) = (mean(&original), mean(&frame));
        for c in 0..3 {
            assert!(
                (a[c] - b[c]).abs() < 2.0,
                "channel {c}: {} vs {}",
                a[c],
                b[c]
            );
        }
        assert_ne!(frame, original);
        // A hard vertical edge of the bars is now a ramp: neighbours differ less than before.
        let step = |f: &FrameBuffer| {
            (0..f.width - 1)
                .map(|x| {
                    let (p, q) = (f.pixel(x, 10), f.pixel(x + 1, 10));
                    (0..3)
                        .map(|c| i32::from(p[c]).abs_diff(i32::from(q[c])))
                        .sum::<u32>()
                })
                .max()
                .unwrap()
        };
        assert!(step(&frame) < step(&original));
    }

    #[test]
    fn the_blur_is_the_same_picture_at_any_resolution() {
        // The same range is the same share of the frame: a half-size frame blurs half as many pixels.
        let big = smpte_color_bars(320, 180);
        let small = smpte_color_bars(160, 90);
        let chain = [VideoEffect::new(GAUSSIAN_BLUR).with("h_range", 0.03)];
        let (mut a, mut b) = (big.clone(), small.clone());
        apply_chain_cpu(&mut a, &chain);
        apply_chain_cpu(&mut b, &chain);
        // Compare the first row sampled at the same relative positions.
        let mut worst = 0u32;
        for x in 0..160u32 {
            let p = a.pixel(x * 2, 20);
            let q = b.pixel(x, 10);
            for c in 0..3 {
                worst = worst.max(u32::from(p[c]).abs_diff(u32::from(q[c])));
            }
        }
        assert!(worst < 40, "worst difference {worst}");
    }

    #[test]
    fn blur_does_not_leave_dark_fringes_at_a_transparent_edge() {
        // A white block on a fully transparent background: premultiplied blur keeps the colour white.
        let mut frame = FrameBuffer::solid(64, 64, [255, 255, 255, 0]);
        for y in 20..44 {
            for x in 20..44 {
                let i = (y * 64 + x) * 4;
                frame.rgba[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
            }
        }
        apply_chain_cpu(
            &mut frame,
            &[VideoEffect::new(GAUSSIAN_BLUR)
                .with("h_range", 0.05)
                .with("v_range", 0.05)],
        );
        for p in frame.rgba.as_chunks::<4>().0.iter() {
            if p[3] > 20 {
                assert!(p[0] > 240 && p[1] > 240 && p[2] > 240, "fringe {p:?}");
            }
        }
    }

    #[test]
    fn sharpen_and_unsharp_raise_contrast_at_an_edge_and_leave_flat_areas() {
        // Tall enough for the unsharp radius (a share of the height) to be over a pixel
        let mut frame = FrameBuffer::solid(160, 200, [100, 100, 100, 255]);
        for y in 0..200 {
            for x in 80..160 {
                let i = (y * 160 + x) * 4;
                frame.rgba[i..i + 3].copy_from_slice(&[160, 160, 160]);
            }
        }
        for chain in [
            vec![VideoEffect::new(SHARPEN)],
            vec![VideoEffect::new(UNSHARP_MASK).with("ratio", 2.0)],
        ] {
            let mut out = frame.clone();
            apply_chain_cpu(&mut out, &chain);
            assert_eq!(out.pixel(5, 100), frame.pixel(5, 100), "flat dark area");
            assert_eq!(
                out.pixel(150, 100),
                frame.pixel(150, 100),
                "flat light area"
            );
            assert!(out.pixel(79, 100)[0] < 100, "undershoot on the dark side");
            assert!(out.pixel(80, 100)[0] > 160, "overshoot on the light side");
        }
    }

    #[test]
    fn the_vignette_darkens_the_corners_and_keeps_the_centre() {
        let mut frame = FrameBuffer::solid(160, 90, [200, 200, 200, 255]);
        apply_chain_cpu(&mut frame, &[VideoEffect::new(VIGNETTE)]);
        assert_eq!(
            frame.pixel(80, 45),
            [200, 200, 200, 255],
            "centre untouched"
        );
        assert!(frame.pixel(0, 0)[0] < 20, "corner is dark");
        assert!(frame.pixel(159, 89)[0] < 20);
        assert_eq!(frame.pixel(0, 0)[3], 255, "alpha is left alone");
        // Moving the centre to the left edge keeps the left edge bright
        let mut off = FrameBuffer::solid(160, 90, [200, 200, 200, 255]);
        apply_chain_cpu(
            &mut off,
            &[VideoEffect::new(VIGNETTE)
                .with("center_x", 0.0)
                .with("center_y", 0.5)],
        );
        assert!(off.pixel(0, 45)[0] > 190);
        assert!(off.pixel(159, 45)[0] < 40);
    }

    #[test]
    fn the_chain_order_matters_and_the_cache_key_follows_every_value() {
        let a = [
            VideoEffect::new(VIGNETTE),
            VideoEffect::new(GAUSSIAN_BLUR).with("h_range", 0.02),
        ];
        let b = [a[1].clone(), a[0].clone()];
        assert_ne!(hash(&a), hash(&b));
        let mut c = a.clone();
        c[1].params.insert("h_range".into(), 0.021);
        assert_ne!(hash(&a), hash(&c), "one changed value changes the key");
        assert_eq!(hash(&a), hash(&a.clone()));
        assert_ne!(hash(&[]), hash(&a));
    }

    #[test]
    fn the_chain_is_capped() {
        let chain: Vec<VideoEffect> = (0..40).map(|_| VideoEffect::new(SHARPEN)).collect();
        assert_eq!(resolve_chain(&chain).len(), MAX_CHAIN);
    }
}
