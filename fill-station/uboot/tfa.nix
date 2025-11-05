{ buildArmTrustedFirmware }:
let
  platform = "k3";
  targetBoard = "lite";
in
buildArmTrustedFirmware {
  platform = platform;
  extraMeta.platforms = [ "aarch64-linux" ];
  extraMakeFlags = [
    "TARGET_BOARD=${targetBoard}"
    # "SPD=opteed"
  ];
  filesToInstall = [
    "build/${platform}/${targetBoard}/release/bl31.bin"
  ];
}
