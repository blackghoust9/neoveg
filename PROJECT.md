# Project: Neoveg NLE Feature Extensions

## Architecture
Neoveg is a pure clean-room Rust non-linear video editing system structured as a modular Cargo workspace across 14 crates:
- `neoveg_common`: Core data models (project, timecode, preferences, keymaps, i18n, command palette, alerts, color grade types).
- `neoveg_text`: Typography, HarfBuzz shaping, rasterization, glyph masks.
- `neoveg_media`: ISOBMFF/MP4 and MXF container demuxers and bitstream utilities.
- `neoveg_h264`: Clean-room H.264 video codec (CABAC, CAVLC, intra/inter prediction, rate control).
- `neoveg_aac`: Clean-room AAC audio codec (MDCT, Huffman coding, ICS, TNS).
- `neoveg_cfb`: Compound File Binary structured storage.
- `neoveg_aaf`: Advanced Authoring Format (AAF) parser and writer.
- `neoveg_ofx`: OpenFX 1.4 plugin host ABI and subprocess runner.
- `neoveg_ofx_sample`: Clean-room sample OFX plugin for host testing.
- `neoveg_vst3_sample`: Clean-room sample VST3 gain plugin for audio host testing.
- `neoveg_audio`: Audio processing pipeline, DSP, LUFS meter, true peak, mixer, recording, VST3/CLAP hosting, Whisper AI transcription.
- `neoveg_video`: Video processing pipeline, WGPU rendering, 36 VFX, transitions, 3D transforms, color grading, LUT, RAW/CinemaDNG, optical flow.
- `neoveg_engine`: NLE timeline model, edit ops, ripple editing, snapping, takes, track arming, sync links, multicam, interchange (EDL, FCP7, FCPXML, OTIO, AAF).
- `neoveg_ui`: Desktop UI built with egui/eframe, docking windows, panels, dialogs, transport, waveform/timeline renderers.

### Data Flow & Execution Boundaries
1. **Model Decoupling**: Business logic and data structures reside in `neoveg_common`, `neoveg_video`, `neoveg_audio`, and `neoveg_engine`. `neoveg_ui` is strictly a presentation and command dispatch layer.
2. **CPU Fallback Strategy**: Dedicated GPU hardware or external AI services cannot be required for CI/test execution. All GPU effects, OFX plugins, and AI models have deterministic pure-CPU software fallbacks and synthetic in-memory fixtures.
3. **Execution Budget**: Sequential execution with `-j 1` and `--test-threads=1`. `neoveg_ui` is verified exclusively with `cargo check` and `cargo clippy` (never `cargo test -p neoveg_ui`).
4. **Clean-Room License & Trademark**: `MIT OR Apache-2.0` compliant (`deny.toml`), zero GPL/AGPL dependencies, zero third-party registered trademarks in runtime strings or user-visible text.

---

## Feature Inventory
Every feature identified during Phase 0 Survey from `ORIGINAL_REQUEST.md` and `results.md`:

| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | Worktree Lint/Test Stabilization | Clean lint and unit test passage for `neoveg_aaf/src/tests.rs`, `neoveg_video/src/raw.rs`, `neoveg_video/src/still.rs` | M1 | Survey / R1 |
| 2 | Input Device Selection & Probing | Audio/video input device enumeration, selection, and probing | M1 | Survey / R1 |
| 3 | Track Arming & State Control | Arming individual tracks for recording with visual state and timeline commands | M1 | Survey / R1 |
| 4 | Recording with Low-Latency Monitoring | Simultaneous audio recording to 32-bit float WAV with real-time input ring buffer monitoring | M1 | Survey / R1 |
| 5 | Trademark String Sanitization | Replace all 9 third-party vendor trademarks with generic interchange terms | M1 | Survey / R3 |
| 6 | Transition Categories & Presets | 6 transition categories with parameterized presets and thumbnail rendering | M2 | Survey / R2.1 |
| 7 | 3D Video Transitions | Spatial 3D transitions (Fly In, Page Peel, Page Roll, Cube, Shuffle) with CPU fallback | M2 | Survey / R2.1 |
| 8 | Transition Progress Envelope | Event automation envelope for non-linear transition progress curves | M2 | Survey / R2.1 |
| 9 | GPU Video Effects & Fallbacks | WGPU video effect pipeline with verified software reference fallbacks | M2 | Survey / R2.1 |
| 10 | OFX 1.4 Plugin Host & Sample | Isolated OpenFX plugin host with mock plugin validation and fallback execution | M2 | Survey / R2.1 |
| 11 | Per-Event Color Grading | Dedicated color grading pipeline per event, track, and output | M3 | Survey / R2.2 |
| 12 | Primary 4-Way Color Wheels | Lift, Gamma, Gain, Offset wheels with Wheel/Slider toggle and luma adjustment | M3 | Survey / R2.2 |
| 13 | White Balance Neutral Picker | Eyedropper neutral tone sampler computing temperature and tint correction | M3 | Survey / R2.2 |
| 14 | RGB & HSL Transfer Curves | Interactive spline transfer curves (Master RGB, R, G, B) and 6 HSL curves | M3 | Survey / R2.2 |
| 15 | OpenColorIO (OCIO) / ACES Pipeline | Standard ACES 1.0..1.2 spaces, IDTs (S-Log1/2/3), working spaces, and display ODTs | M3 | Survey / R2.2, R2.10 |
| 16 | Color Match Across Clips | Reference-to-target color balance and luminance transfer | M3 | Survey / R2.10 |
| 17 | Point & Planar Motion Tracking | Optical flow feature tracking generating keyframed coordinate trajectories | M4 | Survey / R2.3 |
| 18 | Geometric & Bézier Event Masks | Parametric (Rect, Oval) and Bézier spline event masks with feathering and inversion | M4 | Survey / R2.3 |
| 19 | Smart Mask AI Object Segmentation | AI-assisted object segmentation generating animated alpha mattes with CPU fallback | M4 | Survey / R2.3 |
| 20 | Text-Based Timeline Editing | Synchronized spoken transcript view linked to timeline event trimming | M4 | Survey / R2.7 |
| 21 | Speech-to-Text Subtitle Generation | Audio speech transcription generating subtitle caption tracks with formatting constraints | M4 | Survey / R2.7 |
| 22 | Smart Upscale & Super-Resolution | High-quality resolution upscaling with bilinear/bicubic software fallbacks | M4 | Survey / R2.7 |
| 23 | AI Auto Reframe | Dynamic pan/crop keyframing maintaining primary subject centered across aspect ratios | M4 | Survey / R2.7 |
| 24 | Project Properties Setup | Video/audio project settings matching Dialog 826/863 | M5 | Survey / R2.4 |
| 25 | Project Templates (.vqpt) | 46 standard project templates across broadcast, web, and disc formats | M5 | Survey / R2.4 |
| 26 | Media Profiles Rule Matcher | Reverse-order rule matching of imported media stream attributes (`profiles.ini`) | M5 | Survey / R2.4 |
| 27 | Project Media Detail Columns | 23 detail metadata columns for media pool list/details view | M5 | Survey / R2.5 |
| 28 | Media Bins & Smart Search Bins | Hierarchical bin folders, quick tags, and dynamic query search bins | M5 | Survey / R2.5 |
| 29 | Unified File Explorer | Drive navigation, favorites, media preview player, and scene detection | M5 | Survey / R2.5 |
| 30 | Make Movie Guided Export Wizard | Novice-friendly export wizard flow for web, camera, and local disk targets | M5 | Survey / R2.6 |
| 31 | Color-Coded Preview Cache Ruler | Timeline ruler reflecting frame cache status (Green = RAM/Disk cached, Yellow = Playable, Red = Unrendered) | M5 | Survey / R2.6 |
| 32 | Dynamic RAM & Selective Prerender | Memory buffer cache (Shift+B) and disk prerender cache (Shift+M) | M5 | Survey / R2.6 |
| 33 | Standardized Alerts Subsystem | Centralized alert subsystem matching §10 error codes and actions | M5 | Survey / R2.8 |
| 34 | Smart Proxies & Quality Switching | Automatic proxy usage on Draft/Preview and original media on Good/Best; original conformed for final render | M5 | Survey / R2.9 |
| 35 | L-Cut & J-Cut Split Editing | Independent trimming of audio or video edges without breaking timeline link | M6 | Survey / R2.11 |
| 36 | Simultaneous Audio/Video Fades | Synchronized fade handle manipulation with audio micro-quickfades | M6 | Survey / R2.11 |
| 37 | Persistent Sync Links | Bi-directional event synchronization with out-of-sync indicators and resyncing | M6 | Survey / R2.11 |
| 38 | Multicamera Audio/Timecode Sync | Multi-angle camera alignment via audio waveform correlation or timecode | M6 | Survey / R2.12 |
| 39 | Multicamera Track & Live Angle Switching | Container track with multi-angle preview grid and real-time click-to-cut | M6 | Survey / R2.12 |
| 40 | Alternative Event Takes System | Multiple media takes within a single event container with cycling and slip-all | M6 | Survey / R2.12 |
| 41 | Parity Matrix & README Synchronization | Update `scripts/parity_matrix.py` classifications and sync `README.md` | M6 | Survey / R3 |
| 42 | Translation Catalogs Synchronization | 100% key synchronization across all 7 translation files in `Assets/i18n/strings/` | M6 | Survey / R3 |
| 43 | 100% E2E Test Suite Validation (Tiers 1-4) | Comprehensive opaque-box test execution across all features | M7 | Dual Track Final |
| 44 | Adversarial Coverage Hardening (Tier 5) | White-box stress testing, gap analysis, and boundary hardening | M7 | Dual Track Final |

---

## Milestones

| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M1 | Recording Stabilization & Trademark Sanitization | Features 1–5: Automated recording tests, worktree verification, and trademark removal | none | DONE |
| M2 | Video Transitions, 3D Transforms, GPU Effects & OFX Host | Features 6–10: Transition categories, presets, 3D transforms, GPU effects, and OFX | M1 | DONE |
| M3 | Color Grading, Color Wheels, OCIO/ACES & Color Match | Features 11–16: 4-way wheels, curves, white balance, OCIO/ACES pipeline, color match | M1 | IN_PROGRESS |
| M4 | Motion Tracking, Event Masks, Smart Mask & AI Features | Features 17–23: Motion tracking, geometric/Bézier masks, Smart Mask, text editing, speech-to-text, upscale, auto reframe | M1 | PLANNED |
| M5 | Project Properties, Media Pool, Explorer, Render Caching & Alerts | Features 24–34: Project properties, templates, media profiles, 23 media columns, bins, explorer, Make Movie, cache ruler, alerts, smart proxies | M1 | PLANNED |
| M6 | Advanced Timeline Editing, Multicam & Takes | Features 35–42: L/J cuts, simultaneous fades, sync links, multicam, takes, i18n catalogs, and parity matrix | M2, M3, M4, M5 | PLANNED |
| M7 | E2E Test Suite Pass (Tiers 1-4) & Adversarial Hardening (Tier 5) | Features 43–44: Pass 100% E2E test suite published in `TEST_READY.md`, followed by Tier 5 adversarial stress testing | M6, TEST_READY.md | PLANNED |

---

## Interface Contracts

### neoveg_audio ↔ neoveg_engine (Recording & Monitoring)
- `neoveg_audio::record::Recorder`:
  - `pub fn start_recording(&mut self, options: RecordOptions) -> Result<(), RecordError>`
  - `pub fn stop_recording(&mut self) -> Result<PathBuf, RecordError>`
  - `pub fn monitor_status(&self) -> MonitorStatus`
  - `pub fn create_mock_loopback() -> Self` (for headless automated testing)
- `neoveg_engine::track_arm::Track`:
  - `pub fn set_armed(&mut self, armed: bool)`
  - `pub fn is_armed(&self) -> bool`

### neoveg_video ↔ neoveg_engine (Transitions, Effects & Grading)
- `neoveg_video::transitions`:
  - `pub trait TransitionRenderer: Send + Sync { fn render(&self, source: &FrameBuffer, target: &FrameBuffer, progress: f32, params: &BTreeMap<String, f32>) -> Result<FrameBuffer, String>; }`
  - `pub fn get_preset(id: &str) -> Option<TransitionPreset>`
- `neoveg_video::grading`:
  - `pub struct ColorWheelsGrade { pub lift: ColorWheelVector, pub gamma: ColorWheelVector, pub gain: ColorWheelVector, pub offset: ColorWheelVector }`
  - `pub trait ColorMatcher: Send + Sync { fn analyze_and_match(&self, reference: &FrameBuffer, target: &FrameBuffer) -> ColorWheelsGrade; }`

### neoveg_engine ↔ neoveg_common (Alerts, Project & Profiles)
- `neoveg_common::alerts`:
  - `pub struct StandardAlert { pub code: AlertCode, pub category: AlertCategory, pub en_us_template: &'static str, pub pt_br_template: &'static str }`
- `neoveg_common::profiles`:
  - `pub struct MediaProfileMatcher { ... }`
  - `pub fn match_stream(&self, key: &MediaProfileKey) -> Option<MediaProfileAttributes>`

### neoveg_engine ↔ neoveg_ui (Timeline, Takes & Multicam)
- `neoveg_engine::sync_link::SyncLink`:
  - `pub fn calculate_out_of_sync(&self, current_offset_sec: f64) -> f64`
- `neoveg_engine::takes::TakeContainer`:
  - `pub fn next_take(&mut self); pub fn prev_take(&mut self); pub fn slip_all_takes(&mut self, delta_sec: f64);`
- `neoveg_engine::multicam::MulticamSynchronizer`:
  - `pub fn sync_by_audio_correlation(&self, tracks: &[(u64, &[f32])], sample_rate: u32) -> Result<Vec<(u64, f64)>, String>;`

---

## Code Layout
- `crates/neoveg_common/`:
  - `src/alerts.rs`: Standardized alerts subsystem matching §10 error codes.
  - `src/profiles.rs`: Media profile rule matcher (`profiles.ini`).
  - `src/project.rs`: Project properties and templates (.vqpt).
  - `src/i18n.rs`: Locale translations management.
- `crates/neoveg_video/`:
  - `src/transitions.rs`: Transition categories, presets, 3D transforms, CPU fallbacks.
  - `src/grading.rs`: 4-way color wheels, curves, white balance picker, OCIO/ACES pipeline.
  - `src/tracking.rs`: Optical flow motion tracking and event masks.
  - `src/render_cache.rs`: Dynamic RAM / prerender cache and color-coded ruler.
- `crates/neoveg_audio/`:
  - `src/record.rs`: Input device selection, arming, monitoring, mock loopback.
  - `src/whisper.rs`: Speech-to-text transcription engine.
- `crates/neoveg_engine/`:
  - `src/sync_link.rs`: Persistent sync links and L/J cut split editing.
  - `src/multicam.rs`: Multicamera track container, angle switching, audio/timecode sync.
  - `src/takes.rs`: Alternative event takes system.
  - `src/media_pool.rs`: 23 detail metadata columns, media bins, smart search bins.
  - `src/ai.rs`: Text-based editing, auto reframe, upscale orchestration.
- `crates/neoveg_ui/`:
  - `src/app.rs`: Application shell, commands, sanitized trademark strings.
  - `src/dialogs/`: Standardized dialogs, Make Movie wizard, Project Properties.
  - `src/dock/`: Media pool view, Explorer view, Color Grading panel.
- `Assets/i18n/strings/`:
  - `de-DE.json`, `en-US.json`, `es-ES.json`, `fr-FR.json`, `ja-JP.json`, `ru-RU.json`, `zh-CN.json`.
- `tests/`:
  - Workspace E2E opaque-box test suites (Tiers 1–4).
