{
  tfa,
  optee,
  ti-linux-firmware,

  buildUBoot,
  buildPackages,
}:
(buildUBoot {
  defconfig = "am64x_evm_a53_defconfig";
  extraMeta.platforms = [ "aarch64-linux" ];

  BL31 = "${tfa}/bl31.bin";
  filesToInstall = [
    "tispl.bin"
    "u-boot.img"
    # "tispl.bin_unsigned"
    # "u-boot.img_unsigned"
  ];
  extraMakeFlags = [
    "BINMAN_INDIRS=${ti-linux-firmware}"
    "TEE=${optee}/tee-raw.bin"
  ];

  extraConfig = ''
    CONFIG_LTO=y
  '';

  # Add library path for libfdt so that it does not look in /usr for libfdt
  preBuild = ''
    export DYLD_LIBRARY_PATH="${buildPackages.dtc}/lib:$DYLD_LIBRARY_PATH"
  '';
})
