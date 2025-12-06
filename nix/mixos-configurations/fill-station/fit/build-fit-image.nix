{
  kernel,
  dtb,
  dtbo ? null,
  initrd,
  debug ? false,

  stdenvNoCC,
  dtc,
  xz,
  ubootTools,
}:
stdenvNoCC.mkDerivation {
  name = "fill-station-fit-image";

  nativeBuildInputs = [
    dtc
    xz
    ubootTools
  ];

  env = {
    kernelParams = toString [
      (if debug then "debug" else "quiet")
      "console=ttyS2,115200n8"
      "panic=-1"
    ];

    # Assuming that the FIT image is loaded to ${addr_fit}, this variable should
    # be set equal to the UBoot $loadaddr env variable
    loadaddr = "0x82000000";
  };

  __structuredAttrs = true;
  unsafeDiscardReferences.out = true;

  buildCommand = ''
    cp ${kernel} kernel
    xz --format=lzma kernel

    cp ${dtb} dtb
    chmod u+w dtb
    
    # Apply device tree overlay if provided
    ${if dtbo != null then ''
      echo "Applying device tree overlay..."
      fdtoverlay -i dtb -o dtb-with-overlay ${dtbo}
      mv dtb-with-overlay dtb
    '' else ""}
    
    fdtput --auto-path --verbose --type=s dtb /chosen bootargs "''${kernelParams[@]}"

    cp ${initrd} initrd

    cp ${./fitImage.its} fitImage.its
    substituteInPlace fitImage.its --subst-var loadaddr

    mkimage -f fitImage.its fitImage.itb
    install -Dm0644 -t $out fitImage.itb
  '';
}
