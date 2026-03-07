inputs:
let
  inherit (inputs.nixpkgs.lib) mapAttrs mkDefault filterAttrs;
in
mapAttrs (
  name: _:
  inputs.mixos.lib.mixosSystem {
    modules = [
      (
        { pkgs, ... }:
        {
          nixpkgs.nixpkgs = inputs.nixpkgs;
          nixpkgs.overlays = [
            inputs.self.overlays.default
            inputs.mixos.overlays.default
            inputs.fenix.overlays.default
          ];
          etc."hostname".source = mkDefault (pkgs.writeText "hostname" name);
        }
      )
      ./${name}/default.nix
    ];
  }
) (filterAttrs (_: type: type == "directory") (builtins.readDir ./.))
