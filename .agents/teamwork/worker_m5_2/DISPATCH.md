# Task Assignment: worker_m5_2

## Objective
Remediation of Milestone 5 (Features 24–34) per reviewer_m5_1 findings, user Blender open-movie directive, and README.md checklist synchronization.


## 2026-11-06T17:58:41Z
You are worker_m5_2, Milestone 5 Remediation & Synchronization Worker for Neoveg.
Your working directory is: C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\worker_m5_2\
Your parent orchestrator conversation ID is: c220c328-9551-48e1-a35b-23e05589d782

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

MANDATORY INPUTS (read all before writing code):
- C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\ORIGINAL_REQUEST.md
- C:\Users\paulomatos\Documents\GitHub\neoveg\PROJECT.md
- C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\reviewer_m5_1\handoff.md
- C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\challenger_m5_1\handoff.md
- C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\challenger_m5_2\handoff.md
- C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\worker_m5_1\handoff.md

TASK DESCRIPTION & REMEDIATION DELIVERABLES:
1. Critical Fix — Genuine Scene Detection in Explorer (`crates/neoveg_engine/src/explorer.rs:203–217`):
   - Replace the hardcoded `vec![(0.0, 10.0)]` dummy stub in `ExplorerModel::detect_scenes` with genuine logic.
   - If the media file does not exist, return `Err(...)`.
   - Probe media properties (duration, dimensions, fps) from container/file. If media is unreadable or duration is <= 0.0, return Err.
   - Genuine cut boundary analysis: sample video frames or packet timestamps at regular intervals across media duration. Compute luminance or frame difference metrics between consecutive samples. If the difference exceeds `threshold`, record a cut point timestamp.
   - If no transitions exceed `threshold` (or threshold is high), return a single continuous span `(0.0, duration)` bounded strictly by the actual media duration (never exceeding file duration, e.g. a 2.0s file must yield `(0.0, 2.0)`, not 10.0s).
   - Add comprehensive tests in `crates/neoveg_engine/tests/m5_features_24_34.rs` for `detect_scenes` covering: short clips, custom thresholds, cut point generation, duration bounding, and error handling.

2. Major Fix — Case-Insensitive Companion File Suppression (`crates/neoveg_engine/src/explorer.rs:125–131`):
   - Change `navigate_to` companion file filtering to case-insensitive:
     `let name_lower = file_name.to_lowercase(); if name_lower.ends_with(".sfk") || name_lower.ends_with(".sfvp0") || name_lower.ends_with(".nvprv") { continue; }`
   - Verify uppercase extensions like `.SFK`, `.SFVP0`, `.NVPRV` are properly suppressed. Add test in `m5_features_24_34.rs`.

3. Minor Fix — Frame Rate Boundary at 1.0 FPS (`crates/neoveg_common/src/project.rs:362–366`):
   - Change `self.fps > 1.0` to `self.fps >= 1.0` in `ProjectSettings::sanitize` so legitimate 1.0 FPS projects are preserved and not reset to 29.97. Add test.

4. User Directive — Blender Open Movie Test Fixture Integration:
   - Provide helper or integration in tests for real-world video/audio validation using Blender Open Movies (e.g. Big Buck Bunny / Sintel) cached in a gitignored/temp test cache (such as `target/test_fixtures/blender/`) or via lightweight synthetic sample frames, ensuring tests run offline without bloating git repository commits/pushes.

5. User Directive — README.md Checkbox Audit & Synchronization:
   - Audit `README.md` lines corresponding to completed Milestone 5 features:
     - Lines 479, 480, 481, 484, 488, 587, 601, 602, 604, 605, 607, 608 (and any others completed in M5).
     - Update all completed roadmap checkboxes from `- [ ]` to `- [x]`.
     - Run `python scripts/parity_matrix.py --check` and verify it exits with 0.

6. Validation Commands (run sequentially with -j 1 and --test-threads=1):
   - `cargo check --workspace -j 1`
   - `cargo clippy --workspace -j 1`
   - `cargo test -p neoveg_common -j 1 -- --test-threads=1`
   - `cargo test -p neoveg_engine -j 1 -- --test-threads=1`
   - `cargo test -p neoveg_common --test adversarial_m5_properties_profiles -j 1 -- --test-threads=1`
   - `cargo test -p neoveg_engine --test adversarial_m5_caching_proxies -j 1 -- --test-threads=1`
   - NEVER execute `cargo test -p neoveg_ui`.

Deliver full handoff report in `handoff.md` and notify parent orchestrator via `send_message`.
