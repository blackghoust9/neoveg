## 2026-11-06T11:31:27Z
You are a Worker subagent (Worker M1: Recording Stabilization & Trademark Sanitization).
Your working directory is: C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\worker_m1_1
Your original request file is: C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\ORIGINAL_REQUEST.md

MANDATORY FIRST STEP: Read C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\ORIGINAL_REQUEST.md completely.
Also read:
- C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\explorer_survey_1\handoff.md
- C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\explorer_survey_3\handoff.md
- C:\Users\paulomatos\Documents\GitHub\neoveg\PROJECT.md

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Write ownership:
- crates/neoveg_ui/src/app.rs
- Assets/i18n/strings/*.json
- crates/neoveg_audio/src/record.rs and audio tests

Tasks:
1. Trademark Sanitization: Replace all 9 third-party vendor trademark occurrences in `crates/neoveg_ui/src/app.rs` (lines 1289, 1307, 1366, 1384, 10316, 10320, 10355, 10362, 12356) and `Assets/i18n/strings/{de-DE,en-US,es-ES,fr-FR,ja-JP,ru-RU,zh-CN}.json` (line 1692 / key for AAF export label). Replace vendor brand names ("Final Cut Pro 7 XML", "Final Cut Pro FCPXML", "Avid, DaVinci Resolve, Premiere Pro e Pro Tools") with generic terminology ("XML de intercâmbio", "FCPXML", "estações de áudio e vídeo de intercâmbio"). Ensure `{}` count and keys across all 7 JSON files remain 100% matched and synchronized.
2. Complete and verify the interrupted recording workflow automated test: implement an automated test verifying input device selection, track arming, and recording with monitoring in `neoveg_audio` (using mock loopback or memory ring buffer monitoring) ensuring headless CI environments test the entire flow cleanly.
3. Verify that `crates/neoveg_aaf/src/tests.rs`, `crates/neoveg_video/src/raw.rs`, and `crates/neoveg_video/src/still.rs` pass linting and unit tests cleanly.
4. Execute builds and tests under strict budget:
   - `cargo check -j 1 --workspace`
   - `cargo clippy -j 1 --workspace`
   - `cargo test -j 1 --test-threads=1 --workspace --exclude neoveg_ui`
   - NEVER execute `cargo test -p neoveg_ui`!
   - `python scripts/parity_matrix.py --check`
5. Write your complete handoff report to `handoff.md` in your working directory and notify the orchestrator via `send_message`.
