# https://docs.u-boot.org/en/latest/board/ti/am64x_evm.html
# https://software-dl.ti.com/processor-sdk-linux/esd/AM64X/08_06_00_42/exports/docs/linux/Foundational_Components/U-Boot/UG-General-Info.html#build-u-boot-label

{
  description = "Fill Station";

  inputs = {
    # TODO: Pin nixpkgs to 25.11 when it comes out
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    mixos = {
      url = "github:jmbaur/mixos";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      forEachSupportedSystem =
        f:
        inputs.nixpkgs.lib.genAttrs supportedSystems (
          system:
          f {
            pkgs = import inputs.nixpkgs { inherit system; };
          }
        );
    in
    {
      packages = forEachSupportedSystem (
        { pkgs }:
        let
          uboot = import ./uboot { inherit pkgs; };
          uboot-all = pkgs.symlinkJoin {
            name = "uboot";
            paths = [
              uboot.r5
              uboot.a53
            ];
          };
          sd-image = import ./sd-image.nix { 
            inherit pkgs uboot;
            fill-station = inputs.self.mixosConfigurations.fill-station;
          };
        in
        {
          uboot-r5 = uboot.r5;
          uboot-a53 = uboot.a53;
          uboot = uboot-all;
          sd-image = sd-image;
          default = sd-image;
        }
      );

      mixosConfigurations.fill-station = import ./linux inputs;
    };
}
