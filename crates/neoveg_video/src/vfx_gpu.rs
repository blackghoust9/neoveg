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
use crate::vfx::{gaussian_kernel, range_pixels, vignette_corner, Resolved, Work, UNSHARP_RADIUS};

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
        }
    }

    /// Runs the resolved chain on `frame` and returns the result. `Err` when the picture does not
    /// fit the device's limits or the read-back fails; the caller then uses the CPU.
    pub fn process(&self, frame: &FrameBuffer, chain: &[Resolved]) -> Result<FrameBuffer, String> {
        if chain.is_empty() {
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

        for effect in chain {
            match *effect {
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
        apply_chain_cpu, resolve_chain, VideoEffect, GAUSSIAN_BLUR, SHARPEN, UNSHARP_MASK, VIGNETTE,
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
