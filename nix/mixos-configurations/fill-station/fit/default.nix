{ config, pkgs, ... }:
{
  system.build.fitImage = pkgs.callPackage ./build-fit-image.nix {
    kernel = "${config.boot.kernel}/Image";
    dtb = "${config.boot.kernel}/dtbs/ti/k3-am642-sk.dtb";
    dtbo = "${config.boot.kernel}/dtbs/ti/k3-am642-sk-crt.dtbo";
    initrd = "${config.system.build.initrd}/initrd";
  };
}
