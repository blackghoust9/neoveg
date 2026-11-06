## 2026-11-06T15:31:22Z

You are worker_m4_1 (Milestone 4 Implementation Worker).
Your working directory is: C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\worker_m4_1\

## MANDATORY INTEGRITY WARNING
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

## Mandatory Files to Read Before Starting Work
1. C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\ORIGINAL_REQUEST.md (verbatim user request)
2. C:\Users\paulomatos\Documents\GitHub\neoveg\PROJECT.md (architecture and Feature Inventory)
3. C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\spec_miner_m4_1\handoff.md (VEGAS Pro specs, math, keyboard shortcuts, menus, edge cases)
4. C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\explorer_m4_2\handoff.md (codebase integration, struct definitions, module layouts)
5. C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\explorer_m4_3\handoff.md (pure-CPU low-spec architecture, memory budgets, fallbacks)

## Scope of Work: Milestone 4 (Features 17–23)
Implement the core production feature extensions for Milestone 4:
1. Feature 17: Point & Planar Motion Tracking (crates/neoveg_video/src/tracking.rs and export in lib.rs)
   - TrackingMode (Location, LocationAndRotation, LocationAndScale, Perspective 8-DOF homography), TrackingPrecision (Fast, Balanced, Accurate), TrackingRegion, TrackKeyframe, MotionTrajectory.
   - Use pyramidal optical flow from optical_flow.rs, search displacement over ROI, compute trajectory keyframes, sample_at(t), and conversion to PanCrop or Bézier mask positions.
2. Feature 18: Geometric & Bézier Event Masks (crates/neoveg_common/src/mask.rs, crates/neoveg_video/src/mask.rs, crates/neoveg_engine/src/timeline.rs)
   - MaskFeatherType (None, In, Out, Both), EventMaskKey, EventMask.
   - Add `pub event_mask: Option<neoveg_common::mask::EventMask>` to TrackEvent in neoveg_engine/src/timeline.rs (with #[serde(default, skip_serializing_if = "Option::is_none")]).
   - Implement `apply_event_mask` in neoveg_video/src/mask.rs using rasterized SDF/scanline + fast 3-pass box feathering from fx_mask.rs/neoveg_text.
3. Feature 19: Smart Mask AI Object Segmentation (crates/neoveg_video/src/smart_mask.rs)
   - SmartMaskPrecision (Low, High), SegmentationPrompt (bounding box, positive/negative point prompts), SmartMaskMatte.
   - Pure-CPU fallback segmentation: color seed clustering & morphological dilation / GrabCut energy solver on thumbnails.
   - Vectorization into Bézier FxMask (vectorize_to_bezier) and apply_to_frame.
4. Feature 20: Text-Based Timeline Editing (crates/neoveg_engine/src/text_edit.rs and export in lib.rs)
   - TranscriptWord, TextCutRange, parse_words_from_segments.
   - delete_transcript_ranges with automatic timeline split and ripple_delete gap closure.
   - reorder_transcript_range.
5. Feature 21: Speech-to-Text Subtitle Generation (crates/neoveg_engine/src/subtitles.rs and export in lib.rs, neoveg_audio/src/whisper.rs)
   - SubtitleFormattingOptions (max_chars_per_line, max_lines <= 2, min_duration_s >= 1.0, max_duration_s <= 7.0, gap_between_cues_s >= 2 frames).
   - format_transcript_to_cues, generate_timeline_captions, generate_title_events_on_track.
   - Expose fallback constructor or auto-fallback in whisper.rs so missing disk models gracefully use synthetic/VAD fallbacks in headless test/CI environments.
6. Feature 22: Smart Upscale & Super-Resolution (crates/neoveg_video/src/upscale.rs)
   - UpscaleAlgorithm (Bilinear, Bicubic, EdgeDirected, SuperResolutionAi), UpscaleConfig, FrameUpscaler.
   - Pure-CPU 16-tap Catmull-Rom bicubic and edge-directed directional interpolation (NEDI) + adaptive unsharp masking.
   - Bounded memory execution fitting 8 GB RAM.
7. Feature 23: AI Auto Reframe (crates/neoveg_video/src/auto_reframe.rs and export in lib.rs)
   - TargetAspectRatio (16:9, 9:16 vertical, 1:1 square, 4:5 portrait, custom), SubjectDetection, AutoReframeEngine.
   - detect_subject via optical flow motion + luma variance saliency centroid.
   - plan_reframe_trajectory with deadband camera filter (0.15 crop width) and ease-in-out Cubic Hermite keyframing outputting PanCrop.

## Constraints & Execution Rules
- Pure clean-room Rust, MIT OR Apache-2.0.
- Zero third-party trademarks in runtime code, structs, enums, or UI strings.
- Pure software/CPU fallbacks and mock implementations for all AI features.
- Strict 8 GB RAM execution budget: run ALL Cargo commands sequentially with `-j 1` and `--test-threads=1`.
- NEVER execute `cargo test -p neoveg_ui` (verify `neoveg_ui` strictly with `cargo check -j 1` and `cargo clippy -j 1`).
- Run tests sequentially across crates:
  - `cargo check --workspace -j 1`
  - `cargo clippy --workspace -j 1`
  - `cargo test -j 1 -p neoveg_common -- --test-threads=1`
  - `cargo test -j 1 -p neoveg_video -- --test-threads=1`
  - `cargo test -j 1 -p neoveg_audio -- --test-threads=1`
  - `cargo test -j 1 -p neoveg_engine -- --test-threads=1`
- When complete, write your handoff report to: C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\worker_m4_1\handoff.md
- Send a completion message via send_message to the orchestrator.


## 2026-11-06T15:42:03Z
**Context**: Milestone 4 Implementation (worker_m4_1)
**Content**: Heartbeat status check. 10 minutes have elapsed since dispatch.
**Action**: Please update progress.md with your active status, implemented features, and estimated completion.


## 2026-11-06T15:53:42Z
**Context**: Milestone 4 Implementation (worker_m4_1)
**Content**: Heartbeat check on Features 20–23 progress and test verification.
**Action**: Please update progress.md with your latest completed items.
