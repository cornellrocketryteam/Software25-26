{ pkgs, uboot, fill-station }:
let
  gapMiB = 2;
  firmwareSizeMiB = 60;
  # Controls for the second partition
  secondPartitionSizeMiB = 100;
  # Partition type codes for sfdisk: 'c' = W95 FAT32 (LBA), '83' = Linux
  secondPartitionType = "c";
  # Name of the second partition
  secondPartitionLabel = "DATA";
  label-id = "0x2178694e";
  populateFirmwareCommands = ''
    cp ${uboot.r5}/tiboot3-am64x_sr2-hs-fs-evm.bin firmware/tiboot3.bin
    cp ${uboot.a53}/tispl.bin firmware/tispl.bin
    cp ${uboot.a53}/u-boot.img firmware/u-boot.img
    cp ${uboot.uEnv} firmware/uEnv.txt
    cp ${fill-station.config.system.build.fitImage}/fitImage.itb firmware/fitImage.itb

  '';
in
pkgs.stdenv.mkDerivation (finalAttrs: {
  name = "fill-station.img";

  nativeBuildInputs = with pkgs; [
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

    # Create the image file sized to fit two partitions (first + second)
    gap=${toString gapMiB}
    firmwareSize=${toString firmwareSizeMiB}
    secondSize=${toString secondPartitionSizeMiB}

    firmwareSizeBlocks=$((firmwareSize * 1024 * 1024 / 512))
    secondSizeBlocks=$((secondSize * 1024 * 1024 / 512))
    imageSize=$((firmwareSizeBlocks * 512 + secondSizeBlocks * 512 + gap * 1024 * 1024))
    truncate -s $imageSize $img

    # Create partition table with two partitions (type set by globals)
    sfdisk --no-reread --no-tell-kernel $img <<EOF
      label: dos
      label-id: ${label-id}

      start=${toString gapMiB}M, size=${toString firmwareSizeMiB}M, type=c,bootable
      start=${
        toString (gapMiB + firmwareSizeMiB)
      }M, size=${toString secondPartitionSizeMiB}M, type=${toString secondPartitionType}
    EOF

    # compute sector offsets for dd
    START1=$((gap * 1024 * 1024 / 512))
    SECTORS1=$firmwareSizeBlocks
    START2=$((START1 + SECTORS1))
    SECTORS2=$secondSizeBlocks

    # create partition filesystem images
    truncate -s $((SECTORS1 * 512)) firmware_part.img
    truncate -s $((SECTORS2 * 512)) second_part.img

    mkfs.vfat --invariant -i ${label-id} -n FIRMWARE firmware_part.img
    mkfs.vfat --invariant -n ${toString secondPartitionLabel} second_part.img

    # Populate firmware files into first partition image
    mkdir -p firmware
    ${populateFirmwareCommands}
    find firmware -exec touch --date=2000-01-01 {} +

    cd firmware
    # Force a fixed order in mcopy for better determinism, and avoid file globbing
    for d in $(find . -type d -mindepth 1 | sort); do
      faketime "2000-01-01 00:00:00" mmd -i ../firmware_part.img "::/$d"
    done
    for f in $(find . -type f | sort); do
      mcopy -pvm -i ../firmware_part.img "$f" "::/$f"
    done
    cd ..

    # Populate second partition image with a small test file
    mkdir -p data
    echo "Test partition 2" > data/README.txt
    find data -exec touch --date=2000-01-01 {} +

    cd data
    for d in $(find . -type d -mindepth 1 | sort); do
      faketime "2000-01-01 00:00:00" mmd -i ../second_part.img "::/$d"
    done
    for f in $(find . -type f | sort); do
      mcopy -pvm -i ../second_part.img "$f" "::/$f"
    done
    cd ..

    # Verify the FAT partitions before copying them into the disk image.
    fsck.vfat -vn firmware_part.img
    fsck.vfat -vn second_part.img

    # Write partition images into the full disk image at the correct offsets
    dd conv=notrunc if=firmware_part.img of=$img seek=$START1 count=$SECTORS1
    dd conv=notrunc if=second_part.img of=$img seek=$START2 count=$SECTORS2
  '';
})
