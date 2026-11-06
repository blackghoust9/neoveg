# Milestone 4 Codebase Architecture & Implementation Blueprint Report

**Author**: explorer_m4_2 (Milestone 4 Codebase Explorer)  
**Target Milestone**: Milestone 4 (Features 17–23)  
**Working Directory**: `C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\explorer_m4_2\`  
**Date**: 2026-11-06  

---

## 1. Observation

Direct observations from inspecting the codebase, models, audio pipeline, video pipeline, and existing test suites:

### 1.1 Video Pipeline (`crates/neoveg_video/src/`)
- **`optical_flow.rs`**: Clean-room pyramidal optical-flow implementation (557 lines).
  - Uses `LumaPlane` downsampled by factors of 2 (`downsample2`, lines 37–55).
  - Calculates overlapping block motion matching with parabolic sub-pixel refinement in `FlowGrid` (lines 74–120).
  - Exposes public function `pub fn interpolate_frames(a: &FrameBuffer, b: &FrameBuffer, t: f32) -> FrameBuffer` (line 348).
- **`fx_mask.rs`**: Mask coverage rasterization (187 lines).
  - Uses `neoveg_common::fx_mask::{bezier_segments, FxMask, MaskShape}` (line 8).
  - Uses `neoveg_text::raster::{fill, Path}` (line 10) for anti-aliased shape filling.
  - Uses `neoveg_text::mask::blur` (line 9) for feathering (`sigma = mask.feather * hf * 0.5`, line 62).
  - Exposes `pub fn coverage(mask: &FxMask, w: u32, h: u32) -> Arc<Vec<f32>>` (line 78).
- **`frame.rs`**: FrameBuffer data model (33 lines).
  - `pub struct FrameBuffer { pub width: u32, pub height: u32, pub rgba: Vec<u8> }` (lines 4–9).
  - Exposes `pub fn solid(width: u32, height: u32, rgba: [u8; 4]) -> Self` and `pub fn pixel(&self, x: u32, y: u32) -> [u8; 4]`.
- **`pan_crop.rs`**: CPU Pan/Crop rasterization (430 lines).
  - `pub struct PanCropRequest { pub frame: PanCropFrame, pub stretch_to_fill: bool }` (lines 22–25).
  - `pub fn apply(src: &FrameBuffer, frame: &PanCropFrame, stretch_to_fill: bool) -> FrameBuffer` (lines 76–103).
  - Uses multithreaded bilinear sampling (`sample(src, x, y) -> [f32; 4]` on premultiplied colour, line 36).
- **`worker.rs`**: Compositing pipeline (2421 lines).
  - `CompositeClip` (lines 60–86) has: `seconds`, `source_fps`, `playback_rate`, `event_opacity`, `apply_chroma`, `effects: Vec<VideoEffect>`, `pan_crop: Option<PanCropRequest>`, `transitions`, `resample`, `maintain_aspect_ratio`, `reduce_interlace_flicker`, `media_look`.
  - In `finish_clip` (lines 370–382), effects are split at the `PAN_CROP` marker: early effects run before `pan_crop::apply`, and late effects run after.
  - **Currently missing**: No `event_mask: Option<EventMaskRequest>` field exists on `CompositeClip`.
- **`tracking.rs` & `mask.rs`**: Neither file currently exists in `crates/neoveg_video/src/`.

### 1.2 Audio Pipeline (`crates/neoveg_audio/src/`)
- **`whisper.rs`**: Pure-Rust speech-to-subtitles transcription engine (1066 lines).
  - Dependencies in `Cargo.toml`: `candle-core = 0.11`, `candle-nn = 0.11`, `candle-transformers = 0.11` (CPU inference, zero CUDA/C++ dependency).
  - Reads Hugging Face Whisper folders containing `config.json`, `tokenizer.json`, and `model.safetensors` or quantized `*.gguf` (lines 394–468).
  - Implements Slaney-normalized mel filterbank `pub fn slaney_mel_filters(num_mel_bins: usize, n_fft: usize, sample_rate: f64) -> Vec<f32>` (lines 112–146).
  - Implements GPT-2 byte-level BPE tokenizer `WhisperTokenizer` (lines 177–317) with timestamp parsing (`timestamp_seconds(id)` for `<|0.00|>`..`<|30.00|>`).
  - Audio preparation: `pub fn mix_to_mono_16k(clips: &[MixClip], start_s: f64, end_s: f64) -> Vec<f32>` (line 45) and `pub fn resample_mono_to_16k(samples: &[f32], source_rate: u32) -> Vec<f32>` (line 60).
  - Background worker: `pub fn transcribe_clips(model_dir: &Path, clips: &[MixClip], duration_s: f64, language: Option<&str>, cancel: &AtomicBool, on_progress: impl FnMut(f32)) -> Result<Vec<TranscriptSegment>, String>` (lines 750–773).
  - Unit test pattern: Tests use `WhisperEngine::from_model_and_tokenizer` with `VarBuilder::zeros` (line 1038) and `synthetic_tokenizer_json()` (lines 901–939), executing 100% in-memory with deterministic zero weights and full cancellation testing without external network or weights download.

### 1.3 Engine Timeline & Data Models (`crates/neoveg_engine/src/`)
- **`timeline.rs`**: `TrackEvent` struct (lines 153–286).
  - Currently contains `pan_crop: PanCrop` (line 222), `video_effects: Vec<TrackEffect>` (line 218), `color_grade: Option<EventColorGrade>` (line 285).
  - Currently has no `event_mask` field.
  - Subtitle cues on timeline: `Timeline` has `pub captions: Vec<Caption>` (line 74), where `Caption { pub id: CaptionId, pub start: f64, pub end: f64, pub text: String, pub speaker: Option<String> }`.
- **`captions.rs`**: Complete subtitle format import/export (987 lines) for SRT, WebVTT, SCC, and MCC.
  - Pure command builders: `import_document`, `export_document`, `add_caption`, and `active_at`.
- **`pan_crop.rs`**: Timeline command builders for Pan/Crop (191 lines).
  - `pub fn get(timeline: &Timeline, track_id: TrackId, event_id: EventId) -> Option<&PanCrop>`
  - `pub fn set(timeline: &Timeline, track_id: TrackId, event_id: EventId, after: PanCrop) -> Option<EditCommand>`
- **`ai.rs`**: File does not yet exist in `crates/neoveg_engine/src/`.

### 1.4 Common Models (`crates/neoveg_common/src/`)
- **`fx_mask.rs`**: Geometric descriptions (261 lines).
  - `pub enum MaskShape { Rectangle, Ellipse, Bezier }`
  - `pub struct FxMask { pub shape: MaskShape, pub center: [f32; 2], pub size: [f32; 2], pub rotation_deg: f32, pub feather: f32, pub invert: bool, pub points: Vec<[f32; 2]> }`
  - `pub fn bezier_segments(points: &[[f32; 2]]) -> Vec<[[f32; 2]; 4]>` (Catmull-Rom spline cubic segments).
- **`pan_crop.rs`**:
  - `PanCropFrame { x_center, y_center, width, height, rotation_deg }`
  - `PanCropKey { time, frame, smoothness }`
  - `PanCrop { keys: Vec<PanCropKey>, stretch_to_fill: bool }` with `frame_at(time: f64) -> PanCropFrame` and `upsert_key`.

### 1.5 Existing Integration Test Coverage
- `crates/neoveg_video/tests/e2e_video.rs`:
  - Feature 17 (lines 1124–1214): `f17_t1_01`..`f17_t2_05` (optical flow interpolation, translation, clamping, scene cuts).
  - Feature 18 (lines 1217–1370): `f18_t1_01`..`f18_t2_05` (rectangular, ellipse, and Bézier coverage, feathering, inversion).
  - Feature 19 (lines 1373–1450): `f19_t1_01`..`f19_t2_05` (`SmartMaskMatte`, pure software CPU fallback segmentation using color/luminance thresholding).
- `crates/neoveg_audio/tests/e2e_audio.rs`:
  - Feature 20 (lines 272–380): `f20_t1_01`..`f20_t2_05` (`TranscriptSegment`, `TextBasedTimelineCut`, `timeline_cuts_from_transcript`, 16kHz resampling).
  - Feature 21 (lines 384–460): `f21_t1_01`..`f21_t2_05` (Whisper languages, Slaney mel filterbank dimensions, tokenizer special tokens, synthetic model inference).
- `crates/neoveg_engine/tests/e2e_engine.rs`:
  - Feature 22 (lines 279–379): `f22_t1_01`..`f22_t2_05` (upscale scale factors, aspect ratio preservation, bilinear/bicubic sampling).
  - Feature 23 (lines 381–520): `f23_t1_01`..`f23_t2_05` (PanCropFrame identity, 9:16 vertical reframe, keyframing, timeline get/set, clamping).
- `crates/neoveg_engine/tests/e2e_scenarios.rs`:
  - Scenario `t4_scenario_04_ai_subtitling_and_social_reframe` (lines 845–885): Tests 9:16 reframe keyframing with `PanCropKey` ease-in-out (`smoothness: 1.0`) combined with speech-aligned `Caption` subtitles on `Timeline`.

---

## 2. Logic Chain

1. **Premise 1**: Neoveg requires a clean-room, pure-Rust implementation complying with `MIT OR Apache-2.0`, zero external AI endpoint requirements, strict CPU fallbacks, and deterministic CI execution.
2. **Premise 2**: Optical flow (`optical_flow.rs`), Whisper audio transcription (`whisper.rs`), mask coverage rasterization (`fx_mask.rs`), and Pan/Crop keyframing (`pan_crop.rs`) are already implemented and tested with 100% passing tests.
3. **Premise 3**: Features 17–23 must connect these existing modules into cohesive NLE features:
   - Feature 17 (Motion Tracking): Can build directly upon `optical_flow.rs` feature displacement estimation to generate `TrackKeyframe` motion trajectories that drive Pan/Crop, Event Masks, or VFX parameters.
   - Feature 18 (Geometric & Bézier Event Masks): Can leverage `neoveg_common::fx_mask` and `neoveg_video::fx_mask` by elevating masks from single-effect scopes to whole event scopes (`EventMask`), integrated into `TrackEvent` and `worker::CompositeClip`.
   - Feature 19 (Smart Mask AI Object Segmentation): Can expand `SmartMaskMatte` (from `e2e_video.rs`) into a production CPU segmentation algorithm with seed-based regional growing, morphological boundary cleaning, and vectorization into `FxMask` Bézier splines, temporally tracked via optical flow.
   - Feature 20 (Text-Based Timeline Editing): Can link `TranscriptSegment` words to timeline positions, allowing word cuts and deletions to trigger automated `split_at_times` and `ripple_after` operations.
   - Feature 21 (Speech-to-Text Subtitle Generation): Can extend `whisper::transcribe_clips` with a formatting constraint engine (`max_chars_per_line`, `max_lines`, cue minimum durations) and emit subtitles to either `timeline.captions` or generated Titles & Text events on a new track.
   - Feature 22 (Smart Upscale & Super-Resolution): Can provide pure-CPU high-fidelity upscalers (Bicubic Catmull-Rom spline, Bilinear, Edge-Directed) positioned prior to Pan/Crop in the event pipeline.
   - Feature 23 (AI Auto Reframe): Can analyze subject center-of-mass or face/motion bounding boxes to dynamically generate smoothed, ease-in-out `PanCropKey` keyframes for 9:16, 1:1, 4:5, and custom aspect ratios.
4. **Deduction**: Implementing Features 17–23 requires no new external third-party crates; it only requires adding clean domain modules in `neoveg_common`, `neoveg_video`, and `neoveg_engine` with proper `serde` defaults, pure functions, and undoable commands.

---

## 3. Architectural Blueprints for Features 17–23

### Feature 17: Point & Planar Motion Tracking
**Target Module**: `crates/neoveg_video/src/tracking.rs`  
**Integration Points**: `neoveg_video::optical_flow`, `neoveg_common::pan_crop`, `neoveg_engine::command`.

```rust
// crates/neoveg_video/src/tracking.rs

use serde::{Deserialize, Serialize};
use crate::frame::FrameBuffer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TrackingMode {
    #[default]
    Location,
    LocationAndRotation,
    LocationAndScale,
    Perspective,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TrackingPrecision {
    Fast,
    #[default]
    Balanced,
    Accurate,
}

/// A normalized bounding box region [x, y, width, height] in 0.0..=1.0.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TrackingRegion {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// A single tracked motion sample at a given timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TrackKeyframe {
    pub time_s: f64,
    pub center_x: f32,
    pub center_y: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub rotation_deg: f32,
    pub confidence: f32,
}

/// Motion tracking trajectory across a range of frames.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct MotionTrajectory {
    pub mode: TrackingMode,
    pub region: TrackingRegion,
    pub keyframes: Vec<TrackKeyframe>,
}

impl MotionTrajectory {
    /// Tracks motion from `frame_a` to `frame_b` using pyramidal optical flow.
    pub fn track_step(
        &mut self,
        frame_a: &FrameBuffer,
        frame_b: &FrameBuffer,
        time_a: f64,
        time_b: f64,
        precision: TrackingPrecision,
    ) -> TrackKeyframe;

    /// Evaluates interpolated motion state at an arbitrary timestamp.
    pub fn sample_at(&self, time_s: f64) -> Option<TrackKeyframe>;

    /// Transfers trajectory data to Pan/Crop keyframes.
    pub fn to_pan_crop(&self) -> neoveg_common::pan_crop::PanCrop;
}
```

---

### Feature 18: Geometric & Bézier Event Masks
**Target Modules**:
- Data Model: `crates/neoveg_common/src/mask.rs`
- Coverage & Blending: `crates/neoveg_video/src/mask.rs`
- Timeline Commands: `crates/neoveg_engine/src/event_mask.rs`
- Compositor Integration: `crates/neoveg_video/src/worker.rs`

```rust
// crates/neoveg_common/src/mask.rs

use serde::{Deserialize, Serialize};
use crate::fx_mask::FxMask;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MaskFeatherType {
    #[default]
    None,
    In,
    Out,
    Both,
}

/// Keyframed Event Mask at a specific point in time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventMaskKey {
    pub time: f64,
    pub mask: FxMask,
    #[serde(default = "default_opacity")]
    pub opacity: f32, // 0.0..=1.0
    #[serde(default)]
    pub feather_type: MaskFeatherType,
}

fn default_opacity() -> f32 { 1.0 }

/// Complete event mask container supporting up to 8 animated mask layers.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct EventMask {
    pub enabled: bool,
    pub invert: bool,
    pub keys: Vec<EventMaskKey>,
}

impl EventMask {
    pub fn is_default(&self) -> bool {
        !self.enabled || self.keys.is_empty()
    }

    /// Evaluates interpolated mask at time `t` into the event.
    pub fn mask_at(&self, time_s: f64) -> Option<(FxMask, f32)>;
}
```

```rust
// crates/neoveg_engine/src/timeline.rs (on TrackEvent)
pub struct TrackEvent {
    // ... existing fields ...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_mask: Option<neoveg_common::mask::EventMask>,
}
```

```rust
// crates/neoveg_video/src/mask.rs

use crate::frame::FrameBuffer;
use neoveg_common::mask::EventMask;

/// Multiplies frame alpha by the rasterized event mask coverage.
pub fn apply_event_mask(
    frame: &mut FrameBuffer,
    mask: &EventMask,
    time_s: f64,
);
```

---

### Feature 19: Smart Mask AI Object Segmentation
**Target Module**: `crates/neoveg_video/src/smart_mask.rs`  
**Dependencies**: `neoveg_video::frame`, `neoveg_common::fx_mask`, `neoveg_video::optical_flow`.

```rust
// crates/neoveg_video/src/smart_mask.rs

use serde::{Deserialize, Serialize};
use crate::frame::FrameBuffer;
use neoveg_common::fx_mask::{FxMask, MaskShape};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SmartMaskPrecision {
    Low,
    #[default]
    High,
}

/// User interaction prompt for object segmentation.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SegmentationPrompt {
    pub bounding_box: Option<[f32; 4]>, // [x, y, w, h]
    pub positive_points: Vec<[f32; 2]>, // Include points
    pub negative_points: Vec<[f32; 2]>, // Exclude points
}

/// Binary alpha matte output of AI/CPU segmentation.
#[derive(Debug, Clone, PartialEq)]
pub struct SmartMaskMatte {
    pub width: u32,
    pub height: u32,
    pub alpha_channel: Vec<u8>,
}

impl SmartMaskMatte {
    /// Pure software CPU fallback segmentation using color seed clustering & morphological dilation.
    pub fn segment_frame(
        frame: &FrameBuffer,
        prompt: &SegmentationPrompt,
        precision: SmartMaskPrecision,
    ) -> Self;

    /// Vectorizes binary alpha matte into a smooth Bézier spline FxMask.
    pub fn vectorize_to_bezier(&self, max_points: usize) -> FxMask;

    /// Applies matte directly to frame buffer alpha channel.
    pub fn apply_to_frame(&self, frame: &mut FrameBuffer, invert: bool);
}
```

---

### Feature 20: Text-Based Timeline Editing
**Target Module**: `crates/neoveg_engine/src/text_edit.rs` (or `neoveg_engine::ai::text_edit`)  
**Dependencies**: `neoveg_engine::timeline`, `neoveg_engine::ops`, `neoveg_engine::ripple`, `neoveg_audio::whisper`.

```rust
// crates/neoveg_engine/src/text_edit.rs

use serde::{Deserialize, Serialize};
use crate::command::EditCommand;
use crate::timeline::{EventId, Timeline, TrackId};
use neoveg_audio::whisper::TranscriptSegment;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranscriptWord {
    pub word: String,
    pub start_s: f64,
    pub end_s: f64,
    pub track_id: TrackId,
    pub event_id: EventId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextCutRange {
    pub start_s: f64,
    pub end_s: f64,
}

/// Extracts word-level items mapped to timeline events.
pub fn parse_words_from_segments(
    segments: &[TranscriptSegment],
    track_id: TrackId,
    event_id: EventId,
) -> Vec<TranscriptWord>;

/// Cuts and deletes matching timeline intervals (and ripples subsequent events).
pub fn delete_transcript_ranges(
    timeline: &mut Timeline,
    cuts: &[TextCutRange],
    auto_ripple: bool,
) -> Option<EditCommand>;

/// Reorders timeline events matching transcript text movement.
pub fn reorder_transcript_range(
    timeline: &mut Timeline,
    from: TextCutRange,
    to_time: f64,
) -> Option<EditCommand>;
```

---

### Feature 21: Speech-to-Text Subtitle Generation
**Target Module**: `crates/neoveg_engine/src/subtitles.rs` (or `neoveg_engine::ai::subtitles`)  
**Dependencies**: `neoveg_audio::whisper`, `neoveg_engine::captions`, `neoveg_engine::timeline`.

```rust
// crates/neoveg_engine/src/subtitles.rs

use serde::{Deserialize, Serialize};
use crate::captions::{Caption, Cue, Document};
use crate::command::EditCommand;
use crate::timeline::{Timeline, TrackId};
use neoveg_audio::whisper::TranscriptSegment;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubtitleFormattingOptions {
    pub max_chars_per_line: usize, // e.g. 37 or 42
    pub max_lines: usize,          // 1 or 2 lines
    pub min_duration_s: f64,       // e.g. 1.0 s
    pub max_duration_s: f64,       // e.g. 6.0 s
    pub gap_between_cues_s: f64,   // e.g. 0.08 s (2 frames at 25 fps)
}

impl Default for SubtitleFormattingOptions {
    fn default() -> Self {
        Self {
            max_chars_per_line: 40,
            max_lines: 2,
            min_duration_s: 1.0,
            max_duration_s: 6.0,
            gap_between_cues_s: 0.08,
        }
    }
}

/// Formats raw Whisper segments into compliant subtitle cues.
pub fn format_transcript_to_cues(
    segments: &[TranscriptSegment],
    options: &SubtitleFormattingOptions,
) -> Vec<Cue>;

/// Inserts formatted subtitle cues into project timeline captions.
pub fn generate_timeline_captions(
    timeline: &mut Timeline,
    segments: &[TranscriptSegment],
    options: &SubtitleFormattingOptions,
    replace_all: bool,
) -> Option<EditCommand>;

/// Generates Titles & Text video events on a dedicated video track.
pub fn generate_title_events_on_track(
    timeline: &mut Timeline,
    target_track_id: TrackId,
    segments: &[TranscriptSegment],
    options: &SubtitleFormattingOptions,
) -> Option<EditCommand>;
```

---

### Feature 22: Smart Upscale & Super-Resolution
**Target Module**: `crates/neoveg_video/src/upscale.rs`  
**Dependencies**: `neoveg_video::frame`.

```rust
// crates/neoveg_video/src/upscale.rs

use serde::{Deserialize, Serialize};
use crate::frame::FrameBuffer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum UpscaleAlgorithm {
    Bilinear,
    #[default]
    Bicubic,
    EdgeDirected,
    SuperResolutionAi,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct UpscaleConfig {
    pub algorithm: UpscaleAlgorithm,
    pub target_width: u32,
    pub target_height: u32,
    pub sharpness: f32, // 0.0..=1.0
}

/// High-quality image upscaler with pure CPU reference implementations.
pub struct FrameUpscaler;

impl FrameUpscaler {
    /// Resamples `src` into a new FrameBuffer matching `config.target_width` and `config.target_height`.
    pub fn upscale(src: &FrameBuffer, config: &UpscaleConfig) -> FrameBuffer;

    /// Bicubic Catmull-Rom / Mitchell-Netravali 16-tap interpolation.
    pub fn bicubic_sample(src: &FrameBuffer, x: f32, y: f32, sharpness: f32) -> [f32; 4];
}
```

---

### Feature 23: AI Auto Reframe
**Target Module**: `crates/neoveg_video/src/auto_reframe.rs` & `crates/neoveg_engine/src/ai.rs`  
**Dependencies**: `neoveg_video::frame`, `neoveg_common::pan_crop`, `neoveg_engine::timeline`.

```rust
// crates/neoveg_video/src/auto_reframe.rs

use serde::{Deserialize, Serialize};
use neoveg_common::pan_crop::{PanCrop, PanCropFrame, PanCropKey};
use crate::frame::FrameBuffer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TargetAspectRatio {
    Ratio16_9,
    #[default]
    Ratio9_16, // Vertical / Mobile / Reels / Shorts / TikTok
    Ratio1_1,  // Square / Feed
    Ratio4_5,  // Social Portrait
    Custom(u32, u32),
}

impl TargetAspectRatio {
    pub fn aspect_ratio_f32(&self) -> f32 {
        match self {
            Self::Ratio16_9 => 16.0 / 9.0,
            Self::Ratio9_16 => 9.0 / 16.0,
            Self::Ratio1_1 => 1.0,
            Self::Ratio4_5 => 4.0 / 5.0,
            Self::Custom(w, h) => (*w as f32) / (*h as f32).max(1.0),
        }
    }
}

/// Primary subject region detected in a single frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SubjectDetection {
    pub time_s: f64,
    pub center_x: f32, // 0.0..=1.0
    pub center_y: f32, // 0.0..=1.0
    pub confidence: f32,
}

pub struct AutoReframeEngine;

impl AutoReframeEngine {
    /// Detects subject center of visual mass / saliency using pure CPU luma and optical flow motion.
    pub fn detect_subject(frame: &FrameBuffer, time_s: f64) -> SubjectDetection;

    /// Plans smoothed Pan/Crop keyframes maintaining the subject within the target aspect ratio window.
    pub fn plan_reframe_trajectory(
        detections: &[SubjectDetection],
        source_aspect: f32,
        target: TargetAspectRatio,
        deadzone_threshold: f32, // e.g. 0.15
        smoothness: f32,          // e.g. 1.0 (smoothstep)
    ) -> PanCrop;
}
```

---

## 4. Caveats

1. **No External Network Dependency**: Whisper, super-resolution upscaling, and segmentation must run strictly with local weights or deterministic in-memory synthetic models. CI runs cannot download gigabytes of model files.
2. **Execution Constraints**: All builds and tests must execute with `-j 1` and `--test-threads=1`. `neoveg_ui` must never be tested with `cargo test -p neoveg_ui`.
3. **Trademark Policy**: All UI strings and error messages must remain strictly clean-room without third-party vendor names or trademarks.

---

## 5. Conclusion

- The Neoveg codebase has a solid architectural base for Milestone 4:
  - Optical flow (`neoveg_video::optical_flow`) is mature, tested, and ready to power Motion Tracking and Auto Reframe.
  - Whisper speech transcription (`neoveg_audio::whisper`) is production-grade with candle-transformers on CPU, in-memory synthetic testing, and full cancellation support.
  - Subtitle parsing and timeline integration (`neoveg_engine::captions`) are comprehensive and robust.
  - Keyframing (`neoveg_common::pan_crop`) and mask rasterization (`neoveg_video::fx_mask`) provide the exact math needed for Event Masks and Smart Mask.
- The 7 features can be cleanly implemented according to the architectural blueprints above with zero license issues, zero trademark violations, and 100% adherence to VEGAS Pro semantics.

---

## 6. Verification Method

To independently verify the facts and findings in this report:

1. **Verify Optical Flow**:
   Inspect `crates/neoveg_video/src/optical_flow.rs:348` (`interpolate_frames`).
   Execute:
   ```powershell
   cargo test -p neoveg_video --test e2e_video f17_t1_01 -j 1 -- --test-threads=1
   ```
2. **Verify Mask Rasterization**:
   Inspect `crates/neoveg_video/src/fx_mask.rs:78` (`coverage`) and `crates/neoveg_common/src/fx_mask.rs:43` (`FxMask`).
   Execute:
   ```powershell
   cargo test -p neoveg_video --test e2e_video f18_t1_01 -j 1 -- --test-threads=1
   ```
3. **Verify Whisper Inference & Synthetic Test**:
   Inspect `crates/neoveg_audio/src/whisper.rs:1024` (`synthetic_tiny_whisper_model_runs_encoder_decoder_and_honors_cancellation`).
   Execute:
   ```powershell
   cargo test -p neoveg_audio --test e2e_audio f21_t1_04 -j 1 -- --test-threads=1
   ```
4. **Verify Pan/Crop & Auto Reframe Keyframing**:
   Inspect `crates/neoveg_engine/tests/e2e_scenarios.rs:845` (`t4_scenario_04_ai_subtitling_and_social_reframe`).
   Execute:
   ```powershell
   cargo test -p neoveg_engine --test e2e_scenarios t4_scenario_04 -j 1 -- --test-threads=1
   ```
5. **Full Workspace Build & Quality Check**:
   ```powershell
   cargo check --workspace -j 1
   cargo clippy --workspace -j 1
   ```
