{
  stdenvNoCC,
  dtc,
  clang,
  python3,
}:
stdenvNoCC.mkDerivation {
  name = "fillstation-pinmux-overlay";

  src = ./src;

  nativeBuildInputs = [
    dtc
    clang
    python3
  ];

  buildPhase = ''
    # Step 4: Preprocess with clang
    clang -E -P -x assembler-with-cpp -I . \
      k3-am64-fillstation-pinmux-overlay.dts \
      -o overlay.pp.dts

    # Step 5: Fold expressions into constants
    python3 ${./fold-expressions.py} overlay.pp.dts overlay.clean.dts

    # Step 6: Compile overlay
    dtc -@ -I dts -O dtb \
      -o k3-am64-fillstation-pinmux-overlay.dtbo \
      overlay.clean.dts

    # Step 7: Verify
    echo "Verifying overlay metadata..."
    fdtdump k3-am64-fillstation-pinmux-overlay.dtbo | grep -q "__symbols__" || \
      (echo "ERROR: Missing __symbols__" && exit 1)
    fdtdump k3-am64-fillstation-pinmux-overlay.dtbo | grep -q "__fixups__" || \
      (echo "ERROR: Missing __fixups__" && exit 1)
    echo "Overlay verification passed!"
  '';

  installPhase = ''
    install -Dm644 k3-am64-fillstation-pinmux-overlay.dtbo $out/k3-am64-fillstation-pinmux-overlay.dtbo
  '';
}
