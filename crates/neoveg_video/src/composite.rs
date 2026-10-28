//! Multi-track and parent/child video compositor: CPU reference + `wgpu` WGSL compute pipeline.
//!
//! Supports all 14 Classic NLE compositing modes plus the 3 spatial map modes
//! (`DisplacementMap`, `HeightMap`, `BumpMap`).

use std::borrow::Cow;
use std::sync::Arc;

use image::imageops::FilterType;
use image::RgbaImage;
use neoveg_common::composite::{luma709, CompositeMode};
use wgpu::util::DeviceExt;

use crate::frame::FrameBuffer;

/// One decoded layer ready to blend onto a canvas.
#[derive(Debug, Clone)]
pub struct DecodedLayer {
    pub frame: Arc<FrameBuffer>,
    pub mode: CompositeMode,
    pub opacity: f32,
}

/// Recursive compositing tree node supporting both flat track stacks and parent/child track groups.
#[derive(Debug, Clone)]
pub enum CompositeNode {
    Layer(DecodedLayer),
    /// Parent/child compositing group:
    /// 1. `children` composite bottom-to-top onto a transparent `[0, 0, 0, 0]` sub-canvas.
    /// 2. If `parent_layer` is `Some`, it composites over that sub-canvas using `parent_layer.mode`
    ///    and `parent_layer.opacity` (so e.g. `Multiply` masks only the child tracks).
    /// 3. The resulting group sub-canvas composites onto the lower canvas using `group_mode` and
    ///    `group_opacity`.
    Group {
        children: Vec<CompositeNode>,
        parent_layer: Option<DecodedLayer>,
        group_mode: CompositeMode,
        group_opacity: f32,
    },
}

/// Aspect-preserving letterbox/pillarbox fit of `src` onto a `(canvas_w, canvas_h)` buffer filled
/// with transparent `[0, 0, 0, 0]` bars when aspect ratios differ. Borrows `src` directly when
/// dimensions already match.
pub fn fit_layer_to_canvas(
    src: &FrameBuffer,
    canvas_w: u32,
    canvas_h: u32,
) -> Cow<'_, FrameBuffer> {
    let cw = canvas_w.max(1);
    let ch = canvas_h.max(1);
    if src.width == cw && src.height == ch {
        return Cow::Borrowed(src);
    }
    if src.width == 0 || src.height == 0 || src.rgba.len() < (src.width * src.height * 4) as usize {
        return Cow::Owned(FrameBuffer::solid(cw, ch, [0, 0, 0, 0]));
    }

    let scale = (cw as f64 / src.width as f64).min(ch as f64 / src.height as f64);
    let fit_w = ((src.width as f64 * scale).round() as u32).clamp(1, cw);
    let fit_h = ((src.height as f64 * scale).round() as u32).clamp(1, ch);

    let Some(src_img) = RgbaImage::from_raw(src.width, src.height, src.rgba.clone()) else {
        return Cow::Owned(FrameBuffer::solid(cw, ch, [0, 0, 0, 0]));
    };
    let resized = if (fit_w, fit_h) == (src.width, src.height) {
        src_img
    } else {
        image::imageops::resize(&src_img, fit_w, fit_h, FilterType::Triangle)
    };

    if fit_w == cw && fit_h == ch {
        return Cow::Owned(FrameBuffer {
            width: cw,
            height: ch,
            rgba: resized.into_raw(),
        });
    }

    let mut out = vec![0u8; (cw * ch * 4) as usize];
    let off_x = (cw - fit_w) / 2;
    let off_y = (ch - fit_h) / 2;
    let raw = resized.as_raw();
    for y in 0..fit_h {
        let src_row = (y * fit_w * 4) as usize;
        let dst_row = (((off_y + y) * cw + off_x) * 4) as usize;
        let row_bytes = (fit_w * 4) as usize;
        out[dst_row..dst_row + row_bytes].copy_from_slice(&raw[src_row..src_row + row_bytes]);
    }
    Cow::Owned(FrameBuffer {
        width: cw,
        height: ch,
        rgba: out,
    })
}

#[inline]
fn unpack_px(rgba: &[u8], idx: usize) -> [f32; 4] {
    let base = idx * 4;
    [
        rgba[base] as f32 / 255.0,
        rgba[base + 1] as f32 / 255.0,
        rgba[base + 2] as f32 / 255.0,
        rgba[base + 3] as f32 / 255.0,
    ]
}

#[inline]
fn pack_px(px: [f32; 4]) -> [u8; 4] {
    [
        (px[0].clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8,
        (px[1].clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8,
        (px[2].clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8,
        (px[3].clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8,
    ]
}

#[inline]
fn sample_clamped(buf: &FrameBuffer, x: i32, y: i32) -> [f32; 4] {
    let cx = x.clamp(0, buf.width as i32 - 1) as usize;
    let cy = y.clamp(0, buf.height as i32 - 1) as usize;
    unpack_px(&buf.rgba, cy * buf.width as usize + cx)
}

/// `SourceAlpha` blend of two RGBA8 buffers of the same size, straight on the bytes. It is the mode of
/// almost every layer, so it skips the float round trip of the generic path: an opaque backdrop
/// (the usual case) is a plain linear mix, anything else uses the same alpha-over formula as
/// [`CompositeMode::composite_rgba`]. The result matches the generic path to within one level.
fn blend_source_alpha(dst: &[u8], src: &[u8], opacity: f32) -> Vec<u8> {
    let mut out = Vec::with_capacity(dst.len());
    for (d, s) in dst
        .as_chunks::<4>()
        .0
        .iter()
        .zip(src.as_chunks::<4>().0.iter())
    {
        let eff = (f32::from(s[3]) * (1.0 / 255.0) * opacity).clamp(0.0, 1.0);
        if eff <= 0.0 {
            out.extend_from_slice(d);
            continue;
        }
        if d[3] == 255 {
            // Opaque backdrop: out = d + (s - d) * eff
            for c in 0..3 {
                let (dc, sc) = (f32::from(d[c]), f32::from(s[c]));
                out.push((dc + (sc - dc) * eff + 0.5).clamp(0.0, 255.0) as u8);
            }
            out.push(255);
        } else {
            let da = f32::from(d[3]) * (1.0 / 255.0);
            let out_a = eff + da * (1.0 - eff);
            if out_a <= 1e-6 {
                out.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }
            let keep = (1.0 - eff) * da;
            for c in 0..3 {
                let value = (eff * f32::from(s[c]) + keep * f32::from(d[c])) / out_a;
                out.push((value + 0.5).clamp(0.0, 255.0) as u8);
            }
            out.push((out_a * 255.0 + 0.5).clamp(0.0, 255.0) as u8);
        }
    }
    out
}

/// CPU reference blend of `src` over `dst` (`dst` and `src` may have different resolutions; `src`
/// is aspect-fit onto `dst`'s dimensions automatically).
pub fn blend_buffer_cpu(
    dst: &FrameBuffer,
    src: &FrameBuffer,
    mode: CompositeMode,
    opacity: f32,
) -> FrameBuffer {
    let op = if opacity.is_finite() {
        opacity.clamp(0.0, 1.0)
    } else {
        1.0
    };
    if op <= 0.0 || dst.width == 0 || dst.height == 0 {
        return dst.clone();
    }
    let fitted = fit_layer_to_canvas(src, dst.width, dst.height);
    let w = dst.width as i32;
    let h = dst.height as i32;
    let min_dim = w.min(h) as f32;
    let mut out = vec![0u8; dst.rgba.len()];

    if mode == CompositeMode::SourceAlpha {
        return FrameBuffer {
            width: dst.width,
            height: dst.height,
            rgba: blend_source_alpha(&dst.rgba, &fitted.rgba, op),
        };
    }

    if !mode.is_spatial() {
        let count = (dst.width * dst.height) as usize;
        for i in 0..count {
            let d = unpack_px(&dst.rgba, i);
            let s = unpack_px(&fitted.rgba, i);
            let res = mode.composite_rgba(d, s, op);
            let bytes = pack_px(res);
            let base = i * 4;
            out[base..base + 4].copy_from_slice(&bytes);
        }
        return FrameBuffer {
            width: dst.width,
            height: dst.height,
            rgba: out,
        };
    }

    for y in 0..h {
        for x in 0..w {
            let idx = (y as usize * dst.width as usize) + x as usize;
            let d = unpack_px(&dst.rgba, idx);
            let s = unpack_px(&fitted.rgba, idx);
            let eff = (s[3] * op).clamp(0.0, 1.0);
            let res = if eff <= 0.0 {
                d
            } else {
                match mode {
                    CompositeMode::DisplacementMap => {
                        let scale = 0.08 * min_dim * eff;
                        let dx = ((s[0] - 0.5) * 2.0 * scale + 0.5).floor() as i32;
                        let dy = ((s[1] - 0.5) * 2.0 * scale + 0.5).floor() as i32;
                        sample_clamped(dst, x + dx, y + dy)
                    }
                    CompositeMode::HeightMap => {
                        let gx = luma709(sample_clamped(&fitted, x + 1, y))
                            - luma709(sample_clamped(&fitted, x - 1, y));
                        let gy = luma709(sample_clamped(&fitted, x, y + 1))
                            - luma709(sample_clamped(&fitted, x, y - 1));
                        let scale = 0.06 * min_dim * eff;
                        let dx = (-gx * scale + 0.5).floor() as i32;
                        let dy = (-gy * scale + 0.5).floor() as i32;
                        let refracted = sample_clamped(dst, x + dx, y + dy);
                        let hi = (gx + gy) * 0.35 * eff;
                        [
                            (refracted[0] + hi).clamp(0.0, 1.0),
                            (refracted[1] + hi).clamp(0.0, 1.0),
                            (refracted[2] + hi).clamp(0.0, 1.0),
                            refracted[3],
                        ]
                    }
                    CompositeMode::BumpMap => {
                        let gx = luma709(sample_clamped(&fitted, x + 1, y))
                            - luma709(sample_clamped(&fitted, x - 1, y));
                        let gy = luma709(sample_clamped(&fitted, x, y + 1))
                            - luma709(sample_clamped(&fitted, x, y - 1));
                        let nx = -gx * 4.0;
                        let ny = -gy * 4.0;
                        let nz = 1.0_f32;
                        let inv_len = 1.0 / (nx * nx + ny * ny + nz * nz).sqrt();
                        // Normalized light vector (1, -1, 2) / sqrt(6); flat surface (0,0,1) gives shade = 1.0.
                        let dot_nl =
                            (nx * 0.408_248_3 + ny * -0.408_248_3 + nz * 0.816_496_6) * inv_len;
                        let shade = (dot_nl / 0.816_496_6).clamp(0.15, 1.85);
                        let lit_r = (d[0] * shade).clamp(0.0, 1.0);
                        let lit_g = (d[1] * shade).clamp(0.0, 1.0);
                        let lit_b = (d[2] * shade).clamp(0.0, 1.0);
                        [
                            d[0] + (lit_r - d[0]) * eff,
                            d[1] + (lit_g - d[1]) * eff,
                            d[2] + (lit_b - d[2]) * eff,
                            d[3],
                        ]
                    }
                    _ => mode.composite_rgba(d, s, op),
                }
            };
            let bytes = pack_px(res);
            let base = idx * 4;
            out[base..base + 4].copy_from_slice(&bytes);
        }
    }

    FrameBuffer {
        width: dst.width,
        height: dst.height,
        rgba: out,
    }
}

/// Evaluates a full recursive `CompositeNode` tree on the CPU starting from `backdrop`
/// (`[0, 0, 0, 255]` for the master timeline canvas, `[0, 0, 0, 0]` for parent/child sub-groups).
pub fn composite_nodes_cpu(
    canvas_w: u32,
    canvas_h: u32,
    backdrop: [u8; 4],
    nodes: &[CompositeNode],
) -> FrameBuffer {
    let cw = canvas_w.max(2);
    let ch = canvas_h.max(2);
    let mut canvas = FrameBuffer::solid(cw, ch, backdrop);
    for node in nodes {
        match node {
            CompositeNode::Layer(layer) => {
                canvas = blend_buffer_cpu(&canvas, &layer.frame, layer.mode, layer.opacity);
            }
            CompositeNode::Group {
                children,
                parent_layer,
                group_mode,
                group_opacity,
            } => {
                let mut sub = composite_nodes_cpu(cw, ch, [0, 0, 0, 0], children);
                if let Some(parent) = parent_layer {
                    sub = blend_buffer_cpu(&sub, &parent.frame, parent.mode, parent.opacity);
                }
                canvas = blend_buffer_cpu(&canvas, &sub, *group_mode, *group_opacity);
            }
        }
    }
    canvas
}

/// Convenience helper for flat bottom-to-top layer stacks over opaque black `[0, 0, 0, 255]`.
pub fn composite_layers_cpu(canvas_w: u32, canvas_h: u32, layers: &[DecodedLayer]) -> FrameBuffer {
    let cw = canvas_w.max(2);
    let ch = canvas_h.max(2);
    let mut canvas = FrameBuffer::solid(cw, ch, [0, 0, 0, 255]);
    for layer in layers {
        canvas = blend_buffer_cpu(&canvas, &layer.frame, layer.mode, layer.opacity);
    }
    canvas
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuCompositeParams {
    width: u32,
    height: u32,
    mode: u32,
    opacity: f32,
}

const COMPOSITE_WGSL: &str = r#"
struct Params {
    width: u32,
    height: u32,
    mode: u32,
    opacity: f32,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> dst_buf: array<u32>;
@group(0) @binding(2) var<storage, read> src_buf: array<u32>;
@group(0) @binding(3) var<storage, read_write> out_buf: array<u32>;

fn unpack_rgba8(v: u32) -> vec4<f32> {
    return vec4<f32>(
        f32(v & 0xffu) / 255.0,
        f32((v >> 8u) & 0xffu) / 255.0,
        f32((v >> 16u) & 0xffu) / 255.0,
        f32((v >> 24u) & 0xffu) / 255.0,
    );
}

fn pack_rgba8(px: vec4<f32>) -> u32 {
    let c = clamp(px, vec4<f32>(0.0), vec4<f32>(1.0));
    let r = u32(floor(c.r * 255.0 + 0.5));
    let g = u32(floor(c.g * 255.0 + 0.5));
    let b = u32(floor(c.b * 255.0 + 0.5));
    let a = u32(floor(c.a * 255.0 + 0.5));
    return (r & 0xffu) | ((g & 0xffu) << 8u) | ((b & 0xffu) << 16u) | ((a & 0xffu) << 24u);
}

fn luma709(px: vec4<f32>) -> f32 {
    return 0.2126 * px.r + 0.7152 * px.g + 0.0722 * px.b;
}

fn sample_dst(x: i32, y: i32) -> vec4<f32> {
    let cx = u32(clamp(x, 0, i32(params.width) - 1));
    let cy = u32(clamp(y, 0, i32(params.height) - 1));
    return unpack_rgba8(dst_buf[cy * params.width + cx]);
}

fn sample_src(x: i32, y: i32) -> vec4<f32> {
    let cx = u32(clamp(x, 0, i32(params.width) - 1));
    let cy = u32(clamp(y, 0, i32(params.height) - 1));
    return unpack_rgba8(src_buf[cy * params.width + cx]);
}

fn blend_channel(mode: u32, d: f32, s: f32) -> f32 {
    switch mode {
        case 0u: { return s; }
        case 1u: { return min(d + s, 1.0); }
        case 2u: { return max(d - s, 0.0); }
        case 3u: { return d * s; }
        case 4u: { return 1.0 - (1.0 - d) * (1.0 - s); }
        case 5u: {
            if d < 0.5 {
                return 2.0 * d * s;
            }
            return 1.0 - 2.0 * (1.0 - d) * (1.0 - s);
        }
        case 6u: {
            if s < 0.5 {
                return 2.0 * d * s;
            }
            return 1.0 - 2.0 * (1.0 - d) * (1.0 - s);
        }
        case 7u: {
            if s >= 0.9999 {
                return 1.0;
            }
            return min(d / (1.0 - s), 1.0);
        }
        case 8u: {
            if s <= 0.0001 {
                return 0.0;
            }
            return max(1.0 - (1.0 - d) / s, 0.0);
        }
        case 9u: { return min(d, s); }
        case 10u: { return max(d, s); }
        case 11u: { return abs(d - s); }
        case 12u: {
            let diff = d - s;
            return diff * diff;
        }
        default: { return s; }
    }
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height {
        return;
    }
    let idx = gid.y * params.width + gid.x;
    let op = clamp(params.opacity, 0.0, 1.0);
    let d = unpack_rgba8(dst_buf[idx]);
    if op <= 0.0 {
        out_buf[idx] = pack_rgba8(d);
        return;
    }
    let s = unpack_rgba8(src_buf[idx]);
    let x = i32(gid.x);
    let y = i32(gid.y);
    let min_dim = f32(min(params.width, params.height));

    var res = d;
    if params.mode == 13u {
        // Cut (stencil replace weighted by track opacity)
        res = d + (s - d) * op;
    } else if params.mode == 3u {
        // Multiply (Mask): multiplies RGB and destination alpha by premultiplied mask
        let masked = vec4<f32>(d.rgb * s.rgb * s.a, d.a * s.a);
        res = clamp(d + (masked - d) * op, vec4<f32>(0.0), vec4<f32>(1.0));
    } else if params.mode == 14u {
        // DisplacementMap
        let eff = clamp(s.a * op, 0.0, 1.0);
        if eff > 0.0 {
            let scale = 0.08 * min_dim * eff;
            let dx = i32(floor((s.r - 0.5) * 2.0 * scale + 0.5));
            let dy = i32(floor((s.g - 0.5) * 2.0 * scale + 0.5));
            res = sample_dst(x + dx, y + dy);
        }
    } else if params.mode == 15u {
        // HeightMap
        let eff = clamp(s.a * op, 0.0, 1.0);
        if eff > 0.0 {
            let gx = luma709(sample_src(x + 1, y)) - luma709(sample_src(x - 1, y));
            let gy = luma709(sample_src(x, y + 1)) - luma709(sample_src(x, y - 1));
            let scale = 0.06 * min_dim * eff;
            let dx = i32(floor(-gx * scale + 0.5));
            let dy = i32(floor(-gy * scale + 0.5));
            let refracted = sample_dst(x + dx, y + dy);
            let hi = (gx + gy) * 0.35 * eff;
            res = vec4<f32>(clamp(refracted.rgb + vec3<f32>(hi), vec3<f32>(0.0), vec3<f32>(1.0)), refracted.a);
        }
    } else if params.mode == 16u {
        // BumpMap
        let eff = clamp(s.a * op, 0.0, 1.0);
        if eff > 0.0 {
            let gx = luma709(sample_src(x + 1, y)) - luma709(sample_src(x - 1, y));
            let gy = luma709(sample_src(x, y + 1)) - luma709(sample_src(x, y - 1));
            let n = normalize(vec3<f32>(-gx * 4.0, -gy * 4.0, 1.0));
            let dot_nl = dot(n, vec3<f32>(0.4082483, -0.4082483, 0.8164966));
            let shade = clamp(dot_nl / 0.8164966, 0.15, 1.85);
            let lit = clamp(d.rgb * shade, vec3<f32>(0.0), vec3<f32>(1.0));
            res = vec4<f32>(d.rgb + (lit - d.rgb) * eff, d.a);
        }
    } else {
        let as_eff = clamp(s.a * op, 0.0, 1.0);
        if as_eff > 0.0 {
            let da = d.a;
            let out_a = as_eff + da * (1.0 - as_eff);
            if out_a <= 1e-6 {
                res = vec4<f32>(0.0);
            } else {
                let br = blend_channel(params.mode, d.r, s.r);
                let bg = blend_channel(params.mode, d.g, s.g);
                let bb = blend_channel(params.mode, d.b, s.b);
                let pr = as_eff * (1.0 - da) * s.r + as_eff * da * br + (1.0 - as_eff) * da * d.r;
                let pg = as_eff * (1.0 - da) * s.g + as_eff * da * bg + (1.0 - as_eff) * da * d.g;
                let pb = as_eff * (1.0 - da) * s.b + as_eff * da * bb + (1.0 - as_eff) * da * d.b;
                res = vec4<f32>(
                    clamp(pr / out_a, 0.0, 1.0),
                    clamp(pg / out_a, 0.0, 1.0),
                    clamp(pb / out_a, 0.0, 1.0),
                    clamp(out_a, 0.0, 1.0),
                );
            }
        }
    }

    out_buf[idx] = pack_rgba8(res);
}
"#;

/// Hardware/software `wgpu` compute compositor implementing all 17 `CompositeMode`s.
pub struct GpuCompositor {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    bgl: wgpu::BindGroupLayout,
    pipeline: wgpu::ComputePipeline,
}

impl GpuCompositor {
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("neoveg_composite_wgsl"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(COMPOSITE_WGSL)),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("neoveg_composite_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("neoveg_composite_layout"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("neoveg_composite_pipeline"),
            layout: Some(&layout),
            module: &shader,
            entry_point: "main",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        Self {
            device,
            queue,
            bgl,
            pipeline,
        }
    }

    /// The device and queue this compositor runs on, shared with the effect chain.
    pub fn device_queue(&self) -> (Arc<wgpu::Device>, Arc<wgpu::Queue>) {
        (Arc::clone(&self.device), Arc::clone(&self.queue))
    }

    /// Attempts to initialize a headless `wgpu` compute device (hardware first, then software
    /// fallback adapter if available). Returns `None` in environments without any Vulkan/DX12/Metal
    /// adapter so callers fall back transparently to `composite_nodes_cpu`.
    pub fn try_headless() -> Option<Self> {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .or_else(|| {
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: true,
            }))
        })?;

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("neoveg_headless_compositor"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .ok()?;
        Some(Self::new(Arc::new(device), Arc::new(queue)))
    }

    /// Blends `src` onto `dst` on the GPU using the WGSL compute pipeline and reads back the
    /// resulting `FrameBuffer`.
    pub fn blend_buffer_gpu(
        &self,
        dst: &FrameBuffer,
        src: &FrameBuffer,
        mode: CompositeMode,
        opacity: f32,
    ) -> Result<FrameBuffer, String> {
        let op = if opacity.is_finite() {
            opacity.clamp(0.0, 1.0)
        } else {
            1.0
        };
        if op <= 0.0 || dst.width == 0 || dst.height == 0 {
            return Ok(dst.clone());
        }
        let fitted = fit_layer_to_canvas(src, dst.width, dst.height);
        let byte_len = dst.rgba.len() as u64;
        if fitted.rgba.len() != dst.rgba.len() || byte_len == 0 {
            return Err("dimensões inválidas no compositor GPU".into());
        }

        let params = GpuCompositeParams {
            width: dst.width,
            height: dst.height,
            mode: mode.code(),
            opacity: op,
        };
        let params_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("composite_params"),
                contents: bytemuck::bytes_of(&params),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let dst_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("composite_dst"),
                contents: &dst.rgba,
                usage: wgpu::BufferUsages::STORAGE,
            });
        let src_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("composite_src"),
                contents: &fitted.rgba,
                usage: wgpu::BufferUsages::STORAGE,
            });
        let out_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("composite_out"),
            size: byte_len,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let staging_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("composite_staging"),
            size: byte_len,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("composite_bg"),
            layout: &self.bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: dst_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: src_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: out_buf.as_entire_binding(),
                },
            ],
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("composite_encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("composite_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.dispatch_workgroups(dst.width.div_ceil(8), dst.height.div_ceil(8), 1);
        }
        encoder.copy_buffer_to_buffer(&out_buf, 0, &staging_buf, 0, byte_len);
        self.queue.submit(Some(encoder.finish()));

        let slice = staging_buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .map_err(|e| format!("falha ao aguardar GPU: {e}"))?
            .map_err(|e| format!("falha ao mapear saída GPU: {e}"))?;

        let data = slice.get_mapped_range().to_vec();
        staging_buf.unmap();
        Ok(FrameBuffer {
            width: dst.width,
            height: dst.height,
            rgba: data,
        })
    }

    /// Evaluates a full recursive `CompositeNode` tree on the GPU.
    pub fn composite_nodes_gpu(
        &self,
        canvas_w: u32,
        canvas_h: u32,
        backdrop: [u8; 4],
        nodes: &[CompositeNode],
    ) -> Result<FrameBuffer, String> {
        let cw = canvas_w.max(2);
        let ch = canvas_h.max(2);
        let mut canvas = FrameBuffer::solid(cw, ch, backdrop);
        for node in nodes {
            match node {
                CompositeNode::Layer(layer) => {
                    canvas =
                        self.blend_buffer_gpu(&canvas, &layer.frame, layer.mode, layer.opacity)?;
                }
                CompositeNode::Group {
                    children,
                    parent_layer,
                    group_mode,
                    group_opacity,
                } => {
                    let mut sub = self.composite_nodes_gpu(cw, ch, [0, 0, 0, 0], children)?;
                    if let Some(parent) = parent_layer {
                        sub = self.blend_buffer_gpu(
                            &sub,
                            &parent.frame,
                            parent.mode,
                            parent.opacity,
                        )?;
                    }
                    canvas = self.blend_buffer_gpu(&canvas, &sub, *group_mode, *group_opacity)?;
                }
            }
        }
        Ok(canvas)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The byte-level `SourceAlpha` path must agree with the generic float path to within one level
    /// on every kind of pixel: opaque and see-through backdrops, any source alpha and opacity.
    #[test]
    fn the_source_alpha_fast_path_matches_the_generic_blend() {
        // A small deterministic generator, so a failure is reproducible
        let mut state = 0x2545_F491_4F6C_DD1D_u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 32) as u32
        };
        for opacity in [1.0_f32, 0.5, 0.25, 0.9] {
            let mut dst = Vec::new();
            let mut src = Vec::new();
            for i in 0..4_000 {
                let mut px = |alpha: Option<u8>| {
                    let v = next();
                    [
                        v as u8,
                        (v >> 8) as u8,
                        (v >> 16) as u8,
                        alpha.unwrap_or((v >> 24) as u8),
                    ]
                };
                // Mostly opaque backdrops, with transparent and half-transparent ones mixed in
                let backdrop = match i % 4 {
                    0 => px(Some(255)),
                    1 => px(Some(0)),
                    _ => px(None),
                };
                dst.extend(backdrop);
                src.extend(px(None));
            }
            let fast = blend_source_alpha(&dst, &src, opacity);
            for i in 0..4_000 {
                let reference = pack_px(CompositeMode::SourceAlpha.composite_rgba(
                    unpack_px(&dst, i),
                    unpack_px(&src, i),
                    opacity,
                ));
                for c in 0..4 {
                    let (a, b) = (fast[i * 4 + c], reference[c]);
                    // A fully transparent result has no colour: any value is the same picture
                    if c < 3 && reference[3] == 0 && fast[i * 4 + 3] == 0 {
                        continue;
                    }
                    assert!(
                        a.abs_diff(b) <= 1,
                        "pixel {i} channel {c} at opacity {opacity}: fast {a} vs reference {b} (dst {:?}, src {:?})",
                        &dst[i * 4..i * 4 + 4],
                        &src[i * 4..i * 4 + 4]
                    );
                }
            }
        }
    }

    #[test]
    fn an_opaque_layer_over_an_opaque_backdrop_replaces_it_and_zero_opacity_keeps_it() {
        let dst = FrameBuffer::solid(8, 8, [10, 20, 30, 255]);
        let src = FrameBuffer::solid(8, 8, [200, 100, 50, 255]);
        let top = blend_buffer_cpu(&dst, &src, CompositeMode::SourceAlpha, 1.0);
        assert_eq!(top.pixel(3, 3), [200, 100, 50, 255]);
        let half = blend_buffer_cpu(&dst, &src, CompositeMode::SourceAlpha, 0.5);
        assert_eq!(half.pixel(3, 3), [105, 60, 40, 255]);
        assert_eq!(
            blend_buffer_cpu(&dst, &src, CompositeMode::SourceAlpha, 0.0),
            dst
        );
    }

    fn test_pattern_dst(w: u32, h: u32) -> FrameBuffer {
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let r = ((x * 235) / w.max(1) + 12) as u8;
                let g = ((y * 220) / h.max(1) + 18) as u8;
                let b = (((x + y) * 190) / (w + h).max(1) + 30) as u8;
                let a = if (x + y) % 5 == 0 { 180 } else { 255 };
                rgba.extend_from_slice(&[r, g, b, a]);
            }
        }
        FrameBuffer {
            width: w,
            height: h,
            rgba,
        }
    }

    fn test_pattern_src(w: u32, h: u32) -> FrameBuffer {
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let cx = x as f32 / w.max(1) as f32 - 0.5;
                let cy = y as f32 / h.max(1) as f32 - 0.5;
                let radial = (1.0 - (cx.hypot(cy) * 1.8)).clamp(0.0, 1.0);
                let r = (radial * 240.0 + 8.0) as u8;
                let g = (((w - 1 - x) * 210) / w.max(1) + 20) as u8;
                let b = (((h - 1 - y) * 200) / h.max(1) + 25) as u8;
                let a = if x < 3 {
                    0
                } else if x < 8 {
                    140
                } else {
                    255
                };
                rgba.extend_from_slice(&[r, g, b, a]);
            }
        }
        FrameBuffer {
            width: w,
            height: h,
            rgba,
        }
    }

    #[test]
    fn parent_child_multiply_mask_isolates_child_leaving_background_intact() {
        let bg = Arc::new(FrameBuffer::solid(16, 16, [0, 40, 200, 255]));
        let child = Arc::new(FrameBuffer::solid(16, 16, [240, 20, 20, 255]));
        // Mask: opaque white inside [4..12, 4..12], transparent outside.
        let mut mask = FrameBuffer::solid(16, 16, [0, 0, 0, 0]);
        for y in 4..12u32 {
            for x in 4..12u32 {
                let i = ((y * 16 + x) * 4) as usize;
                mask.rgba[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
            }
        }

        let tree = vec![
            CompositeNode::Layer(DecodedLayer {
                frame: bg,
                mode: CompositeMode::SourceAlpha,
                opacity: 1.0,
            }),
            CompositeNode::Group {
                children: vec![CompositeNode::Layer(DecodedLayer {
                    frame: child,
                    mode: CompositeMode::SourceAlpha,
                    opacity: 1.0,
                })],
                parent_layer: Some(DecodedLayer {
                    frame: Arc::new(mask),
                    mode: CompositeMode::Multiply,
                    opacity: 1.0,
                }),
                group_mode: CompositeMode::SourceAlpha,
                group_opacity: 1.0,
            },
        ];

        let out = composite_nodes_cpu(16, 16, [0, 0, 0, 255], &tree);
        // Outside the mask: background blue stays untouched!
        assert_eq!(out.pixel(1, 1), [0, 40, 200, 255]);
        // Inside the mask: child red shows cleanly!
        assert_eq!(out.pixel(8, 8), [240, 20, 20, 255]);
    }

    #[test]
    fn gpu_compositor_matches_cpu_reference_across_all_17_modes() {
        let Some(gpu) = GpuCompositor::try_headless() else {
            // Headless container without Vulkan/DX12/Metal driver: CPU path already verified.
            return;
        };
        let dst = test_pattern_dst(32, 24);
        let src = test_pattern_src(32, 24);

        for mode in CompositeMode::ALL {
            for opacity in [1.0_f32, 0.62_f32] {
                let cpu_out = blend_buffer_cpu(&dst, &src, mode, opacity);
                let gpu_out = gpu
                    .blend_buffer_gpu(&dst, &src, mode, opacity)
                    .expect("GPU blend must succeed");
                assert_eq!(cpu_out.rgba.len(), gpu_out.rgba.len());
                let max_diff = cpu_out
                    .rgba
                    .iter()
                    .zip(gpu_out.rgba.iter())
                    .map(|(a, b)| a.abs_diff(*b))
                    .max()
                    .unwrap_or(0);
                assert!(
                    max_diff <= 2,
                    "GPU vs CPU mismatch for mode {mode:?} (opacity={opacity}): max_diff={max_diff}"
                );
            }
        }
    }
}
