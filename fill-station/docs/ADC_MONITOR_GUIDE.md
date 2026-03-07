# ADS1015 ADC Monitor - Usage Guide

## Overview

The `adc_monitor` executable continuously reads from two ADS1015 12-bit ADCs connected via I2C bus 2 on the TI AM64x SK board.

**Hardware Configuration:**
- **I2C Bus**: `/dev/i2c-2`
- **ADC1 Address**: `0x48`
- **ADC2 Address**: `0x49`
- **Total Channels**: 8 (4 per ADC)
- **Voltage Range**: ±4.096V
- **Sample Rate**: Up to 3300 SPS per channel

---

## Building and Deploying to TI AM64x SK Board

### Option 1: Build SD Card Image with Nix (Recommended)

The fill-station binary and all utilities are automatically included in the SD card image when you build with Nix.

```bash
# From the repository root
nix build .#mixosConfigurations.fill-station.config.system.build.sdImage
```

**Result:** 
- SD card image at `./result/fill-station.img`
- Contains the entire MixOS system with fill-station binaries

**Flash to SD card:**
```bash
# Find your SD card device (e.g., /dev/sdb, /dev/mmcblk0)
lsblk

# Flash the image (CAUTION: This will erase the SD card!)
sudo dd if=./result/fill-station.img of=/dev/sdX bs=4M status=progress
sudo sync
```

### Option 2: Cross-Compile and Copy to Existing System

If you already have a running system on the TI board:

```bash
# Cross-compile for ARM64
cd fill-station
cargo build --release --target aarch64-unknown-linux-musl

# Copy to the board (via SSH or SD card)
scp target/aarch64-unknown-linux-musl/release/adc_monitor root@<board-ip>:/usr/local/bin/
```

---

## Running on the TI Board

### 1. Boot the Board
- Insert the SD card into the TI AM64x SK board
- Power on the board
- The system will boot into MixOS

### 2. Access the Shell

**Via Serial Console:**
```bash
# Connect via USB-to-UART (typically ttyS2)
screen /dev/ttyUSB0 115200
# Or use minicom, picocom, etc.
```

**Via SSH (if network is configured):**
```bash
ssh root@<board-ip>
# Default password hash is in nix/mixos-configurations/fill-station/default.nix
```

### 3. Run the ADC Monitor

```bash
# If binaries are in /bin/ (from Nix build):
adc_monitor

# Or if you copied manually:
/usr/local/bin/adc_monitor
```

### 4. Expected Output

```
============================================
  Dual ADS1015 ADC Monitor
  Cornell Rocketry Team - Fill Station
============================================

Initializing I2C devices on /dev/i2c-2...
✓ ADC1 ready at address 0x48
✓ ADC2 ready at address 0x49

Configuration:
  • Gain: ±4.096V range (1x)
  • Sample Rate: 3300 SPS per channel
  • Channels: 4 single-ended inputs per ADC (8 total)
  • Mode: Continuous single-shot conversion

Press Ctrl+C to stop

════════════════════════════════════════════════════════════════════════════
Sample │ Throughput  │       ADC1 (0x48) - Ch0, Ch1, Ch2, Ch3              │       ADC2 (0x49) - Ch0, Ch1, Ch2, Ch3              
       │             │  Raw / Volts   │  Raw / Volts   │  Raw / Volts   │  Raw / Volts   │  Raw / Volts   │  Raw / Volts   │  Raw / Volts   │  Raw / Volts   
────────────────────────────────────────────────────────────────────────────
     1 │   800.0 SPS │      0  0.000V │      0  0.000V │      0  0.000V │      0  0.000V │      0  0.000V │      0  0.000V │      0  0.000V │      0  0.000V │
```

### 5. Stop the Monitor
Press `Ctrl+C` to exit.

---

## Troubleshooting

### I2C Bus Not Found
```bash
# Check available I2C buses
ls -l /dev/i2c-*

# If i2c-2 doesn't exist, check device tree or use i2c-1
# Edit the binary source to change I2C_BUS constant
```

### Permission Denied
```bash
# Run as root or add user to i2c group
sudo chmod 666 /dev/i2c-2
# Or permanently:
sudo usermod -a -G i2c $(whoami)
```

### ADC Not Responding
```bash
# Scan I2C bus to detect devices
i2cdetect -y 2

# Should show devices at 0x48 and 0x49:
#      0  1  2  3  4  5  6  7  8  9  a  b  c  d  e  f
# 00:          -- -- -- -- -- -- -- -- -- -- -- -- --
# ...
# 40: -- -- -- -- -- -- -- -- 48 49 -- -- -- -- -- --
```

### No i2cdetect Tool
```bash
# Install i2c-tools if not included
# Or use the fill-station's built-in diagnostics
```

---

## Integration with Fill Station Service

The ADC readings can be integrated into the main fill-station WebSocket service:

### 1. Modify `hardware.rs` to Use ADCs

The `Hardware` struct already includes both ADCs:

```rust
pub struct Hardware {
    pub ig1: Igniter,
    pub ig2: Igniter,
    pub adc1: Ads1015,  // Already included!
    pub adc2: Ads1015,  // Already included!
}
```

### 2. Add ADC Commands to `command.rs`

```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Command {
    Ignite,
    ReadAdc { adc: u8, channel: u8 },
    ReadAllAdcs,
}
```

### 3. Use in Main Service

```rust
// In execute_command()
Command::ReadAllAdcs => {
    // Read all 8 channels and return via WebSocket
}
```

---

## Additional Binaries

The fill-station crate includes other useful binaries:

- **`fill-station`** - Main WebSocket service (runs at boot)
- **`adc_monitor`** - This ADC monitoring tool
- **`dual_adc_monitor`** - Alternative ADC monitor (similar functionality)

All are available in `/bin/` on the SD card image.

---

## Performance Tuning

### Maximum Sample Rate

To achieve the highest sample rate, reduce or remove the sleep delay:

```rust
// In src/bin/adc_monitor.rs, line ~110
thread::sleep(Duration::from_millis(100));  // Reduce or comment out
```

Rebuild and redeploy to see the maximum throughput.

### Continuous Mode

The driver has been optimized with microsecond-precision sleeps to achieve higher throughput in single-shot mode.

---

## Hardware Connections

**I2C Bus 2 on TI AM64x SK:**
- Usually exposed on expansion headers
- Check the TI AM64x SK schematic for pin locations
- Typical: SCL2, SDA2 pins

**ADS1015 Wiring:**
- VDD: 3.3V or 5V (depending on your ADS1015 breakout)
- GND: Ground
- SCL: I2C Clock (to SCL2 on AM64x)
- SDA: I2C Data (to SDA2 on AM64x)
- ADDR: Set to GND (0x48) or VDD/SDA/SCL (0x49, 0x4A, 0x4B)
- A0-A3: Analog input channels

**Pull-up Resistors:** Typically 4.7kΩ on SCL and SDA (may already be on board or breakout module).

---

## See Also

- [ADS1015 Component Documentation](../src/components/ads1015.rs)
- [Hardware Abstraction](../src/hardware.rs)
- [Fill Station Main Service](../src/main.rs)
- [TI AM64x SK User Guide](https://www.ti.com/tool/SK-AM64)
