{
  rustPlatform,
  crt-software-root,
}:
rustPlatform.buildRustPackage (finalAttrs: {
  pname = "fill-station";
  version = "0.1.0";

  src = crt-software-root + /fill-station;
  cargoLock.lockFile = finalAttrs.src + /Cargo.lock;

  # Skip tests (since we don't have any)
  doCheck = false;

  meta = {
    description = "Fill Station Binary";
    mainProgram = "fill-station";
    # platforms = lib.platforms.linux;
  };
})
