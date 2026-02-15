inputs:
inputs.nixpkgs.lib.mapAttrs (_: pkgs: {
  default = pkgs.mkShell {
    packages = with pkgs; [
      (fenix.combine [
        fenix.stable.defaultToolchain
        fenix.targets.aarch64-unknown-linux-musl.stable.rust-std
        fenix.targets."thumbv8m.main-none-eabihf".stable.rust-std
      ])
      rust-analyzer
    ];
  };
}) inputs.self.legacyPackages
