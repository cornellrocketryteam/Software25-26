# WebSocket API Reference

This document provides a reference for all supported WebSocket commands for the fill station server.

## Connection Details
- **Protocol**: WebSocket
- **Port**: 9000
- **URL**: `ws://[board-ip]:9000`

---

## Commands

### `ignite`
Fires both igniters concurrently for 3 seconds.

**Format:**
```json
{"command": "ignite"}
```

**Response:**
```json
{"type": "success"}
```
*Returns `{"type": "error"}` if the command fails (e.g., non-Linux platform).*

---

### `start_adc_stream`
Begins pushing real-time, 10x averaged ADC data to the connecting client at 10 Hz.

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

### `actuate_valve`
Actuates (opens/energizes) or de-actuates (closes/de-energizes) a specific solenoid valve.

**Format:**
```json
{"command": "actuate_valve", "valve": "SV1", "state": true}
```
*   `valve`: Valve identifier ("SV1", "SV2", "SV3", "SV4", "SV5", case-insensitive).
*   `state`: `true` to actuate (open), `false` to de-actuate (close).
    *   *Note: For Normally Closed (NC) valves, `true` = Open. For Normally Open (NO) like SV5, `true` = Closed (Energized).*

**Response:**
```json
{"type": "success"}
```

---

## Server Push Messages

### `adc_data`
Sent periodically (at 10 Hz) after a `start_adc_stream` command. Note that each reading is an **arithmetic average of 10 samples** to reduce signal noise.

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
- `raw`: Raw 12-bit ADC value (-2048 to 2047), averaged over 10 samples.
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
{"type": "error"}
```
