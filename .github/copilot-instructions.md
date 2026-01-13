<!-- Copilot instructions for Cornell Rocketry Team Software repo -->
# Copilot / AI Agent Instructions

Purpose: Help an AI coding agent become productive quickly in this repository by describing the architecture, developer workflows, conventions, and important integration points with concrete file references and commands.

  - This repo contains multiple subsystems: embedded flight firmware (`fsw/`), a ground-side fill station service (`fill-station/`), hardware/system docs and helpers (`RATS/`, `Common/`), and Nix-based build/image tooling (`nix/`). See `RATS/SystemDoc.md` for the high-level design (radio/dual‑Pico architecture).
  - Flight firmware (`fsw/`) is a no_std Rust binary built for a Cortex-M target using `embassy` async runtimes. Key files: `fsw/src/main.rs`, `fsw/Cargo.toml`, `fsw/build.rs`, `fsw/memory.x`.
  - Fill station (`fill-station/`) is a host-side Rust service (async, `smol`) exposing a WebSocket interface on port 9000. Key files: `fill-station/src/main.rs`, `fill-station/Cargo.toml`.

  - Rocket telemetry arrives on RFD900x → Radio Pico UART (RX only). Radio Pico parses 107‑byte packets, logs to SD, and publishes via MQTT/Wi‑Fi. It forwards simple tracking commands to Motor Pico via an inter‑Pico UART link. (See `RATS/SystemDoc.md` sections 3–6.)
  - Motor Pico receives commands and drives stepper motor drivers via GPIO/PIO. Micro‑controller-side code follows a `module::init_*` pattern for hardware setup (`fsw/src/module.rs`).

  - Local firmware (FSW) compile: from `fsw/`:
    - `rustup target add thumbv7em-none-eabihf`
    - `cargo build --release` (works when toolchain and environment are set)
    - `cargo run --release` can flash/run when the pico toolchain and USB are available.
  - Install host tools: `brew install picotool` (used for flashing Pico 2). Use `defmt` + `panic-probe` for embedded logging.
  - Nix image build (system images, SD images): example used in this repo:
    - `nix build .#mixosConfigurations.fill-station.config.system.build.sdImage`
    - Developer shells and cross toolchains live under `nix/dev-shells/` and overlay packages under `nix/overlays/`.
  - Fill station (host) run from repo root or `fill-station/`:
    - `cargo run --manifest-path fill-station/Cargo.toml` or build with `cargo build --manifest-path fill-station/Cargo.toml --release`.

  - Embedded code is `#![no_std]` and uses `embassy` — spawn tasks with `#[embassy_executor::task]` and `#[embassy_executor::main]` patterns. Avoid inserting std-only code or allocations in `fsw/`.
  - Logging on embedded uses `defmt`/`defmt-rtt`; host-side uses `tracing` and writes rotated logs into `logs/` (see `fill-station/src/main.rs`).
  - Hardware initialization centralizes in `module::init_*` helpers; prefer adding/initing hardware there rather than scattering pin setup across files.
  - Packet parsing expects sync‑word search and fixed packet sizes (see `RATS/SystemDoc.md` and `Common/packet_parser.*` if present). Do not change packet framing without updating docs and both Picos.
  - Pin mappings, serial ports, and timing constraints are authoritative in `RATS/SystemDoc.md` — treat that file as the single source of truth for hardware wiring.

  - RFD900x radio (UART) — radio telemetry at 115200 bps. Look at UART init in `fsw/src/module.rs` and parsing in `fsw/src/packet.rs`.
  - Inter‑Pico UART (Radio Pico ↔ Motor Pico) — check `RATS/SystemDoc.md` and `Common/serial_protocol.h` for packet structure; changes require coordinated updates on both sides.
  - MicroSD (SPI) logging — `fsw` uses SPI1 pins and FAT32; ensure file system code matches `RATS` pin assignments.
  - Nix overlays provide cross-toolchains and image builders. Avoid ad‑hoc environment changes; prefer adding dev-shells in `nix/dev-shells/`.

  - Changing `Cargo.toml` dependencies to a different major version (embedded toolchains and `embassy` are sensitive). Test builds in the Nix dev shell or locally with the correct target.
  - Pin or protocol changes — update `RATS/SystemDoc.md` and any `Common/*` protocol headers.
  - Build artifacts and images — use the provided `nix` targets to produce repeatable images.

  - Embedded main loop & task spawn: `fsw/src/main.rs`.
  - Hardware init pattern: `fsw/src/module.rs` (search for `init_spi`, `init_i2c`, `init_uart`).
  - Packet definitions & formats: `RATS/SystemDoc.md` + `Common/packet_parser.*`.
  - Host websocket service: `fill-station/src/main.rs` (listen on port 9000, JSON commands via `serde`).

If any section is unclear or you need more details (pin names, packet fields, or build failures), tell me which area to expand and I will iterate.

# Copilot / AI Agent Instructions

Goal: Make AI agents productive immediately in this repo by documenting actual architecture, workflows, conventions, and integration points with concrete file references and commands.

**Big Picture**
- Embedded flight firmware in `fsw/` (Rust `#![no_std]`, `embassy` async) targets Cortex‑M; key files: `fsw/src/main.rs`, `fsw/src/module.rs`, `fsw/src/packet.rs`, `fsw/Cargo.toml`, `fsw/memory.x`, `fsw/build.rs`.
- Ground‑side fill station in `fill-station/` (Rust async via `smol`) exposes a WebSocket server on port 9000; runs on TI AM64x SK board. Key files: `fill-station/src/main.rs`, `fill-station/src/command.rs`, `fill-station/src/components/`, `fill-station/src/hardware.rs`.
- Hardware/system docs + host C/C++ Pico code in `RATS/` and `RATS/Common/`; see `RATS/SystemDoc.md` for dual‑Pico radio architecture and packet framing.
- Nix tooling for images and dev shells in `nix/` (`nix/mixos-configurations/fill-station`, `nix/dev-shells`, `nix/overlays`). Complete build system docs: `fill-station/docs/LINUX_IMAGE_BUILD_PROCESS.md`.

**Data Flow**
- RFD900x → Radio Pico UART (RX): parse fixed 107‑byte packets, log to SD, publish via MQTT/Wi‑Fi; forward tracking commands to Motor Pico over inter‑Pico UART. See `RATS/SystemDoc.md`, `RATS/Common/packet_parser.*`.
- Motor Pico drives steppers via GPIO/PIO; init follows `module::init_*` patterns. See `fsw/src/module.rs` for UART/SPI/I2C init and pin roles.
- Fill station coordinates hardware components (igniters, ADCs, sensors) and exposes command JSON over WebSocket. Includes background ADC monitoring task (10 Hz sampling). See `fill-station/src/components/igniter.rs`, `fill-station/src/components/ads1015.rs`, `fill-station/src/command.rs`.

**Build / Flash / Run**
- Firmware (`fsw/`):
  - `rustup target add thumbv7em-none-eabihf`
  - `cargo build --release`
  - `cargo run --release` (flashes if Pico tooling/USB available).
- Embedded logging: `defmt` + `panic-probe` (RTT). Avoid `std` and heap in `fsw/`.
- Fill station (host dev):
  - `cargo run --manifest-path fill-station/Cargo.toml`
  - `cargo build --manifest-path fill-station/Cargo.toml --release`
- Fill station (production - TI AM64x):
  - `nix build .#mixosConfigurations.fill-station.config.system.build.sdImage`
  - Result: `./result/fill-station.img` (bootable SD card image)
  - Flash: `sudo dd if=./result/fill-station.img of=/dev/sdX bs=4M status=progress`
  - Includes: MixOS init, U-Boot, kernel, FIT image, all binaries
- Dev shells/toolchains under `nix/dev-shells/`; overlays under `nix/overlays/by-name/crt/`.
- Pico flashing tools: `brew install picotool`.

**Conventions**
- Fill station uses platform-aware `#[cfg]` for Linux-only hardware code (GPIO, I2C); compiles on macOS for development.
- Treat `RATS/SystemDoc.md` as the source of truth for pin mappings, serial ports, timings, and protocol specifics.
- Device tree overlays: Auto-built from SysConfig via `nix/overlays/by-name/crt/fillstation-dtbo/`. Update `src/sysconfig-pinmux.dtsi` and rebuild. See `fill-station/docs/DTBO_BUILDER.md`p hardware setup centralized in `module::init_*`.
- Packet parsing uses a sync‑word and fixed sizes; do not change framing without coordinated updates in `RATS/SystemDoc.md` and both Pico codebases.
- Host logging uses `tracing` (rotated to `logs/` if configured in `fill-station/src/main.rs`).
- I2C (fill station ADCs): `/dev/i2c-2` on TI AM64x; dual ADS1015 at 0x48/0x49. See `fill-station/src/components/ads1015.rs`.
- WebSocket commands: JSON schema in `fill-station/src/command.rs`; components in `fill-station/src/components/`; handlers in `main.rs`.
- Nix build/overlay integration: 
  - System: `nix/mixos-configurations/fill-station/default.nix` (MixOS config)
  - FIT image: `nix/mixos-configurations/fill-station/fit/` (kernel+dtb+initrd)
  - Packages: `nix/overlays/by-name/crt/` (custom-linux, ti-uboot-*, fill-station, fillstation-dtbo)
  - Device tree: Upstream kernel DTB + auto-built overlay merged at build time via `fdtoverlay`

**Integration Points**
- UART (RFD900x, inter‑Pico): init and usage in `fsw/src/module.rs`; packet definitions in `RATS/Common/packet_types.h`, `packet_parser.*`, `serial_protocol.h`.
- SPI (MicroSD logging): pins and setup referenced in `fsw/src/module.rs`; ensure consistency with `RATS` pin assignments.
- Fill station server: `fill-station/src/main.rs` (WebSocket + ADC background task); commands in `fill-station/src/command.rs`; components in `fill-station/src/components/`.
- Hardware aggregation: `fill-station/src/hardware.rs` (collects all components).
- Kernel config: `nix/overlays/by-name/crt/custom-linux/kernel.config` (edit for kernel options like PWM).

**Fill Station Documentation Hub**
Comprehensive docs in `fill-station/docs/`:
- `INDEX.md` - Documentation navigation hub
- `ADDING_FEATURES.md` - Guide to extending the system (components, commands, tasks)
- `ADC_STREAMING.md` - Background ADC monitoring and WebSocket protocol
- `LINUX_IMAGE_BUILD_PROCESS.md` - Complete Nix/FIT/U-Boot build system guide
- `DTBO_BUILDER.md` - Automated device tree overlay builder
- `QUICKSTART_ADC.md`, `ADC_MONITOR_GUIDE.md`, `TROUBLESHOOTING.md`

If any area is unclear (pins, packet fields, UART speeds, Nix targets, build process), consult the relevant docs in `fill-station/docs/` or ask for clarification.

**Useful File Examples**
- Task spawn + main loop: `fsw/src/main.rs`.
- Hardware init helpers: `fsw/src/module.rs` (`init_spi`, `init_i2c`, `init_uart`).
- Packet formats: `RATS/SystemDoc.md`, `RATS/Common/packet_parser.cpp`, `packet_types.h`.
- Fill station server: `fill-station/src/main.rs`; commands in `fill-station/src/command.rs`; igniter component in `fill-station/src/components/igniter.rs`.

If any area is unclear (pins, packet fields, UART speeds, Nix targets), tell me which section to expand and we’ll iterate.
