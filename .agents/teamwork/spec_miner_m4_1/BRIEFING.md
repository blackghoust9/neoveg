# BRIEFING — 2026-11-06T15:25:30Z

## Mission
Mine precise VEGAS Pro and technical specifications for Milestone 4 (Features 17–23) across manuals, results.md, and codebase, maintaining clean-room trademark compliance and pure-CPU fallbacks.

## 🔒 My Identity
- Archetype: Specification Miner
- Roles: Milestone 4 Spec Miner (`spec_miner_m4_1`)
- Working directory: C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\spec_miner_m4_1\
- Original parent: c220c328-9551-48e1-a35b-23e05589d782
- Milestone: Milestone 4 (Features 17-23)

## 🔒 Key Constraints
- Pure clean-room Rust implementation compliant with `MIT OR Apache-2.0` (`deny.toml`), without GPL/LGPL dependencies.
- Zero third-party trademarks in runtime strings or identifiers.
- Strict 8 GB RAM execution budget (sequential cargo `-j 1`, `--test-threads=1`, never run `cargo test -p neoveg_ui`).
- Pure software/CPU fallbacks and mock implementations must be provided for GPU effects, OFX, and AI features so all tests and builds execute within CI/local environments without requiring dedicated GPU hardware or live external AI endpoints.
- Strictly obey VEGAS Pro semantics across the entire program (especially GUI behavior and keyboard shortcuts), consulting reference manuals in `manuais/`. Document any non-standard or beneficial behavior notes.
- Do NOT implement anything — read-only specification mining.

## Current Parent
- Conversation ID: c220c328-9551-48e1-a35b-23e05589d782
- Updated: 2026-11-06T15:15:47Z

## Task Summary
- **What to mine**: Specifications for Features 17-23:
  1. Feature 17: Motion Tracking (Point, planar, optical flow LK/pyramidal, modes, regions, keyframes, search windows, import/export)
  2. Feature 18: Geometric & Bézier Event Masks (Rectangle, Oval/Ellipse, Bézier spline curves with control points, feathering in/out/both, opacity, inversion, blend modes)
  3. Feature 19: Smart Mask (AI-assisted object segmentation, bounding boxes, alpha matte generation, confidence thresholds, pure-CPU fallback)
  4. Feature 20: Text-Based Timeline Editing (Word-level sync, transcript selection to cut/trim, search-in-transcript, delete-word ripples)
  5. Feature 21: Speech-to-Text Subtitle Generation (Whisper transcription, timecode-aligned subtitle events SRT/WebVTT, formatting constraints CPL/CPS)
  6. Feature 22: Smart Upscale & Super-Resolution (Resolution scaling 2x, 4x, sharpness/denoise, CPU fallbacks bicubic/edge-directed)
  7. Feature 23: AI Auto Reframe (Dynamic pan/crop keyframe generation tracking primary subject across aspect ratios 16:9, 9:16, 1:1, 4:5)
- **Status**: Completed. Report delivered in `handoff.md`.

## Key Decisions Made
- Fully specified pure-CPU mathematical models: Lucas-Kanade optical flow, GrabCut/GMM segmentation, NEDI/EDDI edge-directed upscaling, and deadband camera auto reframing.
- Documented full VEGAS Pro parity including keyboard shortcuts (`Alt + M` Motion Tracking, `Ctrl + L` Auto Ripple, `Alt + G` Color Grading, `32662` / `R` AI Auto Reframe).
- Provided complete trademark mapping table for generic clean-room runtime naming.

## Artifact Index
- `DISPATCH.md` — Dispatch prompt
- `progress.md` — Liveness & progress tracking
- `handoff.md` — Final spec mining report
