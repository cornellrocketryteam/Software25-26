{
  lib,
  rustPlatform,
  crt-software-root,
}:
rustPlatform.buildRustPackage rec {
  pname = "fill-station-binary";
  version = "0.1.0";

  src = crt-software-root + /fill-station;
  cargoLock.lockFile = src + /Cargo.lock;

  meta = {
    description = "Fill Station Binary";
    mainProgram = "fill-station";
    # platforms = lib.platforms.linux;
  };
}