# E2E Test Infra: Neoveg NLE Feature Extensions

## Test Philosophy
- **Opaque-Box & Requirement-Driven**: Tests are derived strictly from `ORIGINAL_REQUEST.md` and `results.md` specifications, exercising the public interfaces and integration points as an end user / external system would.
- **Headless & Hardware-Agnostic**: All tests execute on CPU in headless CI environments without requiring physical audio hardware, dedicated GPUs, or remote AI servers.
- **Methodology**: Systematic 4-tier approach (Category-Partition, Boundary Value Analysis, Pairwise Combinatorial, and Real-World Workload Testing).

## Feature Inventory
| # | Feature | Source | Tier 1 | Tier 2 | Tier 3 |
|---|---------|--------|:------:|:------:|:------:|
| 1 | Worktree Verification | R1 | 5 | 5 | ✓ |
| 2 | Input Device Selection | R1 | 5 | 5 | ✓ |
| 3 | Track Arming | R1 | 5 | 5 | ✓ |
| 4 | Recording with Monitoring | R1 | 5 | 5 | ✓ |
| 5 | Trademark Cleanliness | R3 | 5 | 5 | ✓ |
| 6 | Transition Categories & Presets | R2.1 | 5 | 5 | ✓ |
| 7 | 3D Transitions | R2.1 | 5 | 5 | ✓ |
| 8 | Transition Envelope | R2.1 | 5 | 5 | ✓ |
| 9 | GPU Video Effects & Fallbacks | R2.1 | 5 | 5 | ✓ |
| 10 | OFX 1.4 Plugin Host | R2.1 | 5 | 5 | ✓ |
| 11 | Per-Event Color Grading | R2.2 | 5 | 5 | ✓ |
| 12 | Color Wheels (Lift/Gamma/Gain/Offset) | R2.2 | 5 | 5 | ✓ |
| 13 | White Balance Picker | R2.2 | 5 | 5 | ✓ |
| 14 | RGB & HSL Curves | R2.2 | 5 | 5 | ✓ |
| 15 | OpenColorIO / ACES Workflow | R2.2, R2.10 | 5 | 5 | ✓ |
| 16 | Color Match Across Clips | R2.10 | 5 | 5 | ✓ |
| 17 | Motion Tracking | R2.3 | 5 | 5 | ✓ |
| 18 | Geometric & Bézier Masks | R2.3 | 5 | 5 | ✓ |
| 19 | Smart Mask AI Segmentation | R2.3 | 5 | 5 | ✓ |
| 20 | Text-Based Editing | R2.7 | 5 | 5 | ✓ |
| 21 | Speech-to-Text Subtitles | R2.7 | 5 | 5 | ✓ |
| 22 | Smart Upscale | R2.7 | 5 | 5 | ✓ |
| 23 | AI Auto Reframe | R2.7 | 5 | 5 | ✓ |
| 24 | Project Properties | R2.4 | 5 | 5 | ✓ |
| 25 | Project Templates | R2.4 | 5 | 5 | ✓ |
| 26 | Media Profiles Matcher | R2.4 | 5 | 5 | ✓ |
| 27 | 23 Media Detail Columns | R2.5 | 5 | 5 | ✓ |
| 28 | Media Bins & Smart Bins | R2.5 | 5 | 5 | ✓ |
| 29 | Explorer Window | R2.5 | 5 | 5 | ✓ |
| 30 | Make Movie Wizard | R2.6 | 5 | 5 | ✓ |
| 31 | Preview Cache Ruler (Green/Yellow/Red) | R2.6 | 5 | 5 | ✓ |
| 32 | Prerender & RAM Preview Cache | R2.6 | 5 | 5 | ✓ |
| 33 | Standardized Alerts (§10) | R2.8 | 5 | 5 | ✓ |
| 34 | Smart Proxies & Quality Switching | R2.9 | 5 | 5 | ✓ |
| 35 | L-Cut & J-Cut Editing | R2.11 | 5 | 5 | ✓ |
| 36 | Simultaneous Audio/Video Fades | R2.11 | 5 | 5 | ✓ |
| 37 | Persistent Sync Links | R2.11 | 5 | 5 | ✓ |
| 38 | Multicam Waveform & Timecode Sync | R2.12 | 5 | 5 | ✓ |
| 39 | Multicamera Track & Live Switching | R2.12 | 5 | 5 | ✓ |
| 40 | Alternative Event Takes | R2.12 | 5 | 5 | ✓ |
| 41 | Parity Matrix Sync | R3 | 5 | 5 | ✓ |
| 42 | Translation Catalogs Sync | R3 | 5 | 5 | ✓ |

## Test Architecture
- **Location**: `tests/` integration test suites in workspace crates (`neoveg_common`, `neoveg_video`, `neoveg_audio`, `neoveg_engine`).
- **Invocation**: `cargo test -j 1 --test-threads=1 --workspace --exclude neoveg_ui`
- **Pass/Fail Semantics**: 0 failures, 0 panics, zero unhandled errors.
- **Mock/Fixture Strategy**:
  - In-memory mock audio recorder loopback (`Recorder::create_mock_loopback()`).
  - Synthetic in-memory Whisper model (`WhisperEngine::from_components`).
  - Pure-CPU reference renderer for all GPU effects and 3D transitions.
  - In-memory OFX sample host (`neoveg_ofx_sample`).

## Real-World Application Scenarios (Tier 4)
| # | Scenario | Features Exercised | Complexity |
|---|----------|--------------------|------------|
| 1 | Multi-Track Recording & Podcasting Session | Input device selection, track arming, low-latency monitor buffer, takes cycling, L/J cut split editing | High |
| 2 | Multicam Concert Event Edit | 4-camera waveform synchronization, multicamera container track creation, live angle switching, takes management, simultaneous crossfades | High |
| 3 | Commercial Color Grading & Delivery Pipeline | S-Log3 / ACEScg color pipeline, 4-way color wheels, white balance neutral picker, color match across clips, Make Movie export | High |
| 4 | AI Subtitling & Social Video Reframe | Dialogue transcription with Whisper, subtitle generation with character limits, 16:9 to 9:16 AI Auto Reframe, Smart Upscale | High |
| 5 | Complex VFX & Motion Masking with Prerender Caching | Geometric and Bézier event masks, optical flow motion tracking, 3D Page Peel transition, RAM / disk prerender cache ruler validation | High |
| 6 | Smart Proxy Offline/Online Workflow & Alert Handling | High-res media ingestion, proxy quality switching (Draft/Good), missing media alert triggering, relink resolution, original-source conforming render | High |

## Coverage Thresholds
- Tier 1: ≥5 per feature (Total: 42 × 5 = 210 test cases)
- Tier 2: ≥5 per feature boundary (Total: 42 × 5 = 210 test cases)
- Tier 3: Pairwise combinations across major feature boundaries (≥42 test cases)
- Tier 4: 6 realistic application scenarios (≥6 test cases)
- **Total Minimum Target: ~468 test cases**
