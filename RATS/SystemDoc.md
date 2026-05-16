# Rotational Antenna Tracking System (RATS)
## System Design Document v2.0

**Project:** Cornell Rocketry Team Launch Vehicle Tracking and Telemetry System
**Date:** May 2026
**Status:** Active Development

---

## Implementation Status

**COMPLETED (RadioPico):**
- RFD900x UART Reception: Full packet reception at 115200 baud (UART0, GP0/GP1)
- Packet Parsing: 197-byte telemetry packets with sync word detection (0x3E5D5967)
- SD Card Logging: CSV logging to microSD card via SPI1 (GP10-13), batch of 10 packets
- Dual-Core Architecture: Core 0 for real-time I/O, Core 1 for logging/MQTT
- Test Modes: Loopback (GP0→GP1 jumper) and Dual-Radio over-the-air
- MQTT/Wi-Fi: Publishing full packets to MQTT broker as JSON
- Inter-Pico UART: Sending TrackingData (flight_mode/lat/lon/alt) via UART1 (GP4 TX)

**COMPLETED (StepperPico):**
- Dual-Core Architecture: Core 0 for high-priority motor step generation, Core 1 for FSM/math/sensors
- 6-State Tracking FSM: GPS_SEARCH → GPS_AVERAGING → STANDBY → PAD_IDLE → ACTIVE_TRACKING ↔ SIGNAL_LOST
- Inter-Pico RX: `pollRealRadio()` reads TrackingData packets from RadioPico via UART1 (GP5)
- GPS Parsing: NMEAParser on UART0 (GP0/GP1), averages 120 fixes for RATS position lock
- Angle Calculation: LLA → ENU → Azimuth/Elevation pipeline via GeoMath module
- Kalman Filter: Constant-velocity Kalman filter for smooth position prediction between packets
- Calibration Offset: Continuously zeroed against GPS drift during PAD_IDLE state
- Motor Control: AccelStepper library, 200 steps/rev × 8 microsteps = 1600 steps/rev

**IN PROGRESS / TODO:**
- Link Loss Predictive Tracking: Kalman continues predicting in SIGNAL_LOST but motors are disabled

---

## Table of Contents

1. [System Overview](#1-system-overview)
2. [Hardware Architecture](#2-hardware-architecture)
3. [Pin Assignments](#3-pin-assignments)
4. [Communication Protocols](#4-communication-protocols)
5. [Software Architecture](#5-software-architecture)
6. [Data Flow](#6-data-flow)
7. [Packet Structures](#7-packet-structures)
8. [Core Responsibilities](#8-core-responsibilities)
9. [StepperPico State Machine](#9-stepperpico-state-machine)
10. [Critical Timing Requirements](#10-critical-timing-requirements)
11. [Testing Strategy](#11-testing-strategy)
12. [Configuration Reference](#12-configuration-reference)

---

## 1. System Overview

### 1.1 Mission

The RATS system tracks a launch vehicle during flight by continuously aiming a directional Yagi antenna at the rocket using real-time GPS telemetry received via LoRa radio (RFD900x at 900MHz).

### 1.2 Key Requirements

| Requirement | Specification |
|-------------|---------------|
| Packet Reception Rate | 20 Hz |
| Packet Size | 197 bytes |
| Tracking Update Rate | 20 Hz |
| Motor Response Time | <100 ms |
| Data Logging | All packets to SD card |
| Ground Station Link | MQTT over WiFi |
| Link Loss Handling | Kalman prediction (SIGNAL_LOST state) |
| Operating Range | 40+ km |

### 1.3 System Components

**Primary Electronics:**
- 2x Raspberry Pi Pico 2 W microcontrollers (dual-core ARM Cortex-M33)
- 1x RFD900x 900MHz LoRa radio modem (receive-only)
- 1x GPS module (UART, u-blox, soldered to PCB)
- 1x MicroSD card socket (integrated on PCB)

**Mechanical:**
- 2x Stepperonline E Series NEMA 23 stepper motors (3.0 Nm holding torque, 4.2A)
- 2x Stepperonline DM556T stepper drivers (1.8-5.6A, 20-50VDC)
- Farnell YAGI-868/914A directional antenna (900MHz, RP-SMA)
- Turret assembly (azimuth/elevation gimbal)

**Power:**
- HV Input: 44V DC via XT60 connector (external PSU)
- Logic: Derived on-board — 44V → Buck converter → 5V → 3.3V LDO

---

## 2. Hardware Architecture

### 2.1 System Components

**Radio/Data Pico #1 (RadioPico):**
- Receives telemetry from RFD900x via UART0
- Logs data to MicroSD card in CSV format
- Publishes JSON packets to ground station via MQTT/WiFi
- Sends TrackingData (flight_mode/lat/lon/alt) to StepperPico via UART1 at 20 Hz
- Dual-core: Core 0 handles I/O, Core 1 handles SD/MQTT

**Motor Control Pico #2 (StepperPico):**
- Receives TrackingData from RadioPico via UART1 on Core 1
- Runs a 6-state FSM on Core 1: GPS lock, pad idle, active tracking, signal lost
- Calculates pointing angles on Core 1: LLA → ENU → AzEl with Kalman smoothing
- Core 0 runs a tight motor control loop, consuming angle targets from Core 1 via queue
- GPS module (u-blox, soldered) on UART0 provides RATS position

### 2.2 PCB Overview

The RATS carrier board is a unified mixed-signal PCB that integrates high-voltage motor power distribution, logic power regulation, and both microcontroller subsystems on a single board.

**Board Specifications:**
- Dimensions: 160mm x 80mm
- Layers: 2-layer FR4, 1.6mm thickness
- HV Input: +44V via XT60-M connector (J3)
- Logic Power: On-board buck converter (44V → 5V) + LDO (5V → 3.3V)

**Power Architecture (4 sheets):**

*Sheet 1 — 48V Power Entry & Distribution:*
- J3 (XT60-M): Primary +44V input
- J1 (XT60-M): Azimuth motor output, fused (F1: 3A slow-blow)
- J2 (XT60-M): Elevation motor output, fused (F2: 3A slow-blow)
- F3 (8A slow-blow): System-level main fuse after J3
- F4 (1A slow-blow): Isolates buck converter feed from HV bus
- Bus decoupling: C6 (680µF electrolytic) + C3 (1µF) + C27 (100nF)
- D1 (green LED): HV bus presence indicator

*Sheet 2 — Power Conversion & Ground Topology:*
- D2 (TVS SMBJ45A): Clamps HV transients at buck input
- B1 (LMR38025S5QDRRRQ1): Automotive-grade fixed 5V buck regulator with L1 (15µH)
- +5V SYS: Filtered logic rail (C17 47µF + C24 100nF referenced to SGND)
- U1 (MCP1826): 3.3V LDO for GPS module and SD card
- R6 (0Ω net tie): Single tie point between PGND (power ground) and SGND (signal ground); centrally placed to force return current separation
- Dual-layer ground pours: PGND under power components, SGND under logic; 20 mil clearance gap between domains

*Sheet 3 — StepperPico & GPS Interface:*
- B2 (RP2350 / Pico 2 W): Motor control microcontroller
- SN74AHCT125 buffers: Level-shift 3.3V GPIO → 5V for stepper driver inputs
- Common anode topology: +5V tied to PUL+/DIR+/EN+; Pico modulates the negative lines (active low); 10kΩ pull-ups ensure safe (inactive) state during Pico reset
- 100Ω series resistors on all GPIO lines for protection
- GPS: SparkFun MAX-M10S (5-pin UART header, 3.3V, local decoupling C34 1µF + C5 100nF)
- J4 (Azimuth) & J5 (Elevation): 4-pin screw terminals carrying +5V, STEP, DIR, EN to external DM556T drivers
- D7 (Schottky): VBUS isolation for USB serial monitor without back-feeding board 5V

*Sheet 4 — RadioPico & Telemetry Interface:*
- B3 (RP2350 / Pico 2 W): Radio/telemetry microcontroller
- SD card: SPI interface, 47kΩ pull-ups on data lines, DS2 LED for card detect
- J6A/B (SFH11 dual 8-pin headers): RFD900x radio modem interface; C32 (10µF) + C33 (100nF) local decoupling; R22 (0Ω) jumper allows radio power rail isolation for current measurement
- D8 (Schottky): VBUS isolation

---

## 3. Pin Assignments

### 3.1 Raspberry Pi Pico 2 W #1 (RadioPico)

**Primary Functions:**
- RFD900x UART reception (UART0)
- MicroSD card logging (SPI1)
- WiFi/MQTT ground station link
- Inter-Pico UART transmission (UART1)

| GPIO | Function | Connection | Notes |
|------|----------|------------|-------|
| GP0 | UART0 TX | RFD900x RX (Pin 7) | TX only used in loopback/dual-radio test modes |
| GP1 | UART0 RX | RFD900x TX (Pin 9) | **Telemetry from radio** |
| GP4 | UART1 TX | StepperPico GP5 (RX) | **TrackingData to StepperPico** |
| GP10 | SPI1 SCK | MicroSD CLK | SD card clock |
| GP11 | SPI1 MOSI | MicroSD CMD | SD card data input |
| GP12 | SPI1 MISO | MicroSD D0 | SD card data output |
| GP13 | SPI1 CS | MicroSD CS | Chip select |
| GP22 | SD Card Detect | MicroSD CD | Presence detect |
| GP26 | External LED | Status indicator | Toggles on each received packet |

**UART0 (RFD900x Reception):**
- Baud: 115200 bps, 8N1
- Direction: RX only in normal operation (GP1 receives from RFD900x Pin 9)
- Buffer: 512 bytes circular, interrupt-driven
- Sync Word: 0x3E5D5967 ("CRT!")
- Packet: 197 bytes @ 20 Hz

**UART1 (Inter-Pico Transmission):**
- Baud: 115200 bps, 8N1
- Direction: TX only (GP4 transmits to StepperPico GP5)
- Sync Word: 0x54524B21 ("TRK!")
- Packet: 20 bytes (4-byte sync + 16-byte TrackingData)
- Rate: Every received radio packet (~20 Hz)

**SPI1 (MicroSD Card):**
- Clock: 12.5 MHz
- Format: FAT32, CSV
- Batch: 10 packets per write

---

### 3.2 Raspberry Pi Pico 2 W #2 (StepperPico)

**Primary Functions:**
- Core 0: High-priority motor step pulse generation
- Core 1: FSM, sensors, angle math
- GPS reception (UART0, soldered u-blox module)
- Inter-Pico UART reception (UART1)
- Two stepper motor axes (azimuth + elevation)

| GPIO | Function | Connection | Notes |
|------|----------|------------|-------|
| GP0 | UART0 TX | GPS RX | GPS commands |
| GP1 | UART0 RX | GPS TX | NMEA input from u-blox |
| GP5 | UART1 RX | RadioPico GP4 (TX) | **TrackingData from RadioPico** |
| GP6 | STEP | Azimuth Driver PUL- | Step pulses |
| GP7 | DIR | Azimuth Driver DIR- | Direction |
| GP8 | ENA | Azimuth Driver ENA- | Enable (active low) |
| GP9 | STEP | Elevation Driver PUL- | Step pulses |
| GP10 | DIR | Elevation Driver DIR- | Direction |
| GP11 | ENA | Elevation Driver ENA- | Enable (active low) |
| GP28 | Status LED | Internal | FSM state indicator |

**Motor Configuration (AccelStepper):**
- `azMotor(DIR=GP7, STEP=GP6, EN=GP8, stepsPerRev=200, microsteps=8)` — 1600 steps/rev
- `elMotor(DIR=GP10, STEP=GP9, EN=GP11, stepsPerRev=200, microsteps=8)` — 1600 steps/rev
- Physical mounting: both axes negated in software (`moveAngleTo(-target)`)
- Default max speed: 8000 steps/s
- Default acceleration: 4000 steps/s²
- Motors are disabled except in ACTIVE_TRACKING state

**UART0 (GPS):**
- Baud: 9600 bps, NMEA 0183 (GGA, RMC minimum)
- Module: u-blox, soldered to PCB
- Averaging: 120 valid fixes used to determine RATS position

**UART1 (Inter-Pico Reception):**
- Baud: 115200 bps, 8N1
- Direction: RX on GP5 (from RadioPico GP4)
- Sync Word: 0x54524B21 ("TRK!")
- Packet: 20 bytes (4-byte sync + 16-byte TrackingData)
- Rate: ~20 Hz
- Parsed by `pollRealRadio()` in Core 1

---

### 3.3 RFD900x Radio Modem Connections

| Pin | Signal | PCB Connection | Notes |
|-----|--------|----------------|-------|
| 1-2 | GND | Common Ground | Heavy ground |
| 4 | Vcc | 5V Rail | 60mA typical |
| 9 | TX | RadioPico GP1 (RX) | UART data |
| 16 | GND | Common Ground | Redundant |

**Antenna Connection:**
- ANT1: RP-SMA connector to Farnell YAGI-868/914A antenna
- ANT2: Not used

**RFD900x Configuration:**
- Air Data Rate: 64 kbps (or 128 kbps)
- TX Power: 20 dBm (100mW)
- Serial Speed: 115200 bps
- Network ID: 217 (must match rocket)
- Mode: Receive only

---

### 3.4 Stepper Driver Connections (Stepperonline DM556T)

**Azimuth Driver (DM556T #1):**

| Pico Pin | Signal | Driver Pin | Notes |
|----------|--------|------------|-------|
| GP6 | STEP | PUL- | Step pulse |
| GP7 | DIR | DIR- | Direction |
| GP8 | ENA | ENA- | Enable (active low) |
| GND | GND | Signal GND | Common ground |

**Elevation Driver (DM556T #2):**

| Pico Pin | Signal | Driver Pin | Notes |
|----------|--------|------------|-------|
| GP9 | STEP | PUL- | Step pulse |
| GP10 | DIR | DIR- | Direction |
| GP11 | ENA | ENA- | Enable (active low) |
| GND | GND | Signal GND | Common ground |

**Motor Power (Both Drivers):**
- VMOT: 44VDC from XT60 outputs (J1 azimuth, J2 elevation) on carrier board
- Motor: Stepperonline E Series NEMA 23 (4-wire, bipolar)
- Current: 4.2A per phase

**DM556T Signal Interface:**
- Signals arrive at 5V from the SN74AHCT125 level-shift buffers on the carrier board
- Common anode: PUL+/DIR+/EN+ tied to +5V; Pico drives PUL−/DIR−/EN− low to activate
- 100Ω series resistors on signal lines; 10kΩ pull-ups hold signals inactive during Pico reset
- Pulse width: 2.5 µs minimum
- Microstepping: 8 microsteps (set via DIP switches SW5-SW8)
- Current: Set via DIP switches (SW1-SW3)

---

### 3.5 GPS Module Connections

| Pin | Signal | Connection | Notes |
|-----|--------|------------|-------|
| VCC | Power | 3.3V or 5V | Per module datasheet |
| GND | Ground | Common GND | |
| TX | UART TX | StepperPico GP1 (RX) | NMEA sentences out |
| RX | UART RX | StepperPico GP0 (TX) | Optional command input |

**GPS Configuration:**
- Module: SparkFun MAX-M10S (u-blox MAX-M10S chipset)
- Protocol: NMEA 0183
- Baud Rate: 9600 bps
- Power: 3.3V from on-board MCP1826 LDO
- Position lock: 120-point average collected in GPS_AVERAGING state

---

### 3.6 MicroSD Card Socket

| Pin | Function | Connection | Notes |
|-----|----------|------------|-------|
| CS | Chip Select | RadioPico GP13 | Active low |
| MOSI | Data In | RadioPico GP11 | |
| CLK | Clock | RadioPico GP10 | |
| MISO | Data Out | RadioPico GP12 | |
| VDD | Power | 3.3V | 50mA max |
| CD | Card Detect | RadioPico GP22 | |

**Configuration:**
- Clock: 10 MHz (init), 25 MHz (operation)
- File System: FAT32

---

## 4. Communication Protocols

### 4.1 RFD900x to RadioPico (UART0)

**Physical Layer:**
- Baud Rate: 115200 bps, 8N1, no flow control
- Direction: RFD900x TX → RadioPico GP1 RX (receive only in normal operation)
- Buffer: 512-byte circular, interrupt-driven

**Packet Detection:**
- Search for sync word 0x3E5D5967 ("CRT!") in byte stream
- Direct `memcpy` of 197 bytes into RadioPacket (both sides little-endian, struct is `#pragma pack(1)`)

---

### 4.2 RadioPico to StepperPico (UART1, Inter-Pico)

**Physical Layer:**
- Baud Rate: 115200 bps, 8N1
- Direction: RadioPico GP4 TX → StepperPico GP5 RX (unidirectional)
- Rate: ~20 Hz (one packet per valid received radio packet)

**Packet Format (20 bytes total):**

| Bytes | Field | Type | Description |
|-------|-------|------|-------------|
| 0–3 | Sync Word | uint32_t | 0x54524B21 ("TRK!") |
| 4–7 | flight_mode | uint32_t | Rocket's current flight state |
| 8–11 | latitude_udeg | int32_t | Latitude in microdegrees (÷1e6 = degrees) |
| 12–15 | longitude_udeg | int32_t | Longitude in microdegrees |
| 16–19 | altitude | float | Altitude in meters above sea level |

**TrackingData structure** (`Common/serial_protocol.h`):
```cpp
struct TrackingData {
    uint32_t flight_mode;    // 4 bytes
    int32_t latitude_udeg;   // 4 bytes
    int32_t longitude_udeg;  // 4 bytes
    float altitude;          // 4 bytes
} __attribute__((packed));
```

Note: RadioPacket stores `latitude`/`longitude` as `float` in degrees. RadioPico converts them to microdegrees via `degrees_to_udeg()` before packing into TrackingData.

---

## 5. Software Architecture

### 5.1 RadioPico

Dual-core RP2040. Core 0 owns all time-critical I/O; Core 1 owns all slow I/O (SD, MQTT). Packets passed between cores via a 64-element lock-free queue (`pico/util/queue.h`).

**Source Files:**

| File | Role |
|------|------|
| `RadioPico.cpp` | Main entry point, dual-core orchestration |
| `rfd900x_uart.h/.cpp` | RFD900x UART0 driver (interrupt-driven RX) |
| `inter_pico_uart.h/.cpp` | UART1 TX driver (TrackingData packets) |
| `mqtt_client.h/.cpp` | WiFi connection and MQTT publish (async state machine) |
| `sd_logger.h/.cpp` | MicroSD SPI1 driver, CSV batch logging |
| `packet_parser.h/.cpp` | Binary → RadioPacket (`memcpy`), RadioPacket → JSON |
| `packet_simulator.h` | Simulated rocket trajectory (test modes only) |
| `Common/packet_types.h` | RadioPacket definition (packed, 197 bytes) |
| `Common/serial_protocol.h` | TrackingData struct, degree/udeg conversion helpers |
| `Common/config.h` | Shared constants |

### 5.2 StepperPico

Dual-core RP2040. Core 0 runs the tight motor-step loop; Core 1 runs the FSM, sensor polling, and math. Angle targets are passed from Core 1 to Core 0 via a 10-element `angle_queue`.

**Source Files:**

| File | Role |
|------|------|
| `main.cpp` | Entry point, Core 0 motor loop, Core 1 FSM/math |
| `StepperMotor.h/.cpp` | AccelStepper wrapper (`moveAngleTo`, `update`, `enable`, `disable`) |
| `GeoMath.h/.cpp` | LLA→ENU (`llatoENU`) and ENU→AzEl (`enuToAzEl`) |
| `KalmanCV.h/.cpp` | Constant-velocity Kalman filter |
| `NMEAParser.h/.cpp` | u-blox GPS NMEA parser (UART0) |
| `AzEl.h` | Azimuth/Elevation struct |
| `LLA.h` | Latitude/Longitude/Altitude struct |
| `packet_simulator.h/.cpp` | Synthetic rocket trajectory (simulator mode only) |
| `Common/packet_types.h` | RadioPacket definition (used by simulator) |
| `Common/serial_protocol.h` | TrackingData struct, conversion helpers |
| `AccelStepper-1.64.0/` | External stepper library with acceleration profiles |

---

## 6. Data Flow

```
Rocket → RFD900x (900MHz RF)
              ↓ UART0 @ 115200 baud
         RadioPico Core 0
         - Interrupt-driven 512-byte RX buffer
         - Detect sync word 0x3E5D5967
         - memcpy 197-byte RadioPacket
         - Toggle GP26 LED
         - Convert lat/lon float→µdeg, extract flight_mode/alt
              ↓ UART1 @ 115200 baud (20-byte TrackingData @ 20 Hz)
         StepperPico Core 1                    RadioPico Core 1
         [pollRealRadio()]                     - JSON conversion
         - Parse 20-byte TrackingData packet   - MQTT publish (rats/raw/1)
         - Update rocket LLA + flight_mode     - SD card CSV batch write
         [FSM + Math]
         - GPS_SEARCH → GPS_AVERAGING
           → STANDBY → PAD_IDLE
           → ACTIVE_TRACKING ↔ SIGNAL_LOST
         - LLA → ENU (GeoMath)
         - Kalman predict + update
         - ENU → AzEl (GeoMath)
         - Calibration offset subtraction
         - Push TargetAngles to angle_queue
              ↓ angle_queue (10-element, lock-free)
         StepperPico Core 0
         - Drain queue: moveAngleTo(-az), moveAngleTo(-el)
         - Enable/disable motors per motors_enabled flag
         - AccelStepper.update() (tight loop)
              ↓
         Stepper drivers → Motors (Azimuth + Elevation)
```

---

## 7. Packet Structures

### 7.1 Rocket Telemetry Packet (RadioPacket, 197 bytes)

Defined in `Common/packet_types.h`. Uses `#pragma pack(push, 1)` — exactly mirrors Rust FSW serialization (little-endian, no padding). The 193-byte payload corresponds to `Packet::SIZE` in `fsw/src/packet.rs`; RATS prepends the 4-byte sync word.

| Bytes | Field | Type | Unit | Description |
|-------|-------|------|------|-------------|
| 0–3 | sync_word | uint32_t | — | 0x3E5D5967 ("CRT!") |
| 4–7 | flight_mode | uint32_t | — | 0=STARTUP, 1=STANDBY, 2=ASCENT, 3=DROGUE, 4=MAIN, 5=FAULT |
| 8–11 | pressure | float | — | Altimeter pressure |
| 12–15 | temp | float | °C | Altimeter temperature |
| 16–19 | altitude | float | m | Barometric altitude |
| 20–23 | latitude | float | deg | GPS latitude (decimal degrees) |
| 24–27 | longitude | float | deg | GPS longitude (decimal degrees) |
| 28–31 | num_satellites | uint32_t | — | GPS satellite count |
| 32–35 | timestamp | float | — | GPS timestamp |
| 36–47 | mag_{x,y,z} | float×3 | — | Magnetometer XYZ |
| 48–59 | accel_{x,y,z} | float×3 | m/s² | IMU accelerometer XYZ |
| 60–71 | gyro_{x,y,z} | float×3 | °/s | IMU gyroscope XYZ |
| 72–83 | pt3, pt4, rtd | float×3 | — | ADC channels |
| 84 | sv_open | bool | — | Solenoid valve state |
| 85 | mav_open | bool | — | Main actuator valve state |
| 86–87 | ssa_drogue_deployed, ssa_main_deployed | uint8_t×2 | — | Deployment flags |
| 88–94 | cmd_n1…cmd_a3 | uint8_t×7 | — | Command event flags |
| 95–98 | airbrake_deployment | float | 0–1 | Airbrake PWM duty fraction |
| 99–102 | predicted_apogee | float | m | Airbrake controller output |
| 103–110 | h_acc, v_acc | uint32_t×2 | mm | GPS horizontal/vertical accuracy |
| 111–142 | vel_n, vel_e, vel_d, g_speed | double×4 | m/s | GPS velocity components and ground speed |
| 143–150 | s_acc, head_acc | uint32_t×2 | mm/s, deg×1e5 | GPS speed/heading accuracy |
| 151 | fix_type | uint8_t | — | 0=none, 2=2D, 3=3D, 4=3D+DGPS |
| 152–155 | head_mot | int32_t | deg×1e5 | GPS heading of motion |
| 156–172 | blims_brakeline_diff, blims_phase_id, blims_pid_p, blims_pid_i, blims_bearing | float/int8_t/float×3 | — | BLiMS outputs |
| 173–192 | blims_upwind_lat/lon, blims_downwind_lat/lon, blims_wind_from_deg | float×5 | deg | BLiMS config |
| 193–196 | ms_since_boot_cfc | uint32_t | ms | CFC uptime since last boot |

---

### 7.2 Inter-Pico Tracking Packet (20 bytes)

See [Section 4.2](#42-radiopico-to-stepperpico-uart1-inter-pico) for the full definition.

---

## 8. Core Responsibilities

### 8.1 RadioPico — Core 0

**Main Loop (tight, <1 ms per iteration):**
1. In test modes: every 50 ms → generate and transmit simulated RadioPacket on UART0
2. Poll `RFD900xUART::packetAvailable()` (interrupt-driven, non-blocking)
3. Read `sizeof(RadioPacket)` bytes from RX buffer
4. Validate sync word (0x3E5D5967)
5. `memcpy` into RadioPacket struct
6. Toggle GP26 LED
7. Convert lat/lon to µdeg, send TrackingData to StepperPico via `InterPicoUART::sendTrackingData()`
8. Push packet to Core 1 queue (non-blocking, drops if queue full)
9. Print RX stats every 5 seconds
10. `tight_loop_contents()`

**Constraints:** No WiFi, no SD writes, no blocking calls.

---

### 8.2 RadioPico — Core 1

**Main Loop (1–100 ms per iteration):**
1. `MqttClient::poll()` — drives async WiFi/MQTT state machine
2. `queue_try_remove()` — get packet from Core 0
3. `PacketParser::radioPacketToJSON()` — format JSON string
4. Batch 10 packets; `SDLogger::logPacketBatch()` when full
5. `MqttClient::publish()` — send JSON immediately
6. Print SD stats every 30 seconds
7. `sleep_ms(1)`

---

### 8.3 StepperPico — Core 0 (Motor Control)

**Main Loop (tight, runs continuously):**
1. Drain `angle_queue`: call `azMotor.moveAngleTo(-targets.azimuth)` and `elMotor.moveAngleTo(-constrain(targets.elevation, -90, 90))` — signs negated for physical motor mounting
2. If `motors_enabled` changed: call `azMotor.enable()` / `azMotor.disable()` and same for elevation
3. If motors are enabled: call `azMotor.update()` and `elMotor.update()` — AccelStepper generates step pulses

Motors are enabled only when Core 1 sets `targets.motors_enabled = true` (only in ACTIVE_TRACKING state).

---

### 8.4 StepperPico — Core 1 (FSM, Math, Sensors)

**Main Loop (sleep_ms(1) at end of each iteration):**

1. **GPS polling** (`NMEAParser::process()`): collect fixes; accumulate 120 for RATS position average
2. **`pollRealRadio()`**: read 20-byte TrackingData from UART1, update `rocketLLA` and `currentFlightMode`
3. **FSM transitions** (see Section 9)
4. **Math** (runs in PAD_IDLE, ACTIVE_TRACKING, SIGNAL_LOST states when both fixes available):
   - On new packet: `GeoMath::llatoENU(ratsLLA, rocketLLA)` → `kalmanRocket.predict(t)` → `kalmanRocket.updatePosition(enu, 25.0)`
   - Every iteration: `kalmanRocket.predictFuture(dt + 0.05s)` → `GeoMath::enuToAzEl(filteredPos)`
   - Apogee clamp: if `ratsLLA.alt + futureState.d[2] > 3048.0 m`, clamp to 3048 m
   - Apply calibration offset: `targets.azimuth = azel.azimuth - calibration_az_offset`
5. **Push to queue**: evict oldest entry if full, add new `TargetAngles`
6. **Debug print** every 500 ms: FSM state, target angles, `motors_enabled`, packet rate

**Compile flags:**
- `USE_PACKET_SIMULATOR 0` (default/deployment): `pollRealRadio()` is the data source
- `USE_PACKET_SIMULATOR 1` (testing): `pollPacketSimulator()` generates synthetic data instead
- `MATH_TEST_MODE` (optional): lower motor speeds, skips GPS, RATS LLA = (0,0,0), starts in ACTIVE_TRACKING

---

## 9. StepperPico State Machine

The FSM runs on Core 1. It drives LED blink patterns on GP28 and controls whether motors are enabled.

```
  Power on
     │
     ▼
  ┌─────────────┐
  │  GPS_SEARCH │  LED: 100ms blink    motors: OFF
  └──────┬──────┘
         │ GPS fix received
         ▼
  ┌──────────────────┐
  │  GPS_AVERAGING   │  LED: 500ms blink    motors: OFF
  │  (120 samples)   │
  └──────┬───────────┘
         │ 120 samples averaged
         ▼
  ┌──────────────┐
  │   STANDBY    │  LED: solid ON    motors: OFF
  └──┬────────┬──┘
     │        └──── signal + mode=ASCENT (mid-flight reboot) ───────────────────────┐
     │ signal + mode=STARTUP or STANDBY                                             │
     ▼                                                                              │
  ┌─────────────┐                                                                   │
  │  PAD_IDLE   │  LED: 1s blink    motors: OFF    calibr: continuously updating    │
  └──┬────────┬─┘                                                                   │
     │        └──── mode=ASCENT ─────────────────────────────────────────────────▶─┤
     │ signal lost (>5s)                                                            │
     ▼                                                                              ▼
  STANDBY (restart)                                                   ┌──────────────────────┐
                                                                      │   ACTIVE_TRACKING    │  LED: toggle/pkt    motors: ON
                                                                      └──────────┬───────────┘
                                                                                 │ signal lost >5s
                                                                                 ▼
                                                                      ┌──────────────────────┐
                                                                      │     SIGNAL_LOST      │  LED: solid OFF    motors: OFF
                                                                      └──────────┬───────────┘
                                                                                 │ signal restored
                                                                                 └──▶ ACTIVE_TRACKING
```

### State Details

| State           | LED Pattern       | Motors  | Entry Condition          | Exit Condition        |
|-----------------|-------------------|:-------:|--------------------------|-----------------------|
| GPS_SEARCH      | 100ms blink       |   OFF   | Power on                 | GPS fix received      |
| GPS_AVERAGING   | 500ms blink       |   OFF   | GPS fix                  | 120 samples averaged  |
| STANDBY         | Solid ON          |   OFF   | Average complete         | Signal received       |
| PAD_IDLE        | 1s blink          |   OFF   | Signal + STARTUP/STANDBY | ASCENT or signal lost |
| ACTIVE_TRACKING | Toggle per packet |  **ON** | Signal + ASCENT          | Signal lost >5s       |
| SIGNAL_LOST     | Solid OFF         |   OFF   | No packet for >5s        | Signal restored       |

### Calibration Offset (PAD_IDLE)

While in PAD_IDLE, the computed AzEl is continuously used to update `calibration_az_offset` and `calibration_el_offset`. When tracking begins, all angle commands are offset by these values so that the physical 0° position corresponds to the launch pad direction, canceling GPS position error and antenna mounting offset.

If the system boots directly into ACTIVE_TRACKING (e.g. reboot mid-flight with no PAD_IDLE), the calibration is set from the very first computed angle.

### Signal Lost Timeout

Signal is considered lost if no TrackingData packet is received for more than **5000 ms**. The Kalman filter continues predicting position in SIGNAL_LOST but motors are disabled until the signal is restored.

---

## 10. Critical Timing Requirements

### 10.1 Packet Reception

| Stage                                   |  Target  | Maximum | Notes                      |
|-----------------------------------------|---------:|--------:|----------------------------|
| UART RX interrupt                       |  <10 µs  |  50 µs  | Copy to buffer             |
| Sync word detection                     | <100 µs  | 500 µs  | Search buffer              |
| Packet parse (memcpy)                   |  <50 µs  | 200 µs  | Packed struct, direct copy |
| UART transfer to StepperPico (20 bytes) |  ~1.7 ms |    5 ms | @ 115200 bps               |
| Total latency (radio → motor command)   |    <5 ms |   10 ms | End-to-end                 |

### 10.2 Motor Control

| Parameter            | Value                  | Notes                                    |
|----------------------|------------------------|------------------------------------------|
| Step pulse width     | 10 µs                  | DM556T min: 2.5 µs                       |
| Max speed            | 8000 steps/s           | Configured in setup()                    |
| Acceleration         | 4000 steps/s²          | Configured in setup()                    |
| Steps per revolution | 1600                   | 200 full steps × 8 microsteps            |
| Motor update rate    | Every Core 0 iteration | AccelStepper generates pulses            |
| Kalman lookahead     | +50 ms                 | Compensates for inter-pico latency       |
| Apogee clamp         | 3048 m (10,000 ft)     | Hard limit on Kalman predicted altitude  |

### 10.3 Network/Storage (Non-Critical)

| Operation              |  Target |  Maximum |
|------------------------|--------:|---------:|
| MQTT publish           |  <20 ms |   100 ms |
| SD write (batch of 10) |  <50 ms |   200 ms |
| JSON formatting        |   <5 ms |    20 ms |
| WiFi reconnect         |     N/A |     30 s |

---

## 11. Testing Strategy

### 11.1 RadioPico Test Modes

Three modes selected at compile time via defines in `RadioPico.cpp`:

| Mode | Define | Description |
|------|--------|-------------|
| Normal (deployment) | both defines = 0 | Real rocket telemetry via RFD900x |
| Loopback | `LOOPBACK_TEST_MODE 1` | GP0→GP1 jumper; simulates full pipeline without radio or WiFi |
| Dual-Radio | `DUAL_RADIO_TEST_MODE 1` | Two RFD900x radios; over-the-air test with simulated packets |

### 11.2 StepperPico Test Modes

| Mode | How to enable | Effect |
|------|---------------|--------|
| Normal (deployment) | `USE_PACKET_SIMULATOR 0`, `USE_GPS true` | Real radio data, real GPS |
| Simulator | `USE_PACKET_SIMULATOR 1` | Synthetic rocket trajectory instead of real radio |
| Math test | `MATH_TEST_MODE` compile flag | Lower motor speeds, RATS at (0,0,0), starts in ACTIVE_TRACKING |

### 11.3 Hardware Tests

**Packet Reception Rate:**
- Target: 20 Hz sustained
- Pass: >95% reception

**UART Latency (RadioPico → StepperPico):**
- Target: <5 ms
- Pass: <10 ms worst case

**Motor Accuracy:**
- Target: ±0.5° error
- Pass: ±1.0° error

**End-to-End Latency:**
- Target: <5 ms
- Pass: <10 ms

### 11.4 System Tests

**Continuous Operation:**
- Duration: 4 hours @ 20 Hz
- Pass: <1% packet loss, 0 crashes

**Link Loss Recovery:**
- Simulate 10s blackout
- Pass: SIGNAL_LOST state entered, motors disabled; seamless restoration when signal returns

**GPS Averaging Sequence:**
- Power on, wait for GPS lock
- Pass: GPS_SEARCH → GPS_AVERAGING → STANDBY transition within reasonable time
- Verify RATS position is within 10m of known coordinates

---

## 12. Configuration Reference

### 12.1 Shared Constants (`Common/config.h`)

```cpp
// WiFi / MQTT
#define WIFI_SSID           "CornellRocketry-2.4G"
#define WIFI_PASS           "Rocketry2526"
#define RATS_UNIT_ID        1
#define MQTT_BROKER_ADDRESS "192.168.1.2"
#define MQTT_BROKER_PORT    1883
#define MQTT_TOPIC          "rats/raw/1"

// Sync words
#define SYNC_WORD           0x3E5D5967   // "CRT!" — radio packet
// Inter-Pico sync: 0x54524B21 ("TRK!") — in serial_protocol.h

// Telemetry
#define EXPECTED_PACKET_RATE_HZ  20
#define PACKET_INTERVAL_MS       50
#define LINK_LOST_TIMEOUT_MS     500     // RadioPico side (note: StepperPico uses 5000ms)

// Packet sizes
#define RADIO_PACKET_SIZE    197         // 4 sync + 193 payload
#define TRACKING_DATA_SIZE   16          // TrackingData struct (no sync)

// SD logging
#define SD_LOG_BATCH_SIZE    10

// Ground station defaults
#define GROUND_STATION_LAT_DEG  42.356000
#define GROUND_STATION_LON_DEG  -76.497000
#define GROUND_STATION_ALT_M    100.0

// UART
#define RFD900X_BAUD_RATE    115200
#define INTER_PICO_BAUD_RATE 115200
```

### 12.2 StepperPico Key Constants (`StepperPico/main.cpp`)

```cpp
// Motor objects: StepperMotor(DIR, STEP, EN, stepsPerRev, microsteps)
StepperMotor azMotor(7, 6, 8, 200, 8);   // 1600 steps/rev
StepperMotor elMotor(10, 9, 11, 200, 8);

// Motor speeds
azMotor.setMaxSpeed(8000);
azMotor.setAcceleration(4000);
elMotor.setMaxSpeed(8000);
elMotor.setAcceleration(4000);

// Compile flags
#define USE_PACKET_SIMULATOR 0    // 0 = real radio (deployment), 1 = simulator
static constexpr bool USE_GPS = true;  // false = skip GPS (testing only)

// Operational constants
static constexpr double MAX_APOGEE_METERS = 3048.0;  // 10,000 ft hard clamp
static constexpr unsigned long PACKET_PERIOD_MS = 50;  // 20 Hz
// Signal lost if no packet for 5000ms (hard-coded in FSM)
```

### 12.3 Pin Summary

**RadioPico #1:**
- GP0–1: RFD900x UART0 (TX for test modes, RX for telemetry)
- GP4: Inter-Pico UART1 TX → StepperPico GP5
- GP10–13: MicroSD SPI1
- GP22: SD card detect
- GP26: External LED (toggles on each received packet)

**StepperPico #2:**
- GP0–1: GPS UART0 (TX, RX)
- GP5: Inter-Pico UART1 RX ← RadioPico GP4
- GP6–8: Azimuth motor (STEP, DIR, ENA)
- GP9–11: Elevation motor (STEP, DIR, ENA)
- GP28: Status LED (FSM state indicator)

### 12.4 Calibration

**Startup Sequence:**
1. Power on — StepperPico enters GPS_SEARCH
2. Wait for GPS lock — enters GPS_AVERAGING (120 fixes)
3. After averaging — enters STANDBY, RATS position locked
4. RadioPico starts sending packets — enters PAD_IDLE
5. In PAD_IDLE: antenna continuously calibrates against pad position
6. On ASCENT flight mode — enters ACTIVE_TRACKING, motors engage

**Steps-Per-Degree:**
- 1600 steps/rev ÷ 360° = ~4.44 steps/degree
- Verify with laser pointer: command known angle, measure actual rotation

**GPS Accuracy:**
- Position is averaged over 120 NMEA fixes
- Resulting RATS position should be within 2–5m of actual location

### 12.5 Troubleshooting

**RadioPico not receiving packets:**
- Check RFD900x power LED and antenna connection (ANT1 RP-SMA)
- Check UART wiring (TX↔RX)
- Enable `LOOPBACK_TEST_MODE` to verify RX/SD/inter-pico without a radio

**StepperPico stuck in GPS_SEARCH:**
- Check GPS module power and NMEA wiring (GP0/GP1)
- Verify GPS has sky view for satellite lock
- Temporarily set `USE_GPS = false` to skip to STANDBY for bench testing

**Motors not moving:**
- Verify FSM reached ACTIVE_TRACKING (check serial output for `[FSM: TRACKING]`)
- Confirm GP8/GP11 enable lines are LOW (active low)
- Verify 48V supply connected to drivers

**Tracking inaccurate:**
- Verify RATS GPS averaged to correct position (check STANDBY serial output)
- Confirm calibration offset was set during PAD_IDLE
- Test `GeoMath::enuToAzEl()` with known target position

**WiFi won't connect:**
- Verify SSID/password in `config.h`
- Pico W supports 2.4 GHz only
- Check firewall for port 1883

**SD card errors:**
- Reformat as FAT32, max 32 GB recommended
- Check SPI1 wiring (GP10–13)

---
