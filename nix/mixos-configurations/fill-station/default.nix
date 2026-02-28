{
  config,
  lib,
  pkgs,
  ...
}:
{
  imports = [
    ./fit
    ./sd-image
  ];

  nixpkgs.buildPlatform = "aarch64-linux"; # TODO: Replace and sense current build platform
  nixpkgs.hostPlatform.config = "aarch64-unknown-linux-musl";

  boot.kernel = pkgs.crt.fill-station-linux;

  etc."dropbear".source = pkgs.emptyDirectory;
  etc."hosts".source = pkgs.writeText "etc-hosts" ''
    127.0.0.1      localhost
    ::1            localhost
  '';

  init = {
    shell = {
      tty = "ttyS2";
      action = "askfirst";
      process = "/bin/sh";
    };

    sshd = {
      action = "respawn";
      process = "${lib.getExe' pkgs.crt.dropbear-minimal "dropbear"} -F -R";
    };

    fill-station = {
      action = "once";
      process = lib.getExe pkgs.crt.fill-station;
    };

    watchdog = {
      action = "respawn";
      process = "/bin/watchdog -F /dev/watchdog";
    };

    wpa_supplicant = {
      action = "respawn";
      process = "${lib.getExe' pkgs.wpa_supplicant "wpa_supplicant"} -i wlan0 -c /etc/wpa_supplicant.conf";
    };

    mount_data = {
      action = "wait";
      process = "sh -c 'mkdir -p /tmp/data && (mount -t vfat -L DATA /tmp/data || mount -t vfat /dev/mmcblk1p2 /tmp/data || mount -t vfat /dev/mmcblk0p2 /tmp/data)'";
    };

    dhcp = {
      action = "respawn";
      process = "${lib.getExe' pkgs.busybox "udhcpc"} -f -i wlan0";
    };
  };

  bin = [
    pkgs.crt.dropbear-minimal
    pkgs.libgpiod
    pkgs.tcpdump
    pkgs.crt.fill-station
    pkgs.iw
    pkgs.wpa_supplicant
    pkgs.util-linux
  ];

  etc."lib/firmware".source = pkgs.runCommand "wl18xx-firmware" { } ''
    mkdir -p $out/ti-connectivity
    cp ${pkgs.crt.ti-linux-firmware}/ti-connectivity/wl18xx-fw-4.bin $out/ti-connectivity/wl18xx-fw-4.bin
  '';

  etc."wpa_supplicant.conf".source = pkgs.writeText "wpa_supplicant.conf" ''
    network={
      ssid="CornellRocketry-Fill"
      psk="Rocketry2526"
    }
  '';

  users.root = {
    uid = 0;
    gid = 0;
    shell = "/bin/sh";
    home = "/";
  };
  groups.root.id = 0;

  etc."shadow".source = pkgs.writeText "fill-station-shadow" ''
    ${config.users.root.name}:$6$cy4JUqDYWZowMaLn$oefiZuEJvrHqU3zB33WKrLxaBrsh8mdYqLvtZHP8X1b48E3MYGAYJ3vXtL9x83AI8H6TVO9rvBcsq7bu11li20:1::::::
  '';
}
