#include "hardware.hpp"
#include <filesystem>
#include <spdlog/spdlog.h>

constexpr const char* gpio_chip_path = "/dev/gpiochip1";

Hardware::Hardware()
    : chip(gpio_chip_path), ig1(chip, 18, 16), ig2(chip, 24, 22) {
  spdlog::info("Hardware initialized with GPIO chip at {}", gpio_chip_path);
  spdlog::info("Igniter 1 configured: continuity_pin=18, signal_pin=16");
  spdlog::info("Igniter 2 configured: continuity_pin=24, signal_pin=22");
}
