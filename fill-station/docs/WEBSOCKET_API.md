# WebSocket API Reference

This document provides a reference for all supported WebSocket commands for the fill station server.

## Connection Details
- **Protocol**: WebSocket
- **Port**: 9000
- **URL**: `ws://[board-ip]:9000`

---

## Commands

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
Begins pushing real-time ADC data to the connecting client at 10 Hz.

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
Sent periodically (at 10 Hz) after a `start_adc_stream` command.

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
- `scaled`: Pressure sensor value (only for ADC1 Ch0 and Ch1, `null` otherwise).

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
Query the current state of a solenoid valve (actuation status and continuity).

**Format:**
```json
{"command": "get_valve_state", "valve": "SV1"}
```

**Response:**
```json
{
  "type": "valve_state",
  "actuated": true,
  "continuity": false
}
```
*   `actuated`: `true` if logically actuated (open).
*   `continuity`: `true` if continuity detected (signal high).

---

### `actuate_valve`
Actuates (opens/energizes) or de-actuates (closes/de-energizes) a specific solenoid valve.

**Format:**
```json
{"command": "actuate_valve", "valve": "SV1", "state": true}
```
*   `valve`: Valve identifier ("SV1" through "SV5", case-insensitive).
    *   *Note: For Normally Closed (NC) valves (SV1-SV4), `true` = HIGH (Open). For Normally Open (SV5), `true` = LOW (Open).*

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
  "pulse_width_us": 1300
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
*   `angle`: Target angle in degrees (0.0 to 90.0).

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