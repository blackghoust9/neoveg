# AGENTS.md — AI Agent Development Guidelines & Standards

This document defines the architectural standards, naming conventions, coding guidelines, and development rules for the **Neoveg** project. All AI agents (including Antigravity) and human contributors must strictly follow these instructions when modifying or extending the repository.

**How this document is organized**

- **Part I (sections 1–11) is the current standard.** Neoveg is a pure Rust Cargo workspace; these rules apply to everything under `crates/`.
- **Part II only records that the legacy C# / C++ / Python code was removed** (it stays in Git history).

Feature status (what is done and what is missing) lives in the roadmap of `README.md`, not here. This file states how things must be built, not how far along they are.

---

# Part I — Current Standard (Pure Rust)

## 1. Project Identity & Naming

- **Official name:** `Neoveg` for UI branding, window titles, documentation and prose; `neoveg` for files, folders, crates, binaries and modules.
- **Prohibited suffixes:** do **not** use `Studio` or `Pro` in any name. The project is solely and strictly **Neoveg**.
- **Crates:** `neoveg_common`, `neoveg_audio`, `neoveg_video`, `neoveg_engine`, `neoveg_ui`, in `crates/<crate_name>/`. The application binary is `neoveg` (`crates/neoveg_ui`, `[[bin]] name = "neoveg"`).
- **Native format and extension:** `.neoveg`.

---

## 2. Architecture

### 2.1. Workspace layout

```
Neoveg Workspace
├── Cargo.toml                 # workspace root (members = crates/*, exclude = rust-core)
├── Cargo.lock
├── crates/
│   ├── neoveg_common/         # project types, SMPTE timecode, FadeCurve, logging, preferences, per-user paths
│   ├── neoveg_audio/          # symphonia decoding, multi-track mixer, cpal output, peak and EBU R128 loudness meters (dsp/ adapted from FilmCraft), waveform source
│   ├── neoveg_video/          # RGBA frames, generators, wgpu upload, FFmpeg decoding + frame worker (feature `ffmpeg`)
│   ├── neoveg_engine/         # timeline, undoable commands, edit rules, crossfades, .neoveg files, autosave, relink
│   └── neoveg_ui/             # egui/eframe application (binary `neoveg`)
├── docs/                      # research knowledge base (docs/research/nle_features_research.txt) and the legacy QA dashboard
├── Assets/i18n/               # runtime localization resources (pt-BR default)
└── extensions/, third_party/, plugins/, scripts/, runtimes/   # supporting material
```

Dependency direction: `neoveg_common` is the base; `neoveg_audio`, `neoveg_video` and `neoveg_engine` depend only on it; `neoveg_ui` depends on all of them. `neoveg_engine` must not depend on audio, video or UI code, so the editing model stays testable without a window or a sound card.

### 2.2. Pure Rust standard

- Neoveg is a pure Rust Cargo Workspace (`crates/neoveg_ui`, `crates/neoveg_engine`, `crates/neoveg_audio`, `crates/neoveg_video`, `crates/neoveg_common`).
- **Zero Legacy Code:** C#, C++, Python and external FFI wrappers are completely deprecated. Do not introduce them for core editing features. All editing, timeline logic, rendering and audio processing run natively in Rust. The legacy directories were removed from the repository (Git history keeps them).
- **UI & graphics:** `egui` / `eframe` with `egui_dock` for Classic NLE-style dockable panels; video and timeline rendering on the GPU through `wgpu` (WebGPU).
- **Audio:** `symphonia` for decoding, `cpal` for hardware output.
- **Video:** FFmpeg through `ffmpeg-next`, behind the optional `ffmpeg` feature (`neoveg_video/ffmpeg`, forwarded as `neoveg_ui/ffmpeg`) so a plain `cargo build --workspace` needs no system FFmpeg. Without the feature the application still starts and reports in the status bar that video decoding is disabled.

### 2.3. Engineering rules

- **The UI thread never blocks.** Media decoding, waveform construction, video frame decoding and autosave run on worker threads; the UI only sends requests and polls results. Scrubbing floods requests, so workers drain the queue and process only the newest one.
- **Every timeline change goes through `History::execute(EditCommand)`.** One user gesture (a drag, a menu action) is exactly one undo step. Commands carry everything needed to apply and revert them. `History::revision()` is the single source of truth for "unsaved changes".
- **Pure rules live in the engine.** Geometry, snapping, trimming, crossfade detection and relinking are plain functions in `neoveg_engine` with unit tests; the UI only translates pointer input into calls. UI state logic (drag handling, hit-testing, clip building) must stay testable without a GUI.
- **Real-time audio callback:** no allocation, locks or I/O. It reads an immutable mix plan and shared atomics only.
- **Persisted formats are versioned and backward compatible:** carry a `format_version`, and give every field added later a `#[serde(default)]` so older files keep loading (tests must cover this).
- **Errors:** return `Result` for anything driven by user data or the filesystem; never panic on a bad file. `unsafe` is not used; if it ever becomes necessary it needs a `// SAFETY:` comment and a test.
- **Lint-clean:** `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all` must pass before every commit.

---

## 3. Build, Test & Quality Gates

- **Single toolchain:** the only mandatory build dependency is the official Rust toolchain (`rustup` / `cargo`). Versions and per-OS system libraries are in `REQUIREMENTS.md`; contributor setup is in `README.md`.
- **Standard commands:**
  - Build: `cargo build --workspace --release`
  - Test: `cargo test --workspace`
  - Lint: `cargo clippy --workspace --all-targets -- -D warnings`
  - Format: `cargo fmt --all -- --check`
  - Coverage: `cargo llvm-cov --workspace --summary-only`
  - Run: `cargo run --release -p neoveg_ui` (add `--features ffmpeg` for video)
  - Package and QA report: `python build_portable.py` (QA gates, release build, portable folders, archives and the standalone exe for this OS), `python build_portable.py qa` (gates and report only), `python build_portable.py check` (toolchain)
- **FFmpeg feature toolchain:** FFmpeg **7.1.x shared** build with headers and import libraries (`FFMPEG_DIR`), LLVM / `libclang` **19.x** (`LIBCLANG_PATH`; LLVM 22 or newer breaks `ffmpeg-sys-next` 7.1.3), and the FFmpeg DLLs on `PATH` at run time. Test it with `cargo test --workspace --features neoveg_ui/ffmpeg`. The decoding tests build a clip with the `ffmpeg` command-line tool and skip silently when it is missing.
- **Tests are mandatory for new behavior:** unit tests for pure rules, round-trip tests for file formats (including older-version files), and state-level tests for UI logic. Do not weaken or delete a test to make a change pass.
- **Benchmarks and profiling:** waveform and mixer benchmarks belong in Criterion (`benches/`), and Tracy is the profiler for representative timeline and rendering workloads. (none exists yet).
- **Memory and thread safety:** sanitizers apply to any future `unsafe`/native code; today the compiler plus Miri-compatible safe code is the guarantee.
- **Release gate:** do not merge to `main` or build a release package until the commands above pass on a clean checkout.

---

## 4. UI / UX Specification

The behavior below is normative. Colors, sizes and thresholds are part of the specification.

### 4.1. Workspace, panels and menus

- **Clean startup:** Neoveg opens directly in the main workspace with an empty project (0 tracks, 0 media clips, `show_welcome_on_startup = false`). A short startup screen (`neoveg_ui/src/splash.rs`) covers the window while the startup steps run one per frame (`STARTUP_STEPS` in `app.rs`: preferences and keymap, graphics adapter, audio output probe, effects warm-up, video decoder worker, recovery files); its progress bar follows those real steps, each step stays on screen at least 0.3 s, and any key or click skips the pacing. It never blocks the UI thread, and a new startup task must be added as a step, not as work in `NeovegApp::new`. The 3D shapes and the film-reel artwork are drawn with egui shapes (no asset files); the artwork is original and released under the project's MIT licence.
- **Themes:** keep visual fidelity with the Classic NLE themes (Classic Foundry, Classic Silver, Modern Carbon Dark, Contemporary Electric Dark).
- **Icon-only buttons:** toolbar and panel buttons show graphical icons only; names, shortcuts and descriptions appear exclusively as hover tooltips. Icons are original flat vector pictograms drawn with egui shapes (`neoveg_ui/src/icons.rs`, `icons::icon_button`), never emoji or font glyphs, so they look the same on every OS; a new icon is a variant of `Icon` plus its drawing, and the tests check that it stays inside its frame.
- **Floating and undockable panels:** the primary panels (**Timeline**, **Video Preview**, **Trimmer / Project Media** and **Master Bus Mixer**) can be detached into independent floating windows; closing a floating window docks it back into its workspace slot.
- **Main toolbar:** the primary icon toolbar sits directly below the multi-track timeline.
- **Docked panel tabs:** every docked window with tabs (Project Media, Trimmer, Video FX, Audio FX, Transitions, Generators, dockable extensions) uses the compact footer tab pattern: tab strip at the bottom of the window, compact flat headers (font size 11, padding `6,2,6,2`, 22 px high, no selection pipe), a horizontally scrolling strip with left/right arrow buttons (and mouse-wheel scrolling) that appear only when the tabs do not fit, and the selected tab always scrolled into view. Tabs stay undockable by dragging them out of the strip. `egui_dock` cannot place its own strip at the bottom, so grouped panels draw their own footer tabs (`footer_tabs` in `neoveg_ui`).
- **Precision slider** (opacity, volume, pan, master fader; `neoveg_ui/src/precision_slider.rs`): rectangular flat thumb (square corners, 8 px wide, solid fill); double-click resets to the default (`1.0` / `100 %` for opacity and volume, `0.0` for pan); holding `Ctrl` while dragging applies `1/10` sensitivity; the right-click menu offers `Restaurar Padrão` and `Digitar Valor...`.
- **Menus:**
  - **Editar** contains **Preferências**, which holds the interface **Idioma** selector (it is not in Ferramentas or the toolbar).
  - Automatic fade-in/out on media import (audio, video, image) is **off by default** (`auto_fade_on_import = false`, fades `0.0`) and is a checkbox in **Editar → Preferências**.
  - The **Inspector** opens from **Ferramentas → Inspetor**. Do not embed input controls (for example a `ComboBox`) inside menus.
  - Scripting lives in Tools → Scripts and the dockable extension manager in View → Window Extensions, as regular menu items.
- **Event header:** every timeline clip renders an event header with *Generated Media*, *Event Pan/Crop*, *Video/Audio FX* and *Hamburger Menu (☰)* buttons.
- **Event Pan/Crop:** it opens as a pop-up window from the video clip's event header button (it is not docked beside the Preview) and includes an embedded side-by-side live Video Preview toggle next to the `'F'` gizmo canvas.
- **Internationalization:** default language Brazilian Portuguese (`pt-BR`) with runtime support for `en-US`, `de-DE`, `es-ES`, `fr-FR`, `zh-CN`, `ja-JP` and `ru-RU`; resources live in `Assets/i18n/`.

### 4.2. Video Preview

- **Clean preview:** every overlay and on-screen tool (SMPTE HUD, safe areas, grid, the `'F'` pan/crop gizmo, split-screen FX bypass, clip overlay labels) starts **disabled** and is exposed through the overlays menu/flyout or the context menu.
- **Toolbar:** compact, flat, square-cornered, icon-only. It offers project video properties (resolution, FPS, pixel/display aspect); one **Preview Quality** selector with only `Draft`, `Preview`, `Good`, `Best` and `Auto`; project video FX bypass; split-screen; overlays; PNG/JPEG snapshot save and clipboard copy; external-monitor pop-out; and full-screen preview (`Esc` closes). There is no separate resolution selector.
- **Quality implies resolution:** Draft = 1/4, Preview = 1/2, Good and Best = full, Auto = fit to the preview window. The decoder scales frames to the selected width.
- **Status strip:** shows live project dimensions, bit depth and frame rate plus the effective preview dimensions, refreshed when project settings, quality or viewport size change.

### 4.3. Timeline

- **Track header indicator:** the dot in the track number box (`• 1`, `• 2`...) blinks **only for the selected track**, alternating white (`#FFFFFF`) and black (`#000000`) every `500 ms`. Unselected tracks keep a static neutral dot; the blink stops when no track is selected. A track becomes selected by clicking its header or any point of its lane.
- **Playhead / edit cursor:**
  - Blue pentagonal head (`#2980B9` / `#3498DB`) with a pointed bottom beak and a horizontal right-extending flag, a `1 px` vertical blue guide line across all tracks, and a timecode box (SMPTE drop-frame `;` or non-drop `:`) next to the marker.
  - The caret blinks (about `500 ms`, alternating opacity `1.0` / `0.0`) while playback is paused, stays solid during playback or scrubbing, and restarts on keyboard frame navigation (`←` / `→`).
  - **Ultra-fine guide line:** exactly one *physical* pixel wide at every zoom level, scroll offset and monitor DPI: stroke width `1 / pixels_per_point`, X centered on a device pixel (`frame_grid::snap_to_device_pixel`). Never scale the line with the zoom.
- **Dynamic frame grid** (`neoveg_ui/src/frame_grid.rs`): density follows the zoom through the ladder `1, 2, 5, 10 frames → 1 s, 2 s, 5 s, 10 s, 15 s, 30 s, 1 min, 2 min, 5 min…`. Minor lines use the smallest step at least `8 px` apart (one line per frame — 1/24, 1/25, 1/30, 1/60 s from the project FPS — at close zoom); major lines use the smallest step at least `90 px` apart and carry the SMPTE ruler labels. Minor lines are `#2C3038`, major lines `#4A505E`; both are one physical pixel, pixel-snapped, drawn over the lane background and behind events, only for the visible range. Grid times derive from integer frame indices (never accumulated floats); second steps land on the frame where the timecode reads `HH:MM:SS;00`, honoring drop-frame (`00:01:00;02` at dropped minute boundaries). Research notes: `docs/research/nle_features_research.txt` (entries 4–5).
- **Zoom and scrolling:** zoom runs from `15` to `480,000` px/s (10 px per sample at 48 kHz); wheel zoom is anchored at the pointer and toolbar zoom at the edit cursor. Horizontal navigation is one time-based offset shared by the scrollbar, ruler, event canvas and playhead; track headers and lanes share one vertical offset. Event widths use durations (never an absolute x that includes the scroll offset), and off-screen events are culled and clamped to ±10,000 px to protect float precision. Scrollbars are compact, flat and square-cornered, with adjacent zoom-out / zoom-in controls.
- **Render status ruler:** a tri-color bar above the timeline: *green* (`#2ECC71`, RAM/prerender at 60 FPS), *yellow* (`#F1C40F`, simple/queued) and *red* (`#E74C3C`, complex/unrendered).
- **Mouse editing of events:**
  - Dragging the body moves the event, also between tracks of the **same kind**; dragging the first or last 7 px (a third of the width on narrow events) trims that edge; dragging a fade handle (top strip, 8 px inside each edge while the fade is zero) sets the fade length.
  - Trimming keeps the source aligned (`trim_in` follows the left edge), never reveals media before the source start or after its end, and keeps at least one frame. Fades are re-clamped to the event.
  - **Snapping** (toggle `F8` or the toolbar magnet; hold `Alt` while dragging to ignore it) aligns to the origin, the playhead and the edges of other events within `8 px`; otherwise starts and trim edges are quantized to frames.
  - A drag previews live and changes the timeline only on release, as **one** undo step. `Esc` cancels. The lane area scrolls automatically when the pointer rests within `28 px` of its ends. A read-out shows the start timecode and duration.
  - **Multiple selection:** `Ctrl`+click toggles, `Shift`+click selects the range, `Shift`+drag over empty lane draws a selection rectangle (`Ctrl` keeps the current selection). Dragging one selected event moves the whole selection in time as one undo step, never before zero.
  - Cursors: resize arrows on trim zones and handles, grab / grabbing on the body.

### 4.4. Audio

- **Sample-accurate waveform** (`neoveg_audio::waveform`, `neoveg_ui/src/waveform_paint.rs`): waveforms come from the real PCM samples, never from interpolated peak summaries.
  - The source keeps the decoded samples plus `256`-frame min/max/sum-of-squares summaries; sources are built on worker threads, never on the UI thread.
  - **≥ 1 sample per pixel:** the exact min/max (outer) and RMS (inner) of every visible pixel column; every non-silent column is at least 1 px tall, so a single-sample transient is never averaged away.
  - **< 1 sample per pixel:** the individual samples joined by lines, with dots and stems once samples are at least `6 px` apart (Audacity style).
  - Only the visible range is computed; clip `trim_in`, gain and fades are honored.
- **Audio Master Bus:**
  - The panel renders exclusively the stereo Left (`L`) and Right (`R`) dBFS peak meters, the dBFS scale (`0`, `-6`, `-12`, `-24`, `-48`, `-∞`), real-time `L` / `R` dBFS readouts and the Master Volume fader (`MASTER BUS: +0.0 dB`). It must **never** contain a video RGB parade or scope in its top area.
  - **Meters respond strictly to the real audio pipeline** (`neoveg_audio::meter::PeakMeter`, fed from the samples sent to the device) — no timers, random numbers, oscillators or mock data in production builds.
  - When playback is stopped, or no unmuted audio is active at the playhead, every meter is completely zeroed and static (`0.0` linear / −∞ dB).
- **Loudness panel** (`neoveg_ui/src/loudness_panel.rs`, dock tab **Loudness** under the Master Bus): momentary (400 ms), short-term (3 s) and integrated loudness in LUFS, loudness range (LRA) and true peak (dBTP) to ITU-R BS.1770-4 / EBU R128, against a selectable target (EBU R128 −23 LUFS / −1 dBTP, ATSC A/85 −24 LKFS / −2 dBTP, streaming −14, podcast −16). The meter (`neoveg_audio::dsp::loudness`, adapted from FilmCraft, see section 8) runs inside the audio callback on the samples sent to the device and publishes through atomics (`neoveg_audio::readout`); it allocates nothing during playback. Every start of playback is a new measurement; M and S read `-∞` when stopped while I, LRA and TP keep the last programme values. No simulated values. It stays out of the Master Bus panel, which only holds the L / R peak meters.
- **Audio effects** (`neoveg_audio::dsp::effects`, adapted from FilmCraft; dock tab **Efeitos de Áudio** under Project Media): every audio track and the master bus have an ordered chain of up to 8 effects (28 effects: parametric, 3-band, graphic and full parametric EQ, notch, classical and FFT filters, compressor, multiband and tube compressors, gate, limiter, delay, reverb, chorus, flanger, phaser, pitch shifter, DeHum, DeNoise, RNNoise neural noise suppression, click remover, DeEsser, DeReverb, speech enhance, stereo width, gain, balance, phase invert), each with bypass and its own parameters. The chain is part of the project (`Track::effects`, `Timeline::master_effects`, `#[serde(default)]`) and every change is one `EditCommand::SetEffects` undo step; a slider drag commits on release. `neoveg_audio::fxmixer::BlockMixer` is built outside the audio callback (buses and effects are allocated there) and runs the chains on blocks of at most 512 frames inside it with no allocation or lock; tracks without effects share one dry bus. The UI turns the project into an `FxPlan` (`fx_panel::fx_plan`, bypassed effects left out). Changing a chain during playback restarts the audio at the cursor. **Latency compensation:** effects that delay the signal (limiter, DeNoise, FFT filter) never shift a track against the others or against the picture: `BlockMixer` delays the faster buses to the slowest one and feeds the mix `lead_frames()` ahead after priming the chains, and the callback keeps a separate `heard_pos` (what the device is playing, reported as the position) and `mix_pos` (the lead). **Auto-ducking** (`Track::ducking`, `EditCommand::SetDucking`, `neoveg_audio::duck`): a track is turned down by `reduce_db` while its controller tracks have sound; the controllers are analysed when playback starts (10 ms level, activity regions, ramps that start before the sound) and the resulting gain curve is applied per frame to the ducked track's own bus.
- **Subtitles** (`neoveg_engine::captions`, tab **Legendas** of the media panel, `neoveg_ui/src/captions_panel.rs`): `Timeline::captions` (`#[serde(default)]`), `EditCommand::ReplaceCaptions` (one undo step for an import or an edit), SubRip and WebVTT reading and writing (adapted from FilmCraft, see section 8), the text drawn over the video preview and File > import / export. They are plain functions in the engine; the panel only returns `CaptionAction`s.
- **Edit decision lists** (`neoveg_engine::edl`): CMX 3600 export of the selected track and import of cuts into the media that is already in the project (matched by source file, clip name or reel), as one `EditCommand::Batch` undo step. Neoveg's own code on the public format; times go through `neoveg_common::timecode::{frame_to_timecode, parse_timecode}`.
- **Mixer** (`neoveg_audio::mixer`): sums every unmuted audio event active at the playback position, honoring `trim_in`, gain, fades and crossfades, with linear interpolation of the source.
- **Fades:** five curves — `Linear`, `Fast`, `Slow`, `Smooth`, `Sharp` (`neoveg_common::fade::FadeCurve`, `gain(x)`: `x`, `1-(1-x)²`, `x²`, smoothstep, inverse smoothstep). A fade-in uses the curve directly; a fade-out evaluates it at the remaining fraction. Each event stores its own fade-in and fade-out length and curve.
- **Automatic crossfades** (`neoveg_engine::crossfade`, project setting `auto_crossfade`, on by default): events on the same track that overlap *partially* (the later one starts inside the earlier one and ends after it) crossfade over the overlap, using the later event's fade-in curve for the fade-in and its mirror for the earlier event's fade-out, multiplied with each event's own fades. An event lying entirely inside another is not crossfaded.
- **Sub-frame editing:** times are `f64` seconds with sub-frame precision (about `1e-6 s`); editing gestures quantize to frames.

### 4.5. Keyboard shortcuts

| Shortcut | Action | State |
| :--- | :--- | :--- |
| `Ctrl+Q` / `Ctrl+Shift+Q` | Insert audio / video track | implemented |
| `Ctrl+R` | Record from the default input device at the cursor; pressed again, stops and puts the take on the timeline | implemented |
| `S` | Split at the cursor: the selected events, or the events under the cursor on every track when nothing is selected; with a time selection, at both of its edges | implemented |
| `Space` | Play / pause | implemented |
| `←` / `→` | Move the cursor one frame | implemented |
| `Delete` | Remove the selected event | implemented |
| `F8` | Toggle snapping | implemented |
| `Ctrl+Z` / `Ctrl+Y` (`Ctrl+Shift+Z`) | Undo / redo | implemented |
| `Ctrl+O` / `Ctrl+S` / `Ctrl+Shift+S` | Open / save / save as | implemented |
| `M` | Marker (opens the inline name box) | implemented |
| `G` / `U` | Group / ungroup the selected events (a group is selected, moved and split together) | implemented |
| `Ctrl+Shift+U` | Ignore event grouping (clicks select a single event) | implemented |
| `R` | Region from the time selection | implemented |
| `I` / `O` | Set the time selection in / out point at the cursor | implemented |
| `Ctrl+A` / `Ctrl+Shift+A` | Select all events / clear the selection and the time selection | implemented |
| `Ctrl+X` / `Ctrl+C` / `Ctrl+V` | Cut / copy / paste events or the time selection (paste at the cursor on the selected track) | implemented |
| `Ctrl+Shift+V` / `Ctrl+B` | Paste insert / paste repeat (opens a dialog) | implemented |
| `Ctrl+T` | Trim events to the time selection | implemented |
| `Alt+[` / `Alt+]` | Trim event start / end to the cursor | implemented |
| `[` / `]` or `Numpad 7` / `Numpad 9` | Select previous / next event edge (edge-trim mode) | planned |
| `Numpad 1` / `3`, `4` / `6` | Trim or nudge by one frame / one pixel (modifiers: `Ctrl` stretch, `Alt` slip, `Ctrl+Alt` slide) | planned |
| `Numpad 5` | Enter / exit expanded edit mode | planned |
| `F` / `Ctrl+F` / `Ctrl+Shift+F` | Post-edit ripple: affected tracks / + markers / everything | planned |
| `Ctrl+L` | Auto ripple | planned |
| `Ctrl+Shift+X` | Automatic crossfades | implemented |
| `Ctrl+F8` / `Shift+F8` / `Ctrl+Shift+F8` | Snap to grid / markers / all events | planned |
| `Alt+F8` | Quantize to frames | planned |
| `Z` / `X` | Mute / solo selected tracks (`Shift` = exclusive) | implemented |
| `V` / `P` | Insert, show or hide the track volume / pan envelope (`Shift` removes) | planned |
| `Ctrl+Shift+E` | Show or hide envelopes | planned |
| `Q` | Loop playback | implemented |
| `Shift+Q` | Set the loop region (time selection or region under the cursor) | implemented |
| `Ctrl+Home` / `Ctrl+End` | Go to project start / end | implemented |
| `Ctrl+←` / `Ctrl+→` | Previous / next marker or region edge | implemented |
| `1`–`9`, `0` | Go to the 1st–9th, 10th marker (by time) | implemented |
| `Ctrl+G` | Go to time (timecode, minutes:seconds, seconds or frames) | implemented |
| `Home` / `End` / `Ctrl+Alt+←` / `Ctrl+Alt+→` / `\` | Cursor to the start / end of the time selection or view; to the previous / next event or fade edge; centre the view on the cursor | implemented |
| `Ctrl+M` | Render to new track | planned |
| `Alt+0`, `Alt+2` … `Alt+5`, `Ctrl+Alt+2`, `Ctrl+Alt+7`, `Ctrl+Alt+9` | Show / hide the Timeline, Trimmer, Master Bus, Preview, Project Media, Video Scopes, Loudness and Audio Effects windows (in front -> closed, behind a tab -> raised, closed -> back); the others (Explorer, Project Notes, Transitions, Video FX, ...) are planned | implemented |
| Exibir → Layouts | Save / recall window layouts 1-9 and the default layout (session only; the `Alt+D` chords are planned) | implemented |

**How shortcuts are implemented:** every action is a `Command` in `neoveg_common::commands` (stable dotted id, pt-BR label, category, `implemented()` flag); `neoveg_common::keymap` holds the default bindings and the user's overrides (`keymap` in `preferences.json`, only the commands that differ from the defaults, an empty list meaning unbound); `neoveg_ui::shortcuts` converts egui key events. The UI dispatches by command, never by a hard-coded key, and shows shortcut hints in menus from the active map. A new command needs an entry in the `commands!` table, a default in `DEFAULT_BINDINGS` (the tests reject duplicate shortcuts and unparseable defaults) and an arm in `AppState::run_command`; **Editar → Personalizar teclado…** lists it automatically. Commands with `implemented() == false` own their default shortcut but do nothing until the feature lands.

**Source and rules for the planned rows:** they come from the default key map of the reference manuals (research entry 17 in `docs/research/nle_features_research.txt`). Divergences already accepted: Neoveg keeps `←` / `→` for one frame (the reference uses `Alt+←` / `Alt+→` and uses the plain arrows for one pixel when quantize is off) and keeps `Alt` as "ignore snapping" while dragging (the reference uses `Shift` for that and `Alt` for slip). Any shortcut added here must also be listed in the customizable key map (`keymap` in `preferences.json`) so users can rebind it.

---

## 5. Project Files, Autosave & Relink

- **Native format (`.neoveg`):** the exclusive project format. It is a ZIP container with `project.json` (name, settings, timeline), `media_pool.json` and `history.json` (reserved; the undo history is session-only). Neoveg creates, opens, autosaves and manages projects strictly in `.neoveg` (no `.veg` or `.otio` project files). Saves are atomic (temporary file, then rename) and files written by a newer `format_version` are rejected with a clear message.
- **Autosave & recovery:** `neoveg_engine::autosave` writes `recovery_<session>.neoveg` — a normal `.neoveg` plus a `recovery.json` entry — to `neoveg_common::paths::autosave_dir()` from a worker thread, and only while the project has unsaved changes (`History::revision`). The interval is a preference (`autosave_minutes`, default 2, `0` disables). A clean exit, a save, opening another project or creating a new one deletes the session's file; leftovers are offered for recovery at the next start and pruned after 30 days. This replaces the legacy `projects/autosave/` location. One Neoveg instance per user is assumed.
- **Unsaved changes:** the window title shows `*` while the project is dirty, and closing a dirty project asks to save, discard or cancel.
- **Missing media (relink):** on opening a project with disconnected media, the engine runs the search below (`neoveg_engine::relink`), then reloads the rest in the background and reports what is still missing in the status bar:
  - *Step 1:* exact file name (case-insensitive) in the project folder, the old media folders and sibling folders.
  - *Step 2:* fuzzy file name (normalized stem, same extension).
  - *Step 3:* metadata validation with FFprobe — frame rate, channels and original duration — of the candidates, followed by waveform regeneration. (Planned; see the README roadmap.)
- **XML interchange:** import/export of FCP7 XML (`.xml`), FCPX (`.fcpxml`) and CMX3600 (`.edl`) for DaVinci Resolve, Premiere Pro and Final Cut Pro. (Planned.)

---

## 6. Logging & Crash Diagnostics

- **Central log directory:** all logs go to `logs/neoveg.log` (active session) and `logs/crash.log` (unhandled panics) inside the application root directory. `neoveg_common::logging::init` installs the `tracing` subscriber and a panic hook that writes both.
- **Retention (target):** keep up to 5 historical files (`logs/neoveg.YYYY-MM-DD.log`) of at most 10 MB each. (Rotation is not implemented yet.)
- **Levels:** `[DEBUG]`, `[INFO]`, `[WARN]`, `[ERROR]`, `[FATAL]`. Media decoding and file I/O must log context (file path, frame or time, error text) **before** raising a UI alert.
- **Log viewer:** the diagnostic log viewer must provide a quick **Copiar Logs** action (copy to the clipboard through `egui`'s output), supporting partial text selection and full-log copying, with the visual confirmation `Copiado!` for 2 seconds. (Planned.)

---

## 7. Platform Support, Data Locations & Packaging

- **Targets:** Windows `win-x64`, Linux `linux-x64`, macOS `osx-x64` and `osx-arm64`. Use `cfg!(windows)` / `cfg!(target_os = ...)` and `std::path` — no hardcoded Windows-only paths; Windows-only helpers (for example the Google Drive mirror) are guarded or overridable through environment variables.
- **Per-user data:**

  | | Windows | Linux | macOS |
  | :--- | :--- | :--- | :--- |
  | App data and autosave | `%LOCALAPPDATA%\Neoveg\` (`autosave\`) | `~/.local/share/neoveg/` | `~/Library/Application Support/neoveg/` |
  | Preferences (`preferences.json`) | `%APPDATA%\neoveg\` | `~/.config/neoveg/` | `~/Library/Application Support/neoveg/` |
  | Logs | `logs/` beside the executable | same | same |

- **Portable release:** build with `cargo build --release -p neoveg_ui` (add `--features ffmpeg` for video and ship the FFmpeg shared libraries beside the executable, because direct launches must not depend on an inherited `PATH`). Archive names (`<version>` is `[workspace.package].version` in `Cargo.toml`): `dist/Neoveg_v<version>_win-x64_Portable.zip`, `..._linux-x64_Portable.tar.gz`, `..._osx-x64_Portable.tar.gz`, `..._osx-arm64_Portable.tar.gz`, plus the standalone `dist/neoveg_v<version>_<rid>[.exe]`, `dist/SHA256SUMS.txt` and `dist/build_manifest.json`. Every package includes `RELEASE_NOTES.md`, `AUTHORS.md` and `LICENSE`. `build_portable.py` (Python standard library only, build tooling and not application code) produces all of this: it runs the quality gates first (`--skip-qa` is for local experiments only), builds the Windows target with a static C runtime so no Visual C++ redistributable is needed, bundles the FFmpeg libraries next to the executable when `--features ffmpeg` is requested (`--ffmpeg`), writes `neoveg.sh` for Linux and an `.app` bundle for macOS, and builds targets of other operating systems only when asked with `--targets` and the Rust target is installed.
- **Cross-platform boundary:** Windows-only prerequisites and installers never run on Linux or macOS.

---

## 8. Attribution & Licensing

- All third-party dependencies and integrated code (FFmpeg, `egui`/`eframe`, `wgpu`, `cpal`, `symphonia`, VST3 SDK, OpenFX, Whisper.cpp, Voukoder, CorridorKey) must have authors and licenses documented in `AUTHORS.md` and `third_party/README.md`.
- **Adapted third-party source** (code copied into the workspace, not a Cargo dependency) additionally needs: the licence text under `third_party/<project>/` (shipped by `build_portable.py`), an origin and copyright header at the top of every copied file, a `third_party/<project>/README.md` with the repository, commit, files copied and changes made, an entry in the in-app credits (`neoveg_ui/src/credits.rs`, a test checks it), and no use of the project's trademarks. Today: FilmCraft `audio-dsp` (loudness meter, effects, ducking analysis) and `captions` (SubRip, WebVTT), `MIT OR Apache-2.0`, used under MIT. **A Cargo dependency whose licence asks for notices in binary distributions** (BSD-3-Clause `nnnoiseless`) gets its licence text under `third_party/<crate>/`, shipped by `build_portable.py`, and an entry in the credits.
- Development builds use the **LGPL** shared FFmpeg variant. Releases must keep FFmpeg dynamically linked and ship its license text.
- Packaging copies `AUTHORS.md` and `LICENSE` into every generated package in `dist/`.

---

## 9. Functional Specifications & Reference Research

Neoveg's UI/UX layout, editing mechanics, shortcut keys and signal-routing pipelines follow broadcast NLE standards and clean-room architectural specifications:

- **Classic NLE audio & timeline foundations (SMPTE ST 12-1 & multi-track DSP):** sub-frame editing, 5-curve fade envelopes and automatic overlap crossfades.
- **Procedural titles, GPU acceleration & media generators:** real-time procedural media (`Neoveg Titles & Text Overlay`, `SMPTE Color Bars`, `Solid Color`, `Test Pattern`).
- **Expanded NLE workflows & Trimmer integration:** synchronized Timeline and Trimmer editing, ripple modes and frame-quantized trimming.
- **Docking UI, color grading & hierarchical FX pipelines:** detachable floating panels, compact bottom-tabbed docks and 5-stage signal routing (`Media FX → Take FX → Event FX → Track FX → Master FX`).
- **Modern dark UI & motion tracking:** vector-gizmo Event Pan/Crop (`'F'` frame canvas), keyframe diamond interpolation and 2D affine camera matrices.
- **Contemporary AI & native plugin ecosystem:** OpenFX, VST3, Voukoder, Whisper speech-to-subtitle transcription and CorridorKey chroma/alpha keying, implemented in Rust (plugin hosts through Rust crates or out-of-process isolation).

**Implementation mandate:** window layouts, keyboard shortcuts (section 4.5), audio bus topology, sample-accurate waveforms and event-manipulation behaviors must mirror these specifications and the research notes in `docs/research/nle_features_research.txt`.

### 9.1. Reference software research & parity

To ensure 100% operational, structural, and visual fidelity with industry standards:
- **Reference Documentation:** Researchers and AI agents are fully permitted and encouraged to reference VEGAS Pro, Sony/MAGIX workflows, menus, shortcuts, and engine behaviors to design exact counterparts in Neoveg.
- **Knowledge Base Persistence:** All scanned features, dialog structures, and mechanics must be documented in `docs/research/nle_features_research.txt` and `results.md` using clear and precise technical descriptions.

---

## 10. Standards Compliance (ISO/IEC & ABNT NBR)

Neoveg aligns its development and QA practice with ISO/IEC 25010 (quality model), ISO/IEC 29119 (testing), ISO/IEC 27001 (information and memory safety) and the harmonized ABNT NBR ISO/IEC 25010 / 29119 / 12207 / 14764 / 27001 standards:

- **Performance efficiency:** timeline scrubbing, waveform drawing and zooming stay responsive (heavy work on worker threads, no per-frame allocations in hot drawing paths, culled off-screen work).
- **Functional suitability:** parity with Classic NLE editing workflows, shortcuts and signal routing (section 9).
- **Reliability:** fault-tolerant behavior with structured logs (`logs/neoveg.log`, `logs/crash.log`), atomic file writes, autosave and recovery, and graceful fallbacks (for example, no audio device or no FFmpeg).
- **Usability:** Classic NLE UI fidelity, `pt-BR` default with 8 locales, discoverable tooltips and keyboard shortcuts, and visible diagnostics.
- **Portability:** native builds on Windows, Linux and macOS through the single Rust toolchain.
- **Maintainability:** strict crate boundaries (section 2.1), pure testable rules, `clippy -D warnings` and `rustfmt`.
- **Testing (29119):** unit, integration and state-level UI tests run by `cargo test --workspace`, with coverage evidence from `cargo llvm-cov`.
- **Security and memory safety (27001):** safe Rust, bounds-checked buffers, overflow-aware time and size arithmetic, and per-user data locations.
- **Traceability:** each automated QA step of the QA runner is tagged with its ISO/IEC 25010 pillar and the ABNT NBR / ISO/IEC requirement it evidences, recorded in `logs/neoveg.log` and `coverage/qa/qa_report.json` (rendered by `docs/index.html`). `python build_portable.py qa` does this today (rustfmt, clippy, tests and release build, each tagged with its pillar and standard).

---

## 11. Git Workspace & Cloud Backup

- **Primary working directory:** the local workspace is the source of truth; all development, Git commits and GitHub synchronization run from it.
- **Active branch:** `refactor/100-percent-rust` carries the Rust rewrite (created from `release/v1.0.0-qa-refactor`). Merge into `main` only after the quality gates of section 3 pass.
- **Commits:** Conventional-Commit style (`feat(...)`, `fix(...)`, `docs(...)`, `arch(...)`), subject line in Portuguese without accents; keep generated artifacts (`target/`, `dist/`, `logs/`, `coverage/`) out of Git.
- **Cloud backup path:** `G:\Meu Drive\vp-rev-eng\teste\teste2`, mirrored with `scripts/sync_gdrive.ps1` (Robocopy). Everything matched by the root `.gitignore` is excluded from both Git and the mirror; keep the script's filters aligned with `.gitignore`. The mirror runs after a successful portable build on Windows or manually.


## 12. Pure Rust Cross-Platform Testing & Docker Protocol

The isolated environment lives in `docker/` and builds only the Rust workspace in `crates/`: no .NET, C++ or Python toolchains are installed in the images (Python 3 is present only to run `build_portable.py`).

- **Layout:** `docker/Dockerfile.rust-linux`, `docker/Dockerfile.rust-macos`, `docker/docker-compose.yml`, `docker/entrypoints/*.sh` (run inside the containers) and `docker/sdk/` (macOS SDK package, never committed).
- **Linux image:** official `rust` image (`RUST_IMAGE`, default `rust:latest`; pin it for reproducible builds) plus `build-essential`, `pkg-config`, `clang`, `libasound2-dev` (cpal), `libgl1-mesa-dev`, `libx11-dev`, `libwayland-dev`, `libvulkan-dev` and a software Vulkan driver (wgpu/egui), `libavcodec-dev`, `libavformat-dev`, `libavutil-dev` and the other FFmpeg libraries (`ffmpeg-next`), and `xvfb`. The FFmpeg development packages must be 7.1.x to use the `ffmpeg` feature; the default build and the tests do not need them.
- **macOS image:** `osxcross` with the Rust targets `x86_64-apple-darwin` and `aarch64-apple-darwin`. The macOS SDK is Apple's property and cannot be redistributed: each contributor supplies the package in `docker/sdk/` (see its README). This service sits behind the compose profile `macos`.
- **Services:** `test-rust-linux` (`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, all inside Xvfb), `build-rust-linux-release` (artifacts in `dist/linux/`, starts only after the tests passed) and `build-rust-macos-release` (artifacts in `dist/macos/`, also depends on the tests). Packaging reuses `build_portable.py --dist`, so the archives and the standalone executable are identical to a local build.
- **Isolation:** the repository is mounted read-only at `/workspace`; Cargo output (`CARGO_TARGET_DIR=/target`) and the registry are named volumes, so host `target/` folders are never mixed with Linux artifacts; the containers need no network access besides crates.io and the package mirrors during the build.
- **What is verified where:** the test suite runs on Linux (container) and natively on the developer's own OS; macOS binaries and test binaries are only compiled in the container because Mach-O executables cannot run on Linux. A green macOS result therefore means "compiles and links", and real macOS runs stay on a Mac.
- **Single command:** `docker compose -f docker/docker-compose.yml up --build`, or `scripts/test_rust_crossplatform.sh` / `scripts/test_rust_crossplatform.ps1`, which also check that Docker is running, parse `dist/reports/test-linux.txt` (`fmt`, `clippy`, `tests` must be `ok`, `failed=0`, `passed>0`), confirm the artifacts exist and print their SHA-256. Options: `--macos`, `--ffmpeg`, `--no-cache`, `--tests-only`.
- **Rules:** the environment must stay lint-clean and reproducible (`--locked`); never copy the macOS SDK, credentials or the `dist/` folder into an image or the repository; a change to a native dependency of a crate must update the Linux package list in `Dockerfile.rust-linux` and the list in section 12 of this file.

---

# Part II — Legacy history

The C# / C++ / Python implementation (`desktop-host/`, `rust-core/`, `cpp-extensions/`, `python-backend/`, the root `Dockerfile` and `docker-compose.yml`) was removed from the tree. Its protocols (Avalonia UI, FFI naming, C++ extensions, Python scripting, AppCache bootstrap, Docker multi-language pipeline) no longer apply; they remain in Git history before commit `36b1d19` as a porting reference only. Nothing in Part I depends on them.
