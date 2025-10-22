# Rotational Antenna Tracking System (RATS)
## Complete System Design Document

---

## Table of Contents
1. [System Overview](#system-overview)
2. [Hardware Architecture](#hardware-architecture)
3. [Software Architecture](#software-architecture)
4. [Core Assignment Strategy](#core-assignment-strategy)
5. [Communication Protocols](#communication-protocols)
6. [PCB?](#pcb?)
7. [Data Flow](#data-flow)

---

## 1. System Overview

### Purpose
The RATS system tracks a launch vehicle during flight by aiming a directional Yagi antenna at the rocket using real-time GPS telemetry received via LoRa radio (RFD900x).

### Key Requirements
- Receive LoRa packets at 10Hz (maximum rate)
- Parse 107-byte telemetry packets
- Calculate antenna pointing angles (azimuth/elevation)
- Control stepper motors for smooth antenna movement
- Log all data to SD cards
- Send telemetry to ground station via MQTT
- Maintain tracking even during temporary link loss

### System Components
1. **Radio Pico**: Receives RFD900x telemetry, handles MQTT, logs data
2. **Stepper Pico**: Controls antenna motors with precise timing
3. **RFD900x Radio Module**: 900MHz LoRa transceiver
4. **Yagi Antenna**: Directional antenna on motorized turret
5. **Stepper Motors (2x)**: Azimuth and elevation angle control
6. **SD Cards (1-2x)**: Data logging on each Pico(Might just do Radio Pico)
7. **Ground Station**: Database and visualization

---

## 2. Hardware Architecture
### Component Connections
#### Radio Pico Connections
| Component | Interface | Pins | Notes |
|-----------|-----------|------|-------|
| RFD900x | UART1 | GP4 (TX), GP5 (RX) | 57600 baud |
| Stepper Pico | UART0 | GP0 (TX), GP1 (RX) | 115200 baud |
| SD Card | SPI0 | GP16-19 | FatFS library |
| USB | Native | USB port | Ground station comms |
| Status LED | GPIO | GP25 | Onboard LED |

#### Stepper Pico Connections
| Component | Interface | Pins | Notes |
|-----------|-----------|------|-------|
| Radio Pico | UART0 | GP0 (RX), GP1 (TX) | 115200 baud |
| Azimuth Motor | PIO/GPIO | GP2 (STEP), GP3 (DIR) | Via driver |
| Elevation Motor | PIO/GPIO | GP6 (STEP), GP7 (DIR) | Via driver |
| SD Card | SPI0 | GP16-19 | FatFS library |
| Limit Switches | GPIO | GP8-11 | Homing and safety |
| Status LED | GPIO | GP25 | Onboard LED |

---

## 3. Software Architecture

### Folder Structure

```
RATS/
├── README.md - TODO
├── Common/
│   ├── packet_types.h
│   ├── packet_parser.h
│   ├── packet_parser.cpp
│   ├── serial_protocol.h
│   ├── config.h
| + ...
├── RadioPico/
│   ├── CMakeLists.txt
│   ├── RadioPico.cpp
│   ├── rfd900x_uart.h
│   ├── rfd900x_uart.cpp
| + ...
├── StepperPico/
│   ├── CMakeLists.txt
│   ├── StepperPico.cpp
| + ...
├── Tests/
│   ├── packet_simulator.h
│   ├── packet_simulator.cpp
| + ...
```

---

## 4. Core Assignment Strategy

### Radio Pico Core Assignment

#### Core 0: Real-Time I/O
**Responsibilities:**
- RFD900x UART reception
- Packet parsing and validation
- UART transmission to Stepper Pico
- Ground station transmission

**Key Characteristics:**
- Priority on packet reception
- No network or SD operations
- Fast turnaround time (<1ms per packet)
- 
#### Core 1: Processing and Networking
**Responsibilities:**
- MQTT publishing over WiFi/Ethernet
- SD card logging (batch writes)
- Prediction logic (when link lost)
- JSON formatting
- Network error handling

**Key Characteristics:**
- Can block on network/SD operations
- Lower priority operations
- Handles slower background tasks
- Queue-based communication from Core 0

---

### Stepper Pico Core Assignment

#### Core 0: Motor Control ONLY
**Responsibilities:**
- PIO state machine management for step pulses
- Step timing and acceleration profiles
- Position tracking
- Safety limits enforcement

**Key Characteristics:**
- Zero blocking operations
- Hardware-level timing via PIO
- Microsecond precision
- No SD, no UART on this core

#### Core 1: Communication and Logic
**Responsibilities:**
- UART reception from Radio Pico
- Angle calculation (lat/lon/alt → azimuth/elevation)
- Trajectory interpolation
- SD card logging (motor positions) - maybe?
- Limit switch monitoring

**Key Characteristics:**
- Handles all blocking operations
- Math-heavy calculations
- Communication with Radio Pico
- Logging for debugging

---

## 5. Communication Protocols

### RFD900x to Radio Pico (UART)
- **Baud Rate**: 57600 (configurable via RFD Tools)
- **Format**: 8N1 (8 data bits, no parity, 1 stop bit)
- **Packet Size**: 107 bytes
- **Sync Word**: 0x3E5D5967 (CRT!) (first 4 bytes)
- **Rate**: Up to 10Hz

### Radio Pico to Stepper Pico (UART)
- **Baud Rate**: 115200
- **Format**: 8N1
- **Packet Size**: 12 bytes (minimal for real-time)
- **Rate**: 10Hz (matching RFD900x)

**Packet Structure:**
```cpp
struct TrackingData {
    int32_t latitude_udeg;   // 4 bytes (microdegrees)
    int32_t longitude_udeg;  // 4 bytes (microdegrees)
    float altitude;          // 4 bytes (meters)
};
```

### Radio Pico to Ground Station
TBD

---

## 6. PCB?
TBD


---

## 7. Data Flow

### Normal Operation Data Flow

```
1. Rocket transmits packet (10Hz)
   ↓
2. RFD900x receives LoRa signal
   ↓
3. RFD900x → Radio Pico UART (107 bytes)
   ↓
4. Radio Pico Core 0:
   - Parses packet
   - Validates sync word
   - Extracts GPS data
   ↓
5. Radio Pico Core 0 → Stepper Pico (12 bytes via UART)
   ↓
6. Stepper Pico Core 1:
   - Calculates azimuth/elevation
   - Updates target angles
   ↓
7. Stepper Pico Core 0:
   - Generates motor steps via PIO
   - Moves antenna smoothly
   ↓
8. Radio Pico Core 1 (parallel):
   - Publishes to MQTT
   - Logs to SD card
   ↓
9. Ground Station:
    - ...
```

### Link Loss Data Flow

```
1. Radio Pico detects no packets for >500ms(TBD)
   ↓
2. Radio Pico Core 1:
   - Triggers prediction mode
   - Calculates ballistic trajectory from last known state
   - Continues sending predicted positions to Stepper Pico
   ↓
3. Stepper Pico:
   - Continues tracking predicted position
   - Logs prediction mode to SD
   ↓
4. Whenn link restored:
   - Resume normal operation
   - Log link restoration event
```

---
