# Wi-Fi Setup Guide

This document details the Wi-Fi configuration for the TI SK-AM64B fill station system.

## Hardware Support
- **Chipset**: TI WL1837MOD (WiLink 8)
- **Interface**: SDIO
- **Drivers**: `wl18xx`, `wlcore_sdio` (kernel built-in)
- **Firmware**: `wl18xx-fw-4.bin`

## Automatic Connection
The system is configured to **automatically connect** to the configured network on boot.

### Current Configuration
- **SSID**: `CornellRocketry`
- **Password**: `Rocketry2526`
- **Configuration File**: `/etc/wpa_supplicant.conf` (generated at build time)

### How It Works
1. **Init Process**: The `init` system spawns `wpa_supplicant` and `udhcpc` on boot (see `nix/mixos-configurations/fill-station/default.nix`).
2. **Connection**: `wpa_supplicant` connects to the network defined in `/etc/wpa_supplicant.conf`.
3. **DHCP**: `udhcpc` requests an IP address from the router.

## Changing Network Credentials
To change the default network:

1. Edit `nix/mixos-configurations/fill-station/default.nix`.
2. Locate the `wpa_supplicant.conf` generation section:
   ```nix
   etc."wpa_supplicant.conf".source = pkgs.writeText "wpa_supplicant.conf" ''
     network={
       ssid="NewSSID"
       psk="NewPassword"
     }
   '';
   ```
3. Rebuild the image (`nix build ...`) and re-flash the SD card.

## Manual Troubleshooting

If Wi-Fi is not working, use the serial console to debug:

### 1. Check Interface
Verify `wlan0` exists:
```sh
iw dev
```
If not found, check boot logs for firmware errors:
```sh
dmesg | grep wlcore
```
*Common error: `wlcore: ERROR could not get firmware...` -> This means the kernel cannot find `/etc/lib/firmware/ti-connectivity/wl18xx-fw-4.bin`.*

### 2. Scan for Networks
```sh
ip link set wlan0 up
iw wlan0 scan | grep SSID
```

### 3. Manual Connection
Stop the auto-connect service (if running) and connect manually:
```sh
pkill wpa_supplicant
wpa_passphrase "SSID" "PASSWORD" > /tmp/wifi.conf
wpa_supplicant -B -i wlan0 -c /tmp/wifi.conf
udhcpc -i wlan0
```

## System Requirements
The build system ensures:
- Kernel drivers are compiled in (`CONFIG_WL18XX=y`).
- Firmware is copied to `/etc/lib/firmware`.
- Kernel boot parameter `firmware_class.path=/etc/lib/firmware` is set.
- `iw`, `wpa_supplicant`, and `udhcpc` are installed.
