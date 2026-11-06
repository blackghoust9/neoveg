# Task Assignment: challenger_m5_1

## Objective
Adversarial empirical stress testing of Features 24–29:
- Media Profiles reverse order matching with edge cases, multiple overlapping rules, corrupt ini files, and floating point FPS epsilon boundaries.
- 46 Project Templates parsing, custom templates persistence, and immutability rejection on overwrite/deletion.
- Media Bins cycle prevention, deep folder nesting, duplicate auto-renaming, and Smart Search Bin dynamic predicate filtering.
- Media Pool 23 detail column sorting (ascending/descending, numeric vs alphanumeric), empty media lists, and relinking missing media.

Create test suite `crates/neoveg_common/tests/adversarial_m5_properties_profiles.rs` and execute via `cargo test -p neoveg_common --test adversarial_m5_properties_profiles -j 1 -- --test-threads=1`.
NEVER execute `cargo test -p neoveg_ui`.


## 2026-11-06T17:36:59Z
You are challenger_m5_1, Empirical Adversarial Verifier for Milestone 5 (Features 24–29).
Your working directory is: C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\challenger_m5_1\
Your parent orchestrator conversation ID is: c220c328-9551-48e1-a35b-23e05589d782

MANDATORY INPUTS:
- C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\ORIGINAL_REQUEST.md
- C:\Users\paulomatos\Documents\GitHub\neoveg\PROJECT.md
- C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\worker_m5_1\handoff.md
- C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\spec_miner_m5_1\handoff.md

TASK:
Write an adversarial stress test suite in `crates/neoveg_common/tests/adversarial_m5_properties_profiles.rs` challenging:
1. Media Profiles reverse order rule matching with multiple overlapping rules, corrupt ini files, negative/infinite numbers, and 0.005 FPS boundary tolerances.
2. 46 Project Templates parsing, custom templates persistence, and strict immutability rejection on overwrite/deletion.
3. Media Bins cycle prevention, deep folder nesting, duplicate auto-renaming, and Smart Search Bin dynamic predicate filtering with complex search conditions.
4. Media Pool 23 detail column sorting (ascending/descending, numeric vs alphanumeric), empty media lists, and relinking missing media.

EXECUTION & VERIFICATION:
- Run: `cargo test -p neoveg_common --test adversarial_m5_properties_profiles -j 1 -- --test-threads=1`
- NEVER execute `cargo test -p neoveg_ui`.
- Report findings and formal verdict (APPROVE or REQUEST_CHANGES) in `handoff.md` and notify orchestrator via `send_message`.
