## Project Naming Conventions & Rules (Diretrizes Estritas de Padronização de Nomes)

Strict adherence to the project naming convention is mandatory across all codebases, configuration files, directories, and documentation (A adesão estrita à convenção de nomes é obrigatória em todo o código-fonte, configurações, diretórios e documentação):

- **Official Name (Nome Oficial):** `Neoveg` (for C# namespaces, `.sln`, `.csproj`, UI branding, and titles) / `neoveg` (for files, folders, Rust crates, C++ CMake targets, and Python modules).
- **Prohibited Names (Nomes Terminantemente Proibidos):** Never use legacy identifiers such as `kvegas`, `openvegas`, `openvegaspro21`, `OpenVegas`, `KVegas`, or `OpenVegasDesktop`.
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
- **Prohibited Identifiers:** Never use legacy names or prefixes such as `kvegas`, `openvegas`, `openvegaspro21`, `OpenVegas`, `KVegas`, or `OpenVegasDesktop`.
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
├── rust-core/            # Rust (neoveg_core) — High-performance video/audio processing, FFI decoding, and .sfk cache
├── cpp-extensions/       # C/C++ (CMake: neoveg_cpp_engine) — VST3 hosting, OpenFX, Voukoder, Whisper.cpp, and Shared Memory IPC
├── python-backend/       # Python (neoveg) — Automation, AI (Whisper.cpp, CorridorKey), XML interchange, and user scripting
├── scripts/              # Dynamic C# (.cs) and Python (.py) scripts + build_portable.py
├── extensions/           # Third-party dockable window extensions
├── third_party/          # External dependencies, SDKs, and open-source libraries
└── Assets/i18n/          # Runtime localization resources (pt-BR default)
```

---

## 3. Subsystem Standards & Guidelines

### 3.1. C# / Avalonia UI (Frontend)
- Use clean **MVVM** patterns and reactive bindings in Avalonia UI (`.axaml`).
- Maintain visual fidelity with historical VEGAS themes (Sonic Foundry, Sony Classic, MAGIX Modern Dark, and Boris FX Contemporary).
- **Event Header:** Every timeline clip (`MultiTrackTimelineControl`) must render an event header with the following buttons: *Generated Media*, *Event Pan/Crop*, *Video/Audio FX*, and *Hamburger Menu (☰)*.
- **Playhead (CTI) & Edit Cursor Blink (`PlayheadOverlay.axaml` / `TimelineCursor.cs` / `PlayheadOverlayCanvas.cs`):**
  - Blue pentagonal head (`#2980B9` / `#3498DB`) with pointed bottom beak and horizontal right-extending flag, `1px` vertical blue guide line across all tracks, and SMPTE Drop-Frame (`;`) / Non-Drop-Frame (`:`) timecode box above the marker tip.
  - Smooth caret blink animation (`DispatcherTimer` ~`500ms` alternating `VerticalLineOpacity` between `1.0` and `0.0`) when playback is paused (`IsPlaying == false`), locking to `1.0` (solid) at `60 FPS` during playback or scrubbing, and resetting on keyboard frame navigation (`Left` / `Right` arrow keys).
- **Render Status Ruler (`RenderStatusRuler.axaml`):** A tri-color bar above the timeline displaying *Green* (`#2ECC71`, RAM/Prerender 60 FPS), *Yellow* (`#F1C40F`, Simple/Queued), and *Red* (`#E74C3C`, Complex/Unrendered) segments.
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
- Classes under `Neoveg.Scripting` must mirror core editor objects (`Vegas`, `Project`, `Track`, `TrackEvent`, `Take`, `Effect`).

---

## 4. File Formats & Interoperability

- **Native Format (`.neoveg`):** Default project format (ZIP container housing `project.json`, `media_pool.json`, and `history.json`).
- **XML Interchange:** Import/export mapping via `Neoveg.ProjectInterchange` supporting FCP7 XML (`.xml`), FCPX (`.fcpxml`), and CMX3600 (`.edl`) for DaVinci Resolve, Premiere Pro, and Final Cut Pro integration.
- **Legacy Projects (`.veg`):** Supported exclusively as an import-only format via the Rust FFI inspector (`neoveg_inspect_veg_container`).

---

## 5. Missing Media Protocol (Relink Engine)

Upon opening a project with disconnected media:
1. Trigger the `MissingMediaWindow.axaml` modal.
2. Execute the 3-step search algorithm via Rust/Python (`Neoveg.Core.MediaRelinker` / `media_relinker.py`):
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
  - **C# / .NET (Frontend):** Use `Serilog` (or `Microsoft.Extensions.Logging`) with File and Console sinks. Intercept global unhandled exceptions via `AppDomain.CurrentDomain.UnhandledException`, `TaskScheduler.UnobservedTaskException`, and Avalonia's `RxApp.DefaultExceptionHandler`.
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

Neoveg's UI/UX layout, editing mechanics, shortcut keys, and signal routing pipelines are strictly specified based on official VEGAS Pro documentation across historical and modern releases (Sony VEGAS Pro 10 through VEGAS Pro 21):

- **VEGAS Pro 10 Manual (Classic Foundations & Audio DSP):** `https://www.strumentimusicali.net/manuali/SONY_VEGASPRO10_ENG.pdf`
- **VEGAS Pro 11 Guide (Titler, GPU Acceleration & Media Generators):** `https://pt.slideshare.net/slideshow/apostila-vegas-pro-11/38777730`
- **VEGAS Pro 12 Reviewer's Guide (Expanded NLE Workflows & Expanded Edit Mode):** `https://pro.sony/s3/cms-static-content/brochures/VP12Edit_ReviewersGuide.pdf`
- **VEGAS Pro 13 Official Manual (Docking UI, Color Grading & FX Pipelines):** `https://archive.org/details/vegaspro13-0-manual-enu-477102/mode/2up`
- **VEGAS Pro 18 Official Manual (Modern Dark UI & Motion Tracking):** `https://shared.steamstatic.com/store_item_assets/steam/apps/1325400/manuals/VEGAS_Pro_18_EN.pdf`
- **VEGAS Pro 21 Reference (Contemporary Feature Parity & AI Tools):** `https://pt.scribd.com/document/908733746/Manual-Vegaspro21-en-47zge9`

**Implementation Mandate:**
All window layouts, keyboard shortcut mappings (`S` split, `M` marker, `G` group, `U` ungroup, `F8` snapping), audio bus topologies (Take FX $\rightarrow$ Event FX $\rightarrow$ Track FX $\rightarrow$ Master FX), and event manipulation behaviors in Neoveg must mirror the specifications defined in these reference manuals.

---

## 11. Quality Assurance (QA), Performance Benchmarking & Docker Protocol

To guarantee code stability, zero-lag timeline performance, memory safety, and reproducible builds across all architectures:

- **Approved QA & Profiling Toolchain:** Use `Avalonia.Headless.XUnit` for UI tests without a display server, Criterion.rs for Rust benchmarks, GoogleTest for C++ unit tests, AddressSanitizer/UndefinedBehaviorSanitizer for native FFI memory and undefined-behavior checks, and Tracy Profiler for performance investigation. Run `scripts/run_qa_suite.ps1` on Windows or `scripts/run_qa_suite.sh` on Linux/macOS before every release; all static analysis, tests, sanitizer checks, and coverage reporting must pass.
- **Unified QA Suite:** The scripts run `dotnet format --verify-no-changes`, `cargo clippy`, and `clang-tidy`, then .NET, Rust, Criterion waveform benchmark, CTest, and Python test suites, and finally collect/verify code-coverage reports. QA prerequisites include .NET 8 SDK, Rust/Cargo, GCC or GNU-compatible Clang with ASan/UBSan, CMake, Ninja, `clang-tidy`, `cargo-llvm-cov`, `gcovr`, Python with `pytest-cov`, and CTest. Missing or unsupported tools must fail the suite explicitly rather than silently skipping a check.
- **Performance & Bottleneck Elimination Benchmarks:**
  - **UI Thread (Avalonia):** The main UI thread must never freeze during timeline zooming, scrubbing, or event dragging. Heavy operations (waveform generation, project parsing, FFmpeg indexing) must be executed asynchronously on background threads or via Rust FFI, with pre-allocated static pens/brushes (`StaticCenterAxisPen`, `BrushCache`) to avoid per-frame GC pressure.
  - **Profiling & Benchmarks:** Maintain waveform engine benchmarks with Criterion.rs and use Tracy Profiler to capture representative timeline/rendering workloads before release.
  - **Sanitizers & Native Tests:** Run C++ tests registered with CTest/GoogleTest under AddressSanitizer and UndefinedBehaviorSanitizer in QA builds. Sanitizer/coverage configurations require GCC or GNU-compatible Clang.
  - **Memory & FFI Management:** Zero buffer leaks when transferring frame buffers or audio samples between C# and Rust/C++. All FFI pointer allocations must have explicit deallocation wrappers (`neoveg_free_buffer`, `neoveg_cpp_free_buffer`, `FreeNativeBuffer`, `IDisposable`).
  - **Drive I/O Optimization:** Ensure execution from `%LOCALAPPDATA%\Neoveg\AppCache\` with fast fingerprint verification (`appcache_sync.stamp` / `ComputeFastCacheFingerprint`) to prevent Google Drive lockouts and latency spikes.
- **Docker Integration & Containerization (`Dockerfile` / `docker-compose.yml`):**
  - Maintain a multi-stage Docker build pipeline (`rust-builder`, `cpp-builder`, `dotnet-builder`, `runtime`) for headless automated testing (`neoveg-qa`), C++ cross-compilation (GCC/Clang), and Linux/macOS artifact generation.
- **GitHub-Style Project Dashboard (`docs/index.html`):**
  - Host a local/web dashboard mirroring GitHub’s interface to track project milestones, module completion status (C#, Rust, C++, Python), QA test results, and docker container status.

---

## 12. QA Audit, Stable Build & Release Deployment Protocol

To ensure software stability, zero runtime regressions, and a production-ready distribution package:

- **Automated QA & Test Suite Execution:**
  - Execute full unit and integration test suites across all core modules:
    - **C# / Avalonia (`desktop-host`):** `dotnet test`
    - **Rust Core (`rust-core`):** `cargo test --release`
    - **C++ Extensions (`cpp-extensions`):** `ctest` / CMake test targets
    - **Python Backend (`python-backend`):** `pytest`
  - Audit FFI bridges for pointer safety, memory leaks, and exception propagation.
  - Verify that unhandled exceptions, UI freezes, and FFI memory leaks are absent; confirm that `logs/neoveg.log` and in-app error capture operate correctly.
- **Production Executable Generation (`.exe`):**
  - Build standalone, optimized Release binaries with `dotnet publish desktop-host/Neoveg.csproj -c Release -r win-x64 --self-contained true`.
  - Bundle all required native dependencies (`neoveg_core.dll`, `neoveg_cpp_engine.dll`), `Assets/`, the Bootstrapper/Local AppCache scripts, and prerequisite installers.
- **Release Packaging & Cloud Mirroring:**
  - Package the final distribution as `dist/Neoveg_v1.0.0_win-x64/` and `dist/Neoveg_v1.0.0_win-x64.zip`, including `RELEASE_NOTES.md` and `AUTHORS.md`.
  - Mirror the project and release package to `G:\Meu Drive\vp-rev-eng\teste\teste2` via `scripts/sync_gdrive.ps1`.

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
  - **Usability:** Historical and contemporary VEGAS UI fidelity, multi-language runtime localization (`pt-BR` default across 8 locales), accessible keyboard shortcuts, and visual diagnostic feedback.
  - **Portability:** Codebase must build natively on Windows (`win-x64`), Linux (`linux-x64`), and macOS (`osx-x64`/`osx-arm64`) using abstraction layers (`OperatingSystem.IsWindows()`, `OperatingSystem.IsLinux()`, `OperatingSystem.IsMacOS()`, `Path.Combine`) without hardcoded Windows-only paths.
  - **Maintainability:** Code structure must maintain clean modular isolation between C# Avalonia, Rust FFI, C++ Extensions, and Python backend.
- **ISO/IEC 29119 (Software Testing Standard):**
  - All features must pass automated unit, integration, and headless UI tests (`Avalonia.Headless`) before being merged or packaged for release, with QA reports (`scripts/run_qa_suite.ps1` and `docs/index.html`) categorized by ISO/IEC 25010 quality pillars.
- **ISO/IEC 27001 / Memory Safety Assurance (Security & Memory Integrity):**
  - Dynamic memory allocations across FFI boundaries must use explicit ownership and cleanup models (AddressSanitizer and LeakSanitizer validated, explicit `neoveg_free_buffer` / `neoveg_cpp_free_buffer` / `IDisposable` deallocators, and 64-bit integer overflow guards).

---

## 17. Cross-Platform OS Detection & Launcher Protocol

To guarantee seamless execution across Windows, Linux, and macOS:

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
