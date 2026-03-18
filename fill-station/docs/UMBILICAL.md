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
serial commands & 80-byte binary telemetry packets
   |   |
   V   |
Flight Software (FSW)
```

## Features

- **Telemetry Parsing**: 80-byte binary Packets (`FswTelemetry`) are read via serial, validated, and broadcast over WebSocket to connected clients.
- **FSW Command Translation**: JSON commands received via WebSocket are forwarded over the umbilical as single-character commands to the FSW.

### FSW Telemetry Data Structure

The 80-byte binary structure `FswTelemetry` from the FSW is parsed inside `src/components/umbilical.rs`. The properties available to clients are:

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
- `fsw_reset_card`: Reset the FSW's SD card writer (`<D>`).
- `fsw_reboot`: Force a software reboot on FSW (`<R>`).
- `fsw_dump_flash`: Dump flash memory contents (`<G>`).
- `fsw_wipe_flash`: Wipe flash memory (`<W>`).
- `fsw_flash_info`: Query flash info (`<I>`).
- `fsw_payload_n1`: Trigger payload event N1 (`<1>`).
- `fsw_payload_n2`: Trigger payload event N2 (`<2>`).
- `fsw_payload_n3`: Trigger payload event N3 (`<3>`).
- `fsw_payload_n4`: Trigger payload event N4 (`<4>`).

For the exact payload and response format for these commands, refer to the [WEBSOCKET_API.md](WEBSOCKET_API.md).
