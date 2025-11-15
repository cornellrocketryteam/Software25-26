{
  config,
  lib,
  pkgs,
  ...
}:
{
  nixpkgs.buildPlatform = "aarch64-linux";
  nixpkgs.hostPlatform.config = "aarch64-unknown-linux-musl";

  boot.kernel = pkgs.linuxKernel.manualConfig {
    inherit (pkgs.linux_latest) src version modDirVersion;
    configfile = ./kernel.config;
  };

  init.shell = {
    tty = "ttyS2";
    action = "askfirst";
    process = "/bin/sh";
  };

  init.ssh-keygen = {
    action = "wait";
    process = pkgs.writeScript "ssh-keygen" ''
      #!/bin/sh

      mkdir -p /var/empty /etc/ssh
      touch /var/log/lastlog
      if [[ ! -f /etc/ssh/host_key ]]; then
        ${lib.getExe' pkgs.dropbear "dropbearkey"} -t ed25519 /etc/ssh/host_key";
      fi
    '';
  };

  init.sshd = {
    action = "respawn";
    process = "${lib.getExe' pkgs.dropbear "dropbear"} -F -r /etc/ssh/host_key";
  };

  bin = [
    pkgs.dropbear
  ];

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

  mixos.testing.enable = false;
}
