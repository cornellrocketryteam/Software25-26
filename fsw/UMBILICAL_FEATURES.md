# Umbilical System — Feature Reference

**Last updated:** 2026-02-20
**Branch:** `fsw-spring26`
**Files changed:** `umbilical.rs`, `main.rs`, `state.rs`, `flight_loop.rs`

---

## 1. What the Umbilical Is

The umbilical is a USB CDC-ACM (Virtual Serial Port) link between the Pico 2 flight computer and a ground station computer. It provides **bidirectional** communication: the flight computer sends live telemetry down, and the ground station sends commands up. The Pico 2 appears as a standard serial device on the host (VID `0xC0DE`, PID `0xCAFE`, product name "Umbilical").

The system has two independent sensing layers:
- **Physical detection** (GPIO 24): Detects whether the umbilical cable is physically connected. Always active regardless of build mode.
- **USB data link** (CDC-ACM): Bidirectional telemetry and commands. Only active in release builds.

---

## 2. Build Mode Switching

The RP2350 has a single USB peripheral. It can either run the debug logger **or** the umbilical, not both. This is handled automatically by `umbilical::setup()`:

| Build | Command | USB Used For |
|-------|---------|--------------|
| **Debug** | `cargo build` / `cargo run` | USB logger (log output via `embassy-usb-logger`) |
| **Release** | `cargo build --release` / `cargo run --release` | Umbilical (telemetry + commands via CDC-ACM) |

In debug mode, a 5-second delay is added at boot to allow the logger host to attach. In release mode, this delay is skipped.

---

## 3. Telemetry: What the Umbilical Sends

The umbilical sends the **exact same 80-byte packet** that is transmitted over the RFD900x radio each flight loop cycle (1 Hz). The data is the full serialized `Packet` struct from `state.rs`:

| Byte Offset | Field | Type | Unit |
|-------------|-------|------|------|
| `0x00–0x03` | `flight_mode` | u32 | Enum (0=Startup, 1=Standby, 2=Ascent, 3=Coast, 4=DrogueDeployed, 5=MainDeployed, 6=Fault) |
| `0x04–0x07` | `pressure` | f32 | Pa |
| `0x08–0x0B` | `temp` | f32 | C |
| `0x0C–0x0F` | `altitude` | f32 | m |
| `0x10–0x13` | `latitude` | f32 | degrees |
| `0x14–0x17` | `longitude` | f32 | degrees |
| `0x18–0x1B` | `num_satellites` | u32 | count |
| `0x1C–0x1F` | `timestamp` | f32 | s |
| `0x20–0x23` | `mag_x` | f32 | uT |
| `0x24–0x27` | `mag_y` | f32 | uT |
| `0x28–0x2B` | `mag_z` | f32 | uT |
| `0x2C–0x2F` | `accel_x` | f32 | m/s^2 |
| `0x30–0x33` | `accel_y` | f32 | m/s^2 |
| `0x34–0x37` | `accel_z` | f32 | m/s^2 |
| `0x38–0x3B` | `gyro_x` | f32 | deg/s |
| `0x3C–0x3F` | `gyro_y` | f32 | deg/s |
| `0x40–0x43` | `gyro_z` | f32 | deg/s |
| `0x44–0x47` | `pt3` | f32 | raw ADC counts |
| `0x48–0x4B` | `pt4` | f32 | raw ADC counts |
| `0x4C–0x4F` | `rtd` | f32 | raw ADC counts |

**All values are little-endian.** Total: 80 bytes per packet, sent once per flight loop cycle (1 Hz).

### Timing behavior

The sender task blocks (via `Signal::wait()`) until the flight loop provides a fresh packet. This means:
- Exactly one packet per flight loop cycle (no stale re-sends).
- If the USB cable is not connected, the sender suspends at `wait_connection()` and does zero work.
- If the flight loop stalls, no packets are sent (fail-safe).

### Data source

The packet is serialized in `FlightState::transmit()` (`state.rs`). After being sent over the radio, the same `[u8; 80]` buffer is passed to `umbilical::update_telemetry()`, which signals the sender task.

---

## 4. Commands: What Can Be Sent to the Flight Computer

Commands are sent from the ground station as ASCII byte strings over the USB serial port. Each command is a token wrapped in angle brackets.

### Implemented Commands (fully wired to flight loop)

| Token | Command | What It Does |
|-------|---------|-------------|
| `<L>` | **Launch** | Sets the `umbilical_launch` flag. When the FSW is in **Standby** mode, this triggers the Standby-to-Ascent transition: opens MAV (for 530 ms), opens SV, records reference pressure, and transitions to Ascent. |
| `<M>` | **Open MAV** | Opens the Motor Actuated Vent servo immediately. Sets the MAV timer and `mav_open` flag. MAV state is persisted to FRAM (address 20). |
| `<m>` | **Close MAV** | Closes the MAV servo immediately. Clears the MAV timer and `mav_open` flag. MAV state is persisted to FRAM. |
| `<S>` | **Open SV** | Opens the Separation Valve (active-low GPIO 8). Sets `sv_open` flag. SV state is persisted to FRAM (address 24). |
| `<s>` | **Close SV** | Closes the Separation Valve. Clears `sv_open` flag. SV state is persisted to FRAM. |
| `<V>` | **Safe Vehicle** | Emergency safe: closes both MAV and SV, clears all valve flags and timers. |
| `<F>` | **Reset FRAM** | Wipes FRAM state (FlightMode, CycleCount, altitude log). Resets FSW to `Startup` mode with cycle count 0. |
| `<R>` | **Reboot** | Triggers a full system reset via `cortex_m::peripheral::SCB::sys_reset()`. The Pico 2 restarts from scratch. |

### Parsed but Not Yet Implemented

| Token | Command | Status |
|-------|---------|--------|
| `<D>` | **Reset SD Card** | Parsed and logged, but SD logging is currently disabled (`sd_logging_enabled = false`). Will be implemented when SD card support is enabled. |

### Recognized but Need Payload Data Protocol

These tokens are recognized by the parser but currently do nothing because they require additional payload data (e.g., a float value following the command token). A more complex parsing protocol is needed.

| Token | Intended Purpose |
|-------|-----------------|
| `<C1>` | Change target latitude |
| `<C2>` | Change target longitude |
| `<C3>` | Change reference pressure |
| `<C4>` | Change altimeter state |
| `<C5>` | Change SD card state |
| `<C6>` | Change altimeter armed flag |
| `<C7>` | Change flight mode |

### Command Timing

The receiver task polls for incoming USB packets every **100 ms** (10 Hz). Commands are buffered in a channel with capacity 4. The flight loop drains all pending commands at the start of each cycle (1 Hz) in `check_umbilical_commands()`. If more than 4 commands arrive between flight loop cycles, the oldest are dropped.

---

## 5. GPIO Physical Sensing (Independent of USB)

Separate from the USB data link, the FSW monitors the physical umbilical connection via **GPIO 24** (input, pull-down). This is read every flight loop cycle in `FlightState::read_sensors()`.

### Behavior by flight mode

| Flight Mode | Umbilical Connected | Umbilical Disconnected |
|-------------|--------------------|-----------------------|
| **Startup** | Logs "connected", resets disconnect timer, buzzes 2x | Starts 15-second timer. After timeout, logs vent command to fill station. Buzzes 3x. |
| **Standby** | Same as Startup | Same as Startup |
| **Ascent** | Logs "connected", buzzes 2x | Buzzes 3x |
| **Coast through MainDeployed** | Not checked | Not checked |
| **Fault** | Not checked | Not checked |

The 15-second disconnect timeout (`UMBILICAL_TIMEOUT_MS = 15000`) is designed as a safety feature: if the umbilical cable is accidentally pulled during fill operations, the FSW signals the fill station to vent after 15 seconds. Reconnecting the cable resets the timer.

---

## 6. Architecture: How It All Fits Together

### Task structure (release mode)

Three embassy async tasks are spawned at boot:

1. **`usb_task`** — Runs the low-level USB peripheral protocol (enumeration, endpoint management). Must be running for the other two tasks to work.
2. **`umbilical_sender_task`** — Waits for fresh telemetry from the `TELEMETRY` Signal, then writes the 80-byte packet to the USB endpoint. Suspends when no cable is connected.
3. **`umbilical_receiver_task`** — Reads USB packets, parses command tokens, and pushes `UmbilicalCommand` variants into the `COMMANDS` Channel.

### Shared state between tasks and flight loop

| Static | Type | Writer | Reader | Purpose |
|--------|------|--------|--------|---------|
| `TELEMETRY` | `Signal<CriticalSectionRawMutex, [u8; 80]>` | `FlightState::transmit()` via `update_telemetry()` | `umbilical_sender_task` | Latest telemetry packet |
| `COMMANDS` | `Channel<CriticalSectionRawMutex, UmbilicalCommand, 4>` | `umbilical_receiver_task` | `FlightLoop::check_umbilical_commands()` via `try_recv_command()` | Pending commands |

### Data flow diagram

```
                         GROUND STATION (Host Computer)
                              |           ^
                         USB CDC-ACM (VID 0xC0DE / PID 0xCAFE)
                              |           |
                    [commands down]   [telemetry up]
                              |           |
                              v           |
                    umbilical_receiver  umbilical_sender
                         task              task
                              |           ^
                    COMMANDS Channel   TELEMETRY Signal
                              |           |
                              v           |
                    FlightLoop          FlightState
                  .check_umbilical    .transmit()
                   _commands()             |
                         |            radio.send()
                         v                 |
                   [actuate valves,        v
                    set flags,        RFD900x Radio
                    reset FRAM,       (same 80-byte packet)
                    reboot, etc.]
```

### Execution order within each flight loop cycle

1. `read_sensors()` — Reads all sensors, updates GPIO states (including umbilical sense on PIN 24)
2. `check_subsystem_health()` — Checks payload/recovery comms
3. `check_ground_commands()` — Radio command handling (placeholder)
4. **`check_umbilical_commands()`** — Drains all pending umbilical commands and executes them
5. `check_transitions()` — State machine transition logic (also checks `umbilical_launch` flag)
6. `transmit()` — Serializes packet, sends over radio, then calls `update_telemetry()` for the umbilical sender

---

## 7. Files Modified

| File | Change |
|------|--------|
| `umbilical.rs` | Complete rewrite. Now contains: `setup()`, `UmbilicalCommand` enum, `TELEMETRY` Signal, `COMMANDS` Channel, `update_telemetry()`, `try_recv_command()`, `logger_task`, `usb_task`, `umbilical_sender_task`, `umbilical_receiver_task`. |
| `main.rs` | Replaced logger init + spawn with single `umbilical::setup()` call. Removed `logger_task` (moved to `umbilical.rs`). Debug-mode delay is now conditional. |
| `state.rs` | Added `crate::umbilical::update_telemetry(&data)` call at the end of `transmit()` to share the serialized packet with the umbilical sender task. |
| `flight_loop.rs` | Replaced placeholder `check_umbilical_commands()` with real implementation that polls `umbilical::try_recv_command()` and handles all 9 command variants. Added `use crate::umbilical`. |

---

## 8. Remaining TODOs

| Item | Where | Notes |
|------|-------|-------|
| SD card reset command (`<D>`) | `flight_loop.rs:179` | Needs SD card support to be enabled first |
| Configuration commands (`<C1>` through `<C7>`) | `umbilical.rs:116-122` | Need a payload-data parsing protocol (command + value) |
| Fill station vent command on disconnect timeout | `flight_loop.rs:219,275` | Currently only logs; needs actual command transmission to fill station |
| Buzzer feedback on USB connect/disconnect | `umbilical.rs` (sender task) | Commented out (`buzzer.buzz_num(3)` / `buzzer.buzz_num(2)`); buzzer is not accessible from the async task without shared state |
