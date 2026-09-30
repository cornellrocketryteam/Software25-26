{
  lib,
  rustPlatform,
  crt-software-root,
  # Also build the ADC CLI tools from src/bin/ (adc_monitor, adc_test, ...).
  # Off by default to keep the production image small.
  withHelpers ? false,
}:
rustPlatform.buildRustPackage (finalAttrs: {
  pname = "fill-station";
  version = "0.1.0";

  src = crt-software-root + /fill-station;
  cargoLock.lockFile = finalAttrs.src + /Cargo.lock;

  # Only build the main binary unless the helpers are wanted
  cargoBuildFlags = lib.optionals (!withHelpers) [
    "--bin"
    "fill-station"
  ];

  # Skip tests (since we don't have any)
  doCheck = false;

  # Install all binaries from src/bin/, not just the main one
  postInstall = lib.optionalString withHelpers ''
    # The main binary is already installed, now add the others
    for bin in target/*/release/*; do
      if [ -f "$bin" ] && [ -x "$bin" ]; then
        basename=$(basename "$bin")
        # Skip .d files, libraries, and the main fill-station binary (already installed)
        if [[ "$basename" != *.d ]] && [[ "$basename" != lib* ]] && [[ "$basename" != "fill-station" ]]; then
          install -Dm755 "$bin" "$out/bin/$basename"
        fi
      fi
    done
  '';

  meta = {
    description = "Fill Station Binary and Utilities";
    mainProgram = "fill-station";
    # platforms = lib.platforms.linux;
  };
})
