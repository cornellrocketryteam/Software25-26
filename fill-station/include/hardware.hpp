#pragma once

#include "igniter.hpp"
#include <gpiod.hpp>

class Hardware final {
public:
  Hardware();
  Igniter ig1;
  Igniter ig2;
private:
  gpiod::chip chip;
};
