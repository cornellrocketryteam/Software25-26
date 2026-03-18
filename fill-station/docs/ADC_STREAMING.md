# ADC Streaming Implementation

## Overview

The fill station now includes a background task that continuously monitors two ADS1015 ADC chips (8 total channels) and streams the readings to connected WebSocket clients.

## Architecture

### Background ADC Monitoring Task
- Runs continuously at **100 Hz** (configurable via `ADC_SAMPLE_RATE_HZ`)
- Reads all 8 channels (4 per ADC) every cycle
- Updates shared `AdcReadings` struct with thread-safe `Arc<Mutex<>>`
- Includes retry logic: up to 5 attempts with 10ms delays between failures
- Marks readings as invalid if all retries fail

### WebSocket Streaming
- Clients connect and send `{"command": "start_adc_stream"}` to begin receiving data
- Server pushes new ADC readings as they become available (~100 Hz)
- Clients send `{"command": "stop_adc_stream"}` to stop the stream
- Multiple clients can stream simultaneously (each gets their own stream)

## Configuration (Easy to Modify)

All ADC parameters are defined as constants at the top of `main.rs`:

```rust
// Sampling rate
const ADC_SAMPLE_RATE_HZ: u64 = 100;  // Change to 20, 50, 100, etc.

// ADC settings (Linux/Android only)
const ADC_GAIN: Gain = Gain::One;              // ±4.096V range
const ADC_DATA_RATE: DataRate = DataRate::Sps3300;  // Maximum speed

// Retry behavior
const ADC_MAX_RETRIES: u32 = 5;
const ADC_RETRY_DELAY_MS: u64 = 10;

// PT1 scaling (ADC1 Ch0) — 0-1500 PSI
const PT1500_SCALE: f32 = 0.909754;
const PT1500_OFFSET: f32 = 5.08926;

// PT2 scaling (ADC1 Ch1) — 0-1000 PSI
const PT1000_SCALE: f32 = 0.6125;
const PT1000_OFFSET: f32 = 5.0;

// Load Cell scaling (ADC2 Ch1)
const LOADCELL_SCALE: f32 = 0.264;
const LOADCELL_OFFSET: f32 = -14.9;
```

### To Change Sampling Rate:
Just modify `ADC_SAMPLE_RATE_HZ`. Example: `const ADC_SAMPLE_RATE_HZ: u64 = 200;` for 200 Hz.

### To Update Pressure Sensor Calibration:
Modify the `PT1500_*`, `PT2000_*`, and `LOADCELL_*` constants with your new calibration values.

## WebSocket Protocol

### Commands

#### Start ADC Streaming
```json
{
  "command": "start_adc_stream"
}
```

**Response:**
```json
{
  "type": "success"
}
```

#### Stop ADC Streaming
```json
{
  "command": "stop_adc_stream"
}
```

**Response:**
```json
{
  "type": "success"
}
```

### ADC Data Format

When streaming is enabled, the server continuously sends:

```json
{
  "type": "adc_data",
  "timestamp_ms": 1734678123456,
  "valid": true,
  "adc1": [
    {
      "raw": 1234,
      "voltage": 2.468,
      "scaled": 4.876  // Only for channels with pressure sensor scaling
    },
    // ... 3 more channels
  ],
  "adc2": [
    {
      "raw": 567,
      "voltage": 1.134,
      "scaled": null  // No scaling for ADC2 channels
    },
    // ... 3 more channels
  ]
}
```

### Field Descriptions

- `timestamp_ms`: Unix timestamp in milliseconds when readings were taken
- `valid`: `true` if readings are fresh, `false` if ADC read failed
- `raw`: Raw 12-bit ADC value (-2048 to 2047)
- `voltage`: Calculated voltage based on gain setting
- `scaled`: Scaled sensor value — PT1 (ADC1 Ch0), PT2 (ADC1 Ch1), Load Cell (ADC2 Ch1). `null` for all other channels

## Testing

### Run the Fill Station Server
```bash
cd fill-station
cargo run --release
```

### Test ADC Streaming (Python)
```bash
cd fill-station
./test_adc_stream.py
```

Or:
```bash
python3 test_adc_stream.py
```

The test script will:
1. Connect to `ws://localhost:9000`
2. Send `start_adc_stream` command
3. Display formatted ADC readings in real-time
4. Press Ctrl+C to stop

### Manual Testing (websocat)
```bash
# Install websocat if needed: brew install websocat

# Connect and manually send commands
websocat ws://localhost:9000

# Then type:
{"command": "start_adc_stream"}

# You'll see streaming data. To stop:
{"command": "stop_adc_stream"}
```

## Adding More Channels/Fields

The `AdcReadings` struct is designed to be easily extensible:

```rust
pub struct AdcReadings {
    pub timestamp_ms: u64,
    pub valid: bool,
    pub adc1: [ChannelReading; 4],
    pub adc2: [ChannelReading; 4],
    // Add more fields here as needed, e.g.:
    // pub temperature: f32,
    // pub humidity: f32,
}
```

Just update:
1. The struct definition
2. The background task to populate new fields
3. The JSON response will automatically include them

## Error Handling

- **ADC I2C failures**: Retries up to 5 times with 10ms delays
- **All retries fail**: Marks readings as `valid: false`, logs error with timestamp
- **WebSocket errors**: Connection closed gracefully, doesn't crash server
- **Platform incompatibility**: ADC monitoring disabled on non-Linux platforms (logs warning)

## Performance Notes

- At 100 Hz with 8 channels: 800 reads/second total
- Actual I2C speed limited by `Sps3300` setting (~3300 samples/sec per channel max)
- WebSocket JSON serialization is negligible overhead
- Each reading is ~500 bytes JSON, so 100 Hz = ~50 KB/s per client
