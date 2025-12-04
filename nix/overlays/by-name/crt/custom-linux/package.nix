{ linuxKernel, linux_latest }:
linuxKernel.manualConfig {
  inherit (linux_latest) src version modDirVersion;
  configfile = ./kernel.config;
}