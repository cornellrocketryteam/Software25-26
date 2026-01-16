#ifndef SD_LOGGER_H
#define SD_LOGGER_H

#include "pico/stdlib.h"
#include "packet_types.h"

class SDLogger {
public:
    // Initialize SD card and create new log file
    static bool init();

    // Check if SD card is mounted and ready
    static bool isReady();

    // Log a single packet (writes CSV line)
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

    // Generate filename based on boot time: RATS_XXXXXXXX.csv
    static void generateFilename(char* buffer, size_t buffer_size);

    // Write string to file
    static bool writeString(const char* str);
};

#endif // SD_LOGGER_H
