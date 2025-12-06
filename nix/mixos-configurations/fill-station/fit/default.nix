{ config, pkgs, ... }:
{
  system.build.fitImage = pkgs.callPackage ./build-fit-image.nix {
    kernel = "${config.boot.kernel}/Image";
    dtb = ./k3-am642-sk-fill-station.dtb;
    initrd = "${config.system.build.initrd}/initrd";
  };
}
