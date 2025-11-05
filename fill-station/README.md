# Beamformer Sim Linux Setup

This repository contains Nix flake configurations for building U-Boot and associated firmware components for the Texas Instruments SK-AM64B evaluation board.

## Overview

The TI AM64x is a dual-core Cortex-A53 processor with additional Cortex-R5F cores for real-time processing. This flake provides a reproducible build environment for the complete boot chain required by the AM64x platform.

## AM64x Boot Flow

The AM64x follows a multi-stage boot process as documented in the [official U-Boot documentation](https://docs.u-boot.org/en/latest/board/ti/am64x_evm.html):

### Boot Stages

1. **ROM Boot Loader (RBL)**
   - First stage executed from on-chip ROM
   - Loads the R5 SPL from boot media (SD card, eMMC, OSPI, etc.)
   - Performs minimal hardware initialization

2. **R5 SPL (Secondary Program Loader)**
   - Runs on the Cortex-R5F cores
   - Exists in the `tiboot3-am64x_sr2-*-evm.bin` files
   - Initializes DDR memory
   - Loads TF-A, OP-TEE, and A53 SPL

3. **Cortex-A53 Boot Chain**
   - **TF-A (Trusted Firmware-A)**: Provides secure boot and runtime services
   - **OP-TEE**: Trusted Execution Environment for secure applications
   - **A53 SPL**: Secondary bootloader for Cortex-A53
   - **U-Boot Proper**: Full U-Boot with all features enabled

## AM64x Security Modes

The TI AM64x processors can be booted in three different security "modes", each with different capabilities and requirements:

### GP (General Purpose)

This is a SoC/board state where there is no devie protection and authentication is not enabled for booting the device.

### HS-FS (High Security - Field Securable)

This is a SoC/board state before a customer has blown the keys in the device. i.e. the state at which HS device leaves TI factory. In this state, the device protects the ROM code, TI keys and certain security peripherals. In this state, device do not force authentication for booting, however DMSC is locked.

### HS-SE (High Security - Security Enforced)

This is a SoC/board state after a customer has successfully blown the keys and set “customer Keys enable”. In HS-SE device all security features enabled. All secrets within the device are fully protected and all of the security goals are fully enforced. The device also enforces secure booting.

> [!NOTE]
> The flake by default builds the "GP" and "HS-FS" variants. It does not build the "HS-SE" variant by default to prevent accidentally blowing the keys into the chip which could (maybe?) brick the device. To build the "HS-SE" variant, enable the `buildHS` option in `uboot-r5.nix`.

## What is OP-TEE?

**OP-TEE (Open Portable Trusted Execution Environment)** is an open-source Trusted Execution Environment (TEE) that provides a secure operating system running alongside the normal Linux kernel.

### Key Features:
- **Isolation**: Runs in ARM TrustZone secure world, isolated from the normal world OS
- **Secure Services**: Provides cryptographic operations, secure storage, and key management
- **Trusted Applications**: Supports running secure applications (TAs) that handle sensitive operations
- **Standards Compliant**: Implements GlobalPlatform TEE specifications

### Why We Use OP-TEE:

1. **Secure Boot Chain**: OP-TEE is part of the trusted boot sequence, ensuring system integrity from power-on. It is required by any of the `HS` boot modes.
2. **Protected Resources**: Manages access to secure hardware resources and cryptographic accelerators
3. **Secure Storage**: Provides encrypted storage for sensitive data like keys and certificates
4. **Runtime Security**: Offers secure services to Linux applications through the TEE Client API
5. **Hardware Features**: Leverages AM64x security features like:
   - Hardware crypto accelerators
   - Secure timers
   - True random number generator (TRNG)
   - Secure memory regions

### Boot Image Files

In order to boot we need tiboot3.bin, tispl.bin and u-boot.img. Each SoC variant (GP, HS-FS, HS-SE) requires a different source for these files.

- GP
   - `tiboot3-am64x-gp-evm.bin` - R5 SPL firmware (first stage)
   - `tispl.bin` or `tispl.bin_unsigned` - Combined A53 SPL + TF-A + OP-TEE image
   - `u-boot.img` or `u-boot.img_unsigned` - U-Boot proper image
- HS-FS (recommended)
   - `tiboot3-am64x_sr2-hs-fs-evm.bin` - R5 SPL firmware (first stage)
   - `tispl.bin` - Combined A53 SPL + TF-A + OP-TEE image
   - `u-boot.img` - U-Boot proper image
- HS-SE
   - `tiboot3-am64x_sr2-hs-se-evm.bin` - R5 SPL firmware (first stage)
   - `tispl.bin` - Combined A53 SPL + TF-A + OP-TEE image
   - `u-boot.img` - U-Boot proper image

## Prerequisites

- Nix with flakes enabled
- Supported build platforms: x86_64-linux, aarch64-linux, x86_64-darwin, aarch64-darwin

> [!WARNING]
> Only aarch64-darwin has been tested so far.

## Building

To build all U-Boot components:

```bash
nix build
```

To build individual components:

```bash
# Build both the R5 SPL and A53 U-Boot (this is the default)
nix build .#uboot-all

# Build only the R5 SPL
nix build .#uboot-r5

# Build only the A53 U-Boot
nix build .#uboot-a53

# Build TF-A
nix build .#tfa

# Build OP-TEE
nix build .#optee
```

## Accessing the serial port

To access the serial port, use either `screen` or `tio`. To use `tio` (recommended), run the following command:

```bash
export SERIAL_PORT=/dev/cu.usbserial-0136E7AD1
nix shell "nixpkgs#tio" -c tio $SERIAL_PORT
```

## Flashing Instructions

After building, all the boot images can be found in the `result` directory:

1. **For SD Card Boot:**
Follow the instructions in the [official TI documentation](https://software-dl.ti.com/processor-sdk-linux/esd/AM64X/07_03_00_02/exports/docs/devices/AM64X/Overview/Create_SD_Card.html#create-sd-card-with-custom-images)

> [!WARNING]
> This has not been tested.

2. **For OSPI Flash (recommended):**
Follow the instructions in the [official U-Boot documentation](https://docs.u-boot.org/en/latest/board/ti/am64x_evm.html#ospi)

```bash
=> sf probe
=> tftp  ${loadaddr} tiboot3.bin
=> sf update $loadaddr 0x0 $filesize
=> tftp $floadaddr} tispl.bin
=> sf update $loadaddr 0x100000 $filesize
=> tftp ${loadaddr} u-boot.img
=> sf update $loadaddr 0x300000 $filesize
```

Instead of using `tftp`, you can use `loadx` and Xmodem to transfer the files (this is what I did). Using Xmodem you would run the below files in U-Boot shell:

```bash
=> sf probe
=> loadx  ${loadaddr}
=> sf update $loadaddr 0x0 $filesize
=> loadx ${loadaddr}
=> sf update $loadaddr 0x100000 $filesize
=> loadx ${loadaddr}
=> sf update $loadaddr 0x300000 $filesize
```

And **at the same time** run these commands on your computer to send the files (replace the serial port with your OS specific port and the files if you are not using the recommended HS-FS mode):

```bash
$ export SERIAL_PORT=/dev/cu.usbserial-0136E7AD1
$ nix shell nixpkgs#lrzsz -c sx -kb ./result/tiboot3-am64x_sr2-hs-fs-evm.bin < $SERIAL_PORT > $SERIAL_PORT
$ nix shell nixpkgs#lrzsz -c sx -kb ./result/tispl.bin < $SERIAL_PORT > $SERIAL_PORT
$ nix shell nixpkgs#lrzsz -c sx -kb ./result/u-boot.img < $SERIAL_PORT > $SERIAL_PORT
```

> [!NOTE]
> If you are setting the Boot Mode Switches to boot using OSPI, *do not* set them to be OSPI. Instead switch the boot mode to be xSPI. On the SK-AM64B, this corrsponds to [ON, ON, ON, OFF] for the primary boot mode bits.

## Cross-Compilation

The flake handles cross-compilation automatically (and can be compiled from MacOS):
- R5 components use ARMv7 toolchain (32-bit)
- A53 components use AArch64 toolchain (64-bit)

## Customization

### Modifying U-Boot Configuration

To add custom U-Boot configurations, edit the `am64x_evm_extra.config` file:

```bash
CONFIG_BOOTCOMMAND="bootflow scan -lb"
CONFIG_BOOTDELAY=0
# Add your custom configs here
```

This file gets imported into the A53 U-Boot configuration.

### Switching Board Support

To support different boards, modify:
- Defconfig names in `uboot-r5.nix` and `uboot-a53.nix`
- Target board settings in `tfa.nix`
- Platform configurations in `optee.nix`

## References

- [TI Official Bootflow Guide for AM64X](https://software-dl.ti.com/mcu-plus-sdk/esd/AM64X/latest/exports/docs/api_guide_am64x/BOOTFLOW_GUIDE.html)
- [U-Boot AM64x Documentation](https://docs.u-boot.org/en/latest/board/ti/am64x_evm.html)
- [TI Processor SDK Documentation for U-Boot](https://software-dl.ti.com/processor-sdk-linux/esd/AM64X/08_06_00_42/exports/docs/linux/Foundational_Components/U-Boot/UG-General-Info.html)
- [OP-TEE Documentation](https://optee.readthedocs.io/)
- [TF-A Documentation](https://trustedfirmware-a.readthedocs.io/en/latest/index.html)
