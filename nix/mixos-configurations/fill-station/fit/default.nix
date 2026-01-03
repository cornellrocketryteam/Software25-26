{ config, pkgs, ... }:
{
  system.build.fitImage = pkgs.callPackage ./build-fit-image.nix {
    kernel = "${config.boot.kernel}/Image";
    dtb = "${config.boot.kernel}/dtbs/ti/k3-am642-sk.dtb";
    dtbOverlay = "${pkgs.crt.fillstation-dtbo}/k3-am64-fillstation-pinmux-overlay.dtbo";
    initrd = "${config.system.build.initrd}/initrd";
  };
}
