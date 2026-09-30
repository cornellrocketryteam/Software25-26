# Using the PRU Header Pins (GPIO / I2C / PWM / SPI)

The SK-AM64B PRU header brings out PRU-subsystem (ICSSG) pads. We don't run PRU
firmware — we just re-mux those pads to ordinary Linux peripherals. Every change
goes through the device tree overlay described in [DTBO_BUILDER.md](DTBO_BUILDER.md).

All overlay files live in `nix/overlays/by-name/crt/fillstation-dtbo/src/`.

## Which pads are usable

| Pads (offset)   | Signals                  | Usable? |
|-----------------|--------------------------|---------|
| `0x160`–`0x1f4` | `PRG0_PRU0/1_GPOx`       | Yes (mode 7 = `GPIO1_0` … `GPIO1_37`) |
| `0x1f8`, `0x1fc`| `PRG0_PRU1_GPO18/19`     | **No** — base SK DTB uses them for `mdio1` (Ethernet PHY MDIO) |
| `0x200`, `0x204`| `PRG0_MDIO0_MDIO/MDC`    | Yes (`GPIO1_40/41`) |
| `0xb8`–`0x15c`  | `PRG1_*`                 | **No** — both RGMII Ethernet ports use them |

Also avoid pads the fill station already uses (see `sysconfig-pinmux.dtsi`).
Which physical header pin each ball goes to is in the SK-AM64B schematic — check
it before wiring.

## General workflow (every peripheral type)

1. **SysConfig**: open the fill-station SysConfig project, add the peripheral,
   and assign it to PRU-header balls. SysConfig flags pin conflicts and tells you
   which peripheral instances (e.g. which I2C/ePWM) can reach which balls.
2. **Export** the device tree `.dtsi`. Turn off "not configured" pins (or delete
   every `*_notconf` group) — they're filler and some overlap pads we use.
3. **Copy the new pin group(s)** into `src/sysconfig-pinmux.dtsi` inside
   `&main_pmx0 { ... }` (or replace the whole file if the export is the full
   project). Give labels meaningful names (`pru_test_pins_default`, not
   `mygpio10_pins_default`).
4. **Attach the group** to a consumer in `src/k3-am64-fillstation-pinmux-overlay.dts`
   — recipes below. A pin group that nothing references is **never applied**.
5. **Kernel config** — only if the driver isn't built in (see each recipe).
   Change it in `nix/overlays/by-name/crt/fill-station-linux/kernel.config`.
   Use `=y`, not `=m`. `=m` modules *are* shipped in the image, but they only
   load via mdev modalias autoloading (nothing loads them explicitly), so a
   built-in driver is the only way to guarantee it's there at boot.
6. **Build check** — add an assertion in
   `nix/mixos-configurations/fill-station/fit/build-fit-image.nix` next to the
   existing ones, e.g.
   ```sh
   expect "main_i2c3 enabled" "$(fdtget dtb-merged /bus@f4000/i2c@20030000 status)" okay
   ```
7. **Build & flash**: `nix build .#mixosConfigurations.fill-station.config.system.build.sdImage`
8. **Verify on the board**, then add app code (see [ADDING_FEATURES.md](ADDING_FEATURES.md)).

If a new SysConfig export fails to compile with an unknown `PIN_*` macro, add
the definition to `src/k3.h`.

### Reading a pin value

`AM64X_IOPAD(offset, flags, mode)` becomes `<offset flags|mode>` in the DTB:

| Flags                 | Value     | Meaning |
|-----------------------|-----------|---------|
| `PIN_OUTPUT`          | `0x10000` | output, no pull |
| `PIN_OUTPUT_PULLDOWN` | `0x00000` | output, pull-down |
| `PIN_INPUT`           | `0x50000` | input enabled, no pull (output still works for GPIO) |
| `PIN_INPUT_PULLUP`    | `0x60000` | input, pull-up |
| `PIN_INPUT_PULLDOWN`  | `0x40000` | input, pull-down |

The low bits are the mux mode: `7` = GPIO, others are the peripheral function
SysConfig picked. E.g. `0x164 0x50007` = pad 0x164, input-enabled, GPIO.

---

## Recipe: GPIO

**dtsi** (mode 7):
```dts
my_pru_gpio_pins_default: my-pru-gpio-default-pins {
    pinctrl-single,pins = <
        AM64X_IOPAD(0x0168, PIN_INPUT|PIN_DRIVE_STRENGTH_NOMINAL, 7) /* (U2) PRG0_PRU0_GPO2.GPIO1_2 */
    >;
};
```

**overlay**: add the label to the pin-controller hog:
```dts
&main_pmx0 {
    pinctrl-names = "default";
    pinctrl-0 = <&allgpio0_pins_default &allgpio1_pins_default
                 &pru_test_pins_default &my_pru_gpio_pins_default>;
};
```

**Kernel**: nothing (`GPIO_DAVINCI`, `GPIO_CDEV` are built in).

**Line numbers**: `GPIOa_n` → controller `a`, line `n`.

| Controller | DT node        | Chip (current image) | `hardware.rs` const |
|------------|----------------|----------------------|---------------------|
| GPIO0      | `gpio@600000`  | `gpiochip1`          | `GPIO_CHIP0` |
| GPIO1      | `gpio@601000`  | `gpiochip2`          | `GPIO_CHIP1` |

All PRU-header GPIOs are on GPIO1. Confirm chip names with `gpiodetect`.

**Test**:
```sh
gpioinfo -c gpiochip2 | grep 'line   2:'   # should be unused
gpioset -c gpiochip2 2=1                   # drive high (Ctrl-C releases)
gpioget -c gpiochip2 2                     # read back (needs PIN_INPUT*)
```

**App**: use the existing pattern, e.g. `Chip::new(GPIO_CHIP1)` and pass the
line number to the component (see `src/hardware.rs`).

---

## Recipe: I2C

**Instances** (main domain — the PRU pads are all main domain):

| Label       | Node               | Current use |
|-------------|--------------------|-------------|
| `main_i2c0` | `i2c@20000000`     | enabled by base SK DTB (board devices) — don't repurpose |
| `main_i2c1` | `i2c@20010000`     | enabled by base SK DTB (board devices) — don't repurpose |
| `main_i2c2` | `i2c@20020000`     | fill-station sense board (ADS1015s) |
| `main_i2c3` | `i2c@20030000`     | **free** |

Use SysConfig to see whether the instance you want can be muxed to PRU-header balls.

**dtsi**: the SCL/SDA group SysConfig generates (not mode 7).

**overlay**: same shape as the existing `&main_i2c2` block:
```dts
&main_i2c3 {
    status = "okay";
    pinctrl-names = "default";
    pinctrl-0 = <&my_i2c3_pins_default>;
    clock-frequency = <400000>;
};
```

**Kernel**: nothing (`I2C_OMAP`, `I2C_CHARDEV` are built in).

**Device path**: only `i2c0`/`i2c1` have DT aliases, so other buses get their
`/dev/i2c-N` number at boot. Find it by address:
```sh
ls -l /sys/bus/i2c/devices/ | grep 20030000   # -> i2c-N
```
Then use `/dev/i2c-N` in the app, like `I2C_BUS` in `src/hardware.rs`.

Don't forget external pull-ups on SCL/SDA if the device board doesn't have them.

---

## Recipe: PWM

**Instances**: `epwm0`–`epwm8` (`pwm@23000000` … `pwm@23080000`, two outputs
each: A = channel 0, B = channel 1) and `ecap0`–`ecap2` (`pwm@23100000` …).
`epwm4` is already enabled for `EHRPWM4_B` on `GPMC0_WEn`.

**dtsi**: the `EHRPWMn_A/B` group SysConfig generates.

**overlay**: same shape as the existing `&epwm4` block:
```dts
&epwm5 {
    status = "okay";
    pinctrl-names = "default";
    pinctrl-0 = <&my_epwm5_pins_default>;
};
```

**Kernel**: ePWM (`PWM_TIEHRPWM`) is built in. eCAP (`PWM_TIECAP`) is currently
`=m` — change it to `=y` before using an `ecapN` instance.

**Test** (sysfs; chip numbers depend on probe order):
```sh
ls -l /sys/class/pwm/                     # find the pwmchipX -> ...23050000.pwm
cd /sys/class/pwm/pwmchipX
echo 0 > export                           # channel 0 = A, 1 = B
echo 1000000 > pwm0/period                # ns (1 kHz)
echo 500000  > pwm0/duty_cycle            # 50 %
echo 1 > pwm0/enable
```

---

## Recipe: SPI

**Instances**: `main_spi0`–`main_spi4`, all disabled in the base DTB. The
overlay explicitly disables `main_spi0`/`main_spi1` because their normal pads
are used as GPIOs (`GPIO1_42`–`51`). If you use one of those instances on
PRU-header pads, delete its `status = "disabled"` block and make sure its old
pads aren't also claimed.

**overlay** (user-space access via spidev):
```dts
&main_spi2 {
    status = "okay";
    pinctrl-names = "default";
    pinctrl-0 = <&my_spi2_pins_default>;
    #address-cells = <1>;
    #size-cells = <0>;

    spidev@0 {
        compatible = "rohm,dh2228fv";   /* a compatible the spidev driver binds to */
        reg = <0>;                      /* chip select 0 */
        spi-max-frequency = <1000000>;
    };
};
```

**Kernel**: `SPI_OMAP24XX` and `SPI_SPIDEV` are currently `=m` — change both
to `=y`.

**Test**: `ls /dev/spidev*` should show the device after boot.

---

## Troubleshooting

- **Build fails in `dtc` with a syntax error**: a `PIN_*` macro from SysConfig
  isn't defined in `src/k3.h`.
- **Build fails in `fdtoverlay`**: the overlay references a label (e.g.
  `&main_i2c5`) that doesn't exist in the base DTB.
- **Build fails with `DTB check failed`**: an assertion in `build-fit-image.nix`
  didn't match — the overlay didn't produce what you expected.
- **Pin doesn't change on the board**: make sure its group is referenced by a
  `pinctrl-0`; check the applied value with
  `grep <addr> /sys/kernel/debug/pinctrl/f4000.pinctrl-pinctrl-single/pins`
  (needs debugfs mounted; address = `f4000 + offset`, e.g. `f4164`).
- **Peripheral node exists but no device appears**: the driver is probably `=m`
  in the kernel config.
