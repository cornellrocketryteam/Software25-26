{ config, pkgs, ... }:
{
  system.build.sdImage = pkgs.callPackage ./build-sd-image.nix {
    fitImage = "${config.system.build.fitImage}/fitImage.itb";
  };
}