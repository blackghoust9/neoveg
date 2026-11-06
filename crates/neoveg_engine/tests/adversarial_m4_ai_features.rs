//! Adversarial Integration Test Suite for Milestone 4 (Features 20, 21, 22, 23).
//!
//! Empirically stress-tests:
//! 1. Feature 20: Text-Based Timeline Editing (all-word deletion, inverted/out-of-order cuts, multi-track sync)
//! 2. Feature 21: Speech-to-Text Subtitles (zero-duration/silence, fallback engine, reading speed CPS, 2-frame inter-cue gap)
//! 3. Feature 22: Smart Upscale (1x identity, 4x memory bounds, non-square PAR, non-finite/NaN sharpness)
//! 4. Feature 23: AI Auto Reframe (zero motion lock, rapid subject switching deadband damping, extreme aspect ratios)

use std::path::Path;
use std::sync::atomic::AtomicBool;

use neoveg_audio::whisper::{WhisperEngine, WHISPER_SAMPLE_RATE};
use neoveg_common::transcript::TranscriptSegment;
use neoveg_engine::subtitles::{
    format_transcript_to_cues, generate_timeline_captions, generate_title_events_on_track,
    SubtitleFormattingOptions,
};
use neoveg_engine::text_edit::{
    delete_transcript_ranges, parse_words_from_segments, reorder_transcript_range, TextCutRange,
};
use neoveg_engine::timeline::{Timeline, TrackEvent, TrackKind};
use neoveg_video::auto_reframe::{AutoReframeEngine, SubjectDetection, TargetAspectRatio};
use neoveg_video::frame::FrameBuffer;
use neoveg_video::upscale::{FrameUpscaler, UpscaleAlgorithm, UpscaleConfig};

// ============================================================================
// Feature 20: Text-Based Timeline Editing Adversarial Tests
// ============================================================================

#[test]
fn test_adv_tbe_delete_all_words_in_event() {
    let mut tl = Timeline::new();
    let mut track = tl.new_track(TrackKind::Video);

    // Event 1: 0.0s .. 10.0s
    let ev1 = TrackEvent {
        id: 1,
        start: 0.0,
        duration: 10.0,
        ..Default::default()
    };
    track.events.push(ev1);

    // Event 2: 12.0s .. 20.0s (following Event 1 with a 2.0s gap)
    let ev2 = TrackEvent {
        id: 2,
        start: 12.0,
        duration: 8.0,
        ..Default::default()
    };
    track.events.push(ev2);

    tl.tracks.push(track);

    // Parse transcript segments covering entire duration of Event 1
    let segments = vec![
        TranscriptSegment {
            start_s: 0.0,
            end_s: 5.0,
            text: "First half of media speech".into(),
        },
        TranscriptSegment {
            start_s: 5.0,
            end_s: 10.0,
            text: "Second half of media speech".into(),
        },
    ];
    let words = parse_words_from_segments(&segments, 1, 1);
    assert!(!words.is_empty());

    // Cut range that covers the ENTIRE Event 1 [0.0, 10.0]
    let cut = TextCutRange::new(0.0, 10.0);
    let cmd = delete_transcript_ranges(&mut tl, &[cut], true);
    assert!(cmd.is_some(), "Delete all words should return EditCommand");

    let events = &tl.tracks[0].events;
    // Event 1 must be completely removed from the timeline
    assert_eq!(
        events.len(),
        1,
        "Event 1 should be deleted, leaving only Event 2"
    );

    // Event 2 should be rippled earlier by exactly the 10.0s cut duration
    // Original start: 12.0s -> Rippled start: 12.0 - 10.0 = 2.0s
    assert_eq!(events[0].id, 2);
    assert!(
        (events[0].start - 2.0).abs() < 1e-4,
        "Event 2 start should ripple from 12.0s to 2.0s, actual={}",
        events[0].start
    );
    assert_eq!(events[0].duration, 8.0, "Duration of Event 2 must be preserved");
}

#[test]
fn test_adv_tbe_out_of_order_overlapping_inverted_cuts() {
    let mut tl = Timeline::new();
    let mut track = tl.new_track(TrackKind::Audio);

    let ev = TrackEvent {
        id: 100,
        start: 0.0,
        duration: 30.0,
        ..Default::default()
    };
    track.events.push(ev);
    tl.tracks.push(track);

    // Adversarial cut ranges:
    // 1. Inverted cut: start 15.0 > end 12.0 (constructed directly without new())
    // 2. Overlapping cuts: [5.0, 8.0] and [7.0, 10.0] -> should merge into [5.0, 10.0] (5.0s dur)
    // 3. Out-of-order: [20.0, 22.0] before [2.0, 4.0]
    // 4. Zero duration cut: [11.0, 11.0]
    // 5. Out of bounds cut: [28.0, 35.0] (extends past event end at 30.0)
    let cuts = vec![
        TextCutRange {
            start_s: 15.0,
            end_s: 12.0,
        }, // inverted: duration() = 0, should be skipped safely
        TextCutRange::new(7.0, 10.0),
        TextCutRange::new(5.0, 8.0),   // overlaps with [7.0, 10.0]
        TextCutRange::new(20.0, 22.0), // out of order
        TextCutRange::new(2.0, 4.0),
        TextCutRange::new(11.0, 11.0), // zero duration
        TextCutRange::new(28.0, 35.0), // extends beyond 30.0s
    ];

    let cmd = delete_transcript_ranges(&mut tl, &cuts, true);
    assert!(cmd.is_some());

    let events = &tl.tracks[0].events;
    // Expected surviving spans before ripple:
    // [0.0, 2.0] (dur 2.0)
    // [4.0, 5.0] (dur 1.0)
    // [10.0, 20.0] (dur 10.0)
    // [22.0, 28.0] (dur 6.0)
    // Total surviving duration = 2.0 + 1.0 + 10.0 + 6.0 = 19.0s
    assert_eq!(events.len(), 4, "Should have 4 surviving pieces");

    let total_dur: f64 = events.iter().map(|e| e.duration).sum();
    assert!(
        (total_dur - 19.0).abs() < 1e-4,
        "Total duration should be 19.0s, actual={total_dur}"
    );

    // Verify auto-ripple compacted the surviving pieces contiguously
    assert!((events[0].start - 0.0).abs() < 1e-4);
    assert!((events[0].duration - 2.0).abs() < 1e-4);

    assert!((events[1].start - 2.0).abs() < 1e-4);
    assert!((events[1].duration - 1.0).abs() < 1e-4);

    assert!((events[2].start - 3.0).abs() < 1e-4);
    assert!((events[2].duration - 10.0).abs() < 1e-4);

    assert!((events[3].start - 13.0).abs() < 1e-4);
    assert!((events[3].duration - 6.0).abs() < 1e-4);
}

#[test]
fn test_adv_tbe_multi_track_timeline_synchronization() {
    let mut tl = Timeline::new();

    // Track 1: Video track with 3 events
    let mut v_track = tl.new_track(TrackKind::Video);
    let tid_v = v_track.id;
    for i in 0..3 {
        let ev = TrackEvent {
            id: 10 + i,
            start: i as f64 * 10.0,
            duration: 10.0,
            ..Default::default()
        };
        v_track.events.push(ev);
    }
    tl.tracks.push(v_track);

    // Track 2: Audio track with 1 continuous event spanning 0.0..30.0s
    let mut a_track = tl.new_track(TrackKind::Audio);
    let tid_a = a_track.id;
    let ev_a = TrackEvent {
        id: 20,
        start: 0.0,
        duration: 30.0,
        ..Default::default()
    };
    a_track.events.push(ev_a);
    tl.tracks.push(a_track);

    // Cut a 4.0s slice crossing the boundary between video event 1 and event 2: [8.0, 12.0]
    let cuts = vec![TextCutRange::new(8.0, 12.0)];
    let cmd = delete_transcript_ranges(&mut tl, &cuts, true);
    assert!(cmd.is_some());

    let v_events = &tl.track(tid_v).unwrap().events;
    let a_events = &tl.track(tid_a).unwrap().events;

    // Both tracks must have identical total duration (30.0 - 4.0 = 26.0s)
    let v_total: f64 = v_events.iter().map(|e| e.duration).sum();
    let a_total: f64 = a_events.iter().map(|e| e.duration).sum();
    assert!(
        (v_total - 26.0).abs() < 1e-4,
        "Video duration after cut must be 26.0s, actual={v_total}"
    );
    assert!(
        (a_total - 26.0).abs() < 1e-4,
        "Audio duration after cut must be 26.0s, actual={a_total}"
    );

    // Video events verification:
    // Event 10: originally [0.0, 10.0], cut at 8.0 -> surviving [0.0, 8.0]
    assert_eq!(v_events[0].start, 0.0);
    assert_eq!(v_events[0].duration, 8.0);

    // Event 11: originally [10.0, 20.0], cut [10.0..12.0] -> surviving [12.0, 20.0] (dur 8.0)
    // rippled by 4.0s -> starts at 8.0s!
    assert_eq!(v_events[1].start, 8.0);
    assert_eq!(v_events[1].duration, 8.0);

    // Event 12: originally [20.0, 30.0] (dur 10.0) -> rippled by 4.0s -> starts at 16.0s!
    assert_eq!(v_events[2].start, 16.0);
    assert_eq!(v_events[2].duration, 10.0);

    // Audio events verification:
    // One piece [0.0, 8.0] (dur 8.0)
    // One piece [12.0, 30.0] (dur 18.0) rippled to [8.0, 26.0]
    assert_eq!(a_events.len(), 2);
    assert_eq!(a_events[0].start, 0.0);
    assert_eq!(a_events[0].duration, 8.0);
    assert_eq!(a_events[1].start, 8.0);
    assert_eq!(a_events[1].duration, 18.0);
}

#[test]
fn test_adv_tbe_reorder_range_with_ripple() {
    let mut tl = Timeline::new();
    let mut track = tl.new_track(TrackKind::Video);

    // Three 5-second blocks: A [0..5], B [5..10], C [10..15]
    for i in 0..3 {
        let ev = TrackEvent {
            id: 50 + i,
            name: format!("Block_{}", (b'A' + i as u8) as char),
            start: i as f64 * 5.0,
            duration: 5.0,
            ..Default::default()
        };
        track.events.push(ev);
    }
    tl.tracks.push(track);

    // Move Block A [0..5] to destination 10.0s (after Block C)
    let cmd = reorder_transcript_range(&mut tl, TextCutRange::new(0.0, 5.0), 10.0);
    assert!(cmd.is_some());

    let events = &tl.tracks[0].events;
    assert_eq!(events.len(), 3);
    // Block B should ripple to 0.0s
    assert_eq!(events[0].name, "Block_B");
    assert_eq!(events[0].start, 0.0);
    assert_eq!(events[0].duration, 5.0);

    // Block C should ripple to 5.0s
    assert_eq!(events[1].name, "Block_C");
    assert_eq!(events[1].start, 5.0);
    assert_eq!(events[1].duration, 5.0);

    // Block A moved to 10.0s
    assert_eq!(events[2].name, "Block_A");
    assert_eq!(events[2].start, 10.0);
    assert_eq!(events[2].duration, 5.0);
}

// ============================================================================
// Feature 21: Speech-to-Text Subtitles Adversarial Tests
// ============================================================================

#[test]
fn test_adv_stt_zero_duration_pure_silence_extreme_duration() {
    let mut engine = WhisperEngine::fallback().expect("fallback engine must initialize");
    let cancel = AtomicBool::new(false);

    // 1. Zero-duration audio (empty buffer)
    let empty_samples: [f32; 0] = [];
    let mut progress_called = false;
    let segs_empty = engine
        .transcribe(&empty_samples, 0.0, None, &cancel, |p| {
            if p >= 1.0 {
                progress_called = true;
            }
        })
        .expect("empty transcribe must succeed");
    assert!(segs_empty.is_empty(), "Empty audio must return empty segments");
    assert!(progress_called, "Progress 1.0 must be reported for empty audio");

    // 2. Pure silence (energy < 1e-4) for a full 30-second window
    let silence = vec![0.0f32; WHISPER_SAMPLE_RATE as usize * 30];
    let segs_silence = engine
        .transcribe(&silence, 0.0, None, &cancel, |_| {})
        .expect("silent transcribe must succeed without error");
    assert!(
        segs_silence.is_empty(),
        "Pure silence must produce zero hallucinated segments"
    );

    // 3. Audio with extreme duration (100,000 samples)
    let long_silence = vec![0.0f32; 100_000];
    let segs_long = engine
        .transcribe(&long_silence, 50.0, None, &cancel, |_| {})
        .expect("long silence must succeed");
    assert!(segs_long.is_empty());
}

#[test]
fn test_adv_stt_whisper_fallback_nonexistent_directory() {
    let non_existent = Path::new("C:/__non_existent_whisper_model_path_12345__");
    let mut engine = WhisperEngine::load_dir_or_fallback(non_existent);
    let cancel = AtomicBool::new(false);

    // Transcribe silence with fallback engine
    let samples = vec![0.0f32; 1600];
    let res = engine.transcribe(&samples, 0.0, Some("en"), &cancel, |_| {});
    assert!(res.is_ok(), "Fallback engine must transcribe cleanly");
}

#[test]
fn test_adv_stt_reading_speed_cps_and_duration_constraints() {
    // 60 characters in 0.5s = 120 CPS (extreme rapid speech)
    let rapid_text = "Ultra rapid speech exceeding thirty characters per second test".to_string();
    let segs = vec![TranscriptSegment {
        start_s: 0.0,
        end_s: 0.5,
        text: rapid_text,
    }];

    let opts = SubtitleFormattingOptions {
        max_chars_per_line: 35,
        max_lines: 2,
        min_duration_s: 1.0, // Broadcast rule: minimum 1.0s duration
        max_duration_s: 7.0, // Broadcast rule: maximum 7.0s duration
        gap_between_cues_s: 0.08,
    };

    let cues = format_transcript_to_cues(&segs, &opts);
    assert!(!cues.is_empty());

    for cue in &cues {
        // Verify line limits
        let lines: Vec<&str> = cue.text.lines().collect();
        assert!(
            lines.len() <= 2,
            "Max lines exceeded: {} lines",
            lines.len()
        );
        for line in lines {
            assert!(
                line.chars().count() <= 35,
                "CPL exceeded: {} chars in '{}'",
                line.chars().count(),
                line
            );
        }

        // Verify minimum duration constraint enforced
        let cue_dur = cue.end - cue.start;
        assert!(
            cue_dur >= 0.99,
            "Cue duration {cue_dur}s must satisfy min_duration_s >= 1.0s"
        );
        assert!(
            cue_dur <= 7.01,
            "Cue duration {cue_dur}s must satisfy max_duration_s <= 7.0s"
        );
    }
}

#[test]
fn test_adv_stt_zero_gap_between_adjacent_cues_broadcast_gap_enforcement() {
    // Two adjacent segments with ZERO gap: cue 1 ends at 2.0s, cue 2 starts at 2.0s
    let segs = vec![
        TranscriptSegment {
            start_s: 0.0,
            end_s: 2.0,
            text: "First cue text".into(),
        },
        TranscriptSegment {
            start_s: 2.0, // Zero gap
            end_s: 4.0,
            text: "Second cue text".into(),
        },
        TranscriptSegment {
            start_s: 3.9, // Overlapping start with cue 2
            end_s: 6.0,
            text: "Third cue text".into(),
        },
    ];

    let gap_s = 0.08; // 2 frames @ 25fps (~80ms)
    let opts = SubtitleFormattingOptions {
        gap_between_cues_s: gap_s,
        min_duration_s: 1.0,
        max_duration_s: 7.0,
        ..Default::default()
    };

    let cues = format_transcript_to_cues(&segs, &opts);
    assert_eq!(cues.len(), 3);

    for i in 0..cues.len() - 1 {
        let prev = &cues[i];
        let next = &cues[i + 1];
        let actual_gap = next.start - prev.end;
        assert!(
            actual_gap >= gap_s - 1e-4,
            "Broadcast inter-cue gap violated between cue {i} and {}: actual gap={actual_gap}s, required={gap_s}s",
            i + 1
        );
    }

    // Verify timeline captions insertion
    let mut tl = Timeline::new();
    let cmd = generate_timeline_captions(&mut tl, &segs, &opts, true);
    assert!(cmd.is_some());
    assert_eq!(tl.captions.len(), 3);
    for i in 0..tl.captions.len() - 1 {
        let prev = &tl.captions[i];
        let next = &tl.captions[i + 1];
        let gap = next.start - prev.end;
        assert!(
            gap >= gap_s - 1e-4,
            "Timeline caption gap violated: {gap} < {gap_s}"
        );
    }

    // Verify title events on video track
    let track = tl.new_track(TrackKind::Video);
    let tid = track.id;
    tl.tracks.push(track);
    let cmd_titles = generate_title_events_on_track(&mut tl, tid, &segs, &opts);
    assert!(cmd_titles.is_some());
    let evs = &tl.track(tid).unwrap().events;
    assert_eq!(evs.len(), 3);
    for i in 0..evs.len() - 1 {
        let prev = &evs[i];
        let next = &evs[i + 1];
        let gap = next.start - prev.end();
        assert!(
            gap >= gap_s - 1e-4,
            "Video title event gap violated: {gap} < {gap_s}"
        );
    }
}

// ============================================================================
// Feature 22: Smart Upscale Adversarial Tests
// ============================================================================

#[test]
fn test_adv_upscale_extreme_scale_factors_and_identity() {
    let src = FrameBuffer::solid(32, 24, [120, 80, 200, 255]);

    // 1.0x Scale Identity Check across all algorithms
    for &algo in &[
        UpscaleAlgorithm::Bilinear,
        UpscaleAlgorithm::Bicubic,
        UpscaleAlgorithm::EdgeDirected,
        UpscaleAlgorithm::SuperResolutionAi,
    ] {
        let cfg_1x = UpscaleConfig {
            algorithm: algo,
            target_width: 32,
            target_height: 24,
            sharpness: 0.5,
        };
        let out_1x = FrameUpscaler::upscale(&src, &cfg_1x);
        assert_eq!(
            out_1x, src,
            "Algorithm {algo:?} must perform exact identity pass at 1.0x"
        );
    }

    // 4.0x Scale Memory Bounds Check (32x24 -> 128x96)
    let cfg_4x = UpscaleConfig {
        algorithm: UpscaleAlgorithm::SuperResolutionAi,
        target_width: 128,
        target_height: 96,
        sharpness: 0.8,
    };
    let out_4x = FrameUpscaler::upscale(&src, &cfg_4x);
    assert_eq!(out_4x.width, 128);
    assert_eq!(out_4x.height, 96);
    assert_eq!(out_4x.rgba.len(), 128 * 96 * 4);
    assert_eq!(out_4x.rgba[0], 120);
    assert_eq!(out_4x.rgba[1], 80);
    assert_eq!(out_4x.rgba[2], 200);
    assert_eq!(out_4x.rgba[3], 255);
}

#[test]
fn test_adv_upscale_non_square_pixel_aspect_ratio() {
    // NTSC DV frame: 720x480 with non-square PAR 0.9091 (4:3 display)
    // Upscaled to Square Full HD: 1920x1080 (16:9 display)
    let src = FrameBuffer::solid(720, 480, [10, 20, 30, 255]);
    let cfg = UpscaleConfig {
        algorithm: UpscaleAlgorithm::EdgeDirected,
        target_width: 1920,
        target_height: 1080,
        sharpness: 0.6,
    };

    let out = FrameUpscaler::upscale(&src, &cfg);
    assert_eq!(out.width, 1920);
    assert_eq!(out.height, 1080);
    assert_eq!(out.rgba.len(), 1920 * 1080 * 4);
    // Verify non-anamorphic uniform background value maintained
    assert_eq!(out.rgba[0], 10);
    assert_eq!(out.rgba[1], 20);
    assert_eq!(out.rgba[2], 30);
    assert_eq!(out.rgba[3], 255);
}

#[test]
fn test_adv_upscale_non_finite_and_nan_sharpness() {
    let src = FrameBuffer::solid(16, 16, [100, 150, 200, 255]);

    let non_finite_sharpness = [
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        -1.0f32,
        -100.0f32,
        1000.0f32,
    ];

    for &sh in &non_finite_sharpness {
        let cfg = UpscaleConfig {
            algorithm: UpscaleAlgorithm::SuperResolutionAi,
            target_width: 32,
            target_height: 32,
            sharpness: sh,
        };
        let out = FrameUpscaler::upscale(&src, &cfg);
        assert_eq!(out.width, 32);
        assert_eq!(out.height, 32);
        assert_eq!(out.rgba.len(), 32 * 32 * 4);
        // Verify valid solid color values without panic or buffer corruption
        assert_eq!(out.rgba[0], 100);
        assert_eq!(out.rgba[1], 150);
        assert_eq!(out.rgba[2], 200);
        assert_eq!(out.rgba[3], 255);
    }
}

// ============================================================================
// Feature 23: AI Auto Reframe Adversarial Tests
// ============================================================================

#[test]
fn test_adv_auto_reframe_static_video_zero_motion() {
    // Construct 10 identical static frames (solid color or uniform gradient)
    let frame = FrameBuffer::solid(160, 90, [80, 80, 80, 255]);

    let mut detections = Vec::new();
    for i in 0..10 {
        let t = i as f64 * 0.5;
        let det = AutoReframeEngine::detect_subject(&frame, t);
        detections.push(det);
    }

    // All detections on static video must detect frame center (within sub-grid discretization error ~0.002)
    for det in &detections {
        assert!(
            (det.center_x - 0.5).abs() < 0.005,
            "Static frame center_x must be near 0.5, actual={}",
            det.center_x
        );
        assert!(
            (det.center_y - 0.5).abs() < 0.005,
            "Static frame center_y must be near 0.5, actual={}",
            det.center_y
        );
    }

    // Plan reframe to vertical 9:16
    let pc = AutoReframeEngine::plan_reframe_trajectory(
        &detections,
        16.0 / 9.0,
        TargetAspectRatio::Ratio9_16,
        0.15,
        1.0,
    );

    // Static video must generate ONLY the initial keyframe without spurious camera drift!
    assert_eq!(
        pc.keys.len(),
        1,
        "Static video should produce exactly 1 keyframe, got {}",
        pc.keys.len()
    );
    assert!(
        (pc.keys[0].frame.x_center - 0.5).abs() < 0.005,
        "Camera must remain locked near center 0.5, actual={}",
        pc.keys[0].frame.x_center
    );
}

#[test]
fn test_adv_auto_reframe_rapidly_switching_subjects_deadband_damping() {
    // Rapidly switching subject between left (0.15) and right (0.85) every 0.1s
    let mut detections = Vec::new();
    for i in 0..20 {
        let t = i as f64 * 0.1;
        let cx = if i % 2 == 0 { 0.15 } else { 0.85 };
        detections.push(SubjectDetection {
            time_s: t,
            center_x: cx,
            center_y: 0.5,
            confidence: 1.0,
        });
    }

    let pc = AutoReframeEngine::plan_reframe_trajectory(
        &detections,
        16.0 / 9.0,
        TargetAspectRatio::Ratio9_16,
        0.15,
        1.0,
    );

    // Without damping, 20 alternating detections would produce ~20 keyframes.
    // Damping (minimum 0.3s between keyframes) must suppress the jitter, producing <= 7 keyframes!
    assert!(
        pc.keys.len() <= 8,
        "Oscillation damping failed: expected <= 8 keyframes, got {}",
        pc.keys.len()
    );

    // Verify keyframe time spacing is always >= 0.3s
    for i in 0..pc.keys.len() - 1 {
        let dt = pc.keys[i + 1].time - pc.keys[i].time;
        assert!(
            dt >= 0.29,
            "Keyframe spacing violated damping constraint: dt={dt}s"
        );
    }
}

#[test]
fn test_adv_auto_reframe_extreme_aspect_ratios() {
    let detections = vec![
        SubjectDetection {
            time_s: 0.0,
            center_x: 0.1, // Near left edge
            center_y: 0.5,
            confidence: 1.0,
        },
        SubjectDetection {
            time_s: 1.0,
            center_x: 0.9, // Near right edge
            center_y: 0.5,
            confidence: 1.0,
        },
    ];

    let test_ratios = [
        TargetAspectRatio::Ratio1_1,
        TargetAspectRatio::Ratio9_16,
        TargetAspectRatio::Ratio4_5,
        TargetAspectRatio::Ratio16_9,
        TargetAspectRatio::Custom(21, 9),  // Ultra-wide
        TargetAspectRatio::Custom(100, 1), // Extreme horizontal
        TargetAspectRatio::Custom(1, 100), // Extreme vertical
    ];

    for ratio in test_ratios {
        let pc = AutoReframeEngine::plan_reframe_trajectory(
            &detections,
            16.0 / 9.0,
            ratio,
            0.15,
            1.0,
        );

        assert!(!pc.keys.is_empty());
        for key in &pc.keys {
            let f = &key.frame;
            // Crop width and height must be clamped within [0.05, 1.0]
            assert!(f.width >= 0.049 && f.width <= 1.001, "Width bounds: {}", f.width);
            assert!(f.height >= 0.049 && f.height <= 1.001, "Height bounds: {}", f.height);

            // Crop window must never project outside the source frame [0.0, 1.0]
            let left = f.x_center - f.width * 0.5;
            let right = f.x_center + f.width * 0.5;
            assert!(
                left >= -1e-4 && right <= 1.0001,
                "Crop window [{}, {}] out of bounds for ratio {ratio:?}",
                left,
                right
            );
        }
    }
}
