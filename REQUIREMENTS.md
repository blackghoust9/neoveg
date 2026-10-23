# Neoveg - Developer Dependencies & Environment Setup

This document specifies all required SDKs, toolchains, and runtime libraries necessary to build, run, and contribute to Neoveg across Windows, Linux, and macOS.

---

## 1. Core Language Toolchains & SDKs

| Tool / SDK | Minimum Version | Component / Usage | Installation Command / Link |
| :--- | :--- | :--- | :--- |
| **.NET SDK** | 8.0.x | C# Frontend (`desktop-host`) & Avalonia UI | [dotnet.microsoft.com](https://dotnet.microsoft.com/download/dotnet/8.0) |
| **Rust Toolchain** | 1.75+ | High-Performance Engine (`rust-core`) | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| **C++ Compiler** | C++17 / C++20 | Native Engine & Extensions (`cpp-extensions`) | **GCC or GNU-mode Clang with ASan/UBSan** (MSVC/clang-cl is rejected by the QA build). **Win:** `winget install MartinStorsjo.LLVM-MinGW.UCRT` (also provides `clang-tidy` and `llvm-cov`) / **Linux:** GCC 11+ or Clang / **macOS:** Apple Clang |
| **QA tools** | — | Static analysis & coverage | `clang-tidy`, `gcovr` (`pip install gcovr`, set `GCOV="llvm-cov gcov"` with LLVM-MinGW), `cargo-llvm-cov` (`cargo install cargo-llvm-cov --locked`). Full per-OS commands: [README.md](./README.md#pré-requisitos-para-contribuidores) |
| **CMake** | 3.20+ | C++ Build System | `apt install cmake` / `brew install cmake` / [cmake.org](https://cmake.org/download/) |
| **Python** | 3.10+ | Scripting Backend (`python-backend`) | `python -m pip install -r python-backend/requirements.txt` |
| **Docker** | 24.0+ | Headless QA Testing & Cross-Compilation | [docker.com/products/docker-desktop](https://www.docker.com/products/docker-desktop) |

---

## 2. System & Runtime Prerequisites

### Windows (Required for FFI & Hardware Acceleration)
Run the automated prerequisites installer from the repository or download manually:
- **Visual C++ Redistributable All-In-One (2005-2022):** [TechPowerUp Link](https://www.techpowerup.com/download/visual-c-redistributable-runtime-package-all-in-one/)
- **DirectX End-User Runtimes (June 2010):** [TechPowerUp Link](https://www.techpowerup.com/download/directx-redistributable-runtime/)
- **Automated Command:** Run `.\scripts\install_prerequisites.bat` as Administrator.

### Linux (Debian / Ubuntu)
Install system build tools and audio/graphics headers:
```bash
sudo apt update && sudo apt install -y \
    build-essential cmake pkg-config \
    libgl1-mesa-dev libx11-dev libxrandr-dev libxi-dev \
    libasound2-dev libpipewire-0.3-dev
```

### macOS
Install Xcode Command Line Tools and Homebrew packages:
```bash
xcode-select --install
brew install cmake ninja python3
```

---

## 3. Quick Start & Virtual Environment Setup

Clone the repository and configure the Python virtual environment:

```bash
git clone https://github.com/YourOrg/neoveg.git
cd neoveg

python -m venv .venv
# Windows:
.venv\Scripts\activate
# Linux/macOS:
source .venv/bin/activate
pip install -r python-backend/requirements.txt
```

---

## 4. Environment Diagnostic & Build Verification

Verify that all required SDKs, compilers, and runtime libraries are installed on your workstation:

```powershell
# Cross-platform Python diagnostic script:
python scripts/check_env.py

# Windows PowerShell diagnostic script:
.\scripts\check_env.ps1
```

Build and test each subsystem:

```bash
# 1. Rust Core (neoveg_core)
cd rust-core && cargo test --release && cd ..

# 2. C++ Extensions (neoveg_cpp_engine)
cmake -S cpp-extensions -B cpp-extensions/build && cmake --build cpp-extensions/build --config Release

# 3. C# / Avalonia Frontend (Neoveg)
dotnet build desktop-host/Neoveg.csproj -c Release

# 4. Python Backend & QA Suite
pytest python-backend/
python python-backend/test_vegas_engine.py
```
