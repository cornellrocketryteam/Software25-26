# Fill Station Documentation

Complete documentation for the Cornell Rocketry Team fill station server.

## Getting Started

- **[Main README](../README.md)** - Project overview, quick start, architecture
- **[QUICKSTART_ADC.md](QUICKSTART_ADC.md)** - Quick guide to testing ADC hardware
- **[TROUBLESHOOTING.md](TROUBLESHOOTING.md)** - Common problems and solutions

## Developer Guides

- **[ADDING_FEATURES.md](ADDING_FEATURES.md)** - 🌟 **Start Here!** Complete guide to extending the fill station
  - Adding new hardware components
  - Creating WebSocket commands
  - Implementing background tasks
  - Step-by-step valve controller example

## System Documentation

- **[LINUX_IMAGE_BUILD_PROCESS.md](LINUX_IMAGE_BUILD_PROCESS.md)** - Complete guide to the Linux image build system
  - Nix build architecture
  - FIT image creation
  - U-Boot bootloader configuration
  - Device tree management
  - SD card image assembly

## Feature Documentation

- **[ADC_STREAMING.md](ADC_STREAMING.md)** - ADC background monitoring and WebSocket streaming
  - Configuration guide
  - Protocol specification
  - Performance notes
  - Testing instructions

- **[UMBILICAL.md](UMBILICAL.md)** - Ground-FSW USB umbilical connection
  - Architecture and Data Flow
  - Telemetry formats
  - Command translations

- **[QD_STEPPER.md](QD_STEPPER.md)** - QD Stepper Motor (Quick Disconnect)
  - ISD02 driver configuration
  - PWM + GPIO control architecture
  - Calibration procedure
  - WebSocket commands

- **[CSV_LOGGING.md](CSV_LOGGING.md)** - Automatic CSV data logger
  - Data column formats
  - File generation behaviors
  - Umbilical data inclusion

- **[WEBSOCKET_API.md](WEBSOCKET_API.md)** - Full reference of all network commands
  - Command JSON formats
  - Response structures
  - Field descriptions

- **[ADC_MONITOR_GUIDE.md](ADC_MONITOR_GUIDE.md)** - Hardware setup for ADS1015 ADCs
  - Wiring diagrams
  - I2C configuration
  - Calibration procedures

- **[DTBO_BUILDER.md](DTBO_BUILDER.md)** - Device Tree Overlay automated builder
  - Automated SysConfig to DTBO conversion
  - How to update pinmux configuration
  - Integrated into Nix build system

- **[WIFI_SETUP.md](WIFI_SETUP.md)** - Wi-Fi Configuration Guide
  - Hardware setup (TI WL1837MOD)
  - Automatic connection configuration
  - Troubleshooting tips

## Quick Reference

### Common Tasks

| Task | Documentation |
|------|---------------|
| Add a new sensor/actuator | [ADDING_FEATURES.md](ADDING_FEATURES.md#adding-a-new-hardware-component) |
| Add a WebSocket command | [ADDING_FEATURES.md](ADDING_FEATURES.md#adding-a-new-websocket-command) |
| Create a background task | [ADDING_FEATURES.md](ADDING_FEATURES.md#adding-background-tasks) |
| View CSV logs | Logs are saved to `/tmp/data` (Linux) or `logs/` (macOS/Windows) |
| Configure ADC sampling | [ADC_STREAMING.md](ADC_STREAMING.md#configuration-easy-to-modify) |
| Test ADC readings | [QUICKSTART_ADC.md](QUICKSTART_ADC.md) |
| Fix I2C permissions | [TROUBLESHOOTING.md](TROUBLESHOOTING.md) |
| Stream ADC data | [ADC_STREAMING.md](ADC_STREAMING.md#websocket-protocol) |

### File Structure

```
fill-station/
├── README.md                    # Main project README
├── docs/
│   ├── INDEX.md                 # This file
│   ├── ADDING_FEATURES.md       # Developer guide (start here!)
│   ├── ADC_STREAMING.md         # ADC feature documentation
│   ├── ADC_MONITOR_GUIDE.md     # Hardware setup guide
│   ├── QUICKSTART_ADC.md        # Quick testing guide
│   ├── QD_STEPPER.md            # QD stepper motor docs
│   ├── TROUBLESHOOTING.md       # Problem solving
│   └── UMBILICAL.md             # Umbilical connection details
├── src/
│   ├── main.rs                  # Server & background tasks
│   ├── command.rs               # WebSocket protocol
│   ├── hardware.rs              # Hardware initialization
│   └── components/              # Hardware drivers
│       ├── igniter.rs
│       ├── solenoid_valve.rs
│       ├── mav.rs
│       ├── ball_valve.rs
│       ├── qd_stepper.rs
│       ├── ads1015.rs
│       └── umbilical.rs
└── test_adc_stream.py           # Test client
```

## Component Examples

### Existing Components

- **Igniter** (`src/components/igniter.rs`)
  - GPIO-based control
  - Continuity checking
  - Timed fire sequence

- **ADS1015** (`src/components/ads1015.rs`)
  - I2C 12-bit ADC driver
  - Configurable gain and data rate
  - Platform-aware (Linux/non-Linux)

- **SolenoidValve** (`src/components/solenoid_valve.rs`)
  - GPIO-based control (Control + Signal lines)
  - Configurable Line Pull (NO/NC)
  - SV1 through SV5 configured by default

- **MAV** (`src/components/mav.rs`)
  - PWM-based servo control
  - Limits and neutral position handling

- **Ball Valve** (`src/components/ball_valve.rs`)
  - Two-pin GPIO control (Signal + ON_OFF)
  - Timed sequencing for open/close operations

- **QD Stepper** (`src/components/qd_stepper.rs`)
  - PWM sysfs for STEP signal + GPIO for DIR/ENA
  - ISD02 integrated stepper driver (NEMA 17)
  - Background task execution (non-blocking moves)

### Example Implementations

See [ADDING_FEATURES.md](ADDING_FEATURES.md#example-adding-a-valve-controller) for complete valve controller example including:
- Component implementation
- Hardware integration
- WebSocket commands
- Test client code

## Architecture Diagrams

### Data Flow
```
WebSocket Client
    ↓ JSON command
main.rs (WebSocket server)
    ↓ deserialize
command.rs (Command enum)
    ↓ route
execute_command()
    ↓ lock & access
Hardware (Arc<Mutex<>>)
    ↓ method call
Component (igniter, ADC, valve, etc.)
    ↓ hardware I/O
GPIO / I2C / SPI
    ↓ serialize
CommandResponse
    ↓ JSON response
WebSocket Client
```

### Background Tasks
```
main() spawns tasks:
    ├─ adc_monitoring_task (100 Hz)
    │   └─ Updates AdcReadings state
    │       └─ Streamed to clients
    │
    ├─ umbilical_task (continuous serial poll)
    │   └─ Updates UmbilicalReadings state
    │       └─ Streamed to clients
    │
    ├─ csv_logging_task (100 Hz)
    │   └─ Writes ADC and Hardware state to CSV file
    │
    └─ your_background_task
        └─ Updates YourState
            └─ Queried by commands
```

## WebSocket Protocol Summary

### Command Format
```json
{
  "command": "command_name",
  "param1": "value",
  "param2": 123
}
```

### Response Format
```json
{
  "type": "response_type",
  "field1": "value",
  "field2": 456
}
```

### Available Commands

| Command | Description | Documentation |
|---------|-------------|---------------|
| `ignite` | Fire both igniters concurrently (3s) | Built-in |
| `start_adc_stream` | Begin ADC data stream | [ADC_STREAMING.md](ADC_STREAMING.md#start-adc-streaming) |
| `stop_adc_stream` | End ADC data stream | [ADC_STREAMING.md](ADC_STREAMING.md#stop-adc-streaming) |
| `actuate_valve` | Open/Close solenoid valve | [WEBSOCKET_API.md](WEBSOCKET_API.md#actuate_valve) |
| `get_valve_state` | Query valve state | [WEBSOCKET_API.md](WEBSOCKET_API.md#get_valve_state) |
| `set_mav_angle` | Set MAV servo angle | [WEBSOCKET_API.md](WEBSOCKET_API.md#set_mav_angle) |
| `mav_open` / `mav_close` | Open or Close MAV fully | [WEBSOCKET_API.md](WEBSOCKET_API.md#mav_open) |
| `bv_open` / `bv_close` | Open or Close Ball Valve | [WEBSOCKET_API.md](WEBSOCKET_API.md#bv_open) |
| `qd_move` | Move QD stepper N steps | [QD_STEPPER.md](QD_STEPPER.md) |
| `qd_open` / `qd_close` | Open or Close QD (preset) | [QD_STEPPER.md](QD_STEPPER.md) |
| `start_fsw_stream` | Stream FSW telemetry | [UMBILICAL.md](UMBILICAL.md) |
| `fsw_launch` | Send Launch command to FSW | [UMBILICAL.md](UMBILICAL.md) |

## Configuration Reference

### ADC Settings (`src/main.rs`)
```rust
const ADC_SAMPLE_RATE_HZ: u64 = 100;          // Sampling frequency
const ADC_GAIN: Gain = Gain::One;             // ±4.096V range
const ADC_DATA_RATE: DataRate = DataRate::Sps3300;  // Max speed (optimized)
const ADC_MAX_RETRIES: u32 = 5;               // Retry attempts
const ADC_AVG_SAMPLES: usize = 10;            // 10x averaging
const ADC_RETRY_DELAY_MS: u64 = 10;           // Retry delay
```

### Pressure Sensor Calibration (`src/main.rs`)
```rust
const PT1500_SCALE: f32 = 0.909754;      // PT1500 Scale (ADC1 Ch0)
const PT1500_OFFSET: f32 = 5.08926;      // PT1500 Offset

const PT2000_SCALE: f32 = 1.22124;       // PT2000 Scale (Other PTs)
const PT2000_OFFSET: f32 = 5.37052;      // PT2000 Offset

const LOADCELL_SCALE: f32 = 1.69661;     // LoadCell Scale (ADC2 Ch1)
const LOADCELL_OFFSET: f32 = 75.37882;   // LoadCell Offset
```

### Hardware Pins (`src/hardware.rs`)
```rust
const GPIO_CHIP0: &str = "gpiochip1";
const GPIO_CHIP1: &str = "gpiochip2";
const I2C_BUS: &str = "/dev/i2c-2";
const ADC1_ADDRESS: u16 = 0x48;
const ADC2_ADDRESS: u16 = 0x49;
// Igniter pins: 38, 39, 40, 42 (across chips)
// Valve pins (Actuate / Sense):
//   SV1: C0-42 / C1-51 (NC)
//   SV2: C0-32 / C0-34 (NC)
//   SV3: C1-44 / C0-37 (NC)
//   SV4: C1-65 / C0-36 (NC)
//   SV5: C1-48 / C1-46 (NO)
// MAV: PWM Chip 0, Channel 0
```

## Testing Tools

### Python WebSocket Client
```bash
./test_adc_stream.py
```

### Rust Test Binaries
```bash
cargo run --bin adc_test          # I2C diagnostic
cargo run --bin adc_monitor       # Calibrated monitor
cargo run --bin dual_adc_monitor  # Raw monitor
```

### Manual Testing
```bash
# Using websocat
websocat ws://localhost:9000

# Using Python one-liner
python3 -c "import asyncio, websockets, json; asyncio.run(websockets.connect('ws://localhost:9000'))"
```

## Best Practices

1. **Platform Awareness**: Always use `#[cfg]` for Linux-only code
2. **Error Handling**: Return `Result<>`, don't panic
3. **Logging**: Use `info!`, `warn!`, `error!` liberally
4. **Thread Safety**: Use `Arc<Mutex<>>` for shared state
5. **Documentation**: Update docs when adding features
6. **Testing**: Test on macOS (dev) and Linux (target)

## Getting Help

### Common Issues
Check [TROUBLESHOOTING.md](TROUBLESHOOTING.md) first.

### Adding Features
Follow the complete guide in [ADDING_FEATURES.md](ADDING_FEATURES.md).

### ADC Problems
See [ADC_MONITOR_GUIDE.md](ADC_MONITOR_GUIDE.md) and [QUICKSTART_ADC.md](QUICKSTART_ADC.md).

### Protocol Questions
Reference [ADC_STREAMING.md](ADC_STREAMING.md#websocket-protocol) for protocol details.

## Contributing

When adding documentation:
1. Update this INDEX.md with links to new docs
2. Add quick reference entries for common tasks
3. Update the main README.md if it affects getting started
4. Keep examples up-to-date with code changes

---

**Last Updated**: March 8, 2026 (Added QD Stepper motor component and documentation)  
**Maintained By**: Cornell Rocketry Team Software Team
