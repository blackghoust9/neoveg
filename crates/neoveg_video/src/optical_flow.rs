//! Clean-room pyramidal optical-flow frame interpolation (`ResampleMode::OpticalFlow`).
//!
//! Estimates bidirectional translational motion between two neighbor frames on a multi-scale Rec.709
//! luminance pyramid (overlapping block matching with zero-motion regularization and parabolic
//! sub-pixel refinement), forward-projects the motion vectors to intermediate instant `t` (`0..1`),
//! and synthesizes the output picture by bidirectional motion-compensated premultiplied bilinear
//! warping with scene-cut and occlusion fallback.

use crate::frame::FrameBuffer;
use crate::pan_crop;
use crate::worker::mix_frames;

const CELL: usize = 4;

#[derive(Clone)]
struct LumaPlane {
    w: usize,
    h: usize,
    data: Vec<f32>,
}

impl LumaPlane {
    fn from_rgba(frame: &FrameBuffer) -> Self {
        let w = frame.width as usize;
        let h = frame.height as usize;
        let mut data = Vec::with_capacity(w * h);
        for px in frame.rgba.as_chunks::<4>().0.iter().take(w * h) {
            let a = f32::from(px[3]) * (1.0 / 255.0);
            let y = (0.2126 * f32::from(px[0])
                + 0.7152 * f32::from(px[1])
                + 0.0722 * f32::from(px[2]))
                * a;
            data.push(y);
        }
        Self { w, h, data }
    }

    fn downsample2(&self) -> Self {
        let nw = (self.w / 2).max(1);
        let nh = (self.h / 2).max(1);
        let mut data = vec![0.0f32; nw * nh];
        for y in 0..nh {
            let y0 = (y * 2).min(self.h - 1);
            let y1 = (y * 2 + 1).min(self.h - 1);
            for x in 0..nw {
                let x0 = (x * 2).min(self.w - 1);
                let x1 = (x * 2 + 1).min(self.w - 1);
                data[y * nw + x] = 0.25
                    * (self.data[y0 * self.w + x0]
                        + self.data[y0 * self.w + x1]
                        + self.data[y1 * self.w + x0]
                        + self.data[y1 * self.w + x1]);
            }
        }
        Self {
            w: nw,
            h: nh,
            data,
        }
    }

    #[inline]
    fn get_clamped(&self, x: isize, y: isize) -> f32 {
        let cx = x.clamp(0, self.w as isize - 1) as usize;
        let cy = y.clamp(0, self.h as isize - 1) as usize;
        self.data[cy * self.w + cx]
    }
}

#[derive(Clone, Copy, Default)]
struct CellFlow {
    u: f32,
    v: f32,
    variance: f32,
    residual: f32,
}

#[derive(Clone)]
struct FlowGrid {
    gw: usize,
    gh: usize,
    cells: Vec<CellFlow>,
}

impl FlowGrid {
    fn new(gw: usize, gh: usize) -> Self {
        Self {
            gw,
            gh,
            cells: vec![CellFlow::default(); gw * gh],
        }
    }

    #[inline]
    fn get_clamped(&self, gx: isize, gy: isize) -> CellFlow {
        let cx = gx.clamp(0, self.gw as isize - 1) as usize;
        let cy = gy.clamp(0, self.gh as isize - 1) as usize;
        self.cells[cy * self.gw + cx]
    }

    /// Bilinear lookup of `(u, v, residual)` at full-frame pixel center `(px, py)`.
    #[inline]
    fn sample(&self, px: f32, py: f32, cell_w: f32, cell_h: f32) -> (f32, f32, f32) {
        let gx = px / cell_w - 0.5;
        let gy = py / cell_h - 0.5;
        let x0 = gx.floor();
        let y0 = gy.floor();
        let tx = gx - x0;
        let ty = gy - y0;
        let ix = x0 as isize;
        let iy = y0 as isize;
        let c00 = self.get_clamped(ix, iy);
        let c10 = self.get_clamped(ix + 1, iy);
        let c01 = self.get_clamped(ix, iy + 1);
        let c11 = self.get_clamped(ix + 1, iy + 1);
        let bilerp = |a: f32, b: f32, c: f32, d: f32| {
            let top = a + (b - a) * tx;
            let bot = c + (d - c) * tx;
            top + (bot - top) * ty
        };
        (
            bilerp(c00.u, c10.u, c01.u, c11.u),
            bilerp(c00.v, c10.v, c01.v, c11.v),
            bilerp(c00.residual, c10.residual, c01.residual, c11.residual),
        )
    }
}

fn patch_variance(src: &LumaPlane, cx: isize, cy: isize, half: isize) -> f32 {
    let mut sum = 0.0f32;
    let mut count = 0usize;
    for py in (cy - half)..(cy + half) {
        for px in (cx - half)..(cx + half) {
            sum += src.get_clamped(px, py);
            count += 1;
        }
    }
    let mean = sum / (count.max(1) as f32);
    let mut mad = 0.0f32;
    for py in (cy - half)..(cy + half) {
        for px in (cx - half)..(cx + half) {
            mad += (src.get_clamped(px, py) - mean).abs();
        }
    }
    mad / (count.max(1) as f32)
}

fn patch_sad(src: &LumaPlane, dst: &LumaPlane, cx: isize, cy: isize, u: isize, v: isize, half: isize) -> f32 {
    let mut sad = 0.0f32;
    for py in (cy - half)..(cy + half) {
        for px in (cx - half)..(cx + half) {
            sad += (src.get_clamped(px, py) - dst.get_clamped(px + u, py + v)).abs();
        }
    }
    sad
}

fn subpixel_offset(left: f32, center: f32, right: f32) -> f32 {
    let denom = 2.0 * (left - 2.0 * center + right);
    if denom.abs() <= 1e-4 || center > left || center > right {
        return 0.0;
    }
    ((left - right) / denom).clamp(-0.5, 0.5)
}

fn smooth_grid(grid: &FlowGrid) -> FlowGrid {
    let mut out = grid.clone();
    for gy in 0..grid.gh as isize {
        for gx in 0..grid.gw as isize {
            let center = grid.get_clamped(gx, gy);
            let mut sum_u = 0.0f32;
            let mut sum_v = 0.0f32;
            let mut w_sum = 0.0f32;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let nb = grid.get_clamped(gx + dx, gy + dy);
                    let spatial = if dx == 0 && dy == 0 { 4.0 } else { 1.0 };
                    let conf = (1.0 + nb.variance * 0.25) / (1.0 + nb.residual * 0.15);
                    let w = spatial * conf;
                    sum_u += nb.u * w;
                    sum_v += nb.v * w;
                    w_sum += w;
                }
            }
            if w_sum > 0.0 {
                let idx = gy as usize * grid.gw + gx as usize;
                out.cells[idx] = CellFlow {
                    u: sum_u / w_sum,
                    v: sum_v / w_sum,
                    ..center
                };
            }
        }
    }
    out
}

fn estimate_pyramid_flow(src_pyr: &[LumaPlane], dst_pyr: &[LumaPlane]) -> FlowGrid {
    let mut prev: Option<FlowGrid> = None;
    for lvl in (0..src_pyr.len()).rev() {
        let src = &src_pyr[lvl];
        let dst = &dst_pyr[lvl];
        let gw = src.w.div_ceil(CELL);
        let gh = src.h.div_ceil(CELL);
        let mut grid = FlowGrid::new(gw, gh);
        let half: isize = if src.w.min(src.h) <= 20 { 3 } else { 4 };
        let area = ((2 * half) * (2 * half)) as f32;
        let radius: isize = if prev.is_none() { 6 } else { 3 };
        let max_disp = (src.w.max(src.h) as isize / 2).max(6);

        for gy in 0..gh {
            let cy = (gy * CELL + CELL / 2).min(src.h - 1) as isize;
            for gx in 0..gw {
                let cx = (gx * CELL + CELL / 2).min(src.w - 1) as isize;
                let variance = patch_variance(src, cx, cy, half);
                if variance < 0.75 {
                    let zero_sad = patch_sad(src, dst, cx, cy, 0, 0, half) / area;
                    grid.cells[gy * gw + gx] = CellFlow {
                        u: 0.0,
                        v: 0.0,
                        variance,
                        residual: zero_sad,
                    };
                    continue;
                }

                let mut seeds: [(isize, isize); 6] = [(0, 0); 6];
                let mut n_seeds = 1usize;
                if let Some(parent) = &prev {
                    let px = (gx / 2) as isize;
                    let py = (gy / 2) as isize;
                    for (ox, oy) in [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)] {
                        let pc = parent.get_clamped(px + ox, py + oy);
                        let cand = (
                            (pc.u * 2.0).round() as isize,
                            (pc.v * 2.0).round() as isize,
                        );
                        if !seeds[..n_seeds].contains(&cand) && n_seeds < seeds.len() {
                            seeds[n_seeds] = cand;
                            n_seeds += 1;
                        }
                    }
                }

                let cost_of = |u: isize, v: isize| -> (f32, f32) {
                    let raw = patch_sad(src, dst, cx, cy, u, v, half);
                    let reg = 0.20 * area * ((u * u + v * v) as f32).sqrt();
                    (raw + reg, raw)
                };

                let (mut best_cost, mut best_raw) = cost_of(0, 0);
                let (mut best_u, mut best_v) = (0isize, 0isize);

                // First pick the strongest predictor seed, then refine in `-radius..=radius`
                let mut center_seed = (0isize, 0isize);
                for &(su, sv) in &seeds[..n_seeds] {
                    let su = su.clamp(-max_disp, max_disp);
                    let sv = sv.clamp(-max_disp, max_disp);
                    let (cost, raw) = cost_of(su, sv);
                    if cost < best_cost {
                        best_cost = cost;
                        best_raw = raw;
                        best_u = su;
                        best_v = sv;
                        center_seed = (su, sv);
                    }
                }

                for dv in -radius..=radius {
                    for du in -radius..=radius {
                        let u = (center_seed.0 + du).clamp(-max_disp, max_disp);
                        let v = (center_seed.1 + dv).clamp(-max_disp, max_disp);
                        let (cost, raw) = cost_of(u, v);
                        if cost < best_cost {
                            best_cost = cost;
                            best_raw = raw;
                            best_u = u;
                            best_v = v;
                        }
                    }
                }

                let sad_l = patch_sad(src, dst, cx, cy, best_u - 1, best_v, half);
                let sad_r = patch_sad(src, dst, cx, cy, best_u + 1, best_v, half);
                let sad_u = patch_sad(src, dst, cx, cy, best_u, best_v - 1, half);
                let sad_d = patch_sad(src, dst, cx, cy, best_u, best_v + 1, half);
                let sub_u = best_u as f32 + subpixel_offset(sad_l, best_raw, sad_r);
                let sub_v = best_v as f32 + subpixel_offset(sad_u, best_raw, sad_d);

                grid.cells[gy * gw + gx] = CellFlow {
                    u: sub_u,
                    v: sub_v,
                    variance,
                    residual: best_raw / area,
                };
            }
        }
        prev = Some(smooth_grid(&grid));
    }
    prev.unwrap_or_else(|| FlowGrid::new(1, 1))
}

fn build_pyramid(frame: &FrameBuffer) -> (Vec<LumaPlane>, f32, f32) {
    let mut base = LumaPlane::from_rgba(frame);
    while base.w > 480 && base.h > 270 {
        base = base.downsample2();
    }
    let scale_x = frame.width as f32 / base.w as f32;
    let scale_y = frame.height as f32 / base.h as f32;
    let mut pyr = vec![base];
    while pyr.len() < 4 {
        let last = pyr.last().unwrap();
        if last.w < 24 || last.h < 24 {
            break;
        }
        pyr.push(last.downsample2());
    }
    (pyr, scale_x, scale_y)
}

/// Bilinear premultiplied sample with edge clamping (pixel centres at `+0.5`).
#[inline]
fn sample_clamped_premul(src: &FrameBuffer, x: f32, y: f32) -> [f32; 4] {
    let (w, h) = (src.width as isize, src.height as isize);
    let (fx, fy) = (x - 0.5, y - 0.5);
    let (x0, y0) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - x0, fy - y0);
    let (ix, iy) = (x0 as isize, y0 as isize);
    let mut acc = [0.0f32; 4];
    for (dy, wy) in [(0, 1.0 - ty), (1, ty)] {
        let py = (iy + dy).clamp(0, h - 1) as usize;
        for (dx, wx) in [(0, 1.0 - tx), (1, tx)] {
            let px = (ix + dx).clamp(0, w - 1) as usize;
            let i = (py * w as usize + px) * 4;
            let weight = wx * wy;
            let a = f32::from(src.rgba[i + 3]);
            let k = weight * a * (1.0 / 255.0);
            acc[0] += f32::from(src.rgba[i]) * k;
            acc[1] += f32::from(src.rgba[i + 1]) * k;
            acc[2] += f32::from(src.rgba[i + 2]) * k;
            acc[3] += weight * a;
        }
    }
    acc
}

/// Motion-compensated optical-flow interpolation between `a` (`t = 0`) and `b` (`t = 1`).
pub fn interpolate_frames(a: &FrameBuffer, b: &FrameBuffer, t: f32) -> FrameBuffer {
    let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };
    if t <= 0.0 {
        return a.clone();
    }
    if t >= 1.0 || a.width != b.width || a.height != b.height {
        return b.clone();
    }
    let expected = (a.width as usize) * (a.height as usize) * 4;
    if a.width < 8 || a.height < 8 || a.rgba.len() < expected || b.rgba.len() < expected {
        return mix_frames(a, b, t);
    }
    if a.rgba == b.rgba {
        return a.clone();
    }

    let (pyr_a, sx, sy) = build_pyramid(a);
    let (pyr_b, _, _) = build_pyramid(b);
    let fwd = estimate_pyramid_flow(&pyr_a, &pyr_b);
    let bwd = estimate_pyramid_flow(&pyr_b, &pyr_a);

    // Scene-cut detector: if global match residual is extreme across the picture, fall back cleanly
    // to a linear dissolve instead of warping unrelated shots.
    let mean_residual = fwd.cells.iter().map(|c| c.residual).sum::<f32>()
        / (fwd.cells.len().max(1) as f32);
    if mean_residual > 65.0 {
        return mix_frames(a, b, t);
    }

    let gw = fwd.gw;
    let gh = fwd.gh;
    let mut mid = FlowGrid::new(gw, gh);
    let mut best_score = vec![f32::NEG_INFINITY; gw * gh];

    // Seed every intermediate cell from colocated forward/backward flows
    for i in 0..(gw * gh) {
        let cf = fwd.cells[i];
        let cb = bwd.cells[i];
        mid.cells[i] = CellFlow {
            u: ((1.0 - t) * cf.u - t * cb.u) * sx,
            v: ((1.0 - t) * cf.v - t * cb.v) * sy,
            variance: cf.variance.max(cb.variance),
            residual: cf.residual.min(cb.residual),
        };
        best_score[i] = 0.25 * (cf.variance.max(cb.variance)) - 2.0 * (cf.residual.min(cb.residual));
    }

    // Forward-project textured cells from `a` (+t * v) and `b` (+(1 - t) * bwd) onto time `t`
    let mut splat = |gx: usize, gy: usize, disp_x: f32, disp_y: f32, u_full: f32, v_full: f32, cell: CellFlow| {
        if cell.variance < 0.75 {
            return;
        }
        let tx = gx as f32 + disp_x / (CELL as f32);
        let ty = gy as f32 + disp_y / (CELL as f32);
        let score = cell.variance - 2.0 * cell.residual;
        let rx = tx.floor() as isize;
        let ry = ty.floor() as isize;
        for oy in 0..=1 {
            for ox in 0..=1 {
                let nx = rx + ox;
                let ny = ry + oy;
                if nx < 0 || ny < 0 || nx >= gw as isize || ny >= gh as isize {
                    continue;
                }
                let dist = (tx - nx as f32).hypot(ty - ny as f32);
                if dist > 0.95 {
                    continue;
                }
                let idx = ny as usize * gw + nx as usize;
                let weighted = score - dist * 2.0;
                if weighted > best_score[idx] {
                    best_score[idx] = weighted;
                    mid.cells[idx] = CellFlow {
                        u: u_full,
                        v: v_full,
                        variance: cell.variance,
                        residual: cell.residual,
                    };
                }
            }
        }
    };

    for gy in 0..gh {
        for gx in 0..gw {
            let cf = fwd.cells[gy * gw + gx];
            splat(gx, gy, t * cf.u, t * cf.v, cf.u * sx, cf.v * sy, cf);
            let cb = bwd.cells[gy * gw + gx];
            splat(
                gx,
                gy,
                (1.0 - t) * cb.u,
                (1.0 - t) * cb.v,
                -cb.u * sx,
                -cb.v * sy,
                cb,
            );
        }
    }

    let cell_w = (CELL as f32) * sx;
    let cell_h = (CELL as f32) * sy;

    pan_crop::render(a.width, a.height, |x, y| {
        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
        let (u, v, res) = mid.sample(px, py, cell_w, cell_h);
        let ca = sample_clamped_premul(a, px - t * u, py - t * v);
        let cb = sample_clamped_premul(b, px + (1.0 - t) * u, py + (1.0 - t) * v);
        let mut warped = [0.0f32; 4];
        for c in 0..4 {
            warped[c] = ca[c] * (1.0 - t) + cb[c] * t;
        }
        let diff = ((ca[0] - cb[0]).abs()
            + (ca[1] - cb[1]).abs()
            + (ca[2] - cb[2]).abs()
            + (ca[3] - cb[3]).abs())
            * 0.25;
        if diff > 90.0 && res > 28.0 {
            let la = sample_clamped_premul(a, px, py);
            let lb = sample_clamped_premul(b, px, py);
            let fallback = ((diff - 90.0) / 100.0).clamp(0.0, 0.75);
            for c in 0..4 {
                let lin = la[c] * (1.0 - t) + lb[c] * t;
                warped[c] += (lin - warped[c]) * fallback;
            }
        }
        warped
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paint_box(
        w: u32,
        h: u32,
        bg: [u8; 4],
        rect: (u32, u32, u32, u32),
        fg: [u8; 4],
    ) -> FrameBuffer {
        let mut f = FrameBuffer::solid(w, h, bg);
        let (x0, y0, bw, bh) = rect;
        for y in y0..(y0 + bh).min(h) {
            for x in x0..(x0 + bw).min(w) {
                let i = ((y * w + x) * 4) as usize;
                f.rgba[i..i + 4].copy_from_slice(&fg);
            }
        }
        f
    }

    #[test]
    fn optical_flow_places_a_translating_object_at_its_intermediate_position_without_ghosting() {
        let bg = [16, 18, 24, 255];
        let fg = [240, 210, 50, 255];
        // A 12x12 bright square jumps 16 pixels to the right (no overlap at x = 14..26 vs 30..42)
        let frame_a = paint_box(64, 64, bg, (12, 26, 12, 12), fg);
        let frame_b = paint_box(64, 64, bg, (28, 26, 12, 12), fg);

        let blended = mix_frames(&frame_a, &frame_b, 0.5);
        let flowed = interpolate_frames(&frame_a, &frame_b, 0.5);

        // At t = 0.5 the true position is x = 20..32 (center x = 26, y = 32):
        // plain frame blend has two ghosts and misses the gap at x = 26..27, while optical flow
        // reconstructs the bright square cleanly in the middle and clears the trails.
        assert!(
            blended.pixel(27, 32)[0] < 30,
            "frame blend leaves a gap between non-overlapping positions"
        );
        let mid_px = flowed.pixel(26, 32);
        assert!(
            mid_px[0] > 200 && mid_px[1] > 170,
            "optical flow places the object at x0 + 0.5 * dx, got {mid_px:?}"
        );
        assert!(
            flowed.pixel(14, 32)[0] < 40 && flowed.pixel(38, 32)[0] < 40,
            "source/destination trails stay clean background: {:?} / {:?}",
            flowed.pixel(14, 32),
            flowed.pixel(38, 32)
        );
    }

    #[test]
    fn diagonal_motion_and_endpoints_interpolate_consistently() {
        let bg = [10, 10, 10, 255];
        let fg = [60, 220, 250, 255];
        let a = paint_box(64, 64, bg, (16, 16, 12, 12), fg);
        let b = paint_box(64, 64, bg, (24, 24, 12, 12), fg);

        assert_eq!(interpolate_frames(&a, &b, 0.0), a);
        assert_eq!(interpolate_frames(&a, &b, 1.0), b);
        assert_eq!(interpolate_frames(&a, &a, 0.5), a);

        let mid = interpolate_frames(&a, &b, 0.5);
        // Expected intermediate box: (20, 20, 12, 12) -> center (26, 26)
        let center = mid.pixel(26, 26);
        assert!(center[1] > 180 && center[2] > 200, "got {center:?}");
    }
}
