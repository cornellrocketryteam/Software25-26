{
  kernel,
  dtb,
  dtbOverlay,
  initrd,

  stdenvNoCC,
  dtc,
  zstd,
  ubootTools,
}:
stdenvNoCC.mkDerivation {
  name = "fill-station-fit-image";

  nativeBuildInputs = [
    dtc
    zstd
    ubootTools
  ];

  # Assuming that the FIT image is loaded to ${addr_fit}, this variable should
  # be set equal to the UBoot $loadaddr env variable
  loadaddr = "0x82000000";

  buildCommand = ''
    zstd -19 ${kernel} -o kernel.zst

    cp ${dtb} dtb
    chmod +w dtb

    # Apply the overlay to the base DTB
    fdtoverlay -i dtb -o dtb-merged ${dtbOverlay}

    # Remove SerDes PHY reference from USB node — we only need USB 2.0
    fdtput -d dtb-merged /bus@f4000/cdns-usb@f900000/usb@f400000 phys
    fdtput -d dtb-merged /bus@f4000/cdns-usb@f900000/usb@f400000 phy-names

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

  __structuredAttrs = true;
  unsafeDiscardReferences.out = true;
}
