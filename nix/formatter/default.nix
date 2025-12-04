inputs:
inputs.nixpkgs.lib.mapAttrs (
  _: pkgs:
  # Copied from here: https://github.com/NixOS/nixpkgs/blob/nixos-25.11/pkgs/by-name/ni/nixfmt-tree/package.nix#L60
  pkgs.treefmt.withConfig {
    settings = {
      on-unmatched = "info";

      formatter.nixfmt = {
        command = "nixfmt";
        includes = [ "*.nix" ];
      };
    };

    runtimeInputs = with pkgs; [
      nixfmt
    ];
  }
) inputs.self.legacyPackages
