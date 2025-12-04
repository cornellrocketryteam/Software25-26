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

  boot.kernel = pkgs.crt.custom-linux;

  etc."dropbear".source = pkgs.emptyDirectory;
  init = {
    shell = {
      tty = "ttyS2";
      action = "askfirst";
      process = "/bin/sh";
    };

    sshd = {
      action = "respawn";
      process = "${lib.getExe' pkgs.dropbear "dropbear"} -F -R";
    };

    fill-station = {
      action = "once";
      process = lib.getExe pkgs.crt.fill-station;
    };
  };

  bin = [
    pkgs.crt.dropbear-minimal
    pkgs.libgpiod
    pkgs.tcpdump
    pkgs.crt.fill-station
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
}
