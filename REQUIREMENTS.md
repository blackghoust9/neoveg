# Neoveg - Developer Dependencies & Environment Setup

Neoveg is a pure Rust Cargo workspace. **The only build dependency is the official Rust toolchain (`rustup` / `cargo`).** .NET, CMake, C/C++ compilers and Python are no longer required to build, test or run the project.

---

## 1. Toolchain

| Tool | Minimum Version | Usage | Installation |
| :--- | :--- | :--- | :--- |
| **Rust (`rustup` / `cargo`)** | 1.80+ (stable) | Everything: `crates/neoveg_*` | Windows: `winget install --id Rustlang.Rustup --exact` / Linux & macOS: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| `clippy`, `rustfmt` | bundled | Lint and formatting | `rustup component add clippy rustfmt` |

Optional, only for native video decoding (`cargo build -p neoveg_video --features ffmpeg`): the FFmpeg development libraries (`ffmpeg-next` links against them).

---

## 2. System Libraries (runtime windowing, GPU and audio)

### Windows
No extra packages beyond a current graphics driver. The default MSVC target (`x86_64-pc-windows-msvc`) needs the Visual Studio *Build Tools* C++ linker that `rustup` offers to install; the `-gnu` target works with MinGW.

### Linux (Debian / Ubuntu)
```bash
sudo apt update && sudo apt install -y \
    build-essential pkg-config \
    libasound2-dev libudev-dev \
    libx11-dev libxcursor-dev libxrandr-dev libxi-dev libxkbcommon-dev libwayland-dev \
    libvulkan-dev
```

### macOS
```bash
xcode-select --install
```

---

## 3. Build, Test and Run

```bash
cargo build --workspace --release
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
cargo run --release -p neoveg_ui
```

Logs are written to `logs/neoveg.log` and `logs/crash.log` next to the executable.

---

## 4. Legacy Directories

`desktop-host/` (C#), `rust-core/` (FFI crate, excluded from the workspace), `cpp-extensions/` and `python-backend/` belong to the previous architecture. They are kept only as a porting reference and are not built by the workspace. Do not add features to them.
