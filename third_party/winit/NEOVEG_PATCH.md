# winit 0.30.13 with a Neoveg patch

Copy of `winit` 0.30.13 (Apache-2.0, see `LICENSE`) with `examples/`, `tests/` and `docs/` left out. Wired in with
`[patch.crates-io]` in the root `Cargo.toml`. The changes are marked `Neoveg patch` in
`src/platform_impl/windows/event_loop.rs` and `window_state.rs`.

**Problem:** a window dragged between two monitors of different scale (100% and 125%) shook: winit resizes
the window by the scale ratio on every `WM_DPICHANGED` and moves it to keep the cursor ratio, which puts it
back on the first monitor, which changes the scale again, and so on.

**Change:** during the move loop (`WM_ENTERSIZEMOVE` to `WM_EXITSIZEMOVE`) `WM_DPICHANGED` only updates the
scale: size and position stay. When the window is dropped (`WM_EXITSIZEMOVE`) it is given the logical size
it had when the move began, at the scale of the monitor it landed on (only if the user did not resize it).

Remove this copy (and the `[patch]`) when upstream fixes the loop.
