# https://docs.u-boot.org/en/latest/board/ti/am64x_evm.html
# https://software-dl.ti.com/processor-sdk-linux/esd/AM64X/08_06_00_42/exports/docs/linux/Foundational_Components/U-Boot/UG-General-Info.html#build-u-boot-label

{
  description = "NixOS + U-Boot for TI SK-AM64B Evaluation Board";

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
          pkgsCross32 = pkgs.pkgsCross.armv7l-hf-multiplatform;
          pkgsCross64 = pkgs.pkgsCross.aarch64-multiplatform;

          ti-linux-firmware = pkgs.fetchgit {
            url = "https://git.ti.com/git/processor-firmware/ti-linux-firmware.git";
            branchName = "ti-linux-firmware";
            rev = "11.02.02";
            hash = "sha256-fTqy2imcfZD68a0Dcvzx/jkBFPAlQNWudDUTO1mJaN4=";
          };

          tfa = pkgsCross64.callPackage ./tfa.nix { };
          optee = pkgsCross64.callPackage ./optee.nix {
            pkgsCross32 = pkgsCross32;
          };

          uboot-r5 = pkgsCross32.callPackage ./uboot-r5.nix {
            ti-linux-firmware = ti-linux-firmware;
          };

          uboot-a53 = pkgsCross64.callPackage ./uboot-a53.nix {
            tfa = tfa;
            optee = optee;
            ti-linux-firmware = ti-linux-firmware;
          };

          uboot-all = pkgs.symlinkJoin {
            name = "uboot-all";
            paths = [
              uboot-r5
              uboot-a53
            ];
          };

          sd-image = pkgs.callPackage ./sd-image.nix {
            uboot-r5 = uboot-r5;
            uboot-a53 = uboot-a53;
          };
        in
        {
          tfa = tfa;
          optee = optee;
          uboot-r5 = uboot-r5;
          uboot-a53 = uboot-a53;
          uboot-all = uboot-all;
          sd-image = sd-image;

          default = sd-image;
        }
      );
    };
}
