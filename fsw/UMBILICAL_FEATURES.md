# Umbilical System — Feature Reference

**Last updated:** 2026-04-22
**Branch:** `fsw-spring26-csv-umbilical`
**Files changed:** `umbilical.rs`, `main.rs`, `state.rs`, `flight_loop.rs`

> **Wire format note (2026-04-22):** This branch reverts the umbilical wire
> format from binary (`[0xAA,0x55] || 82-byte LE Packet`) back to CSV text
> (`$TELEM,<22 fields>\n`) shared on the same channel as logs. The binary
> single-slot atomic buffer is gone; telemetry is now queued through
> `RAW_OUTBOUND` like every other text payload. To prevent log/dump bytes
> from interleaving with `$TELEM` lines (and to keep dumps from being
> back-pressured behind queued telemetry), `umbilical::begin_dump()` /
> `end_dump()` set a `DUMP_IN_PROGRESS` flag that suppresses
> `emit_telemetry` for the duration of a flash/FRAM dump.

---

## 1. What the Umbilical Is

The umbilical is a USB CDC-ACM (Virtual Serial Port) link between the Pico 2 flight computer and a ground station computer. It provides **bidirectional** communication: the flight computer sends live telemetry down, and the ground station sends commands up. The Pico 2 appears as a standard serial device on the host (VID `0xC0DE`, PID `0xCAFE`, product name "Umbilical").

The system has two independent sensing layers:
- **Physical detection** (GPIO 24): Detects whether the umbilical cable is physically connected. Always active regardless of build mode.
- **USB data link** (CDC-ACM): Bidirectional telemetry and commands. Only active in release builds.

---

## 2. Build Mode Switching

A single USB CDC-ACM endpoint carries the umbilical in **both** build modes. The only difference is whether `log::*!` macros emit text to the same wire:

| Build | Command | What goes on the wire |
|-------|---------|------------------------|
| **Debug** | `cargo build` / `cargo run` | `$TELEM,…` lines + `[INFO] …` log lines + explicit `print_str`/`print_bytes` (logger active). |
| **Release** | `cargo build --release` / `cargo run --release` | `$TELEM,…` lines + explicit `print_str`/`print_bytes` only (logger compiled out, no `log::*` output reaches the wire). |

In both modes, commands `<X>` flow back from the host. In debug mode a 5-second boot delay lets the host attach to the serial port before output begins.

---

## 3. Telemetry: What the Umbilical Sends

Each flight loop cycle (1 Hz) the FSW emits **one CSV line** containing the same fields as the radio packet:

```
$TELEM,<flight_mode>,<pressure>,<temp>,<altitude>,<latitude>,<longitude>,<num_satellites>,<timestamp>,<mag_x>,<mag_y>,<mag_z>,<accel_x>,<accel_y>,<accel_z>,<gyro_x>,<gyro_y>,<gyro_z>,<pt3>,<pt4>,<rtd>,<sv_open>,<mav_open>\n
```

22 comma-separated fields after the `$TELEM,` prefix, terminated by `\n`. Field count is exposed as `pub const TELEM_FIELD_COUNT: usize = 22;` and the host-side parser (`fill-station/src/components/umbilical.rs`) imports the same constant — both sides must move together when fields are added.

| # | Field | Type | Unit |
|---|-------|------|------|
| 0  | `flight_mode` | u32 | Enum (0=Startup, 1=Standby, 2=Ascent, 3=Coast, 4=DrogueDeployed, 5=MainDeployed, 6=Fault) |
| 1  | `pressure` | f32 | Pa |
| 2  | `temp` | f32 | C |
| 3  | `altitude` | f32 | m |
| 4  | `latitude` | f32 | degrees |
| 5  | `longitude` | f32 | degrees |
| 6  | `num_satellites` | u32 | count |
| 7  | `timestamp` | f32 | s |
| 8–10  | `mag_x/y/z` | f32 | uT |
| 11–13 | `accel_x/y/z` | f32 | m/s^2 |
| 14–16 | `gyro_x/y/z` | f32 | deg/s |
| 17 | `pt3` | f32 | raw ADC counts |
| 18 | `pt4` | f32 | raw ADC counts |
| 19 | `rtd` | f32 | raw ADC counts |
| 20 | `sv_open` | `0`/`1` | — |
| 21 | `mav_open` | `0`/`1` | — |

### Timing behavior

`emit_telemetry()` is called once per flight loop cycle from `FlightState::transmit()`. The line is formatted into a 512 B stack buffer and pushed into `RAW_OUTBOUND` (32 × 64 B chunks) via non-blocking `try_send` — if the channel is full (e.g. during heavy log output), the chunk is dropped. The sender task drains `RAW_OUTBOUND` FIFO and writes 64-byte USB packets. If the USB cable is not connected, the sender suspends at `wait_connection()`.

### Dump-vs-telemetry interlock

When `umbilical::begin_dump()` is called (currently from `state.rs::print_flash_dump`), `emit_telemetry` becomes a no-op until `end_dump()` is called. This:
1. Keeps the host line parser sane — raw flash bytes will not be interleaved with `$TELEM` lines.
2. Stops telemetry from competing with `print_bytes_async` for `RAW_OUTBOUND` slots, so the dump finishes faster.

Telemetry resumes automatically once the dump completes.

### Data source

The packet is serialized in `FlightState::transmit()` (`state.rs`). After being sent over the radio, the same `Packet` struct is passed to `umbilical::emit_telemetry(&packet)`, which formats the CSV line.

---

## 4. Commands: What Can Be Sent to the Flight Computer

Commands are sent from the ground station as ASCII byte strings over the USB serial port. Each command is a token wrapped in angle brackets.

### Implemented Commands (fully wired to flight loop)

| Token | Command | What It Does |
|-------|---------|-------------|
| `<H>` | **Heartbeat** | Hot-path command: bumps the last-heartbeat timestamp used by the umbilical-disconnect timeout logic. |
| `<L>` | **Launch** | Sets the `umbilical_launch` flag. When the FSW is in **Standby** mode, this triggers the launch actuator sequence (see "Launch Sequence" below) and the Standby-to-Ascent transition. |
| `<M>` | **Open MAV** | Opens the Motor Actuated Vent servo immediately. No auto-close. Sets `mav_open` flag, persisted to FRAM. |
| `<m>` | **Close MAV** | Closes the MAV servo immediately. Clears `mav_open` flag, persisted to FRAM. |
| `<S>` | **Open SV** | Opens the Separation Valve (active-low GPIO 8). No auto-close. Sets `sv_open` flag, persisted to FRAM. |
| `<s>` | **Close SV** | Closes the Separation Valve. Clears `sv_open` flag, persisted to FRAM. |
| `<V>` | **Safe Vehicle** | Emergency safe: closes both MAV and SV, clears all valve flags and timers. |
| `<F>` | **Reset FRAM** | Wipes FRAM state (FlightMode, CycleCount, altitude log). Resets FSW to `Startup` mode with cycle count 0. |
| `<f>` | **Dump FRAM** | Streams FRAM contents over the umbilical. |
| `<R>` | **Reboot** | Triggers a full system reset via `cortex_m::peripheral::SCB::sys_reset()`. |
| `<G>` | **Dump Flash** | Streams the QSPI CSV flash storage region over the umbilical via `print_bytes_async`. Telemetry is suppressed for the duration via `begin_dump`/`end_dump`. |
| `<W>` | **Wipe Flash** | Erases the QSPI flash storage region. |
| `<I>` | **Flash Info** | Prints flash usage (used / total KB and percent). |
| `<X>` | **Wipe FRAM + Reboot** | Calls `reset_fram()` and then `SCB::sys_reset()` — clears FRAM and reboots in one step. |
| `<K>` | **Key Arm** | Sets `key_armed = true`. Required to allow the Startup → Standby transition. |
| `<k>` | **Key Disarm** | Sets `key_armed = false`. From Standby this reverts the FSW to Startup. |
| `<T,lat,lon>` | **Set BLiMS Target** | Variable-length text command carrying two `f32` decimal-degree values (e.g. `<T,42.4419130,-76.4878000>`). Range checked: lat ∈ [-90, 90], lon ∈ [-180, 180]. Stores the BLiMS landing-zone target and forwards it to the BLiMS controller. Arming of guidance happens at the MainDeployed transition (see §below). |
| `<1>`–`<4>` | **Payload N1–N4** | Send a payload event byte (`N1`/`N2`/`N3`/`N4`) over the payload UART. `<1>` (camera deploy) is restricted to Startup/Standby. |

> **Removed:** `<D>` (Reset SD Card) is no longer recognized by the FSW.

### Launch Sequence (triggered by `<L>` in Standby with umbilical connected)

When the FSW receives `<L>` in Standby and the umbilical is physically connected, `flight_loop.rs::handle_launch_sequence` runs the following actuator timeline (all from MAV/SV closed):

| Time | Stage | Action |
|------|-------|--------|
| t = 0 ms | `PreVent` | SV opens |
| t = `LAUNCH_SV_PREVENT_MS` (2 000 ms) | `SvToMavWait` | SV closes |
| t = +`LAUNCH_SV_TO_MAV_WAIT_MS` (1 000 ms) | `MavOpen` | MAV opens |
| t = +`MAV_OPEN_DURATION_MS` (4 000 ms L3 / 7 880 ms LV) | `Done` | MAV closes; sequence ends |

The `LaunchStage` enum is `{ None, PreVent, SvToMavWait, MavOpen, Done }`. There is no longer an automatic final-vent stage at the end of the launch sequence (the old `LAUNCH_POST_MAV_WAIT_MS` constant and `FinalVent` stage are gone).

### Recovery Vent (replaces the old final-vent)

In place of an end-of-launch automatic vent, the FSW now performs a **one-shot recovery vent**: the first time `check_transitions` enters `DrogueDeployed`, `MainDeployed`, or `Fault`, it opens SV exactly once. A `recovery_vent_sent` flag in `FlightLoop` guarantees this happens at most once per boot.

### Key Arming

The Startup → Standby transition is gated on `key_armed`. Previously this was hardcoded to `true` at the top of the Startup handler; that hardcoded assignment has been removed. The operator must now send `<K>` (sets `key_armed = true`) before Startup will advance to Standby. Sending `<k>` from Standby clears `key_armed` and bounces the FSW back to Startup.

### BLiMS Target Setting

`<T,lat,lon>` stores the target into a controller-side field and updates a `blims_target_set` flag on `FlightLoop`. The function `set_blims_target` no longer auto-arms BLiMS guidance — it only stores/forwards the value, so mid-flight retargets are supported.

The previously-hardcoded fallback target of `42.696969 / -42.696969` (set on first MainDeployed entry) has been **removed**. On entry to MainDeployed:

- If `blims_target_set == true`, BLiMS guidance is armed (`blims_armed = true`) and `blims.execute()` is called each cycle.
- If no target was ever set from the ground, the FSW logs `BLiMS: no target set from ground; guidance disabled` and `blims.execute()` is **not** called for the rest of the flight.

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

### Task structure

Three embassy async tasks are spawned at boot from `umbilical::setup()`:

1. **`usb_task`** — Runs the low-level USB peripheral protocol (enumeration, endpoint management). Must be running for the other two tasks to work.
2. **`usb_sender_task`** — `RAW_OUTBOUND.receive().await` → `sender.write_packet()` in a tight loop. Drains telemetry lines, log lines, and dump bytes uniformly. Suspends at `wait_connection()` when no cable is attached.
3. **`usb_receiver_task`** — Reads USB packets, matches command tokens (`<L>`, `<f>`, `<X>`, …), and pushes `UmbilicalCommand` variants into the `COMMANDS` Channel. On `BufferOverflow` the packet is dropped with a `warn!` rather than a panic.

### Shared state between tasks and flight loop

| Static | Type | Writer | Reader | Purpose |
|--------|------|--------|--------|---------|
| `RAW_OUTBOUND` | `Channel<CriticalSectionRawMutex, heapless::Vec<u8, 64>, 32>` | `send_bytes` (telemetry, logs), `print_bytes_async` (dumps) | `usb_sender_task` | All outbound bytes |
| `COMMANDS` | `Channel<CriticalSectionRawMutex, UmbilicalCommand, 4>` | `usb_receiver_task` | `FlightLoop::check_umbilical_commands()` via `try_recv_command()` | Pending commands |
| `IS_CONNECTED` | `AtomicBool` | sender/receiver tasks | `is_connected()` | USB-level connection state |
| `DUMP_IN_PROGRESS` | `AtomicBool` | `begin_dump` / `end_dump` | `emit_telemetry` | Telemetry suppression during flash/FRAM dump |

### Data flow diagram

```
                         GROUND STATION (Host Computer)
                              |           ^
                         USB CDC-ACM (VID 0xC0DE / PID 0xCAFE)
                              |           |
                    [commands down]   [telemetry up]
                              |           |
                              v           |
                    usb_receiver_task   usb_sender_task
                              |           ^
                    COMMANDS Channel   RAW_OUTBOUND Channel
                              |           |  (telemetry CSV
                              |           |   + logs + dumps)
                              v           |
                    FlightLoop          FlightState
                  .check_umbilical    .transmit()
                   _commands()             |
                         |            radio.send()
                         v                 |
                   [actuate valves,        v
                    set flags,        RFD900x Radio
                    reset FRAM,       (same Packet)
                    reboot, etc.]          |
                                           v
                                   emit_telemetry()
                                  → "$TELEM,…\n" into
                                     RAW_OUTBOUND
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
| `umbilical.rs` | Provides `setup()`, `UmbilicalCommand` enum (21 variants incl. `Heartbeat`, `WipeFramReboot`, `KeyArm`, `KeyDisarm`, `SetBlimsTarget { lat, lon }`, payload `N1..N4`), `RAW_OUTBOUND` + `COMMANDS` channels, `emit_telemetry()` (CSV format, gated by `DUMP_IN_PROGRESS`), `print_str` / `print_bytes` / `print_bytes_async`, `begin_dump` / `end_dump`, `try_recv_command()`, debug-only USB serial logger, `usb_task`, `usb_sender_task`, `usb_receiver_task`. The variable-length `<T,lat,lon>` token is parsed in the receiver task. |
| `main.rs` | Single `umbilical::setup()` call. Debug-mode boot delay. |
| `state.rs` | `transmit()` calls `umbilical::emit_telemetry(&self.packet)` to push a `$TELEM,…\n` line. `print_flash_dump` brackets the dump with `umbilical::begin_dump()` / `end_dump()`. The old `trigger_standby` helper has been removed. |
| `flight_loop.rs` | Drains `umbilical::try_recv_command()` each cycle and dispatches all 21 command variants. Owns `handle_launch_sequence` (PreVent → SvToMavWait → MavOpen → Done), the one-shot `recovery_vent_sent` logic in `check_transitions`, and the `blims_target_set` / `blims_armed` flags. |

---

## 8. Remaining TODOs

| Item | Where | Notes |
|------|-------|-------|
| Fill station vent command on disconnect timeout | `flight_loop.rs` | Currently only logs; needs actual command transmission to fill station |
| Buzzer feedback on USB connect/disconnect | `umbilical.rs` (sender task) | Commented out (`buzzer.buzz_num(3)` / `buzzer.buzz_num(2)`); buzzer is not accessible from the async task without shared state |
