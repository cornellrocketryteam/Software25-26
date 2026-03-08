# QD Stepper Motor

## Overview

The QD (Quick Disconnect) stepper motor component drives a NEMA 17 stepper motor through an ISD02 integrated stepper driver. It is used to actuate the quick disconnect mechanism on the fill system. The motor is controlled via three signals from the TI AM64x SK board:

- **STEP** (PWM pulse train) — each pulse advances the motor one step
- **DIR** (GPIO) — sets rotation direction
- **ENA** (GPIO) — enables/disables the driver

All move operations run as **non-blocking background tasks**, identical to the igniter pattern. The WebSocket command returns `success` immediately and the motor runs in the background.

## Hardware

### Driver: ISD02

| Parameter | Value |
|-----------|-------|
| Supply Voltage | 10-28 VDC |
| Max Output Current | 2A per phase (adjustable via trimmer) |
| Microstepping | Full step (200 steps/rev) via DIP switches |
| Max Step Frequency (full step) | 12 KHz |
| Min Pulse Width | > 4 us |
| Enable Wake Delay | 1 ms after ENA goes high |
| Inputs | Opto-isolated (3.3-5V VCC, no series resistor needed) |

The ISD02 spec sheet is located at `src/components/ISD02-04-08.pdf`.

### Motor: NEMA 17

| Parameter | Value |
|-----------|-------|
| Step Angle | 1.8 degrees |
| Steps per Revolution | 200 (full step mode) |

### Pin Assignments

| Signal | Interface | Board Pin | Code Reference |
|--------|-----------|-----------|----------------|
| STEP | PWM sysfs | EHRPWM4 Channel A (pwmchip0, channel 0) | Same chip as MAV (channel 1) |
| DIR | GPIO | gpiochip2 (chip1), line 43 | `chip1, 43` in hardware.rs |
| ENA | GPIO | gpiochip2 (chip1), line 64 | `chip1, 64` in hardware.rs |

**ENA logic (per ISD02 spec):**
- HIGH or unconnected = driver enabled (motor powered)
- LOW = driver disabled (motor unpowered, free to rotate)

**DIR logic:**
- HIGH = one direction (mapped to "open" in code)
- LOW = opposite direction (mapped to "close" in code)

## Software Architecture

### Component: `src/components/qd_stepper.rs`

The `QdStepper` struct manages PWM via sysfs (same approach as the MAV) and GPIO via `async-gpiod` (same approach as the Ball Valve).

### Stepping Mechanism

To move the motor N steps:

1. **Set DIR** GPIO high or low based on desired direction
2. **Set ENA** GPIO high (ensure driver is enabled)
3. **Wait 2 ms** for driver to wake from possible idle state
4. **Configure PWM** — set 50% duty cycle at the configured step frequency
5. **Enable PWM** — hardware generates pulses automatically
6. **Sleep** for `N / frequency` seconds (mutex released during sleep)
7. **Disable PWM** — motor stops

The step frequency is set to **1 KHz** (1000 steps/second), well under the 12 KHz maximum for full-step mode. At this rate:
- 200 steps (1 revolution) takes **200 ms**
- 50% duty cycle = 500 us pulse width, far exceeding the 4 us minimum

### Background Task Pattern

All move commands use the same lock/sleep/lock pattern as the igniter:

```
1. Lock hardware mutex
2. Call begin_stepping(direction)  — sets GPIO + starts PWM
3. Release mutex
4. Sleep for computed duration     — other commands can execute
5. Lock hardware mutex
6. Call stop_stepping()            — disables PWM
7. Release mutex
```

This ensures the hardware mutex is NOT held during the step duration, so ADC reads, valve commands, and other operations continue uninterrupted.

## Configuration

### Stepping Constants (`src/components/qd_stepper.rs`)

```rust
const STEP_FREQUENCY_HZ: u32 = 1000;  // Step rate in Hz (max 12000 for full-step)
const ENABLE_WAKE_MS: u64 = 2;        // Delay after enable before pulsing
```

### Preset Constants (calibrate on hardware)

```rust
pub const QD_OPEN_STEPS: u32 = 200;        // Steps for open preset
pub const QD_CLOSE_STEPS: u32 = 200;       // Steps for close preset
pub const QD_OPEN_DIRECTION: bool = true;   // Direction for open
pub const QD_CLOSE_DIRECTION: bool = false; // Direction for close
```

These are **placeholder values** (1 full revolution each). They must be calibrated on the actual QD mechanism to determine the correct step count and direction for open/close operations.

## WebSocket Commands

### `qd_move`
Move the QD stepper a specific number of steps in a given direction. Runs as a background task.

**Format:**
```json
{"command": "qd_move", "steps": 100, "direction": true}
```
- `steps`: Number of full steps to execute (u32)
- `direction`: `true` for one direction, `false` for the other

**Response:**
```json
{"type": "success"}
```
Returns immediately. Motor runs in background.

---

### `qd_open`
Execute the QD open preset (uses `QD_OPEN_STEPS` and `QD_OPEN_DIRECTION` constants).

**Format:**
```json
{"command": "qd_open"}
```

**Response:**
```json
{"type": "success"}
```

---

### `qd_close`
Execute the QD close preset (uses `QD_CLOSE_STEPS` and `QD_CLOSE_DIRECTION` constants).

**Format:**
```json
{"command": "qd_close"}
```

**Response:**
```json
{"type": "success"}
```

## CSV Logging

The QD stepper state is logged at 100 Hz in the CSV data file with two columns appended at the end of each row:

| Column | Description |
|--------|-------------|
| `QD_Enabled` | Boolean — `true` if the ENA GPIO is high (driver enabled) |
| `QD_Direction` | Boolean — `true` if the DIR GPIO is high |

## Testing

### Mock Testing (macOS)
The QD stepper compiles and runs on non-Linux platforms with mock stubs. All operations log to tracing output instead of touching hardware.

```bash
cargo run --release
```

Then send commands via websocat:
```bash
websocat ws://localhost:9000
{"command": "qd_move", "steps": 50, "direction": true}
{"command": "qd_open"}
{"command": "qd_close"}
```

Check logs for:
```
QD 'QD': begin stepping, direction=OPEN
QD 'QD': stop stepping
QD move complete (50 steps)
```

### Hardware Testing
1. Connect oscilloscope to EHRPWM4_A output
2. Verify pulse train at 1 KHz with 50% duty cycle
3. Verify DIR GPIO toggles with direction parameter
4. Verify ENA GPIO is high during operation
5. Count pulses to confirm step accuracy

## Calibration Procedure

To determine the correct preset values:

1. Send `qd_move` commands with small step counts (e.g., 10-50) to understand the mechanism's range of motion
2. Determine which `direction` value corresponds to open vs close
3. Count the total steps needed for a full open and full close
4. Update the constants in `src/components/qd_stepper.rs`:
   ```rust
   pub const QD_OPEN_STEPS: u32 = <your_value>;
   pub const QD_CLOSE_STEPS: u32 = <your_value>;
   pub const QD_OPEN_DIRECTION: bool = <true_or_false>;
   pub const QD_CLOSE_DIRECTION: bool = <true_or_false>;
   ```
5. Rebuild and redeploy

## Troubleshooting

### Motor doesn't move
- Verify ISD02 power supply (10-28 VDC) is connected
- Check that ENA pin is HIGH (driver enabled) — a LOW signal disables the driver
- Verify motor phase wiring (A+/A-, B+/B-) matches ISD02 P2 connector
- Check DIP switches are set for full-step mode (both switches OFF on old version)

### Motor moves wrong direction
- Swap `QD_OPEN_DIRECTION` / `QD_CLOSE_DIRECTION` constants
- Or swap motor phase wiring (swap A+/A- or B+/B-)

### Motor skips steps or stalls
- Reduce `STEP_FREQUENCY_HZ` (try 500 Hz)
- Increase ISD02 output current via the onboard trimmer potentiometer
- Ensure supply voltage is adequate (higher voltage = better high-speed performance)

### PWM channel won't export
- Check that the device tree overlay enables EHRPWM4
- Verify `/sys/class/pwm/pwmchip0` exists on the target board
- The PWM channel shares a chip with the MAV — ensure both channels can coexist
