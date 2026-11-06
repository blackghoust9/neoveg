# Handoff Report: Workspace Architecture & Compliance Explorer

**Agent**: Explorer Subagent (`explorer_survey_3`)  
**Parent Conversation ID**: `c220c328-9551-48e1-a35b-23e05589d782`  
**Date**: 2026-11-06  
**Working Directory**: `C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\explorer_survey_3`  
**Handoff Type**: Hard (Task Complete)

---

## 1. Observation

### 1.1 Crate Layout and Architecture
Inspection of root `Cargo.toml` and crate manifests identifies exactly 14 workspace crates:

| Crate | Crate Type | Core Responsibilities | Dependencies | Public API Highlights |
|---|---|---|---|---|
| `neoveg_common` | `rlib` | Core data models, math, timecode, project structure, keymaps, command palette, i18n, preferences, themes | `tracing`, `tracing-subscriber`, `serde`, `serde_json`, `sysinfo`, `whoami`, `time` | `project::*`, `timecode::*`, `commands::*`, `command_palette::*`, `i18n::{tr, tr_args, tr_text, LANGUAGES}`, `preferences::*`, `keymap::*`, `grade::*` |
| `neoveg_text` | `rlib` | Clean-room typography, font loading, HarfBuzz shaping, rasterization, glyph masks | `skrifa`, `harfrust`, `unicode-bidi`, `unicode-linebreak`, `unicode-segmentation` | `fonts::*`, `layout::*`, `mask::*`, `render::*`, `sfnt::*` |
| `neoveg_media` | `rlib` | Pure Rust container demuxing and parsing: ISOBMFF/MP4, MXF, bitstream operations | `thiserror` | `isobmff::*`, `mxf::*`, `bitstream::*` |
| `neoveg_h264` | `rlib` | Pure clean-room H.264 video codec: CABAC, CAVLC, intra/inter prediction, transform, rate control | `neoveg_media`, `thiserror`, `rayon` | `cabac::*`, `cavlc::*`, `deblock::*`, `ratecontrol::*`, `slice::*`, `syntax::*` |
| `neoveg_aac` | `rlib` | Pure clean-room AAC audio codec: MDCT, Huffman coding, ICS, TNS | `neoveg_media`, `thiserror`, `rayon` | `decoder::*`, `encoder::*`, `config::*`, `huffman::*`, `mdct::*` |
| `neoveg_cfb` | `rlib` | Compound File Binary (OLE / CFBF structured storage), used by AAF | (Zero external dependencies) | `read::*`, `write::*`, `tests::*` |
| `neoveg_aaf` | `rlib` | Advanced Authoring Format (AAF) parser and writer for NLE exchange | `neoveg_cfb`, `serde`, `thiserror` | `read_aaf`, `write_aaf`, `model::*`, `time::*` |
| `neoveg_ofx` | `rlib` | Open Effects (OFX) plugin host ABI, isolated subprocess runner | `libloading`, `serde`, `serde_json` | `client::*`, `describe::*`, `suites::*`, `abi::*`, `report::*` |
| `neoveg_ofx_sample` | `cdylib` | Clean-room sample OFX plugin (`com.neoveg.sample.invert`) for host testing | `neoveg_ofx` | OFX C ABI entry points (`OfxGetNumberOfPlugins`, `OfxGetPlugin`) |
| `neoveg_vst3_sample` | `cdylib` | Clean-room sample VST3 gain plugin for audio host testing | `vst3` | VST3 COM exports (`GetPluginFactory`, `InitDll`, `ExitDll`) |
| `neoveg_audio` | `rlib` | Audio pipeline, DSP, LUFS meter, true peak, mixer, recording, VST3/CLAP hosting, Whisper AI transcription | `neoveg_common`, `neoveg_media`, `serde`, `serde_json`, `tracing`, `symphonia`, `cpal`, `nnnoiseless`, `clack-host`, `clack-extensions`, `vst3`, `libloading`, `candle-core`, `candle-nn`, `candle-transformers` | `record::{Recorder, RecordOptions, MonitorStatus, devices}`, `meter::*`, `mixer::*`, `whisper::{WhisperEngine, TranscriptSegment}`, `vst3_isolated::*`, `clap_host::*` |
| `neoveg_video` | `rlib` | Video engine, WGPU rendering, 36 CPU/GPU effects, transitions, color grading, LUT, RAW/CinemaDNG, optical flow | `neoveg_common`, `neoveg_text`, `neoveg_media`, `neoveg_h264`, `neoveg_aac`, `tracing`, `serde`, `serde_json`, `bytemuck`, `pollster`, `image`, `wgpu`, `ffmpeg-next` | `vfx::{VideoEffect, apply_chain_cpu}`, `vfx_gpu::*`, `transitions::*`, `grading::*`, `lut::*`, `raw::*`, `still::*`, `optical_flow::*` |
| `neoveg_engine` | `rlib` | Core NLE timeline model, edit ops, ripple editing, snapping, takes, track arming, interchange (EDL, FCP7, FCPXML, OTIO, AAF) | `neoveg_common`, `neoveg_aaf`, `serde`, `serde_json`, `roxmltree`, `zip` | `timeline::*`, `command::*`, `ops::*`, `track_arm::*`, `takes::*`, `fcp7::*`, `fcpxml::*`, `edl::*`, `otio::*`, `aaf::*` |
| `neoveg_ui` | `rlib` / `bin` | egui/eframe desktop application, docking windows, panels, dialogs, transport, waveform/timeline renderers | `neoveg_common`, `neoveg_text`, `neoveg_audio`, `neoveg_video`, `neoveg_engine`, `tracing`, `eframe`, `egui_dock`, `serde`, `serde_json`, `egui_extras`, `rfd`, `pollster`, `image` | `app::*`, `dock::*`, `dialogs::*`, `icons::*`, `theme::*` |

### 1.2 License Constraints & `deny.toml`
- Root `deny.toml`:
  - Line 24: Allowed licenses: `MIT`, `Apache-2.0`, `Apache-2.0 WITH LLVM-exception`, `BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `Zlib`, `0BSD`, `BSL-1.0`, `CC0-1.0`, `Unlicense`, `BlueOak-1.0.0`, `Unicode-3.0`, `Unicode-DFS-2016`, `MPL-2.0`, `OFL-1.1`, `WTFPL`, `LGPL-2.1-or-later` (for dynamically linked FFmpeg).
  - Line 46: `copyleft = "deny"`, `allow-osi-fsf-free = "neither"`, `default = "deny"`. Prohibits all AGPL, GPL-2.0, GPL-3.0 crates.
  - Line 65: Banned crates: `openssl-src` ("Use OS native TLS or rustls").
  - Line 55: Clarification exception: `Ubuntu-font-1.0` for `epaint_default_fonts`.
- Lockfile inspection (`Cargo.lock`):
  - 606 total package entries.
  - Zero GPL or AGPL packages found.
  - `cargo-deny` CLI binary is not currently installed in the host's PATH or `~/.cargo/bin` (`cargo-llvm-cov.exe` is present).

### 1.3 Parity Matrix Validation (`scripts/parity_matrix.py`)
- Direct execution:
  ```powershell
  python scripts/parity_matrix.py --check
  ```
  Result: Exit code 0, output: `872 comandos da varredura; 0 sem linha própria (a classificar)`.
- Mechanism:
  - If `VegasScan/22_Functions.md` exists, parses 872 scanned reference commands, matches against regex patterns in `ROWS`, and verifies that zero commands fall through to `FALLBACK` (`aux.solto`).
  - If the scan file is absent, falls back to `docs/research/parity_matrix.json`.
  - When run without `--check`, updates `docs/research/parity_matrix.json` and renders `docs_obsidian/ui_references/matriz_de_paridade.md`.
- Key status mapping in `ROWS` for features specified in `ORIGINAL_REQUEST.md`:
  - `fer.ia`: "Ferramentas de IA" -> `previsto` ("roteiro (fase de IA; Whisper)", "P3")
  - `fer.cor_mov`: "Modo de gradação de cor e rastreamento de movimento" -> `parcial` ("correção de cor primária e LUT feitas; gradação por evento e rastreamento não", "P2")
  - `fer.prerender`: "Pré-renderização seletiva e prévia dinâmica em RAM" -> `previsto` ("roteiro: barra verde, amarela e vermelha", "P2")
  - `fer.multicam`: "Multicâmera: sincronizar, criar, expandir e editar" -> `previsto` ("roteiro (README)", "P3")
  - `edit.takes`: "Takes do evento" -> `previsto` ("`edit.take.*` têm atalho, sem ação", "P2")
  - `exi.explorador`: "Janela do explorador de arquivos" -> `previsto` ("roteiro (Explorer)", "P2")
  - `exi.transicoes`: "Janela de transições" -> `parcial` ("aba Transições... faltam miniaturas, pré-ajustes e favoritos", "P2")
  - `aux.midia_colunas` & `aux.midia_acoes`: `previsto`

### 1.4 Translation Catalogs (`Assets/i18n/strings/*.json`)
- Analysis across all 7 catalogs (`de-DE`, `en-US`, `es-ES`, `fr-FR`, `ja-JP`, `ru-RU`, `zh-CN`):
  - Every file contains exactly **1,795 keys**.
  - All 7 files share the exact same set of 1,795 keys (0 missing keys, 100% synchronized).
  - Catalogs are compiled into `neoveg_common::i18n` via `include_str!`.
  - Source text in code is `pt-BR`. `tr(key)` returns the translated string in the active locale, or `key` when in `pt-BR` or if the key is not in the catalog.
  - Unit tests in `neoveg_common/src/i18n.rs` (`a_translation_keeps_the_markers_of_its_text`) enforce that every translation string has the identical count of `{}` format specifiers as the source text.

### 1.5 Fallbacks & Mock Patterns for GPU, OFX, and AI
1. **GPU Effects Fallbacks**:
   - `crates/neoveg_video/src/vfx.rs` implements `apply_chain_cpu(&mut FrameBuffer, &[VideoEffect])` covering all 36 effects in software.
   - `crates/neoveg_video/src/vfx_gpu.rs` verifies bit-exact/threshold equivalence (`assert!(max_difference(&cpu, &on_gpu) <= 1)`).
   - If GPU initialization fails, memory limits are exceeded, or an effect/transition is CPU-only (e.g. 3D transitions returning `Err("esta transicao roda na CPU")`), the video worker automatically falls back to CPU execution.
2. **OFX Host Mocks**:
   - `crates/neoveg_ofx` isolates plugin execution into a child process (`neoveg_ofx_host`).
   - `crates/neoveg_ofx_sample` provides a mock cdylib plugin (`com.neoveg.sample.invert`) to run end-to-end host testing without foreign binaries.
3. **Audio VST3 Mocks**:
   - `crates/neoveg_vst3_sample` provides a pure-Rust mock gain plugin for testing out-of-process VST3 hosting (`neoveg_audio/src/vst3_isolated.rs`).
4. **AI Features (Whisper) Mocks**:
   - `crates/neoveg_audio/src/whisper.rs` implements `WhisperEngine::from_components(...)` allowing tests to construct in-memory models using synthetic tokenizer JSON and synthetic tiny weights on `Device::Cpu`.
   - Unit tests (`synthetic_tiny_whisper_model_runs_encoder_decoder_and_honors_cancellation`) execute in pure Rust on CPU in CI without network downloads or GPU hardware.

### 1.6 Trademark Restrictions & Runtime Audit
Automated regex scan across all string literals in `crates/` and entries in `Assets/i18n/` identified exactly 9 third-party trademark violations:
1. `crates/neoveg_ui/src/app.rs`:
   - Line 1289 & Line 1307: `.add_filter("Final Cut Pro 7 XML", &["xml"])`
   - Line 1366 & Line 1384: `.add_filter("Final Cut Pro FCPXML", &["fcpxml"])`
   - Line 10316: `ui.button("XML (Final Cut Pro 7)…")`
   - Line 10320: `ui.button("FCPXML (Final Cut Pro)…")`
   - Line 10355: `egui::Button::new("XML (Final Cut Pro 7)…")`
   - Line 10362: `egui::Button::new("FCPXML (Final Cut Pro)…")`
   - Line 12356: `ui.label(tr("Exportar a linha do tempo para AAF (compatível com Avid, DaVinci Resolve, Premiere Pro e Pro Tools)."));`
2. `Assets/i18n/strings/*.json`:
   - Line 1692 in all 7 translation files translates the line 12356 string with literal brand names ("Avid, DaVinci Resolve, Premiere Pro, Pro Tools").

### 1.7 Standardized Alerts & Dialogs (`results.md §10`)
- Located `results.md` (3.08 MB) at repository root.
- Section 10 ("Alertas, mensagens, erros e confirmações") contains 240 cataloged dialog/alert strings categorized into:
  - `§10.1`: Missing media / offline media (25 entries)
  - `§10.2`: Codecs / formats / decoding (28 entries)
  - `§10.3`: Render / prerender / Make Movie (29 entries)
  - `§10.4`: Memory / disk / resources (7 entries)
  - `§10.5`: GPU / acceleration (5 entries)
  - `§10.6`: Plugins / effects / VST (31 entries)
  - `§10.7` to `§10.13`: Scripts, version/license, project save/load, audio/devices, and generic message dialogs.

---

## 2. Logic Chain

1. **Workspace Architecture**:
   - The workspace enforces strict architectural layering: `neoveg_common` (core models) <- `neoveg_media`, `neoveg_text`, `neoveg_cfb` <- `neoveg_h264`, `neoveg_aac`, `neoveg_aaf`, `neoveg_ofx` <- `neoveg_audio`, `neoveg_video` <- `neoveg_engine` <- `neoveg_ui`.
   - High-level business logic is decoupled from GUI presentation. Business rules reside as pure functions in `neoveg_engine`, while GUI rendering and input handling reside in `neoveg_ui`.
2. **License Compliance**:
   - The repository strictly dual-licenses under `MIT OR Apache-2.0`.
   - `deny.toml` is configured to reject copyleft (`copyleft = "deny"`) and specifically bans `openssl-src`.
   - Dynamic linking to system FFmpeg is permitted under `LGPL-2.1-or-later`.
   - Inspection of `Cargo.lock` verified 0 prohibited or copyleft packages.
3. **Parity Matrix State**:
   - `scripts/parity_matrix.py --check` currently succeeds (0 loose items).
   - However, when developers complete functional milestones (e.g., transitions presets, takes, color grading, AI tools), `scripts/parity_matrix.py` lines in `ROWS` must be updated from `previsto`/`parcial` to `feito`, and the script re-run to regenerate `matriz_de_paridade.md` and `parity_matrix.json`.
4. **i18n Consistency**:
   - 1,795 keys are perfectly matched across all 7 locale files.
   - Any new feature that introduces user-visible text must:
     a) Use `tr("Chave em Português")` or `tr_args("...", &[...])`.
     b) Add the key and translated values to all 7 files in `Assets/i18n/strings/*.json`.
     c) Keep `{}` argument placeholders strictly identical between key and translations.
5. **Clean-Room Trademark Compliance**:
   - While documentation comments reference historical manuals for algorithmic fidelity, user-facing runtime strings must contain zero third-party registered trademarks.
   - The 9 occurrences found in `app.rs` and the corresponding line in `Assets/i18n/strings/*.json` constitute trademark non-compliance and should be replaced with generic terminology (e.g. "XML de intercâmbio", "FCPXML", "estações de áudio e vídeo de intercâmbio").
6. **Execution Budget & CPU Fallback**:
   - System memory constraints require `-j 1` and `--test-threads=1` to prevent OOM linking failures.
   - `cargo test -p neoveg_ui` must never be run due to excessive symbol allocation in `eframe`/`egui`/`wgpu` test linking; `neoveg_ui` is verified exclusively via `cargo check` and `cargo clippy`.
   - CI and test reliability requires pure-CPU fallback for all GPU shaders and synthetic mocks for OFX, VST3, and AI.

---

## 3. Caveats

1. **`cargo-deny` Executable**:
   - `cargo-deny` binary is not installed in the local Windows environment. Compliance was verified by inspecting `deny.toml` configuration rules and auditing `Cargo.lock` (606 crates) via automated script.
2. **Cargo Lock Contention**:
   - Only one `cargo` command can hold the workspace build lock at a time. Parallel subagents running `cargo test` or `cargo check` will queue on `Blocking waiting for file lock on build directory`.
3. **`results.md` Size**:
   - `results.md` is 3.08 MB and contains 33,594 lines. It serves as an offline specifications reference; implementations of dialogs in §10 should extract specific message templates on demand.

---

## 4. Conclusion

1. **Architecture Ready**: The 14-crate layout is well-structured, modular, and enforces clean layering.
2. **Licensing Clean**: Workspace is compliant with `MIT OR Apache-2.0`, zero GPL/AGPL dependencies.
3. **Parity Check Passing**: `python scripts/parity_matrix.py --check` passes cleanly with 872 commands classified.
4. **i18n Complete**: All 7 language files have 1,795 keys with 0 missing entries.
5. **Fallbacks Proven**: Clear patterns exist for GPU CPU-fallback (`apply_chain_cpu`), OFX/VST3 sample mocks (`neoveg_ofx_sample`, `neoveg_vst3_sample`), and AI synthetic in-memory fixtures (`WhisperEngine::from_components`).
6. **Remediation Required**:
   - 9 trademark occurrences in `crates/neoveg_ui/src/app.rs` and `Assets/i18n/strings/*.json` must be replaced with generic interchange terms.
   - Feature extensions must maintain CPU fallbacks, update `scripts/parity_matrix.py`, and synchronize all 7 i18n JSON files.

---

## 5. Verification Method

To independently reproduce and verify every finding in this report:

1. **Crate Layout & Dependencies**:
   ```powershell
   python .agents\teamwork\explorer_survey_3\map_crates.py
   ```
2. **Licensing & Dependencies**:
   ```powershell
   python .agents\teamwork\explorer_survey_3\check_lockfile.py
   ```
3. **Parity Matrix Check**:
   ```powershell
   python scripts/parity_matrix.py --check
   ```
4. **i18n Key Synchronization**:
   ```powershell
   python -c "import json, glob; files = glob.glob('Assets/i18n/strings/*.json'); sets = {f: set(json.load(open(f, encoding='utf-8')).keys()) for f in files}; all_k = set().union(*sets.values()); print(f'Total: {len(all_k)} keys'); [print(f'{f}: {len(s)} keys (missing: {len(all_k - s)})') for f, s in sets.items()]"
   ```
5. **Trademark Compliance Audit**:
   ```powershell
   python .agents\teamwork\explorer_survey_3\check_trademarks.py
   python .agents\teamwork\explorer_survey_3\check_assets_trademarks.py
   ```
6. **Execution Budget Verification**:
   ```powershell
   cargo check -p neoveg_common -j 1
   cargo check -p neoveg_engine -j 1
   ```
