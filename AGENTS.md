## Project Naming Conventions & Rules (Diretrizes Estritas de Padronização de Nomes)

Strict adherence to the project naming convention is mandatory across all codebases, configuration files, directories, and documentation (A adesão estrita à convenção de nomes é obrigatória em todo o código-fonte, configurações, diretórios e documentação):

- **Official Name (Nome Oficial):** `Neoveg` (for C# namespaces, `.sln`, `.csproj`, UI branding, and titles) / `neoveg` (for files, folders, Rust crates, C++ CMake targets, and Python modules).
- **Prohibited Suffixes (Sufixos Proibidos):** Do **NOT** use `Studio` or `Pro` in names (use strictly `Neoveg` / `neoveg`).
- **C# / .NET:** All namespaces, solutions, and assemblies must use `Neoveg` (e.g., `Neoveg.sln`, `desktop-host/Neoveg.csproj`, `namespace Neoveg.Core`, `namespace Neoveg.Controls`, `namespace Neoveg.Views`, `namespace Neoveg.ProjectInterchange`, `namespace Neoveg.Scripting`).
- **Rust Backend:** Crate name is `neoveg_core` (`rust-core/Cargo.toml`), FFI symbols prefixed with `neoveg_`.
- **C++ Extensions:** CMake project and shared library target name is `neoveg_cpp_engine` (`cpp-extensions/CMakeLists.txt`, `cpp-extensions/src/neoveg_cpp_engine.cpp`).
- **Python Scripts & Backend:** Module and package imports must use `neoveg` (`python-backend/neoveg/__init__.py`, `python-backend/neoveg_integration.py`).

# AGENTS.md — AI Agent Development Guidelines & Standards

This document defines the architectural standards, naming conventions, coding guidelines, and development rules for the **Neoveg** project. All AI agents (including Antigravity) and human contributors must strictly follow these instructions when modifying or extending the repository.

---

## 1. Strict Naming Conventions & Branding Rules

- **Official Project Name:** Use exclusively **`Neoveg`** (for C#, UI, namespaces, and documentation) and **`neoveg`** (for folders, files, Rust crates, C++ CMake targets, and Python packages).
- **Prohibited Suffixes:** Do **NOT** use suffixes such as `Studio` or `Pro` (the project name is solely and strictly **Neoveg**).
- **C# / .NET:** All namespaces, solutions (`Neoveg.sln`), and `.csproj` files (`desktop-host/Neoveg.csproj`) must use the `Neoveg` prefix (e.g., `namespace Neoveg.Core`, `namespace Neoveg.Views`, `namespace Neoveg.Controls`).
- **Rust Backend:** The main crate must be named `neoveg_core`, with all FFI-exported functions using the `neoveg_` prefix.
- **C++ Extensions:** The CMake build target must be defined as `neoveg_cpp_engine` inside the `cpp-extensions/` folder (`cpp-extensions/src/neoveg_cpp_engine.cpp`).
- **Python Backend & Scripts:** Modules and packages must use the `neoveg` namespace (`python-backend/neoveg/`).

---

## 2. Multi-Language Architecture Overview

```
Neoveg Workspace
├── Neoveg.sln            # Root Visual Studio / .NET Solution
├── desktop-host/         # C# (.NET / Avalonia UI) — Frontend UI and MVVM (Neoveg.csproj)
├── desktop-host.Tests/   # C# (Avalonia.Headless.XUnit) — Automated Headless UI & Unit Tests (Neoveg.Tests.csproj)
├── rust-core/            # Rust (neoveg_core) — High-performance video/audio processing, FFI decoding, and .sfk cache
├── cpp-extensions/       # C/C++ (CMake: neoveg_cpp_engine) — VST3 hosting, OpenFX, Voukoder, Whisper.cpp, and Shared Memory IPC
├── python-backend/       # Python (neoveg) — Automation, AI (Whisper.cpp, CorridorKey), XML interchange, and user scripting
├── scripts/              # Dynamic C# (.cs) and Python (.py) scripts, QA runners, launchers + build_portable.py
├── extensions/           # Third-party dockable window extensions
├── third_party/          # External dependencies, SDKs, and open-source libraries
├── docs/                 # Project QA compliance dashboard (index.html) and Classic NLE research knowledge base
└── Assets/i18n/          # Runtime localization resources (pt-BR default)
```

---

## 3. Subsystem Standards & Guidelines

### 3.1. C# / Avalonia UI (Frontend)
- Use clean **MVVM** patterns and reactive bindings in Avalonia UI (`.axaml`).
- Maintain visual fidelity with Classic NLE themes (Classic Foundry, Classic Silver, Modern Carbon Dark, and Contemporary Electric Dark).
- **Icon-Only Buttons & Toolbars (`NeovegIcons.cs` / `MainWindow.cs`):** All toolbar and panel buttons render graphical vector icons (`StreamGeometry` / `PathIcon`), exposing their names, shortcuts, and descriptions exclusively as `ToolTip` on hover.
- **Floating & Undockable Panels (`DetachableDockPanel.cs`):** All primary workspace panels (**Timeline**, **Video Preview**, **Trimmer / Project Media**, and **Master Bus Mixer**) can be detached and dragged into independent floating windows (closing a floating window docks it back into its workspace slot).
- **Main Toolbar Below Timeline:** The primary icon toolbar (`BuildMainIconToolbar`) is positioned directly below the Multi-Track Timeline (`BuildTimelineWorkspace`).
- **Clean Video Preview (`VideoPreviewControl.cs`):** All graphical overlays and on-screen tools (`SMPTE HUD`, `Safe Areas`, `Grid`, `'F' Pan/Crop Gizmo`, `Split-Screen FX Bypass`, `Clip Overlay Labels`) start disabled by default (`false`) and are exposed optionally via the Preview Overlays menu/flyout or context menu.
- **Video Preview Toolbar (`MainWindow.cs` / `VideoPreviewControl.cs`):** Keep the compact, flat, square-cornered, icon-only toolbar equipped with Project Video Properties (resolution, FPS, pixel/display aspect), a single Preview Quality selector offering only `Draft`, `Preview`, `Good`, `Best` and `Auto` (each quality implies the preview resolution: Draft = 1/4, Preview = 1/2, Good/Best = full, Auto = fit to the preview window; no separate resolution selector), project video FX bypass, Split-Screen, overlays, PNG/JPEG frame snapshot save and clipboard copy, external-monitor pop-out, and full-screen preview (`Esc` closes). Tooltips describe every icon and selector. Show live project dimensions/bit depth/frame rate and effective preview dimensions in the preview status strip; refresh it when project settings, preview resolution, or viewport size changes.
- **Docked Panel Tabs (`desktop-host/Styles/NeovegDockTabStyles.cs`):** Every Neoveg docked window with tabs (Project Media, Trimmer, Video FX, Audio FX, Transitions, Generators, dockable extensions) must use the Classic NLE compact footer tab pattern via `NeovegDockTabStyles.Apply(tabControl)`: tab strip at the bottom of the window (`TabStripPlacement = Bottom`), compact flat headers (`FontSize = 11`, `Padding = 6,2,6,2`, 22px height, no Fluent selection pipe), and a horizontally scrolling tab strip with left/right `RepeatButton` arrows (plus mouse-wheel scrolling) that appear only when the tabs do not fit the panel width. The selected tab is always scrolled into view. Tabs remain undockable by dragging them out of the strip (`DetachableDockPanel.EnableTabUndocking`).
- **Track Header Number Box Blinking Indicator (`MainWindow.cs` / `TrackHeaderControl.axaml` / `TrackViewModels.cs`):** The blinking indicator dot inside the track identification number box (`numBadge` / `PART_TrackNumberBox`, e.g. `• 1`, `• 2`) is **exclusive to the currently active/selected track** (`SelectedTrackId` / `TrackViewModelBase.IsSelected == true`): only that track's dot alternates between White (`#FFFFFF`) and Black (`#000000`) on a `500ms` `DispatcherTimer`. Unselected tracks keep a static neutral dot with no animation (hidden in `TrackHeaderControl.axaml` via `IsVisible="{Binding IsSelected}"`), and the timer is stopped whenever no track is selected. A track becomes selected by clicking its header or any point of its timeline lane.
- **Precision NLE Slider (`desktop-host/Controls/NeovegPrecisionSlider.cs` / `MainWindow.cs` / `MasterBusView.cs`):** Opacity, volume, pan, and master fader sliders render a rectangular/square flat thumb (`CornerRadius = 0`, compact `8px` width, solid fill), reset to default on double-click (`1.0` / `100%` for opacity/volume, `0.0` for pan), apply `1/10` (`0.1x`) fine sensitivity while `Ctrl` is held during drag, and expose a right-click `ContextMenu` with `'Restaurar Padrão'` and `'Digitar Valor...'`.
- **Clean Startup (`NeovegProject.CreateEmpty`):** Neoveg starts directly in the main workspace with an empty project (0 tracks, 0 demo media clips, and `ShowWelcomeOnStartup = false`).
- **Event Pan/Crop & Inspector Restructuring (`EventPanCropWindow.cs` / `MainWindow.cs`):**
  - Event Pan/Crop is not docked beside the main Preview; it opens as a pop-up window (`EventPanCropWindow`) from the video clip's Event Header button and includes an embedded side-by-side live Video Preview toggle (`ShowSidePreview` / `SetSidePreviewVisible`) right next to the `'F'` gizmo canvas.
  - The **Inspector** panel is accessed via the **Ferramentas** (`_Ferramentas` / `Tools`) drop-down menu item **Inspetor** (`OpenInspectorWindow`). Do not embed input controls (e.g. a `ComboBox`) inside menus; the Roslyn console and dockable extension manager stay as regular menu items (Tools > Scripts, View > Window Extensions).
- **Menus & Preferences (`AdvancedSettingsWindow.cs` / `LocalizationManager.cs`):**
  - **Preferências** (`OpenAdvancedSettingsDialog`) is located inside the **Editar** (`_Editar`) menu, housing the interface **Idioma** (`Language`) selector (removed from the `Ferramentas` menu and main toolbar).
  - Automatic fade-in and fade-out on media import (audio, video, and image) are disabled by default (`AutoFadeOnImport = false`, `FadeIn = 0.0`, `FadeOut = 0.0`) and configurable via checkbox in **Editar -> Preferências**.
- **Event Header:** Every timeline clip (`MultiTrackTimelineControl`) must render an event header with the following buttons: *Generated Media*, *Event Pan/Crop*, *Video/Audio FX*, and *Hamburger Menu (☰)*.
- **Playhead (CTI) & Edit Cursor Blink (`PlayheadOverlay.axaml` / `TimelineCursor.cs` / `PlayheadOverlayCanvas.cs`):**
  - Blue pentagonal head (`#2980B9` / `#3498DB`) with pointed bottom beak and horizontal right-extending flag, `1px` vertical blue guide line across all tracks, and SMPTE Drop-Frame (`;`) / Non-Drop-Frame (`:`) timecode box above the marker tip.
  - Smooth caret blink animation (`DispatcherTimer` ~`500ms` alternating `VerticalLineOpacity` between `1.0` and `0.0`) when playback is paused (`IsPlaying == false`), locking to `1.0` (solid) at `60 FPS` during playback or scrubbing, and resetting on keyboard frame navigation (`Left` / `Right` arrow keys).
  - **1px Ultra-Fine, Pixel-Snapped Guide Line:** the vertical guide line is exactly one *physical* pixel wide at every zoom level, scroll offset and monitor DPI: pen thickness `1 / RenderScaling`, X centered on a device pixel via `TimelineFrameGrid.SnapToDevicePixel`, drawn under `RenderOptions.EdgeMode = Aliased` (`UseLayoutRounding = true` on the overlay). Never scale the line with `PixelsPerSecond`.
- **Sample-Accurate Audio Waveform (`rust-core/src/waveform_source.rs` / `desktop-host/Interop/NeovegWaveformSource.cs` / `AudioWaveformRenderer.cs`):** Timeline waveforms come from the real PCM samples, never from interpolated peak summaries:
  - Rust FFI `neoveg_waveform_open` / `_info` / `_query_columns` / `_read_samples` / `_close` (handle-based, null-checked, `catch_unwind`, explicit close) keeps the WAV PCM payload plus 256-frame min/max/sum-of-squares summaries; a managed C# fallback implements the same algorithm when the native library is unavailable. Non-WAV media is decoded once to a cached 16-bit WAV with the bundled FFmpeg (`%LOCALAPPDATA%\Neoveg\waveforms\`, `~/.cache/neoveg/waveforms/`, `~/Library/Caches/neoveg/waveforms/`). Sources open asynchronously; the UI thread never decodes audio.
  - ≥ 1 sample per pixel: exact min/max (outer) and RMS (inner) of every visible pixel column, drawn as one filled polygon per channel; a single-sample transient is never averaged away. < 1 sample per pixel: the individual samples joined by lines, with sample dots/stems once samples are ≥ 6px apart (Audacity style). Only the visible range is computed; clip `TrimIn`, `PlaybackRate`, gain and fades are honored.
  - Timeline zoom ranges from `MultiTrackTimelineControl.MinPixelsPerSecond` (15) to `MaxPixelsPerSecond` (480,000 px/s = 10px per sample at 48 kHz); mouse-wheel zoom is anchored at the pointer and toolbar zoom at the edit cursor. Event widths/fades use `DurationToWidth` (never `TimeToX`, which includes the scroll offset), and off-screen events are culled / clamped to ±10,000px for float precision.
- **Dynamic Timeline Frame Grid (`desktop-host/Controls/TimelineFrameGrid.cs` / `MultiTrackTimelineControl.cs`):** Grid density adapts to zoom using the step ladder `1, 2, 5, 10 frames → 1 s, 2 s, 5 s, 10 s, 15 s, 30 s, 1 min, 2 min, 5 min…`: minor lines use the smallest step at least `8px` apart (one line per frame — 1/24, 1/25, 1/30, 1/60 s from the project FPS — at close zoom), major lines the smallest step at least `90px` apart and carry the SMPTE ruler labels. Minor (frame) lines are `#2C3038`, major (second/ruler) lines `#4A505E`; both are 1 physical pixel, pixel-snapped and aliased, drawn over the lane background and behind events, only for the visible time range. Grid times are always derived from integer frame indices (never accumulated floats); second steps land on the frame where the SMPTE timecode reads `HH:MM:SS;00`, honoring drop-frame (`00:01:00;02` at dropped minute boundaries). Covered by `desktop-host.Tests/TimelineFrameGridTests.cs`; research notes in `docs/research/nle_features_research.txt` (entries 4–5).
- **Timeline Scrolling (`MainWindow.cs` / `MultiTrackTimelineControl.cs` / `PlayheadOverlayCanvas.cs`):** Keep horizontal navigation bidirectionally synchronized between the timeline scrollbar, SMPTE ruler, event canvas, and playhead using one time-based horizontal offset; update the scrollbar thumb from the visible time range at the current zoom and clamp it to the project duration. Keep track headers and event lanes synchronized through one vertical pixel offset, including when either side is scrolled, track heights change, or the viewport is resized. Timeline scrollbars use compact, flat, square-cornered dark-theme styling and provide adjacent zoom-out / zoom-in controls.
- **Render Status Ruler (`RenderStatusRuler.axaml` / `desktop-host/Controls/RenderStatusRuler.cs`):** A tri-color bar above the timeline displaying *Green* (`#2ECC71`, RAM/Prerender 60 FPS), *Yellow* (`#F1C40F`, Simple/Queued), and *Red* (`#E74C3C`, Complex/Unrendered) segments.
- **Audio Master Bus & Real-Time FFI Audio Metering (`MasterBusView.axaml` / `MasterBusView.cs` / `MasterBusViewModel.cs` / `MasterScopeControl.cs`):**
  - The Audio Master Bus panel renders exclusively the stereo Left (`L`) and Right (`R`) dBFS peak meters, dBFS scale (`0`, `-6`, `-12`, `-24`, `-48`, `-∞`), real-time `L`/`R` dBFS digital readouts, and the Master Volume fader (`MASTER BUS: +0.0 dB`). It must **never** include a video RGB Parade / Scope control in its top area.
  - All audio meters (VU / Peak meters) in Neoveg must respond **strictly** to the real-time FFI audio pipeline (`rust-core` / `neoveg_sample_wav_stereo_peak_at` and `cpp-extensions`), without timers, random number generators, trigonometric wave oscillators (`Math.Sin` / `Math.Cos`), or mock/test data in production builds.
  - When playback is stopped (`IsPlaying == false`) or when there is no active unmuted audio media on the timeline at the current playhead, all VU meters must remain completely zeroed and static (`0.0` linear / $-\infty\text{ dB}$).
- **Internationalization (i18n):** Managed via `LocalizationManager.cs` in `Assets/i18n/`. Default language is Brazilian Portuguese (`pt-BR`), with runtime support for `en-US`, `de-DE`, `es-ES`, `fr-FR`, `zh-CN`, `ja-JP`, and `ru-RU`.

### 3.2. Rust Core (`rust-core/`)
- Crate name: `neoveg_core`.
- Responsible for memory management, high-speed decoding, frame synchronization, SMPTE timecode math, 5-curve fades, auto-crossfades, and audio peak rendering (`.sfk`).
- All FFI functions exposed to C# must be annotated with `#[no_mangle] pub extern "C"` and prefixed with `neoveg_`, with explicit null pointer checking and catch-unwind safety boundaries.

### 3.3. C/C++ Extensions (`cpp-extensions/`)
- Managed via cross-platform `CMakeLists.txt` (Windows, Linux, macOS) targeting `neoveg_cpp_engine`.
- Used for interfacing with C++ SDKs involving multiple inheritance or virtual classes (VST3 SDK, OpenFX Core, Voukoder, Whisper.cpp).
- **Out-of-Process Isolation:** Unstable third-party plugins must be isolated via Shared Memory Mapped Files (`NVSH` protocol).

### 3.4. Python & Scripting (`Neoveg.Scripting` & `neoveg`)
- Enables users to run dynamic C# (`.cs` via Roslyn `Microsoft.CodeAnalysis.CSharp.Scripting`) and Python (`.py`) scripts directly from **Tools -> Scripting**.
- Classes under `Neoveg.Scripting` must mirror core editor objects (`NeovegHost`, `Project`, `Track`, `TrackEvent`, `Take`, `Effect`).

---

## 4. File Formats & Interoperability

- **Native Format (`.neoveg`):** Exclusive project format used across Neoveg (`Neoveg_Broadcast_Demo.neoveg`, `Neoveg_MovedMedia_RelinkTest.neoveg`, and `projects/autosave/recovery_*.neoveg` — ZIP container housing `project.json`, `media_pool.json`, and `history.json`). Neoveg creates, opens, autosaves, and manages projects strictly in `.neoveg` format (no `.veg` or `.otio` project files in `projects/`).
- **XML Interchange:** Import/export mapping via `Neoveg.ProjectInterchange` supporting FCP7 XML (`.xml`), FCPX (`.fcpxml`), and CMX3600 (`.edl`) for DaVinci Resolve, Premiere Pro, and Final Cut Pro integration.

---

## 5. Missing Media Protocol (Relink Engine)

Upon opening a project with disconnected media:
1. Trigger the `MissingMediaWindow.axaml` (`desktop-host/Views/MissingMediaWindow.cs`) modal.
2. Execute the 3-step search algorithm via Rust/Python (`Neoveg.Core.MediaRelinker` in `desktop-host/Services/MediaRelinker.cs` / `python-backend/media_relinker.py`):
   - *Step 1:* Exact filename match + Same-Directory Sibling Auto-Relink.
   - *Step 2:* Fuzzy filename match.
   - *Step 3:* Metadata validation (`FFprobe`) checking framerate, channels, and original duration + Rust FFI `.sfk` regeneration.

---

## 6. Attribution & Licensing

- All third-party dependencies and integrated codebases (Avalonia, FFmpeg, OpenFX, VST3 SDK, Whisper.cpp, Voukoder, CorridorKey) must have authors and licenses documented in `AUTHORS.md` and `third_party/README.md`.
- The portable packaging script (`scripts/build_portable.py` / `build_portable.py`) must copy `AUTHORS.md` and `LICENSE` to all generated packages in `dist/`.

### 6.1 External Dependencies & System Prerequisites

- **Visual C++ Redistributable All-in-One:** The official source for the Windows Visual C++ Redistributable All-in-One prerequisite package is [https://www.techpowerup.com/download/visual-c-redistributable-runtime-package-all-in-one/](https://www.techpowerup.com/download/visual-c-redistributable-runtime-package-all-in-one/). Extract its contents to `addon/vcredist_all_in_one/` (so `install_all.bat` is available there) or `addon/vcredist2022-2015_14.36.32532/`.
- **DirectX End-User Runtimes (June 2010):** The official source for the DirectX End-User Runtimes prerequisite package is [https://www.techpowerup.com/download/directx-redistributable-runtime/](https://www.techpowerup.com/download/directx-redistributable-runtime/). Place `directx_Jun2010_redist.exe` in `addon/directx_redist/` (or `addon/`).
- **Unified Silent Prerequisite Setup (`scripts/install_prerequisites.bat`):** Executes silent installation of both the Visual C++ Redistributable (`/q /norestart`) and the DirectX End-User Runtimes (`directx_Jun2010_redist.exe /q /t:%TEMP%\dxredist && %TEMP%\dxredist\DXSETUP.exe /silent`).
- **Startup Runtime & Hardware Acceleration Verification:** On Windows, the Neoveg bootstrap (`desktop-host/Program.cs` and `cpp-extensions/src/neoveg_cpp_engine.cpp`) checks for the Visual C++ runtime DLLs (`vcruntime140.dll`, `msvcp140.dll`) and DirectX visual/audio acceleration libraries (`d3d11.dll`, `XAudio2_7.dll`) before startup and records their status in `logs/neoveg.log`. If required DLLs are missing, `scripts/install_vcredist.bat` or `scripts/install_prerequisites.bat` requests administrator approval and runs the package installers silently. Do not bypass the Windows UAC prompt.
- **Cross-Platform Prerequisite Boundary:** Visual C++ All-in-One and DirectX installers are Windows-only. Their checks and installer invocations must be guarded with `OperatingSystem.IsWindows()`; Linux and macOS builds/startup must never invoke Windows installers. Keep native FFI loading platform-specific (`.dll` on Windows, `.so` on Linux, `.dylib` on macOS), and keep CMake platform dependencies inside `WIN32`, `APPLE`, or `UNIX` branches. Do not link Windows-only DirectX import libraries into the shared engine target.

---

## 7. Local Git Workspace & Google Drive Sync Protocol

To maintain maximum performance with Git/GitHub while preserving continuous cloud backups:

- **Primary Working Directory:** The local workspace is the source of truth. All development, Git commits, and GitHub synchronization must run from this local path.
- **Active Branch:** All current development and QA-suite fixes (Clippy, real QA reporting, Slider controls, trademark neutrality, Windows release rebuild) happen on the branch `release/v1.0.0-qa-refactor`, created from `main` at `f96b187`. Commit there and merge back into `main` only after `scripts/run_qa_suite.ps1` passes.
- **Target Cloud Backup Path:** `G:\Meu Drive\vp-rev-eng\teste\teste2`
- **Synchronization Rules (`scripts/sync_gdrive.ps1`):**
  - Use Robocopy to mirror the local workspace to the Google Drive target.
  - Exclude all files and directories matched by the root `.gitignore` from both Git commits and Google Drive synchronization. Keep the sync script's exclusion filters aligned with `.gitignore`.
  - Run automatically after a successful portable build on Windows, or manually through `scripts/sync_gdrive.ps1`.

---

## 8. Logging & Crash Diagnostics Protocol

To ensure full observability during runtime and diagnose crashes or silent failures across all subsystems:

- **Centralized Log Directory:** All logs must be written to `logs/neoveg.log` (active session) and `logs/crash.log` (unhandled crashes) inside the application root directory.
- **Log Rotation & Retention:** Retain up to 5 historical log files (`logs/neoveg.YYYY-MM-DD.log`) with a 10MB file size limit per log file.
- **Subsystem Integration:**
  - **C# / .NET (Frontend):** Use structured rotating file + console logging (`Program.LogEvent` / `WriteCrashLog`, compatible with `Serilog` or `Microsoft.Extensions.Logging` sinks). Intercept global unhandled exceptions via `AppDomain.CurrentDomain.UnhandledException`, `TaskScheduler.UnobservedTaskException`, and Avalonia's `RxApp.DefaultExceptionHandler`.
  - **Rust Backend (`rust-core`):** Use the `tracing` / `tracing-subscriber` crate, capturing Rust panics via `std::panic::set_hook` and piping logs to the central file.
  - **C++ Extensions (`cpp-extensions`):** Log VST3/OpenFX plugin initialization, FFI calls, and native exceptions to `logs/neoveg.log`.
  - **Python Backend (`python-backend`):** Configure Python's standard `logging` module with a file handler pointing to `logs/neoveg.log`.
- **Log Levels:** Enforce strict log leveling (`[DEBUG]`, `[INFO]`, `[WARN]`, `[ERROR]`, `[FATAL]`). FFI boundaries and media decoding routines must log contextual details (file paths, frame indices, HRESULTs, error codes) before raising UI alerts.
- **Log Viewer Clipboard Support (`LogViewerWindow.axaml` / `LogViewerViewModel.cs`):** The diagnostic log viewer window must mandatorily provide a quick **Copy Logs to Clipboard** (`Copiar Logs` / `CopyToClipboardCommand`) feature using Avalonia's `TopLevel.GetTopLevel(window)?.Clipboard?.SetTextAsync(...)` API, supporting both partial user text selection (`TextBox` / `SelectableTextBlock`) and full-log copying with visual confirmation feedback (`Copiado!` for 2 seconds).

---

## 9. Local App Cache & Cloud Drive Execution Protocol

To avoid performance bottlenecks, file-locking issues, and DLL loading failures when running from cloud-synced or virtual drives (e.g., Google Drive `G:\`, OneDrive, network shares):

- **Cloud Drive Detection:** On startup (`Program.cs`), check if `AppDomain.CurrentDomain.BaseDirectory` resides on a virtual drive, network share (`\\`), or known cloud sync paths (e.g., `G:\`, `G:\Meu Drive`).
- **Local Cache Target:**
  - **Windows:** `%LOCALAPPDATA%\Neoveg\AppCache\`
  - **Linux:** `~/.cache/neoveg/app_cache/`
  - **macOS:** `~/Library/Caches/neoveg/app_cache/`
- **Cache Synchronization Logic:**
  1. Compare modification timestamps or file hashes of `neoveg.exe`, DLLs, and `Assets/` between the source path and the local cache directory.
  2. If the local cache is missing or outdated, mirror/copy the updated application files to the local cache folder.
- **Process Relocation & Execution:**
  - Launch the cached binary (`%LOCALAPPDATA%\Neoveg\AppCache\neoveg.exe`) in a new process, preserving the original command-line arguments and working directory context.
  - Exit the initial drive-hosted process immediately (`Environment.Exit(0)`).
- **Log Indication:** Record the execution path and caching status in `logs/neoveg.log`.

---

## 10. Functional Specifications & Behavioral Reference Manuals

Neoveg's UI/UX layout, editing mechanics, shortcut keys, and signal routing pipelines are strictly specified based on broadcast NLE standards and clean-room architectural specifications:

- **Classic NLE Audio & Timeline Foundations (SMPTE ST 12-1 & Multi-Track DSP):** Sub-frame editing (`1e-6`s), 5-curve fade envelopes (`Linear`, `Fast`, `Slow`, `Smooth`, `Sharp`), and automatic overlap crossfades.
- **Procedural Titles, GPU Acceleration & Media Generators:** Real-time procedural media generation (`Neoveg Titles & Text Overlay`, `SMPTE Color Bars`, `Solid Color`, `Test Pattern`).
- **Expanded NLE Workflows & Trimmer Integration:** Synchronized Timeline and Trimmer editing, ripple modes, and frame-quantized trimming.
- **Docking UI, Color Grading & Hierarchical FX Pipelines:** Detachable floating panels, compact bottom-tabbed docks, and 5-stage hierarchical signal routing (`Media FX -> Take FX -> Event FX -> Track FX -> Master FX`).
- **Modern Dark UI & Motion Tracking:** Vector gizmo Event Pan/Crop (`'F'` frame canvas), keyframe timeline diamond interpolation, and 2D affine camera matrices.
- **Contemporary AI & Native Plugin Ecosystem:** Native C/C++ OpenFX, VST3, Voukoder, Whisper.cpp speech-to-subtitle transcription, and CorridorKey chroma/alpha keying.

**Implementation Mandate:**
All window layouts, keyboard shortcut mappings (`Ctrl+Q` insert Audio Track, `Ctrl+Shift+Q` insert Video Track, `S` split, `M` marker, `G` group, `U` ungroup, `F8` snapping), audio bus topologies (Media FX $\rightarrow$ Take FX $\rightarrow$ Event FX $\rightarrow$ Track FX $\rightarrow$ Master FX), high-zoom sample-accurate audio waveform rendering (exact per-pixel min/max/RMS from the real PCM samples and individual sample points at sample-level zoom via `rust-core/src/waveform_source.rs`; summarized peaks are never interpolated), and event manipulation behaviors in Neoveg must mirror these specifications and the research notes in `docs/research/nle_features_research.txt`.

---

## 11. Quality Assurance (QA), Performance Benchmarking & Docker Protocol

To guarantee code stability, zero-lag timeline performance, memory safety, and reproducible builds across all architectures:

- **Approved QA & Profiling Toolchain:** Use `Avalonia.Headless.XUnit` for UI tests without a display server, Criterion.rs for Rust benchmarks, GoogleTest for C++ unit tests, AddressSanitizer/UndefinedBehaviorSanitizer for native FFI memory and undefined-behavior checks, and Tracy Profiler for performance investigation. Run `scripts/run_qa_suite.ps1` on Windows or `scripts/run_qa_suite.sh` on Linux/macOS before every release; all static analysis, tests, sanitizer checks, and coverage reporting must pass.
- **Unified QA Suite:** The scripts run `dotnet format --verify-no-changes`, `cargo clippy`, and `clang-tidy`, then .NET, Rust, Criterion waveform benchmark, CTest, and Python test suites (`scripts/test_neoveg_engine.py`), and finally collect/verify code-coverage reports. QA prerequisites include .NET 8 SDK, Rust/Cargo, GCC or GNU-compatible Clang with ASan/UBSan, CMake, Ninja, `clang-tidy`, `cargo-llvm-cov`, `gcovr`, Python with `pytest-cov`, and CTest. Missing or unsupported tools must fail the suite explicitly rather than silently skipping a check.
- **QA Reporting & Standards Traceability:** Every QA step is tagged with an ISO/IEC 25010 pillar (Performance Efficiency, Reliability, Usability, Portability, Maintainability) and the ABNT NBR / ISO/IEC requirement it evidences. Each step logs `START` / `PASS` / `FAIL` lines with that requirement to `logs/neoveg.log` (`[QA/<Pillar>]` source), and the suite always writes `coverage/qa/qa_report.json` plus `docs/qa_report.js` (also on failure), which `docs/index.html` renders in its ISO/ABNT compliance panel.
- **Performance & Bottleneck Elimination Benchmarks:**
  - **UI Thread (Avalonia):** The main UI thread must never freeze during timeline zooming, scrubbing, or event dragging. Heavy operations (waveform generation, project parsing, FFmpeg indexing) must be executed asynchronously on background threads or via Rust FFI, with pre-allocated static pens/brushes (`StaticCenterAxisPen`, `BrushCache`) to avoid per-frame GC pressure.
  - **Profiling & Benchmarks:** Maintain waveform engine benchmarks with Criterion.rs and use Tracy Profiler to capture representative timeline/rendering workloads before release.
  - **Sanitizers & Native Tests:** Run C++ tests registered with CTest/GoogleTest under AddressSanitizer and UndefinedBehaviorSanitizer in QA builds. Sanitizer/coverage configurations require GCC or GNU-compatible Clang.
  - **Memory & FFI Management:** Zero buffer leaks when transferring frame buffers or audio samples between C# and Rust/C++. All FFI pointer allocations must have explicit deallocation wrappers (`neoveg_free_buffer`, `neoveg_cpp_free_buffer`, `FreeNativeBuffer`, `IDisposable`).
  - **Drive I/O Optimization:** Ensure execution from `%LOCALAPPDATA%\Neoveg\AppCache\` with fast fingerprint verification (`appcache_sync.stamp` / `ComputeFastCacheFingerprint`) to prevent Google Drive lockouts and latency spikes.
- **Docker Integration & Containerization (`Dockerfile` / `docker-compose.yml`):**
  - Maintain a multi-stage Docker build pipeline (`rust-builder`, `cpp-builder`, `dotnet-builder`, `runtime`) for headless automated testing (`neoveg-qa`), C++ cross-compilation (GCC/Clang), and Linux/macOS artifact generation.
- **GitHub-Style Project Dashboard (`docs/index.html`):**
  - Host a local/web dashboard mirroring GitHub’s interface to track project milestones, module completion status (C#, Rust, C++, Python), QA test results, and real Docker container status.

---

## 12. QA Audit, Stable Build & Release Deployment Protocol

To ensure software stability, zero runtime regressions, and a production-ready distribution package:

- **Automated QA & Test Suite Execution:**
  - Execute full unit and integration test suites across all core modules:
    - **C# / Avalonia (`desktop-host`):** `dotnet test`
    - **Rust Core (`rust-core`):** `cargo test --release`
    - **C++ Extensions (`cpp-extensions`):** `ctest` / CMake test targets
    - **Python Backend (`python-backend` / `scripts`):** `pytest` and `python scripts/test_neoveg_engine.py`
  - Audit FFI bridges for pointer safety, memory leaks, and exception propagation.
  - Verify that unhandled exceptions, UI freezes, and FFI memory leaks are absent; confirm that `logs/neoveg.log` and in-app error capture operate correctly.
- **Production Executable Generation (`.exe`):**
  - Build standalone, optimized Release binaries with `dotnet publish desktop-host/Neoveg.csproj -c Release -r win-x64 --self-contained true`.
  - Bundle all required native dependencies (`neoveg_core.dll`, `neoveg_cpp_engine.dll`), `Assets/`, the Bootstrapper/Local AppCache scripts, and prerequisite installers.
- **Release Packaging & Cloud Mirroring:**
  - Package the final distribution as `dist/Neoveg-Portable-win-x64/` and `dist/Neoveg_v1.0.0_win-x64.zip`, including `RELEASE_NOTES.md` and `AUTHORS.md`.
  - Mirror the project and release package to `G:\Meu Drive\vp-rev-eng\teste\teste2` via `scripts/sync_gdrive.ps1`.

---

## 13. Developer Environment & Dependency Enforcement Protocol

To ensure smooth onboarding and prevent missing dependency errors across dev environments:

- **Dependency Specifications:** Maintain explicit toolchain versions in `REQUIREMENTS.md` and `python-backend/requirements.txt`.
- **Pre-Flight Environment Check (`desktop-host/Program.cs`):**
  - Verify presence of required runtimes (.NET 8 runtime/self-contained, C++ VC++ Redistributable on Windows, system shared libraries on Linux/macOS) prior to application launch.
  - Log diagnostic reports of missing SDKs/toolchains to `logs/neoveg.log` with actionable resolution steps.

---

## 14. Cross-Platform & Portable Release Protocol

To ensure seamless distribution across all major operating systems without requiring system installation privileges:

- **Target Platforms & Runtime Identifiers (RIDs):**
  - **Windows (x64):** `win-x64`
  - **Linux (x64):** `linux-x64`
  - **macOS (Intel x64):** `osx-x64`
  - **macOS (Apple Silicon M-Series):** `osx-arm64`
- **Portable Build Standard:**
  - Build all binaries as **Self-Contained** with embedded .NET runtime (`--self-contained true`).
  - Bundle platform-native shared libraries alongside the main executable:
    - Windows: `neoveg_core.dll`, `neoveg_cpp_engine.dll`
    - Linux: `libneoveg_core.so`, `libneoveg_cpp_engine.so`
    - macOS: `libneoveg_core.dylib`, `libneoveg_cpp_engine.dylib`
- **Archive Formats:**
  - **Windows:** `dist/Neoveg_v1.0.0_win-x64_Portable.zip`
  - **Linux:** `dist/Neoveg_v1.0.0_linux-x64_Portable.tar.gz` (plus executable launcher script `neoveg.sh`)
  - **macOS:** `dist/Neoveg_v1.0.0_osx-universal_Portable.tar.gz` (`dist/Neoveg_v1.0.0_osx-x64_Portable.tar.gz` and `dist/Neoveg_v1.0.0_osx-arm64_Portable.tar.gz`, or `.app` bundle)

---

## 15. Native DLL Bundling & Assembly Resolution Protocol

To guarantee that `Neoveg.exe` loads native shared libraries seamlessly in portable mode:

- **Automatic DLL Copying (`desktop-host/Neoveg.csproj`):**
  - Configure compiled Windows Rust and C++ libraries (`neoveg_core.dll` and `neoveg_cpp_engine.dll`) as content copied beside the executable on build and publish.
  - Copy the Windows Avalonia rendering runtime libraries (`libSkiaSharp.dll`, `libHarfBuzzSharp.dll`, and `av_libglesv2.dll`) beside the executable as well; direct launches must not depend on an inherited `PATH`.
- **Dynamic FFI Resolution (`desktop-host/Program.cs`):**
  - Register `NativeLibrary.SetDllImportResolver` for the application assembly before startup. Search the executable directory first, then `%LOCALAPPDATA%\Neoveg\AppCache\`, followed by the packaged `runtimes/<RID>/native/` directory; leave unrelated platform/system imports to the runtime.
- **Validation on Startup:**
  - Verify the required Rust and C++ libraries are available beside `Neoveg.exe`, in AppCache, or in the packaged runtime directory, and log their resolved paths to `logs/neoveg.log`.

---

## 16. ISO/IEC Software Quality & Standards Compliance Protocol

To ensure industrial-grade software reliability, maintainability, and cross-platform performance, Neoveg strictly aligns its development, QA, and architectural practices with recognized ISO/IEC standards:

- **ISO/IEC 25010 (Software Quality Model):**
  - **Performance Efficiency:** Timeline scrubbing and audio waveform generation must maintain low-latency async execution (Rust Core) without blocking the UI thread.
  - **Reliability:** Fault-tolerant execution across C#, Rust, C++, and Python subsystems with structured logging (`logs/neoveg.log` / `logs/crash.log`), `std::panic::catch_unwind` FFI boundaries, and graceful fallback modes.
  - **Usability:** Classic NLE UI fidelity, multi-language runtime localization (`pt-BR` default across 8 locales), accessible keyboard shortcuts, and visual diagnostic feedback.
  - **Portability:** Codebase must build natively on Windows (`win-x64`), Linux (`linux-x64`), and macOS (`osx-x64`/`osx-arm64`) using abstraction layers (`OperatingSystem.IsWindows()`, `OperatingSystem.IsLinux()`, `OperatingSystem.IsMacOS()`, `Path.Combine`) without hardcoded Windows-only paths.
  - **Maintainability:** Code structure must maintain clean modular isolation between C# Avalonia, Rust FFI, C++ Extensions, and Python backend.
- **ISO/IEC 29119 (Software Testing Standard):**
  - All features must pass automated unit, integration, and headless UI tests (`Avalonia.Headless`) before being merged or packaged for release, with QA reports (`scripts/run_qa_suite.ps1`, `scripts/run_qa_suite.sh`, `coverage/qa/qa_report.json`, and `docs/index.html`) categorized by ISO/IEC 25010 quality pillars. See Section 17 for the ABNT NBR harmonization.
- **ISO/IEC 27001 / Memory Safety Assurance (Security & Memory Integrity):**
  - Dynamic memory allocations across FFI boundaries must use explicit ownership and cleanup models (AddressSanitizer and LeakSanitizer validated, explicit `neoveg_free_buffer` / `neoveg_cpp_free_buffer` / `IDisposable` deallocators, and 64-bit integer overflow guards).

---

## 17. ABNT NBR & ISO/IEC Technical Standards Compliance

Neoveg strictly aligns its software architecture, quality assurance protocols, and life-cycle management with official Brazilian technical standards (ABNT NBR) in direct harmonization with international ISO/IEC specifications:

- **ABNT NBR ISO/IEC 25010 (Quality Model for Software Products):**
  - **Performance Efficiency:** Sub-millisecond audio peak calculations (Rust Core, Criterion `audio_peaks` benchmark) and smooth timeline rendering (Avalonia UI) without thread contention.
  - **Functional Suitability:** Full parity with Classic NLE editing workflows, keyboard shortcuts, and signal routing topology (Section 10).
  - **Portability:** Native build capability across Windows (`win-x64`), Linux (`linux-x64`), and macOS (`osx-x64`/`osx-arm64`); Windows-only paths (e.g., the Google Drive mirror) must be guarded by `OperatingSystem.IsWindows()` / `os.name == "nt"` or be overridable through environment variables.
- **ABNT NBR ISO/IEC 29119 (Software Testing Framework):**
  - Continuous execution of multi-tier test suites (Unit, Integration, FFI Safety, and Headless UI rendering via `Avalonia.Headless`) through `scripts/run_qa_suite.ps1` / `scripts/run_qa_suite.sh`, with coverage evidence in `coverage/qa/`.
- **ABNT NBR ISO/IEC 12207 & 14764 (Life Cycle Processes & Maintenance):**
  - Strict modular separation across C# frontend, Rust core, C++ extensions, and Python backend to support long-term maintainability, enforced by `dotnet format`, `cargo clippy`, and `clang-tidy`.
- **ABNT NBR ISO/IEC 27001 (Information & Memory Security):**
  - Memory-safe FFI boundaries (caller-owned buffers with explicit lengths, `catch_unwind`, `neoveg_free_buffer` / `neoveg_cpp_free_buffer`), zero buffer overruns validated under ASan/UBSan, and isolated local caching (`%LOCALAPPDATA%\Neoveg\AppCache\`).
- **Traceability in Logs & Reports:** Every automated check must reference its corresponding ABNT NBR / ISO/IEC requirement in `logs/neoveg.log` and in `coverage/qa/qa_report.json`; `docs/index.html` displays the ABNT NBR ISO/IEC badges and the per-pillar QA results.

---

## 18. Cross-Platform OS Detection & Launcher Protocol

To guarantee seamless execution across Windows, Linux, and macOS and preserve research notes:

- **Runtime OS Detection (`desktop-host/Program.cs`):**
  - Use `OperatingSystem.IsWindows()`, `OperatingSystem.IsLinux()`, and `OperatingSystem.IsMacOS()` during application bootstrap.
- **Platform-Agnostic Local Cache & Log Directories:**
  - **Windows (`OperatingSystem.IsWindows()`):** `%LOCALAPPDATA%\Neoveg\` (`%LOCALAPPDATA%\Neoveg\AppCache\` and `%LOCALAPPDATA%\Neoveg\logs\`, alongside workspace `logs/`).
  - **Linux (`OperatingSystem.IsLinux()`):** `~/.cache/neoveg/` (`~/.cache/neoveg/app_cache/` and `~/.cache/neoveg/logs/`).
  - **macOS (`OperatingSystem.IsMacOS()`):** `~/Library/Caches/neoveg/` (`~/Library/Caches/neoveg/app_cache/` and `~/Library/Caches/neoveg/logs/`).
- **Dynamic FFI Shared Library Resolution (`RustCoreInterop.cs` / `CppCoreInterop.cs` / `Program.cs`):**
  - **Windows:** `.dll` (`neoveg_core.dll`, `neoveg_cpp_engine.dll`)
  - **Linux:** `.so` (`libneoveg_core.so`, `libneoveg_cpp_engine.so`)
  - **macOS:** `.dylib` (`libneoveg_core.dylib`, `libneoveg_cpp_engine.dylib`)
- **Cross-Platform Launcher Scripts (`scripts/`):**
  - **`scripts/neoveg.sh` (Linux & macOS):** Must carry executable permissions (`chmod +x`), auto-detect the kernel (`Linux` vs `Darwin`), configure `LD_LIBRARY_PATH` (Linux) or `DYLD_LIBRARY_PATH` (macOS) pointing to `runtimes/<rid>/native/` and `runtimes/<rid>/ffmpeg/lib/`, initialize the platform cache directory, and launch `Neoveg`.
  - **`scripts/neoveg.bat` (Windows):** Configures `PATH` for `runtimes\win-x64\native\` and `runtimes\win-x64\ffmpeg\bin\`, prepares `%LOCALAPPDATA%\Neoveg\AppCache`, and launches `Neoveg.exe`.

---

## 19. Strict Trademark & Brand Neutrality Protocol

To prevent intellectual property conflicts and maintain complete independence:

- **Prohibited Terms:** Under no circumstances should the terms `Sony`, `Vegas`, `VEGAS`, `MAGIX`, `BorisFX`, `OpenVegas`, or `KVegas` appear in user-facing UI strings, source code comments, variable/file names, asset identifiers, test scripts, or documentation.
- **Approved Naming Alternatives:**
  - Use **`Neoveg`** for project identity.
  - Use generic industry terms for features/UI: **`Classic NLE`**, **`Professional Timeline`**, **`Standard Video FX`**, **`Procedural Titles`**, **`Track Header`**, **`Precision Fader`**.
- **Automated Sanity Check:**
  - Prior to every release build, the build pipeline must run a string-audit scan to verify zero occurrences of prohibited trademarks in the compiled output and repository.

---

## 20. Fallback Web Research & NLE Knowledge Base Protocol

To keep behavioral fidelity with the reference NLE workflows (Section 10) even when the local manuals lack implementation details:

- **Missing Manual Specification Fallback:**
  - If a feature, UX interaction, or algorithm (e.g., dynamic frame grid calculation, timecode snapping rules) is not detailed in the local reference manuals, research official online help first, then community documentation, and record which kind of source backs each rule.
- **Knowledge Base Persistence (`docs/research/nle_features_research.txt`):**
  - Save every discovered specification, rule, and research note in `docs/research/nle_features_research.txt`, structured as *Feature Name*, *Exact Reference Behavior*, *Visual/Technical Rules*, *Implementation Guidelines for Neoveg*, *Sources* and the research date.
  - When no source documents a detail, write `NOT DOCUMENTED` and record the rule as a Neoveg design decision instead of presenting it as reference behavior.
  - Consult this file before implementing or changing any parity behavior, and update it when new evidence contradicts an entry.
  - Research notes follow Section 19: third-party product names, trademarks and their URLs stay out of the repository (keep them only in local, untracked notes).

---

## 22. Pure Rust Architecture & Single-Toolchain Protocol

To permanently eliminate FFI memory overhead, multi-language build complexity, and cross-platform binary inconsistencies:

- **Scope:** This section supersedes any earlier section that requires C#, C++, Python or FFI for the core (sections 2, 3.1-3.4, 11-15 and 18). The legacy directories (`desktop-host/`, `rust-core/`, `cpp-extensions/`, `python-backend/`) are a porting reference only and are removed once their features are ported.
- **100% Rust Ecosystem Standard:**
  - Neoveg is strictly constructed as a pure Rust Cargo Workspace (`crates/neoveg_ui`, `crates/neoveg_engine`, `crates/neoveg_audio`, `crates/neoveg_video`, `crates/neoveg_common`).
  - Prohibit introduction of C#, C++, Python, or external FFI bindings for core editing features.
- **UI & Graphics Architecture:**
  - UI is rendered using `egui` with `egui_dock` for Classic NLE-style dockable panels.
  - Video and timeline rendering is handled via `wgpu` (WebGPU) for cross-platform GPU acceleration.
- **Audio & Media Pipelines:**
  - Audio playback uses `cpal` for cross-platform hardware output and `symphonia` for audio decoding.
  - Video frame extraction uses native Rust bindings for FFmpeg (`ffmpeg-next`), behind the optional `ffmpeg` feature of `neoveg_video` so a plain `cargo build --workspace` needs no system FFmpeg.
- **Unified Build & QA:**
  - Build entire project across all target OSs: `cargo build --workspace --release`
  - Run full test suite & linter: `cargo test --workspace` && `cargo clippy --workspace -- -D warnings`
- **Carried-over rules:** Sections 1 (naming), 19 (trademark neutrality) and the behavioral rules of 3.1 and 10 (meters driven only by real samples and zeroed when stopped, 1-physical-pixel playhead line, sample-accurate waveforms, adaptive frame grid, square precision sliders, bottom dock tabs) still apply and are implemented in the crates above.
