# Handoff Report — Milestone 6: Advanced Timeline Editing, Multicam & Takes (Features 35–42)

- **Date**: 2026-11-06T18:55:00Z
- **Author**: `explorer_m6_2` (Architecture Explorer)
- **Target Recipient**: Milestone 6 Implementer Worker & Orchestrator (`c220c328-9551-48e1-a35b-23e05589d782`)
- **Status**: Comprehensive Analysis & Implementation Blueprint Complete

---

## 1. Observation

### 1.1 Contract Specifications (`PROJECT.md` & `ORIGINAL_REQUEST.md`)
- `PROJECT.md` (lines 68–75):
  - **Feature 35**: L-Cut & J-Cut Split Editing (independent trimming of audio or video edges without breaking timeline link).
  - **Feature 36**: Simultaneous Audio/Video Fades (synchronized fade handle manipulation with audio micro-quickfades).
  - **Feature 37**: Persistent Sync Links (bi-directional / one-way event synchronization with out-of-sync indicators and resyncing).
  - **Feature 38**: Multicamera Audio/Timecode Sync (multi-angle camera alignment via audio waveform correlation or timecode).
  - **Feature 39**: Multicamera Track & Live Angle Switching (container track with multi-angle preview grid and real-time click-to-cut).
  - **Feature 40**: Alternative Event Takes System (multiple media takes within a single event container with cycling and slip-all).
  - **Feature 41**: Parity Matrix & README Synchronization (`scripts/parity_matrix.py --check` and `README.md`).
  - **Feature 42**: Translation Catalogs Synchronization (100% key parity across all 7 catalogs in `Assets/i18n/strings/`).
- `PROJECT.md` (lines 122–129) specifies exact interface contracts:
  ```rust
  // neoveg_engine::sync_link::SyncLink
  pub fn calculate_out_of_sync(&self, current_offset_sec: f64) -> f64;

  // neoveg_engine::takes::TakeContainer
  pub fn next_take(&mut self);
  pub fn prev_take(&mut self);
  pub fn slip_all_takes(&mut self, delta_sec: f64);

  // neoveg_engine::multicam::MulticamSynchronizer
  pub fn sync_by_audio_correlation(
      &self,
      tracks: &[(u64, &[f32])],
      sample_rate: u32,
  ) -> Result<Vec<(u64, f64)>, String>;
  ```

### 1.2 Current Engine State (`crates/neoveg_engine/src/`)
- `sync_link.rs`: **Does NOT exist**. `crates/neoveg_engine/src/lib.rs` does not declare `pub mod sync_link;`.
- `multicam.rs`: **Does NOT exist**. `crates/neoveg_engine/src/lib.rs` does not declare `pub mod multicam;`.
- `takes.rs` (lines 1–360):
  - Contains procedural functions: `count`, `active_index`, `take_name`, `add`, `step`, `set_active`, `delete_active`.
  - **Missing**: `struct TakeContainer` and explicit `next_take`, `prev_take`, `slip_all_takes` methods required by contract.
- `timeline.rs`:
  - `pub struct Take` (lines 139–150): holds `media_id: Option<u64>`, `trim_offset: f64`, `name: String`.
  - `pub struct TrackEvent` (lines 153–289): holds `takes: Vec<Take>`, `active_take: usize`, `group: Option<GroupId>`. It lacks fields or associations for `SyncLink`.
- `groups.rs` (lines 51–117):
  - `trim_group_edge` trims all group members sharing an edge.
- `crossfade.rs` (lines 10–47):
  - `find_crossfades(events: &[TrackEvent]) -> Vec<Crossfade>` finds single-track overlapping crossfades.
- `ops.rs` (lines 86–150, 168–192):
  - `split_list` splits events at cursor and resets new cut-edge fades to 0.0 (`out.fade_in = 0.0; out.fade_out = 0.0;`). There is no automatic micro-quickfade (10 ms) applied to split audio cuts.

### 1.3 Current Audio Synchronization DSP (`crates/neoveg_audio/src/sync.rs`)
- Lines 1–124: Fully implemented and tested:
  ```rust
  pub fn find_offset(
      reference: &[f32],
      reference_rate: u32,
      other: &[f32],
      other_rate: u32,
  ) -> Option<Sync>;
  ```
  Uses `ENVELOPE_RATE = 200.0`, windowed loudness envelope comparison, normalized cross-correlation, and parabolic peak interpolation.

### 1.4 Current UI State (`crates/neoveg_ui/src/`)
- `app.rs`:
  - Lines 5322–5329: `DragMode::FadeIn` and `DragMode::FadeOut` only modify the single dragged event (`drag.current = edit::set_fade_in(drag.origin, length)`). There is **no simultaneous fade** propagation across grouped or sync-linked audio/video partner events.
  - Lines 5415–5423: `trim_group_edge` trims grouped members, but `Shift` or `ignore_grouping` bypasses it, allowing single-edge trimming (L/J cuts).
  - Lines 16834–16840: Event header renders `[take {}/{}]` text in event title.
  - Line 17141: Event borders are drawn white when selected and gray(15) otherwise. **No purple outline** for sync-linked events, and **no out-of-sync badge** (e.g. `+0.12s`) rendered on drifted events.
  - Lines 13859–13950: `preview_ui` only displays the single video texture; **no multicam grid preview** (2x2, 3x3) exists.
  - Keyboard shortcuts:
    - `'T'` and `'Shift+T'` trigger `Command::EditNextTake` and `Command::EditPreviousTake` via `step_take(1)` and `step_take(-1)`.
    - `'Ctrl+Shift+U'` triggers `Command::EditIgnoreGrouping` via `toggle_ignore_grouping()`.
    - `Key::Num1..Num9` is currently wired in keyboard trim mode (`nudge_trim_edge`) and layout recall (`Alt+D` chord), but **no handlers exist for choosing multicam takes (1–9 or Ctrl+1–9)**.

### 1.5 Reference Manuals Semantics (`manuais/`)
- VEGAS Pro Manuals (`manual_vegaspro21_en.md`, `5828511.md`, `VP12Edit_ReviewersGuide.md`):
  - **Sync Links**: One-way event links. An event stays synchronized with its master event, but not vice-versa. Moving the master moves linked events; moving linked events does not move the master. Outlined in purple when the master is selected. Out-of-sync offset shown as badge. Right-click options: "Synchronize > By Moving", "Synchronize > By Slipping".
  - **Simultaneous Fades**: Dragging a fade handle on a grouped or linked event adjusts fades on all group members simultaneously unless grouping is ignored.
  - **Micro-quickfades**: Default 10 ms rapid fade on cut edges of audio events to eliminate harsh transients/clicks.
  - **Multicamera Editing**:
    - `Create Multicamera Track`: Synchronizes selected camera tracks and merges them into a single track with takes per camera.
    - `Enable Multicamera Editing` (`Ctrl+Shift+D`): Switches Video Preview to a tiled grid showing all angles, with the active take highlighted by a colored border and camera name.
    - `Angle Switching / Cut-to-Take`: Clicking a grid tile or pressing `1..9` switches active take (when paused) or cuts to that take (during playback). `Ctrl + 1..9` creates a crossfade cut.

---

## 2. Logic Chain

1. **Missing Engine Modules**:
   - Because `crates/neoveg_engine/src/sync_link.rs` and `multicam.rs` do not exist, all code referencing `neoveg_engine::sync_link::SyncLink` and `neoveg_engine::multicam::MulticamSynchronizer` cannot compile until these modules are created and exported in `lib.rs`.
2. **Takes Model Gap**:
   - `PROJECT.md` specifies `TakeContainer` with `next_take()`, `prev_take()`, and `slip_all_takes(delta_sec: f64)`. While `takes.rs` has procedural `step`, having a dedicated struct `TakeContainer` ensures encapsulation of takes logic, direct slip-all offset calculations, and clean UI/E2E test interfacing.
3. **Sync Links & Out-of-Sync Calculation**:
   - In VEGAS semantics, when audio and video clips created from the same source media drift in time, `out_of_sync = (video_start - audio_start) - (video_trim_in - audio_trim_in)`.
   - `SyncLink::calculate_out_of_sync(&self, current_offset_sec: f64) -> f64` compares `current_offset_sec` against the established `base_offset_sec`.
   - Restoring sync requires two standard operations:
     a. `Resync by Moving`: adjust event timeline `start` to re-align with master.
     b. `Resync by Slipping`: adjust event source `trim_in` to re-align media playback without changing event placement.
4. **Multicamera Architecture**:
   - `MulticamSynchronizer` needs audio correlation sync across multiple audio streams. Since `neoveg_engine` does not directly depend on `neoveg_audio` (audio is in dev-dependencies), `MulticamSynchronizer` can implement envelope-based cross-correlation in pure Rust math (identical algorithm to `neoveg_audio::sync`) or import it.
   - Merging tracks into a multicam track creates an event series where each event has takes for cameras 1..N.
   - Live switching during playback splits the event at the playhead and switches the new event to the chosen angle.
5. **Simultaneous Fades & Quickfades**:
   - When a fade-in or fade-out is dragged, if the event is grouped or sync-linked and `!ignore_grouping`, all partner events sharing the edge should receive the updated fade length.
   - For audio events, splits in `ops.rs` should support applying default 10 ms quickfades when enabled.
6. **UI Integration**:
   - Event rendering in `app.rs` must:
     a. Query sync link status and draw purple outline for linked events when master is selected.
     b. Query out-of-sync drift and draw a badge (e.g. `+0.12s` or `-0.05s`) in contrasting red/amber on drifted events.
     c. Render a multicam angle grid in `preview_ui` when `multicam_mode` is enabled.
     d. Intercept keys `1..9` (and `Ctrl+1..9`) to trigger angle switching/cutting.
7. **Verification**:
   - A dedicated integration test suite `crates/neoveg_engine/tests/m6_features_35_42.rs` must verify Features 35–42 end-to-end, and all 7 translation files must be synchronized.

---

## 3. Caveats

1. **RAM Budget & Sequential Execution**: All Cargo runs must use `-j 1` and `-- --test-threads=1`.
2. **No UI Test Execution**: `neoveg_ui` must NEVER be tested with `cargo test -p neoveg_ui`; it must be verified strictly with `cargo check -j 1` and `cargo clippy -j 1`.
3. **No External Dependencies / GPL**: All new algorithms (cross-correlation, multicam track generation, sync link math) must remain clean-room Rust without GPL/LGPL dependencies.
4. **Zero Trademark Strings**: UI strings must strictly avoid vendor trademarks (use generic terms: "Multicâmera", "Vínculo de sincronização", "Takes alternativos", "Sincronizar por áudio").

---

## 4. Conclusion & Technical Implementation Blueprint

### 4.1 New File: `crates/neoveg_engine/src/sync_link.rs`
Define:
```rust
use crate::command::{EditCommand, TrackChange};
use crate::selection::EventRef;
use crate::timeline::{EventId, Timeline, TrackEvent, TrackId, TIME_EPSILON};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncLink {
    pub master_track: TrackId,
    pub master_event: EventId,
    pub dependent_track: TrackId,
    pub dependent_event: EventId,
    /// Established baseline offset: (dependent_start - master_start) - (dependent_trim_in - master_trim_in)
    pub base_offset_sec: f64,
}

impl SyncLink {
    pub fn new(
        master_track: TrackId,
        master_event: EventId,
        dependent_track: TrackId,
        dependent_event: EventId,
        base_offset_sec: f64,
    ) -> Self {
        Self {
            master_track,
            master_event,
            dependent_track,
            dependent_event,
            base_offset_sec,
        }
    }

    /// Calculates out-of-sync drift given current timeline offset.
    pub fn calculate_out_of_sync(&self, current_offset_sec: f64) -> f64 {
        current_offset_sec - self.base_offset_sec
    }
}

/// Computes out-of-sync drift between two events created from the same source media.
pub fn calculate_media_out_of_sync(master: &TrackEvent, dependent: &TrackEvent) -> Option<f64> {
    if master.media_id.is_none() || master.media_id != dependent.media_id {
        return None;
    }
    let media_offset = dependent.trim_in - master.trim_in;
    let timeline_offset = dependent.start - master.start;
    Some(timeline_offset - media_offset)
}

// Operational functions:
pub fn create_sync_link(
    timeline: &Timeline,
    master: EventRef,
    dependents: &[EventRef],
) -> Option<Vec<SyncLink>>;

pub fn resync_by_moving(
    timeline: &Timeline,
    master: EventRef,
    dependent: EventRef,
) -> Option<EditCommand>;

pub fn resync_by_slipping(
    timeline: &Timeline,
    master: EventRef,
    dependent: EventRef,
) -> Option<EditCommand>;
```

### 4.2 New File: `crates/neoveg_engine/src/multicam.rs`
Define:
```rust
use crate::command::EditCommand;
use crate::selection::EventRef;
use crate::timeline::{EventId, Timeline, TrackEvent, TrackId, TrackKind};

pub struct MulticamSynchronizer;

impl MulticamSynchronizer {
    pub fn new() -> Self {
        Self
    }

    /// Multi-angle camera alignment via audio waveform correlation.
    pub fn sync_by_audio_correlation(
        &self,
        tracks: &[(u64, &[f32])],
        sample_rate: u32,
    ) -> Result<Vec<(u64, f64)>, String> {
        if tracks.is_empty() {
            return Ok(Vec::new());
        }
        let ref_id = tracks[0].0;
        let ref_samples = tracks[0].1;
        let mut results = vec![(ref_id, 0.0)];

        for &(track_id, samples) in &tracks[1..] {
            let offset = correlate_audio_lags(ref_samples, samples, sample_rate)?;
            results.push((track_id, offset));
        }
        Ok(results)
    }

    /// Multi-angle camera alignment via SMPTE timecode.
    pub fn sync_by_timecode(
        &self,
        clips: &[(u64, f64)], // (id, timecode_start_sec)
    ) -> Vec<(u64, f64)> {
        if clips.is_empty() {
            return Vec::new();
        }
        let min_tc = clips.iter().map(|c| c.1).fold(f64::INFINITY, f64::min);
        clips.iter().map(|(id, tc)| (*id, tc - min_tc)).collect()
    }
}

/// Creates a multicamera track from multiple synchronized camera tracks.
pub fn create_multicamera_track(
    timeline: &mut Timeline,
    camera_track_ids: &[TrackId],
) -> Option<TrackId>;

/// Switches the active camera take of an event, optionally with a crossfade.
pub fn switch_camera_angle(
    timeline: &Timeline,
    event: EventRef,
    angle_index: usize,
    crossfade: bool,
) -> Option<EditCommand>;

/// Splits event at playhead and switches incoming section to new angle.
pub fn cut_and_switch_angle(
    timeline: &mut Timeline,
    track_id: TrackId,
    event_id: EventId,
    split_time: f64,
    new_angle: usize,
    crossfade: bool,
) -> Option<EditCommand>;
```

### 4.3 Update: `crates/neoveg_engine/src/takes.rs`
Add `TakeContainer`:
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct TakeContainer {
    pub takes: Vec<Take>,
    pub active_take: usize,
    pub trim_in: f64,
    pub media_id: Option<u64>,
}

impl TakeContainer {
    pub fn new(takes: Vec<Take>, active_take: usize, trim_in: f64, media_id: Option<u64>) -> Self {
        Self {
            takes,
            active_take,
            trim_in,
            media_id,
        }
    }

    pub fn from_event(ev: &TrackEvent) -> Self {
        Self {
            takes: ev.takes.clone(),
            active_take: active_index(ev),
            trim_in: ev.trim_in,
            media_id: ev.media_id,
        }
    }

    pub fn next_take(&mut self) {
        if self.takes.len() >= 2 {
            self.active_take = (self.active_take + 1) % self.takes.len();
            self.media_id = self.takes[self.active_take].media_id;
        }
    }

    pub fn prev_take(&mut self) {
        if self.takes.len() >= 2 {
            self.active_take = (self.active_take + self.takes.len() - 1) % self.takes.len();
            self.media_id = self.takes[self.active_take].media_id;
        }
    }

    pub fn slip_all_takes(&mut self, delta_sec: f64) {
        self.trim_in = (self.trim_in + delta_sec).max(0.0);
        // Takes offsets remain relative to active take trim_in
    }
}
```

### 4.4 Update: `crates/neoveg_engine/src/ops.rs` & `preferences.rs`
- In `crates/neoveg_common/src/preferences.rs`:
  Add `quickfade_audio_edits: bool` (default: `true`) and `quickfade_duration_ms: f64` (default: `10.0`).
- In `crates/neoveg_engine/src/ops.rs`:
  In `split_list`, when an audio event is split and `quickfade_audio_edits` is true, apply `0.010` s fade-out to the left half and `0.010` s fade-in to the right half.

### 4.5 Update: `crates/neoveg_ui/src/app.rs`
- **Simultaneous Fades**:
  In `update_drag` for `DragMode::FadeIn` / `DragMode::FadeOut`:
  When dragging a fade handle on an event with grouped or linked partner events and `!ignore_grouping`, update fade values on all partner events sharing the edge simultaneously.
- **Out-of-Sync Badge**:
  In timeline event rendering loop (around line 16840):
  Compare paired audio/video events (same media or sync link). If `drift.abs() > TIME_EPSILON`, render an out-of-sync badge (e.g. `[+0.12s]` in red/amber `Color32::from_rgb(0xE7, 0x4C, 0x3C)`) and right-click context menu options for "Sincronizar > Mover" and "Sincronizar > Deslizar".
- **Purple Outline for Sync Links**:
  When master event is selected, outline linked events in purple (`Color32::from_rgb(0x9B, 0x59, 0xB6)`).
- **Multicam Grid in Video Preview**:
  In `preview_ui`, when `state.multicam_mode` is enabled (`Ctrl+Shift+D`), lay out a 2x2 or 3x3 grid of angle frames. Draw active camera with highlight border and name. Clicking an angle switches the camera.
- **Key Handlers for 1..9**:
  In input handling, when `state.multicam_mode` is active, intercept digits 1..9 (`Key::Num1..Num9`) to switch/cut to that camera take (with crossfade if Ctrl is held).

### 4.6 New Integration Test Suite: `crates/neoveg_engine/tests/m6_features_35_42.rs`
Create a comprehensive test file covering all 8 milestone features with both happy path and adversarial edge cases:
- `test_f35_l_cut_j_cut_split_editing_and_slip()`
- `test_f36_simultaneous_fades_and_audio_micro_quickfades()`
- `test_f37_sync_link_drift_calculation_and_resync_by_moving_and_slipping()`
- `test_f38_multicam_audio_correlation_and_timecode_sync()`
- `test_f39_multicam_track_creation_and_live_switching()`
- `test_f40_take_container_cycling_and_slip_all_takes()`
- `test_f41_parity_matrix_and_readme_synchronization()`
- `test_f42_i18n_catalog_synchronization_all_seven_languages()`

---

## 5. Verification Method

To verify the implementation independently, execute the following commands in sequence (never exceeding the 8 GB / single-threaded budget):

1. **Workspace Compilation**:
   ```powershell
   cargo check -j 1
   ```
2. **Clippy Cleanliness**:
   ```powershell
   cargo clippy -j 1
   ```
3. **Milestone 6 Integration Tests**:
   ```powershell
   cargo test -p neoveg_engine --test m6_features_35_42 -j 1 -- --test-threads=1
   ```
4. **All Engine Tests (Regression Check)**:
   ```powershell
   cargo test -p neoveg_engine -j 1 -- --test-threads=1
   ```
5. **Common & i18n Tests**:
   ```powershell
   cargo test -p neoveg_common -j 1 -- --test-threads=1
   ```
6. **Parity Matrix Check**:
   ```powershell
   python scripts/parity_matrix.py --check
   ```
7. **Invalidation Conditions**:
   - Any compiler error or clippy warning.
   - Any test failure in `m6_features_35_42` or regressions in existing tests.
   - Missing or misaligned keys in any of the 7 JSON files in `Assets/i18n/strings/`.
   - Any third-party trademark present in runtime strings.
