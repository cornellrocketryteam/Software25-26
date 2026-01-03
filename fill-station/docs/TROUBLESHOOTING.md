# Troubleshooting: adc_monitor not found

## Quick Fix - Try these commands on the TI board:

```bash
# 1. Check if the binary exists and where
which adc_monitor
find / -name "adc_monitor" 2>/dev/null

# 2. List all binaries in common locations
ls -la /bin/ | grep adc
ls -la /usr/bin/ | grep adc
ls -la /usr/local/bin/ | grep adc

# 3. Check what fill-station binaries are available
ls -la /bin/fill*
ls -la /bin/ | grep -E "(fill|adc)"

# 4. Try with full path if it's in a different location
/bin/adc_monitor
/usr/bin/adc_monitor
/usr/local/bin/adc_monitor
```

## Most Likely Issues:

### Issue 1: Binary wasn't included in the Nix build

The Nix package definition might only include the main binary. Check:

```bash
# On your Mac, inspect the Nix package
nix build .#packages.aarch64-linux.fill-station
ls -la result/bin/
```

If `adc_monitor` isn't there, we need to fix the package definition.

### Issue 2: PATH issue on the board

```bash
# On the TI board, check your PATH
echo $PATH

# Try running with explicit path
./adc_monitor
```

### Issue 3: Binary name changed during build

```bash
# List ALL executables on the system
ls -la /bin/
# Look for anything related to fill-station or ADC
```

## Solution: Verify the Nix package installs all binaries

✅ **GOOD NEWS**: The package definition in `nix/overlays/by-name/crt/fill-station/package.nix` already installs all binaries!

Current implementation:

```nix
# In nix/overlays/by-name/crt/fill-station/package.nix
{
  rustPlatform,
  crt-software-root,
}:
rustPlatform.buildRustPackage (finalAttrs: {
  pname = "fill-station";
  version = "0.1.0";

  src = crt-software-root + /fill-station;
  cargoLock.lockFile = finalAttrs.src + /Cargo.lock;

  doCheck = false;

  # Install all binaries from src/bin/, not just the main one
  postInstall = ''
    # The main binary is already installed, now add the others
    for bin in target/*/release/*; do
      if [ -f "$bin" ] && [ -x "$bin" ]; then
        basename=$(basename "$bin")
        # Skip .d files, libraries, and the main fill-station binary
        if [[ "$basename" != *.d ]] && [[ "$basename" != lib* ]] && [[ "$basename" != "fill-station" ]]; then
          install -Dm755 "$bin" "$out/bin/$basename"
        fi
      fi
    done
  '';

  meta = {
    description = "Fill Station Binary and Utilities";
    mainProgram = "fill-station";
  };
})
```

This already installs: `fill-station`, `adc_monitor`, `adc_test`, and `dual_adc_monitor`.

If `adc_monitor` is missing on your board, verify:
1. The build completed successfully
2. The SD image was fully flashed
3. Check `/bin/` on the board for all installed binaries
```

## Quick Test Without Rebuilding

If you have SSH access to the board and want to test quickly:

```bash
# On your Mac, cross-compile just the binary
cd fill-station
cargo build --release --target aarch64-unknown-linux-musl --bin adc_monitor

# Copy to the board
scp target/aarch64-unknown-linux-musl/release/adc_monitor root@<board-ip>:/tmp/

# On the board, run it
/tmp/adc_monitor
```

## Let me know which of these you want me to fix!
