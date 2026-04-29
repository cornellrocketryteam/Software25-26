# Airbrake Controller Reference

> Living document — update whenever airbrake architecture, phases, or IO pins change.

The Airbrake system is an active drag-control mechanism designed to achieve a precise target apogee. The controller runs as a dual-core application on the FSW Raspberry Pi Pico 2, interacting with an external ODrive S1 motor controller.

---

## Architecture Overview

The airbrake system utilizes the dual-core architecture of the RP2350B microcontroller to ensure real-time control without blocking critical flight operations.

### Dual-Core Execution
* **Core 0 (Main Executor):** Runs the primary flight loop, reads sensors (IMU, Barometer, GPS), manages other actuators, and handles radio/umbilical telemetry.
* **Core 1 (Airbrake Task):** Exclusively runs the airbrake controller logic. 

### Cross-Core Communication
* **Input (Core 0 → Core 1):** Every time Core 0 completes a sensor read, it sends the latest state (time, altitude, gyro, accel, and flight phase) to Core 1 via an Embassy `Signal` (`AIRBRAKE_INPUT`). This signal does not queue; it always holds the most recent data, allowing Core 1 to skip intermediate frames if the binary search computation takes too long.
* **Output (Core 1 → Core 0):** Core 1 writes the calculated deployment level (0.0 to 1.0) and predicted apogee to an `AtomicU32` (`AIRBRAKE_DEPLOYMENT`). Core 0 reads this value in a lock-free, non-blocking manner during its actuator update cycle.

---

## Flight Phases

The airbrake controller operates differently depending on the overall flight mode of the rocket:

| FSW FlightMode | AirbrakePhase | Controller Behaviour |
|---|---|---|
| Startup / Standby | `Pad` | Collects gyro/accel calibration data. Airbrakes fully retracted (0%). |
| Ascent | `Boost` | Tracks velocity and altitude during the motor burn. Airbrakes fully retracted (0%). |
| Coast | `Coast` | **Active Control Phase.** Runs binary-search deployment control to hit target apogee. Deploys airbrakes between 0% and 100%. |
| DrogueDeployed+ | *(not sent)* | Core 1 blocks. Core 0 forces the `AirbrakeActuator` to retract immediately and stay at 0% to prevent tangling. |

---

## Hardware Interface (ODrive S1)

The mechanical airbrakes are driven by a brushless motor managed by an **ODrive S1** motor controller. The FSW interfaces with the ODrive via standard RC PWM.

### Pinout
| Pin | Function | Description |
|---|---|---|
| **GPIO 37** | `ENABLE` | Digital Output. High = Enable ODrive, Low = Disable ODrive. |
| **GPIO 38** | `PWM` | RC PWM Output (50 Hz frame rate, 20 ms period). Connected to ODrive isolated IO (G08). |

### Deployment Mapping
The airbrake deployment maps directly to the PWM pulse width:
* **0.0 (0%) - Fully Retracted:** 1000 µs pulse
* **1.0 (100%) - Fully Deployed:** 2000 µs pulse

The actuator class (`AirbrakeActuator` in `actuator.rs`) clamps all deployment requests between 0.0 and 1.0.

---

## Safety & Failsafes

* **Phase Guard:** If the flight software enters `DrogueDeployed`, `MainDeployed`, or `Fault` modes, Core 0 will bypass the Core 1 signal and command the `AirbrakeActuator` directly to retract (1000 µs).
* **Safe Initialization:** The system initializes with a 1000 µs pulse (fully retracted) to prevent accidental deployment on the pad.
* **Watchdog Isolation:** The controller executes on Core 1 without using `CriticalSectionRawMutex` for logging, ensuring that the 20 Hz calculation loop does not block Core 0's I²C/SPI transactions or trip the hardware watchdog.
