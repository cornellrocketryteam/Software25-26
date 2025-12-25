# Rotational Antenna Tracking System (RATS)
## System Design Document v1.0

**Project:** Cornell Rocket Team Launch Vehicle Tracking System
**Date:** November 2025
**Status:** Development & Testing Phase

---

## Implementation Status

**COMPLETED (Radio Pico):**
- RFD900x UART Reception: Full packet reception at 115200 baud (UART0, GP0/GP1)
- Packet Parsing: 107-byte telemetry packets with sync word detection
- SD Card Logging: CSV logging to microSD card via SPI1 (GP10-13), all 27 fields
- Dual-Core Architecture: Core 0 for real-time I/O, Core 1 for logging/MQTT
- Test Mode: Loopback test mode with simulated packets (GP0→GP1 jumper)
- MQTT/Wi-Fi: Publishing full packets to MQTT broker
- Inter-Pico UART: Sending tracking data (lat/lon/alt) via UART1 (GP4 TX)

**IN PROGRESS:**
- Ground Station Integration: MQTT topic structure and command handling
- Motor Control Pico: Receiving tracking data, motor control TBD
  
**TODO:**
- Motor Control (Stepper Pico): Motor control functionality
- Angle Calculation: GPS coordinate to azimuth/elevation conversion
- Trajectory Prediction: Link loss handling with ballistic prediction
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
10. [Testing Strategy](#11-testing-strategy)
11. [Configuration Reference](#12-configuration-reference)

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
- 2x Stepperonline E Series NEMA 23 stepper motors (3.0 Nm holding torque, 4.2A)
- 2x Stepperonline DM556T stepper drivers (1.8-5.6A, 20-50VDC)
- Farnell YAGI-868/914A directional antenna (900MHz, RP-SMA)
- Turret assembly (azimuth/elevation gimbal)

**Power:**
- PCB: 5V @ 1A (USB-C input)
- Motors: 48VDC @ 10A power supply (external)

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
- RFD900x UART reception (UART0)
- MicroSD card logging (SPI1)
- WiFi/MQTT ground station link
- Inter-Pico UART transmission (UART1)

| GPIO | Function | Connection | Notes |
|------|----------|------------|-------|
| GP0 | UART0 TX | RFD900x RX (Pin 7) | TX only used in loopback test mode |
| GP1 | UART0 RX | RFD900x TX (Pin 9) | **Telemetry from radio** |
| GP4 | UART1 TX | Stepper Pico GP5 (RX) | **Tracking data to Stepper Pico** |
| GP10 | SPI1 SCK | MicroSD CLK | SD card clock |
| GP11 | SPI1 MOSI | MicroSD CMD | SD card data input |
| GP12 | SPI1 MISO | MicroSD D0 | SD card data output |
| GP13 | SPI1 CS | MicroSD CS | Chip select |
| GP25 | Onboard LED | Internal | Status indicator |

**UART0 (RFD900x Reception):**
- Baud: 115200 bps, 8N1
- Direction: RX only (GP1 receives from RFD900x Pin 9)
- Buffer: 512 bytes circular, interrupt-driven
- Sync Word: 0x3E5D5967 ("CRT!")
- Packet: 107 bytes @ 10 Hz

**UART1 (Inter-Pico Transmission):**
- Baud: 115200 bps, 8N1
- Direction: TX only (GP4 transmits to Stepper Pico GP5)
- Sync Word: 0x54524B21 ("TRK!")
- Packet: 16 bytes (4-byte sync + 12-byte TrackingData)
- Rate: Every received packet (~10 Hz)
- Data: latitude (µdeg), longitude (µdeg), altitude (m)

**SPI1 (MicroSD Card):**
- Clock: 12.5 MHz
- Format: FAT32, CSV with 27 fields
- Batch: 10 packets per write
- Card Detect: DISABLED (unreliable)

---

### 3.2 Raspberry Pi Pico 2 W #2 (Stepper/Motor Pico)

**Primary Functions:**
- Inter-Pico UART reception (UART1)
- Stepper motor control (DM556T drivers)
- Angle calculations (azimuth/elevation)
- GPS module (UART0) - optional

| GPIO | Function | Connection | Notes |
|------|----------|------------|-------|
| GP5 | UART1 RX | Radio Pico GP4 (TX) | **Receives tracking data** |
| GP6 | STEP | Azimuth Driver PUL- | Step pulses |
| GP7 | DIR | Azimuth Driver DIR- | Direction |
| GP8 | ENA | Azimuth Driver ENA- | Enable (active low) |
| GP9 | STEP | Elevation Driver PUL- | Step pulses |
| GP10 | DIR | Elevation Driver DIR- | Direction |
| GP11 | ENA | Elevation Driver ENA- | Enable (active low) |

**UART1 (Inter-Pico Reception):**
- Baud: 115200 bps, 8N1
- Direction: RX only (GP5 receives from Radio Pico GP4)
- Sync Word: 0x54524B21 ("TRK!")
- Packet: 16 bytes (4-byte sync + 12-byte TrackingData)
- Rate: ~10 Hz
- Data: latitude (µdeg), longitude (µdeg), altitude (m)

**Receiver Example:**
```cpp
#include "hardware/uart.h"
#include "hardware/irq.h"
#include "serial_protocol.h"

#define UART_ID uart1
#define UART_RX_PIN 5
#define BAUD_RATE 115200
#define SYNC_WORD 0x54524B21

volatile uint8_t rx_buffer[256];
volatile uint32_t rx_write = 0, rx_read = 0;

void on_uart_rx() {
    while (uart_is_readable(UART_ID)) {
        rx_buffer[rx_write++] = uart_getc(UART_ID);
        rx_write %= 256;
    }
}

void init() {
    uart_init(UART_ID, BAUD_RATE);
    gpio_set_function(UART_RX_PIN, GPIO_FUNC_UART);
    irq_set_exclusive_handler(UART1_IRQ, on_uart_rx);
    irq_set_enabled(UART1_IRQ, true);
    uart_set_irq_enables(UART_ID, true, false);
}

bool receive_packet(TrackingData* data) {
    // Wait for 16 bytes (4 sync + 12 data)
    if ((rx_write - rx_read) % 256 < 16) return false;

    // Find sync word
    uint32_t sync = 0;
    for (int i = 0; i < 4; i++) {
        sync |= rx_buffer[(rx_read + i) % 256] << (i * 8);
    }

    if (sync != SYNC_WORD) {
        rx_read = (rx_read + 1) % 256; // Advance by 1
        return false;
    }

    // Read data (skip sync)
    rx_read = (rx_read + 4) % 256;
    for (int i = 0; i < 12; i++) {
        ((uint8_t*)data)[i] = rx_buffer[rx_read++];
        rx_read %= 256;
    }

    return true;
}
```

**Stepper Control:**
- Motor: NEMA 23, 3.0 Nm, 1.8°/step (200 steps/rev)
- Driver: DM556T, microstepping 1/2 to 1/256 (DIP switches)
- Min Pulse: 2.5 µs, signals are 3.3V compatible

---

### 3.3 RFD900x Radio Modem Connections

| Pin | Signal | PCB Connection | Notes |
|-----|--------|----------------|-------|
| 1-2 | GND | Common Ground | Heavy ground |
| 4 | Vcc | 5V Rail | 60mA typical |
| 9 | TX | Pico #1 GP1 (RX) | UART data |
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

**STATUS: Implementation TBD**

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
- VMOT: 48VDC from external PSU (Generic 48V 10A AC-DC)
- Motor: Stepperonline E Series NEMA 23 (4-wire, bipolar)
- Current: 4.2A per phase

**DM556T Configuration:**
- Pulse width: 2.5 µs minimum
- Signal level: 3.3V compatible (3-5V)
- Microstepping: Set via DIP switches (SW5-SW8)
- Current: Set via DIP switches (SW1-SW3)
- Enable: Active low (pull low to enable)

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
- Baud Rate: 115200 bps
- Data Format: 8N1
- Flow Control: None
- Direction: RFD900x TX → Pico GP1 RX (receive only)

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

**STATUS: Implementation TBD**

**Physical Layer:**
- Baud Rate: 115200 bps
- Data Format: 8N1
- Flow Control: None
- Pins: Radio Pico GP4/GP5 ↔ Motor Pico GP5/GP4
- Update Rate: 10 Hz target

**Proposed Command Packet Structure (Radio → Motor):**

```cpp
// TODO: Define exact packet format
// Minimum required data:
// - Target GPS coordinates (lat, lon, alt)
// - Command type (track, home, emergency stop)
// - Checksum/CRC
//
// Example:
// struct CommandPacket {
//     uint8_t header;           // 0xAA
//     uint8_t cmd_type;         // 0x01=Track, 0x02=Home, 0x03=Stop
//     int32_t target_lat_udeg;  // Target latitude (microdegrees)
//     int32_t target_lon_udeg;  // Target longitude (microdegrees)
//     float target_alt_m;       // Target altitude (meters)
//     uint16_t checksum;        // CRC16-CCITT
// };
```

**Proposed Status Packet Structure (Motor → Radio):**

```cpp
// TODO: Define exact packet format
// Suggested data to send back:
// - Current motor position (azimuth, elevation)
// - Status flags (homed, tracking, error)
// - GPS position of turret
//
// Example:
// struct StatusPacket {
//     uint8_t header;           // 0xBB
//     uint8_t status_flags;     // Motor status bitfield
//     float current_az_deg;     // Current azimuth (degrees)
//     float current_el_deg;     // Current elevation (degrees)
//     uint16_t checksum;        // CRC16-CCITT
// };
```

**Implementation Tasks:**
1. Define final packet structures in Common/serial_protocol.h
2. Implement UART send/receive in both Picos
3. Implement CRC16 checksum validation
4. Test bidirectional communication at 10 Hz
5. Handle packet loss and timeouts

---

### 4.3 Radio Pico to Ground Station (MQTT)

**STATUS: Implementation TBD**

**MQTT Configuration:**
- Broker Address: TBD
- Port: 1883 (standard MQTT)
- WiFi SSID/Password: See config.h
- QoS Level: TBD

**Proposed Topics:**

| Topic | Direction | Rate | Description |
|-------|-----------|------|-------------|
| rats/raw/{unit_id} | Publish | 10 Hz | Full rocket telemetry (JSON) |
| rats/status/{unit_id} | Publish | 1 Hz | System status |
| rats/command/{unit_id} | Subscribe | On-demand | Ground commands |

**Implementation Notes:**
- Use existing mqtt_client.cpp as starting point
- Telemetry already formatted as JSON by packet_parser.cpp
- WiFi credentials configured in Common/config.h
- MQTT broker address in config.h: MQTT_BROKER_ADDRESS
- Recommend using PubSubClient or Pico SDK MQTT library

**Implementation Tasks:**
1. Define exact topic structure
2. Define command message format
3. Implement MQTT publish in Core 1 loop
4. Test WiFi connection reliability
5. Handle reconnection logic

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
│   ├── ...
|
├── RadioPico/                # Radio/Data Pico
│   ├── RadioPico.cpp         # Main (core assignment)
│   ├── ...
│
├── StepperPico/              # Motor Control Pico
│   ├── StepperPico.cpp       # Main (core assignment)
│   ├── ...
│
└── Tests/                    # Test utilities
    ├── packet_simulator.cpp  # Fake telemetry
    └── ...
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
10. Continue immediately (tight_loop_contents)

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

**STATUS: Implementation TBD**

**Proposed Main Loop:**
1. Get target position from Core 1 queue
2. Calculate step requirements for both axes
3. Apply acceleration/deceleration profiles
4. Generate step pulses (via PIO or GPIO)
5. Update current position tracking
6. Check safety limits (min/max angles)
7. Sleep briefly (target 1 kHz update rate)

**Suggested Timing:** 1 kHz loop

**Constraints:**
- NO UART on Core 0 (use Core 1 for UART)
- NO blocking operations
- Keep loop fast for responsive motor control

**TODO:**
- Implement step pulse generation (PIO or GPIO)
- Implement acceleration profiles
- Add limit switch support (optional)

### 8.4 Motor Pico - Core 1

**STATUS: Implementation TBD**

**Proposed Main Loop:**
1. Check UART1 for commands from Radio Pico
2. Parse command packet, validate checksum
3. Read GPS (if using GPS for turret position)
4. Calculate pointing angles (azimuth/elevation) from GPS coordinates
5. Send target angles to Core 0 via queue
6. Send status back to Radio Pico via UART1
7. Sleep briefly

**Suggested Timing:** 10-100 Hz update rate

**TODO:**
- Implement angle calculation algorithm
- Implement UART receive/transmit
- Add GPS parsing (optional)
- Handle command timeouts

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

## 10. Testing Strategy

### 10.1 Unit Tests (PC-based)

Test packet parser with valid/invalid packets, test angle calculator with known coordinates, verify CRC implementation.

### 10.2 Hardware Tests

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

### 10.3 System Tests

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

## 11. Configuration Reference

### 11.1 Constants

```cpp
// Radio (IMPLEMENTED - see Common/config.h)
#define RFD_UART_ID        uart0
#define RFD900X_BAUD_RATE  115200

// Inter-Pico (TODO - Motor Control team)
#define INTER_PICO_UART_ID uart1
#define INTER_PICO_BAUD    115200

// Motor (TBD - configure based on DM556T microstepping settings)
#define MOTOR_FULL_STEPS   200     // NEMA 23: 1.8° per step
#define MICROSTEPS         TBD     // Set via DM556T DIP switches (2-256)
#define STEPS_PER_REV      (MOTOR_FULL_STEPS * MICROSTEPS)
#define STEPS_PER_DEGREE   (STEPS_PER_REV / 360.0)
#define MAX_STEP_RATE      TBD     // Test with actual motors (Hz)
#define ACCELERATION       TBD     // Tune for smooth motion (steps/s²)

// Limits (TBD - set based on mechanical constraints)
#define AZ_MIN_ANGLE       -180.0  // Or physical limit
#define AZ_MAX_ANGLE       180.0   // Or physical limit
#define EL_MIN_ANGLE       0.0
#define EL_MAX_ANGLE       90.0

// Timing
#define LINK_LOSS_TIMEOUT_MS  500
#define GPS_TIMEOUT_MS        2000
#define PACKET_RATE_HZ        10
#define SD_LOG_BATCH_SIZE     10

// Earth
#define EARTH_RADIUS       6371000.0  // meters
#define GRAVITY            9.81       // m/s²

// WiFi/MQTT (TODO - Ground Station team - see Common/config.h)
#define WIFI_SSID          "CornellRocketry-2.4G"
#define WIFI_PASSWORD      "Rocketry2526"
#define MQTT_BROKER        "192.168.1.2"
#define MQTT_PORT          1883
#define RATS_UNIT_ID       1

// Sync Word (IMPLEMENTED)
#define SYNC_WORD          0x3E5D5967  // "CRT!"
```

### 11.2 Pin Summary

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

### 11.3 Calibration

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

### 11.4 Troubleshooting

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
