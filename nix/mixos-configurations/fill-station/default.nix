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

  boot = {
    kernelPackages = pkgs.linuxPackagesFor pkgs.crt.fill-station-linux;
    firmware = [
      (pkgs.runCommand "wl18xx-firmware" { } ''
        mkdir -p $out/lib/firmware/ti-connectivity
        cp ${pkgs.crt.ti-linux-firmware}/ti-connectivity/wl18xx-fw-4.bin \
           $out/lib/firmware/ti-connectivity/
      '')
    ];
  };

  state = {
    enable = true;
    # TODO: This can become null when we update nixpkgs and mixos
    init = pkgs.writeShellScript "state-init" "";
    fsType = "vfat";
    source = "/dev/mmcblk0p2";
    options = [
      "rw"
      "noatime"
    ];
  };

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

    wpa_supplicant = {
      action = "respawn";
      process = "${lib.getExe' pkgs.wpa_supplicant "wpa_supplicant"} -i wlan0 -c /etc/wpa_supplicant.conf";
    };

    dhcp = {
      action = "respawn";
      process = "/bin/udhcpc -f -i wlan0";
    };

    fill-station = {
      action = "once";
      process = lib.getExe pkgs.crt.fill-station;
    };
  };

  packages = [
    pkgs.crt.dropbear-minimal
    pkgs.libgpiod
    pkgs.tcpdump
    pkgs.crt.fill-station
    pkgs.iw
    pkgs.wpa_supplicant
  ];

  etc."dropbear".source = pkgs.emptyDirectory;
  etc."wpa_supplicant.conf".source = pkgs.writeText "wpa_supplicant.conf" ''
    network={
      ssid="CornellRocketry-Fill"
      psk="Rocketry2526"
    }
  '';
  etc."shadow".source = pkgs.writeText "fill-station-shadow" ''
    ${config.users.root.name}:$6$cy4JUqDYWZowMaLn$oefiZuEJvrHqU3zB33WKrLxaBrsh8mdYqLvtZHP8X1b48E3MYGAYJ3vXtL9x83AI8H6TVO9rvBcsq7bu11li20:1::::::
  '';

  users.root = {
    uid = 0;
    gid = 0;
    shell = "/bin/sh";
    home = "/root";
  };
  groups.root.id = 0;
}
