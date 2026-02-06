inputs: final: prev: {
  bash = inputs.nixpkgs-2411.legacyPackages.${final.system}.bash;
}
