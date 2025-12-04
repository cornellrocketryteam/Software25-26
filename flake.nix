{
  description = "Cornell Rocketry Nix";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/release-25.11";
    mixos = {
      url = "github:jmbaur/mixos";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs: {
    devShells = import ./nix/dev-shells inputs;
    legacyPackages = import ./nix/legacy-packages inputs;
    mixosConfigurations = import ./nix/mixos-configurations inputs;
    overlays = import ./nix/overlays inputs;
  };
}
