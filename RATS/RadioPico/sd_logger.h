#ifndef SD_LOGGER_H
#define SD_LOGGER_H

#include "pico/stdlib.h"
#include "packet_types.h"

// SD card pin definitions (per system design: Radio Pico SPI1)
// SD Breakout Pinout (SPI mode):
//   3V    -> 3.3V power
//   GND   -> GND
//   CLK   -> GP10 (SPI1 SCK)
//   D0    -> GP12 (SPI1 MISO) - Data OUT from SD card
//   S1    -> Not used in SPI mode
//   CMD   -> GP11 (SPI1 MOSI) - Data IN to SD card
//   CS/D3 -> GP13 (SPI1 CS)
//   D1    -> Not used in SPI mode
//   D2    -> Not used in SPI mode
//   DET   -> GP22 (Card Detect)
#define SD_SPI_ID spi1
#define SD_CLK_PIN 10   // GP10 - SPI1 SCK -> CLK
#define SD_MOSI_PIN 11  // GP11 - SPI1 MOSI -> CMD
#define SD_MISO_PIN 12  // GP12 - SPI1 MISO -> D0
#define SD_CS_PIN 13    // GP13 - SPI1 CS -> CS/D3
#define SD_CD_PIN 22    // GP22 - Card Detect -> DET

class SDLogger {
public:
    // Initialize SD card and create new log file
    static bool init();

    // Check if SD card is mounted and ready
    static bool isReady();

    // Log a single packet (writes JSON line)
    static bool logPacket(const RadioPacket& packet);

    // Batch log multiple packets (more efficient)
    static bool logPacketBatch(const RadioPacket* packets, size_t count);

    // Flush any pending writes to SD card
    static void flush();

    // Close current log file
    static void close();

    // Get current log filename
    static const char* getCurrentFilename();

    // Get statistics
    static void getStats(uint32_t& packets_logged, uint32_t& bytes_written, uint32_t& write_errors);

private:
    static bool sd_mounted;
    static char current_filename[32];
    static uint32_t packet_count;
    static uint32_t total_bytes_written;
    static uint32_t write_error_count;
    static void* file_handle;  // FIL* pointer

    // Generate filename based on timestamp: RATS_YYYYMMDD_HHMMSS.json
    static void generateFilename(char* buffer, size_t buffer_size);

    // Write string to file
    static bool writeString(const char* str);
};

#endif // SD_LOGGER_H
