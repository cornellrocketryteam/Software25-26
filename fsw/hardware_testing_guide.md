# Pico 2 Flight Software Hardware Testing Guide

This guide details how to compile and run specific hardware verification test loops directly on the Raspberry Pi Pico 2.

We use **Cargo Feature Flags** to isolate these tests. This guarantees that test code is completely isolated and never accidentally compiled into the production flight software binary. Once you run a given test command, the microcontroller will bypass its normal flight loop and execute the isolated test loop indefinitely until disconnected.

---

## 🚀 Getting Started (For New Users)

If you have entirely fresh hardware and want to run these tests, follow these steps:

1. **Clone the Repository**
   ```bash
   git clone https://github.com/cornellrocketryteam/Software25-26.git
   cd Software25-26/fsw
   ```

2. **Checkout the Testing Branch**
   ```bash
   git checkout cfc-test
   ```

3. **Install Dependencies**
   Ensure you have Rust and the `picotool` toolchain installed for your Pico 2. On a Mac, you can install this using Homebrew:
   ```bash
   brew install picotool
   ```

4. **Connect the Hardware**
   Plug the Raspberry Pi Pico 2 into your computer via USB. Hold the `BOOTSEL` button on the board while plugging it in.

---

## 🛠️ Testing Commands

Run the following commands while inside the `fsw/` directory.

### 1. Combined Full Hardware Sequence
Tests all major sensors, actuates the Main Valve, actuates the Solenoid Valve, and fires the Drogue parachute in a repeating loop.

```bash
cargo run --features "test_all"
```

### 2. Main Actuation Valve (MAV)
Repeatedly actuates the MAV Open for 2.5 seconds, then actuates it Closed, and waits 5 seconds before repeating.

```bash
cargo run --features "test_mav"
```

### 3. Solenoid Valve (SV)
Cycles the SV Open for a configurable duration, then closes it and waits 30 seconds before repeating. The duration is set via the `SV_DURATION_SECS` environment variable (in seconds). If not provided, it defaults to 2 seconds.

**With custom duration (e.g. 120 seconds):**
```bash
SV_DURATION_SECS=120 cargo run --features "test_sv"
```

**With default duration (2 seconds):**
```bash
cargo run --features "test_sv"
```

### 4. Parachutes / Solid State Arrays (SSA)
Fires the Drogue Chute pin for 2 seconds, then immediately fires the Main Chute pin for 2 seconds, waiting 5 seconds before repeating.

```bash
cargo run --features "test_ssa"
```

### 5. Buzzer
Triggers the buzzer 3 times, waits 2 seconds, then triggers the buzzer 2 times, waiting 5 seconds before repeating.

```bash
cargo run --features "test_buzzer"
```

### 6. Sensor Verification (Telemetry Spam)
The fastest way to verify all sensor connections. Bypasses the flight state machine and spams a rapid stream of direct readings from the BMP390, GPS, IMU, Magnetometer, and ADC directly to your computer console.

```bash
cargo run --features "test_sensors"
```

### 7. Radio Transceiver (rfd900x)
Tests the radio transceiver of the RFD900x radio. It transmits a packet featuring the latest sensor state, then listens for 5000ms for an incoming packet from a Ground Station or external terminal. It prints any received data to the Serial Monitor.

```bash
cargo run --features "test_radio"
```

---

## ✈️ Returning to Normal Flight Mode

To compile and load the actual, production Flight Software state machine, simply run `cargo run` without supplying any test feature flags:

```bash
cargo run
```
