{ config, pkgs, ... }:
{
  system.build.fitImage = pkgs.callPackage ./build-fit-image.nix {
    kernel = "${config.boot.kernel}/Image";
    dtb = "${config.boot.kernel}/dtbs/ti/k3-am642-sk.dtb";
    initrd = "${config.system.build.initrd}/initrd";
  };
}