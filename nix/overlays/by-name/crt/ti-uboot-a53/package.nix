{
  ti-optee,
  ti-arm-trusted-firmware,
  ti-linux-firmware,
  buildUBoot,
  buildPackages,
}:
let
  defconfigFile = ./am64x_a53_defconfig;
  defconfigName = baseNameOf defconfigFile;
in
buildUBoot {
  defconfig = defconfigName;
  preConfigure = ''
    cp ${defconfigFile} configs/${defconfigName}
  '';
  extraPatches = [
    ./patches/ti-board-detect-fixes.patch
  ];

  extraMeta.platforms = [ "aarch64-linux" ];

  BL31 = "${ti-arm-trusted-firmware}/bl31.bin";
  filesToInstall = [
    "tispl.bin" # For HS-FS and HS-SE
    "u-boot.img" # For HS-FS and HS-SE
    "tispl.bin_unsigned" # For GP
    "u-boot.img_unsigned" # For GP
  ];
  extraMakeFlags = [
    "BINMAN_INDIRS=${ti-linux-firmware}"
    "TEE=${ti-optee}/tee-raw.bin"
  ];

  # Add library path for libfdt so that it does not look in /usr for libfdt
  preBuild = ''
    export DYLD_LIBRARY_PATH="${buildPackages.dtc}/lib:$DYLD_LIBRARY_PATH"
  '';
}
