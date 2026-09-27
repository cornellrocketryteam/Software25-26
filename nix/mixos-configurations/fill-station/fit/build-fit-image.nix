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
      "firmware_class.path=/etc/lib/firmware"
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
    
    # Remove SerDes PHY reference from USB node — we only need USB 2.0
    fdtput -d dtb-merged /bus@f4000/cdns-usb@f900000/usb@f400000 phys
    fdtput -d dtb-merged /bus@f4000/cdns-usb@f900000/usb@f400000 phy-names

    # Sanity-check that the overlay did what we meant
    expect() {
      if [ "$2" != "$3" ]; then
        echo "DTB check failed: $1: expected '$3', got '$2'" >&2
        exit 1
      fi
    }
    usb0=/bus@f4000/cdns-usb@f900000/usb@f400000
    expect "usb0 dr_mode" "$(fdtget dtb-merged $usb0 dr_mode)" host
    expect "usb0 phys removed" "$(fdtget -d absent dtb-merged $usb0 phys)" absent
    expect "PRU test pin (GPIO1_1)" \
      "$(fdtget -tx dtb-merged /bus@f4000/pinctrl@f4000/pru-test-default-pins pinctrl-single,pins)" \
      "164 50007"

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
