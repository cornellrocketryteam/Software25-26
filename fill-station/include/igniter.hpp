#pragma once

#include <gpiod.hpp>

class Igniter final {
public:
  Igniter(gpiod::chip &chip, gpiod::line::offset continuity_pin,
          gpiod::line::offset signal_pin);

  bool has_continuity();
  void ignite();
  bool is_igniting();

private:
  const gpiod::line::offset continuity_pin;
  const gpiod::line::offset signal_pin;
  gpiod::line_request line;

  static gpiod::line_request request_line(gpiod::chip &chip,
                                          gpiod::line::offset continuity_pin,
                                          gpiod::line::offset signal_pin);
};
