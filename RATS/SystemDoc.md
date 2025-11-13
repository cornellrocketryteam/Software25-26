# Rotational Antenna Tracking System (RATS)
## System Design Document v1.0

**Project:** Cornell Rocket Team Launch Vehicle Tracking System  
**Date:** January 2025  
**Status:** Hardware Design Phase

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
9. [Critical Timing Requirements](#9-critical-timing-requirements)
10. [Implementation Roadmap](#10-implementation-roadmap)
11. [Testing Strategy](#11-testing-strategy)
12. [Configuration Reference](#12-configuration-reference)

---

## 1. System Overview

### 1.1 Mission

The RATS system tracks a launch vehicle during flight by continuously aiming a directional Yagi antenna at the rocket using real-time GPS telemetry received via LoRa radio (RFD900x at 900MHz).

### 1.2 Key Requirements

| Requirement | Specification |
|-------------|---------------|
| Packet Reception Rate | 10 Hz maximum |
| Packet Size | 107 bytes |
| Tracking Update Rate | 10 Hz |
| Motor Response Time | <100 ms |
| Data Logging | All packets to SD card |
| Ground Station Link | MQTT over WiFi |
| Link Loss Handling | Predictive tracking |
| Operating Range | 40+ km |

### 1.3 System Components

**Primary Electronics:**
- 2x Raspberry Pi Pico 2 W microcontrollers (dual-core ARM Cortex-M33)
- 1x RFD900x 900MHz LoRa radio modem (receive-only)
- 1x GPS module (UART, turret position reference)
- 1x MicroSD card socket (integrated on PCB)

**Mechanical:**
- 2x NEMA 23 stepper motors (3.0 Nm holding torque, 4.2A)
- 2x DM556T stepper drivers (1.8-5.6A, 20-50VDC, external power)
- Directional Yagi antenna (900MHz, RP-SMA connections)
- Turret assembly (azimuth/elevation gimbal)

**Power:**
- PCB: 5V @ 1A (USB-C input)
- Motors: 36VDC @ 10A (external supply, not on PCB)

---

## 2. Hardware Architecture

### 2.1 System Components

**Radio/Data Pico #1:**
- Receives telemetry from RFD900x via UART
- Logs data to MicroSD card
- Publishes to ground station via MQTT/WiFi
- Sends tracking commands to Motor Pico via UART
- Implements trajectory prediction during link loss

**Motor Control Pico #2:**
- Receives tracking commands from Radio Pico via UART
- Controls stepper motors using PIO
- Reads GPS for turret position
- Calculates pointing angles (azimuth/elevation)
- Implements safety limits

### 2.2 PCB Overview

**Board Specifications:**
- Dimensions: 140mm x 110mm (5.5" x 4.3")
- Layers: 2-layer FR4, 1.6mm thickness
- Power Input: USB-C, 5V @ 1A
- Components: 2x Pico 2 W sockets, RFD900x socket, integrated MicroSD

**Key Features:**
- Integrated 5V power distribution
- 3.3V LDO for pull-ups and peripherals
- 50Ω RF traces for antenna connections
- Via fence around RF section for noise isolation
- Screw terminals for stepper driver connections
- Status LEDs for debugging

---

## 3. Pin Assignments

### 3.1 Raspberry Pi Pico 2 W #1 (Radio/Data Pico)

**Primary Functions:**
- RFD900x UART communication (UART0)
- MicroSD card data logging (SPI1)
- WiFi/MQTT ground station link
- Inter-Pico communication (UART1)

| GPIO | Function | Connection | Notes |
|------|----------|------------|-------|
| GP0 | UART0 TX | RFD900x RX | Radio telemetry |
| GP1 | UART0 RX | RFD900x TX | Radio telemetry |
| GP2 | UART0 CTS | RFD900x RTS | Flow control |
| GP3 | UART0 RTS | RFD900x CTS | Flow control |
| GP4 | UART1 TX | Pico #2 GP5 (RX) | Inter-Pico commands |
| GP5 | UART1 RX | Pico #2 GP4 (TX) | Inter-Pico status |
| GP10 | SPI1 SCK | MicroSD CLK | SD card |
| GP11 | SPI1 TX | MicroSD MOSI | SD card |
| GP12 | SPI1 RX | MicroSD MISO | SD card |
| GP13 | SPI1 CSn | MicroSD CS | SD card |
| GP22 | GPIO Input | MicroSD Card Detect | Card presence |
| GP26 | GPIO Output | Status LED (Green) | Activity indicator |

**UART0 Configuration (RFD900x):**
- Baud Rate: 57600 bps
- Data Format: 8N1
- Flow Control: RTS/CTS enabled
- Buffer Size: 512 bytes RX

**UART1 Configuration (Inter-Pico):**
- Baud Rate: 115200 bps
- Data Format: 8N1
- Flow Control: None
- Buffer Size: 256 bytes RX/TX

**SPI1 Configuration (MicroSD):**
- Clock Speed: 10 MHz (init), up to 25 MHz
- Mode: Mode 0
- File System: FAT32

---

### 3.2 Raspberry Pi Pico 2 W #2 (Motor Control Pico)

**Primary Functions:**
- Stepper motor control (PIO)
- GPS module communication (UART0)
- Angle calculations
- Inter-Pico communication (UART1)

| GPIO | Function | Connection | Notes |
|------|----------|------------|-------|
| GP0 | UART0 TX | GPS RX | Optional commands |
| GP1 | UART0 RX | GPS TX | NMEA sentences |
| GP4 | UART1 TX | Pico #1 GP5 (RX) | Inter-Pico status |
| GP5 | UART1 RX | Pico #1 GP4 (TX) | Inter-Pico commands |
| GP6 | GPIO Output | Azimuth STEP | Via 220Ω to DM556T |
| GP7 | GPIO Output | Azimuth DIR | Via 220Ω to DM556T |
| GP8 | GPIO Output | Azimuth ENA | Via 220Ω to DM556T |
| GP9 | GPIO Output | Elevation STEP | Via 220Ω to DM556T |
| GP10 | GPIO Output | Elevation DIR | Via 220Ω to DM556T |
| GP11 | GPIO Output | Elevation ENA | Via 220Ω to DM556T |
| GP28 | GPIO Output | Status LED (Red) | Motor activity |

**UART0 Configuration (GPS):**
- Baud Rate: 9600 bps
- Data Format: 8N1
- Protocol: NMEA 0183
- Update Rate: 1 Hz (standard), 5 Hz preferred

**UART1 Configuration (Inter-Pico):**
- Baud Rate: 115200 bps
- Data Format: 8N1
- Flow Control: None

**Stepper Control:**
- Step Resolution: 1.8° per full step (200 steps/rev)
- Microstepping: 1/16 (3200 steps/rev)
- Effective Resolution: 0.1125° per microstep
- Maximum Step Rate: 20 kHz practical

---

### 3.3 RFD900x Radio Modem Connections

| Pin | Signal | PCB Connection | Notes |
|-----|--------|----------------|-------|
| 1-2 | GND | Common Ground | Heavy ground |
| 3 | CTS | Pico #1 GP3 (RTS) | Flow control |
| 4 | Vcc | 5V Rail | 60mA typical |
| 7 | RX | Pico #1 GP0 (TX) | UART data |
| 9 | TX | Pico #1 GP1 (RX) | UART data |
| 11 | RTS | Pico #1 GP2 (CTS) | Flow control |
| 16 | GND | Common Ground | Redundant |

**Antenna Connections:**
- ANT1: RP-SMA connector (primary)
- ANT2: RP-SMA connector (diversity)
- Impedance: 50Ω controlled traces

**RFD900x Configuration:**
- Air Data Rate: 64 kbps
- TX Power: 30 dBm (1W)
- Serial Speed: 57600 bps
- Network ID: 217 (match rocket)

---

### 3.4 DM556T Stepper Driver Connections

**Azimuth Driver Connector (J1):**

| Pin | Signal | Connection | Notes |
|-----|--------|------------|-------|
| 1 | VCC | From driver | Not used |
| 2 | PUL- | GP6 via 220Ω | Step pulse |
| 3 | DIR- | GP7 via 220Ω | Direction |
| 4 | ENA- | GP8 via 220Ω | Enable (active low) |
| 5 | GND | Common Ground | Signal ground |

**Elevation Driver Connector (J2):**

| Pin | Signal | Connection | Notes |
|-----|--------|------------|-------|
| 1 | VCC | From driver | Not used |
| 2 | PUL- | GP9 via 220Ω | Step pulse |
| 3 | DIR- | GP10 via 220Ω | Direction |
| 4 | ENA- | GP11 via 220Ω | Enable (active low) |
| 5 | GND | Common Ground | Signal ground |

**External Power (NOT on PCB):**
- VMOT: 36VDC @ 5-10A per driver
- Motor: 4-wire NEMA 23, 18 AWG minimum

**DM556T Settings:**
- Pulse width: 10 µs minimum
- Step rate: 20 kHz maximum
- Acceleration: 1000 steps/s²

---

### 3.5 GPS Module Connections

| Pin | Signal | Connection | Notes |
|-----|--------|------------|-------|
| VCC | Power | 3.3V or 5V | Check datasheet |
| GND | Ground | Common GND | Ground reference |
| TX | UART TX | Pico #2 GP1 (RX) | NMEA out |
| RX | UART RX | Pico #2 GP0 (TX) | Commands (optional) |

**GPS Configuration:**
- Protocol: NMEA 0183
- Messages: GGA, RMC minimum
- Baud Rate: 9600 bps
- Accuracy: 2.5m CEP typical

---

### 3.6 MicroSD Card Socket

| Pin | Function | Connection | Notes |
|-----|----------|------------|-------|
| CS | Chip Select | Pico #1 GP13 | Active low |
| MOSI | Data In | Pico #1 GP11 | Data to card |
| CLK | Clock | Pico #1 GP10 | SPI clock |
| MISO | Data Out | Pico #1 GP12 | Data from card |
| VDD | Power | 3.3V | 50mA max |
| CD | Card Detect | Pico #1 GP22 | Presence detect |

**Configuration:**
- Clock: 10 MHz (init), 25 MHz (operation)
- File System: FAT32
- Max File Size: 4GB per file

---

## 4. Communication Protocols

### 4.1 RFD900x to Radio Pico (UART0)

**Physical Layer:**
- Baud Rate: 57600 bps
- Data Format: 8N1
- Flow Control: RTS/CTS hardware
- Direction: RFD900x TX → Pico RX (receive only)

**Packet Structure (107 bytes):**

| Offset | Field | Type | Size | Description |
|--------|-------|------|------|-------------|
| 0-3 | Sync Word | uint32 | 4 | 0x3E5D5967 |
| 4-5 | Metadata | bitfield | 2 | Validity flags |
| 6-9 | Timestamp | uint32 | 4 | Milliseconds since boot |
| 10-13 | Events | bitfield | 4 | Event flags |
| 14-17 | Altitude | float | 4 | Barometric (m) |
| 18-21 | Temperature | float | 4 | °C |
| 22-25 | Latitude | int32 | 4 | Microdegrees |
| 26-29 | Longitude | int32 | 4 | Microdegrees |
| 30 | GPS Satellites | uint8 | 1 | Count |
| 31-34 | GPS Time | uint32 | 4 | Unix timestamp |
| 35-38 | GPS Accuracy | uint32 | 4 | Millimeters |
| 39-74 | IMU Data | floats | 36 | Accel, gyro, orient |
| 75-86 | High-G Accel | floats | 12 | High-g accelerometer |
| 87-106 | Sensors | floats | 20 | Battery, pressure, temp |

**Metadata Bitfield:**
- Bit 0: Altitude armed
- Bit 1: Altimeter valid
- Bit 2: GPS valid
- Bit 3: IMU valid
- Bit 13-15: Flight mode (0=Startup, 1=Standby, 2=Ascent, 3=Drogue, 4=Main, 5=Fault)

**Packet Reception:**
- Rate: 5-10 Hz typical
- Sync Detection: Search for 0x3E5D5967
- Buffer: 512-byte circular buffer

---

### 4.2 Radio Pico to Motor Pico (UART1)

**Physical Layer:**
- Baud Rate: 115200 bps
- Data Format: 8N1
- Flow Control: None
- Update Rate: 10 Hz

**Command Packet Structure (Radio → Motor, 35 bytes):**

```
struct CommandPacket {
    uint8_t header;           // 0xAA
    uint8_t cmd_type;         // 0x01=Normal, 0x02=Predicted, 0x03=Home
    int32_t target_lat_udeg;  // Target latitude (microdegrees)
    int32_t target_lon_udeg;  // Target longitude (microdegrees)
    float target_alt_m;       // Target altitude (meters)
    float velocity_x_ms;      // Velocity X (m/s)
    float velocity_y_ms;      // Velocity Y (m/s)
    float velocity_z_ms;      // Velocity Z (m/s)
    uint16_t checksum;        // CRC16-CCITT
};
```

**Status Packet Structure (Motor → Radio, 31 bytes):**

```
struct StatusPacket {
    uint8_t header;           // 0xBB
    uint8_t status_flags;     // Motor status bitfield
    float current_az_deg;     // Current azimuth (degrees)
    float current_el_deg;     // Current elevation (degrees)
    float turret_lat_udeg;    // Turret GPS latitude
    float turret_lon_udeg;    // Turret GPS longitude
    float turret_alt_m;       // Turret GPS altitude
    uint16_t checksum;        // CRC16-CCITT
};
```

**Status Flags:**
- Bit 0: Motors enabled
- Bit 1: Azimuth homed
- Bit 2: Elevation homed
- Bit 3: At target position
- Bit 4: GPS lock valid
- Bit 5: Azimuth limit hit
- Bit 6: Elevation limit hit
- Bit 7: Error state

---

### 4.3 Radio Pico to Ground Station (MQTT)

**MQTT Topics:**

| Topic | Direction | Rate | Description |
|-------|-----------|------|-------------|
| rats/telemetry | Publish | 10 Hz | Full rocket telemetry |
| rats/status | Publish | 1 Hz | System status |
| rats/turret | Publish | 1 Hz | Turret position |
| rats/command | Subscribe | On-demand | Ground commands |

**Telemetry Payload (JSON):**
- Rocket: lat, lon, alt, velocity, attitude, flight_mode
- Turret: azimuth, elevation, tracking status
- Link: RSSI, packet rate

**Ground Commands:**
- "home": Return to 0°, 0°
- "emergency_stop": Stop motors
- "resume": Resume tracking
- "reboot": Reboot system

---

### 4.4 GPS to Motor Pico (UART0)

**Physical Layer:**
- Baud Rate: 9600 bps
- Protocol: NMEA 0183
- Update Rate: 1 Hz minimum

**Required NMEA Sentences:**
- GGA: Position, altitude, satellite count
- RMC: Position, date/time, validity

**Parsing:**
- 128-byte circular buffer
- Character-by-character state machine
- Checksum validation
- 2-second timeout for GPS lock

---

## 5. Software Architecture

### 5.1 Project Structure

```
RATS/
├── Common/                   # Shared code
│   ├── packet_types.h        # Telemetry structures
│   ├── packet_parser.cpp     # Packet parsing
│   ├── serial_protocol.h     # Inter-Pico protocol
│   ├── config.h              # Configuration
│   └── crc16.cpp             # Checksums
│
├── RadioPico/                # Radio/Data Pico
│   ├── RadioPico.cpp         # Main (core assignment)
│   ├── rfd900x_uart.cpp      # RFD900x interface
│   ├── inter_pico_uart.cpp   # UART to Motor Pico
│   ├── sd_logger.cpp         # SD card logging
│   ├── mqtt_client.cpp       # WiFi/MQTT
│   ├── predictor.cpp         # Trajectory prediction
│   └── ring_buffer.h         # Circular buffer
│
├── StepperPico/              # Motor Control Pico
│   ├── StepperPico.cpp       # Main (core assignment)
│   ├── stepper_control.cpp   # PIO motor control
│   ├── stepper_control.pio   # PIO assembly
│   ├── angle_calculator.cpp  # Pointing math
│   ├── gps_parser.cpp        # NMEA parsing
│   └── inter_pico_uart.cpp   # UART from Radio Pico
│
└── Tests/                    # Test utilities
    ├── packet_simulator.cpp  # Fake telemetry
    └── test_parser.cpp       # Unit tests
```

### 5.2 Core Assignment Strategy

**Radio Pico:**
- Core 0: RFD900x reception, packet parsing, inter-Pico UART
- Core 1: WiFi/MQTT, SD logging, trajectory prediction

**Motor Pico:**
- Core 0: PIO motor control, position tracking
- Core 1: Inter-Pico UART, GPS parsing, angle calculation

**Communication:** Lockless queues between cores

---

## 6. Data Flow

### 6.1 Normal Operation

1. **Rocket transmits** telemetry @ 10 Hz via LoRa (900MHz)
2. **RFD900x receives** and forwards to Radio Pico via UART0
3. **Radio Pico Core 0** parses packet, validates sync word
4. **Radio Pico Core 0** sends target position to Motor Pico via UART1
5. **Motor Pico Core 1** receives command, gets turret GPS position
6. **Motor Pico Core 1** calculates azimuth/elevation angles
7. **Motor Pico Core 1** sends target to Core 0 via lockless queue
8. **Motor Pico Core 0** generates step pulses via PIO
9. **Motors move** to point antenna at rocket
10. **Radio Pico Core 1** logs to SD and publishes to MQTT (parallel)

**Total Latency:** ~2 ms (packet RX to motor update)

### 6.2 Link Loss Mode

1. **Radio Pico Core 0** detects no packets for >500ms
2. **Radio Pico Core 1** initializes prediction from last known state
3. **Radio Pico Core 1** calculates ballistic trajectory
4. **Radio Pico Core 1** sends predicted positions @ 10 Hz to Motor Pico
5. **Motor Pico** continues tracking predicted position
6. **Radio Pico Core 0** detects packet → clears loss flag
7. **System resumes** normal tracking

**Prediction Algorithm:**
- Ballistic trajectory using last position and velocity
- Applies gravity (9.81 m/s²)
- Updates every 100ms

---

## 7. Packet Structures

### 7.1 Rocket Telemetry Packet (107 bytes)

```c
#pragma pack(push, 1)
struct TelemetryPacket {
    uint32_t sync_word;        // 0x3E5D5967
    uint16_t metadata;         // Validity flags
    uint32_t timestamp_ms;     // Milliseconds since boot
    uint32_t events;           // Event flags
    float altitude_m;          // Barometric altitude
    float temperature_c;       // Temperature
    int32_t latitude_udeg;     // Latitude * 1e6
    int32_t longitude_udeg;    // Longitude * 1e6
    uint8_t satellites;        // GPS satellite count
    uint32_t gps_time_unix;    // Unix timestamp
    uint32_t h_accuracy_mm;    // Horizontal accuracy
    float accel_x_mps2;        // IMU acceleration X
    float accel_y_mps2;        // IMU acceleration Y
    float accel_z_mps2;        // IMU acceleration Z
    float gyro_x_dps;          // IMU gyro X
    float gyro_y_dps;          // IMU gyro Y
    float gyro_z_dps;          // IMU gyro Z
    float orient_x_deg;        // Orientation roll
    float orient_y_deg;        // Orientation pitch
    float orient_z_deg;        // Orientation yaw
    float accel_hg_x_g;        // High-G accel X
    float accel_hg_y_g;        // High-G accel Y
    float accel_hg_z_g;        // High-G accel Z
    float battery_v;           // Battery voltage
    float pt3_psi;             // Pressure transducer 3
    float pt4_psi;             // Pressure transducer 4
    float rtd_temp_c;          // RTD temperature
    float blims_state_in;      // Motor position
};
#pragma pack(pop)
```

### 7.2 Inter-Pico Packets

See section 4.2 for CommandPacket and StatusPacket structures.

---

## 8. Core Responsibilities

### 8.1 Radio Pico - Core 0

**Main Loop:**
1. Check UART RX buffer for data from RFD900x
2. Search for sync word (0x3E5D5967)
3. Read 107-byte packet when complete
4. Validate packet (metadata flags)
5. Extract GPS data (lat, lon, alt)
6. Format command packet
7. Send to Motor Pico via UART1
8. Queue packet for Core 1 (MQTT/SD)
9. Check for link loss (>500ms timeout)
10. Sleep 100µs

**Timing:** <1ms per iteration

**Constraints:** NO WiFi, NO SD writes, NO printf

### 8.2 Radio Pico - Core 1

**Main Loop:**
1. Check MQTT queue → format JSON → publish
2. Check SD queue → batch write (10 packets)
3. Monitor link health → start prediction if lost
4. Calculate predicted positions @ 10 Hz
5. Publish status @ 1 Hz
6. Handle ground commands
7. Sleep 10ms

**Timing:** 10-100ms per task

### 8.3 Motor Pico - Core 0

**Main Loop:**
1. Get target position from Core 1 queue
2. Calculate step requirements (azimuth)
3. Apply acceleration profile
4. Feed PIO FIFO with step count
5. Set direction pin
6. Track current position
7. Repeat for elevation axis
8. Check safety limits
9. Update position for Core 1
10. Sleep 1ms (1 kHz loop)

**Timing:** 1 kHz loop, ±1µs precision

**Constraints:** NO UART, NO blocking operations

### 8.4 Motor Pico - Core 1

**Main Loop:**
1. Check UART1 for command from Radio Pico
2. Parse command packet, validate CRC
3. Read GPS NMEA sentences
4. Calculate pointing angles (azimuth/elevation)
5. Convert to motor steps
6. Send target to Core 0 via queue
7. Update status packet
8. Send status to Radio Pico via UART1
9. Sleep 100µs

**Timing:** 1-10ms per task

---

## 9. Critical Timing Requirements

### 9.1 Packet Reception

| Stage | Target | Maximum | Notes |
|-------|--------|---------|-------|
| UART RX interrupt | <10 µs | 50 µs | Copy to buffer |
| Sync word detection | <100 µs | 500 µs | Search buffer |
| Packet parsing | <300 µs | 1 ms | Extract fields |
| UART transfer (35 bytes) | ~3 ms | 5 ms | @ 115200 bps |
| Total latency | <5 ms | 10 ms | RX to motor |

### 9.2 Motor Control

| Parameter | Value | Notes |
|-----------|-------|-------|
| Step pulse width | 10 µs | DM556T min: 2.5 µs |
| Step pulse period | 50 µs | 20 kHz max rate |
| Position update | 1 kHz | Core 0 loop |
| Acceleration | 10000 steps/s² | Prevents stalling |
| Max velocity | 20000 steps/s | ~6 rev/s |

### 9.3 Network/Storage (Non-Critical)

| Operation | Target | Maximum |
|-----------|--------|---------|
| MQTT publish | <20 ms | 100 ms |
| SD write (batch) | <50 ms | 200 ms |
| JSON formatting | <5 ms | 20 ms |
| WiFi reconnect | N/A | 30 s |

---

## 10. Implementation Roadmap

### Phase 1: Hardware Bring-Up (Week 1-2)
- Assemble PCB, solder components
- Power-on testing, measure voltages
- Program both Picos with "Hello World"
- Test UART communication (loopback tests)
- Test GPIO (LED blink, motor enable)

### Phase 2: Radio Reception (Week 3)
- Implement packet parser
- Test with packet simulator
- Implement RFD900x UART driver
- Test with real RFD900x
- Measure packet reception rate

### Phase 3: Inter-Pico Communication (Week 4)
- Implement UART protocol (both Picos)
- Test command/status exchange
- Validate CRC checksums
- Test at 10 Hz update rate

### Phase 4: Motor Control (Week 5-6)
- Implement PIO stepper program
- Test single motor (azimuth)
- Implement acceleration profiles
- Test both motors simultaneously
- Calibrate steps-per-degree

### Phase 5: Angle Calculation (Week 7)
- Implement GPS parser
- Test with real GPS module
- Implement angle calculation
- Validate with known coordinates
- Test end-to-end: GPS → motors

### Phase 6: WiFi/MQTT (Week 8)
- Implement WiFi connection
- Implement MQTT client
- Test telemetry publishing
- Test command reception

### Phase 7: SD Card Logging (Week 9)
- Implement SD card driver
- Test file writes
- Implement batch writing
- Test data integrity

### Phase 8: Prediction & Link Loss (Week 10)
- Implement trajectory prediction
- Test with simulated link loss
- Validate prediction accuracy

### Phase 9: Integration Testing (Week 11-12)
- End-to-end system test
- Performance profiling
- Stress testing (4+ hours)
- Bug fixes and optimization

### Phase 10: Launch Preparation (Week 13-14)
- Final hardware checkout
- Calibration procedures
- Pre-flight checklist
- Launch day operations

---

## 11. Testing Strategy

### 11.1 Unit Tests (PC-based)

Test packet parser with valid/invalid packets, test angle calculator with known coordinates, verify CRC implementation.

### 11.2 Hardware Tests

**Packet Reception Rate:**
- Target: 10 Hz sustained
- Pass: >95% reception

**UART Latency:**
- Target: <5 ms
- Pass: <10 ms worst case

**Motor Accuracy:**
- Target: ±0.5° error
- Pass: ±1.0° error

**End-to-End Latency:**
- Target: <5 ms
- Pass: <10 ms

### 11.3 System Tests

**Continuous Operation:**
- Duration: 4 hours @ 10 Hz
- Pass: <1% packet loss, 0 crashes

**Link Loss Recovery:**
- Simulate 10s blackout
- Pass: Prediction activates, seamless restoration

**WiFi Resilience:**
- Simulate disconnect/reconnect
- Pass: <30s reconnect, no motor impact

---

## 12. Configuration Reference

### 12.1 Constants

```cpp
// Radio
#define RFD_UART_ID        uart0
#define RFD_BAUD_RATE      57600

// Inter-Pico
#define INTER_PICO_UART_ID uart1
#define INTER_PICO_BAUD    115200

// Motor
#define STEPS_PER_REV      3200
#define STEPS_PER_DEGREE   (STEPS_PER_REV / 360.0)
#define MAX_STEP_RATE      20000
#define ACCELERATION       10000

// Limits
#define AZ_MIN_ANGLE       -180.0
#define AZ_MAX_ANGLE       180.0
#define EL_MIN_ANGLE       0.0
#define EL_MAX_ANGLE       90.0

// Timing
#define LINK_LOSS_TIMEOUT_MS  500
#define GPS_TIMEOUT_MS        2000

// Earth
#define EARTH_RADIUS       6371000.0  // meters
#define GRAVITY            9.81       // m/s²

// WiFi/MQTT
#define WIFI_SSID          "your_ssid"
#define WIFI_PASSWORD      "your_password"
#define MQTT_BROKER        "192.168.1.100"
#define MQTT_PORT          1883
```

### 12.2 Pin Summary

**Radio Pico #1:**
- GP0-3: RFD900x UART0 (TX, RX, CTS, RTS)
- GP4-5: Inter-Pico UART1 (TX, RX)
- GP10-13: MicroSD SPI1
- GP22: SD card detect
- GP26: Status LED (green)

**Motor Pico #2:**
- GP0-1: GPS UART0 (TX, RX)
- GP4-5: Inter-Pico UART1 (TX, RX)
- GP6-8: Azimuth motor (STEP, DIR, ENA)
- GP9-11: Elevation motor (STEP, DIR, ENA)
- GP28: Status LED (red)

### 12.3 Calibration

**Home Position:**
1. Manually position to 0°, 0° (north, horizon)
2. Send home command via MQTT
3. Verify motors remain stationary

**Steps-Per-Degree:**
1. Attach laser pointer
2. Command 360° rotation
3. Measure actual rotation
4. Calculate: actual_steps / actual_degrees
5. Update STEPS_PER_DEGREE in config.h

**GPS Position:**
1. Position turret at known location
2. Average coordinates over 5 minutes
3. Compare with GPS module output
4. Document any offset

**Angle Validation:**
1. Place target at known GPS coordinates
2. Manually aim antenna
3. Measure angles (compass, inclinometer)
4. Compare with calculated angles
5. Acceptable error: ±1°

### 12.4 Pre-Flight Checklist

**T-1 Hour:**
- [ ] Connect all cables (power, motors, antennas)
- [ ] Power on, verify both Picos boot
- [ ] Check LED status (green/red)
- [ ] Verify GPS lock (8+ satellites)
- [ ] Test motor movement (full range)
- [ ] Confirm SD card mounted
- [ ] Test RFD900x reception
- [ ] Confirm MQTT publishing

**T-15 Minutes:**
- [ ] Clear antenna field of view
- [ ] Secure all cables
- [ ] Final GPS position check
- [ ] Set system to Armed mode
- [ ] Verify link with rocket

**T-0 (Launch):**
- [ ] Confirm tracking active
- [ ] Monitor antenna movement
- [ ] Watch for link loss events

**T+Recovery:**
- [ ] Download SD card data
- [ ] Export MQTT logs
- [ ] Review tracking accuracy

### 12.5 Troubleshooting

**No packets received:**
- Check RFD900x power LED
- Verify antenna connections
- Check UART wiring (TX↔RX)
- Test with rocket transmitter

**Motors not moving:**
- Check enable pins (should be LOW)
- Verify 36V supply connected
- Test with single step command
- Check driver DIP switches

**Tracking inaccurate:**
- Recalibrate steps-per-degree
- Check GPS lock quality
- Verify turret GPS coordinates
- Test angle calculation

**WiFi won't connect:**
- Verify SSID/password
- Check 2.4GHz availability
- Move closer to access point
- Check firewall (port 1883)

**SD card errors:**
- Reformat card (FAT32)
- Try different card (32GB max)
- Check card detect switch
- Test card in PC first

---

**END OF DOCUMENT**
