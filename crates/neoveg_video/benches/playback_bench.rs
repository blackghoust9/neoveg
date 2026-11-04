//! Per-frame cost of the video playback path at the resolutions editors work in, 480p to 8K UHD.
//!
//! What is measured: the CPU work Neoveg does on ONE already decoded RGBA8 frame, single-threaded:
//!
//! * `generate`: building a frame (the SMPTE colour-bars generator, a stand-in for a decoded frame),
//! * `grade`: the primary colour correction applied to the preview,
//! * `scopes`: the video scopes (histogram, waveform, vectorscope) of the frame,
//! * `crossfade`: compositing two full-frame layers (a transition) over a black canvas,
//! * `effects_cpu`: a chain of three video effects (blur, brightness and contrast, vignette) on the
//!   processor.
//!
//! The same work on the graphics card, when the machine has an adapter (without one these groups
//! are left out), each with the upload of the frame and the read-back of the result included:
//!
//! * `crossfade_gpu`: the same two layers composited by the compute shader,
//! * `effects_gpu`: the same chain of effects.
//!
//! What is NOT measured: decoding (that needs FFmpeg and a media file), the window and the audio
//! path. The figures are therefore a lower bound of the time per frame, not a playback frame rate.
//!
//! Run `cargo bench -p neoveg_video --bench playback_bench`; `scripts/bench_table.py` turns the
//! results in `target/criterion` into the table of `README.md`.

use std::sync::Arc;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use neoveg_common::composite::CompositeMode;
use neoveg_common::grade::Grade;
use neoveg_video::composite::{composite_layers_cpu, CompositeNode, DecodedLayer, GpuCompositor};
use neoveg_video::frame::FrameBuffer;
use neoveg_video::generators::smpte_color_bars;
use neoveg_video::grading::apply_grade;
use neoveg_video::scopes::Scopes;
use neoveg_video::vfx::{self, apply_chain_cpu, resolve_chain, VideoEffect};
use neoveg_video::vfx_gpu::VfxGpu;

/// Name, width and height of each resolution (16:9, 480p is the 854x480 wide format).
const RESOLUTIONS: [(&str, u32, u32); 6] = [
    ("480p", 854, 480),
    ("720p", 1280, 720),
    ("1080p", 1920, 1080),
    ("1440p", 2560, 1440),
    ("4K", 3840, 2160),
    ("8K", 7680, 4320),
];

fn configure(group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>) {
    // A frame at 8K is 130 MB, so the number of samples is kept small
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(3));
}

fn generate(c: &mut Criterion) {
    let mut group = c.benchmark_group("generate");
    configure(&mut group);
    for (name, width, height) in RESOLUTIONS {
        group.throughput(Throughput::Elements(u64::from(width) * u64::from(height)));
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            &(width, height),
            |b, &(w, h)| {
                b.iter(|| smpte_color_bars(w, h));
            },
        );
    }
    group.finish();
}

fn grade(c: &mut Criterion) {
    let adjustment = Grade {
        exposure: 0.4,
        contrast: 0.2,
        saturation: 1.1,
        ..Grade::default()
    };
    let mut group = c.benchmark_group("grade");
    configure(&mut group);
    for (name, width, height) in RESOLUTIONS {
        let source = smpte_color_bars(width, height);
        group.throughput(Throughput::Elements(u64::from(width) * u64::from(height)));
        group.bench_function(BenchmarkId::from_parameter(name), |b| {
            // The copy of the source is made outside the measured time
            b.iter_batched(
                || source.clone(),
                |mut frame| apply_grade(&mut frame, &adjustment),
                BatchSize::PerIteration,
            );
        });
    }
    group.finish();
}

fn scopes(c: &mut Criterion) {
    let mut group = c.benchmark_group("scopes");
    configure(&mut group);
    for (name, width, height) in RESOLUTIONS {
        let source = smpte_color_bars(width, height);
        group.throughput(Throughput::Elements(u64::from(width) * u64::from(height)));
        group.bench_function(BenchmarkId::from_parameter(name), |b| {
            b.iter(|| Scopes::from_frame(&source));
        });
    }
    group.finish();
}

fn layer(frame: &Arc<FrameBuffer>, opacity: f32) -> DecodedLayer {
    DecodedLayer {
        frame: Arc::clone(frame),
        mode: CompositeMode::SourceAlpha,
        opacity,
    }
}

fn crossfade(c: &mut Criterion) {
    let mut group = c.benchmark_group("crossfade");
    configure(&mut group);
    for (name, width, height) in RESOLUTIONS {
        let a = Arc::new(smpte_color_bars(width, height));
        let b_frame = Arc::new(FrameBuffer::solid(width, height, [40, 90, 200, 255]));
        let layers = [layer(&a, 1.0), layer(&b_frame, 0.5)];
        group.throughput(Throughput::Elements(u64::from(width) * u64::from(height)));
        group.bench_function(BenchmarkId::from_parameter(name), |bench| {
            bench.iter(|| composite_layers_cpu(width, height, &layers));
        });
    }
    group.finish();
}

/// The chain the effect benchmarks run: a blur, a brightness and contrast change and a vignette,
/// what a graded and softened shot carries.
fn effect_chain() -> Vec<VideoEffect> {
    vec![
        VideoEffect::new(vfx::GAUSSIAN_BLUR)
            .with("h_range", 0.01)
            .with("v_range", 0.01),
        VideoEffect::new(vfx::BRIGHTNESS_CONTRAST)
            .with("brightness", 0.1)
            .with("contrast", 0.2),
        VideoEffect::new(vfx::VIGNETTE),
    ]
}

fn effects_cpu(c: &mut Criterion) {
    let chain = effect_chain();
    let mut group = c.benchmark_group("effects_cpu");
    configure(&mut group);
    for (name, width, height) in RESOLUTIONS {
        let source = smpte_color_bars(width, height);
        group.throughput(Throughput::Elements(u64::from(width) * u64::from(height)));
        group.bench_function(BenchmarkId::from_parameter(name), |b| {
            b.iter_batched(
                || source.clone(),
                |mut frame| apply_chain_cpu(&mut frame, &chain),
                BatchSize::PerIteration,
            );
        });
    }
    group.finish();
}

/// The compositor on the graphics card, or `None` (with a note) on a machine without one.
fn gpu() -> Option<GpuCompositor> {
    let gpu = GpuCompositor::try_headless();
    if gpu.is_none() {
        eprintln!("no graphics adapter: the GPU groups are left out");
    }
    gpu
}

fn crossfade_gpu(c: &mut Criterion) {
    let Some(gpu) = gpu() else {
        return;
    };
    let mut group = c.benchmark_group("crossfade_gpu");
    configure(&mut group);
    for (name, width, height) in RESOLUTIONS {
        let a = Arc::new(smpte_color_bars(width, height));
        let b_frame = Arc::new(FrameBuffer::solid(width, height, [40, 90, 200, 255]));
        let nodes = [
            CompositeNode::Layer(layer(&a, 1.0)),
            CompositeNode::Layer(layer(&b_frame, 0.5)),
        ];
        // A picture the card refuses (too big for its buffers) is left out
        if let Err(e) = gpu.composite_nodes_gpu(width, height, [0, 0, 0, 255], &nodes) {
            eprintln!("crossfade_gpu {name}: {e}");
            continue;
        }
        group.throughput(Throughput::Elements(u64::from(width) * u64::from(height)));
        group.bench_function(BenchmarkId::from_parameter(name), |bench| {
            bench.iter(|| gpu.composite_nodes_gpu(width, height, [0, 0, 0, 255], &nodes));
        });
    }
    group.finish();
}

fn effects_gpu(c: &mut Criterion) {
    let Some(gpu) = gpu() else {
        return;
    };
    let (device, queue) = gpu.device_queue();
    let effects = VfxGpu::new(device, queue);
    let chain = resolve_chain(&effect_chain());
    let mut group = c.benchmark_group("effects_gpu");
    configure(&mut group);
    for (name, width, height) in RESOLUTIONS {
        let source = smpte_color_bars(width, height);
        if let Err(e) = effects.process(&source, &chain) {
            eprintln!("effects_gpu {name}: {e}");
            continue;
        }
        group.throughput(Throughput::Elements(u64::from(width) * u64::from(height)));
        group.bench_function(BenchmarkId::from_parameter(name), |b| {
            b.iter(|| effects.process(&source, &chain));
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    generate,
    grade,
    scopes,
    crossfade,
    effects_cpu,
    crossfade_gpu,
    effects_gpu
);
criterion_main!(benches);
