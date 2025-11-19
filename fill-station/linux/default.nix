inputs:
inputs.mixos.lib.mixosSystem {
  modules = [
    (
      { ... }:
      {
        nixpkgs.nixpkgs = inputs.nixpkgs;
      }
    )

    ./mixosConfiguration.nix
    ./mixos-fit-image.nix
  ];
}
