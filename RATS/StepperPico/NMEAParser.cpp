#include "NMEAParser.h"

void NMEAParser::init(uart_inst_t *uart_id, uint tx_pin, uint rx_pin,
                      uint baud_rate) {
  uart = uart_id;

  // Initialize the hardware UART
  uart_init(uart, baud_rate);

  // Set the TX and RX pins by using the function select on the GPIO
  gpio_set_function(tx_pin, GPIO_FUNC_UART);
  gpio_set_function(rx_pin, GPIO_FUNC_UART);

  uart_set_hw_flow(uart, false, false);
  uart_set_format(uart, 8, 1, UART_PARITY_NONE);
  uart_set_fifo_enabled(uart, true);

  bufferIndex = 0;
}

bool NMEAParser::process() {
  bool updated = false;

  // Read characters while they are available in the hardware FIFO buffer
  while (uart_is_readable(uart)) {
    char c = uart_getc(uart);

    // If we get a newline, the string is completely assembled
    if (c == '\n' || c == '\r') {
      if (bufferIndex > 0) {
        buffer[bufferIndex] = '\0'; // Null-terminate
        parseLine();                // Break down the NMEA sentence

        // If the parser successfully saw a $GNGGA or $GPGGA line with a valid
        // fix, mark updated
        if (strncmp(buffer, "$GPGGA", 6) == 0 ||
            strncmp(buffer, "$GNGGA", 6) == 0) {
          updated = isValid;
        }

        bufferIndex = 0; // Reset for next line
      }
    } else if (bufferIndex < sizeof(buffer) - 1) {
      buffer[bufferIndex++] = c;
    }
  }

  return updated;
}

void NMEAParser::parseLine() {
  // Only parse GGA sentences for 3D location and fix data
  if (strncmp(buffer, "$GPGGA", 6) == 0 || strncmp(buffer, "$GNGGA", 6) == 0) {
    parseGGA(buffer);
  }
}

// Example GGA:
// $GNGGA,210230.00,4221.4068,N,07629.8274,W,1,12,0.8,100.0,M,-34.0,M,,*47
bool NMEAParser::parseGGA(const char *line) {
  // Tokenize the string by commas
  char copy[128];
  strncpy(copy, line, sizeof(copy));
  copy[sizeof(copy) - 1] = '\0';

  char *tokens[15];
  int tokenCount = 0;

  char *token = strtok(copy, ",");
  while (token != NULL && tokenCount < 15) {
    tokens[tokenCount++] = token;
    token = strtok(NULL, ",");
  }

  // GGA must have at least 10 fields for altitude, and the 6th index is the Fix
  // Quality
  if (tokenCount >= 10) {
    int fixQuality = atoi(tokens[6]);

    if (fixQuality > 0) { // 1 = GPS Fix, 2 = DGPS Fix
      isValid = true;

      // Tokens[2] is Lat, [3] is N/S
      latitude = parseNMEACoord(tokens[2], tokens[3][0]);

      // Tokens[4] is Lon, [5] is E/W
      longitude = parseNMEACoord(tokens[4], tokens[5][0]);

      satellites = atoi(tokens[7]);
      altitude = atof(tokens[9]);
      return true;
    } else {
      isValid = false;
    }
  }
  return false;
}

// NMEA returns DDMM.MMMM (Degrees, Minutes)
// This strictly converts it back to DD.DDDD (Decimal Degrees)
double NMEAParser::parseNMEACoord(const char *degreeString, char hemisphere) {
  if (degreeString == NULL || strlen(degreeString) < 4)
    return 0.0;

  double raw = atof(degreeString);
  int degrees = (int)(raw / 100);
  double minutes = raw - (degrees * 100);

  double decimalDegrees = degrees + (minutes / 60.0);

  if (hemisphere == 'S' || hemisphere == 'W') {
    decimalDegrees = -decimalDegrees;
  }

  return decimalDegrees;
}
