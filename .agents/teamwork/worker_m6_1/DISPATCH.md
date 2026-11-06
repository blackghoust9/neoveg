# Task Assignment: worker_m6_1

## Objective
Implementation of Milestone 6 (Features 35–42: Timeline Editing, Multicam, Takes, VEGAS Pro Parity & i18n Sync).


## 2026-11-06T18:56:46Z
You are worker_m6_1, Milestone 6 Implementation Worker for Neoveg.
Your working directory is: C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\worker_m6_1\
Your parent orchestrator conversation ID is: c220c328-9551-48e1-a35b-23e05589d782

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

MANDATORY INPUTS (read all before implementing):
- C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\ORIGINAL_REQUEST.md
- C:\Users\paulomatos\Documents\GitHub\neoveg\PROJECT.md
- C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\spec_miner_m6_1\handoff.md
- C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\explorer_m6_2\handoff.md
- C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\explorer_m6_3\handoff.md

TASK:
Implement all Milestone 6 features (Features 35–42: Timeline Editing, Multicam & Takes):

1. Feature 35 — L-Cut & J-Cut Split Editing:
   - Independent trimming of audio or video edges without breaking timeline link.
   - Support "Ignore Event Grouping" toggle (`Ctrl+Shift+U`) and Shift-drag single-edge trimming.
   - Single-track ripple editing in "Affected Tracks" mode.

2. Feature 36 — Simultaneous Audio/Video Fades:
   - When dragging fade handle on a grouped or sync-linked event, propagate fade length simultaneously to partner events sharing the edge unless grouping is ignored.
   - Audio micro-quickfades: apply default 10 ms rapid linear/smooth fade to cut edges of split audio events in `ops.rs` to eliminate transients/clicks.

3. Feature 37 — Persistent Sync Links (`crates/neoveg_engine/src/sync_link.rs`):
   - Create `SyncLink` model: `master_event`, `linked_event`, `base_offset_sec`, `calculate_out_of_sync(&self, current_offset_sec: f64) -> f64`.
   - `SyncLinkManager` or timeline integration: track one-way sync links, query out-of-sync drift.
   - Implement resynchronization operations: `resync_by_moving` (adjusts event start) and `resync_by_slipping` (adjusts event trim_in).

4. Feature 38 — Multicamera Audio/Timecode Sync (`crates/neoveg_engine/src/multicam.rs`):
   - Implement `MulticamSynchronizer`:
     - `sync_by_audio_correlation(&self, tracks: &[(u64, &[f32])], sample_rate: u32) -> Result<Vec<(u64, f64)>, String>`:
       Use the two-stage hierarchical envelope cross-correlation (200 Hz coarse + 2 kHz fine + parabolic sub-sample refinement) designed in `explorer_m6_3`, operating in < 100 ms with < 15 MB RAM, achieving < 0.1 ms precision.
     - `sync_by_timecode`: alignment based on SMPTE timecode track metadata.

5. Feature 39 — Multicamera Track & Live Angle Switching (`crates/neoveg_engine/src/multicam.rs` & `neoveg_ui`):
   - Container track model with `create_multicam_track`, take generation per camera, and empty takes `(no camera)`.
   - Live switching / cutting: cut-to-take (splits event at playhead and switches angle) and switch-angle (switches active take without split), cut-to-overlap with crossfades.
   - UI hooks: multicam preview grid (2x2, 3x3) in `preview_ui`, angle badges, and keyboard shortcuts `Key::Num1..Num9` and `Ctrl + Num1..Num9`.

6. Feature 40 — Alternative Event Takes System (`crates/neoveg_engine/src/takes.rs`):
   - Implement `TakeContainer` struct wrapping takes for an event:
     `pub fn next_take(&mut self); pub fn prev_take(&mut self); pub fn slip_all_takes(&mut self, delta_sec: f64);`
   - `slip_all_takes`: adjusts `trim_offset` of all takes in the container simultaneously by `delta_sec`.
   - Shortcuts 'T' (next take) and 'Shift+T' (previous take).

7. Feature 41 & 42 — Parity Matrix, Checkboxes & i18n Synchronization:
   - Export `pub mod sync_link;` and `pub mod multicam;` in `crates/neoveg_engine/src/lib.rs`.
   - Add the ~25 new localization keys identified by `explorer_m6_3` across all 7 catalogs in `Assets/i18n/strings/` (`de-DE`, `en-US`, `es-ES`, `fr-FR`, `ja-JP`, `ru-RU`, `zh-CN`), maintaining 100% key parity and 0 missing keys.
   - Update `scripts/parity_matrix.py` classifications for `edit.takes`, `fer.multicam`, `opc.slip_takes`, `exi.evento` to `"feito"` and verify `python scripts/parity_matrix.py --check` exits with 0.
   - Audit and check off completed Milestone 6 checkboxes in `README.md` (lines 493, 494, 547, 622, 651, 659) and lingering completed M1-M5 checkboxes (lines 636, 656, 663, 686).

8. Integration Test Suite & Verification:
   - Create `crates/neoveg_engine/tests/m6_features_35_42.rs` verifying all features.
   - Run:
     `cargo check --workspace -j 1`
     `cargo clippy --workspace -j 1`
     `cargo test -p neoveg_common -j 1 -- --test-threads=1`
     `cargo test -p neoveg_engine -j 1 -- --test-threads=1`
     `cargo test -p neoveg_engine --test m6_features_35_42 -j 1 -- --test-threads=1`
     `python scripts/parity_matrix.py --check`
   - NEVER execute `cargo test -p neoveg_ui`.


## 2026-11-06T19:40:22Z
**Context**: Milestone 6 Wrap-up Deadline (17:00 local / 20:00Z)
**Content**: 
1. Use `;` instead of `&&` in PowerShell: `git add crates/neoveg_engine/src/multicam.rs crates/neoveg_ui/src/app.rs crates/neoveg_engine/tests/m6_features_35_42.rs; git commit -m "feat(m6): integration tests and clippy fixes"`
2. Verify that `README.md` lines 493, 494, 547, 622, 651, 659 and lingering 636, 656, 663, 686 are `- [x]` if completed.
3. Write your handoff report to `C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\worker_m6_1\handoff.md` immediately with the test results (`m6_features_35_42.rs` 7/7 passed, clippy 0 warnings, parity matrix 0 unclassified).
4. Send completion message back so we can push to `preview/version-2.1.0` before 16:58.
**Action**: Commit changes, write handoff.md, and reply immediately.
