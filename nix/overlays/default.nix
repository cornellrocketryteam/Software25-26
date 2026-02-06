inputs: {
  default = inputs.nixpkgs.lib.composeManyExtensions [
    (import ./fixes.nix)
    (import ./bash-fix.nix inputs)
    (
      final: prev:
      prev.lib.packagesFromDirectoryRecursive {
        inherit (final) callPackage;
        inherit (prev) newScope;
        directory = ./by-name;
      }
    )
    (import ./crt.nix)
  ];
}
