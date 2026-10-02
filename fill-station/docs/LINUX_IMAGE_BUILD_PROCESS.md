# Linux Image Build Process Documentation

## Overview

This document explains how the embedded Linux system image is built for the fill-station target on the Texas Instruments AM64x platform. The build system uses Nix flakes and the MixOS framework to create a bootable SD card image containing U-Boot bootloaders, a FIT (Flattened Image Tree) image with the kernel, device tree, and initrd.

---

## Architecture Overview

The build process produces a complete bootable SD card image (`fill-station.img`) through several stages:

```
┌─────────────────────────────────────────────────────────────────┐
│                     Nix Flake Entry Point                         │
│                      (flake.nix)                                  │
└───────────────────────────────┬─────────────────────────────────┘
                                │
                ┌───────────────┴───────────────┐
                │                               │
                ▼                               ▼
┌───────────────────────────┐   ┌──────────────────────────────┐
│   MixOS Configuration     │   │   Custom Overlays            │
│   (mixos-configurations/) │   │   (overlays/by-name/crt/)    │
└───────────┬───────────────┘   └──────────┬───────────────────┘
            │                               │
            │                               │ Provides:
            │                               │ - fill-station-linux
            │                               │ - ti-uboot-r5/a53
            │                               │ - ti-arm-trusted-firmware
            │                               │ - ti-optee
            │                               │ - fill-station binary
            │                               │
            ▼                               │
┌─────────────────────────────────┐         │
│  Fill Station Configuration     │◄────────┘
│  (fill-station/default.nix)     │
│                                 │
│  - Kernel: fill-station-linux         │
│  - Init processes               │
│  - Binaries to include          │
│  - User/group config            │
└──────────┬──────────────────────┘
           │
           │ Imports:
           │
           ├──────────────────────────┐
           │                          │
           ▼                          ▼
┌──────────────────────┐   ┌────────────────────┐
│   FIT Image Build    │   │  SD Image Build    │
│   (fit/default.nix)  │   │ (sd-image/)        │
└──────┬───────────────┘   └────────┬───────────┘
       │                            │
       │ Produces:                  │ Produces:
       │ fitImage.itb               │ fill-station.img
       │                            │
       ▼                            ▼
┌──────────────────────┐   ┌────────────────────┐
│   Kernel (Image)     │   │  Bootable SD Card  │
│   DTB (device tree)  │   │  - U-Boot R5       │
│   Initrd (ramdisk)   │   │  - U-Boot A53      │
└──────────────────────┘   │  - FIT Image       │
                           │  - Boot config     │
                           └────────────────────┘
```

---

## Build Components

### 1. Custom Overlay Packages (`nix/overlays/by-name/crt/`)

The system requires several custom-built packages specific to the TI AM64x platform:

#### **fill-station-linux** (`fill-station-linux/package.nix`)
- Custom Linux kernel with AM64x-specific configuration
- Based on `linux_latest` from nixpkgs
- Uses manual kernel configuration from `kernel.config`
- Outputs: kernel `Image` file (uncompressed ARM64 kernel)

#### **ti-uboot-r5** (`ti-uboot-r5/package.nix`)
- U-Boot for the R5 core (ARM Cortex-R5F) - first-stage bootloader
- Built for `armv7l-linux` platform
- Requires TI Linux firmware (`ti-linux-firmware`)
- Outputs multiple boot images:
  - `tiboot3-am64x-gp-evm.bin` (GP = General Purpose devices)
  - `tiboot3-am64x_sr2-hs-fs-evm.bin` (HS-FS = High Security - Field Securable)
  - `tiboot3-am64x_sr2-hs-evm.bin` (HS-SE = High Security - Security Enforced)
- **Role**: Initial boot ROM loads this from SD card; it initializes DDR and loads the A53 bootloader

#### **ti-uboot-a53** (`ti-uboot-a53/package.nix`)
- U-Boot for the A53 cores (ARM Cortex-A53) - second-stage bootloader
- Built for `aarch64-linux` platform
- Depends on:
  - `ti-arm-trusted-firmware` (provides `bl31.bin` - ARM Trusted Firmware BL31)
  - `ti-optee` (provides `tee-raw.bin` - OP-TEE trusted execution environment)
  - `ti-linux-firmware` (TI-specific firmware blobs)
- Outputs:
  - `tispl.bin` (SPL - Secondary Program Loader, for HS devices)
  - `u-boot.img` (main U-Boot image)
  - Unsigned versions for GP devices
- **Role**: Loads and executes the FIT image containing kernel, DTB, and initrd

#### **ti-arm-trusted-firmware** (`ti-arm-trusted-firmware/package.nix`)
- ARM Trusted Firmware for secure boot and runtime services
- Platform: `k3` (TI's Keystone 3 architecture family)
- Target board: `lite`
- Includes OP-TEE dispatcher (`SPD=opteed`)
- Outputs: `bl31.bin` (Boot Loader stage 3.1 - EL3 runtime firmware)

#### **ti-optee** (`ti-optee/package.nix`)
- OP-TEE OS (Open Portable Trusted Execution Environment)
- Provides secure world execution environment
- Cross-compiled for `armv7l-hf-multiplatform`
- Outputs: `tee-raw.bin`

#### **fill-station** (`fill-station/package.nix`)
- The actual ground station Rust application
- WebSocket server on port 9000
- Controls hardware components (igniters, ADCs, valves, etc.)
- Also installs the helper binaries from `fill-station/src/bin/` (`adc_monitor`, `adc_test`, `dual_adc_monitor`)

#### **fillstation-dtbo** (`fillstation-dtbo/package.nix`)
- Device tree overlay (pinmux, I2C2, ePWM, USB0 host, PRU-header GPIOs)
- Preprocessed with `clang -E`, compiled with `dtc -@`
- Merged into the kernel's `k3-am642-sk.dtb` at FIT build time (see below)
- See [DTBO_BUILDER.md](DTBO_BUILDER.md) and [PRU_HEADER.md](PRU_HEADER.md)

---

### 2. MixOS Configuration (`nix/mixos-configurations/fill-station/`)

#### **Main Configuration** (`default.nix`)

Defines the system configuration:

```nix
{
  # Build and host platform configuration
  nixpkgs.buildPlatform = "aarch64-linux";
  nixpkgs.hostPlatform.config = "aarch64-unknown-linux-musl";

  # Kernel selection
  boot.kernel = pkgs.crt.fill-station-linux;

  # Init processes (PID 1 responsibilities)
  init = {
    shell = {
      tty = "ttyS2";  # Serial console on UART2
      action = "askfirst";
      process = "/bin/sh";
    };
    sshd = {
      action = "respawn";
      process = "${lib.getExe' pkgs.crt.dropbear-minimal "dropbear"} -F -R";
    };
    fill-station = {
      action = "respawn";  # restarted by init if it exits/crashes
      process = lib.getExe pkgs.crt.fill-station;
    };
    wpa_supplicant = {
      action = "respawn";
      process = "${lib.getExe' pkgs.wpa_supplicant "wpa_supplicant"} -i wlan0 -c /etc/wpa_supplicant.conf";
    };
    mount_data = {
      action = "wait";  # mounts the DATA partition at /tmp/data (CSV logs)
      process = "sh -c 'mkdir -p /tmp/data && (mount -t vfat -L DATA /tmp/data || ...)'";
    };
    dhcp = {
      action = "respawn";
      process = "${lib.getExe' pkgs.busybox "udhcpc"} -f -i wlan0";
    };
  };

  # Binaries included in system image
  bin = [
    pkgs.crt.dropbear-minimal  # Lightweight SSH server
    pkgs.libgpiod              # GPIO control utilities
    pkgs.tcpdump               # Network debugging
    pkgs.crt.fill-station      # Main application (+ src/bin helpers)
    pkgs.iw                    # Wi-Fi configuration
    pkgs.wpa_supplicant        # WPA2 connection tool
    pkgs.util-linux            # mount -L etc.
  ];

  # wl18xx Wi-Fi firmware, found via firmware_class.path=/etc/lib/firmware
  etc."lib/firmware".source = ...;
  etc."wpa_supplicant.conf".source = ...;  # SSID/PSK baked into the image
  etc."shadow".source = ...;               # root password hash baked in
}
```

**Key aspects:**
- Everything is cross-compiled for `aarch64-unknown-linux-musl` (dynamically linked against musl; the libraries ship in the image's `/nix/store`)
- `buildPlatform` is hardcoded to `aarch64-linux`, so building from a Mac needs an aarch64 Linux builder
- Minimal busybox init from MixOS (not systemd); services are generated into `/etc/inittab`
- MixOS also adds default services: `mixos sysinit`, `syslogd`, `klogd`, `crond`, and `mdev -d` (device hotplug + module autoload)
- Console on `ttyS2` (115200 baud, set in FIT image kernel parameters)
- Imports both FIT and SD image build definitions

---

### 3. FIT Image Build (`nix/mixos-configurations/fill-station/fit/`)

The FIT (Flattened Image Tree) image is a standard format for bundling kernel, device tree, and initrd into a single bootable image with cryptographic verification.

#### **FIT Entry Point** (`fit/default.nix`)

```nix
system.build.fitImage = pkgs.callPackage ./build-fit-image.nix {
  kernel = "${config.boot.kernel}/Image";
  dtb = "${config.boot.kernel}/dtbs/ti/k3-am642-sk.dtb";
  dtbOverlay = "${pkgs.crt.fillstation-dtbo}/k3-am64-fillstation-pinmux-overlay.dtbo";
  initrd = "${config.system.build.initrd}/initrd";
};
```

Passes four inputs to the build script:
1. **Kernel Image**: Uncompressed ARM64 kernel from `fill-station-linux`
2. **DTB** (Device Tree Blob): The stock SK-AM64 DTB built by the kernel itself
3. **DTB overlay**: Fill-station pinmux/peripheral overlay from `fillstation-dtbo`
4. **Initrd**: Initial RAM disk built by MixOS (see [Initrd and Root Filesystem](#initrd-and-root-filesystem))

#### **FIT Build Script** (`fit/build-fit-image.nix`)

The build process:

```bash
# 1. Compress kernel with LZMA
cp ${kernel} kernel
xz --format=lzma kernel

# 2. Merge the fill-station overlay into the stock SK DTB
cp ${dtb} dtb
fdtoverlay -i dtb -o dtb-merged ${dtbOverlay}

# 3. Delete properties an overlay can't remove (USB0 SerDes PHY — USB 2.0 only)
fdtput -d dtb-merged /bus@f4000/cdns-usb@f900000/usb@f400000 phys
fdtput -d dtb-merged /bus@f4000/cdns-usb@f900000/usb@f400000 phy-names

# 4. Sanity-check the merged tree (build fails on mismatch)
expect "usb0 dr_mode" "$(fdtget dtb-merged $usb0 dr_mode)" host
expect "PRU test pin (GPIO1_1)" "$(fdtget -tx dtb-merged .../pru-test-default-pins pinctrl-single,pins)" "164 50007"

# 5. Write kernel boot parameters into /chosen/bootargs
fdtput --auto-path --verbose --type=s dtb-merged /chosen bootargs \
  "quiet console=ttyS2,115200n8 panic=-1 firmware_class.path=/etc/lib/firmware"
mv dtb-merged dtb

# 6. Copy initrd (already xz-compressed)
cp ${initrd} initrd

# 7. Generate FIT image using U-Boot mkimage tool
cp ${./fitImage.its} fitImage.its
substituteInPlace fitImage.its --subst-var loadaddr  # Insert load address
mkimage -f fitImage.its fitImage.itb
```

**Kernel parameters set:**
- `quiet` (or `debug` if debug=true): Controls verbosity
- `console=ttyS2,115200n8`: Serial console configuration
- `panic=-1`: Reboot immediately on kernel panic
- `firmware_class.path=/etc/lib/firmware`: Where the kernel looks for the wl18xx Wi-Fi firmware

**Load address**: `0x82000000` - Memory address where U-Boot loads the FIT image

#### **FIT Image Tree Source** (`fit/fitImage.its`)

Defines the structure of the FIT image:

```dts
/dts-v1/;

/ {
    description = "FIT image with kernel, initrd and DTB";
    
    images {
        kernel {
            data = /incbin/("./kernel.lzma");
            type = "kernel";
            arch = "arm64";
            compression = "lzma";
            load = <0x82000000>;    # Load address
            entry = <0x82000000>;   # Entry point (same as load)
            hash { algo = "crc32"; };
        };
        
        fdt-0 {
            data = /incbin/("./dtb");
            type = "flat_dt";
            compression = "none";
            hash { algo = "crc32"; };
        };
        
        initrd {
            data = /incbin/("initrd");
            type = "ramdisk";
            compression = "none";
            hash { algo = "crc32"; };
        };
    };
    
    configurations {
        default = "board-0";
        board-0 {
            kernel = "kernel";
            fdt = "fdt-0";
            ramdisk = "initrd";
            signature {
                algo = "crc32";
                key-name-hint = "dev";
                sign-images = "fdt-0", "kernel", "ramdisk";
            };
        };
    };
};
```

**Key features:**
- Single configuration named "board-0" (default)
- All components include CRC32 hashes for integrity verification
- Signature block for verified boot (currently using CRC32, upgradable to RSA/ECDSA)

---

### Initrd and Root Filesystem

The `initrd` inside the FIT is produced by MixOS (`system.build.initrd`). It is
tiny on its own: an xz-compressed cpio with just two things:

```
/init                 -> mixos-rdinit (small Zig program)
/nix/store/…-mixos.erofs   (the real root filesystem, LZMA-compressed EROFS)
```

At boot, `mixos-rdinit` mounts `/dev`, `/sys`, `/proc`, loop-mounts the EROFS
image **read-only** at `/sysroot`, moves the pseudo-filesystems into it,
`chroot`s, and execs busybox `/sbin/init`. The whole system runs from RAM — the
SD card is only read at boot (plus the DATA partition, see below).

The EROFS root contains:
- `/nix/store` — the runtime closure (fill-station, busybox, dropbear, libgpiod,
  iw, wpa_supplicant, tcpdump, util-linux, musl, …)
- `/bin` — merged `bin` packages + busybox applets
- `/etc/*` — symlinks into the store (`inittab`, `mdev.conf`, `passwd`,
  `shadow`, `wpa_supplicant.conf`, `lib/firmware`, …)
- `/lib/modules/<version>/` — **all** `=m` kernel modules (see below)

Runtime storage:
- `/` is read-only; `/tmp` (and `/run` → `/tmp`) is tmpfs
- `/state` (and `/var`) is a tmpfs — **nothing persists across reboots**
- The only persistent storage is the SD card's `DATA` partition, mounted at
  `/tmp/data` by the `mount_data` init entry (CSV logs go here)

#### Kernel modules

Because `CONFIG_MODULES=y`, MixOS splits the kernel's modules into a separate
output and copies **every** `=m` module into `/lib/modules/<version>/` in the
EROFS (≈1,400 `.ko` files, ~70 MB), then runs busybox `depmod`.

Nothing loads modules explicitly — `boot.kernelModules` is unset (empty).
Instead MixOS installs this mdev rule:

```
$MODALIAS=.* 0:0 660 @modprobe "$MODALIAS"
```

so a module is loaded only if a device node with a matching modalias appears:
at boot via `mixos sysinit` → `mdev -s` (coldplug), and afterwards via the
`mdev -d` hotplug daemon. This is best-effort. Anything the fill station
depends on (I2C, USB CDC-ACM, Wi-Fi, ePWM, …) is built in (`=y`). When adding a
driver, prefer `=y`; if you must use `=m` and it doesn't autoload, add it to
`boot.kernelModules` in `default.nix`. Check with `lsmod` over SSH.

#### Boot-time services (`/etc/inittab`)

Generated from `init` in `default.nix` plus MixOS defaults:

| Action | Process |
|--------|---------|
| `sysinit` | `mixos sysinit` — mounts, module loading, `mdev -s`, hostname, `/state` |
| `wait` | `mount_data` — DATA partition → `/tmp/data` |
| `respawn` | `fill-station`, `dropbear`, `wpa_supplicant`, `udhcpc`, `syslogd`, `klogd`, `crond`, `mdev -d` |
| `askfirst` | `/bin/sh` on `ttyS2` |

---

### 4. SD Card Image Build (`nix/mixos-configurations/fill-station/sd-image/`)

#### **SD Image Entry Point** (`sd-image/default.nix`)

```nix
system.build.sdImage = pkgs.callPackage ./build-sd-image.nix {
  fitImage = "${config.system.build.fitImage}/fitImage.itb";
};
```

Takes the FIT image as input and builds the complete SD card image.

#### **SD Image Build Script** (`sd-image/build-sd-image.nix`)

Creates a bootable SD card with DOS partition table:

**Partition Layout:**
```
┌────────────────────────────────────────────────────────┐
│  Gap: 2 MiB (unpartitioned space for ROM bootloader)   │
├────────────────────────────────────────────────────────┤
│  Partition 1: FIRMWARE (128 MiB, FAT32, bootable)     │
│  - tiboot3.bin      (R5 U-Boot first stage)           │
│  - tispl.bin        (A53 U-Boot SPL)                  │
│  - u-boot.img       (A53 U-Boot main)                 │
│  - uEnv.txt         (U-Boot environment/config)       │
│  - fitImage.itb     (Kernel + DTB + initrd)           │
├────────────────────────────────────────────────────────┤
│  Partition 2: DATA (1024 MiB, FAT32)                  │
│  - Mounted at /tmp/data; CSV logs                     │
│  - Ships with only a placeholder README.txt           │
└────────────────────────────────────────────────────────┘
Total: ~1.2 GiB
```

> ⚠️ Flashing the image with `dd` overwrites the **whole card, including
> DATA**. Copy any logs off `/tmp/data` before reflashing.

**Build process:**

1. **Partition Table Creation:**
   ```bash
   sfdisk $img <<EOF
     label: dos
     label-id: 0x2178694e
     
     start=2M, size=128M, type=c, bootable
     start=130M, size=1024M, type=c
   EOF
   ```

2. **Filesystem Creation:**
   ```bash
   mkfs.vfat --invariant -i 0x2178694e -n FIRMWARE firmware_part.img
   mkfs.vfat --invariant -n DATA second_part.img
   ```
   - `--invariant`: Produces deterministic/reproducible filesystems

3. **Firmware Partition Population:**
   ```bash
   cp ${ti-uboot-r5}/tiboot3-am64x_sr2-hs-fs-evm.bin firmware/tiboot3.bin
   cp ${ti-uboot-a53}/tispl.bin firmware/tispl.bin
   cp ${ti-uboot-a53}/u-boot.img firmware/u-boot.img
   cp ${./uEnv.txt} firmware/uEnv.txt
   cp ${fitImage} firmware/fitImage.itb
   ```
   - Uses `mcopy` and `mmd` (mtools) to copy files into FAT image
   - All timestamps set to 2000-01-01 for reproducibility

4. **Verification & Assembly:**
   ```bash
   fsck.vfat -vn firmware_part.img  # Verify filesystem integrity
   dd conv=notrunc if=firmware_part.img of=$img seek=$START1 count=$SECTORS1
   dd conv=notrunc if=second_part.img of=$img seek=$START2 count=$SECTORS2
   ```

#### **U-Boot Environment** (`sd-image/uEnv.txt`)

```
boot_targets=mmc1 mmc0
bootdelay=1
uenvcmd=echo "Booting FIT from mmc 1:1..."; load mmc 1:1 ${addr_fit} fitImage.itb; bootm ${addr_fit}#board-0
```

- `mmc 1:1` = MMC device 1, partition 1 (the FIRMWARE partition)
- `${addr_fit}` = U-Boot variable containing FIT load address (set by AM64x defaults)
- `bootm` = U-Boot command to boot from a multi-image (FIT)
- `#board-0` = explicitly select the FIT configuration (avoids a warning U-Boot prints when falling back to the default config)

---

## Boot Sequence

### Complete Boot Flow:

```
1. Power On
   │
   ▼
2. ROM Bootloader (in SoC)
   - Reads tiboot3.bin from SD card (first 2 MiB or partition 1)
   - Loads to internal SRAM
   - Executes on R5 core
   │
   ▼
3. tiboot3.bin (R5 U-Boot SPL)
   - Initializes DDR memory
   - Loads tispl.bin from SD card
   - Jumps to tispl.bin
   │
   ▼
4. tispl.bin (A53 U-Boot SPL)
   - Loads bl31.bin (ARM Trusted Firmware) to EL3
   - Loads tee-raw.bin (OP-TEE) to secure world
   - Loads u-boot.img to non-secure world
   - Jumps to u-boot.img
   │
   ▼
5. u-boot.img (A53 U-Boot Main)
   - Reads uEnv.txt for boot configuration
   - Executes: load mmc 1:1 ${addr_fit} fitImage.itb
   - Loads FIT image to address 0x82000000
   - Verifies FIT image hashes/signatures
   - Executes: bootm ${addr_fit}
   │
   ▼
6. FIT Image Boot (bootm command)
   - Decompresses kernel (LZMA) to memory
   - Passes DTB to kernel (via FDT address)
   - Loads initrd to memory (passed to kernel)
   - Kernel boot parameters from DTB /chosen/bootargs:
     "quiet console=ttyS2,115200n8 panic=-1"
   - Jumps to kernel entry point (0x82000000)
   │
   ▼
7. Linux Kernel
   - Uncompresses itself if needed
   - Initializes hardware using DTB
   - Unpacks the initrd (cpio) into a RAM filesystem
   - Executes /init (mixos-rdinit)
   │
   ▼
8. mixos-rdinit
   - Loop-mounts the EROFS root image read-only at /sysroot
   - Moves /dev, /sys, /proc into it and chroots
   - Execs busybox /sbin/init
   │
   ▼
9. Busybox init (/etc/inittab)
   - sysinit: `mixos sysinit` — mounts tmpfs/cgroups/etc., runs
     `mdev -s` (coldplug + module autoload), sets up /state (tmpfs)
   - wait: mount DATA partition at /tmp/data
   - respawn: fill-station, dropbear, wpa_supplicant, udhcpc,
     syslogd, klogd, crond, mdev -d
   - askfirst: shell on ttyS2
   │
   ▼
10. System Running
   - Fill station WebSocket server on port 9000 (restarted if it exits)
   - SSH access via dropbear
   - Serial console on ttyS2
```

---

## Building the Image

### Command:

From the repository root:

```bash
nix build .#mixosConfigurations.fill-station.config.system.build.sdImage
```

### What happens:

1. Nix evaluates `flake.nix` → imports `nix/mixos-configurations/default.nix`
2. MixOS configuration loads `fill-station/default.nix`
3. Overlay packages are built (or fetched from cache):
   - `fill-station-linux` (kernel)
   - `ti-uboot-r5`, `ti-uboot-a53` (bootloaders)
   - `ti-arm-trusted-firmware`, `ti-optee` (secure firmware)
   - `fill-station` (application)
4. MixOS builds the root filesystem (EROFS) and wraps it in the initrd:
   - busybox userspace + init, generated `/etc/inittab`
   - `bin` packages (dropbear, libgpiod, tcpdump, fill-station, iw, wpa_supplicant, util-linux)
   - all kernel modules in `/lib/modules`
5. FIT image is built:
   - Kernel compressed to LZMA
   - DTB patched with boot parameters
   - All components packaged with `mkimage`
6. SD image is built:
   - Partition table created
   - FAT32 filesystems created
   - Bootloaders and FIT image copied to firmware partition
   - Image assembled with `dd`

### Output:

```
result/
└── fill-station.img
```

A symlink `result` points to the Nix store path containing the complete SD card image.

---

## Flashing to SD Card

```bash
# Identify SD card device (be careful!)
diskutil list  # macOS
lsblk          # Linux

# Unmount if mounted
diskutil unmountDisk /dev/diskN  # macOS
sudo umount /dev/sdX*            # Linux

# Write image
sudo dd if=result/fill-station.img of=/dev/diskN bs=1M status=progress  # macOS
sudo dd if=result/fill-station.img of=/dev/sdX bs=1M status=progress    # Linux

# Sync and eject
sync
diskutil eject /dev/diskN  # macOS
sudo eject /dev/sdX        # Linux
```

---

## Why FIT Images?

### Traditional Boot (what we DON'T use):
```
U-Boot → load kernel → load dtb → load initrd → bootm <kernel> <initrd> <dtb>
```
Problems:
- Three separate load commands
- No integrity verification
- No built-in signing
- Complex boot scripts

### FIT Image Boot (what we DO use):
```
U-Boot → load fitImage.itb → bootm ${addr_fit}
```
Benefits:
- **Single artifact**: One file contains kernel, DTB, initrd
- **Integrity**: Built-in hashing (CRC32, SHA256, etc.)
- **Verified boot**: Support for RSA/ECDSA signatures
- **Configurations**: Multiple boot configs in one image (different boards, debug/release)
- **Compression**: Per-component compression (we use LZMA for kernel)
- **Standard format**: Well-supported by U-Boot, kernel tool (`mkimage`)

### Security Features (available but not fully utilized):
- RSA signature verification (we use CRC32 currently, upgradable)
- Key revocation support
- Chain of trust from ROM → U-Boot → FIT → Kernel

---

## Key Files Reference

| File | Purpose |
|------|---------|
| `flake.nix` | Top-level Nix flake, defines all outputs |
| `nix/mixos-configurations/default.nix` | MixOS configuration entry point |
| `nix/mixos-configurations/fill-station/default.nix` | Fill station system configuration |
| `nix/mixos-configurations/fill-station/fit/default.nix` | FIT image build entry |
| `nix/mixos-configurations/fill-station/fit/build-fit-image.nix` | FIT image build script |
| `nix/mixos-configurations/fill-station/fit/fitImage.its` | FIT image tree source |
| `nix/overlays/by-name/crt/fillstation-dtbo/` | Device tree overlay merged into the stock SK DTB |
| `nix/mixos-configurations/fill-station/sd-image/default.nix` | SD image build entry |
| `nix/mixos-configurations/fill-station/sd-image/build-sd-image.nix` | SD image build script |
| `nix/mixos-configurations/fill-station/sd-image/uEnv.txt` | U-Boot environment |
| `nix/overlays/by-name/crt/fill-station-linux/package.nix` | Custom kernel package |
| `nix/overlays/by-name/crt/ti-uboot-r5/package.nix` | R5 U-Boot package |
| `nix/overlays/by-name/crt/ti-uboot-a53/package.nix` | A53 U-Boot package |
| `nix/overlays/by-name/crt/ti-arm-trusted-firmware/package.nix` | ARM TF package |
| `nix/overlays/by-name/crt/ti-optee/package.nix` | OP-TEE package |
| `nix/overlays/by-name/crt/fill-station/package.nix` | Fill station binary package |

---

## Customization Guide

### Change Kernel Config:
Edit `nix/overlays/by-name/crt/fill-station-linux/kernel.config`.
Ensure Wi-Fi drivers are built-in for connectivity:
```
CONFIG_WL18XX=y
CONFIG_WLCORE=y
CONFIG_MAC80211=y
```

### Change Boot Parameters:
Edit `nix/mixos-configurations/fill-station/fit/build-fit-image.nix`:
```nix
env = {
  kernelParams = toString [
    "debug"  # or "quiet"
    "console=ttyS2,115200n8"
    "panic=-1"
    "firmware_class.path=/etc/lib/firmware" # Location of Wi-Fi firmware
    # Add more parameters here
  ];
};
```

### Change Device Tree:
Edit the overlay in `nix/overlays/by-name/crt/fillstation-dtbo/src/` (see [DTBO_BUILDER.md](DTBO_BUILDER.md) / [PRU_HEADER.md](PRU_HEADER.md)).
For properties an overlay can't delete, add an `fdtput -d` in `build-fit-image.nix`.

### Modify Init Processes:
Edit `nix/mixos-configurations/fill-station/default.nix`:
```nix
init = {
  my-service = {
    action = "respawn";  # or "once", "askfirst"
    process = "/path/to/binary";
  };
};
```

### Add Binaries to Image:
Edit `nix/mixos-configurations/fill-station/default.nix`:
```nix
bin = [
  pkgs.mypackage
  # ... existing packages
];
```

### Change Partition Sizes:
Edit `nix/mixos-configurations/fill-station/sd-image/build-sd-image.nix`:
```nix
gapMiB = 2;
firmwareSizeMiB = 128;
secondPartitionSizeMiB = 1024;
```

---

## Troubleshooting

### Build fails with kernel errors:
- Check `kernel.config` for invalid options
- Ensure all required kernel features are enabled (e.g., `CONFIG_OF`, `CONFIG_ARM64`)

### U-Boot doesn't boot:
- Verify SD card was written correctly: `fdisk -l result/fill-station.img`
- Check serial console output on `ttyS2` (115200 baud)
- Ensure correct `tiboot3.bin` variant for your board (GP vs HS-FS)

### FIT image fails to load:
- Check `uEnv.txt` has correct MMC device and partition
- Verify load address `0x82000000` doesn't conflict with other memory regions
- Run `mkimage -l fitImage.itb` to inspect FIT image structure

### System boots but application doesn't start:
- Check init config: `cat /etc/inittab` on the board (or inspect `nix build .#mixosConfigurations.fill-station.config.system.build.root`)
- The app runs with stdout/stderr on `/dev/null` (inittab `null::`); to see its output, run `fill-station` manually over SSH
- Verify init configuration in `default.nix`
- Check serial console for error messages

### Reproducibility issues:
- All timestamps forced to 2000-01-01
- Use `--invariant` flag for FAT filesystems
- Nix content-addressing ensures reproducible builds
- If builds differ, check for non-deterministic tool versions

---

## Further Reading

- [U-Boot FIT Image Format](https://docs.u-boot.org/en/latest/usage/fit/index.html)
- [MixOS Documentation](https://github.com/jmbaur/mixos)
- [TI AM64x Technical Reference](https://www.ti.com/product/AM6442)
- [Device Tree Specification](https://www.devicetree.org/)
- [Nix Flakes](https://nixos.wiki/wiki/Flakes)
- [ARM Trusted Firmware](https://www.trustedfirmware.org/)
