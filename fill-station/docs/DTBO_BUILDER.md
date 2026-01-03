# Fill Station Device Tree Overlay Builder

This package automates the conversion of TI SysConfig pinmux output to a device tree blob overlay (`.dtbo`).

## How It Works

The build process automatically:
1. Preprocesses the overlay with clang to expand macros
2. Folds arithmetic expressions into literal constants with Python
3. Compiles the overlay with dtc
4. Verifies the overlay has required metadata

## Updating the SysConfig

To update the pinmux configuration:

1. Export your pinmux configuration from TI SysConfig
2. Replace `src/sysconfig-pinmux.dtsi` with the new output
3. If needed, update `src/k3-am64-fillstation-pinmux-overlay.dts` to attach pinctrl to the correct device nodes
4. Rebuild: `nix build .#mixosConfigurations.fill-station.config.system.build.sdImage`

## Files

- `src/sysconfig-pinmux.dtsi` - Raw SysConfig output (UPDATE THIS)
- `src/k3.h` - Pinctrl macro definitions (don't modify)
- `src/k3-am64-fillstation-pinmux-overlay.dts` - Wrapper overlay that includes the dtsi and attaches to devices - modify as needed 
- `fold-expressions.py` - Python script to fold expressions
- `package.nix` - Nix build definition

## Output

The package produces: `k3-am64-fillstation-pinmux-overlay.dtbo`

This is automatically merged into the base DTB during FIT image build.