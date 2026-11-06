# Specification Mining Report — Milestone 6: Timeline Editing, Multicam & Takes

## 1. Observation

Authoritative specification mining was performed across the reference documentation and reverse-engineering records:
- `manuais/markdown/manual_vegaspro21_en.md` (VEGAS Pro 21 User Manual)
- `manuais/markdown/VEGAS_Pro_18_EN_Keyboard_shortcuts.md` (VEGAS Pro 18 Keyboard Shortcuts)
- `results.md` (VEGAS Pro 21 binary analysis, dialog resources, preferences, and menu command catalogs)
- `PROJECT.md` & `ORIGINAL_REQUEST.md` (Project specifications, Milestone 6 roadmap, clean-room rules)
- `crates/neoveg_common/src/fade.rs`, `crates/neoveg_engine/src/takes.rs`, `crates/neoveg_engine/src/groups.rs`, `crates/neoveg_engine/src/timeline.rs`

### Direct Citations from Authoritative Sources:

1. **L-Cut / J-Cut Split Editing & Ignore Event Grouping**:
   - `manual_vegaspro21_en.md:13299`:
     > "Hold Shift while dragging the event edge to ignore event grouping for fast J and L cuts."
   - `manual_vegaspro21_en.md:13415`:
     > "Hold Ctrl+Alt+Shift while dragging the event edge between two events to ignore event grouping for fast J and L cuts. If you are using the Slide tool, hold Shift to ignore event grouping."
   - `manual_vegaspro21_en.md:14355-14359`:
     > "Select the IGNORE EVENT GROUPING button (or press Ctrl+Shift+U) to override event groups without removing the groups."
   - `VEGAS_Pro_18_EN_Keyboard_shortcuts.md:265-275`:
     - `Shift + Numeric Keypad 1/3`: "L/J trim left/right by frames (ignores event grouping so you can trim audio and video events independently)."
     - `Shift + Numeric Keypad 4/6`: "L/J trim left/right by pixels."
     - `Ctrl+Alt+Shift + Numeric Keypad 1/3`: "L/J cut left/right by frames (trim adjacent while ignoring event grouping)."
     - `Ctrl+Alt+Shift + Numeric Keypad 4/6`: "L/J cut left/right by pixels."
     - `Ctrl+Alt + Numeric Keypad 1/3`: "Slide a transition or crossfade one frame left/right (trim adjacent)."
     - `Alt + drag inside event`: "Slip: move media within event without moving the event."
     - `Alt + drag edge of event`: "Slip trim: moves the media with the edge as it is trimmed."
   - `manual_vegaspro21_en.md:12547-12566`:
     - Three post-edit ripple modes: "Affected Tracks" (`F`), "Affected Tracks, Bus Tracks, Markers, and Regions" (`Ctrl+F`), and "All Tracks, Markers, and Regions" (`Ctrl+Shift+F`). Auto Ripple toggle is `Ctrl+L`.
     - In "Affected Tracks" mode, trimming only the video track in an L/J cut ripples downstream video events only, leaving audio intact.

2. **Simultaneous Audio/Video Fades, Micro-Quickfades & Curve Functions**:
   - `manual_vegaspro21_en.md:13867, 13917`:
     > "When you have multiple events selected, the fade of all selected events is adjusted simultaneously."
     - When grouped audio and video events are edited and `Ignore Event Grouping` is OFF, dragging a fade handle adjusts both audio and video fades simultaneously.
   - `manual_vegaspro21_en.md:48005`:
     > "A rapid fade is applied to the edges of audio events to soften potentially harsh transitions that can result from cutting data or splitting events. Enter a value in the box to change the length of the fade. From the Options menu, choose QUICKFADE AUDIO EDITS to turn quick fades on or off."
   - `results.md:6078, 17160, 18805`:
     - Dialog Label: `&Quick fade length for audio events (ms):`
     - Command: `Options.QuickfadeAudioEdits` (id `32315` / `0x7E3B`, menu 1003).
     - Preference key: `TPT_ntQuickfadeAudioSize` (default: 10 ms = 0.010 s) and `TPT_fQuickfadeAudio` (default: true).
   - `crates/neoveg_common/src/fade.rs:33-42` & `manual_vegaspro21_en.md:44681`:
     - 5 curve types: `Linear`, `Fast`, `Slow`, `Smooth`, `Sharp`.
     - Mathematical transfer functions $g(x)$ for normalized progress $x \in [0, 1]$:
       - Linear: $g(x) = x$
       - Fast: $g(x) = 1.0 - (1.0 - x)^2$
       - Slow: $g(x) = x^2$
       - Smooth: $g(x) = x^2 (3.0 - 2.0x)$ (cubic Hermite smoothstep)
       - Sharp: $g(x) = 0.5 - \sin\left(\frac{\arcsin(1.0 - 2.0x)}{3.0}\right)$ (inverse smoothstep)

3. **Persistent Sync Links & Out-of-Sync Indicators**:
   - `manual_vegaspro21_en.md:13757-13805`:
     > "Repairing audio/video synchronization offsets: When audio and video events (or events from multichannel audio files) are not aligned, VEGAS Pro will highlight the events in the timeline so you can see synchronization problems at a glance."
     > "The amount of offset is displayed when ACTIVE TAKE INFORMATION is selected from the View menu (Ctrl+Shift+I). Very small offsets that are below the resolution of the timeline may be displayed as 0.00. Set the project time format to samples to see the offset amount."
     > "To restore synchronization, right-click the audio or video event you want to synchronize, choose SYNCHRONIZE from the shortcut menu, and then choose a command from the submenu:
     > - By Moving: Moves the event you clicked so it is synchronized to its corresponding audio or video event.
     > - By Slipping: Slips the contents of the event you clicked so the audio and video are synchronized. The events do not move, but the contents of the event you right-click are shifted forward or backward to restore synchronization."
   - `manual_vegaspro21_en.md:14393-14400`:
     > "Using sync links: Sync links are like one-way event groups. A linked event stays synchronized with the event to which it is linked, but not the other way around. Linked events do not contribute to the affected tracks for ripple editing. Sync links are useful for keeping additional audio mix and video compositing elements in sync with the main events in your project. Linked events are displayed shorter and vertically aligned with the synchronized event. When you select an event that has linked events, the linked events are outlined in purple."
     - Context commands: `Create Sync Link with Selected Events`, `Remove Sync Link from Selected Events`, `Sync Link ▸ Unlink`, `Sync Link ▸ Select All`.

4. **Multicamera Audio & Timecode Sync**:
   - `manual_vegaspro21_en.md:17255-17265, 17290-17300`:
     - Timecode synchronization: `Tools ▸ Multicamera ▸ Lay Out Tracks Using Media Timecode` (`0x81B5`). Aligns tracks by comparing embedded SMPTE timecode track (`tmcd` in MP4/MOV, MXF timecode, BWF `bext`).
     - Timestamp synchronization: `Tools ▸ Multicamera ▸ Lay Out Tracks Using Media Date/Time Stamp` (`0x81B6`). Aligns tracks using file recording timestamp.
     - Audio synchronization: `Tools ▸ Multicamera ▸ Synchronize Audio to Align Events` (`0x8319`).
       > "In this process the whole audio data is analyzed to find similarities so this can take a while with longer recordings."
       > "The events below the topmost one are moved forward or backward so that they are synchronized."
   - `results.md:271, 368`: Native library `MuCaAudioSync_x64.dll`.
     - Standard waveform cross-correlation algorithm: downsample audio to uniform sample rate (e.g. 11025 Hz or 16000 Hz mono), compute normalized cross-correlation $R_{xy}[k]$, find peak lag $k^*$, calculate offset $\Delta t = k^* / f_s$. Minimum peak correlation threshold $\rho \ge 0.20$.
     - Delay tolerance: sound travels $\approx 343\text{ m/s}$ (approx. 1 frame for every 11.5 meters per manual p. 418).

5. **Multicamera Track & Live Angle Switching**:
   - `manual_vegaspro21_en.md:17335-17345`:
     > "Tools ▸ Multicamera ▸ Create Multicamera Track: VEGAS Pro builds a set of tracks containing a series of events, with takes representing each of the cameras. A new event is created for each time a camera was started and stopped, as well as empty '(no camera)' takes for video events where cameras were missing."
   - `manual_vegaspro21_en.md:17380-17400, 17440-17480`:
     - Toggle Multicam Editing: `Tools ▸ Multicamera ▸ Enable Multicamera Editing` (`Ctrl+Shift+D`).
     - Split View: Video Preview window displays tiled multi-camera view (2x2 for up to 4 angles, 3x3 for up to 9 angles, 4x4 for up to 16 angles). Each cell shows angle number and take name. Active angle is highlighted with a colored border (user-configured color).
     - Angle selection shortcuts: Keys `1` .. `9` (or Numeric Keypad `1` .. `9`) or mouse click on camera cell.
     - Cut-to-angle mode: during playback, pressing `1`..`9` or clicking a take splits the event at the playhead position and assigns the clicked take as active for the new event.
     - Cut-to-overlap (crossfade): holding `Ctrl` while choosing a take (`Ctrl + 1`..`9`) creates a crossfade transition with the duration specified in `Cut-to-overlap conversion` preferences.
     - Switch-angle mode: when paused or recutting a selected event, choosing a take changes the active take without splitting.
     - Empty takes `(no camera)` cannot be selected.
     - Expand: `Tools ▸ Multicamera ▸ Expand to Multiple Tracks` (`0x827E`) unpacks the container track back into component tracks, prompting whether to retain unused takes as muted events or delete them.

6. **Alternative Event Takes System**:
   - `manual_vegaspro21_en.md:14179-14240`:
     - Takes associate multiple media files with a single timeline event container. The event retains its timeline position, start, and duration; the active take determines what media is played.
     - Shortcuts:
       - `T`: Next take (`Edit ▸ Take ▸ Next Take`, `0x7E6B`)
       - `Shift+T`: Previous take (`Edit ▸ Take ▸ Previous Take`, `0x7E6C`)
     - Take Chooser: `Edit ▸ Take ▸ Choose Active...` (`0x7E6A`)
     - Adding takes: right-click and drag media file(s) onto timeline event -> `Add as Takes`.
     - Deleting: `Edit ▸ Take ▸ Delete Active` (`0x7E47`), `Edit ▸ Take ▸ Delete...` (`0x7E69`).
     - Slip all takes: `Options ▸ Slip All Takes`. When enabled, slipping the event media adjusts the `trim_in` / `trim_offset` of all takes in the event container simultaneously by `delta_sec`, preserving relative alignment between takes.

---

## 2. Logic Chain

1. **Split Editing (L/J Cuts) vs Event Grouping**:
   - Audio and video events generated from a single container media file are grouped together by default (`GroupId`).
   - Standard trimming (`trim_group_edge`) locks both audio and video edges together to preserve synchronization.
   - To make an L-cut (audio continues under next scene) or J-cut (audio precedes next scene video), either `Ignore Event Grouping` (`Ctrl+Shift+U`) must be toggled on, or the user must hold `Shift` during mouse drag, or press `Shift + Numpad 1/3` (frames) / `Shift + Numpad 4/6` (pixels).
   - This single-event trim changes the event's `trim_in` or `duration` independently of its grouped counterpart.
   - Ripple semantics: in "Affected Tracks" mode (`F`), ripple occurs only on the track being trimmed; the unedited track is untouched, preserving downstream alignment on that track.

2. **Persistent Sync Link Offset Calculation & Correction**:
   - When an event is trimmed independently, its timeline start $T_{start}$ and source in-point $T_{trim\_in}$ diverge relative to its paired event.
   - For a paired audio/video event pair $(A, V)$:
     $$\Delta_{original} = (T_{start, A} - T_{start, V}) - (T_{trim\_in, A} - T_{trim\_in, V})$$
     In initial sync, $\Delta_{original} = 0$.
     After an independent edit:
     $$\Delta_{current} = (T_{start, A} - T_{start, V}) - (T_{trim\_in, A} - T_{trim\_in, V})$$
     $$\Delta_{out\_of\_sync} = \Delta_{current} - \Delta_{original}$$
   - When $\Delta_{out\_of\_sync} \ne 0$:
     - A visual badge/header (red border/badge) highlights the event.
     - View ▸ Active Take Information (`Ctrl+Shift+I`) displays $\Delta_{out\_of\_sync}$ formatted in timecode or samples.
   - The user can restore synchronization via context menu `Synchronize`:
     - `By Moving`: adjusts the event's timeline start: $T_{start, clicked} \leftarrow T_{start, clicked} - \Delta_{out\_of\_sync}$.
     - `By Slipping`: adjusts the event's source media in-point: $T_{trim\_in, clicked} \leftarrow T_{trim\_in, clicked} + \Delta_{out\_of\_sync}$, without altering its timeline position or duration.

3. **Multicamera Container Track & Switching Architecture**:
   - Rather than creating a separate proprietary event format, VEGAS Pro implements multicamera editing directly on top of the Takes system:
     - Component tracks are aligned (via Timecode, Date/Time Stamp, or Audio Cross-Correlation).
     - `Create Multicamera Track` collapses the synchronized tracks into a single container track where each event has $N$ takes corresponding to the $N$ camera angles.
     - Gaps in camera coverage are populated with empty take instances (`Take { media_id: None, name: "(no camera)" }`).
     - Toggling `Enable Multicamera Editing` (`Ctrl+Shift+D`) switches the video preview renderer into a split grid (2x2, 3x3, 4x4) where each tile renders a specific take.
     - During playback, pressing `1`..`9` or clicking tile $K$ splits the container event at the playhead position ($t_{split}$):
       - Event before $t_{split}$: retains previously active take.
       - Event after $t_{split}$: sets active take to $K$.
       - If `Ctrl` is held, a crossfade of duration $T_{overlap}$ (from preferences) is created at the split seam.

4. **Audio Waveform Cross-Correlation Math & Tolerances**:
   - Let reference audio be $x[n]$ and target camera audio be $y[n]$.
   - Both are downsampled to a mono analysis rate (e.g. $f_s = 16000\text{ Hz}$).
   - Cross-correlation is computed:
     $$R_{xy}[k] = \sum_{n} x[n] y[n + k]$$
   - Normalized cross-correlation coefficient:
     $$\rho[k] = \frac{R_{xy}[k]}{\sqrt{\sum x[n]^2 \cdot \sum y[n+k]^2}}$$
   - Peak lag $k^* = \arg\max_k |\rho[k]|$.
   - Alignment time offset: $\Delta t = \frac{k^*}{f_s}$.
   - Acceptance threshold: $\rho[k^*] \ge 0.20$. If $\rho[k^*] < 0.20$, the alignment fails or is rejected as ambiguous.

---

## 3. Caveats

- Hardware controller integration (e.g. Mackie Control, ShuttlePRO) supports multicam switching via dedicated surface buttons, but the pure-software core interface relies on keyboard shortcuts (`1`..`9`, `Ctrl+Shift+D`, `T`, `Shift+T`) and UI mouse clicks.
- Timecode matching relies on media files having valid embedded SMPTE timecode (e.g. `tmcd` track or BWF `bext` chunk); if missing or corrupt, fallback to audio waveform cross-correlation or date/time stamp is required.
- Third-party trademarks ("Sony", "MAGIX", "VEGAS Pro") must never appear in runtime strings; generic terms ("Multicamera Track", "Take Container", "Split Edit", "Waveform Alignment") are used throughout the clean-room implementation.

---

## 4. Conclusion & Feature Tables

### Features Discovered Table
| # | Category | Feature | Description | Inputs | Outputs | Error Behavior | Discovered Via |
|---|----------|---------|-------------|--------|---------|----------------|----------------|
| 35.1 | Split Edit | L-Cut & J-Cut Mouse Drag | Independent edge trimming of audio or video while ignoring event grouping | Mouse drag on event edge with `Shift` held | Event start/end updated on one track; grouped counterpart unchanged | Clamped to media bounds unless Loop switch active | Manual p. 324 |
| 35.2 | Split Edit | Ignore Event Grouping Toggle | Global mode toggle allowing single-event manipulation across timeline | Shortcut `Ctrl+Shift+U` or toolbar button | Toggles global state `ignore_event_grouping` | None (idempotent boolean toggle) | Manual p. 351, results.md |
| 35.3 | Split Edit | Numpad Frame/Pixel L/J Trim | Edge trimming by frame or pixel ignoring event grouping | `Shift + Numpad 1/3` (frame), `Shift + Numpad 4/6` (pixel) | Adjusts active event edge by 1 frame or 1 pixel | Beeps/clamps at media source limits | Shortcuts manual p. 8 |
| 35.4 | Split Edit | Adjacent L/J Cut (Rolling) | Trims adjacent cut edge ignoring grouping | `Ctrl+Alt+Shift` + drag edge or Numpad 1/3 | Moves edit seam between two events on one track | Clamped if either event hits minimum duration | Shortcuts manual p. 8 |
| 35.5 | Split Edit | Post-Edit Ripple on Split Cuts | Ripple downstream events after single-track edit | Auto Ripple mode (`Ctrl+L`): Affected Tracks (`F`) vs All Tracks (`Ctrl+Shift+F`) | Shifts downstream events on affected track or all tracks | If unaligned, out-of-sync badge appears downstream | Manual p. 305 |
| 36.1 | Fades | Simultaneous Group Fade Drag | Dragging fade handle adjusts all grouped/selected event fades | Drag fade handle on grouped event with grouping ON | Both audio and video fades update to identical duration | Clamped to half of shortest event duration | Manual p. 336 |
| 36.2 | Fades | Micro-Quickfades on Audio Cuts | Automatic anti-pop/click ramp applied to split/trimmed audio edges | Event split (`S`) or cut edit; Preference `Options.QuickfadeAudioEdits` | 10 ms fade-in and fade-out applied at cut boundary | Clamped if event duration < 20 ms | Manual p. 1026, results.md |
| 36.3 | Fades | 5 Fade Curve Transfer Functions | Mathematical transfer curves for fade-in/fade-out/crossfade | Progress $x \in [0, 1]$, Curve type (Linear, Fast, Slow, Smooth, Sharp) | Gain multiplier $g(x) \in [0, 1]$ | Clamped to $[0.0, 1.0]$ | `neoveg_common::fade`, Manual p. 1026 |
| 37.1 | Sync Links | Persistent Sync Link Creation | One-way hierarchical linking of slave events to a master event | Right-click master -> `Create Sync Link with Selected Events` | Stores master-slave relationship; slave moves with master | Self-links or circular links rejected | Manual p. 351 |
| 37.2 | Sync Links | Out-of-Sync Delta Calculation | Computes timing offset between paired audio/video events | Paired events $(A, V)$ start times and source in-points | Signed time delta $\Delta_{sync}$ in seconds or samples | Returns $0.0$ if within epsilon ($10^{-6}\text{ s}$) | Manual p. 334 |
| 37.3 | Sync Links | Visual Out-of-Sync Badge | Timeline indicator displaying sync warning and delta | `View ▸ Active Take Information` (`Ctrl+Shift+I`) active & $\Delta_{sync} \ne 0$ | Red border/header badge displaying signed offset (e.g. `+0.12s`) | Hidden when delta is zero | Manual p. 335, Shortcuts manual p. 2 |
| 37.4 | Sync Links | Synchronize by Moving | Context menu action restoring alignment by moving event start | Right-click event -> `Synchronize ▸ By Moving` | Moves clicked event start time by $-\Delta_{sync}$ | Clamped to timeline zero ($t \ge 0$) | Manual p. 335 |
| 37.5 | Sync Links | Synchronize by Slipping | Context menu action restoring alignment by slipping media | Right-click event -> `Synchronize ▸ By Slipping` | Adjusts `trim_in` by $\Delta_{sync}$; start & duration stay fixed | Clamped to $[0, \text{source\_duration} - \text{duration}]$ | Manual p. 335 |
| 37.6 | Sync Links | Add Missing Stream | Recreates missing paired audio or video stream from same media | Right-click event -> `Add Missing Stream for Selected Event` | Inserts new track with matching event in sync | Fails if source file has no alternate stream | Manual p. 336 |
| 38.1 | Multicam Sync | Media Timecode Alignment | Aligns tracks using embedded SMPTE timecode | Selected media tracks with timecode headers | Track layout with events aligned to common timecode base | Fails if media lacks valid timecode | Manual p. 418, results.md |
| 38.2 | Multicam Sync | Date/Time Stamp Alignment | Coarse track alignment using filesystem / EXIF recording dates | Selected media tracks with timestamps | Track layout aligned to earliest recorded timestamp | Fallback if timecode absent | Manual p. 418 |
| 38.3 | Multicam Sync | Audio Cross-Correlation Alignment | Sub-frame event alignment via audio waveform similarity | Audio streams `&[f32]`, sample rate $f_s$ | Relative time offsets $\Delta t_i$ for each track | Returns error if correlation peak $\rho < 0.20$ | Manual p. 418, results.md |
| 39.1 | Multicam Track | Create Multicamera Track | Combines synchronized parallel camera tracks into one container track | Selected tracks containing synchronized camera events | Single track with multi-take events and `(no camera)` takes | Fails if fewer than 2 tracks selected | Manual p. 420 |
| 39.2 | Multicam Track | Split Grid Preview Mode | Multi-angle preview rendering in Video Preview window | Multicam editing enabled (`Ctrl+Shift+D`) | 2x2, 3x3, or 4x4 tiled view showing all angles | Degrades gracefully on low-spec CPU | Manual p. 421 |
| 39.3 | Multicam Track | Live Click-to-Cut Switching | Real-time camera switching during timeline playback | Mouse click on angle cell or keys `1`..`9` / Numpad `1`..`9` | Splits event at playhead; sets new active take on right event | Ignored if clicked take is `(no camera)` | Manual p. 424 |
| 39.4 | Multicam Track | Cut-to-Overlap (Crossfade Cut) | Switch camera angle with crossfade dissolve transition | `Ctrl + 1`..`9` or `Ctrl` + mouse click during playback | Creates crossfade transition between angles at seam | Reverts to hard cut if event duration < overlap | Manual p. 424 |
| 39.5 | Multicam Track | Expand to Multiple Tracks | Unpacks multicamera container track back into component tracks | Tools ▸ Multicamera ▸ Expand to Multiple Tracks | Multiple separate tracks restored per camera | Prompts user to keep or discard unused takes | Manual p. 425 |
| 40.1 | Takes | Event Take Container | Stores alternative media takes within a single timeline event | Event container with list of `Take` structs | Active take media and offset loaded into event | Validates media ID exists | Manual p. 346, `takes.rs` |
| 40.2 | Takes | Cycle Next/Previous Take | Cycles active take forward or backward | Key `T` (next take) or `Shift+T` (previous take) | Updates `active_take`, re-bases relative `trim_offset` | Wraps around modulo total takes; no-op if 1 take | Manual p. 348 |
| 40.3 | Takes | Choose Active Take | Explicitly selects active take from list | Take Chooser dialog or context menu `Take ▸ Choose Active` | Sets chosen take as active | Fails if take index out of range | Manual p. 347 |
| 40.4 | Takes | Slip All Takes | Slips source media offsets of all takes simultaneously | Slip tool (Alt+drag) with `Options.SlipAllTakes` ON | Shifts `trim_in` of active take and offsets of all takes | Inactive takes preserve relative offsets | Manual p. 332, 347 |
| 40.5 | Takes | Delete Active Take | Removes current take and activates adjacent take | Edit ▸ Take ▸ Delete Active | Removes take; selects next (or previous if last) | No-op if event has only 1 take | Manual p. 348 |

---

### Edge Cases Table
| # | Feature | Input | Observed Behavior |
|---|---------|-------|-------------------|
| 1 | L/J Cut Trim | Shift+drag audio edge past media end without Loop switch | Clamps at media duration; audio silence drawn, video unaffected |
| 2 | L/J Cut Trim | Shift+drag video edge past media end with Loop switch ON | Video loops media from trim_in point; audio remains non-looping |
| 3 | L/J Cut Ripple | Auto Ripple on "Affected Tracks" mode while trimming L-cut video | Downstream video events ripple by trim delta; downstream audio remains fixed |
| 4 | L/J Cut Ripple | Auto Ripple on "All Tracks" mode while trimming L-cut video | All downstream tracks (audio + video + markers) ripple together |
| 5 | Micro-Quickfade | Event split on an audio event of duration 12 ms (< 20 ms) | Quickfades clamp so $fade\_in + fade\_out \le 12\text{ ms}$ (max 6 ms each) |
| 6 | Fade Drag | Dragging fade handle when Ignore Event Grouping is ON (`Ctrl+Shift+U`) | Only the dragged event's fade changes; grouped counterpart fade is unchanged |
| 7 | Fade Curve | Sharp curve evaluated at midpoint $x = 0.5$ | Output is exactly $0.5$; tangents are steep at $0$ and $1$, flat in middle |
| 8 | Sync Link | Dragging child linked event in a one-way Sync Link | Child moves freely without moving parent; purple outline indicates link |
| 9 | Sync Link | Dragging parent event in a one-way Sync Link | Parent moves and all child linked events translate by identical timeline delta |
| 10 | Sync Link | Ripple edit triggered on child linked track | Child linked events do not contribute to affected tracks for ripple |
| 11 | Synchronize | Synchronize By Slipping when out-of-sync delta exceeds source headroom | Clamps to source start (0.0) or source end; residual offset remains |
| 12 | Multicam Sync | Audio correlation on silent or pure white-noise tracks ($\rho < 0.20$) | Synchronizer returns `Err("insufficient audio correlation")`; tracks remain unshifted |
| 13 | Multicam Sync | Multi-rate audio sync (e.g. 44.1 kHz vs 48 kHz vs 96 kHz) | Resamples both inputs to uniform 16 kHz mono before computing cross-correlation |
| 14 | Multicam Track | Camera started late or stopped early during recording | Multicam track inserts `(no camera)` take with `media_id: None` for missing intervals |
| 15 | Multicam Cut | User presses key `3` during live playback when angle 3 is `(no camera)` | Switch command is rejected / no-op; playback continues uninterrupted |
| 16 | Multicam Cut | User holds `Ctrl` while pressing `2` during playback with duration < overlap | Falls back to cut without overlap if event duration cannot accommodate crossfade |
| 17 | Takes | Event with 1 take executes `Delete Active Take` | Command is rejected; an event must never lose its sole remaining take |
| 18 | Takes | "Slip all takes" executed on event where one take hits source boundary | All takes slip together until the most restrictive take reaches boundary, or individual clamped |

---

## 5. Verification Method

To independently verify the facts, formulas, and references documented in this report:

1. **Verify Manual References & Quotes**:
   Inspect `manuais/markdown/manual_vegaspro21_en.md` using PowerShell:
   ```powershell
   Select-String -Path manuais\markdown\manual_vegaspro21_en.md -Pattern 'Hold Shift while dragging the event edge to ignore event grouping' -Context 2,2
   Select-String -Path manuais\markdown\manual_vegaspro21_en.md -Pattern 'Repairing audio/video synchronization offsets' -Context 2,2
   Select-String -Path manuais\markdown\manual_vegaspro21_en.md -Pattern 'Using sync links' -Context 2,2
   Select-String -Path manuais\markdown\manual_vegaspro21_en.md -Pattern 'Quick fade length for audio events' -Context 2,2
   Select-String -Path manuais\markdown\manual_vegaspro21_en.md -Pattern 'Create Multicamera Track' -Context 2,2
   ```

2. **Verify Keyboard Shortcuts**:
   Inspect `manuais/markdown/VEGAS_Pro_18_EN_Keyboard_shortcuts.md`:
   ```powershell
   Select-String -Path manuais\markdown\VEGAS_Pro_18_EN_Keyboard_shortcuts.md -Pattern 'L/J trim|L/J cut|Enable multicamera editing|Choose multicamera take' -Context 1,1
   ```

3. **Verify Reverse-Engineered Command IDs & Preferences**:
   Inspect `results.md`:
   ```powershell
   Select-String -Path results.md -Pattern 'Tools.Multicamera|QuickfadeAudioEdits|TPT_ntQuickfadeAudioSize' -Context 1,1
   ```

4. **Verify Existing Code Contracts & Fade Formulas**:
   Run cargo test on `neoveg_common` to verify fade curve mathematical properties:
   ```powershell
   cargo test -p neoveg_common --lib fade -j 1 -- --test-threads=1
   ```
