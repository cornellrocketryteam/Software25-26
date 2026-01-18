# Fill Station Server

Ground-side service for Cornell Rocketry Team's rocket fill operations. This Rust application provides a WebSocket-based control interface for hardware components including igniters, ADCs, valves, and sensors.

## Quick Start

### Development (macOS/Linux)
```bash
cargo run --release
```

### Production (Nix Build)
```bash
# Build the entire system image including fill-station
cd /path/to/Software25-26
nix build .#mixosConfigurations.fill-station.config.system.build.sdImage
```

The server listens on `ws://0.0.0.0:9000` for WebSocket connections. The service starts **automatically on boot**.

## Architecture

```
fill-station/
├── src/
│   ├── main.rs              # WebSocket server & background tasks
│   ├── command.rs           # Command/response protocol definitions
│   ├── hardware.rs          # Hardware initialization & aggregation
│   ├── lib.rs               # Public API exports
│   └── components/          # Individual hardware drivers
│       ├── igniter.rs       # GPIO-based igniter control
│       ├── ads1015.rs       # I2C ADC driver (pressure sensors)
│       └── mod.rs           # Component exports
├── docs/                    # Documentation
│   ├── ADDING_FEATURES.md   # Guide to extending the system
│   ├── ADC_STREAMING.md     # ADC background monitoring docs
│   ├── ADC_MONITOR_GUIDE.md # ADC hardware setup
│   ├── QUICKSTART_ADC.md    # Quick ADC testing guide
│   └── TROUBLESHOOTING.md   # Common issues & solutions
└── test_adc_stream.py       # WebSocket client test script
```

## Features

### ✅ WebSocket Server
- Async WebSocket server using `smol` runtime
- JSON-based command/response protocol
- Multiple concurrent client support
- Robust error handling (doesn't crash on client errors)

### ✅ Hardware Control
- **Igniters**: GPIO-based control with continuity checking and concurrent firing
- **Solenoid Valves**: 5x GPIO control (SV1-SV5) with NO/NC logic
- **MAV**: Servo control (PWM) for Mechanically Actuated Valve
- **ADC Monitoring**: Dual ADS1015 12-bit ADCs (8 channels total)
- **Pressure Sensors**: Calibrated scaling for ADC channels
- Platform-aware: Compiles on macOS for dev, runs on Linux

### ✅ Background Tasks
- **ADC Monitoring**: Continuous 10 Hz sampling with retry logic
- **Streaming**: Real-time ADC data pushed to WebSocket clients
- Thread-safe shared state using `Arc<Mutex<>>`

### ✅ Logging
- Dual output: stdout + rotating file logs
- Structured logging with `tracing` crate
- Per-connection span tracking

## WebSocket Commands

### Igniter Control
```json
{"command": "ignite"}
```
*Note: This command fires both Igniter 1 and Igniter 2 concurrently for 3 seconds.*

### Solenoid Valve Control
```json
{"command": "actuate_valve", "valve": "SV1", "state": true}
{"command": "get_valve_state", "valve": "SV1"}
```

### ADC Streaming
```json
{"command": "start_adc_stream"}
{"command": "stop_adc_stream"}
```

### MAV Control
```json
{"command": "set_mav_angle", "valve": "MAV", "angle": 45.0}
{"command": "mav_open", "valve": "MAV"}
```

See [`docs/ADC_STREAMING.md`](docs/ADC_STREAMING.md) for detailed protocol specification.

## Hardware Configuration

### ADC Settings
- **I2C Bus**: `/dev/i2c-2`
- **ADC1 Address**: `0x48`
- **ADC2 Address**: `0x49`
- **Gain**: ±4.096V (configurable)
- **Sample Rate**: 10 Hz (configurable)

### GPIO Pins
- **Igniter 1**: GPIO Chip 0, Pin 38 (signal), Pin 39 (continuity)
- **Igniter 1**: GPIO Chip 0, Pin 38 (signal), Pin 39 (continuity)
- **Igniter 2**: GPIO Chip 0, Pin 40 (signal), GPIO Chip 1, Pin 42 (continuity)
- **Valves**:
  - **SV1**: Actuate (Chip 0, 42), Sense (Chip 1, 51) - NC
  - **SV2**: Actuate (Chip 0, 32), Sense (Chip 0, 34) - NC
  - **SV3**: Actuate (Chip 1, 44), Sense (Chip 0, 37) - NC
  - **SV4**: Actuate (Chip 1, 65), Sense (Chip 0, 36) - NC
  - **SV5**: Actuate (Chip 1, 48), Sense (Chip 1, 46) - NO
- **MAV**: PWM Chip 0, Channel 0 (330 Hz)
- **Ball Valve**:
  - **Signal**: Chip 1, Line 62
  - **ON_OFF**: Chip 1, Line 63

See [`src/hardware.rs`](src/hardware.rs) for pin mappings.

## Configuration

All configuration constants are at the top of `src/main.rs`:

```rust
// ADC sampling rate
const ADC_SAMPLE_RATE_HZ: u64 = 10;  // Change to 20, 50, 100...

// Pressure sensor calibration
// Pressure sensor scaling for PT1500
const PT1500_SCALE: f32 = 0.909754;
const PT1500_OFFSET: f32 = 5.08926;

// Pressure sensor scaling for PT2000
const PT2000_SCALE: f32 = 1.22124;
const PT2000_OFFSET: f32 = 5.37052;

// Load Cell scaling
const LOADCELL_SCALE: f32 = 1.69661;
const LOADCELL_OFFSET: f32 = 75.37882;
```

Easy to modify without diving into code logic.

## Safety Features

### Connection Monitoring
The system implements a **deadman switch** safety feature:
- If a client is connected but sends no messages for **15 seconds** (connection timeout):
  - All Solenoid Valves (SV1-SV5) are closed.
  - The MAV is closed.
  - The client is disconnected.
- Clients should send a `{"command": "heartbeat"}` message periodically (e.g., every 5-10 seconds) if they are not sending other commands.


## Testing

### Test ADC Streaming
```bash
./test_adc_stream.py
```

Or use `websocat`:
```bash
websocat ws://localhost:9000
# Then type commands:
{"command": "start_adc_stream"}
```

### Unit ADC Testing
```bash
cargo run --bin adc_test       # Test I2C communication
cargo run --bin adc_monitor    # Monitor with scaling
cargo run --bin dual_adc_monitor  # Basic dual ADC monitor
```

## Development

### Adding New Features

See the comprehensive guide: **[`docs/ADDING_FEATURES.md`](docs/ADDING_FEATURES.md)**

This covers:
- Creating new hardware components
- Adding WebSocket commands
- Implementing background tasks
- Integration with main server
- Complete valve controller example

### Adding New Hardware Component

1. Create driver in `src/components/your_component.rs`
2. Export in `src/components/mod.rs`
3. Add to `Hardware` struct in `src/hardware.rs`
4. Add commands to `src/command.rs`
5. Handle in `execute_command()` in `src/main.rs`

Detailed walkthrough in [`docs/ADDING_FEATURES.md`](docs/ADDING_FEATURES.md).

### Dependencies

Core:
- `smol` - Async runtime
- `async-tungstenite` - WebSocket server
- `serde` / `serde_json` - JSON serialization
- `tracing` - Structured logging
- `anyhow` - Error handling

Hardware (Linux only):
- `async-gpiod` - GPIO control
- `i2cdev` - I2C communication

## Building

### Local Development
```bash
cargo build --release
```

### Nix-based Build
The fill station is built as a Nix package defined in:
```
nix/overlays/by-name/crt/fill-station/package.nix
```

Build the package alone:
```bash
nix build .#crt.fill-station
```

Build full system SD card image:
```bash
nix build .#mixosConfigurations.fill-station.config.system.build.sdImage
```

The SD image is at `./result/fill-station.img` and can be flashed directly to an SD card.

## Deployment

The fill-station runs on the **TI AM64x SK board** with a custom MixOS-based Linux system.

**Init System**: MixOS uses a minimal init system (not systemd). The service is configured in:
```
nix/mixos-configurations/fill-station/default.nix
```

**Service Configuration**:
```nix
init = {
  fill-station = {
    action = "once";
    process = lib.getExe pkgs.crt.fill-station;
  };
};
```

**Logs**:
- **stdout**: Visible if running manually.
- **File**: Rotating logs in `/tmp/fill-station/logs/` (Linux) or `logs/` (macOS).

**SD Card Image**: The complete bootable system is built with:
```bash
nix build .#mixosConfigurations.fill-station.config.system.build.sdImage
```

See [LINUX_IMAGE_BUILD_PROCESS.md](docs/LINUX_IMAGE_BUILD_PROCESS.md) for complete build system documentation.

## Troubleshooting

See [`docs/TROUBLESHOOTING.md`](docs/TROUBLESHOOTING.md) for:
- I2C permission issues
- GPIO access problems
- WebSocket connection errors
- ADC reading issues

## Documentation

- **[INDEX.md](docs/INDEX.md)** - Documentation hub with navigation
- **[ADDING_FEATURES.md](docs/ADDING_FEATURES.md)** - Complete guide to extending the system
- **[ADC_STREAMING.md](docs/ADC_STREAMING.md)** - ADC background monitoring & streaming
- **[ADC_MONITOR_GUIDE.md](docs/ADC_MONITOR_GUIDE.md)** - ADC hardware setup
- **[QUICKSTART_ADC.md](docs/QUICKSTART_ADC.md)** - Quick ADC testing
- **[TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md)** - Common issues & fixes
- **[DTBO_BUILDER.md](docs/DTBO_BUILDER.md)** - Device tree overlay automation
- **[LINUX_IMAGE_BUILD_PROCESS.md](docs/LINUX_IMAGE_BUILD_PROCESS.md)** - Complete build system guide

## License

Cornell Rocketry Team - Internal Use

## Contributing

When adding features:
1. Follow the patterns in existing components
2. Use `#[cfg]` for platform-specific code
3. Add comprehensive error handling
4. Document your commands and protocol
5. Test on both macOS (dev) and Linux (target)
6. Update this README if adding major features

See [`docs/ADDING_FEATURES.md`](docs/ADDING_FEATURES.md) for detailed contribution guidelines.
