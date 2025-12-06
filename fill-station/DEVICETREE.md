# Fill Station Device Tree Integration

## What Was Done

Integrated TI Sysconfig-generated pin configuration into the Fill Station build system.

### Changes Made

1. **Created Device Tree Overlay** (`nix/overlays/by-name/crt/custom-linux/am642-sk-crt.dtso`)
   - Converted TI Sysconfig pin mux definitions to device tree overlay format
   - Enables: PWM (EHRPWM4), GPIO (main_gpio0, main_gpio1), I2C2 (senseboard)
   
2. **Modified Kernel Build** (`nix/overlays/by-name/crt/custom-linux/package.nix`)
   - Added `dtc` (device tree compiler) as build dependency
   - Copies overlay into kernel source tree during build
   - Compiles overlay to `.dtbo` (device tree blob overlay)
   
3. **Modified FIT Image Build** (`nix/mixos-configurations/fill-station/fit/`)
   - Added support for applying device tree overlays
   - Uses `fdtoverlay` to merge overlay with base device tree
   - Produces final device tree with custom pin configurations

## Pin Configuration Summary

**PWM:**
- EHRPWM4_A on GP

MC0_OEn_REn (R18)

**GPIO Bank 0:**
- GPIO0_32-42: Various GPMC pins configured as outputs/inputs with pull-downs/pull-ups

**GPIO Bank 1:**
- GPIO1_42-51: SPI pins repurposed as GPIO (inputs with pull-downs, some outputs)
- GPIO1_62-65: MCAN1 and I2C0 pins repurposed as GPIO

**I2C:**
- I2C2 on GPMC0_CSn2/CSn3 (P19/R21) at 400kHz for senseboard communication

## Building

The device tree overlay is automatically included when building the SD image:

```bash
# Full system build (takes 20-30 minutes due to kernel compilation)
nix build .#mixosConfigurations.fill-station.config.system.build.sdImage

# Or build just the FIT image (faster if kernel is cached)
nix build .#mixosConfigurations.fill-station.config.system.build.fitImage
```

## Testing/Verification

After flashing to the board, verify pins are configured:
```bash
# Check GPIO exports
ls /sys/class/gpio/

# Check I2C bus 2 exists
ls /sys/bus/i2c/devices/i2c-2/

# Check PWM
ls /sys/class/pwm/
```

## Modifying Pin Configuration

To change pin assignments:

1. Edit `nix/overlays/by-name/crt/custom-linux/am642-sk-crt.dtso`
2. Pin format: `<offset mode>`
   - offset: Pin offset from TI Sysconfig (e.g., `0x0088`)
   - mode: Encoded as `0xMMMF` where:
     - `F` = function/mux selection (0-7)
     - `MMM` = pin configuration bits:
       - `0x000` = output (default)
       - `0x400` = input with pull-down
       - `0x100` = output with pull-up
3. Test syntax: `nix-shell -p dtc --run "dtc -@ -I dts -O dtb -o /tmp/test.dtbo nix/overlays/by-name/crt/custom-linux/am642-sk-crt.dtso"`
4. Rebuild kernel/image

## References

- TI AM64x Technical Reference Manual for pin assignments
- Device tree overlay documentation: https://www.kernel.org/doc/Documentation/devicetree/overlay-notes.txt
- K3 pinctrl bindings: `linux/Documentation/devicetree/bindings/pinctrl/ti,k3-pinctrl.yaml`
