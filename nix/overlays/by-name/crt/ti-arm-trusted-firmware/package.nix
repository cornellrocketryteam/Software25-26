{ buildArmTrustedFirmware, openssl }:
let
  platform = "k3";
  targetBoard = "lite";
in
buildArmTrustedFirmware {
  inherit platform;
  extraMeta.platforms = [ "aarch64-linux" ];
  extraMakeFlags = [
    "TARGET_BOARD=${targetBoard}"
    "SPD=opteed"
  ];
  filesToInstall = [
    "build/${platform}/${targetBoard}/release/bl31.bin"
  ];

  # Override and fix build issue
  rk3399-m0-oc = null;
  nativeBuildInputs = [
    openssl
  ];
}
