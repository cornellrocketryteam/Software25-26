inputs:
inputs.mixos.lib.mixosSystem {
  modules = [
    (
      { ... }:
      {
        nixpkgs.nixpkgs = inputs.nixpkgs;
        nixpkgs.overlays = [
          inputs.mixos.overlays.default
        ];
      }
    )

    ./mixosConfiguration.nix
  ];
}
