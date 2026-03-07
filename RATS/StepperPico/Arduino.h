#pragma once

// Fake Arduino.h for Raspberry Pi Pico SDK bare-metal compilation
// This translates Arduino functions into Pico SDK functions so AccelStepper can
// compile unmodified.

#include "pico/stdlib.h"
#include <algorithm>
#include <cmath>
#include <stdint.h>

#define HIGH 1
#define LOW 0

#define OUTPUT true
#define INPUT false
#define INPUT_PULLUP false

typedef bool boolean;

#ifdef __cplusplus
extern "C" {
#endif

inline void pinMode(uint8_t pin, bool mode) {
  gpio_init(pin);
  gpio_set_dir(pin, mode);
  // Note: If you need INPUT_PULLUP, add gpio_pull_up(pin);
}

inline void digitalWrite(uint8_t pin, uint8_t val) { gpio_put(pin, val); }

inline int digitalRead(uint8_t pin) { return gpio_get(pin) ? 1 : 0; }

inline unsigned long millis() { return to_ms_since_boot(get_absolute_time()); }

inline unsigned long micros() { return to_us_since_boot(get_absolute_time()); }

inline void delay(unsigned long ms) { sleep_ms(ms); }

inline void delayMicroseconds(unsigned int us) { sleep_us(us); }

// Add constrain macro if missing
#ifndef constrain
#define constrain(amt, low, high)                                              \
  ((amt) < (low) ? (low) : ((amt) > (high) ? (high) : (amt)))
#endif

// Add min/max helpers since AccelStepper uses them heavily
#ifndef max
#define max(a, b) ((a) > (b) ? (a) : (b))
#endif

#ifndef min
#define min(a, b) ((a) < (b) ? (a) : (b))
#endif

#ifdef __cplusplus
}
#endif
