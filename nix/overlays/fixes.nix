final: prev: {
  libgpiod = prev.libgpiod.overrideAttrs (oldAttrs: {
    configureFlags = (oldAttrs.configureFlags or []) ++ final.lib.optionals final.stdenv.hostPlatform.isMusl [
      # AC_FUNC_MALLOC is broken on cross builds.
      "ac_cv_func_malloc_0_nonnull=yes"
      "ac_cv_func_realloc_0_nonnull=yes"
    ];
  });
}
