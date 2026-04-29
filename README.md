# Software 2025-2026

Welcome to the Software repository for the 2025-2026 rocket. This repository contains all the code for the flight systems, ground support equipment, and user interfaces.

## Repository Structure

* **`fsw/`**: Flight Software. Rust `no_std` application running on the Raspberry Pi Pico 2 (RP2350) using the Embassy async runtime. Handles sensors, actuators, flight state machine, and telemetry.
* **`fill-station/`**: Ground Support Server. Rust application running on a Linux host (e.g., Raspberry Pi) at the pad. Interfaces with the rocket via the umbilical, controls fill valves, and hosts a WebSocket server for the UI.
* **`Ground_Station_UI/`**: The frontend dashboard for the ground station. Provides a real-time UI to monitor telemetry, view actuator states, and send commands to the fill station and rocket.
* **`BLIMS/`**: Balloon-Launched Intelligent Mechanism System. Code for the steerable parachute payload.
* **`RATS/`**: Code for the Rocket Actuation and Testing System, including stepper motor controls and test benches.
* **`air-brake-controls/`**: Development and simulation code for the active airbrake drag-control system.

## Documentation Reference

For detailed subsystem documentation, see the following living documents:
* **FSW Architecture & Sensors:** `fsw/FSW_REFERENCE.md`
* **Umbilical Commands & Telemetry:** `UMBILICAL_REFERENCE.md`
* **Safety & Failsafes:** `FAILSAFES.md`
* **System Doc:** `RATS/SystemDoc.md`

Make sure to configure the correct Rust toolchains (`thumbv8m.main-none-eabihf` for FSW) before building the embedded projects. See subsystem READMEs for specific instructions.
