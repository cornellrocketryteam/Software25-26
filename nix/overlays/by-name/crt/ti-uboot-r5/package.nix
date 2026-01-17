{
  ti-linux-firmware,
  buildUBoot,
  buildPackages,
  python3Packages,
}:
let
  defconfigFile = ./am64x_r5_defconfig;
  defconfigName = baseNameOf defconfigFile;
in
(buildUBoot {
  defconfig = defconfigName;
  preConfigure = ''
    cp ${defconfigFile} configs/${defconfigName}
    for f in $(find arch/arm/dts -name "k3-am642-sk.dts" -o -name "k3-am642-r5-sk.dts"); do
      echo "" >> $f
      echo "&epwm5 { status = \"disabled\"; };" >> $f
    done
  '';
  extraMeta.platforms = [ "armv7l-linux" ];

  filesToInstall = [
    "tiboot3-am64x-gp-evm.bin"
    "tiboot3-am64x_sr2-hs-fs-evm.bin"
    "tiboot3-am64x_sr2-hs-evm.bin"
  ];

  extraMakeFlags = [
    "BINMAN_INDIRS=${ti-linux-firmware}"
  ];

  # Add library path for libfdt so that it does not look in /usr for libfdt
  preBuild = ''
    export DYLD_LIBRARY_PATH="${buildPackages.dtc}/lib:$DYLD_LIBRARY_PATH"
  '';
}).overrideAttrs
  (oldAttrs: {
    nativeBuildInputs = oldAttrs.nativeBuildInputs ++ [
      # These Python packages are required by a ti-linux-firmware script
      python3Packages.pyyaml
      python3Packages.yamllint
      python3Packages.jsonschema
    ];
  })
