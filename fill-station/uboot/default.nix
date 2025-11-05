{
  pkgsCross,
  fetchgit,
  symlinkJoin,
}:
let
  pkgsCross32 = pkgsCross.armv7l-hf-multiplatform;
  pkgsCross64 = pkgsCross.aarch64-multiplatform;

  ti-linux-firmware = fetchgit {
    url = "https://git.ti.com/git/processor-firmware/ti-linux-firmware.git";
    branchName = "ti-linux-firmware";
    rev = "11.02.02";
    hash = "sha256-fTqy2imcfZD68a0Dcvzx/jkBFPAlQNWudDUTO1mJaN4=";
  };

  tfa = pkgsCross64.callPackage ./tfa.nix { };

  optee = pkgsCross64.callPackage ./optee.nix {
    inherit pkgsCross32;
  };

  uboot-r5 = pkgsCross32.callPackage ./uboot-r5.nix {
    inherit ti-linux-firmware;
  };

  uboot-a53 = pkgsCross64.callPackage ./uboot-a53.nix {
    inherit tfa optee ti-linux-firmware;
  };

  uboot-all = symlinkJoin {
    name = "uboot";
    paths = [
      uboot-r5
      uboot-a53
    ];
  };
in
{
  r5 = uboot-r5;
  a53 = uboot-a53;
  all = uboot-all;
}
