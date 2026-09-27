# Fill Station Device Tree Overlay Builder

This package automates the conversion of TI SysConfig pinmux output to a device tree blob overlay (`.dtbo`).

## How It Works

The build process automatically:
1. Preprocesses the overlay with clang to expand macros (dtc evaluates the resulting pinmux arithmetic itself)
2. Compiles the overlay with dtc
3. Verifies the overlay has required metadata

During the FIT image build the overlay is applied to the kernel's `k3-am642-sk.dtb` with `fdtoverlay`, and a few `fdtget` checks assert the result (see `nix/mixos-configurations/fill-station/fit/build-fit-image.nix`). Add a check there for each new peripheral.

## Adding pins / peripherals

For step-by-step recipes for GPIO, I2C, PWM and SPI (including the SK-AM64B PRU header pads), see [PRU_HEADER.md](PRU_HEADER.md).

## Updating the SysConfig

To update the pinmux configuration:

1. Export your pinmux configuration from TI SysConfig
2. Replace `src/sysconfig-pinmux.dtsi` with the new output. Turn off SysConfig's "not configured" pins option (or strip the `*_notconf` groups) — they're hundreds of unused groups, some covering pads we use
3. If needed, update `src/k3-am64-fillstation-pinmux-overlay.dts` to attach pinctrl to the correct device nodes
4. Rebuild: `nix build .#mixosConfigurations.fill-station.config.system.build.sdImage`

## Files
All source files are in the `nix/overlays/by-name/crt/fillstation-dtbo` directory:
- `src/sysconfig-pinmux.dtsi` - Raw SysConfig output (UPDATE THIS)
- `src/k3.h` - Pinctrl macro definitions (upstream `k3-pinctrl.h` plus the extra flag names SysConfig emits, e.g. `PIN_SCMITT_TRIGGER_*`, `PIN_DRIVE_STRENGTH_*`). If a new SysConfig export uses an undefined macro, add it here
- `src/k3-am64-fillstation-pinmux-overlay.dts` - Wrapper overlay that includes the dtsi and attaches to devices - modify as needed 
- `package.nix` - Nix build definition

## Output

The package produces: `k3-am64-fillstation-pinmux-overlay.dtbo`

This is automatically merged into the base DTB during FIT image build.