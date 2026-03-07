#ifndef NMEA_PARSER_H
#define NMEA_PARSER_H

#include "hardware/uart.h"
#include "pico/stdlib.h"
#include <stdint.h>
#include <stdlib.h>
#include <string.h>


class NMEAParser {
public:
  // Initializes the hardware UART on the Pico
  // UART 1: TX on GP4, RX on GP5
  void init(uart_inst_t *uart_id = uart1, uint tx_pin = 4, uint rx_pin = 5,
            uint baud_rate = 9600);

  // Reads from the FIFO buffer and parses completed lines.
  // Returns true if a new position was acquired this cycle.
  bool process();

  // Latest extracted position
  double getLatitude() const { return latitude; }
  double getLongitude() const { return longitude; }
  double getAltitude() const { return altitude; }
  uint8_t getSatellites() const { return satellites; }
  bool hasFix() const { return isValid; }

private:
  uart_inst_t *uart;

  // NMEA parsing state
  char buffer[128];
  uint8_t bufferIndex = 0;

  // Extracted data
  double latitude = 0.0;
  double longitude = 0.0;
  double altitude = 0.0;
  uint8_t satellites = 0;
  bool isValid = false;

  // Helper functions
  void parseLine();
  bool parseGGA(const char *line);
  double parseNMEACoord(const char *degreeString, char hemisphere);
};

#endif // NMEA_PARSER_H
