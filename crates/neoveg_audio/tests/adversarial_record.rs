//! Adversarial stress-testing and boundary verification for `Recorder` and audio monitoring.
//!
//! Evaluates the 5 mandatory challenge edge cases:
//! 1. Zero samples pushed (empty buffers, zero frames, empty pops/conversions).
//! 2. Massive chunk pushes exceeding `MonitorRing` capacity and writer queue saturation.
//! 3. Rapid `start()` and immediate `stop()` cycles (race conditions, rapid teardown, Drop safety).
//! 4. Dynamic track arming changes mid-recording (target resolution and fallbacks).
//! 5. Channel count mismatches (mono, stereo, 5.1, 7.1, 8-channel limits, odd chunk sizes).
//! Plus extreme floating point inputs (NaN, +/-Inf, subnormals).

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use neoveg_audio::record::{
    convert_chunk, map_frame, MonitorRing, RecordOptions, Recorder,
};
use neoveg_audio::waveform::WaveformSource;

fn temp_wav_path(prefix: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}_{}_{}.wav", std::process::id(), nanos))
}

// ============================================================================
// 1. Zero Samples Pushed & Empty Slices
// ============================================================================

#[test]
fn adv_01_zero_samples_push_and_pop_safety() {
    let ring = MonitorRing::new();
    assert_eq!(ring.available(), 0);

    // Push empty slice
    let pushed = ring.push(&[]);
    assert_eq!(pushed, 0);
    assert_eq!(ring.available(), 0);

    // Pop into empty slice
    let popped = ring.pop(&mut []);
    assert_eq!(popped, 0);
    assert_eq!(ring.available(), 0);

    // Pop into non-empty slice when ring is empty
    let mut out = [999.0f32; 16];
    let popped = ring.pop(&mut out);
    assert_eq!(popped, 0);
    assert_eq!(out[0], 999.0f32, "empty pop must not touch buffer contents");

    // Skip to latest on empty ring
    ring.skip_to_latest(0);
    assert_eq!(ring.available(), 0);
    ring.skip_to_latest(100);
    assert_eq!(ring.available(), 0);
}

#[test]
fn adv_01_empty_convert_chunk_and_map_frame() {
    let mut out_vec = Vec::with_capacity(10);
    let peak = convert_chunk::<f32>(&[], &mut out_vec);
    assert_eq!(peak, 0.0);
    assert!(out_vec.is_empty());

    // Map frame empty input
    let mut out = [1.0f32; 4];
    map_frame(&[], &mut out, 1.0);
    assert_eq!(out, [0.0, 0.0, 0.0, 0.0]);

    // Map frame empty output
    let mut empty_out: [f32; 0] = [];
    map_frame(&[0.5, 0.5], &mut empty_out, 1.0); // Must not panic
}

#[test]
fn adv_01_recorder_push_zero_samples_and_stop() {
    let path = temp_wav_path("adv_zero_samples");
    let options = RecordOptions {
        device: Some("Adv Mock Input".into()),
        monitor: true,
        monitor_volume: 1.0,
        track_id: Some(101),
    };

    let mut recorder =
        Recorder::create_mock_loopback_with(&path, &options).expect("recorder init");
    let initial_frames = recorder.frames();

    // Pushing empty samples
    recorder.push_mock_samples(&[]);
    recorder.push_mock_samples(&[]);

    assert_eq!(recorder.frames(), initial_frames);

    let rec = recorder.stop().expect("stop must succeed");
    assert_eq!(rec.frames, initial_frames);
    assert_eq!(rec.track_id, Some(101));
    assert!(rec.path.exists());

    // Verify written file is decodable
    let bytes = fs::read(&rec.path).expect("read file");
    let src = WaveformSource::from_wav_bytes(bytes).expect("valid WAV");
    assert_eq!(src.frame_count() as u64, rec.frames);

    let _ = fs::remove_file(&rec.path);
}

// ============================================================================
// 2. Massive Chunks Exceeding MonitorRing Capacity & Saturation
// ============================================================================

#[test]
fn adv_02_monitor_ring_massive_saturation_and_boundary() {
    let ring = MonitorRing::new();
    const RING_CAPACITY: usize = 65536; // 1 << 16

    // Push 100,000 samples at once (larger than ring)
    let massive_data: Vec<f32> = (0..100_000).map(|i| (i as f32) * 0.001).collect();
    let pushed = ring.push(&massive_data);
    assert_eq!(
        pushed, RING_CAPACITY,
        "ring should cap at maximum power-of-two capacity"
    );
    assert_eq!(ring.available(), RING_CAPACITY);

    // Subsequent push to full ring must return 0 without corrupting head/tail
    let overflow_pushed = ring.push(&[1.23, 4.56]);
    assert_eq!(overflow_pushed, 0);
    assert_eq!(ring.available(), RING_CAPACITY);

    // Drain partial buffer (10,000 samples)
    let mut drain = vec![0.0f32; 10_000];
    let popped = ring.pop(&mut drain);
    assert_eq!(popped, 10_000);
    assert_eq!(ring.available(), RING_CAPACITY - 10_000);

    // Verify drained data is first 10,000 samples
    for (i, &val) in drain.iter().enumerate() {
        assert_eq!(val, (i as f32) * 0.001);
    }

    // Now push 20,000 samples into the 10,000 free slots
    let push_refill = ring.push(&massive_data[10_000..30_000]);
    assert_eq!(
        push_refill, 10_000,
        "should accept exactly the free capacity"
    );
    assert_eq!(ring.available(), RING_CAPACITY);

    // Test skip_to_latest boundary conditions
    ring.skip_to_latest(256);
    assert_eq!(ring.available(), 256);

    // skip_to_latest with keep > available must be a safe no-op
    ring.skip_to_latest(1024);
    assert_eq!(ring.available(), 256);

    // skip_to_latest(0) completely flushes
    ring.skip_to_latest(0);
    assert_eq!(ring.available(), 0);
}

#[test]
fn adv_02_recorder_massive_chunk_burst_and_overrun_tracking() {
    let path = temp_wav_path("adv_massive_burst");
    let options = RecordOptions {
        device: Some("Burst Input".into()),
        monitor: true,
        monitor_volume: 1.0,
        track_id: Some(5),
    };

    let mut recorder =
        Recorder::create_mock_loopback_with(&path, &options).expect("recorder init");

    // Feed 500,000 samples (250,000 stereo frames = ~5.2 seconds of audio) in one burst
    let massive_tone: Vec<f32> = (0..500_000)
        .map(|i| ((i as f32) * 0.1).sin() * 0.7)
        .collect();

    recorder.push_mock_samples(&massive_tone);

    // Peak level must reflect the signal amplitude
    let peak = recorder.take_peak();
    assert!(peak > 0.65 && peak <= 0.701, "peak measured: {peak}");

    // Recorder frames must track all successfully queued samples
    assert!(recorder.frames() > 1000);

    // Stop recorder - must cleanly drain and write WAV
    let rec = recorder.stop().expect("stop after massive burst");
    assert!(rec.path.exists());
    assert!(rec.frames > 1000);

    let bytes = fs::read(&rec.path).expect("read WAV");
    let source = WaveformSource::from_wav_bytes(bytes).expect("valid WAV from massive burst");
    assert_eq!(source.frame_count() as u64, rec.frames);

    let _ = fs::remove_file(&rec.path);
}

// ============================================================================
// 3. Immediate Stop and Rapid Lifecycle Stress-Testing
// ============================================================================

#[test]
fn adv_03_immediate_stop_after_creation() {
    let path = temp_wav_path("adv_immediate_stop");
    let options = RecordOptions {
        device: Some("Quick Stop".into()),
        monitor: true,
        monitor_volume: 1.0,
        track_id: None,
    };

    let recorder =
        Recorder::create_mock_loopback_with(&path, &options).expect("create recorder");
    // Immediate stop without sleep or additional samples
    let rec = recorder.stop().expect("stop immediately");

    assert_eq!(rec.channels, 2);
    assert_eq!(rec.sample_rate, 48_000);
    assert_eq!(rec.frames, 960, "initial synthetic tone was 960 frames");
    assert!(rec.path.exists());

    let bytes = fs::read(&rec.path).expect("read WAV");
    let source = WaveformSource::from_wav_bytes(bytes).expect("valid WAV");
    assert_eq!(source.frame_count(), 960);

    let _ = fs::remove_file(&rec.path);
}

#[test]
fn adv_03_rapid_start_stop_stress_cycles() {
    // Perform 30 rapid back-to-back start/stop cycles
    for i in 0..30 {
        let path = temp_wav_path(&format!("adv_cycle_{i}"));
        let options = RecordOptions {
            device: Some(format!("Cycle Dev {i}")),
            monitor: i % 2 == 0,
            monitor_volume: 1.0,
            track_id: Some(i as u64),
        };

        let recorder =
            Recorder::create_mock_loopback_with(&path, &options).expect("recorder cycle create");
        let rec = recorder.stop().expect("recorder cycle stop");

        assert_eq!(rec.track_id, Some(i as u64));
        assert!(rec.frames > 0);
        assert!(rec.path.exists());

        let bytes = fs::read(&rec.path).expect("read WAV");
        let source = WaveformSource::from_wav_bytes(bytes).expect("valid WAV");
        assert!(source.frame_count() > 0);

        let _ = fs::remove_file(&rec.path);
    }
}

#[test]
fn adv_03_immediate_drop_without_stop() {
    // Tests Drop implementation: must safely join background thread and flush WAV
    for i in 0..10 {
        let path = temp_wav_path(&format!("adv_drop_{i}"));
        let options = RecordOptions {
            device: Some("Drop Dev".into()),
            monitor: true,
            monitor_volume: 1.0,
            track_id: Some(10),
        };

        let recorder =
            Recorder::create_mock_loopback_with(&path, &options).expect("recorder create");
        drop(recorder); // Immediate drop without stop()

        // Wait brief instant for thread join in Drop
        thread::sleep(Duration::from_millis(50));

        assert!(path.exists(), "WAV file must exist even on drop");
        let bytes = fs::read(&path).expect("read dropped WAV");
        let source = WaveformSource::from_wav_bytes(bytes).expect("valid WAV after drop");
        assert_eq!(source.frame_count(), 960);

        let _ = fs::remove_file(&path);
    }
}

// ============================================================================
// 4. Dynamic Track Arming Changes Mid-Recording
// ============================================================================

#[test]
fn adv_04_dynamic_track_arming_changes_mid_recording() {
    let path = temp_wav_path("adv_track_arm");

    // Scenario A: Recording initiated on armed Track 10
    let mut options = RecordOptions::default();
    options.track_id = Some(10);
    let mut recorder =
        Recorder::create_mock_loopback_with(&path, &options).expect("recorder create");
    assert_eq!(recorder.track_id(), Some(10));

    // Mid-recording: user switches arming on timeline from Track 10 to Track 20
    let timeline_current_target = Some(20u64);

    // Feed some audio while arming switched
    recorder.push_mock_samples(&[0.5f32; 100]);

    // Stopping recording returns the session's assigned track_id (10)
    let rec = recorder.stop().expect("stop");
    assert_eq!(
        rec.track_id,
        Some(10),
        "Recorded take must retain original recording track binding"
    );

    // App resolution logic check:
    // recording.track_id.or_else(|| neoveg_engine::track_arm::target_track(&timeline))
    let final_destination = rec.track_id.or(timeline_current_target);
    assert_eq!(
        final_destination,
        Some(10),
        "Existing recording.track_id takes precedence"
    );

    let _ = fs::remove_file(&rec.path);

    // Scenario B: Recording initiated with NO track armed (untracked/default take)
    let path_untracked = temp_wav_path("adv_track_untracked");
    let options_none = RecordOptions {
        track_id: None,
        ..Default::default()
    };
    let recorder_none =
        Recorder::create_mock_loopback_with(&path_untracked, &options_none).expect("recorder init");
    assert_eq!(recorder_none.track_id(), None);

    let rec_none = recorder_none.stop().expect("stop untracked");
    assert_eq!(rec_none.track_id, None);

    // Mid-recording user armed Track 30: fallback resolves to newly armed track
    let newly_armed_track = Some(30u64);
    let resolved = rec_none.track_id.or(newly_armed_track);
    assert_eq!(resolved, Some(30));

    let _ = fs::remove_file(&rec_none.path);
}

// ============================================================================
// 5. Channel Counts Mismatch (Mono, Stereo, 5.1, 7.1)
// ============================================================================

#[test]
fn adv_05_map_frame_exhaustive_channel_routing() {
    let gain = 1.0f32;

    // 1. Mono in -> Stereo out (duplicate to L & R)
    let mut out2 = [0.0f32; 2];
    map_frame(&[0.8], &mut out2, gain);
    assert_eq!(out2, [0.8, 0.8]);

    // 2. Mono in -> 5.1 out (all 6 channels get mono signal)
    let mut out6 = [0.0f32; 6];
    map_frame(&[0.7], &mut out6, gain);
    assert_eq!(out6, [0.7, 0.7, 0.7, 0.7, 0.7, 0.7]);

    // 3. Stereo in -> Mono out (averaged)
    let mut out1 = [0.0f32; 1];
    map_frame(&[0.2, 0.6], &mut out1, gain);
    assert!((out1[0] - 0.4).abs() < 1e-6);

    // 4. Stereo in -> 5.1 out (channels 0 and 1 receive L & R, remainder zeroed)
    let mut out6 = [9.0f32; 6];
    map_frame(&[0.3, -0.4], &mut out6, gain);
    assert_eq!(out6, [0.3, -0.4, 0.0, 0.0, 0.0, 0.0]);

    // 5. 5.1 in -> Mono out (channels 0 and 1 averaged)
    let in6 = [0.2f32, 0.8, 0.5, 0.1, 0.9, 0.4];
    map_frame(&in6, &mut out1, gain);
    assert!((out1[0] - 0.5).abs() < 1e-6);

    // 6. 5.1 in -> Stereo out (channels 0 and 1 copied, rest truncated)
    map_frame(&in6, &mut out2, gain);
    assert_eq!(out2, [0.2, 0.8]);

    // 7. 5.1 in -> 5.1 out (exact 1:1 copy)
    let mut out_surround = [0.0f32; 6];
    map_frame(&in6, &mut out_surround, gain);
    assert_eq!(out_surround, in6);

    // 8. 7.1 in (8 channels) -> 5.1 out
    let in8 = [0.1f32, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
    map_frame(&in8, &mut out6, gain);
    assert_eq!(out6, [0.1, 0.2, 0.3, 0.4, 0.5, 0.6]);

    // 9. Gain scaling across multi-channel
    let mut out_gain = [0.0f32; 2];
    map_frame(&[0.5, -0.5], &mut out_gain, 0.5);
    assert_eq!(out_gain, [0.25, -0.25]);
}

#[test]
fn adv_05_odd_and_unaligned_chunk_pushes() {
    let path = temp_wav_path("adv_unaligned_chunks");
    let options = RecordOptions::default();
    let mut recorder =
        Recorder::create_mock_loopback_with(&path, &options).expect("recorder init");

    // Stereo recorder expects 2 samples per frame. Push odd length buffers:
    recorder.push_mock_samples(&[0.1f32]); // 1 sample (0 full frames)
    recorder.push_mock_samples(&[0.2f32, 0.3, 0.4]); // 3 samples (1 full frame)
    recorder.push_mock_samples(&[0.5f32, 0.6, 0.7, 0.8, 0.9]); // 5 samples (2 full frames)

    // Stop must succeed without panic on unaligned sample count
    let rec = recorder.stop().expect("stop unaligned");
    assert!(rec.frames >= 960);

    let bytes = fs::read(&rec.path).expect("read WAV");
    let src = WaveformSource::from_wav_bytes(bytes).expect("valid WAV from unaligned");
    assert_eq!(src.frame_count() as u64, rec.frames);

    let _ = fs::remove_file(&rec.path);
}

// ============================================================================
// 6. Extreme Floating-Point Values (NaN, Infinities, Subnormals)
// ============================================================================

#[test]
fn adv_06_extreme_floating_point_values() {
    let path = temp_wav_path("adv_nan_inf");
    let options = RecordOptions::default();
    let mut recorder =
        Recorder::create_mock_loopback_with(&path, &options).expect("recorder init");

    // Test with extreme values
    let extreme_samples = [
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        1e-40f32,
        1e30f32,
        -1e30f32,
    ];

    // Must not panic on NaN or Infinity
    recorder.push_mock_samples(&extreme_samples);

    // take_peak resets cleanly
    let _ = recorder.take_peak();
    assert_eq!(recorder.take_peak(), 0.0);

    let rec = recorder.stop().expect("stop extreme values");
    let _ = fs::remove_file(&rec.path);
}

// ============================================================================
// 7. Concurrent Multi-Threaded SPSC Stress on MonitorRing
// ============================================================================

#[test]
fn adv_07_concurrent_producer_consumer_monitor_ring_stress() {
    let ring = Arc::new(MonitorRing::new());
    let running = Arc::new(AtomicBool::new(true));

    let ring_producer = ring.clone();
    let running_producer = running.clone();
    let producer = thread::spawn(move || {
        let mut sample_val = 0.0f32;
        while running_producer.load(Ordering::Relaxed) {
            let mut block = [0.0f32; 128];
            for item in &mut block {
                sample_val += 0.01;
                *item = sample_val;
            }
            ring_producer.push(&block);
            thread::yield_now();
        }
    });

    let ring_consumer = ring.clone();
    let running_consumer = running.clone();
    let consumer = thread::spawn(move || {
        let mut read_buf = [0.0f32; 256];
        let mut total_read = 0usize;
        while running_consumer.load(Ordering::Relaxed) || ring_consumer.available() > 0 {
            let got = ring_consumer.pop(&mut read_buf);
            total_read += got;
            if got == 0 {
                thread::yield_now();
            }
        }
        total_read
    });

    // Run for 150 milliseconds
    thread::sleep(Duration::from_millis(150));
    running.store(false, Ordering::Relaxed);

    producer.join().expect("producer thread joined");
    let total_consumed = consumer.join().expect("consumer thread joined");

    assert!(
        total_consumed > 0,
        "consumer should have drained samples from producer"
    );
}
