//! GPU version of the video effect chain (`wgpu` compute shaders).
//!
//! The picture is uploaded once as premultiplied floats, the chain runs as a series of compute
//! passes that ping-pong between three buffers, and the result is read back once. The maths is the
//! one of the CPU reference in [`crate::vfx`] (same kernels, same vignette formula), and a test
//! compares the two. Pictures that do not fit the device's buffer limits return an error so the
//! caller falls back to the CPU.

use std::borrow::Cow;
use std::sync::Arc;

use wgpu::util::DeviceExt;

use crate::frame::FrameBuffer;
use crate::transitions as tr;
use crate::vfx::{
    contrast_gain, gaussian_kernel, pixelate_block, range_pixels, vignette_corner, Resolved, Work,
    UNSHARP_RADIUS,
};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    width: u32,
    height: u32,
    half: u32,
    _pad: u32,
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    e: f32,
    f: f32,
    g: f32,
    h: f32,
}

impl Params {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            half: 0,
            _pad: 0,
            a: 0.0,
            b: 0.0,
            c: 0.0,
            d: 0.0,
            e: 0.0,
            f: 0.0,
            g: 0.0,
            h: 0.0,
        }
    }
}

const WGSL: &str = r#"
struct Params {
    width: u32,
    height: u32,
    half: u32,
    pad: u32,
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    e: f32,
    f: f32,
    g: f32,
    h: f32,
};

@group(0) @binding(0) var<uniform> p: Params;
@group(0) @binding(1) var<storage, read> src: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> dst: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read> weights: array<f32>;
@group(0) @binding(4) var<storage, read> aux: array<vec4<f32>>;

fn at(x: i32, y: i32) -> vec4<f32> {
    let cx = clamp(x, 0, i32(p.width) - 1);
    let cy = clamp(y, 0, i32(p.height) - 1);
    return src[u32(cy) * p.width + u32(cx)];
}

fn clamp_pm(v: vec4<f32>) -> vec4<f32> {
    let a = clamp(v.w, 0.0, 1.0);
    return vec4<f32>(clamp(v.x, 0.0, a), clamp(v.y, 0.0, a), clamp(v.z, 0.0, a), a);
}

@compute @workgroup_size(8, 8, 1)
fn blur_h(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    var acc = vec4<f32>(0.0);
    let n = i32(2u * p.half + 1u);
    for (var k = 0; k < n; k = k + 1) {
        acc = acc + weights[k] * at(i32(id.x) + k - i32(p.half), i32(id.y));
    }
    dst[id.y * p.width + id.x] = acc;
}

@compute @workgroup_size(8, 8, 1)
fn blur_v(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    var acc = vec4<f32>(0.0);
    let n = i32(2u * p.half + 1u);
    for (var k = 0; k < n; k = k + 1) {
        acc = acc + weights[k] * at(i32(id.x), i32(id.y) + k - i32(p.half));
    }
    dst[id.y * p.width + id.x] = acc;
}

@compute @workgroup_size(8, 8, 1)
fn sharpen(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let x = i32(id.x);
    let y = i32(id.y);
    let v = 5.0 * at(x, y) - at(x - 1, y) - at(x + 1, y) - at(x, y - 1) - at(x, y + 1);
    dst[id.y * p.width + id.x] = clamp_pm(v);
}

// src = blurred picture, aux = original picture, a = ratio
@compute @workgroup_size(8, 8, 1)
fn unsharp_mix(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    let o = aux[i];
    let b = src[i];
    let v = o + p.a * (o - b);
    dst[i] = clamp_pm(vec4<f32>(v.x, v.y, v.z, o.w));
}

// a = outer, b = inner, c = aspect, d/e = centre, f = corner distance, g = frame aspect ratio
@compute @workgroup_size(8, 8, 1)
fn vignette(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    let nx = (f32(id.x) + 0.5) / f32(p.width) - p.d;
    let ny = 1.0 - (f32(id.y) + 0.5) / f32(p.height) - p.e;
    let r = sqrt(pow((nx * p.g) / p.c, 2.0) + ny * ny) / p.f;
    let t = clamp((r - p.b) / (p.a - p.b), 0.0, 1.0);
    let gain = 1.0 - t * t * (3.0 - 2.0 * t);
    let s = src[i];
    dst[i] = vec4<f32>(s.x * gain, s.y * gain, s.z * gain, s.w);
}

// a = amount
@compute @workgroup_size(8, 8, 1)
fn black_and_white(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    let s = src[i];
    let l = s.x * 0.2126 + s.y * 0.7152 + s.z * 0.0722;
    dst[i] = vec4<f32>(s.x + p.a * (l - s.x), s.y + p.a * (l - s.y), s.z + p.a * (l - s.z), s.w);
}

// a = amount
@compute @workgroup_size(8, 8, 1)
fn invert(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    let s = src[i];
    let n = vec3<f32>(s.w - s.x, s.w - s.y, s.w - s.z);
    let c = s.xyz + p.a * (n - s.xyz);
    dst[i] = vec4<f32>(c.x, c.y, c.z, s.w);
}

// a = brightness, b = contrast gain
@compute @workgroup_size(8, 8, 1)
fn brightness_contrast(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    let s = src[i];
    if (s.w <= 0.000001) {
        dst[i] = s;
        return;
    }
    let v = clamp((s.xyz / s.w - vec3<f32>(0.5)) * p.b + vec3<f32>(0.5 + p.a), vec3<f32>(0.0), vec3<f32>(1.0));
    dst[i] = vec4<f32>(v.x * s.w, v.y * s.w, v.z * s.w, s.w);
}

// a = block width, b = block height (pixels)
@compute @workgroup_size(8, 8, 1)
fn pixelate(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let bw = u32(p.a);
    let bh = u32(p.b);
    let sx = min(id.x / bw * bw + bw / 2u, p.width - 1u);
    let sy = min(id.y / bh * bh + bh / 2u, p.height - 1u);
    dst[id.y * p.width + id.x] = src[sy * p.width + sx];
}

fn sample_bilinear(x: f32, y: f32) -> vec4<f32> {
    let fx = x - 0.5;
    let fy = y - 0.5;
    let x0 = floor(fx);
    let y0 = floor(fy);
    let tx = fx - x0;
    let ty = fy - y0;
    var acc = vec4<f32>(0.0);
    for (var j = 0; j < 2; j = j + 1) {
        for (var i = 0; i < 2; i = i + 1) {
            let px = i32(x0) + i;
            let py = i32(y0) + j;
            if (px < 0 || py < 0 || px >= i32(p.width) || py >= i32(p.height)) { continue; }
            var wx = 1.0 - tx;
            if (i == 1) { wx = tx; }
            var wy = 1.0 - ty;
            if (j == 1) { wy = ty; }
            acc = acc + wx * wy * src[u32(py) * p.width + u32(px)];
        }
    }
    return acc;
}

// a = angle, b/c = centre (fractions of the frame, lower-left origin)
@compute @workgroup_size(8, 8, 1)
fn mirror(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let w = f32(p.width);
    let h = f32(p.height);
    let r = radians(p.a);
    let n = vec2<f32>(-sin(r), cos(r));
    let q = vec2<f32>(f32(id.x) + 0.5, h - (f32(id.y) + 0.5));
    let side = dot(q - vec2<f32>(p.b * w, p.c * h), n);
    let i = id.y * p.width + id.x;
    if (side >= 0.0) {
        dst[i] = src[i];
    } else {
        let rr = q - 2.0 * side * n;
        dst[i] = sample_bilinear(rr.x, h - rr.y);
    }
}

// Transitions. a = progress, b = frame aspect ratio, c = angle (start angle for the clock wipe),
// d = softness, e = blind count, half = mode (0 dissolve, 1 linear wipe, 2 clock wipe, 3 iris,
// 4 barn door, 5 blinds)
fn reveal(s: f32, progress: f32, soft: f32) -> f32 {
    let sf = max(soft, 0.002);
    let t = clamp((progress * (1.0 + sf) - s) / sf, 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

fn along(u: f32, v: f32, aspect: f32, degrees: f32) -> f32 {
    let r = radians(degrees);
    let sn = sin(r);
    let cs = cos(r);
    let c0 = 0.0;
    let c1 = aspect * cs;
    let c2 = sn;
    let c3 = aspect * cs + sn;
    let lo = min(min(c0, c1), min(c2, c3));
    let hi = max(max(c0, c1), max(c2, c3));
    if (hi - lo < 0.000001) { return 0.0; }
    return clamp((u * aspect * cs + v * sn - lo) / (hi - lo), 0.0, 1.0);
}

@compute @workgroup_size(8, 8, 1)
fn trans_mask(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let u = (f32(id.x) + 0.5) / f32(p.width);
    let v = (f32(id.y) + 0.5) / f32(p.height);
    let aspect = p.b;
    var m = p.a;
    if (p.half == 1u) {
        m = reveal(along(u, v, aspect, p.c), p.a, p.d);
    } else if (p.half == 2u) {
        let dx = (u - 0.5) * aspect;
        let dy = v - 0.5;
        let turn = atan2(dx, -dy) / 6.283185307179586;
        let q = turn - p.c / 360.0;
        m = reveal(q - floor(q), p.a, p.d);
    } else if (p.half == 3u) {
        let r = length(vec2<f32>((u - 0.5) * aspect, v - 0.5)) / length(vec2<f32>(aspect / 2.0, 0.5));
        m = reveal(r, p.a, p.d);
    } else if (p.half == 4u) {
        m = reveal(abs(along(u, v, aspect, p.c) - 0.5) * 2.0, p.a, p.d);
    } else if (p.half == 5u) {
        m = reveal(fract(along(u, v, aspect, p.c) * p.e), p.a, p.d);
    }
    let i = id.y * p.width + id.x;
    dst[i] = src[i] * m;
}

// c = cosine and d = sine of the angle (computed on the CPU: a GPU cosine of 0 is not exactly 1,
// and at the edge of the frame that leaks a sliver of the picture)
@compute @workgroup_size(8, 8, 1)
fn trans_slide(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let ox = -p.c * (1.0 - p.a) * f32(p.width);
    let oy = -p.d * (1.0 - p.a) * f32(p.height);
    dst[id.y * p.width + id.x] = sample_bilinear(f32(id.x) + 0.5 - ox, f32(id.y) + 0.5 - oy);
}

@compute @workgroup_size(8, 8, 1)
fn trans_grow(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    if (p.a <= 0.001) {
        dst[i] = vec4<f32>(0.0);
        return;
    }
    let cx = f32(p.width) / 2.0;
    let cy = f32(p.height) / 2.0;
    dst[i] = sample_bilinear(cx + (f32(id.x) + 0.5 - cx) / p.a, cy + (f32(id.y) + 0.5 - cy) / p.a);
}

// a = progress, half = edge (0 left, 1 top, 2 right, 3 bottom)
@compute @workgroup_size(8, 8, 1)
fn trans_squeeze(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    if (p.a <= 0.001) {
        dst[i] = vec4<f32>(0.0);
        return;
    }
    let w = f32(p.width);
    let h = f32(p.height);
    let px = f32(id.x) + 0.5;
    let py = f32(id.y) + 0.5;
    if (p.half == 0u) {
        dst[i] = sample_bilinear(px / p.a, py);
    } else if (p.half == 1u) {
        dst[i] = sample_bilinear(px, py / p.a);
    } else if (p.half == 2u) {
        dst[i] = sample_bilinear(w - (w - px) / p.a, py);
    } else {
        dst[i] = sample_bilinear(px, h - (h - py) / p.a);
    }
}

// c = cosine, d = sine of the turn left, e = scale (computed on the CPU)
@compute @workgroup_size(8, 8, 1)
fn trans_spiral(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    if (p.e <= 0.001) {
        dst[i] = vec4<f32>(0.0);
        return;
    }
    let cx = f32(p.width) / 2.0;
    let cy = f32(p.height) / 2.0;
    let dx = f32(id.x) + 0.5 - cx;
    let dy = f32(id.y) + 0.5 - cy;
    dst[i] = sample_bilinear(cx + (p.c * dx + p.d * dy) / p.e, cy + (p.c * dy - p.d * dx) / p.e);
}

// a = progress, b = flash strength, c/d/e = flash colour
@compute @workgroup_size(8, 8, 1)
fn trans_flash(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    let flash = vec4<f32>(p.c * p.b, p.d * p.b, p.e * p.b, p.b);
    dst[i] = flash + src[i] * (p.a * (1.0 - p.b));
}
"#;

/// The compute pipelines of the effect chain, built once per device.
pub struct VfxGpu {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    bgl: wgpu::BindGroupLayout,
    blur_h: wgpu::ComputePipeline,
    blur_v: wgpu::ComputePipeline,
    sharpen: wgpu::ComputePipeline,
    unsharp_mix: wgpu::ComputePipeline,
    vignette: wgpu::ComputePipeline,
    black_and_white: wgpu::ComputePipeline,
    invert: wgpu::ComputePipeline,
    brightness_contrast: wgpu::ComputePipeline,
    pixelate: wgpu::ComputePipeline,
    mirror: wgpu::ComputePipeline,
    trans_mask: wgpu::ComputePipeline,
    trans_slide: wgpu::ComputePipeline,
    trans_grow: wgpu::ComputePipeline,
    trans_squeeze: wgpu::ComputePipeline,
    trans_spiral: wgpu::ComputePipeline,
    trans_flash: wgpu::ComputePipeline,
}

fn storage_entry(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

/// Which pipeline a pass runs.
#[derive(Clone, Copy)]
enum Pass {
    BlurH,
    BlurV,
    Sharpen,
    UnsharpMix,
    Vignette,
    BlackAndWhite,
    Invert,
    BrightnessContrast,
    Pixelate,
    Mirror,
    TransMask,
    TransSlide,
    TransGrow,
    TransSqueeze,
    TransSpiral,
    TransFlash,
}

/// One step of a GPU run: an effect, or a transition at a progress.
#[derive(Clone, Copy)]
enum Op {
    Effect(Resolved),
    Transition(tr::Resolved, f32),
}

impl VfxGpu {
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("neoveg_vfx_wgsl"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(WGSL)),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("neoveg_vfx_bgl"),
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
                storage_entry(1, true),
                storage_entry(2, false),
                storage_entry(3, true),
                storage_entry(4, true),
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("neoveg_vfx_layout"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });
        let make = |entry: &str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(&layout),
                module: &shader,
                entry_point: entry,
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            })
        };
        Self {
            blur_h: make("blur_h"),
            blur_v: make("blur_v"),
            sharpen: make("sharpen"),
            unsharp_mix: make("unsharp_mix"),
            vignette: make("vignette"),
            black_and_white: make("black_and_white"),
            invert: make("invert"),
            brightness_contrast: make("brightness_contrast"),
            pixelate: make("pixelate"),
            mirror: make("mirror"),
            trans_mask: make("trans_mask"),
            trans_slide: make("trans_slide"),
            trans_grow: make("trans_grow"),
            trans_squeeze: make("trans_squeeze"),
            trans_spiral: make("trans_spiral"),
            trans_flash: make("trans_flash"),
            device,
            queue,
            bgl,
        }
    }

    fn pipeline(&self, pass: Pass) -> &wgpu::ComputePipeline {
        match pass {
            Pass::BlurH => &self.blur_h,
            Pass::BlurV => &self.blur_v,
            Pass::Sharpen => &self.sharpen,
            Pass::UnsharpMix => &self.unsharp_mix,
            Pass::Vignette => &self.vignette,
            Pass::BlackAndWhite => &self.black_and_white,
            Pass::Invert => &self.invert,
            Pass::BrightnessContrast => &self.brightness_contrast,
            Pass::Pixelate => &self.pixelate,
            Pass::Mirror => &self.mirror,
            Pass::TransMask => &self.trans_mask,
            Pass::TransSlide => &self.trans_slide,
            Pass::TransGrow => &self.trans_grow,
            Pass::TransSqueeze => &self.trans_squeeze,
            Pass::TransSpiral => &self.trans_spiral,
            Pass::TransFlash => &self.trans_flash,
        }
    }

    /// Runs the resolved chain on `frame` and returns the result. `Err` when the picture does not
    /// fit the device's limits or the read-back fails; the caller then uses the CPU.
    pub fn process(&self, frame: &FrameBuffer, chain: &[Resolved]) -> Result<FrameBuffer, String> {
        let ops: Vec<Op> = chain.iter().map(|e| Op::Effect(*e)).collect();
        self.process_ops(frame, &ops)
    }

    /// The layer as it looks at `progress` of the transition `t` (the same picture as
    /// [`tr::apply`]). `Err` as for [`VfxGpu::process`].
    pub fn process_transition(
        &self,
        frame: &FrameBuffer,
        t: &tr::Resolved,
        progress: f32,
    ) -> Result<FrameBuffer, String> {
        let progress = if progress.is_finite() {
            progress.clamp(0.0, 1.0)
        } else {
            1.0
        };
        if progress >= 1.0 {
            return Ok(frame.clone());
        }
        self.process_ops(frame, &[Op::Transition(*t, progress)])
    }

    fn process_ops(&self, frame: &FrameBuffer, ops: &[Op]) -> Result<FrameBuffer, String> {
        if ops.is_empty() {
            return Ok(frame.clone());
        }
        let (w, h) = (frame.width, frame.height);
        if w == 0 || h == 0 {
            return Ok(frame.clone());
        }
        let bytes = u64::from(w) * u64::from(h) * 16;
        let limits = self.device.limits();
        if bytes > u64::from(limits.max_storage_buffer_binding_size)
            || bytes > limits.max_buffer_size
        {
            return Err("quadro grande demais para os efeitos na GPU".into());
        }

        let work = Work::from_frame(frame);
        let usage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC;
        let first = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("vfx_a"),
                contents: bytemuck::cast_slice(&work.px),
                usage,
            });
        let make_empty = |label| {
            self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: bytes,
                usage,
                mapped_at_creation: false,
            })
        };
        let bufs = [first, make_empty("vfx_b"), make_empty("vfx_c")];
        // Never bound for reading and writing in the same pass, so a tiny buffer stands in for an
        // input a pass does not use.
        let dummy = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("vfx_dummy"),
                contents: &[0u8; 16],
                usage: wgpu::BufferUsages::STORAGE,
            });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("vfx_encoder"),
            });
        let mut cur = 0usize;
        let run = |encoder: &mut wgpu::CommandEncoder,
                   pass: Pass,
                   params: Params,
                   src: usize,
                   dst: usize,
                   weights: Option<&[f32]>,
                   aux: Option<usize>| {
            let params_buf = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("vfx_params"),
                    contents: bytemuck::bytes_of(&params),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
            let weights_buf = weights.map(|w| {
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("vfx_weights"),
                        contents: bytemuck::cast_slice(w),
                        usage: wgpu::BufferUsages::STORAGE,
                    })
            });
            let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("vfx_bg"),
                layout: &self.bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: params_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: bufs[src].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: bufs[dst].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: weights_buf
                            .as_ref()
                            .map_or_else(|| dummy.as_entire_binding(), |b| b.as_entire_binding()),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: aux.map_or_else(
                            || dummy.as_entire_binding(),
                            |i| bufs[i].as_entire_binding(),
                        ),
                    },
                ],
            });
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("vfx_pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(self.pipeline(pass));
            cpass.set_bind_group(0, &bg, &[]);
            cpass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
        };

        for op in ops {
            let effect = match *op {
                Op::Effect(e) => e,
                Op::Transition(t, progress) => {
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    p.a = progress;
                    p.b = w as f32 / h as f32;
                    let pass = match t {
                        tr::Resolved::Slide { angle } => {
                            (p.d, p.c) = angle.to_radians().sin_cos();
                            Pass::TransSlide
                        }
                        tr::Resolved::Grow => Pass::TransGrow,
                        tr::Resolved::Squeeze { angle } => {
                            p.half = tr::squeeze_edge(angle);
                            Pass::TransSqueeze
                        }
                        tr::Resolved::Spiral {
                            turns,
                            zoom,
                            clockwise,
                        } => {
                            (p.c, p.d, p.e) =
                                tr::spiral_transform(turns, zoom, clockwise, progress);
                            Pass::TransSpiral
                        }
                        tr::Resolved::Flash { color } => {
                            p.b = tr::flash_strength(progress);
                            (p.c, p.d, p.e) = (color[0], color[1], color[2]);
                            Pass::TransFlash
                        }
                        tr::Resolved::Dissolve => Pass::TransMask,
                        tr::Resolved::LinearWipe { angle, softness } => {
                            (p.half, p.c, p.d) = (1, angle, softness);
                            Pass::TransMask
                        }
                        tr::Resolved::ClockWipe { start, softness } => {
                            (p.half, p.c, p.d) = (2, start, softness);
                            Pass::TransMask
                        }
                        tr::Resolved::Iris { softness } => {
                            (p.half, p.d) = (3, softness);
                            Pass::TransMask
                        }
                        tr::Resolved::BarnDoor { angle, softness } => {
                            (p.half, p.c, p.d) = (4, angle, softness);
                            Pass::TransMask
                        }
                        tr::Resolved::Blinds {
                            count,
                            angle,
                            softness,
                        } => {
                            (p.half, p.c, p.d, p.e) = (5, angle, softness, count);
                            Pass::TransMask
                        }
                    };
                    run(&mut encoder, pass, p, cur, dst, None, None);
                    cur = dst;
                    continue;
                }
            };
            match effect {
                Resolved::GaussianBlur { h_range, v_range } => {
                    for (range, pass) in [(h_range, Pass::BlurH), (v_range, Pass::BlurV)] {
                        if let Some(k) = gaussian_kernel(range_pixels(range, h)) {
                            let dst = (cur + 1) % 3;
                            let mut p = Params::new(w, h);
                            p.half = (k.len() / 2) as u32;
                            run(&mut encoder, pass, p, cur, dst, Some(&k), None);
                            cur = dst;
                        }
                    }
                }
                Resolved::Sharpen => {
                    let dst = (cur + 1) % 3;
                    run(
                        &mut encoder,
                        Pass::Sharpen,
                        Params::new(w, h),
                        cur,
                        dst,
                        None,
                        None,
                    );
                    cur = dst;
                }
                Resolved::UnsharpMask { ratio } => {
                    let Some(k) = gaussian_kernel(range_pixels(UNSHARP_RADIUS * 2.0, h)) else {
                        continue;
                    };
                    let (tmp, blurred) = ((cur + 1) % 3, (cur + 2) % 3);
                    let mut p = Params::new(w, h);
                    p.half = (k.len() / 2) as u32;
                    run(&mut encoder, Pass::BlurH, p, cur, tmp, Some(&k), None);
                    run(&mut encoder, Pass::BlurV, p, tmp, blurred, Some(&k), None);
                    let mut mix = Params::new(w, h);
                    mix.a = ratio;
                    // The free buffer `tmp` takes the result; `cur` is still the original (aux)
                    run(
                        &mut encoder,
                        Pass::UnsharpMix,
                        mix,
                        blurred,
                        tmp,
                        None,
                        Some(cur),
                    );
                    cur = tmp;
                }
                Resolved::Vignette {
                    outer,
                    inner,
                    aspect,
                    center,
                } => {
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    p.a = outer;
                    p.b = inner;
                    p.c = aspect;
                    p.d = center[0];
                    p.e = center[1];
                    p.f = vignette_corner(w, h);
                    p.g = w as f32 / h as f32;
                    run(&mut encoder, Pass::Vignette, p, cur, dst, None, None);
                    cur = dst;
                }
                Resolved::BlackAndWhite { amount } | Resolved::Invert { amount } => {
                    let pass = if matches!(effect, Resolved::Invert { .. }) {
                        Pass::Invert
                    } else {
                        Pass::BlackAndWhite
                    };
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    p.a = amount;
                    run(&mut encoder, pass, p, cur, dst, None, None);
                    cur = dst;
                }
                Resolved::BrightnessContrast {
                    brightness,
                    contrast,
                } => {
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    p.a = brightness;
                    p.b = contrast_gain(contrast);
                    run(
                        &mut encoder,
                        Pass::BrightnessContrast,
                        p,
                        cur,
                        dst,
                        None,
                        None,
                    );
                    cur = dst;
                }
                Resolved::Pixelate { h_size, v_size } => {
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    p.a = pixelate_block(h_size, h) as f32;
                    p.b = pixelate_block(v_size, h) as f32;
                    run(&mut encoder, Pass::Pixelate, p, cur, dst, None, None);
                    cur = dst;
                }
                Resolved::Mirror { angle, center } => {
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    p.a = angle;
                    p.b = center[0];
                    p.c = center[1];
                    run(&mut encoder, Pass::Mirror, p, cur, dst, None, None);
                    cur = dst;
                }
            }
        }

        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vfx_staging"),
            size: bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        encoder.copy_buffer_to_buffer(&bufs[cur], 0, &staging, 0, bytes);
        self.queue.submit(Some(encoder.finish()));

        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .map_err(|e| format!("falha ao aguardar a GPU: {e}"))?
            .map_err(|e| format!("falha ao mapear a saída da GPU: {e}"))?;
        let mapped = slice.get_mapped_range();
        let floats: &[[f32; 4]] = bytemuck::cast_slice(&mapped);
        let out = Work {
            width: w as usize,
            height: h as usize,
            px: floats.to_vec(),
        };
        drop(mapped);
        staging.unmap();

        let mut result = frame.clone();
        out.write_to(&mut result);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composite::GpuCompositor;
    use crate::generators::smpte_color_bars;
    use crate::vfx::{
        apply_chain_cpu, resolve_chain, VideoEffect, BLACK_AND_WHITE, BRIGHTNESS_CONTRAST,
        GAUSSIAN_BLUR, INVERT, MIRROR, PIXELATE, SHARPEN, UNSHARP_MASK, VIGNETTE,
    };

    fn gpu() -> Option<VfxGpu> {
        let comp = GpuCompositor::try_headless()?;
        let (device, queue) = comp.device_queue();
        Some(VfxGpu::new(device, queue))
    }

    fn max_difference(a: &FrameBuffer, b: &FrameBuffer) -> u8 {
        a.rgba
            .iter()
            .zip(&b.rgba)
            .map(|(x, y)| x.abs_diff(*y))
            .max()
            .unwrap_or(0)
    }

    #[test]
    fn the_gpu_chain_matches_the_cpu_reference() {
        let Some(gpu) = gpu() else {
            eprintln!("no graphics adapter: GPU effect parity test skipped");
            return;
        };
        let chain = [
            VideoEffect::new(GAUSSIAN_BLUR)
                .with("h_range", 0.02)
                .with("v_range", 0.01),
            VideoEffect::new(SHARPEN),
            VideoEffect::new(UNSHARP_MASK).with("ratio", 1.5),
            VideoEffect::new(VIGNETTE).with("center_x", 0.3),
            VideoEffect::new(BRIGHTNESS_CONTRAST).with("contrast", 0.3),
            VideoEffect::new(PIXELATE).with("h_size", 0.02),
            VideoEffect::new(INVERT).with("amount", 0.5),
            VideoEffect::new(BLACK_AND_WHITE),
        ];
        let original = smpte_color_bars(320, 180);
        let mut cpu = original.clone();
        apply_chain_cpu(&mut cpu, &chain);
        let on_gpu = gpu
            .process(&original, &resolve_chain(&chain))
            .expect("gpu chain");
        assert_ne!(on_gpu, original);
        let diff = max_difference(&cpu, &on_gpu);
        assert!(diff <= 2, "largest per-channel difference {diff}");
    }

    #[test]
    fn each_effect_alone_matches_on_a_picture_with_transparency() {
        let Some(gpu) = gpu() else {
            eprintln!("no graphics adapter: GPU effect parity test skipped");
            return;
        };
        let mut original = smpte_color_bars(200, 120);
        for (i, p) in original.rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            if (i % 200) < 60 {
                p[3] = 90;
            }
        }
        for fx in [
            VideoEffect::new(GAUSSIAN_BLUR).with("h_range", 0.03),
            VideoEffect::new(SHARPEN),
            VideoEffect::new(UNSHARP_MASK),
            VideoEffect::new(VIGNETTE),
            VideoEffect::new(BLACK_AND_WHITE).with("amount", 0.7),
            VideoEffect::new(INVERT),
            VideoEffect::new(BRIGHTNESS_CONTRAST)
                .with("brightness", 0.1)
                .with("contrast", 0.4),
            VideoEffect::new(BRIGHTNESS_CONTRAST).with("contrast", -0.5),
            VideoEffect::new(PIXELATE)
                .with("h_size", 0.07)
                .with("v_size", 0.03),
            VideoEffect::new(MIRROR),
            VideoEffect::new(MIRROR)
                .with("angle", 35.0)
                .with("center_x", 0.4),
            VideoEffect::new(MIRROR)
                .with("angle", 270.0)
                .with("center_y", 0.7),
        ] {
            let chain = [fx];
            let mut cpu = original.clone();
            apply_chain_cpu(&mut cpu, &chain);
            let on_gpu = gpu
                .process(&original, &resolve_chain(&chain))
                .expect("gpu chain");
            let diff = max_difference(&cpu, &on_gpu);
            assert!(diff <= 2, "{}: difference {diff}", chain[0].kind);
        }
    }

    #[test]
    fn every_transition_matches_the_cpu_on_a_picture_with_transparency() {
        let Some(gpu) = gpu() else {
            eprintln!("no graphics adapter: GPU transition parity test skipped");
            return;
        };
        let mut original = smpte_color_bars(160, 90);
        for (i, p) in original.rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            if (i % 160) < 30 {
                p[3] = 120;
            }
        }
        for spec in tr::transitions() {
            let t = tr::resolve(spec.id, &std::collections::BTreeMap::new()).expect("resolves");
            for progress in [0.0, 0.2, 0.55, 0.9] {
                let cpu = tr::apply(&original, &t, progress);
                let on_gpu = gpu
                    .process_transition(&original, &t, progress)
                    .expect("gpu transition");
                let diff = max_difference(&cpu, &on_gpu);
                assert!(diff <= 3, "{} at {progress}: difference {diff}", spec.id);
            }
        }
        // The end of a transition is the layer untouched
        let t = tr::resolve(tr::IRIS, &std::collections::BTreeMap::new()).unwrap();
        assert_eq!(
            gpu.process_transition(&original, &t, 1.0).unwrap(),
            original
        );
    }

    #[test]
    fn an_empty_chain_returns_the_frame_and_a_huge_frame_is_refused() {
        let Some(gpu) = gpu() else {
            eprintln!("no graphics adapter: GPU effect test skipped");
            return;
        };
        let frame = smpte_color_bars(64, 36);
        assert_eq!(gpu.process(&frame, &[]).unwrap(), frame);
        let limit = u64::from(gpu.device.limits().max_storage_buffer_binding_size);
        let side = ((limit / 16) as f64).sqrt() as u32 + 64;
        let big = FrameBuffer {
            width: side,
            height: side,
            rgba: Vec::new(),
        };
        let chain = resolve_chain(&[VideoEffect::new(SHARPEN)]);
        assert!(gpu.process(&big, &chain).is_err());
    }
}
