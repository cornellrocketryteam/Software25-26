{
  kernel,
  dtb,
  dtbOverlay,
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
    
    # Apply the overlay to the base DTB
    fdtoverlay -i dtb -o dtb-merged ${dtbOverlay}
    
    # Add kernel boot parameters to the merged DTB
    fdtput --auto-path --verbose --type=s dtb-merged /chosen bootargs "''${kernelParams[@]}"
    
    # Use the merged DTB
    mv dtb-merged dtb

    cp ${initrd} initrd

    cp ${./fitImage.its} fitImage.its
    substituteInPlace fitImage.its --subst-var loadaddr

    mkimage -f fitImage.its fitImage.itb
    install -Dm0644 -t $out fitImage.itb
  '';
}
