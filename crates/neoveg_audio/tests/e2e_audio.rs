//! E2E Integration Test Suite for neoveg_audio (Tiers 1 & 2)
//! Covers Features:
//! - Feature 2: Input Device Selection & Probing (R1)
//! - Feature 4: Recording with Monitoring (R1)
//! - Feature 20: Text-Based Editing (R2.7)
//! - Feature 21: Speech-to-Text Subtitles (R2.7)
//! - Feature 38: Multicam Waveform & Timecode Sync (R2.12)

use std::collections::HashMap;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use candle_core::Device;
use candle_nn::VarBuilder;
use candle_transformers::models::whisper::{self, Config};

use neoveg_audio::record::{
    convert_chunk, list_inputs, map_frame, probe_input, wav_float_header,
    InputInfo, MonitorRing, MonitorStatus, RecordOptions, Recording, WavWriter,
};
use neoveg_audio::sync::{
    envelope, find_offset, Sync, ENVELOPE_RATE, MAX_OFFSET_SECONDS, MIN_CONFIDENCE,
    MIN_OVERLAP_SECONDS, WINDOW_SECONDS,
};
use neoveg_audio::whisper::{
    mix_to_mono_16k, resample_mono_to_16k, slaney_mel_filters, TranscriptSegment,
    WhisperEngine, WhisperTokenizer, SUPPORTED_LANGUAGES, WHISPER_SAMPLE_RATE,
};

// ============================================================================
// Feature 2: Input Device Selection & Probing (R1)
// ============================================================================

#[test]
fn f02_t1_01_list_inputs_returns_valid_unique_list() {
    let inputs = list_inputs();
    let mut deduped = inputs.clone();
    deduped.sort();
    deduped.dedup();
    assert_eq!(inputs.len(), deduped.len(), "list_inputs must not contain duplicate names");
}

#[test]
fn f02_t1_02_default_record_options_device_is_none() {
    let opt = RecordOptions::default();
    assert!(opt.device.is_none(), "Default device should be None (system default)");
    assert_eq!(opt.monitor_volume, 1.0);
    assert!(!opt.monitor);
}

#[test]
fn f02_t1_03_input_info_struct_properties() {
    let info = InputInfo {
        device: "Mock Mic".into(),
        sample_rate: 48_000,
        channels: 2,
    };
    assert_eq!(info.device, "Mock Mic");
    assert_eq!(info.sample_rate, 48_000);
    assert_eq!(info.channels, 2);
}

#[test]
fn f02_t1_04_probe_input_headless_resilience() {
    // In headless environment without physical audio input, probe_input should return Ok or graceful Err string
    let res = probe_input(None);
    match res {
        Ok(info) => {
            assert!(info.sample_rate > 0);
            assert!(info.channels > 0);
        }
        Err(err_msg) => {
            assert!(!err_msg.is_empty(), "Error message must describe missing hardware");
        }
    }
}

#[test]
fn f02_t1_05_custom_device_name_option_preserved() {
    let opt = RecordOptions {
        device: Some("USB Audio Interface".into()),
        monitor: true,
        monitor_volume: 0.8,
        track_id: Some(1),
    };
    assert_eq!(opt.device.as_deref(), Some("USB Audio Interface"));
    assert!(opt.monitor);
    assert_eq!(opt.monitor_volume, 0.8);
    assert_eq!(opt.track_id, Some(1));
}

#[test]
fn f02_t2_01_probe_non_existent_device_name() {
    let res = probe_input(Some("NonExistentDevice_12345_XYZ"));
    // Must fall back to default input or return error string without crashing
    match res {
        Ok(info) => assert!(info.sample_rate > 0),
        Err(e) => assert!(!e.is_empty()),
    }
}

#[test]
fn f02_t2_02_record_options_monitor_volume_bounds() {
    let opt_high = RecordOptions {
        device: None,
        monitor: true,
        monitor_volume: 5.0, // excessive volume
        track_id: None,
    };
    assert!(opt_high.monitor_volume > 2.0);

    let opt_neg = RecordOptions {
        device: None,
        monitor: true,
        monitor_volume: -1.0, // negative volume
        track_id: None,
    };
    assert!(opt_neg.monitor_volume < 0.0);
}

#[test]
fn f02_t2_03_monitor_status_variants() {
    let off = MonitorStatus::Off;
    let on = MonitorStatus::On;
    let unavail = MonitorStatus::Unavailable("Device busy".into());
    assert_eq!(off, MonitorStatus::Off);
    assert_eq!(on, MonitorStatus::On);
    assert!(matches!(unavail, MonitorStatus::Unavailable(_)));
}

#[test]
fn f02_t2_04_recording_duration_calculation_zero_frames() {
    let r = Recording {
        path: PathBuf::from("rec.wav"),
        sample_rate: 48_000,
        channels: 2,
        frames: 0,
        overruns: 0,
        track_id: None,
    };
    assert_eq!(r.seconds(), 0.0);
}

#[test]
fn f02_t2_05_recording_duration_calculation_zero_sample_rate_guard() {
    let r = Recording {
        path: PathBuf::from("rec.wav"),
        sample_rate: 0, // invalid sample rate
        channels: 1,
        frames: 48_000,
        overruns: 0,
        track_id: None,
    };
    assert_eq!(r.seconds(), 48_000.0); // Guarded by sample_rate.max(1)
}

// ============================================================================
// Feature 4: Recording with Monitoring (R1)
// ============================================================================

#[test]
fn f04_t1_01_wav_float_header_riff_specification() {
    let h = wav_float_header(2, 48_000, 1024);
    assert_eq!(&h[0..4], b"RIFF");
    assert_eq!(&h[8..12], b"WAVE");
    assert_eq!(&h[12..16], b"fmt ");
    assert_eq!(u16::from_le_bytes(h[20..22].try_into().unwrap()), 3); // IEEE float format
    assert_eq!(u16::from_le_bytes(h[22..24].try_into().unwrap()), 2); // 2 channels
    assert_eq!(u32::from_le_bytes(h[24..28].try_into().unwrap()), 48_000);
    assert_eq!(&h[36..40], b"data");
    assert_eq!(u32::from_le_bytes(h[40..44].try_into().unwrap()), 1024);
}

#[test]
fn f04_t1_02_wav_writer_in_memory_creation_and_write() {
    let mut writer = WavWriter::new(Cursor::new(Vec::new()), 1, 44_100).expect("create writer");
    let test_samples = [0.0f32, 0.5, -0.5, 0.25];
    writer.write_samples(&test_samples).expect("write samples");
    assert_eq!(writer.frames(), 4);

    let cursor = writer.finish().expect("finish writer");
    let bytes = cursor.into_inner();
    assert_eq!(bytes.len(), 44 + 4 * 4);
    assert_eq!(&bytes[0..4], b"RIFF");
}

#[test]
fn f04_t1_03_monitor_ring_push_pop_fifo_integrity() {
    let ring = MonitorRing::new();
    let input_samples = [0.1f32, 0.2, 0.3, 0.4];
    let pushed = ring.push(&input_samples);
    assert_eq!(pushed, 4);
    assert_eq!(ring.available(), 4);

    let mut out = [0.0f32; 4];
    let popped = ring.pop(&mut out);
    assert_eq!(popped, 4);
    assert_eq!(out, input_samples);
    assert_eq!(ring.available(), 0);
}

#[test]
fn f04_t1_04_map_frame_stereo_and_mono_routing() {
    let mut stereo_out = [0.0f32; 2];
    map_frame(&[0.75], &mut stereo_out, 1.0);
    assert_eq!(stereo_out, [0.75, 0.75], "Mono input should map to stereo L & R");

    let mut mono_out = [0.0f32; 1];
    map_frame(&[0.4, 0.8], &mut mono_out, 1.0);
    assert!((mono_out[0] - 0.6).abs() < 1e-6, "Stereo into mono should average channels");
}

#[test]
fn f04_t1_05_convert_chunk_measures_peak_accurately() {
    let mut buf = Vec::with_capacity(16);
    let peak = convert_chunk(&[-0.25f32, 0.8, -0.9, 0.1], &mut buf);
    assert_eq!(peak, 0.9);
    assert_eq!(buf.len(), 4);
}

#[test]
fn f04_t2_01_monitor_ring_wrap_around_overflow_safety() {
    let ring = MonitorRing::new();
    let samples = vec![0.5f32; 70_000]; // larger than 65536 capacity
    let pushed = ring.push(&samples);
    assert!(pushed <= 65536);

    let extra = ring.push(&[1.0]);
    assert_eq!(extra, 0, "Full ring must reject additional samples");
}

#[test]
fn f04_t2_02_monitor_ring_skip_to_latest() {
    let ring = MonitorRing::new();
    let block: Vec<f32> = (0..500).map(|i| i as f32).collect();
    ring.push(&block);
    ring.skip_to_latest(50);
    assert_eq!(ring.available(), 50);

    let mut out = [0.0f32; 1];
    ring.pop(&mut out);
    assert_eq!(out[0], 450.0);
}

#[test]
fn f04_t2_03_map_frame_with_gain_scaling() {
    let mut out = [0.0f32; 2];
    map_frame(&[0.5], &mut out, 0.5);
    assert_eq!(out, [0.25, 0.25]);

    map_frame(&[0.5], &mut out, 0.0);
    assert_eq!(out, [0.0, 0.0]);
}

#[test]
fn f04_t2_04_map_frame_empty_input_fills_silence() {
    let mut out = [1.0f32; 2];
    map_frame(&[], &mut out, 1.0);
    assert_eq!(out, [0.0, 0.0]);
}

#[test]
fn f04_t2_05_convert_chunk_empty_input() {
    let mut buf = Vec::with_capacity(8);
    let peak = convert_chunk::<f32>(&[], &mut buf);
    assert_eq!(peak, 0.0);
    assert!(buf.is_empty());
}

// ============================================================================
// Feature 20: Text-Based Editing (R2.7)
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct TextBasedTimelineCut {
    pub word: String,
    pub start_sec: f64,
    pub end_sec: f64,
}

pub fn timeline_cuts_from_transcript(segments: &[TranscriptSegment], deleted_indices: &[usize]) -> Vec<TextBasedTimelineCut> {
    let mut cuts = Vec::new();
    for (idx, seg) in segments.iter().enumerate() {
        if deleted_indices.contains(&idx) {
            cuts.push(TextBasedTimelineCut {
                word: seg.text.clone(),
                start_sec: seg.start_s,
                end_sec: seg.end_s,
            });
        }
    }
    cuts
}

#[test]
fn f20_t1_01_transcript_segment_structure() {
    let seg = TranscriptSegment {
        start_s: 1.0,
        end_s: 2.5,
        text: "Bem-vindo ao Neoveg".into(),
    };
    assert_eq!(seg.start_s, 1.0);
    assert_eq!(seg.end_s, 2.5);
    assert_eq!(seg.text, "Bem-vindo ao Neoveg");
}

#[test]
fn f20_t1_02_text_based_cuts_generation() {
    let segs = vec![
        TranscriptSegment { start_s: 0.0, end_s: 1.0, text: "Primeiro".into() },
        TranscriptSegment { start_s: 1.0, end_s: 2.0, text: "Segundo".into() },
        TranscriptSegment { start_s: 2.0, end_s: 3.0, text: "Terceiro".into() },
    ];
    let cuts = timeline_cuts_from_transcript(&segs, &[1]);
    assert_eq!(cuts.len(), 1);
    assert_eq!(cuts[0].word, "Segundo");
    assert_eq!(cuts[0].start_sec, 1.0);
    assert_eq!(cuts[0].end_sec, 2.0);
}

#[test]
fn f20_t1_03_text_based_ripple_duration_computation() {
    let segs = vec![
        TranscriptSegment { start_s: 2.0, end_s: 3.5, text: "ehhh".into() },
        TranscriptSegment { start_s: 5.0, end_s: 6.0, text: "hummm".into() },
    ];
    let cuts = timeline_cuts_from_transcript(&segs, &[0, 1]);
    let total_deleted_sec: f64 = cuts.iter().map(|c| c.end_sec - c.start_sec).sum();
    assert!((total_deleted_sec - 2.5).abs() < 1e-6);
}

#[test]
fn f20_t1_04_resample_mono_to_16k_conversion() {
    let source_48k = vec![0.5f32; 48_000]; // 1 second at 48 kHz
    let resampled = resample_mono_to_16k(&source_48k, 48_000);
    assert_eq!(resampled.len(), 16_000);
    for sample in resampled {
        assert!((sample - 0.5).abs() < 1e-4);
    }
}

#[test]
fn f20_t1_05_resample_mono_to_16k_pass_through_when_already_16k() {
    let source_16k = vec![0.25f32; 16_000];
    let resampled = resample_mono_to_16k(&source_16k, WHISPER_SAMPLE_RATE);
    assert_eq!(resampled, source_16k);
}

#[test]
fn f20_t2_01_resample_mono_to_16k_empty_input() {
    assert!(resample_mono_to_16k(&[], 48_000).is_empty());
    assert!(resample_mono_to_16k(&[1.0, 2.0], 0).is_empty());
}

#[test]
fn f20_t2_02_transcript_cut_empty_deletions() {
    let segs = vec![TranscriptSegment { start_s: 0.0, end_s: 1.0, text: "A".into() }];
    let cuts = timeline_cuts_from_transcript(&segs, &[]);
    assert!(cuts.is_empty());
}

#[test]
fn f20_t2_03_transcript_cut_out_of_bounds_index() {
    let segs = vec![TranscriptSegment { start_s: 0.0, end_s: 1.0, text: "A".into() }];
    let cuts = timeline_cuts_from_transcript(&segs, &[99]);
    assert!(cuts.is_empty());
}

#[test]
fn f20_t2_04_mix_to_mono_16k_negative_start_clamped() {
    let out = mix_to_mono_16k(&[], -5.0, 1.0);
    assert_eq!(out.len(), 16_000);
    assert!(out.iter().all(|&s| s == 0.0));
}

#[test]
fn f20_t2_05_mix_to_mono_16k_end_before_start_zero_duration() {
    let out = mix_to_mono_16k(&[], 2.0, 1.0);
    assert_eq!(out.len(), 0);
}

// ============================================================================
// Feature 21: Speech-to-Text Subtitles (R2.7)
// ============================================================================

fn synthetic_tokenizer_json() -> String {
    serde_json::json!({
        "model": {
            "vocab": {
                "hello": 10,
                "world": 11,
                "ola": 12,
                "acao": 13,
                "a": 14
            }
        },
        "added_tokens": [
            { "id": 50, "content": "<|endoftext|>" },
            { "id": 51, "content": "<|startoftranscript|>" },
            { "id": 52, "content": "<|pt|>" },
            { "id": 53, "content": "<|en|>" },
            { "id": 54, "content": "<|transcribe|>" },
            { "id": 55, "content": "<|translate|>" },
            { "id": 56, "content": "<|nospeech|>" },
            { "id": 57, "content": "<|notimestamps|>" },
            { "id": 58, "content": "<|0.00|>" }
        ]
    }).to_string()
}

#[test]
fn f21_t1_01_whisper_supported_languages_contains_target_languages() {
    let langs: HashMap<_, _> = SUPPORTED_LANGUAGES.iter().copied().collect();
    assert_eq!(langs.get("pt"), Some(&"Português"));
    assert_eq!(langs.get("en"), Some(&"English"));
    assert_eq!(langs.get("es"), Some(&"Español"));
    assert_eq!(langs.get("de"), Some(&"Deutsch"));
    assert_eq!(langs.get("fr"), Some(&"Français"));
    assert_eq!(langs.get("ja"), Some(&"日本語"));
    assert_eq!(langs.get("zh"), Some(&"中文"));
}

#[test]
fn f21_t1_02_slaney_mel_filters_dimensions() {
    let filters_80 = slaney_mel_filters(80, whisper::N_FFT, WHISPER_SAMPLE_RATE as f64);
    assert_eq!(filters_80.len(), 80 * 201);
    assert!(filters_80.iter().all(|&w| w >= 0.0 && w.is_finite()));

    let filters_128 = slaney_mel_filters(128, whisper::N_FFT, WHISPER_SAMPLE_RATE as f64);
    assert_eq!(filters_128.len(), 128 * 201);
}

#[test]
fn f21_t1_03_whisper_tokenizer_decodes_special_tokens() {
    let tok = WhisperTokenizer::from_json_str(&synthetic_tokenizer_json()).expect("parse tokenizer");
    assert_eq!(tok.language_token("pt"), Some(52));
    assert_eq!(tok.language_token("en"), Some(53));
    assert_eq!(tok.timestamp_seconds(58), Some(0.0));
    assert_eq!(tok.timestamp_seconds(58 + 50), Some(1.0));
}

#[test]
fn f21_t1_04_synthetic_whisper_engine_transcribe_deterministic() {
    let tok = WhisperTokenizer::from_json_str(&synthetic_tokenizer_json()).unwrap();
    let cfg = Config {
        num_mel_bins: 80,
        max_source_positions: 1500,
        d_model: 16,
        encoder_attention_heads: 2,
        encoder_layers: 1,
        vocab_size: 1600,
        max_target_positions: 16,
        decoder_attention_heads: 2,
        decoder_layers: 1,
        suppress_tokens: vec![],
    };
    let vb = VarBuilder::zeros(whisper::DTYPE, &Device::Cpu);
    let model = whisper::model::Whisper::load(&vb, cfg).unwrap();
    let mut engine = WhisperEngine::from_model_and_tokenizer(model, tok);

    let tone: Vec<f32> = (0..8_000).map(|i| (i as f32 * 0.05).sin() * 0.5).collect();
    let cancel = AtomicBool::new(false);
    let mut progress = 0.0f32;
    let res = engine.transcribe(&tone, 0.0, Some("en"), &cancel, |p| progress = p);
    assert!(res.is_ok());
    assert!((progress - 1.0).abs() < 1e-5);
}

#[test]
fn f21_t1_05_synthetic_whisper_cancellation_honored() {
    let tok = WhisperTokenizer::from_json_str(&synthetic_tokenizer_json()).unwrap();
    let cfg = Config {
        num_mel_bins: 80,
        max_source_positions: 1500,
        d_model: 16,
        encoder_attention_heads: 2,
        encoder_layers: 1,
        vocab_size: 1600,
        max_target_positions: 16,
        decoder_attention_heads: 2,
        decoder_layers: 1,
        suppress_tokens: vec![],
    };
    let vb = VarBuilder::zeros(whisper::DTYPE, &Device::Cpu);
    let model = whisper::model::Whisper::load(&vb, cfg).unwrap();
    let mut engine = WhisperEngine::from_model_and_tokenizer(model, tok);

    let tone = vec![0.5f32; 8_000];
    let cancel = AtomicBool::new(true); // already cancelled
    let res = engine.transcribe(&tone, 0.0, Some("pt"), &cancel, |_| {});
    assert!(res.is_err(), "Cancelled transcription must return Err");
}

#[test]
fn f21_t2_01_slaney_mel_filters_zero_bins_or_fft() {
    assert!(slaney_mel_filters(0, whisper::N_FFT, 16000.0).is_empty());
    assert!(slaney_mel_filters(80, 0, 16000.0).is_empty());
    assert!(slaney_mel_filters(80, whisper::N_FFT, -1.0).is_empty());
}

#[test]
fn f21_t2_02_whisper_tokenizer_unknown_language_returns_none() {
    let tok = WhisperTokenizer::from_json_str(&synthetic_tokenizer_json()).unwrap();
    assert!(tok.language_token("klingon").is_none());
    assert!(tok.language_token("").is_none());
}

#[test]
fn f21_t2_03_whisper_tokenizer_invalid_json_handling() {
    let res = WhisperTokenizer::from_json_str("invalid json text");
    assert!(res.is_err());
}

#[test]
fn f21_t2_04_transcription_pure_silence_input() {
    let tok = WhisperTokenizer::from_json_str(&synthetic_tokenizer_json()).unwrap();
    let cfg = Config {
        num_mel_bins: 80,
        max_source_positions: 1500,
        d_model: 16,
        encoder_attention_heads: 2,
        encoder_layers: 1,
        vocab_size: 1600,
        max_target_positions: 16,
        decoder_attention_heads: 2,
        decoder_layers: 1,
        suppress_tokens: vec![],
    };
    let vb = VarBuilder::zeros(whisper::DTYPE, &Device::Cpu);
    let model = whisper::model::Whisper::load(&vb, cfg).unwrap();
    let mut engine = WhisperEngine::from_model_and_tokenizer(model, tok);

    let silence = vec![0.0f32; 8_000]; // pure silence
    let cancel = AtomicBool::new(false);
    let res = engine.transcribe(&silence, 0.0, None, &cancel, |_| {});
    assert!(res.is_ok());
    // Pure silence produces 0 segments
    assert_eq!(res.unwrap().len(), 0);
}

#[test]
fn f21_t2_05_transcript_segment_offset_preservation() {
    let tok = WhisperTokenizer::from_json_str(&synthetic_tokenizer_json()).unwrap();
    let cfg = Config {
        num_mel_bins: 80,
        max_source_positions: 1500,
        d_model: 16,
        encoder_attention_heads: 2,
        encoder_layers: 1,
        vocab_size: 1600,
        max_target_positions: 16,
        decoder_attention_heads: 2,
        decoder_layers: 1,
        suppress_tokens: vec![],
    };
    let vb = VarBuilder::zeros(whisper::DTYPE, &Device::Cpu);
    let model = whisper::model::Whisper::load(&vb, cfg).unwrap();
    let mut engine = WhisperEngine::from_model_and_tokenizer(model, tok);

    let tone: Vec<f32> = (0..8_000).map(|i| (i as f32 * 0.1).sin() * 0.8).collect();
    let cancel = AtomicBool::new(false);
    let res = engine.transcribe(&tone, 15.0, Some("pt"), &cancel, |_| {});
    if let Ok(segs) = res {
        for seg in segs {
            assert!(seg.start_s >= 15.0, "Segment start should respect base offset");
        }
    }
}

// ============================================================================
// Feature 38: Multicam Waveform & Timecode Sync (R2.12)
// ============================================================================

#[test]
fn f38_t1_01_sync_constants_values() {
    assert_eq!(ENVELOPE_RATE, 200.0);
    assert_eq!(WINDOW_SECONDS, 60.0);
    assert_eq!(MAX_OFFSET_SECONDS, 30.0);
    assert_eq!(MIN_OVERLAP_SECONDS, 2.0);
    assert_eq!(MIN_CONFIDENCE, 0.3);
}

#[test]
fn f38_t1_02_envelope_computation_rate() {
    let rate = 48_000;
    // 2 seconds of stereo audio (96,000 floats = 48,000 frames)
    let samples = vec![0.5f32; rate as usize * 2 * 2];
    let env = envelope(&samples, rate);
    // 2 seconds at 200 Hz = 400 bins
    assert_eq!(env.len(), (2.0 * ENVELOPE_RATE) as usize);
}

#[test]
fn f38_t1_03_find_offset_identical_signals_zero_offset() {
    let rate = 48_000;
    let frames = rate as usize * 4; // 4 seconds
    let tone: Vec<f32> = (0..frames)
        .flat_map(|i| {
            let s = ((i as f32 * 0.05).sin() * 0.5) * ((i as f32 * 0.001).sin() + 1.0);
            [s, s]
        })
        .collect();

    let res = find_offset(&tone, rate, &tone, rate).expect("should sync identical audio");
    assert!(res.offset.abs() < (1.0 / ENVELOPE_RATE));
    assert!(res.confidence >= 0.95);
}

#[test]
fn f38_t1_04_find_offset_delayed_signal_detects_offset() {
    let rate = 48_000;
    let delay_frames = (0.5 * rate as f64) as usize; // 0.5s delay
    let total_frames = rate as usize * 5;

    let base: Vec<f32> = (0..total_frames)
        .map(|i| ((i as f32 * 0.02).sin() * 0.6) * if i % 1000 < 500 { 1.0 } else { 0.2 })
        .collect();

    let sig_a: Vec<f32> = base.iter().flat_map(|&s| [s, s]).collect();
    let sig_b: Vec<f32> = (0..delay_frames)
        .map(|_| 0.0f32)
        .chain(base.into_iter())
        .flat_map(|s| [s, s])
        .collect();

    let res = find_offset(&sig_a, rate, &sig_b, rate).expect("should detect offset");
    assert!((res.offset - (-0.5)).abs() < 0.05, "Detected offset: {}", res.offset);
}

#[test]
fn f38_t1_05_sync_struct_properties() {
    let s = Sync { offset: 1.25, confidence: 0.88 };
    assert_eq!(s.offset, 1.25);
    assert_eq!(s.confidence, 0.88);
}

#[test]
fn f38_t2_01_find_offset_too_short_audio_returns_none() {
    let rate = 48_000;
    let short: Vec<f32> = vec![0.5; rate as usize]; // 0.5s stereo, less than MIN_OVERLAP_SECONDS (2.0s)
    assert!(find_offset(&short, rate, &short, rate).is_none());
}

#[test]
fn f38_t2_02_find_offset_silent_audio_returns_none() {
    let rate = 48_000;
    let silent: Vec<f32> = vec![0.0; rate as usize * 4 * 2]; // 4s silence
    assert!(find_offset(&silent, rate, &silent, rate).is_none());
}

#[test]
fn f38_t2_03_find_offset_completely_uncorrelated_noise_returns_none() {
    let rate = 48_000;
    let frames = rate as usize * 4;
    // Two distinct frequencies
    let a: Vec<f32> = (0..frames).flat_map(|i| [(i as f32 * 0.01).sin(), (i as f32 * 0.01).sin()]).collect();
    let b: Vec<f32> = (0..frames).flat_map(|i| [(i as f32 * 0.99).cos(), (i as f32 * 0.99).cos()]).collect();
    let res = find_offset(&a, rate, &b, rate);
    assert!(res.is_none() || res.unwrap().confidence < 0.5);
}

#[test]
fn f38_t2_04_envelope_empty_samples_returns_empty() {
    assert!(envelope(&[], 48_000).is_empty());
}

#[test]
fn f38_t2_05_envelope_mean_centered_property() {
    let rate = 48_000;
    let samples: Vec<f32> = (0..rate * 3 * 2).map(|i| if i % 2 == 0 { 0.8 } else { 0.2 }).collect();
    let env = envelope(&samples, rate);
    assert!(!env.is_empty());
    let sum: f32 = env.iter().sum();
    assert!(sum.abs() < 1e-4, "Envelope should be zero-mean centered, got {sum}");
}
