{
  ti-linux-firmware,

  buildUBoot,
  buildPackages,
  python3Packages,
}:
let
  defconfigFile = ./configs/am64x_r5_defconfig;
  defconfigName = baseNameOf defconfigFile;
in
(buildUBoot {
  defconfig = defconfigName;
  preConfigure = ''
    cp ${defconfigFile} configs/${defconfigName}
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
