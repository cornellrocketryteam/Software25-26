{
  uboot-r5,
  uboot-a53,

  stdenv,
  dosfstools,
  mtools,
  libfaketime,
  util-linux,
}:
let
  gapMiB = 2;
  firmwareSizeMiB = 30;
  label-id = "0x2178694e";
  populateFirmwareCommands = ''
    cp ${uboot-r5}/tiboot3-am64x_sr2-hs-fs-evm.bin firmware/tiboot3.bin
    cp ${uboot-a53}/tispl.bin firmware/tispl.bin
    cp ${uboot-a53}/u-boot.img firmware/u-boot.img
  '';
in
stdenv.mkDerivation (finalAttrs: {
  name = "fill-station.img";

  nativeBuildInputs = [
    dosfstools
    libfaketime
    mtools
    util-linux
  ];

  buildCommand = ''
    mkdir -p $out
    export img=$out/${finalAttrs.name}

    # Gap in front of the first partition, in MiB
    gap=${toString gapMiB}

    # Create the image file sized to fit firmware
    firmwareSizeBlocks=$((${toString firmwareSizeMiB} * 1024 * 1024 / 512))
    imageSize=$((firmwareSizeBlocks * 512 + gap * 1024 * 1024))
    truncate -s $imageSize $img

    # type=b is 'W95 FAT32'
    sfdisk --no-reread --no-tell-kernel $img <<EOF
        label: dos
        label-id: ${label-id}

        start=${toString gapMiB}M, size=${toString firmwareSizeMiB}M, type=c,bootable
    EOF

    # Create a FAT32 firmware partition
    START=$((gap * 1024 * 1024 / 512))
    SECTORS=$firmwareSizeBlocks
    truncate -s $((SECTORS * 512)) firmware_part.img
    mkfs.vfat --invariant -i ${label-id} -n FIRMWARE firmware_part.img

    # Populate firmware files
    mkdir firmware
    ${populateFirmwareCommands}
    find firmware -exec touch --date=2000-01-01 {} +

    # Copy files to FAT partition
    cd firmware
    # Force a fixed order in mcopy for better determinism, and avoid file globbing
    for d in $(find . -type d -mindepth 1 | sort); do
      faketime "2000-01-01 00:00:00" mmd -i ../firmware_part.img "::/$d"
    done
    for f in $(find . -type f | sort); do
      mcopy -pvm -i ../firmware_part.img "$f" "::/$f"
    done
    cd ..

    # Verify the FAT partition before copying it.
    fsck.vfat -vn firmware_part.img
    dd conv=notrunc if=firmware_part.img of=$img seek=$START count=$SECTORS
  '';
})
