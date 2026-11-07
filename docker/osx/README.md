# Docker-OSX (native macOS test guest)

`docker-osx` in `docker/docker-compose.yml` runs [sickcodes/Docker-OSX](https://github.com/sickcodes/Docker-OSX)
(macOS under QEMU/KVM). `test-rust-macos-native` then copies the sources into the guest over SSH and runs
`cargo test --workspace` and `cargo build --release -p neoveg_ui` there.

Requirements (not verified by CI):

- `/dev/kvm` in the container: a Linux host, or Docker Desktop on Windows with WSL2 nested virtualization.
- 8 GB of RAM for the guest (`NEOVEG_OSX_RAM`) and about 60 GB of disk.
- Apple's licence: macOS may only run on Apple hardware; using Docker-OSX elsewhere is the contributor's responsibility.

One-time setup of the disk image `mac_hdd.img` (kept in `$NEOVEG_ENV_DIR/docker-osx/`, `D:\ambiente_neoveg\docker-osx\`):

1. Install macOS following the Docker-OSX README (`sickcodes/docker-osx:latest`) and, in the guest, enable
   **Remote Login** (System Settings > General > Sharing) for the user in `NEOVEG_OSX_USER`.
2. Install the Xcode Command Line Tools (`xcode-select --install`); Rust is installed on demand by the script.
3. Shut the guest down and copy its disk to `$NEOVEG_ENV_DIR/docker-osx/mac_hdd.img`.

Run with `scripts/test_rust_crossplatform.ps1 -MacosNative`.
