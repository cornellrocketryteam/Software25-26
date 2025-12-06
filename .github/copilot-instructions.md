<!-- Copilot instructions for Cornell Rocketry Team Software repo -->
# Copilot / AI Agent Instructions

Purpose: Help an AI coding agent become productive quickly in this repository by describing the architecture, developer workflows, conventions, and important integration points with concrete file references and commands.

- **Big Picture:**
  - This repo contains multiple subsystems: embedded flight firmware (`fsw/`), a ground-side fill station service (`fill-station/`), hardware/system docs and helpers (`RATS/`, `Common/`), and Nix-based build/image tooling (`nix/`). See `RATS/SystemDoc.md` for the high-level design (radio/dual‑Pico architecture).
  - Flight firmware (`fsw/`) is a no_std Rust binary built for a Cortex-M target using `embassy` async runtimes. Key files: `fsw/src/main.rs`, `fsw/Cargo.toml`, `fsw/build.rs`, `fsw/memory.x`.
  - Fill station (`fill-station/`) is a host-side Rust service (async, `smol`) exposing a WebSocket interface on port 9000. Key files: `fill-station/src/main.rs`, `fill-station/Cargo.toml`.

- **Architecture & data flow (concise):**
  - Rocket telemetry arrives on RFD900x → Radio Pico UART (RX only). Radio Pico parses 107‑byte packets, logs to SD, and publishes via MQTT/Wi‑Fi. It forwards simple tracking commands to Motor Pico via an inter‑Pico UART link. (See `RATS/SystemDoc.md` sections 3–6.)
  - Motor Pico receives commands and drives stepper motor drivers via GPIO/PIO. Micro‑controller-side code follows a `module::init_*` pattern for hardware setup (`fsw/src/module.rs`).

- **Build / Flash / Debug workflows (explicit commands):**
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

- **Conventions & patterns agents must follow:**
  - Embedded code is `#![no_std]` and uses `embassy` — spawn tasks with `#[embassy_executor::task]` and `#[embassy_executor::main]` patterns. Avoid inserting std-only code or allocations in `fsw/`.
  - Logging on embedded uses `defmt`/`defmt-rtt`; host-side uses `tracing` and writes rotated logs into `logs/` (see `fill-station/src/main.rs`).
  - Hardware initialization centralizes in `module::init_*` helpers; prefer adding/initing hardware there rather than scattering pin setup across files.
  - Packet parsing expects sync‑word search and fixed packet sizes (see `RATS/SystemDoc.md` and `Common/packet_parser.*` if present). Do not change packet framing without updating docs and both Picos.
  - Pin mappings, serial ports, and timing constraints are authoritative in `RATS/SystemDoc.md` — treat that file as the single source of truth for hardware wiring.

- **Integration points & external deps:**
  - RFD900x radio (UART) — radio telemetry at 115200 bps. Look at UART init in `fsw/src/module.rs` and parsing in `fsw/src/packet.rs`.
  - Inter‑Pico UART (Radio Pico ↔ Motor Pico) — check `RATS/SystemDoc.md` and `Common/serial_protocol.h` for packet structure; changes require coordinated updates on both sides.
  - MicroSD (SPI) logging — `fsw` uses SPI1 pins and FAT32; ensure file system code matches `RATS` pin assignments.
  - Nix overlays provide cross-toolchains and image builders. Avoid ad‑hoc environment changes; prefer adding dev-shells in `nix/dev-shells/`.

- **When modifying code, be careful about:**
  - Changing `Cargo.toml` dependencies to a different major version (embedded toolchains and `embassy` are sensitive). Test builds in the Nix dev shell or locally with the correct target.
  - Pin or protocol changes — update `RATS/SystemDoc.md` and any `Common/*` protocol headers.
  - Build artifacts and images — use the provided `nix` targets to produce repeatable images.

- **Concrete examples to look at when performing tasks:**
  - Embedded main loop & task spawn: `fsw/src/main.rs`.
  - Hardware init pattern: `fsw/src/module.rs` (search for `init_spi`, `init_i2c`, `init_uart`).
  - Packet definitions & formats: `RATS/SystemDoc.md` + `Common/packet_parser.*`.
  - Host websocket service: `fill-station/src/main.rs` (listen on port 9000, JSON commands via `serde`).

If any section is unclear or you need more details (pin names, packet fields, or build failures), tell me which area to expand and I will iterate.
