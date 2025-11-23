#include "igniter.hpp"

#include <chrono>
#include <spdlog/spdlog.h>
#include <thread>

gpiod::line_request Igniter::request_line(gpiod::chip &chip,
                                          gpiod::line::offset continuity_pin,
                                          gpiod::line::offset signal_pin) {
  gpiod::line_settings continuity_settings =
      gpiod::line_settings().set_direction(gpiod::line::direction::INPUT);

  gpiod::line_settings signal_settings =
      gpiod::line_settings()
          .set_direction(gpiod::line::direction::OUTPUT)
          .set_output_value(gpiod::line::value::INACTIVE);

  return chip.prepare_request()
      .set_consumer("fill-station-igniter")
      .add_line_settings(continuity_pin, continuity_settings)
      .add_line_settings(signal_pin, signal_settings)
      .do_request();
}

Igniter::Igniter(gpiod::chip &chip, gpiod::line::offset continuity_pin,
                 gpiod::line::offset signal_pin)
    : continuity_pin(continuity_pin), signal_pin(signal_pin),
      line(request_line(chip, continuity_pin, signal_pin)) {}

bool Igniter::has_continuity() {
  return line.get_value(continuity_pin) == gpiod::line::value::ACTIVE;
}

void Igniter::ignite() {
  line.set_value(signal_pin, gpiod::line::value::ACTIVE);
  std::this_thread::sleep_for(std::chrono::seconds(1));
  line.set_value(signal_pin, gpiod::line::value::INACTIVE);
}

bool Igniter::is_igniting() {
  return line.get_value(signal_pin) == gpiod::line::value::ACTIVE;
}
