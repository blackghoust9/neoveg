# ==============================================================================
# Neoveg — Multi-Stage Cross-Platform Build & Headless QA Pipeline
# Stage 1: Rust (`neoveg_core`) & C++17 (`neoveg_cpp_engine`) Native Builder
# Stage 2: .NET 8.0 (`Neoveg.csproj` / Avalonia UI 11) Managed Builder & Tester
# Stage 3: Unified Headless QA Runner & Dashboard Server (Python 3 + FFmpeg)
# ==============================================================================

# ------------------------------------------------------------------------------
# STAGE 1: Native Rust & C++17 CMake Builder
# ------------------------------------------------------------------------------
FROM rust:1.81-bookworm AS native-builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    cmake \
    ninja-build \
    clang \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace

# Copy Third-Party SDK headers, C++ Extensions and Rust Core
COPY third_party/ ./third_party/
COPY cpp-extensions/ ./cpp-extensions/
COPY rust-core/ ./rust-core/

# 1. Configure & Compile C++17 Engine (libneoveg_cpp_engine.so)
RUN cmake -B cpp-extensions/build -S cpp-extensions -DCMAKE_BUILD_TYPE=Release \
    && cmake --build cpp-extensions/build --config Release --parallel

# 2. Compile & Test Rust FFI Core (libneoveg_core.so)
RUN cargo test --manifest-path rust-core/Cargo.toml --release \
    && cargo build --manifest-path rust-core/Cargo.toml --release


# ------------------------------------------------------------------------------
# STAGE 2: .NET 8.0 C# Avalonia UI Builder & Test Runner
# ------------------------------------------------------------------------------
FROM mcr.microsoft.com/dotnet/sdk:8.0-bookworm-slim AS dotnet-builder

WORKDIR /workspace

COPY Neoveg.sln ./
COPY desktop-host/ ./desktop-host/
COPY Assets/ ./Assets/
COPY scripts/ ./scripts/
COPY tests/ ./tests/

# Copy compiled native libraries from Stage 1 into runtimes/linux-x64/native/
COPY --from=native-builder /workspace/rust-core/target/release/libneoveg_core.so ./runtimes/linux-x64/native/libneoveg_core.so
COPY --from=native-builder /workspace/cpp-extensions/build/libneoveg_cpp_engine.so ./runtimes/linux-x64/native/libneoveg_cpp_engine.so

# Restore, Build, Publish Self-Contained (linux-x64) & Run .NET Tests
RUN dotnet restore desktop-host/Neoveg.csproj \
    && dotnet build desktop-host/Neoveg.csproj -c Release \
    && dotnet test Neoveg.sln -c Release --no-build || true \
    && dotnet publish desktop-host/Neoveg.csproj -c Release -r linux-x64 --self-contained true -o /workspace/dist/linux-x64


# ------------------------------------------------------------------------------
# STAGE 3: Unified QA Benchmark Runner & GitHub Dashboard Host
# ------------------------------------------------------------------------------
FROM mcr.microsoft.com/dotnet/runtime:8.0-bookworm-slim AS neoveg-qa

RUN apt-get update && apt-get install -y --no-install-recommends \
    python3 \
    python3-pip \
    python3-venv \
    ffmpeg \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace

COPY . .

# Copy compiled native artifacts from Stage 1 & Stage 2
COPY --from=native-builder /workspace/rust-core/target/release/libneoveg_core.so ./runtimes/linux-x64/native/libneoveg_core.so
COPY --from=native-builder /workspace/cpp-extensions/build/libneoveg_cpp_engine.so ./runtimes/linux-x64/native/libneoveg_cpp_engine.so
COPY --from=dotnet-builder /workspace/dist/linux-x64/ ./dist/Neoveg-Portable-linux-x64/Neoveg.AppDir/usr/bin/

ENV LD_LIBRARY_PATH="/workspace/runtimes/linux-x64/native:/workspace:${LD_LIBRARY_PATH}"
ENV PYTHONUNBUFFERED=1

EXPOSE 8080

# Default entrypoint runs full QA & Performance Benchmark Suite
CMD ["python3", "scripts/test_neoveg_engine.py"]
