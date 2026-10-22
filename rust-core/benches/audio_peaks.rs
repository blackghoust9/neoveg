use criterion::{black_box, criterion_group, criterion_main, Criterion};
use neoveg_core::audio_peaks::AudacityWaveformMipmap;

fn benchmark_waveform_generation(criterion: &mut Criterion) {
    let pcm: Vec<i16> = (0..480_000)
        .map(|sample| ((sample % 2_000) as i16) - 1_000)
        .collect();

    criterion.bench_function("waveform_generation_5s_stereo", |bencher| {
        bencher.iter(|| {
            AudacityWaveformMipmap::from_interleaved_pcm_i16(black_box(&pcm), 2, 48_000, 256)
        });
    });
}

criterion_group!(benches, benchmark_waveform_generation);
criterion_main!(benches);
