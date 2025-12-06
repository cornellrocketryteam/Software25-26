{ linuxKernel, linux_latest, dtc }:
(linuxKernel.manualConfig {
  inherit (linux_latest) src version modDirVersion;
  configfile = ./kernel.config;
}).overrideAttrs (oldAttrs: {
  nativeBuildInputs = (oldAttrs.nativeBuildInputs or []) ++ [ dtc ];
  
  # Copy our custom device tree overlay into the kernel source tree
  postPatch = (oldAttrs.postPatch or "") + ''
    echo "Installing CRT device tree overlay..."
    cp ${./am642-sk-crt.dtso} arch/arm64/boot/dts/ti/k3-am642-sk-crt.dtso
    
    # Add our overlay to the device tree Makefile
    echo "Updating device tree Makefile..."
    if ! grep -q "k3-am642-sk-crt.dtbo" arch/arm64/boot/dts/ti/Makefile; then
      sed -i '/k3-am642-sk.dtb/a dtb-$(CONFIG_ARCH_K3) += k3-am642-sk-crt.dtbo' arch/arm64/boot/dts/ti/Makefile
    fi
  '';
  
  # Install the compiled overlay
  postInstall = (oldAttrs.postInstall or "") + ''
    echo "Installing CRT device tree overlay..."
    if [ -f "$out/dtbs/ti/k3-am642-sk-crt.dtbo" ]; then
      echo "Successfully built k3-am642-sk-crt.dtbo"
    else
      echo "Warning: k3-am642-sk-crt.dtbo was not built"
      ls -la "$out/dtbs/ti/" || true
    fi
  '';
})
