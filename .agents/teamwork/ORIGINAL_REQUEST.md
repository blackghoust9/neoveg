# Original User Request

## Initial Request — 2026-11-06T10:41:49Z

Complete the interrupted recording workflow and resolve active worktree modifications, then implement the requested production feature extensions across neoveg on branch `preview/version-2.1.0`.

Working directory: C:\Users\paulomatos\Documents\GitHub\neoveg
Integrity mode: demo
The team must NOT: copy code from existing open-source projects for core logic

## Requirements

### R1. Interrupted Recording Task & Worktree Stabilization
- Resolve pending lint/test modifications in the worktree (`crates/neoveg_aaf/src/tests.rs`, `crates/neoveg_video/src/raw.rs`, `crates/neoveg_video/src/still.rs`).
- Complete the interrupted recording workflow: input device selection, track arming, and recording with monitoring ("Escolha do dispositivo de entrada, armar faixas e gravação com monitoramento").

### R2. Core Feature Extensions
Implement the requested feature extensions in the order specified:
- Transitions: categories, presets, thumbnails, and 3D transitions; GPU video effects and OFX support.
- Color grading per event, color wheels, and OpenColorIO (OCIO) integration.
- Motion tracking, masks, and Smart Mask.
- Project properties (resolution, FPS, aspect ratio, color space), templates, and media profiles.
- Project Media window (list/thumbnail views, filters, search, properties) and Explorer.
- Make Movie rendering with presets and pre-rendering/cache with color-coded preview ruler (green/yellow/red).
- AI features: text-based editing, speech-to-text, upscale, auto reframe.
- Standardized alerts and dialogs (missing media, codec, disk, GPU, plug-in) matching results.md §10.
- Smart proxies: automatic switching between original and proxy based on preview quality, with final render using the original (VP12).
- Color Match across clips and S-Log/ACES workflow with OpenColorIO per media and project (VP12).
- L-cut and J-cut editing, simultaneous audio and video fades, and event synchronization (sync links) (VP12/VP13).
- Multicam editing (synchronize, create multicam event, switch cameras) and takes as alternative event versions (VP13).

### R3. Repository, Architecture & Infrastructure Constraints
- Pure clean-room Rust implementation compliant with `MIT OR Apache-2.0` (`deny.toml`), without GPL/LGPL dependencies, and zero third-party trademarks in runtime strings.
- Strict 8 GB RAM execution budget: run all Cargo commands sequentially with `-j 1` and `--test-threads=1`. NEVER execute `cargo test -p neoveg_ui` (verify `neoveg_ui` strictly using `cargo check` and `cargo clippy`).
- Pure software/CPU fallbacks and mock implementations must be provided for GPU effects, OFX, and AI features so all tests and builds execute within CI/local environments without requiring dedicated GPU hardware or live external AI endpoints.
- Synchronize translation catalogs (`Assets/i18n/strings/{de-DE,en-US,es-ES,fr-FR,ja-JP,ru-RU,zh-CN}.json`), `scripts/parity_matrix.py` (`--check`), and `README.md`.

## Acceptance Criteria

### Automated Verification & Integrity
- [ ] `cargo check -j 1` and `cargo clippy -j 1` pass cleanly without errors or warnings across all workspace crates, including `neoveg_ui`.
- [ ] `cargo test -j 1 --test-threads=1` passes for all workspace crates excluding `neoveg_ui`.
- [ ] `cargo deny check` passes with no license violations or unapproved licenses.
- [ ] `python scripts/parity_matrix.py --check` passes without discrepancies.
- [ ] All translation files in `Assets/i18n/strings/` (`de-DE`, `en-US`, `es-ES`, `fr-FR`, `ja-JP`, `ru-RU`, `zh-CN`) are synchronized with zero missing keys.
- [ ] Worktree files `crates/neoveg_aaf/src/tests.rs`, `crates/neoveg_video/src/raw.rs`, and `crates/neoveg_video/src/still.rs` pass linting and unit tests cleanly.
- [ ] Automated tests verify input device selection, track arming, and recording with monitoring.
- [ ] Automated tests verify transitions (presets, categories, thumbnails, 3D), color wheels/OCIO, motion tracking/masks, project properties, project media/explorer, rendering/cache, AI fallbacks, standardized alerts (§10), smart proxies, color match, L/J-cuts, sync links, and multicam/takes.
- [ ] No third-party trademarks exist in runtime strings or user-visible text.


## Follow-up — 2026-11-06T13:16:12Z

User follow-up directive / scope expansion for this active Teamwork run (incorporate into `PROJECT.md`, Generation 2 succession, and all current/remaining milestones without discarding completed M1/M2 work):

**Task**: veja todos os itens desmarcados e veja o que ainda precisa ser feito do readme para o programa, veja se o programa tem um bom comportamento com o uso somente da cpu e em hardware limitados, veja também os problemas de comportamentos da interface gráfica com a lógica do programa e corrija os erros dela, também verifique os erros da build para que não tenha nenhum tipo de erro durante o processo, todo o programa tem que obedecer a semântica do VEGAS Pro, principalmente na interface gráfica e nos comandos do teclado, veja os manuais em C:\Users\paulomatos\Documents\GitHub\neoveg\manuais, se quiser pode pesquisar melhor sobre o funcionamento do VEGAS na internet, veja quais funcionalidades afetem a semântica do VEGAS Pro, se não afetar e for bom para o programa pode colocar, se não evite colocar e coloque as notas no readme, sempre faça os commites e pushes a cada alteração nova no projeto, faça uma atualização geral do readme de tudo que já foi feito

**Additional Context**:
- Milestones 1 and 2 are already completed / in Gate 2 review on branch `preview/version-2.1.0` in `C:\Users\paulomatos\Documents\GitHub\neoveg`.
- Please fold all points from the user's new directive into the active execution plan (`PROJECT.md` and the Generation 2 Orchestrator handoff):
  1. Audit ALL remaining unchecked items (`[ ]`) in `README.md` and see everything that still needs to be done for the program.
  2. Verify and optimize CPU-only execution and behavior on limited/low-spec hardware (memory/CPU budgets, fallbacks, responsiveness).
  3. Audit GUI (`neoveg_ui`) behavior integration with engine/core logic (`neoveg_engine`, `neoveg_audio`, `neoveg_video`, `neoveg_common`) and fix any wiring, state synchronization, or UI behavior bugs.
  4. Verify the full build process (Cargo workspace build & `build_portable.py` checks, strictly sequential `-j 1` and `--test-threads=1`, never running `cargo test -p neoveg_ui`) so there are zero build errors or warnings.
  5. Strictly obey VEGAS Pro semantics across the entire program — especially GUI behavior and keyboard shortcuts — consulting the reference manuals in `C:\Users\paulomatos\Documents\GitHub\neoveg\manuais` (and web documentation where needed). Evaluate features against VEGAS Pro semantics: include beneficial features that preserve VEGAS Pro semantics; avoid features that conflict with VEGAS Pro semantics and document clear notes in `README.md` explaining why. (Keep runtime UI strings free of third-party trademarks per repository policy while documenting VEGAS Pro parity/notes in `README.md`.)
  6. Commit and push (`git commit` and `git push`) incrementally after each completed milestone / new change (including committing/pushing completed M1 & M2 changes once Gate 2 passes).
  7. Perform a comprehensive update of `README.md` (and `scripts/parity_matrix.py --check` + 7 i18n catalogs) reflecting everything that has been completed and any semantic notes.


## Follow-up — 2026-11-06T15:00:31Z

User focus directive update for this active Teamwork run:

**Task**: apenas siga o roteiro, as caixas desmarcadas do readme, até terminar todas elas, foque somente no que falta a colcluir no readme

**Additional Context**:
- Keep the team strictly focused ONLY on completing the unchecked checklist items (`- [ ]`) in `README.md` until all of them are completed and checked off (`- [x]`), without spending cycles on extra tasks outside the unchecked `README.md` roadmap items (while still maintaining build/test/clippy/i18n/parity_matrix cleanliness and incremental git commits/pushes on `preview/version-2.1.0`).
