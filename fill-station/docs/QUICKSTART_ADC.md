# Quick Start: Running ADC Monitor on TI AM64x SK

## 🚀 Fastest Path to Running

### 1. Build the SD Card Image
```bash
cd /path/to/Software25-26
nix build .#mixosConfigurations.fill-station.config.system.build.sdImage
```

### 2. Flash to SD Card
```bash
# Find your SD card
lsblk

# Flash (replace /dev/sdX with your SD card device)
sudo dd if=./result/fill-station.img of=/dev/sdX bs=4M status=progress && sync
```

### 3. Boot and Connect
- Insert SD card into TI AM64x SK board
- Connect serial console (115200 baud, ttyS2)
- Power on the board
- Wait for boot (login appears on serial console)

### 4. Run the ADC Monitor
```bash
# At the shell prompt:
adc_monitor
```

That's it! The monitor will continuously display readings from both ADCs.

---

## 📊 What You'll See

```
Sample │ Throughput  │ ADC1 (0x48) Channels      │ ADC2 (0x49) Channels
────────────────────────────────────────────────────────────────────
   123 │   800.0 SPS │ Ch0, Ch1, Ch2, Ch3        │ Ch0, Ch1, Ch2, Ch3
```

Real-time voltage and raw ADC values from all 8 channels (4 per ADC).

---

## 🔧 Where the Code Lives

**Source Files:**
- **ADC Driver**: `fill-station/src/components/ads1015.rs`
- **Monitor Binary**: `fill-station/src/bin/adc_monitor.rs`
- **Hardware Integration**: `fill-station/src/hardware.rs`

**Key Configuration** (in `adc_monitor.rs`):
```rust
const I2C_BUS: &str = "/dev/i2c-2";
const ADC1_ADDR: u16 = 0x48;
const ADC2_ADDR: u16 = 0x49;
```

---

## 🎯 How It Gets Into the SD Image

The Nix build system automatically includes the ADC monitor:

1. **Package Definition**: `nix/overlays/by-name/crt/fill-station/package.nix`
   - Builds all Rust binaries in the fill-station crate
   - Includes: `fill-station`, `adc_monitor`, and any other `src/bin/*.rs` files

2. **System Configuration**: `nix/mixos-configurations/fill-station/default.nix`
   ```nix
   bin = [
     pkgs.crt.fill-station    # This includes ALL binaries!
     # ...
   ];
   ```

3. **Result**: All binaries end up in `/bin/` on the final system
   - `/bin/fill-station` - Main service
   - `/bin/adc_monitor` - ADC monitoring tool
   - `/bin/dual_adc_monitor` - Alternative monitor

---

## 🔍 Verifying I2C Devices

Before running the monitor, verify your ADCs are connected:

```bash
# List I2C buses
ls -l /dev/i2c-*

# Scan bus 2 for devices
i2cdetect -y 2
```

You should see devices at addresses `48` and `49`.

---

## 📝 Full Documentation

See [`ADC_MONITOR_GUIDE.md`](./ADC_MONITOR_GUIDE.md) for:
- Detailed hardware connections
- Troubleshooting steps
- Integration with main fill-station service
- Performance tuning options
- Cross-compilation instructions

---

## 🏗️ Adding More Binaries in the Future

To add a new executable that will automatically be included in the SD image:

1. **Create the binary**: `fill-station/src/bin/my_tool.rs`
2. **Rebuild**: `nix build .#mixosConfigurations.fill-station.config.system.build.sdImage`
3. **Flash and run**: `/bin/my_tool`

No changes to Nix configuration needed - all `src/bin/*.rs` files are automatically built and included!

---

## 💡 Pro Tips

- **Edit on the fly**: Modify the binary source, rebuild with Nix, reflash SD card
- **Quick iteration**: Use `cargo build --release --bin adc_monitor` for faster local builds
- **Cross-compile**: Target `aarch64-unknown-linux-musl` and copy via SSH for faster testing
- **Integration**: The ADCs are already in `Hardware` struct - ready to use in main service!
