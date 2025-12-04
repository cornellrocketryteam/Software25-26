{ dropbear }:
dropbear.overrideAttrs (oldAttrs: {
  # https://github.com/mkj/dropbear/blob/master/SMALL.md#tips-for-a-small-system
  preConfigure = ''
    makeFlagsArray=(
      VPATH=$(cat $NIX_CC/nix-support/orig-libc)/lib
      PROGRAMS="dropbear"
      MULTI=1
    )
  '';

  # https://github.com/mkj/dropbear/blob/master/MULTI.md#multi-binary-compilation
  postInstall = ''
    ln -s $out/bin/dropbearmulti $out/bin/dropbear
  '';
})