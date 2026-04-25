# Umbilical Connection

The `fill-station` software now includes an **Umbilical** connection to the Flight Software (FSW). This feature allows the fill station to act as a bridge between the ground control clients (via WebSocket) and the FSW running on the vehicle (via a USB CDC-ACM serial connection).

## Architecture

The umbilical connection revolves around a background task in `src/main.rs` that polls `/dev/ttyACM0` at a default rate. The hardware implementation is transparent to the end user and handled via `smol`'s async channels.

```
WebSocket Client
   |   ^
   |   |
  json commands & telemetry stream
   |   |
   V   |
fill-station (Umbilical background task)
   |   ^
   |   |
serial commands & `$TELEM,<22 fields>\n` CSV telemetry lines
   |   |
   V   |
Flight Software (FSW)
```

## Features

- **Telemetry Parsing**: The FSW emits one telemetry record per line as `$TELEM,<56 comma-separated fields>\n`. The umbilical task line-buffers the serial stream, parses each `$TELEM,` line via `FswTelemetry::from_csv` (strict 56-field match — see `TELEM_FIELD_COUNT`), and broadcasts the result over WebSocket. Non-`$TELEM` lines are forwarded to debug logs.
- **Sync on (re)connect**: The first two newline-terminated chunks after opening the serial port are discarded so a partial line picked up mid-stream cannot produce a garbage frame.
- **Line buffer cap**: If `\n` never arrives (FSW hung mid-line), the line buffer is cleared with a warning at 8 KB.
- **Dump suppression**: While the FSW is mid-flash-dump it sets an internal `DUMP_IN_PROGRESS` flag and stops emitting `$TELEM` lines. Telemetry pauses for the duration of the dump and resumes automatically afterward.
- **FSW Command Translation**: JSON commands received via WebSocket are forwarded over the umbilical as `<X>`-style ASCII tokens to the FSW.

### FSW Telemetry Data Structure

`FswTelemetry` is parsed from the 56 CSV fields of each `$TELEM,` line in `src/components/umbilical.rs`. The fields, in order, are:

| Field | Type | Unit | Description |
|-------|------|------|-------------|
| `flight_mode` | `u32` | Enum | The current state of the flight control finite state machine (e.g., Startup, Standby, Ascent) |
| `pressure` | `f32` | Pa | Barometric pressure |
| `temp` | `f32` | C | Internal temperature |
| `altitude` | `f32` | m | Calculated altitude |
| `latitude` | `f32` | deg | GPS Latitude |
| `longitude`| `f32` | deg | GPS Longitude |
| `num_satellites` | `u32` | count | GPS Satellites locked |
| `timestamp`| `f32` | s | Uptime |
| `mag_x/y/z`| `f32` | uT | Magnetometer readings |
| `accel_x/y/z`| `f32` | m/s^2 | Accelerometer readings |
| `gyro_x/y/z` | `f32` | deg/s | Gyroscope readings |
| `pt3/pt4` | `f32` | counts | Additional pressure transducer readings on the vehicle |
| `rtd` | `f32` | counts | Resistance Temperature Detector reading |
| `sv_open` | `bool` | — | Separation Valve actuation state |
| `mav_open` | `bool` | — | MAV actuation state |
| `ssa_drogue_deployed` | `u8` | flag | Drogue parachute deployment triggered (0 or 1) |
| `ssa_main_deployed` | `u8` | flag | Main parachute deployment triggered (0 or 1) |
| `cmd_n1` | `u8` | flag | Payload event N1 triggered |
| `cmd_n2` | `u8` | flag | Payload event N2 triggered |
| `cmd_n3` | `u8` | flag | Payload event N3 triggered |
| `cmd_n4` | `u8` | flag | Payload event N4 triggered |
| `cmd_a1/a2/a3` | `u8` | flag | Actuator/command events A1, A2, A3 triggered |
| `airbrake_state` | `u8` | enum | Airbrake system state |
| `predicted_apogee` | `f32` | m | Airbrake controller predicted apogee |
| `h_acc/v_acc` | `u32` | mm | GPS horizontal/vertical accuracy estimate |
| `vel_n/e/d` | `f64` | m/s | GPS North/East/Down velocity |
| `g_speed` | `f64` | m/s | GPS ground speed |
| `s_acc` | `u32` | mm/s | GPS speed accuracy estimate |
| `head_acc` | `u32` | deg*1e5 | GPS heading accuracy estimate |
| `fix_type` | `u8` | enum | GPS fix type |
| `head_mot` | `i32` | deg*1e5 | GPS heading of motion |
| `blims_motor_position` | `f32` | deg | BLiMS parafoil motor position |
| `blims_phase_id` | `i8` | enum | BLiMS control phase |
| `blims_pid_p/i` | `f32` | — | BLiMS PID values |
| `blims_bearing` | `f32` | deg | BLiMS bearing |
| `blims_loiter_step` | `i8` | — | BLiMS loiter step |
| `blims_heading_des` | `f32` | deg | BLiMS desired heading |
| `blims_heading_error` | `f32` | deg | BLiMS heading error |
| `blims_error_integral` | `f32` | — | BLiMS error integral |
| `blims_dist_to_target_m` | `f32` | m | BLiMS distance to target |
| `blims_target_lat/lon` | `f32` | deg | BLiMS configured target coordinate |
| `blims_wind_from_deg` | `f32` | deg | BLiMS estimated wind direction |

## WebSocket API Extentions

When streaming FSW telemetry (`start_fsw_stream`), clients receive JSON updates. 

### Stream Commands
- `start_fsw_stream`: Start streaming parsed telemetry from FSW over WebSocket.
- `stop_fsw_stream`: Stop streaming telemetry.

### FSW Actuation Commands
Commands sent over the WebSocket that get parsed and sent across the serial connection to FSW:
- `fsw_launch`: Trigger Launch sequence (`<L>`).
- `fsw_open_mav`: Open the MAV on the vehicle (`<M>`).
- `fsw_close_mav`: Close the MAV on the vehicle (`<m>`).
- `fsw_open_sv`: Open the vehicle Solenoid Valve (`<S>`).
- `fsw_close_sv`: Close the vehicle Solenoid Valve (`<s>`).
- `fsw_safe`: Safe all FSW actuators (`<V>`).
- `fsw_reset_fram`: Clear the FSW's FRAM data (`<F>`).
- `fsw_dump_fram`: Dump FRAM contents over the umbilical (`<f>`).
- `fsw_wipe_fram_reboot`: Wipe FRAM and immediately reboot (`<X>`).
- `fsw_key_arm`: Arm the launch key (`<K>`); required to allow Startup → Standby.
- `fsw_key_disarm`: Disarm the launch key (`<k>`); reverts Standby → Startup.
- `fsw_set_blims_target`: Set BLiMS landing-zone target (`<T,lat,lon>`); takes two `f32` decimal-degree numbers.
- `fsw_reboot`: Force a software reboot on FSW (`<R>`).
- `fsw_dump_flash`: Dump flash memory contents (`<G>`).
- `fsw_wipe_flash`: Wipe flash memory (`<W>`).
- `fsw_flash_info`: Query flash info (`<I>`).
- `fsw_payload_n1`: Trigger payload event N1 (`<1>`).
- `fsw_payload_n2`: Trigger payload event N2 (`<2>`).
- `fsw_payload_n3`: Trigger payload event N3 (`<3>`).
- `fsw_payload_n4`: Trigger payload event N4 (`<4>`).

> **Removed:** `fsw_reset_card` (`<D>`) and `fsw_fault_mode` (the old name for what is now `fsw_wipe_fram_reboot`) are no longer recognized.

For the exact payload and response format for these commands, refer to the [WEBSOCKET_API.md](WEBSOCKET_API.md).
