#include "sd_logger.h"
#include "packet_parser.h"
#include "hw_config.h"  // SD card hardware configuration
#include "f_util.h"     // FatFS utility functions
#include "ff.h"         // FatFS library
#include <stdio.h>
#include <string.h>

// Static member initialization
bool SDLogger::sd_mounted = false;
char SDLogger::current_filename[32] = {0};
uint32_t SDLogger::packet_count = 0;
uint32_t SDLogger::total_bytes_written = 0;
uint32_t SDLogger::write_error_count = 0;
void* SDLogger::file_handle = nullptr;

// FatFS file object
static FIL fil;
static sd_card_t *pSD = nullptr;

// Stub function for FatFS - RP2350 doesn't have hardware RTC
// Returns a dummy timestamp (Jan 1, 2025, 00:00:00)
extern "C" DWORD get_fattime(void) {
    // FAT timestamp format: bits 0-4=day, 5-8=month, 9-15=year from 1980
    // bits 16-20=second/2, 21-26=minute, 27-31=hour
    // Year 2025 = 45 years since 1980
    return ((DWORD)(2025 - 1980) << 25) | ((DWORD)1 << 21) | ((DWORD)1 << 16);
}

bool SDLogger::init() {
    printf("[SD] Initializing SD card on SPI1...\n");

    // Note: RP2350 doesn't have hardware RTC, using boot time for filenames

    // Get SD card object (configured in hw_config.c)
    pSD = sd_get_by_num(0);
    if (!pSD) {
        printf("[SD] Failed to get SD card object\n");
        return false;
    }

    // Mount the filesystem
    FRESULT fr = f_mount(&pSD->fatfs, pSD->pcName, 1);
    if (fr != FR_OK) {
        printf("[SD] Failed to mount filesystem: %s (%d)\n", FRESULT_str(fr), fr);
        return false;
    }
    printf("[SD] Filesystem mounted\n");

    // Generate unique filename
    generateFilename(current_filename, sizeof(current_filename));

    // Create and open the log file
    fr = f_open(&fil, current_filename, FA_CREATE_ALWAYS | FA_WRITE);
    if (fr != FR_OK) {
        printf("[SD] Failed to create file '%s': %s (%d)\n",
               current_filename, FRESULT_str(fr), fr);
        f_unmount(pSD->pcName);
        return false;
    }

    file_handle = &fil;
    sd_mounted = true;

    printf("[SD] Created log file: %s\n", current_filename);

    // TEMPORARY: Write simplified CSV header for test packet structure
    const char* header = "flight_mode,pt3_pressure,temperature,altitude,latitude_deg,longitude_deg,num_satellites,ms_since_boot\n";

    if (!writeString(header)) {
        printf("[SD] Failed to write header\n");
        return false;
    }

    flush();
    printf("[SD] Initialization complete\n\n");

    return true;
}

bool SDLogger::isReady() {
    return sd_mounted && (file_handle != nullptr);
}

bool SDLogger::logPacket(const RadioPacket& packet) {
    if (!isReady()) {
        return false;
    }

    // TEMPORARY: Format simplified test packet as CSV line
    char line[128];
    uint8_t flight_mode = (packet.metadata >> 13) & 0x07;
    float lat_deg = packet.latitude / 1000000.0f;
    float lon_deg = packet.longitude / 1000000.0f;

    int len = snprintf(line, sizeof(line),
        "%u,%.2f,%.2f,%.2f,%.6f,%.6f,%u,%u\n",
        flight_mode,
        packet.pt3_pressure,
        packet.temperature,
        packet.altitude,
        lat_deg,
        lon_deg,
        packet.num_satellites,
        packet.ms_since_boot
    );

    if (len < 0 || len >= (int)sizeof(line)) {
        write_error_count++;
        return false;
    }

    if (writeString(line)) {
        packet_count++;
        return true;
    }

    return false;
}

bool SDLogger::logPacketBatch(const RadioPacket* packets, size_t count) {
    if (!isReady()) {
        return false;
    }

    bool all_success = true;
    for (size_t i = 0; i < count; i++) {
        if (!logPacket(packets[i])) {
            all_success = false;
        }
    }

    // Flush after batch write
    flush();

    return all_success;
}

void SDLogger::flush() {
    if (file_handle != nullptr) {
        f_sync((FIL*)file_handle);
    }
}

void SDLogger::close() {
    if (file_handle != nullptr) {
        flush();
        f_close((FIL*)file_handle);
        file_handle = nullptr;
    }

    if (sd_mounted && pSD) {
        f_unmount(pSD->pcName);
        sd_mounted = false;
    }

    printf("[SD] Closed log file: %s (%u packets, %u bytes)\n",
           current_filename, packet_count, total_bytes_written);
}

const char* SDLogger::getCurrentFilename() {
    return current_filename;
}

void SDLogger::getStats(uint32_t& packets_logged, uint32_t& bytes_written, uint32_t& write_errors) {
    packets_logged = packet_count;
    bytes_written = total_bytes_written;
    write_errors = write_error_count;
}

void SDLogger::generateFilename(char* buffer, size_t buffer_size) {
    // Use boot time as simple timestamp (since we don't have RTC)
    uint32_t boot_ms = to_ms_since_boot(get_absolute_time());
    uint32_t boot_sec = boot_ms / 1000;

    snprintf(buffer, buffer_size, "RATS_%08u.csv", boot_sec);
}

bool SDLogger::writeString(const char* str) {
    if (!isReady()) {
        return false;
    }

    size_t len = strlen(str);
    UINT bytes_written = 0;

    FRESULT fr = f_write((FIL*)file_handle, str, len, &bytes_written);

    if (fr != FR_OK || bytes_written != len) {
        write_error_count++;
        printf("[SD] Write failed (error %d, wrote %u/%u bytes)\n", fr, bytes_written, len);
        return false;
    }

    total_bytes_written += bytes_written;
    return true;
}
