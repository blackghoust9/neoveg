# Neoveg - Developer Dependencies & Environment Setup

Neoveg is a pure Rust Cargo workspace. **The only build dependency is the official Rust toolchain (`rustup` / `cargo`).** .NET, CMake, C/C++ compilers and Python are no longer required to build, test or run the project.

---

## 1. Toolchain

| Tool | Minimum Version | Usage | Installation |
| :--- | :--- | :--- | :--- |
| **Rust (`rustup` / `cargo`)** | 1.80+ (stable) | Everything: `crates/neoveg_*` | Windows: `winget install --id Rustlang.Rustup --exact` / Linux & macOS: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| `clippy`, `rustfmt` | bundled | Lint and formatting | `rustup component add clippy rustfmt` |

Optional tools used by QA and coverage:

```bash
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --locked      # cargo llvm-cov --workspace
```

Optional, only for native video decoding (`cargo build -p neoveg_video --features ffmpeg`). `ffmpeg-next` 7.1 needs three things:

| Requirement | Version | Windows install | Environment |
| :--- | :--- | :--- | :--- |
| FFmpeg **shared** build with `include/` and `lib/` | **7.1.x** (not 8/9) | `winget install --id BtbN.FFmpeg.LGPL.Shared.7.1 --exact` | `FFMPEG_DIR` = the extracted `ffmpeg-n7.1...-win64-lgpl-shared-7.1` folder |
| libclang (bindgen) | **LLVM 19.x** (LLVM 22+ makes `ffmpeg-sys-next` 7.1.3 fail with `attempt to compute 1_usize - N_usize`) | `winget install --id LLVM.LLVM --exact --version 19.1.7` | `LIBCLANG_PATH` = `C:\Program Files\LLVMin` |
| `ffmpeg`'s `bin/` DLLs on `PATH` at run time | 7.1.x | added by the winget package | - |

Linux: `sudo apt install libavcodec-dev libavformat-dev libavutil-dev libswscale-dev libclang-dev pkg-config`. macOS: `brew install ffmpeg@7 llvm pkg-config`.

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
