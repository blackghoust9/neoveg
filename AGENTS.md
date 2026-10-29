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
- **FFmpeg feature toolchain:** FFmpeg **7.1.x shared** build with headers and import libraries (`FFMPEG_DIR`), LLVM / `libclang` **19.x** (`LIBCLANG_PATH`; LLVM 22 or newer breaks `ffmpeg-sys-next` 7.1.3), and the FFmpeg DLLs on `PATH` at run time. Test it with `cargo test --workspace --features neoveg_ui/ffmpeg`. The decoding tests build a clip with the `ffmpeg` command-line tool and skip silently when it is missing. `an_imported_video_reaches_the_preview_texture` (feature `ffmpeg`) takes a clip through the whole preview path (import thread, event on the video track, decoder thread, preview texture); `NEOVEG_TEST_VIDEO=<file>` runs it on a real file, read only.
- **Tests are mandatory for new behavior:** unit tests for pure rules, round-trip tests for file formats (including older-version files), and state-level tests for UI logic. Do not weaken or delete a test to make a change pass.
- **Benchmarks and profiling:** benchmarks belong in Criterion (`benches/`), and Tracy is the profiler for representative timeline and rendering workloads. `neoveg_video/benches/playback_bench.rs` measures the per-frame CPU cost of the video path from 480p to 8K (`cargo bench -p neoveg_video --bench playback_bench`; `python scripts/bench_table.py --readme` rewrites the table of `README.md`); waveform and mixer benchmarks do not exist yet.
- **Memory and thread safety:** sanitizers apply to any future `unsafe`/native code; today the compiler plus Miri-compatible safe code is the guarantee.
- **Release gate:** do not merge to `main` or build a release package until the commands above pass on a clean checkout.

---

## 4. UI / UX Specification

The behavior below is normative. Colors, sizes and thresholds are part of the specification.

### 4.1. Workspace, panels and menus

- **Clean startup:** Neoveg opens in the main workspace with an empty project (0 tracks, 0 media clips) and, over it, the **welcome screen** (`neoveg_ui/src/welcome.rs`, `show_welcome_on_startup = true` by default, turned off in Preferences or by its own checkbox): the recent projects (`Preferences::recent_projects`, `recent_projects_max`, newest first, missing files flagged), *Open project…*, and *New project* with a template, a resolution list, a frame-rate list and a choice of two window layouts (`default_dock`, `audio_dock`). *Help > Welcome screen…* shows it again, and *File > Recent projects* lists the same projects. A short startup screen (`neoveg_ui/src/splash.rs`) covers the window while the startup steps run one per frame (`STARTUP_STEPS` in `app.rs`: preferences and keymap, graphics adapter, audio output probe, effects warm-up, video decoder worker, recovery files); its progress bar follows those real steps, each step stays on screen at least 0.3 s, and any key or click skips the pacing. It never blocks the UI thread, and a new startup task must be added as a step, not as work in `NeovegApp::new`. The screen is a rectangular card drawn with egui shapes (no asset files): see section 30.
- **Themes:** keep visual fidelity with the Classic NLE themes (Classic Foundry, Classic Silver, Modern Carbon Dark, Contemporary Electric Dark) and the three that follow the reference versions' looks: Classic Gray (medium grey windows, early versions), Charcoal Lilac (charcoal panels, near-black lanes, lilac audio, wine selection, recent versions) and High Contrast (black, white and yellow, WCAG AAA). Every built-in theme keeps its text at 4.5:1 or better on the panel (a test checks it).
- **Icon-only buttons:** toolbar and panel buttons show graphical icons only; names, shortcuts and descriptions appear exclusively as hover tooltips. Icons are original flat vector pictograms drawn with egui shapes (`neoveg_ui/src/icons.rs`, `icons::icon_button`), never emoji or font glyphs, so they look the same on every OS; a new icon is a variant of `Icon` plus its drawing, and the tests check that it stays inside its frame. The essential tools (pan/crop `F` frame, event `FX` frame, snapping magnet, auto ripple, split, play / pause / stop) also ship as SVG files in `crates/neoveg_ui/assets/icons/` (24x24 `viewBox`, every color `currentColor`, flat and square: no rounded joins, caps or corners); `icons::SvgIcon` embeds them with `include_str!`, `icons::install` registers the `egui_extras` image loaders (features `svg` and `image`), `icons::paint_svg` / `icons::svg_button` draw them tinted with the theme text color, and `icon_button` uses the SVG of an `Icon` that has one (`svg_for`), falling back to the shape drawing. A new SVG needs an `SvgIcon` variant; the tests check the style rules and that the embedded text equals the file.
- **Floating and undockable panels:** the primary panels (**Timeline**, **Video Preview**, **Trimmer / Project Media** and **Master Bus Mixer**) can be detached into independent floating windows; closing a floating window docks it back into its workspace slot.
- **Main toolbar:** the primary icon toolbar sits directly below the multi-track timeline.
- **Docked panel tabs:** every docked window with tabs (Project Media, Trimmer, Video FX, Audio FX, Transitions, Generators, dockable extensions) uses the compact footer tab pattern: tab strip at the bottom of the window, compact flat headers (font size 11, padding `6,2,6,2`, 22 px high, no selection pipe), a horizontally scrolling strip with left/right arrow buttons (and mouse-wheel scrolling) that appear only when the tabs do not fit, and the selected tab always scrolled into view. Tabs stay undockable by dragging them out of the strip. `egui_dock` cannot place its own strip at the bottom, so grouped panels draw their own footer tabs (`footer_tabs` in `neoveg_ui`).
- **Precision slider** (opacity, volume, pan, master fader; `neoveg_ui/src/precision_slider.rs`): rectangular flat thumb (square corners, 8 px wide, solid fill); double-click resets to the default (`1.0` / `100 %` for opacity and volume, `0.0` for pan); holding `Ctrl` while dragging applies `1/10` sensitivity; the right-click menu offers `Restaurar Padrão` and `Digitar Valor...`.
- **Menus:**
  - **Editar** contains **Preferências**, which holds the interface **Idioma** selector (it is not in Ferramentas or the toolbar).
  - **Arquivo** is grouped with separators: new / open / recent projects / sub-project; save (Salvar, Salvar como, versão incremental); the **Importar** and **Exportar** submenus (media, EDL, XML, FCPXML, OpenTimelineIO, subtitles; Exportar also holds **Renderizar como…**); project properties; Sair. **Ferramentas** is grouped as Inspetor, then the video tools (color correction, key, LUT), then the audio tools (metronome).
  - **Where boxes open** (`neoveg_ui/src/dialogs.rs`): dialogs (preferences, project properties, render, generators, slideshow, shortcuts, credits, repeat paste) open centred and stay draggable; tool windows (inspector, color correction, key, metronome, edit details, notes, logs) open cascaded at the right edge, each new one a slot lower than the tool windows already open (by order of opening, not a fixed slot per window, so a later one never hides an earlier one's title bar). egui remembers the position after the user moves a window.
  - Automatic fade-in/out on media import (audio, video, image) is **off by default** (`auto_fade_on_import = false`, fades `0.0`) and is a checkbox in **Editar → Preferências**.
  - The **Inspector** opens from **Ferramentas → Inspetor**. Do not embed input controls (for example a `ComboBox`) inside menus.
  - Scripting lives in Tools → Scripts and the dockable extension manager in View → Window Extensions, as regular menu items.
- **Event header:** every timeline clip renders an event header with *Generated Media*, *Event Pan/Crop*, *Video/Audio FX* and *Hamburger Menu (☰)* buttons.
- **Event Pan/Crop:** it opens as a pop-up window from the video clip's event header button (it is not docked beside the Preview) and includes an embedded side-by-side live Video Preview toggle next to the `'F'` gizmo canvas.
  - **Built so far** (`neoveg_common::pan_crop`, `neoveg_engine::pan_crop`, `neoveg_video::pan_crop`, `neoveg_ui/src/pan_crop_window.rs`; Ferramentas > Pan/Crop do evento, or the event context menu): `TrackEvent::pan_crop` holds a selection box over the source (centre, size and rotation as fractions of the source, so the same box frames a quarter-size preview and the render) with keyframes measured from the event start and a *smoothness* per key; `EditCommand::SetPanCrop` is one undo step per gesture. The window shows the source frame, the box with eight resize handles, a rotation handle and a move area, the numeric fields (centre X and Y, width, height, rotation), *Lock aspect*, *Size about center*, move free or on one axis, *Stretch to fill*, restore, centre, match the source or the project aspect, the keyframe strip and the side-by-side preview. While a drag is going on the preview shows the draft (`PanCropDraft`) and the project changes on release. The worker draws it on the CPU (bilinear, premultiplied) right after the chroma key and before the effects; the output keeps the source size and the compositor fits it as usual. The rotation is the box's: a box turned clockwise straightens what is inside it, so the picture appears turned counter-clockwise. Not built yet: masks, a rotation centre other than the box centre, flip commands and the Pan/Crop button on the event header.
- **Internationalization:** default language Brazilian Portuguese (`pt-BR`) with runtime support for `en-US`, `de-DE`, `es-ES`, `fr-FR`, `zh-CN`, `ja-JP` and `ru-RU`; resources live in `Assets/i18n/`.

### 4.2. Video Preview

- **Clean preview:** every overlay and on-screen tool (SMPTE HUD, safe areas, grid, the `'F'` pan/crop gizmo, split-screen FX bypass, clip overlay labels) starts **disabled** and is exposed through the overlays menu/flyout or the context menu.
- **Toolbar:** compact, flat, square-cornered, icon-only. It offers project video properties (resolution, FPS, pixel/display aspect); one **Preview Quality** selector with only `Draft`, `Preview`, `Good`, `Best` and `Auto`; project video FX bypass; split-screen; overlays; PNG/JPEG snapshot save and clipboard copy; external-monitor pop-out; and full-screen preview (`Esc` closes). There is no separate resolution selector.
- **Quality implies resolution:** Draft = 1/4, Preview = 1/2, Good and Best = full, Auto = fit to the preview window. The decoder scales frames to the selected width. Draft and Preview are shown with nearest-neighbour sampling so the reduced resolution is visible as pixels instead of a blur; Good, Best and Auto stay smooth.
- **Status strip:** shows live project dimensions, bit depth and frame rate plus the effective preview dimensions, refreshed when project settings, quality or viewport size change.

### 4.3. Timeline

- **Track header indicator:** the dot in the track number box (`• 1`, `• 2`...) blinks **only for the selected track**, alternating white (`#FFFFFF`) and black (`#000000`) every `500 ms`. Unselected tracks keep a static neutral dot; the blink stops when no track is selected. A track becomes selected by clicking its header or any point of its lane.
- **Playhead / edit cursor:**
  - Blue pentagonal head (`#2980B9` / `#3498DB`) with a pointed bottom beak and a horizontal right-extending flag, a `1 px` vertical blue guide line across all tracks, and a timecode box (SMPTE drop-frame `;` or non-drop `:`) next to the marker.
  - The caret blinks (about `500 ms`, alternating opacity `1.0` / `0.0`) while playback is paused, stays solid during playback or scrubbing, and restarts on keyboard frame navigation (`←` / `→`).
  - **Ultra-fine guide line:** exactly one *physical* pixel wide at every zoom level, scroll offset and monitor DPI: stroke width `1 / pixels_per_point`, X centered on a device pixel (`frame_grid::snap_to_device_pixel`). Never scale the line with the zoom.
- **Dynamic frame grid** (`neoveg_ui/src/frame_grid.rs`): density follows the zoom through the ladder `1, 2, 5, 10 frames → 1 s, 2 s, 5 s, 10 s, 15 s, 30 s, 1 min, 2 min, 5 min…`. Minor lines use the smallest step at least `8 px` apart (one line per frame — 1/24, 1/25, 1/30, 1/60 s from the project FPS — at close zoom); major lines use the smallest step at least `90 px` apart and carry the SMPTE ruler labels. Minor lines are `#2C3038`, major lines `#4A505E`; both are one physical pixel, pixel-snapped, drawn over the lane background and behind events, only for the visible range. Grid times derive from integer frame indices (never accumulated floats); second steps land on the frame where the timecode reads `HH:MM:SS;00`, honoring drop-frame (`00:01:00;02` at dropped minute boundaries). Research notes: `docs/research/nle_features_research.txt` (entries 4–5).
- **Scrollbars:** a horizontal bar under the lanes spans the project plus a quarter of the view and drives the same time offset as the wheel; a vertical bar beside the lanes scrolls the track rows under the ruler (the wheel over the track headers does the same). A drag on the thumb moves it, a click on the empty track centres the thumb there. The rules are pure functions in `neoveg_ui/src/timeline_scroll.rs`.
- **Zoom and scrolling:** zoom runs from `15` to `480,000` px/s (10 px per sample at 48 kHz); wheel zoom is anchored at the pointer and toolbar zoom at the edit cursor. Horizontal navigation is one time-based offset shared by the scrollbar, ruler, event canvas and playhead; track headers and lanes share one vertical offset. Event widths use durations (never an absolute x that includes the scroll offset), and off-screen events are culled and clamped to ±10,000 px to protect float precision. Scrollbars are compact, flat and square-cornered, with adjacent zoom-out / zoom-in controls.
- **Event velocity** (`neoveg_engine::velocity`, `TrackEvent::velocity` = a `TrackEnvelope` with the target `EventVelocity`, `EditCommand::SetVelocity`, `neoveg_ui/src/velocity_editor.rs`; section *Velocidade do evento* of the Event scope in the Video Effects tab, and the event context menu *Inserir / Remover envelope de velocidade* and *Congelar quadro no cursor*): the speed of a video event over time, `-100 %` (backwards) to `300 %`, `1.0` normal and `0` a freeze. **The event never changes length**: the envelope only decides how much of the source plays, the position in the source being the integral of the speed (`velocity::source_offset`, midpoint rule between keyframes, exact for lines and steps). Keyframes are in seconds from the start of the event and use the shapes of the other envelopes. The event's own speed (`playback_rate`) multiplies the envelope; running backwards stops at the start of the media and holds its first frame, and running past the end holds the last one (`composite::source_seconds_with`). *Freeze frame at cursor* (`velocity::freeze_at`) keeps the speed up to the cursor, steps to `0` and drops the later keys. Every edit that moves the start of an event together with its in point (left-edge trim, slide, group and rolling trim, keyboard trim, ripple, clipping to a time range, the window of a sub-project) keeps the picture where it is on the timeline: `History::execute` runs each command through `velocity::keep_picture`, which turns such an `EditEvent` into a batch with the matching `SetVelocity` (`shift_start`: the cut-off front takes its share of the media, an extended head plays at the first speed), so one undo step puts both back; a plain move, a slip and a right-edge trim leave the envelope alone. Splitting an event keeps the picture continuous too (`source_offset` for the left half's end, `from_offset` for the right half's envelope). *Congelar quadro no cursor* acts on every selected video event under the cursor, on any track, in one undo step (`velocity::freeze_command`). The preview goes through the composite path while an enabled envelope exists, an envelope being dragged is a draft (`VelocityDraft`, part of the frame key) and a drop is one undo step; the clip request carries the signed speed so the decoder reads ahead the right way. Not built: the envelope drawn over the event on the timeline, the audio of linked events following the speed, reverse presets other than `-100 %`, and velocity in the exports (EDL, FCP XML, OpenTimelineIO write the event at normal speed) and on an event that plays a sub-project (ignored when the sub-project is expanded). Backwards playback seeks for every frame, so media with long groups of pictures will not reverse in real time. After a reverse that ran past the start of the media, a later positive speed first has to climb back above the start (the position is the plain integral): marked *a confirmar* in `docs/research/ui_audit.md`.
- **Resample mode** (`neoveg_engine::ResampleMode`, `TrackEvent::resample`, `neoveg_engine::switches::{set_resample, resample_of}`, `VideoClipRequest::blend`; manual v21 p.355-358): an event switch for video events, in Editar > Interruptores and the event menu, as *Reamostragem*: *Usar configuração do projeto* (today no resampling, as the project setting is not built), *Mistura de quadros* and *Desativar reamostragem*. With the blend on, a moment between two source frames shows the blend of both in proportion to how close each is (`decode_clip_raw`, `mix_frames`); on a frame, or within 2 % of one, nothing is blended, and the last frame has no next one. The composite cache key follows the position in 1/16 of a frame for blended clips, because the plain key rounds to a frame. A clip with the blend on sends the preview through the composite path, locked and adjustment events keep their mode, and `Paste event attributes` copies the mode with the switches. Not built: optical flow (the manual's third mode), the project's resample setting, *Maintain aspect ratio* and *Reduce interlace flicker*.
- **Adjustment events** (`neoveg_engine::adjustment`, `TrackEvent::adjustment`, `TrackCompositeNode::Adjust`, Faixas > Inserir evento de ajuste, `Ctrl+Alt+J`; manual v21 p.281-284): an empty video event (no media, 5 s long when inserted) whose own video effect chain, edited in the Efeitos de Vídeo tab like any event's, runs on the picture of the video tracks **below** its track for as long as it lasts; it has fades, opacity (times the track level), trim, split, copy and paste, Mute and Lock like a video event, while velocity and Pan/Crop are refused for it. `composite_request` puts an `Adjust` node before the layer of its own track, so the track's other clips stay above it; the worker (`composite_with_adjustments`) cuts the node list there: it composites what is below, runs the chain and mixes the result back with the opacity (`mix_frames`), then composites the rest over it. Lists without an adjustment keep the old single pass. The timeline draws it hatched. Not built: adjustment tracks, resizing to the project, transitions and Pan/Crop on it, and the colour-grading panel on it.
- **Video transitions** (`neoveg_video::transitions`, `neoveg_engine::transitions`, `TrackEvent::{transition_in, transition_out}`, `EditCommand::SetTransition`, `neoveg_ui/src/transitions_panel.rs`; dock tab **Transições** beside the effects tabs, `Ctrl+Alt+6`): a transition replaces the plain dissolve of a fade. It runs over the fade-in of the event (or the overlap with the event before it, the automatic crossfade) and over the fade-out, and its **progress is the gain of that fade**, so the length and the curve (five fade curves) are the fade's. Eight transitions: dissolve, linear wipe, clock wipe, iris, barn door, blinds, slide and grow; wipes multiply the alpha by a soft-edged mask, slide and grow move or scale the layer. Progress `0` shows nothing and `1` returns the layer untouched (tests check both for every transition). The tab picks entrance or exit, the transition and its values (a slider commits on release); choosing one on a side with no fade and no overlap gives that side a one-second fade in the same undo step. `clip_look` in `composite_request` turns a fade that has a transition into a progress and keeps the clip opaque; a plain fade stays an opacity, so the fast path is untouched. Not built: thumbnails and presets, favourites, the free-form *Transition Progress* envelope, 3D transitions and transitions between events on different tracks.
- **Track motion** (`TrackMotion` = the same keyframed box as the Pan/Crop, `Track::motion`, `EditCommand::SetTrackMotion`, `neoveg_video::pan_crop::place`; Faixas or Ferramentas > Movimento da trilha): where the whole layer of a video track sits in the project's frame. A smaller box is a picture-in-picture, a moved one a pan, a turned one a tilted layer; what the box does not cover is transparent, so the tracks below show. Key times are timeline seconds. The editor is the Pan/Crop window pointed at a track (no *Stretch to fill*, no source picture: the frame of the project with its thirds, and the preview beside it). `composite_request` evaluates the box at the frame time, and the worker places the layer after the track effects and the fade to color, so the preview and the render agree. Not built: 3D orientation, shadow, glow and the motion of the parent of a composite group.
- **Video track envelopes** (`neoveg_engine::envelope`, `AutomationTarget::{VideoLevel, VideoFade, VideoMute}`, `video_track_state`; Faixas > Envelopes de vídeo, or `V` on a video track): three keyframed curves per video track, drawn and edited in the lane like the audio ones (same hit-testing, shapes and undo). *Level* (`0..1`, `1` opaque) is the track's composite opacity, *fade to color* (`0..1`) moves the layer's colour toward `Track::fade_color` (`EditCommand::SetFadeColor`, a palette in the Efeitos de Vídeo tab, Track scope) and keeps its alpha, and *mute* (`>= 0.5`) hides the track. A video target only lives on a video track and an audio target on an audio track. `composite_request` evaluates them at the frame time, so the preview and the render agree, and a track with an enabled video envelope sends the preview through the composite path. The event velocity and the transition progress envelopes are not built yet.
- **Render status ruler:** a tri-color bar above the timeline: *green* (`#2ECC71`, RAM/prerender at 60 FPS), *yellow* (`#F1C40F`, simple/queued) and *red* (`#E74C3C`, complex/unrendered).
- **Mouse editing of events:**
  - Dragging the body moves the event, also between tracks of the **same kind**; dragging the first or last 7 px (a third of the width on narrow events) trims that edge; dragging a fade handle (top strip, 8 px inside each edge while the fade is zero) sets the fade length.
  - Trimming keeps the source aligned (`trim_in` follows the left edge), never reveals media before the source start or after its end, and keeps at least one frame. Fades are re-clamped to the event.
  - **Snapping** (toggle `F8` or the toolbar magnet; hold `Alt` while dragging to ignore it) aligns to the origin, the playhead, the edges of the time selection and the edges of events on the dragged track within `8 px`; `Ctrl+F8` adds the frame grid, `Shift+F8` markers and regions, `Ctrl+Shift+F8` the events of other tracks (markers and other tracks are on by default). `Shift` while moving inverts snapping for that drag (on edges it ignores the group); `Alt` wins over `Shift`. A yellow line across the lanes marks the aligned point, dashed when it only rounds to a frame. Otherwise starts and trim edges are quantized to frames, unless Quantize to Frames (`Alt+F8`) is off.
  - A drag previews live and changes the timeline only on release, as **one** undo step. `Esc` cancels. The lane area scrolls automatically when the pointer rests within `28 px` of its ends. A read-out shows the start timecode and duration.
  - **Multiple selection:** `Ctrl`+click toggles, `Shift`+click selects the range, `Shift`+drag over empty lane draws a selection rectangle (`Ctrl` keeps the current selection). Dragging one selected event moves the whole selection in time as one undo step, never before zero.
  - **Rolling trim:** `Ctrl`+`Alt` + drag on an edge that touches a neighbouring event moves the cut between them (both events change, the total length does not, each source stays aligned; one undo step; a locked neighbour blocks it; without a touching neighbour it is a plain trim). **Shuffle tool** (cycle `D`): dropping an event among the others of its track reorders a plain sequence (no overlaps, nothing locked), keeping the gap lengths in order.
  - **Group trim:** trimming an edge of a grouped event trims every member whose same edge sits at the same time (the shared edge stops at the most restrictive member); `Shift` while dragging, or Ignore Grouping (`Ctrl+Shift+U`), trims only the dragged event (J and L cuts). Not applied while Auto Ripple is on.
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
- **Video effects** (`neoveg_video::vfx`, `neoveg_video::vfx_gpu`, `neoveg_engine::video_effects`; dock tab **Efeitos de Vídeo** beside Efeitos de Áudio, `Ctrl+Alt+8`): four chains, each ordered and of up to 32 effects: the media pool entry (`Timeline::media_video_effects`, by media id, for every event that plays it), the event (`TrackEvent::video_effects`), the video track (`Track::video_effects`) and the project (`Timeline::video_effects`), all `#[serde(default)]`, each with bypass and parameters; every change is one `EditCommand::SetVideoEffects` undo step and a slider commits on release. The engine stores only the registry id and the values; the registry (names, ranges, defaults) lives in `neoveg_video::vfx`, with the CPU version as the reference and a `wgpu` compute version that has to match it (a test compares them and skips without an adapter). Today: Gaussian blur, Sharpen, Unsharp mask and Vignette. Spatial parameters are fractions of the frame height, so Draft, Best and the render look alike, and neighbourhood effects work on premultiplied colour. Order of the signal: decode, chroma key, **media effects**, **event effects**, event opacity and crossfade, **track effects**, composite, **project effects**, project grade and LUT. While a slider is dragged the preview shows its value live (`VfxDraft`, part of the preview's frame key; the project changes on release). Any clip with an effect on sends the preview through the composite path, the only one that runs the chains. The worker folds the chains into its composite cache key and never changes a cached source frame. `composite_request` builds the chains, so the render uses them too. Sub-projects keep their events' chains; a child project's media, track and project chains are not applied (the same limit as audio).
- **Subtitles** (`neoveg_engine::captions`, tab **Legendas** of the media panel, `neoveg_ui/src/captions_panel.rs`): `Timeline::captions` (`#[serde(default)]`), `EditCommand::ReplaceCaptions` (one undo step for an import or an edit), SubRip and WebVTT reading and writing (adapted from FilmCraft, see section 8), the text drawn over the video preview and File > import / export. They are plain functions in the engine; the panel only returns `CaptionAction`s.
- **Edit decision lists** (`neoveg_engine::edl`): CMX 3600 export of the selected track and import of cuts into the media that is already in the project (matched by source file, clip name or reel), as one `EditCommand::Batch` undo step. Neoveg's own code on the public format; times go through `neoveg_common::timecode::{frame_to_timecode, parse_timecode}`.
- **Playback clock** (`neoveg_audio::clock`, `AudioPlayer::synced_position`): the picture follows the sound at the loudspeaker. Every audio callback publishes an anchor (programme position of the first frame of its buffer, the time of the callback and the device's output latency from `OutputCallbackInfo`) through a lock-free cell (`ClockCell`, a sequence counter around atomics: no lock or allocation in the callback), and the UI extrapolates from it with the wall clock (`heard_position`), wraps it into the loop and clamps it to the programme. `steady` keeps the cursor from stepping back (or forward when playing backwards) by under 40 ms when a new anchor corrects the estimate; bigger jumps (a seek, a loop) pass. `play_from` invalidates the anchor so a seek never reads the old one. Without audio the clock is the wall clock. Frames are asked for at the cursor and the worker keeps only the newest request, so a slow decoder drops intermediate frames instead of falling behind. Not built: leading the request by the decode and draw time, and a dropped-frame counter.
- **Mixer** (`neoveg_audio::mixer`): sums every unmuted audio event active at the playback position, honoring `trim_in`, gain, fades and crossfades, with linear interpolation of the source.
- **Fades:** five curves — `Linear`, `Fast`, `Slow`, `Smooth`, `Sharp` (`neoveg_common::fade::FadeCurve`, `gain(x)`: `x`, `1-(1-x)²`, `x²`, smoothstep, inverse smoothstep). A fade-in uses the curve directly; a fade-out evaluates it at the remaining fraction. Each event stores its own fade-in and fade-out length and curve.
- **Automatic crossfades** (`neoveg_engine::crossfade`, project setting `auto_crossfade`, on by default): events on the same track that overlap *partially* (the later one starts inside the earlier one and ends after it) crossfade over the overlap, using the later event's fade-in curve for the fade-in and its mirror for the earlier event's fade-out, multiplied with each event's own fades. An event lying entirely inside another is not crossfaded.
- **Sub-frame editing:** times are `f64` seconds with sub-frame precision (about `1e-6 s`); editing gestures quantize to frames.

### 4.5. Keyboard shortcuts

| Shortcut | Action | State |
| :--- | :--- | :--- |
| `Ctrl+Q` / `Ctrl+Shift+Q` | Insert audio / video track | implemented |
| `Ctrl+N` / `Ctrl+Shift+N` | New project in a new tab / create a sub-project from the selected events | implemented |
| `J` / `K` / `L` | Scrub: `L` plays forward and each further press goes faster (1x, 2x, 4x, 8x, 16x), `J` the same backwards (the other key turns around), `K` pauses | implemented |
| `Ctrl+R` | Record from the default input device at the cursor; pressed again, stops and puts the take on the timeline | implemented |
| `S` | Split at the cursor: the selected events, or the events under the cursor on every track when nothing is selected; with a time selection, at both of its edges | implemented |
| `Space` | Play / pause | implemented |
| `←` / `→` | Move the cursor one frame | implemented |
| `Delete` | Remove the selected event | implemented |
| `F8` | Toggle snapping | implemented |
| `Ctrl+Z` / `Ctrl+Y` (`Ctrl+Shift+Z`) | Undo / redo | implemented |
| `Ctrl+O` / `Ctrl+S` / `Ctrl+Shift+S` | Open / save / save as | implemented |
| `M` | Marker (opens the inline name box) | implemented |
| `G` / `U` | Group the selected events (a group is selected, moved and split together) / take the selected events out of their group, the others stay grouped (a group left with one member is dissolved) | implemented |
| `Ctrl+U` / `Shift+G` | Dissolve the whole group of the selected events / select every event of their groups | implemented |
| `Alt+Shift+Up` / `Alt+Shift+Down` | Move the track focus one track up / down | implemented |
| `D` / `Shift+D` / `Ctrl+D` | Editing tool: next / previous / normal (Normal, Slip, Slide). With Slip or Slide, dragging the body of an event slips its media or slides the event over its media; the edges keep trimming | implemented |
| `Ctrl+Alt+S` | Save Version: save the project as `name_0001.neoveg`, `name_0002.neoveg`… and go on working in that file | implemented |
| `Ctrl+Alt+T` | Insert Time: open empty time at the cursor on the tracks of the selected events (every track when none is selected), moving the ruler too in the latter case (Neoveg's own binding) | implemented |
| `Ctrl+Alt+M` / `Ctrl+Alt+L` / `Ctrl+Alt+I` | Event switches Mute / Lock / Invert Phase on the selected events (Editar → Interruptores, event context menu; Lock also has a toolbar button). Neoveg's own bindings: the reference manuals list the switches but give them no shortcut | implemented |
| `Ctrl+Alt+J` | Insert an adjustment event on the selected video track at the cursor (Neoveg's own binding: the manual names the Insert menu command without a shortcut) | implemented |
| `Ctrl+Alt+V` / `Ctrl+Alt+Shift+V` | Paste event attributes / selectively (a dialog with one check box per attribute): the first copied event lends its switches, audio pitch handling, video playback rate, video effects, Pan/Crop, velocity envelope and transitions to the selected events, in one undo step (`neoveg_engine::attributes`); position, length, fades, name and media never move, a locked target is skipped. Neoveg's own bindings: the manual lists the commands without a shortcut | implemented |
| `Ctrl+Alt+O` / `Ctrl+Alt+N` / `Ctrl+Alt+Shift+N` | Event switches Loop / Normalize, and Recalculate normalization (Neoveg's own bindings) | implemented |
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
| `F` / `Ctrl+F` / `Ctrl+Shift+F` | Ripple scope: affected tracks / + markers and regions / everything; used by Auto Ripple and ripple delete; they also ripple the last edit (plain delete, edge trim or stretch) while it is still the latest change in the history (move and paste are planned) | partial |
| `[` / `]` | Keyboard trim: select the previous / next edit edge; `←` / `→` then move it one frame (the active edit tool picks trim, slip or slide), `Shift+←` / `Shift+→` stretch it, `Ctrl+Shift+←` / `Ctrl+Shift+→` move the cut shared with the neighbouring event (rolling), `Esc` leaves | implemented |
| `Ctrl+L` | Auto ripple: deleting events closes the gap they leave, and trimming or stretching an edge moves what follows by the change in length (scope from `F` / `Ctrl+F` / `Ctrl+Shift+F`) | implemented |
| `Ctrl+Delete` | Ripple delete: remove the selected events and move the later ones of each track earlier | implemented |
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
| `Alt+0`, `Alt+2` … `Alt+5`, `Ctrl+Alt+2`, `Ctrl+Alt+6`, `Ctrl+Alt+7`, `Ctrl+Alt+8`, `Ctrl+Alt+9` | Show / hide the Timeline, Trimmer, Master Bus, Preview, Project Media, Video Scopes, Loudness, Transitions, Video Effects and Audio Effects windows (in front -> closed, behind a tab -> raised, closed -> back); the others (Explorer, Project Notes, Transitions, Video FX, ...) are planned | implemented |
| `Alt+D`, then 1-9 / `Ctrl+Alt+D`, then 1-9 | Load / save the window layout that has that number (the number key row, not the keypad; the second key must come within 3 s). Chords are commands (`view.layout_load`, `view.layout_save`), so they can be rebound | implemented |
| Exibir → Layouts | Saved window layouts (`neoveg_ui::layouts`, one `.neoveglayout` JSON file each in `<app data>/layouts`, written atomically, `format_version` 1, a newer one is refused): the list with the current one marked, *Salvar layout como…* (name and shortcut number; a number belongs to one layout), *Salvar layout atual*, *Recarregar layout selecionado*, *Organizar layouts…* and the default layout. Dock only: detached windows are not stored. The dock's own rectangles are infinite until laid out, which JSON writes as `null`; reading turns them into zero. The screenshots read no layout folder | implemented |

**How shortcuts are implemented:** every action is a `Command` in `neoveg_common::commands` (stable dotted id, pt-BR label, category, `implemented()` flag); `neoveg_common::keymap` holds the default bindings and the user's overrides (`keymap` in `preferences.json`, only the commands that differ from the defaults, an empty list meaning unbound); `neoveg_ui::shortcuts` converts egui key events. The UI dispatches by command, never by a hard-coded key, and shows shortcut hints in menus from the active map. A new command needs an entry in the `commands!` table, a default in `DEFAULT_BINDINGS` (the tests reject duplicate shortcuts and unparseable defaults) and an arm in `AppState::run_command`; **Editar → Personalizar teclado…** lists it automatically. Commands with `implemented() == false` own their default shortcut but do nothing until the feature lands.

**Source and rules for the planned rows:** they come from the default key map of the reference manuals (research entry 17 in `docs/research/nle_features_research.txt`). Divergences already accepted: Neoveg keeps `←` / `→` for one frame (the reference uses `Alt+←` / `Alt+→` and uses the plain arrows for one pixel when quantize is off) and keeps `Alt` as "ignore snapping" while dragging (the reference uses `Shift` for that and `Alt` for slip). Any shortcut added here must also be listed in the customizable key map (`keymap` in `preferences.json`) so users can rebind it.

---

## 5. Project Files, Autosave & Relink

- **Native format (`.neoveg`):** the exclusive project format. It is a ZIP container with `project.json` (name, settings, timeline), `media_pool.json` and `history.json` (reserved; the undo history is session-only). Neoveg creates, opens, autosaves and manages projects strictly in `.neoveg` (no `.veg` or `.otio` project files). Saves are atomic (temporary file, then rename) and files written by a newer `format_version` are rejected with a clear message.
- **Autosave & recovery:** `neoveg_engine::autosave` writes `recovery_<session>.neoveg` — a normal `.neoveg` plus a `recovery.json` entry — to `neoveg_common::paths::autosave_dir()` from a worker thread, and only while the project has unsaved changes (`History::revision`). The interval is a preference (`autosave_minutes`, default 2, `0` disables). A clean exit, a save, opening another project or creating a new one deletes the session's file; leftovers are offered for recovery at the next start and pruned after 30 days. This replaces the legacy `projects/autosave/` location. One Neoveg instance per user is assumed.
- **Media conversion** (`neoveg_video::convert`, `neoveg_ui/src/app.rs`: `load_message`, `convert_offer_window`, `start_conversion`): when an import fails because the file is not compatible, a dialog offers to convert a copy and import it. Sound files become WAV (PCM 16-bit, 48 kHz), everything else MP4 (H.264 + AAC, even dimensions, `yuv420p`), written by the `ffmpeg` program on the `PATH` on a worker thread to `convertidos/` beside the project (the app data folder for an unsaved one) under a name that never takes an existing file; the original is never touched. The question is not asked for the reload of a project's media, for the result of a conversion (no loop), for a file that no longer exists, or for a container video in a build without the `ffmpeg` feature (converting would not help: `video_decoding_enabled`). The file picker has an *all files* filter. `every_media_format_imports_directly_or_after_the_conversion` makes files in many formats with `ffmpeg` and checks that each imports directly or after the conversion; it skips without `ffmpeg`, skips container videos in a build without the `ffmpeg` feature, and reports formats the local `ffmpeg` cannot write.
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
- **Oldest macOS (`osx-legacy`, `--macos-min`):** `python build_portable.py build --targets osx-legacy` builds the Intel package for the oldest macOS the installed rustc can target (macOS 10.12 Sierra with rustc 1.99; the value is read with `rustc --print deployment-target`), sets `MACOSX_DEPLOYMENT_TARGET` and writes `LSMinimumSystemVersion` into `Info.plist`; `--macos-min X.Y` does the same for any macOS target. A value under the floor is refused before the build starts, never clamped. **Mac OS X Lion (10.7) cannot be built:** rustc does not target anything under 10.12, and Lion predates Metal (10.11), the API `wgpu` uses on macOS. `osx-legacy` is not part of `--targets all`, needs the Rust target and the Apple SDK (a Mac or osxcross) and was only checked here up to the refusal and the package metadata, because the development machine has no macOS target.
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


## 12. Pure Rust Isolated Multi-OS Testing & Docker-OSX Protocol

The isolated environment lives in `docker/` and builds only the Rust workspace in `crates/`: no .NET, C++ or Python toolchains are installed in the images (Python 3 is present only to run `build_portable.py`).

- **Layout:** `docker/Dockerfile.rust-linux`, `docker/Dockerfile.rust-macos`, `docker/docker-compose.yml`, `docker/entrypoints/*.sh` (run inside the containers) and `docker/sdk/` (macOS SDK package, never committed).
- **Linux image:** official `rust` image (`RUST_IMAGE`, default `rust:latest`; pin it for reproducible builds) plus `build-essential`, `pkg-config`, `clang`, `libasound2-dev` (cpal), `libgl1-mesa-dev`, `libx11-dev`, `libwayland-dev`, `libvulkan-dev` and a software Vulkan driver (wgpu/egui), `libavcodec-dev`, `libavformat-dev`, `libavutil-dev` and the other FFmpeg libraries (`ffmpeg-next`), and `xvfb`. The FFmpeg development packages must be 7.1.x to use the `ffmpeg` feature; the default build and the tests do not need them.
- **macOS image:** `osxcross` with the Rust targets `x86_64-apple-darwin` and `aarch64-apple-darwin`. The macOS SDK is Apple's property and cannot be redistributed: each contributor supplies the package in `docker/sdk/` (see its README). This service sits behind the compose profile `macos`.
- **Services:** `test-rust-linux` (`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, all inside Xvfb), `build-rust-linux-release` (artifacts in `dist/linux/`, starts only after the tests passed) and `build-rust-macos-release` (artifacts in `dist/macos/`, also depends on the tests). Packaging reuses `build_portable.py --dist`, so the archives and the standalone executable are identical to a local build.
- **Isolation:** the repository is mounted read-only at `/workspace`; Cargo output (`CARGO_TARGET_DIR=/target`) and the registry are bind mounts under `NEOVEG_ENV_DIR`, so host `target/` folders are never mixed with Linux artifacts; the containers need no network access besides crates.io and the package mirrors during the build.
- **What is verified where:** the test suite runs on Linux (container) and natively on the developer's own OS; macOS binaries and test binaries are only compiled in the container because Mach-O executables cannot run on Linux. A green macOS result therefore means "compiles and links", and real macOS runs stay on a Mac.
- **Single command:** `docker compose -f docker/docker-compose.yml up --build`, or `scripts/test_rust_crossplatform.sh` / `scripts/test_rust_crossplatform.ps1`, which also check that Docker is running, parse `dist/reports/test-linux.txt` (`fmt`, `clippy`, `tests` must be `ok`, `failed=0`, `passed>0`), confirm the artifacts exist and print their SHA-256. Options: `--macos`, `--ffmpeg`, `--no-cache`, `--tests-only`.
- **Environment drive (`D:\ambiente_neoveg`):** every heavy I/O location lives there instead of drive C: `target\` (`CARGO_TARGET_DIR`, honored by `build_portable.py`), `docker\` (Cargo registry and the per-service target directories, bind-mounted by `docker-compose.yml` through `NEOVEG_ENV_DIR`), `docker-osx\mac_hdd.img` (the macOS guest disk) and `dist\` (artifacts and reports, `NEOVEG_DIST`). `scripts/test_rust_crossplatform.ps1` creates the folders and sets these variables (override `NEOVEG_ENV_DIR`; on Linux/macOS hosts the default is `.neoveg_env/` inside the checkout); `.env.example` documents them. The path is the maintainer's Windows setup and must never be hardcoded in Rust code. Moving Docker Desktop's own WSL2 disk image to D: is a Docker Desktop setting (Settings > Resources > Advanced > Disk image location), not something the repository can change.
- **Native macOS (Docker-OSX):** service `docker-osx` (`sickcodes/docker-osx`, profile `macos-native`) boots macOS under QEMU/KVM with the disk `mac_hdd.img`; `test-rust-macos-native` (`docker/entrypoints/test-macos-native.sh`) copies the sources into the guest over SSH, runs `cargo test --workspace` and `cargo build --release -p neoveg_ui` natively and writes `reports/test-macos-native.txt` and `macos-native/neoveg_macos_<arch>` into `dist`. It needs `/dev/kvm`, a prepared disk image and Apple-licence compliance (`docker/osx/README.md`); run it with `scripts/test_rust_crossplatform.ps1 -MacosNative`. The osxcross service stays the way to produce the release archives.
- **Rules:** the environment must stay lint-clean and reproducible (`--locked`); never copy the macOS SDK, credentials or the `dist/` folder into an image or the repository; a change to a native dependency of a crate must update the Linux package list in `Dockerfile.rust-linux` and the list in section 12 of this file.

---

## 24. Multi-Project Tabs & Nested Sub-Projects Architecture

Several projects can be open at once, one tab each, and a project can play another project as a single clip (a nested sequence, "sub-project").

- **Engine (`neoveg_engine::project_manager`).** `ProjectManager` keeps every open `Project` (`ProjectId`, `ProjectInfo`, `Timeline`, `History`, path, `saved_revision`) in tab order plus the active id: `register`, `open_project(path)` (returns the tab of a file that is already open), `close_project(id)` (the neighbour tab becomes active), `switch_active_project(id)`, `find_by_path`, `take` / `put`, and `create_subproject_from_events(...)`. The application edits the active project in its own state (`AppState.project`, `timeline`, `history`, ...) and the manager holds the others: a tab switch stashes the live project with `put` and brings the other back with `take`, so every editing function keeps working on `self.timeline` unchanged. Pure rules and unit tests live in the engine; the UI only calls them.
- **Media kinds (`neoveg_common::project`).** A media pool entry is `MediaSource::File(path)` or `MediaSource::SubProject(path)`; `MediaRef::source()` reads it from the persisted `subproject: bool` flag (`#[serde(default)]`, not written for plain files, so older projects load). A sub-project is a normal `.neoveg` file; paths are absolute and relative ones are taken from the parent's folder.
- **Circular dependencies.** `validate_no_cycle(parent, child)` follows the `SubProject` entries of the project files (depth-first, canonical paths) and refuses a nesting that reaches the parent again or goes deeper than `MAX_NESTING` (8 levels); `create_subproject_from_events` checks it before writing, and `open_project` refuses a project that reaches itself. Playback expansion also stops at `MAX_NESTING`, so a broken file can never loop.
- **Creating a sub-project** (`Ctrl+Shift+N`, command `file.create_subproject`, Edit/event context menu "Criar Sub-projeto a partir da seleção"): the selected events are copied (media pool entries included) into a new project, moved so the earliest one starts at zero, saved under `<project folder>/subprojetos/` (the app data folder for an unsaved project) and opened in a tab; in the parent the selection is replaced, as one undo step (`EditCommand::ReplaceEvents`), by one event that plays it (`assign_event_ids` gives it its id). `Ctrl+Shift+N` replaced the unbound "new empty project without dialog".
- **Playback (`flatten_subprojects`).** Before audio or video is built, every event that plays a sub-project is expanded: the child's events inside the window the event shows (`trim_in` .. `trim_in + duration`) are copied onto new tracks of a flattened timeline at the parent's time (child fades and trims kept; the child's track effects, fader and pan are not applied), and the event itself is dropped. The media of the child get stable ids from the hash of their path (`nested_media_id`, top bit set, so they never collide with a pool id) and are decoded on the worker like any media (`AppState::flattened()` caches the result until the project, a tab or a saved file changes). The audio mixer (`build_clips`) and the video preview (`request_preview_frame`) read the flattened view; a sub-project open in another tab is played as it is being edited.
- **Interface (`neoveg_ui/src/app/tabs.rs`).** A tab strip under the menu shows the open projects (`name`, `*` when dirty, `x` to close, `+` for a new one); closing a dirty tab asks to save, discard or cancel; the last tab is replaced by an empty project. File > New (`Ctrl+N`) opens an empty project in a new tab, File > "Abrir em nova aba…" opens a file in one. **Double-clicking an event that plays a sub-project** (or its context-menu entry) opens that project in a tab (the existing tab if it is already open).
- **Known limits.** Closing the whole window only checks the active project for unsaved changes; the layout of the dock (`egui_dock`) is shared by all tabs; autosave and recovery cover the active project only.

---

## 26. Obsidian Architecture Mapping & Documentation Protocol

The vault `docs_obsidian/` holds the visual window planning, the module documentation and the user manual. It is part of the repository; the maintainer's working copy is mirrored to `D:\ambiente_neoveg\docs_obsidian` (see section 12 for the environment drive).

- **Referências de interface (`ui_references/`):** notas por painel que cruzam os manuais, a varredura `VegasScan/` e as imagens extraídas de `results_assets/` (só descrições e medidas; protocolo na seção 37).
- **Referência funcional (`05 - Referencia Funcional/`):** o vault é o segundo cérebro do projeto. Ali ficam notas por tema (efeitos, transições, geradores, menus) escritas em palavras próprias a partir da varredura local `VegasScan/` (ignorada pelo Git, nunca versionada). Decisão do mantenedor de 2026-10-28; sem textos de menu, strings, capturas, ícones nem nomes de marca, e o comportamento continua vindo dos manuais (§33).
- **Layout:** `00 - Mapa Geral/` (hub `Index.md`), `01 - Arquitetura Rust/` (one note per crate, named after it), `02 - UI e Planejamento de Janelas/` (`layout_janelas.canvas`, `especificacao_janelas.md`, `inventario_icones.md`), `03 - Manual do Usuario/` (`guia_de_inicio.md`, `atalhos_teclado.md`, `subprojetos_e_abas.md`), `04 - Pesquisas e Benchmarks/` (`regras_de_timeline.md`, `desempenho_resolucoes.md`).
- **Links:** notes connect with `[[wikilinks]]`; every note links back to `[[Index]]`. A new crate or window needs its note and a link from the hub in the same change.
- **Canvas:** `layout_janelas.canvas` mirrors `default_dock()` in `neoveg_ui/src/app.rs`. Changing the default dock, a window title or a proportion updates the canvas and `especificacao_janelas.md` in the same commit.
- **Generated blocks:** the pages of `03` and `04` hold blocks between `<!-- auto:begin -->` and `<!-- auto:end -->` written by `python scripts/update_obsidian_docs.py` (Python standard library; `pypdf` is optional, for page counts). They come from `commands.rs` and `keymap.rs` (shortcut table), `AGENTS.md` section 24, `docs/research/nle_features_research.txt` (timeline rules), the reference-manual folder (`--pdfs` or `NEOVEG_PDF_DIR`) and the results folder (`--results`). Text outside the markers is hand-written and never touched; do not edit inside them. `--mirror [DIR]` copies the vault to the environment drive and is skipped, with a warning, when that drive is absent. Run the script after changing a command, a default shortcut or a research entry.
- **Clean-room:** the vault carries descriptions, measures and page references only; no text, screenshots or icons copied from the reference manuals or from the reference product. Rendered manual pages stay outside the repository.
- **Git:** `docs_obsidian/` is tracked; `docs_obsidian/.obsidian/workspace.json` and `workspace-mobile.json` are ignored (per-machine state). The rest of `.obsidian/` (`app.json`, `graph.json`) is shared.

---

## 27. Background Task Execution & Headless UI Screenshot Generation Protocol

The screenshots in `README.md` (`docs/screenshots/*.png`) are produced by the program itself, offscreen, and heavy tasks can run in a background job.

- **Offscreen renderer (`neoveg_ui/src/app/headless.rs`):** `neoveg --screenshots <dir>` draws the workspace with egui into a `wgpu` texture (`Rgba8Unorm`, an adapter requested with no surface; a software adapter is the fallback) and reads it back to PNG. There is no window and no screen capture; if no adapter exists the run fails with a clear message and must never fall back to opening the real window. The frame is `NeovegApp::show(ctx)` (the part of `update` after the startup screen), so the images show the real interface. The run skips the startup screen, the preferences, the audio probe, the decoder worker and the recovery files, so nothing of the user's is read or written; the demonstration projects are built in memory, and the only file it writes besides the PNGs is a sub-project in a temporary folder that is removed afterwards.
- **Stack:** the ten demonstration projects live in one function and a debug build gives each its own stack slot, so the run happens on a thread with a 256 MB stack (`main.rs`); without it the tenth capture overflowed the 1 MB stack of the Windows main thread.
- **Determinism:** fixed screen size, `pixels_per_point = 1.0`, a fixed UI time (the blinking caret and selected-track dot are in their visible half), nine warm-up frames (fonts, SVG icons and the dock settle) and every frame's texture changes applied. The outputs are `ui_main_layout.png`, `ui_timeline_events.png`, `ui_multi_project_tabs.png`, `ui_icon_set.png`, `ui_welcome_screen.png`, `ui_project_properties.png` and `ui_edit_details.png` (one cell per `SvgIcon`, so it follows the library automatically).
- **Test (`neoveg_ui/tests/headless_screenshot.rs`):** it starts the binary and checks that the four PNGs exist, have the expected size and are not blank. It runs only with `NEOVEG_SCREENSHOTS=1` (optionally `NEOVEG_SCREENSHOT_DIR`); otherwise it prints that it was skipped, because it needs a graphics adapter and rewrites tracked files, and the Docker services mount the repository read-only (section 12). With the variable set and no adapter it fails.
- **Regenerate** the PNGs, and commit them with the change, whenever the default dock, a panel's look, the icon library or the demonstration content changes. The gallery text in `README.md` describes what the images show (crossfade, envelope, switches, tabs, sub-project); keep both in step.
- **Real preview frame:** the preview of the main layout shows the frame at the cursor of `neoveg_video/tests/assets/sample_video.mp4` (a 1080p `testsrc` clip, 15 s, made with `ffmpeg`), decoded with the `ffmpeg` program (so the test needs neither the `ffmpeg` feature nor FFmpeg development libraries), turned into the preview texture and its scopes. Without the file or the program the preview stays black and the test skips its colour check.
- **Content rules:** only synthetic or generated media, no personal project, path or name; nothing taken from the reference manuals or the reference product (section 28).
- **Background job (`scripts/run_background_agent.ps1`, Windows PowerShell 5.1):** `Start-Job` named `NeovegBackgroundPipeline` runs, in the repository root, the screenshot test, `cargo bench --bench playback_bench` and `scripts/generate_icon_formats.py`; each step reports ok, failed or skipped, one failing step does not stop the others, and `-Status`, `-Wait` and `-Stop` use `Get-Job`, `Receive-Job` and `Remove-Job`. `CARGO_TARGET_DIR` goes to `D:\ambiente_neoveg\target` only when that drive exists. A job lives as long as the PowerShell session that started it.
- **Benchmarks:** performance figures in `README.md` must come from a benchmark that exists in the repository and ran on a stated machine. The table between the `<!-- bench:begin -->` markers is written by `scripts/bench_table.py --readme` from `target/criterion` after `cargo bench -p neoveg_video --bench playback_bench`; it states what is measured (CPU work on one decoded frame: colour correction, scopes, crossfade) and what is not (decoding, GPU, audio). Re-run it, on the same machine class, when a stage changes.

---

## 28. Copyright, IP & License Compliance Protocol (Clean Room Standard)

Neoveg enforces strict **Clean-Room Engineering**, intellectual-property isolation, and open-source license governance across source code, UI assets, documentation, and binary distributions:

1. **Isolation & Purge of Proprietary Binary Assets:**
   - **Never commit** `.ico`, `.bmp`, `.dll`, `.exe`, `.veg`, PE/BAML resource dumps, or bitmap images extracted from commercial software (`results_assets/`, `icons 22/`, `G:/*`).
   - **Exception:** `crates/neoveg_ui/assets/icons_color/*.bmp` are the optional colour icons (32-bit BMP with alpha) derived from the MIT-licensed Flat Color Icons (`third_party/flat-color-icons/`, `scripts/fetch_color_icons.py`); they are tracked, off for the High Contrast theme, and switched in Editar → Preferências.
   - `.gitignore` blocks `results_assets/`, `icons 22/`, `*.ico`, `*.bmp`, `*.dll`, `*.exe`, and `G:/*`.
   - If proprietary dumps ever entered historical Git commits, rewrite history on a coordinated maintenance window using `git-filter-repo` (preferred) or `bfg`:
     ```bash
     # Option A — git-filter-repo (recommended)
     git filter-repo --invert-paths \
       --path results_assets/ \
       --path-glob '*.ico' \
       --path-glob '*.bmp' \
       --path-glob '*.dll' \
       --path-glob '*.exe' \
       --path-glob '*.veg'

     # Option B — BFG Repo-Cleaner
     bfg --delete-folders results_assets --delete-files '*.{ico,bmp,dll,exe,veg}'
     git reflog expire --expire=now --all && git gc --prune=now --aggressive
     ```

2. **100% Original Clean-Room Vector Artwork (`crates/neoveg_ui/assets/icons/`):**
   - Every UI icon in `crates/neoveg_ui/assets/icons/*.svg` must be authored from scratch in pure geometric SVG primitives (`<path>`, `<rect>`, `<circle>`), `viewBox="0 0 24 24"`, `currentColor`, with flat `butt`/`miter` strokes.
   - Pixel tracing, embedded bitmaps (`<image>`), `<text>`, and hardcoded hex/RGB colors are banned and rejected automatically by unit tests in `crates/neoveg_ui/src/icons.rs`.
   - File names use universal NLE functional terminology (`split.svg`, `snapping.svg`, `pan_crop.svg`, `event_fx.svg`, `mute.svg`, `solo.svg`, etc.). Secondary raster exports (`svg/`, `png/`, 32-bit alpha `bmp/`, `#1e1e1e` `jpg/`) live outside Git in `D:\ambiente_neoveg\assets\icons\` via `scripts/generate_icon_formats.py`.

3. **Trademark Neutrality:**
   - Rust crates (`crates/`), UI strings (`Assets/i18n/`), and the Obsidian vault (`docs_obsidian/`) must never use third-party registered NLE brand names or trademarks; the software is exclusively named **Neoveg** (`.neoveg`).

4. **Dual Licensing & `cargo-deny` Governance:**
   - All Neoveg workspace crates are dual-licensed under `MIT OR Apache-2.0`, backed by root `LICENSE-MIT` and `LICENSE-APACHE` (plus `LICENSE`, `AUTHORS.md`, and `third_party/README.md`).
   - `deny.toml` (`cargo deny check licenses bans advisories sources`) enforces the approved SPDX allowlist and forbids viral/network-copyleft licenses (`AGPL-3.0*`, `GPL-2.0*`, `GPL-3.0*`).

5. **FFmpeg LGPL v2.1+ Shared Dynamic Linking (`crates/neoveg_video`):**
   - `ffmpeg-next` is optional (`--features ffmpeg`) with `default-features = false` and only `codec`, `format`, `software-scaling`, and `software-resampling` enabled — `static`, `build`, `build-license-gpl`, and `build-license-nonfree` are disabled, and `crates/neoveg_video/build.rs` rejects `FFMPEG_STATIC=1` at compile time.
   - Portable distributions with video enabled link dynamically against **LGPL v2.1+** shared libraries (`.dll` / `.so` / `.dylib`) placed beside the executable together with FFmpeg's LGPL license notice, preserving user relinking rights and codec compliance.

---

## 30. Dynamic Splash Screen & App Branding Protocol

- **The card (`neoveg_ui/src/splash.rs`):** a rectangular card in the middle of the window. Left: the title `NEOVEG`, the program version (`CARGO_PKG_VERSION`, the workspace version) and the date of the last update (the build date, `NEOVEG_BUILD_DATE`, written as `dd/mm/aaaa` by `neoveg_ui/build.rs`, reproducible through `SOURCE_DATE_EPOCH`). Right: the sections *SISTEMA OPERACIONAL*, *CONFIG. DA MÁQUINA* (`CPU`, `MEM`, `GPU`) and *USUÁRIO*. Footer: the copyright line and the loading progress bar with the current step. The pure parts (`sections`, `card_rect`, `Pacing`) are unit-tested; the progress is the real startup steps (`STARTUP_STEPS`), never a timer.
- **No logo inside the card.** The "Nv" mark is the icon of the program in the system, not a picture on the splash screen.
- **Machine facts (`neoveg_common::sys_info`):** `sysinfo` (system part only) and `whoami` give the operating system with its word size (`Windows 11 Pro 64-bit`), the processor model without the legal marks and the clock (`Intel Core i9-13900K`), the installed memory in whole gigabytes (`64 GB`) and the user name. They are collected on a worker thread started with the splash (never on the UI thread), the card reads `Detectando…` until they arrive, anything unknown reads `Desconhecido`, and nothing is sent anywhere or logged. The graphics adapter is what `wgpu` reports once the window exists (`NeovegApp::gpu`), passed to the splash.
- **The program icon:** `assets/app_icon/app_icon.png` (512 px, the window icon, loaded with `eframe::icon_data::from_png_bytes` in `main.rs`) and `app_icon.ico` (16 to 256 px, embedded in the Windows executable by `winres` in `neoveg_ui/build.rs`, so Explorer, the task bar and alt-tab show it). A missing Windows resource compiler is a build warning, not an error. Both files are drawn from plain polygons by `scripts/generate_app_icon.py` (Pillow), original artwork under the project licence; regenerate them there, never edit them by hand.
- **Order of the startup work:** a new startup task is a new step of `STARTUP_STEPS`, run one per frame; nothing heavy goes in `NeovegApp::new`.

---

## 32. Multi-Monitor & Multi-Viewport Docking Protocol

- **Panels in their own window (`neoveg_ui/src/app/detached.rs`).** A panel dragged out of the dock becomes a floating `egui_dock` window; on the next frame `promote_floating_windows` removes that surface and opens the panel as a native operating-system window (an egui viewport, `show_viewport_immediate`), which can be moved to any monitor. Closing the window (or a backend without native windows, `ViewportClass::Embedded`) docks the panel back (`redock`). The detached panels use the same `Viewer` as the dock, so they are the same code and the same state; the keyboard shortcuts also work in them (`handle_shortcuts` per viewport). `F11` toggles full screen on a detached window and `Esc` leaves it. Choosing a detached panel in the Exibir menu focuses its window; recalling a layout that contains it docks it again.
- **Saved windows.** `Preferences::detached_windows` (`#[serde(default)]`, older files load) holds, per panel, a stable id (`Tab::id`) and a `WindowGeometry` (position, size, maximized, size of its monitor, pixel density) from `neoveg_common::window_layout`. It is written when a panel is detached or docked and when the program exits (`eframe::App::on_exit`), and restored on the first frame after the startup screen.
- **Position safety (`WindowGeometry::sanitized`).** Non-finite values fall back to defaults, windows keep a minimum size and never exceed the monitor they were saved on, and a position farther than three monitor sizes from the main monitor is pulled back. A window on a monitor left of or above the main one (negative coordinates) is kept. egui 0.29 cannot enumerate monitors, so this is a heuristic; the operating system still places a window whose monitor is gone on a visible one.
- **DPI.** The pixel density is per viewport (the windowing layer reports it for the monitor the window is on), and it is stored with the geometry for information; dragging a window between monitors of different density rescales it without help from Neoveg code.
- **Known limits.** Not verified on a machine with several monitors (the development machine has one); the pure parts are unit-tested and the promotion / redock / restore logic has state-level tests. Layout slots (`SaveLayout` / `RecallLayout`) store the dock only, not the detached windows. Panels opened as dialogs (`egui::Window`) are not detached.

---

## 33. Documentation-Driven Development Protocol

- **The manuals are the only source** for UI, UX and features. Look first in `manuais/` inside the repository (git-ignored, local) and, when it is missing there, in `G:\Meu Drive\vp-rev-eng\teste\teste2\manuais`. Search the extracted text of the PDFs before designing a window, a menu, a shortcut or a behaviour.
- **Zero hallucination.** Do not add a feature, menu item, dialog or shortcut from memory of other editors. If the manuals do not describe it, it is not built; if it is a deliberate decision of the project, it is recorded in `docs/research/ui_audit.md` with the reason.
- **Clean room stays.** The manuals describe behaviour; their text, screenshots and icons are not copied (section 9.1 and the memory notes on reverse engineering still apply).
- **Audit.** `docs/research/ui_audit.md` lists what in `neoveg_ui` has support in the manuals and what is Neoveg's own decision. Anything marked "a confirmar" is checked against the PDFs before it is changed or removed; nothing is mass-deleted.

---

## 29. Workspace Hygiene & Dependency Audit Protocol

- **Junk files.** `*.log`, `*.tmp`, `*.bak`, `Thumbs.db` and `.DS_Store` never belong in the repository (the root `.gitignore` covers them); a cleanup deletes the ones lying around in the working tree (`logs/`, `crash.log`, `dist/**/logs`), but never `target/` content other than through `cargo clean`, and never the local-only `manuais/` and `results_assets/`.
- **Assets.** The SVG pictograms live only in `crates/neoveg_ui/assets/icons/` (section 4.1). `assets/app_icon/` (the "Nv" icon, section 30) and `docs/screenshots/` are single, generated copies and stay; no other image is tracked.
- **Dependencies.** Every crate uses the workspace `version` and `edition = "2021"`. Each dependency declared in a `Cargo.toml` must be used by that crate's code, and one crate is kept on one version across the workspace (for example `pollster` 0.4). A dependency change is checked with `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`, and `Cargo.lock` is committed with it.
- **Cleanup routine.** Delete the junk files, `cargo clean`, `cargo fmt --all`, `cargo clippy --workspace --all-targets --fix --allow-dirty -- -W clippy::all` (review the diff before keeping it), audit the manifests as above, `git status`, `git gc --prune=now --aggressive`, then rebuild and run the quality gates of section 3 before the commit.

### 29.1. Repository Sanitation & Maintenance Protocol

The numbered steps of a full maintenance pass, in order. Each one states what it may touch and what it must leave alone.

1. **Dead files.** Delete `*.log`, `*.tmp`, `*.bak`, `Thumbs.db` and `.DS_Store` found in the working tree (`logs/`, legacy build leftovers such as `cpp-extensions/build/**/Testing`). Never delete `target/` content other than with `cargo clean`, the local-only `manuais/` and `results_assets/`, the Google Drive sync folders (`.tmp.drivedownload/`, `.tmp.driveupload/`) or untracked folders you did not create (list them and ask).
2. **Build output.** `cargo clean` empties `target/` (several GB on a full debug and release history). Prefer `CARGO_INCREMENTAL=0` for maintenance builds so the cache does not grow again, and check the free space of the drive before a full rebuild.
3. **Assets.** SVG pictograms exist only in `crates/neoveg_ui/assets/icons/`; the only other tracked images are `assets/app_icon/` and `docs/screenshots/`. Icons whose files are identical but whose names are registered separately in `icons.rs` (`mute`/`mute_track`, `solo`/`solo_track`) are not duplicates to merge: merging them changes the icon registry and its tests.
4. **Format and lints.** `cargo fmt --all`, then `cargo clippy --workspace --all-targets --fix --allow-dirty -- -W clippy::all`; read the resulting diff before keeping it. Clippy runs with `--all-targets` so test code is fixed too.
5. **Dependencies.** Every crate keeps `version.workspace = true` and `edition.workspace = true` (Rust 2021). A dependency is removed only when no source file of that crate names it; confirm with a build of the crate. Versions pulled in by other crates (for example `pollster 0.3` through `eframe`) are reported, not forced. Outdated crates are listed with `cargo update --dry-run`; patch updates are applied only together with the quality gates, and `Cargo.lock` is committed with any manifest change.
6. **Git.** `git status`, then `git gc --prune=now --aggressive`. Commit only files the pass changed; leave per-machine state such as `.obsidian/workspace.json` and untracked local folders out.
7. **Gates and commit.** Run the commands of section 3 (`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`) and then commit on `refactor/100-percent-rust` with a `chore(repo): ...` subject in Portuguese without accents.

---

## 37. Extended Reference & Asset Scanning Protocol

The visual and structural study of the interface uses three local reference folders together. All three are git-ignored and never committed (section 28.1); the conclusions live in `docs_obsidian/ui_references/` (section 26).

- **Where they are.** `manuais/`, `VegasScan/` and `results_assets/` in the repository root, with `G:\Meu Drive\vp-rev-eng\teste\teste2\` as the fallback. A missing folder is reported and skipped, never recreated.
- **What each source is for.**
  - `manuais/` (the official PDFs): behaviour. Shortcuts, tools, toggle and hover states, workflows. It is the source of truth for what a feature does (section 33).
  - `VegasScan/` (static scan of the installed files): structure. Menu and panel hierarchy, control names, the size in dialog units of the few titled dialogs, the size of the bitmap strips. It carries **no** panel positions, spacing, colours or default shortcuts (its own README says so).
  - `results_assets/` (extracted resource images, named by resource number): **measures only**. Cell sizes (16 px at 100 %, 20 / 32 / 64 at larger scales), strip sizes, counts. A resource number does not say which button uses it.
- **What may be derived.** Descriptions in the project's own words, measures (pixels, dialog units, counts), the hierarchy of widgets, function names, shortcut facts with their manual page, and a map from each reference *function* to the Neoveg icon file that serves it (`mapa_de_icones_por_funcao`). Manual pages found by title search are labelled as candidates until someone opens the manual.
- **What is never done.** No image, icon, bitmap, dialog or palette from `results_assets/`, the scan or the manuals is copied, converted, traced, recoloured or embedded into `crates/`, `assets/`, `docs/` or the vault, and the `.bmp` exception of section 28.1 covers only `crates/neoveg_ui/assets/icons_color/` (Flat Color Icons, MIT). Neoveg icons stay original (section 28.2). A request to bring those images in is declined; the nearest allowed result is the function map above.
- **Layout metrics.** `neoveg_ui/src/theme.rs` holds colours only. An egui metric (button size, spacing, panel size) changes only when one of the sources gives a number for it, the change cites the note that holds the measure, and a screenshot (section 27) shows the result. Today the only usable number is the 16 px icon cell, and the 26 px button already draws a glyph of about 16.6 px, so nothing changed.
- **Evidence.** Each statement in a note says whether it comes from a manual (with page), from the scan, or from a measure; inference is marked as such. Where the sources disagree, the manual wins for behaviour and the scan wins over memory for structure. A deliberate project decision goes to `docs/research/ui_audit.md` with its reason.
- **Parity matrix.** `scripts/parity_matrix.py` assigns every command of the local scan to one hand-written row (function in the project's own words, state in Neoveg, priority) and writes `docs_obsidian/ui_references/matriz_de_paridade.md` plus `docs/research/parity_matrix.json`. The scan text is never written out. Finishing a function means changing its row in the script and regenerating; `--check` fails when a scan command has no row.
- **Upkeep.** A change to a panel updates its note in the same commit; a new shortcut is checked against `validacao_de_atalhos` before it is bound (section 4.5).

---

# Part II — Legacy history

The C# / C++ / Python implementation (`desktop-host/`, `rust-core/`, `cpp-extensions/`, `python-backend/`, the root `Dockerfile` and `docker-compose.yml`) was removed from the tree. Its protocols (Avalonia UI, FFI naming, C++ extensions, Python scripting, AppCache bootstrap, Docker multi-language pipeline) no longer apply; they remain in Git history before commit `36b1d19` as a porting reference only. Nothing in Part I depends on them.
