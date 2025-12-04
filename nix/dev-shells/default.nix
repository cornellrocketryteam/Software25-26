inputs:
inputs.nixpkgs.lib.mapAttrs (
  system: pkgs: {
    default = pkgs.mkShell {
      packages = with pkgs; [
        (fenix.combine [
          fenix.stable.defaultToolchain
          fenix.targets.aarch64-unknown-linux-musl.stable.rust-std
        ])
        rust-analyzer
      ];
    };
  }
) inputs.self.legacyPackages