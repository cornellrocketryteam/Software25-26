{
  kernel,
  dtb,
  initrd,

  dtc,
  lib,
  stdenvNoCC,
  ubootTools,
  xz,
  zstd,
}:
stdenvNoCC.mkDerivation {
  name = "fit-image";

  nativeBuildInputs = [
    zstd
    dtc
    xz
    ubootTools
  ];

  env = {
    kernelParams = toString [
      "debug"
      "console=ttyS2,115200n8"
      "panic=-1"
    ];

    # We know we are fitting this FIT image in QSPI at least twice, which is
    # 256 MiB in size. There is a bunch of other stuff in QSPI taking up space,
    # so we aren't even taking up all the space. To be on the highly
    # conservative side, let's assume we take 1/4 the size of QSPI and set the
    # load address of our kernel to be U-Boot's load address plus that size. We
    # have to add onto U-Boot's load address because that is where we are
    # loading the FIT image itself, and U-Boot will unpack and decompress the
    # kernel from the FIT image into the kernel's load address.
    loadAddress =
      let
        loadAddress = (2080 + 45) * 1024 * 1024; # 0x82000000 + 45 MiB
      in
      "0x${lib.toHexString loadAddress}";
  };

  __structuredAttrs = true;
  unsafeDiscardReferences.out = true;

  buildCommand = ''
    cp ${kernel} kernel
    xz --format=lzma kernel

    cp ${dtb} dtb
    chmod u+w dtb
    fdtput --auto-path --verbose --type=s dtb /chosen bootargs "''${kernelParams[@]}"

    cp ${initrd} initrd

    cp ${./kernel.its} fitImage.its
    substituteInPlace fitImage.its --subst-var loadAddress

    mkimage -f fitImage.its fitImage.itb
    install -Dm0644 -t $out fitImage.itb
  '';
}
