# https://docs.u-boot.org/en/latest/board/ti/am64x_evm.html
# https://software-dl.ti.com/processor-sdk-linux/esd/AM64X/08_06_00_42/exports/docs/linux/Foundational_Components/U-Boot/UG-General-Info.html#build-u-boot-label

{
  description = "Fill Station";

  # TODO: Pin nixpkgs to 25.11 when it comes out
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs =
    { nixpkgs, ... }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      forEachSupportedSystem =
        f:
        nixpkgs.lib.genAttrs supportedSystems (
          system:
          f {
            pkgs = import nixpkgs { inherit system; };
          }
        );
    in
    {
      packages = forEachSupportedSystem (
        { pkgs }:
        let
          uboot = pkgs.callPackage ./uboot { };

          sd-image = pkgs.callPackage ./sd-image.nix {
            inherit uboot;
          };
        in
        {
          uboot-r5 = uboot.r5;
          uboot-a53 = uboot.a53;
          uboot = uboot.all;
          sd-image = sd-image;
          default = sd-image;
        }
      );
    };
}
