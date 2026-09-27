{
  stdenvNoCC,
  dtc,
  clang,
}:
stdenvNoCC.mkDerivation {
  name = "fillstation-pinmux-overlay";

  src = ./src;

  nativeBuildInputs = [
    dtc
    clang
  ];

  buildPhase = ''
    # Preprocess with clang (-undef so predefined macros like `linux` can't
    # leak into the DTS). Pinmux cell expressions are left for dtc to evaluate.
    clang -E -P -undef -nostdinc -x assembler-with-cpp -I . \
      k3-am64-fillstation-pinmux-overlay.dts \
      -o overlay.pp.dts

    # Compile overlay
    dtc -@ -I dts -O dtb \
      -o k3-am64-fillstation-pinmux-overlay.dtbo \
      overlay.pp.dts

    # Verify
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
