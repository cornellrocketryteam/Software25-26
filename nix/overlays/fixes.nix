final: prev: {
  # D-Bus support alone drags dbus -> systemd-minimal, libx11, libxcb, audit
  # into the image (~30 MB uncompressed). We only drive it via the config file.
  wpa_supplicant = prev.wpa_supplicant.override {
    dbusSupport = false;
    withPcsclite = false;
    withReadline = false;
  };

  libgpiod = prev.libgpiod.overrideAttrs (oldAttrs: {
    configureFlags =
      (oldAttrs.configureFlags or [ ])
      ++ final.lib.optionals final.stdenv.hostPlatform.isMusl [
        # AC_FUNC_MALLOC is broken on cross builds.
        "ac_cv_func_malloc_0_nonnull=yes"
        "ac_cv_func_realloc_0_nonnull=yes"
      ];
  });
}
