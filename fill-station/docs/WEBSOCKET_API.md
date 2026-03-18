# WebSocket API Reference

This document provides a reference for all supported WebSocket commands for the fill station server.

## Connection Details
- **Protocol**: WebSocket
- **Port**: 9000
- **URL**: `ws://[board-ip]:9000`

---
## Safety & Heartbeat

### Connection Monitoring
The server enforces a **15-second timeout** on idle connections to ensure safety. 
- If no message is received from a connected client for 15 seconds, the server will:
  1. **Close SV1**.
  2. **Close the MAV**.
  3. **Disconnect the client**.

To prevent this, clients must send any valid JSON command at least once every 15 seconds. If no other command is needed, use the `heartbeat` command.

---

## Commands

### `heartbeat`
Keep the connection alive without performing any action.

**Format:**
```json
{"command": "heartbeat"}
```

**Response:**
```json
{"type": "success"}
```

---

### `get_igniter_continuity`
Query the continuity of a specific igniter.

**Format:**
```json
{"command": "get_igniter_continuity", "id": 1}
```

**Response:**
```json
{
  "type": "igniter_continuity",
  "id": 1,
  "continuity": true
}
```
*   `id`: 1 for Igniter 1, 2 for Igniter 2.
*   `continuity`: `true` if circuit is closed (continuity present), `false` otherwise.

### `ignite`
Initiates a **non-blocking** ignition sequence. Fires both igniters concurrently for 3 seconds in a background task. 

**Format:**
```json
{"command": "ignite"}
```

**Response:**
```json
{"type": "success"}
```
*   Returns `{"type": "success"}` **immediately** upon starting the sequence.
*   The ADC stream and other commands will continue to function normally during the 3-second ignition window.
*   Returns `{"type": "error"}` if the command fails to start (e.g., non-Linux platform).

---

### `start_adc_stream`
Begins pushing real-time ADC data to the connecting client at 100 Hz.

**Format:**
```json
{"command": "start_adc_stream"}
```

**Response:**
```json
{"type": "success"}
```
*The server will then begin sending `adc_data` messages (see below).*

---

### `stop_adc_stream`
Stops the ADC data stream for the current client.

**Format:**
```json
{"command": "stop_adc_stream"}
```

**Response:**
```json
{"type": "success"}
```

---

## Server Push Messages

### `adc_data`
Sent periodically (at 100 Hz) after a `start_adc_stream` command.

**Format:**
```json
{
  "type": "adc_data",
  "timestamp_ms": 1734678123456,
  "valid": true,
  "adc1": [
    {
      "raw": 1234,
      "voltage": 2.468,
      "scaled": 4.876
    },
    { "raw": 100, "voltage": 0.2, "scaled": 1.5 },
    { "raw": 0, "voltage": 0.0, "scaled": null },
    { "raw": 0, "voltage": 0.0, "scaled": null }
  ],
  "adc2": [
    { "raw": 567, "voltage": 1.134, "scaled": null },
    { "raw": 0, "voltage": 0.0, "scaled": null },
    { "raw": 0, "voltage": 0.0, "scaled": null },
    { "raw": 0, "voltage": 0.0, "scaled": null }
  ]
}
```

**Field Descriptions:**
- `timestamp_ms`: Unix timestamp in milliseconds when readings were taken.
- `valid`: `true` if readings are fresh, `false` if ADC read failed.
- `raw`: Raw 12-bit ADC value (-2048 to 2047).
- `voltage`: Calculated voltage based on gain setting.
- `scaled`: Scaled sensor value — PT1 (ADC1 Ch0), PT2 (ADC1 Ch1), Load Cell (ADC2 Ch1). `null` for all other channels.

---

## Generic Responses

### `success`
Command was received and executed successfully.
```json
{"type": "success"}
```

### `error`
An error occurred during command parsing or execution.
```json
{"type": "error", "message": "Description of what went wrong"}
```

---

## Solenoid Valve Commands

### `get_valve_state`
Query the current state of a solenoid valve (open/closed and continuity).

**Format:**
```json
{"command": "get_valve_state", "valve": "SV1"}
```

**Response:**
```json
{
  "type": "valve_state",
  "open": true,
  "continuity": false
}
```
*   `open`: `true` if the valve is open, `false` if closed.
*   `continuity`: `true` if continuity detected (signal high).

---

### `actuate_valve`
Opens or closes a specific solenoid valve. The server automatically handles the correct GPIO level based on whether the valve is Normally Open (NO) or Normally Closed (NC).

**Format:**
```json
{"command": "actuate_valve", "valve": "SV1", "open": true}
```
*   `valve`: Valve identifier ("SV1", case-insensitive).
*   `open`: `true` to open the valve, `false` to close it.

**Response:**
```json
{"type": "success"}
```

---

## MAV Commands

### `get_mav_state`
Query the current state of the MAV (angle and pulse width).

**Format:**
```json
{"command": "get_mav_state", "valve": "MAV"}
```

**Response:**
```json
{
  "type": "mav_state",
  "angle": 45.0,
  "pulse_width_us": 1422
}
```

---

### `set_mav_angle`
Sets the angle of the Mechanically Actuated Valve (MAV) servo.

**Format:**
```json
{"command": "set_mav_angle", "valve": "MAV", "angle": 45.0}
```
*   `valve`: Valve identifier (currently "MAV").
*   `angle`: Target angle in degrees (0.0 to 126.0).

**Response:**
```json
{"type": "success"}
```

---

### `mav_open`
Opens the MAV to its maximum position (90 degrees).

**Format:**
```json
{"command": "mav_open", "valve": "MAV"}
```

**Response:**
```json
{"type": "success"}
```

---

### `mav_close`
Closes the MAV to its minimum position (0 degrees).

**Format:**
```json
{"command": "mav_close", "valve": "MAV"}
```

**Response:**
```json
{"type": "success"}
```

---

### `mav_neutral`
Sets the MAV to its neutral position (1300 µs).

**Format:**
```json
{"command": "mav_neutral", "valve": "MAV"}
```

**Response:**
```json
{"type": "success"}
```

---

## Ball Valve Commands

### `bv_open`
Execute the opening sequence for the Ball Valve (Signal HIGH & ON_OFF HIGH -> 3s wait -> ON_OFF LOW).

**Format:**
```json
{"command": "bv_open"}
```

**Response:**
```json
{"type": "success"}
```

---

### `bv_close`
Execute the closing sequence for the Ball Valve (Signal LOW & ON_OFF HIGH -> 3s wait -> ON_OFF LOW).

**Format:**
```json
{"command": "bv_close"}
```

**Response:**
```json
{"type": "success"}
```

---

### `bv_signal`
Set the Ball Valve signal line state manually. Only allowed if ON_OFF is LOW.

**Format:**
```json
{"command": "bv_signal", "state": "high"}
```
* `state`: "high", "low", "open", "close", "true", "false"

**Response:**
```json
{"type": "success"}
```

---

### `bv_on_off`
Set the Ball Valve ON_OFF line state manually.

**Format:**
```json
{"command": "bv_on_off", "state": "high"}
```
* `state`: "high", "low", "on", "off", "true", "false"

**Response:**
```json
{"type": "success"}
```

---

## QD Stepper Commands

### `qd_move`
Move the QD stepper motor a specific number of steps. Runs as a **non-blocking background task** (returns immediately).

**Format:**
```json
{"command": "qd_move", "steps": 100, "direction": true}
```
* `steps`: Number of full steps to execute.
* `direction`: `true` for CW (retract), `false` for CCW (extend).

**Response:**
```json
{"type": "success"}
```

---

### `qd_retract`
Execute the QD retract preset (CW, preconfigured number of steps). Non-blocking.

**Format:**
```json
{"command": "qd_retract"}
```

**Response:**
```json
{"type": "success"}
```

---

### `qd_extend`
Execute the QD extend preset (CCW, preconfigured number of steps). Non-blocking.

**Format:**
```json
{"command": "qd_extend"}
```

**Response:**
```json
{"type": "success"}
```

See [QD_STEPPER.md](QD_STEPPER.md) for hardware details, calibration, and configuration.

---

## FSW Umbilical Commands

These commands interface with the Flight Software over the USB Umbilical. 

### `start_fsw_stream`
Begin streaming parsed FSW telemetry data (`fsw_telemetry`) to the client as fast as it arrives.

**Format:**
```json
{"command": "start_fsw_stream"}
```

**Response:**
```json
{"type": "success"}
```

---

### `stop_fsw_stream`
Stop streaming FSW telemetry data.

**Format:**
```json
{"command": "stop_fsw_stream"}
```

**Response:**
```json
{"type": "success"}
```

---

### FSW Actuators and States

The following commands send simple 1-byte command characters over the serial connection to the Flight Software. They all follow the same format and response structure.

*   `fsw_launch` — Trigger launch sequence (`<L>`)
*   `fsw_open_mav` — Open MAV on vehicle (`<M>`)
*   `fsw_close_mav` — Close MAV on vehicle (`<m>`)
*   `fsw_open_sv` — Open SV on vehicle (`<S>`)
*   `fsw_close_sv` — Close SV on vehicle (`<s>`)
*   `fsw_safe` — Safe all FSW actuators (`<V>`)
*   `fsw_reset_fram` — Clear FRAM data (`<F>`)
*   `fsw_reset_card` — Reset SD card writer (`<D>`)
*   `fsw_reboot` — Reboot FSW (`<R>`)
*   `fsw_dump_flash` — Dump flash memory (`<G>`)
*   `fsw_wipe_flash` — Wipe flash memory (`<W>`)
*   `fsw_flash_info` — Query flash info (`<I>`)
*   `fsw_payload_n1` — Payload event N1 (`<1>`)
*   `fsw_payload_n2` — Payload event N2 (`<2>`)
*   `fsw_payload_n3` — Payload event N3 (`<3>`)
*   `fsw_payload_n4` — Payload event N4 (`<4>`)

**Format:**
```json
{"command": "fsw_launch"} 
```

**Response:**
```json
{"type": "success"}
```

---

### Push Message `fsw_telemetry`
Data received back from the Flight software, pushed to clients when `start_fsw_stream` is active.

**Format:**
```json
{
  "type": "fsw_telemetry",
  "timestamp_ms": 1734678125456,
  "connected": true,
  "flight_mode": "Standby",
  "telemetry": {
    "flight_mode": 1,
    "pressure": 101325.0,
    "temp": 22.5,
    "altitude": 10.0,
    "latitude": 42.44,
    "longitude": -76.48,
    "num_satellites": 8,
    "timestamp": 12.34,
    "mag_x": 0.0, "mag_y": 0.0, "mag_z": 0.0,
    "accel_x": 0.0, "accel_y": 0.0, "accel_z": 9.81,
    "gyro_x": 0.0, "gyro_y": 0.0, "gyro_z": 0.0,
    "pt3": 1200.0,
    "pt4": 1500.0,
    "rtd": 500.0
  }
}
```

* `connected`: True if the background task can communicate with the serial device.
* `flight_mode`: Human readable string.
* `telemetry`: The full 82-byte `FswTelemetry` packet parsed into JSON variables.