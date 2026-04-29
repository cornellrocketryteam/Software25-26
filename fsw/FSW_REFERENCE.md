# Flight Software (FSW) Reference Document

**Platform:** Raspberry Pi Pico 2 (RP2350, ARM Cortex-M, 2 MiB Flash / 512 KiB RAM)
**Language:** Rust (no_std, Embassy async runtime)
**Main Loop Cycle:** 1000 ms (1 Hz)

---

## Current Capabilities

### Flight State Machine

The FSW runs a 7-phase state machine defined in `state.rs` (`FlightMode` enum) with transition logic in `flight_loop.rs`:

| Phase | ID | Entry Condition | Key Actions |
|-------|----|-----------------|-------------|
| **Startup** | 0 | Power-on | Read sensors, validate altimeter, set reference pressure, wait for ground `<K>` (Key Arm) command |
| **Standby** | 1 | `key_armed = true` (set via umbilical `<K>`) + altimeter valid | Monitor sensors, check umbilical, wait for launch command. `<k>` reverts to Startup. |
| **Ascent** | 2 | Umbilical launch command received | Open MAV + SV, rapid data collection, log to FRAM |
| **Coast** | 3 | MAV auto-closes (~530 ms) | Apogee detection via 10-sample moving average + 3-point descending trend |
| **DrogueDeployed** | 4 | Apogee detected (filtered altitude descending) | Fire drogue SSA, wait for main deploy altitude |
| **MainDeployed** | 5 | Altitude < 610 m + 1 s delay after drogue | Fire main SSA, BLiMS initiation, 20-minute log timeout |
| **Fault** | 6 | Altimeter reads invalid | Halt autonomous control, persist state to FRAM |

### Sensors

| Sensor | Driver File | Bus | Address | Data Provided |
|--------|-------------|-----|---------|---------------|
| **BMP390** (altimeter) | `driver/bmp390.rs` | SPI | 0x77 | Pressure (Pa), temperature (°C), altitude (m) |
| **LSM6DSOX** (IMU) | `driver/lsm6dsox.rs` | I2C0 | 0x6A | Accel XYZ (m/s²), gyro XYZ (°/s) |
| **ADS1015** (ADC) | `driver/ads1015.rs` | I2C0 | 0x48 | PT3, PT4, RTD (scaled) |
| **u-blox MAX-M10S** (GPS) | `driver/ublox_max_m10s.rs` | I2C0 | 0x42 | Latitude, longitude, satellite count, timestamp |

All I2C sensors share a single bus (GPIO 0 SDA / GPIO 1 SCL, 400 kHz) through `embassy_embedded_hal::shared_bus`.

### Actuators

| Actuator | Driver | Pin | Type | Purpose |
|----------|--------|-----|------|---------|
| **SSA Drogue** | `actuator.rs` → `Ssa` | GPIO 36 | Digital output | Fire drogue e-match (1 s pulse) |
| **SSA Main** | `actuator.rs` → `Ssa` | GPIO 39 | Digital output | Fire main e-match (1 s pulse) |
| **Buzzer** | `actuator.rs` → `Buzzer` | GPIO 21 | Digital output | Audio status beeps (100 ms on/off) |
| **MAV** (vent servo) | `actuator.rs` → `Mav` | GPIO 40 | PWM (~330 Hz) | Motor Actuated Vent, servo position 0.0–1.0 |
| **SV** (separation valve) | `actuator.rs` → `SV` | GPIO 47 | Digital output (active low) | Binary valve open/close |

### Communications

| System | Driver File | Interface | Details |
|--------|-------------|-----------|---------|
| **RFD900x Radio** | `driver/rfd900x.rs` | UART1 (GPIO 4 TX / GPIO 5 RX, 115200 baud) | Transmit-only. 4-byte sync (`0x3E5D5967`) + 199-byte packet at 1 Hz |
| **USB Logger** | Built-in (embassy-usb-logger) | USB CDC-ACM | Debug log output, 1024-byte buffer |
| **Umbilical** | `umbilical.rs` | USB CDC-ACM | Command parser (H=heartbeat, L=launch, M/m=MAV, S/s=SV, V=safe, F=resetFRAM, f=dumpFRAM, R=reboot, G/W/I=flash dump/wipe/info, X=wipeFRAM+reboot, KA/KD=key arm/disarm, D/d=Trigger Drogue/Main, `<T,lat,lon>`=set BLiMS target, 1–4=payload N events, A1-A3=payload A events). Drained by `flight_loop.rs::check_umbilical_commands` each cycle. |

### Telemetry Packet

199-byte struct (`packet.rs`) transmitted each cycle via Radio, and emitted as a 57-field CSV via the Umbilical:

```text
Bytes 0x00–0x03: flight_mode (u32)
Bytes 0x04–0x0F: pressure (f32), temp (f32), altitude (f32)
Bytes 0x10–0x1F: latitude (f32), longitude (f32), num_satellites (u32), timestamp (f32)
Bytes 0x20–0x2B: mag_x (f32), mag_y (f32), mag_z (f32)
Bytes 0x2C–0x37: accel_x (f32), accel_y (f32), accel_z (f32)
Bytes 0x38–0x43: gyro_x (f32), gyro_y (f32), gyro_z (f32)
... (extends to 199 bytes including BLiMS, Airbrakes, and GPS velocities)
```

### Data Storage

**MB85RS2 FRAM** (`driver/main_fram.rs`) — 256 KiB via SPI (GPIO 17 CS, 1 MHz):

| Address | Content | Purpose |
|---------|---------|---------|
| 0x00 | Flight mode (u32) | Recover state after power loss |
| 0x04 | Cycle count (u32) | Track loop iterations |
| 0x08–0x10 | Pressure, temp, altitude (f32 each) | Last sensor snapshot |
| 0x14–0x18 | MAV state, SV state (u32 each) | Actuator positions |
| 0x64 | Altitude log (f32) | Fallback when SD card unavailable |

SD card logging is defined but defaults to disabled (`sd_logging_enabled = false`).

### GPIO Summary

| Pin | Function | Direction |
|-----|----------|-----------|
| GPIO 0 | I2C0 SDA | Bidirectional |
| GPIO 1 | I2C0 SCL | Output |
| GPIO 36 | SSA Drogue | Output |
| GPIO 39 | SSA Main | Output |
| GPIO 4 | UART1 TX (radio) | Output |
| GPIO 5 | UART1 RX (radio) | Input |
| GPIO 21 | Buzzer | Output |
| GPIO 40 | MAV PWM | Output |
| GPIO 47 | Solenoid Valve | Output |
| GPIO 10 | Arming Switch | Input (pull-down) |
| GPIO 24 | Umbilical Sense | Input (pull-down) |
| GPIO 16 | SPI0 MISO (FRAM) | Input |
| GPIO 17 | SPI0 CS (FRAM) | Output |
| GPIO 18 | SPI0 CLK (FRAM) | Output |
| GPIO 19 | SPI0 MOSI (FRAM) | Output |
| GPIO 25 | Status LED | Output |

---

## Architecture Overview

```
main.rs
  └── Initializes hardware (module.rs), spawns USB logger, enters flight loop

flight_loop.rs (FlightLoop)
  ├── execute()          — called each cycle: read sensors → check health → run transitions → transmit
  └── check_transitions() — match on FlightMode, apply transition logic

state.rs (FlightState)
  ├── Owns all sensor instances, actuators, radio, FRAM
  ├── read_sensors()     — polls every sensor, updates Packet
  ├── transmit()         — serializes Packet, sends via RFD900x
  ├── trigger_drogue() / trigger_main()  — fire chutes
  └── log/write/reset FRAM helpers

driver/
  ├── bmp390.rs          — altimeter
  ├── lsm6dsox.rs        — IMU
  ├── lis3mdl.rs         — magnetometer
  ├── ublox_max_m10s.rs  — GPS
  ├── rfd900x.rs         — radio
  └── main_fram.rs       — non-volatile storage

actuator.rs              — SSA, Buzzer, MAV, SV drivers
packet.rs                — 68-byte telemetry struct
constants.rs             — all thresholds, pin assignments, bus config
```

**Sensor abstraction pattern:** Every sensor driver exposes:
```rust
pub async fn new(i2c_bus: &'static SharedI2c) -> Self
pub async fn read_into_packet(&mut self, packet: &mut Packet) -> Result<(), Error>
```

The shared I2C bus is a `Mutex<NoopRawMutex, I2c>` stored in a `static_cell`. Each sensor gets an `I2cDevice` wrapper that borrows from the mutex.

---

## Adding a New Sensor — Step by Step

This walkthrough uses a hypothetical **LIS3DH** accelerometer (I2C address 0x19) as an example.

### Step 1: Create the driver file

Create `fsw/src/driver/lis3dh.rs`:

```rust
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_rp::{i2c::I2c, peripherals::I2C0};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;

use crate::packet::Packet;

const LIS3DH_ADDR: u8 = 0x19;

// Register addresses
const WHO_AM_I: u8 = 0x0F;       // Should return 0x33
const CTRL_REG1: u8 = 0x20;
const OUT_X_L: u8 = 0x28;

type SharedI2c = Mutex<NoopRawMutex, I2c<'static, I2C0, embassy_rp::i2c::Async>>;

pub struct Lis3dh {
    i2c: I2cDevice<'static, NoopRawMutex, I2c<'static, I2C0, embassy_rp::i2c::Async>>,
}

impl Lis3dh {
    pub async fn new(i2c_bus: &'static SharedI2c) -> Self {
        let mut dev = Self {
            i2c: I2cDevice::new(i2c_bus),
        };
        dev.init().await;
        dev
    }

    async fn init(&mut self) {
        // Verify chip ID
        let mut buf = [0u8; 1];
        self.i2c.write_read(LIS3DH_ADDR, &[WHO_AM_I], &mut buf).await.ok();
        if buf[0] != 0x33 {
            log::warn!("LIS3DH: unexpected WHO_AM_I: {:#x}", buf[0]);
        }

        // Configure: 100 Hz, normal mode, XYZ enabled
        self.i2c.write(LIS3DH_ADDR, &[CTRL_REG1, 0x57]).await.ok();
    }

    pub async fn read_into_packet(&mut self, packet: &mut Packet) -> Result<(), embassy_rp::i2c::Error> {
        let mut buf = [0u8; 6];
        // Auto-increment read starting at OUT_X_L (set bit 7)
        self.i2c.write_read(LIS3DH_ADDR, &[OUT_X_L | 0x80], &mut buf).await?;

        let raw_x = i16::from_le_bytes([buf[0], buf[1]]) as f32;
        let raw_y = i16::from_le_bytes([buf[2], buf[3]]) as f32;
        let raw_z = i16::from_le_bytes([buf[4], buf[5]]) as f32;

        // Convert to m/s² (example scale factor for ±2g, 10-bit left-justified)
        let scale = 9.80665 / 16384.0;
        packet.accel_x = raw_x * scale;
        packet.accel_y = raw_y * scale;
        packet.accel_z = raw_z * scale;

        Ok(())
    }
}
```

**Key conventions to follow:**
- Use `I2cDevice::new(i2c_bus)` to share the bus — no extra wiring
- `new()` takes `&'static SharedI2c` and calls `init()` internally
- `read_into_packet()` writes directly into the shared `Packet` struct
- Return `Result` so the caller can handle failures gracefully
- Use `log::warn!` / `log::info!` (via defmt) for diagnostics

### Step 2: Register the module

In `fsw/src/driver/mod.rs`, add:

```rust
pub mod lis3dh;
```

### Step 3: Add fields to `Packet` (if needed)

If the new sensor provides data not already in the packet, edit `packet.rs` to add fields. If it writes to existing fields (like a second accelerometer replacing the LSM6DSOX accel data), skip this step.

```rust
// In packet.rs — add new fields at the end
pub new_field_x: f32,
pub new_field_y: f32,
```

Update the byte size comment and the `as_bytes()` serialization accordingly. The ground station must also be updated to parse the new packet layout.

### Step 4: Add the sensor to `FlightState`

In `state.rs`:

1. Import the driver:
   ```rust
   use crate::driver::lis3dh::Lis3dh;
   ```

2. Add a field to `FlightState`:
   ```rust
   pub struct FlightState {
       // ... existing fields ...
       lis3dh: Lis3dh,
   }
   ```

3. Initialize it in `FlightState::new()`:
   ```rust
   let lis3dh = Lis3dh::new(i2c_bus).await;
   ```

4. Call it in `read_sensors()`:
   ```rust
   if let Err(e) = self.lis3dh.read_into_packet(&mut self.packet).await {
       log::warn!("LIS3DH read failed: {:?}", e);
   }
   ```

### Step 5: Wire into hardware init (if new bus/pins needed)

If the sensor uses the existing I2C0 bus, no changes to `module.rs` are needed — it already gets a reference to the shared bus.

If the sensor needs a **new bus** (e.g., I2C1 or a second SPI), add an init function in `module.rs` following the pattern of `init_shared_i2c()` or `init_spi()`, assign unused GPIO pins, and pass the new bus into `FlightState::new()`.

### Step 6: Add to telemetry and FRAM (if applicable)

- If the sensor data should be persisted to FRAM, add new addresses in the FRAM memory map (after 0x18) and corresponding `write_*` / `read_*` helpers in `state.rs`.
- The radio packet is automatically sent each cycle via `transmit()`, so any data written to `Packet` will be downlinked.

### Step 7: Update constants

Add any sensor-specific thresholds or configuration values to `constants.rs`:

```rust
pub const LIS3DH_I2C_ADDR: u8 = 0x19;
pub const LIS3DH_SCALE_FACTOR: f32 = 9.80665 / 16384.0;
```

### Step 8: Test

- Use the simulation framework in `test/flight_sim.rs` to inject mock data via setter methods
- Flash to the Pico 2 with `cargo run --release` and verify data appears in USB logs and radio telemetry

### Checklist Summary

| Step | File(s) | What to do |
|------|---------|------------|
| 1 | `driver/<sensor>.rs` (new) | Write driver: `new()`, `init()`, `read_into_packet()` |
| 2 | `driver/mod.rs` | Add `pub mod <sensor>;` |
| 3 | `packet.rs` | Add fields if sensor provides new data types |
| 4 | `state.rs` | Add field to `FlightState`, init in `new()`, call in `read_sensors()` |
| 5 | `module.rs` | Only if sensor needs a new bus or pins |
| 6 | `state.rs` / FRAM map | Add FRAM persistence if needed |
| 7 | `constants.rs` | Add addresses, scale factors, thresholds |
| 8 | `test/flight_sim.rs` | Add setter methods and test scenarios |
