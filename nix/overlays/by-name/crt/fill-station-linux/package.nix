{ linuxManualConfig, linux_latest }:
linuxManualConfig {
  inherit (linux_latest) src version;
  configfile = ./kernel.config;
}
