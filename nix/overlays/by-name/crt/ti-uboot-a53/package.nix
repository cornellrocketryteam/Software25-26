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
    for f in $(find arch/arm/dts -name "k3-am642-sk.dts" -o -name "k3-am642-r5-sk.dts"); do
      echo "" >> $f
      echo "&epwm5 { status = \"disabled\"; };" >> $f
    done
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
