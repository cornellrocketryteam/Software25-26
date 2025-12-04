inputs:
inputs.nixpkgs.lib.mapAttrs (
  _: pkgs:
  # Copied from here: https://github.com/NixOS/nixpkgs/blob/nixos-25.11/pkgs/by-name/ni/nixfmt-tree/package.nix#L60
  pkgs.treefmt.withConfig {
    settings = {
      on-unmatched = "info";

      formatter = {
        nixfmt = {
          command = "nixfmt";
          includes = [ "*.nix" ];
        };
        rustfmt = {
          command = "rustfmt";
          options = [ "--edition=2024" ];
          includes = [ "*.rs" ];
        };
        yamlfmt = {
          command = "yamlfmt";
          includes = [
            "*.yaml"
            "*.yml"
          ];
        };
      };
    };

    runtimeInputs = with pkgs; [
      nixfmt
      rustfmt
      yamlfmt
    ];
  }
) inputs.self.legacyPackages
