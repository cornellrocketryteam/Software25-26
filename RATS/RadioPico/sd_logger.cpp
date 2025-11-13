#include "sd_logger.h"
#include "packet_parser.h"
#include "hardware/spi.h"
#include "hardware/gpio.h"
#include "ff.h"  // FatFS library
#include <stdio.h>
#include <string.h>
#include <time.h>

// Static member initialization
bool SDLogger::sd_mounted = false;
char SDLogger::current_filename[32] = {0};
uint32_t SDLogger::packet_count = 0;
uint32_t SDLogger::total_bytes_written = 0;
uint32_t SDLogger::write_error_count = 0;
void* SDLogger::file_handle = nullptr;

// FatFS objects
static FATFS fs;
static FIL fil;

bool SDLogger::init() {
    printf("[SD] Initializing SD card on SPI1...\n");

    // Check if card is present (Card Detect pin)
    gpio_init(SD_CD_PIN);
    gpio_set_dir(SD_CD_PIN, GPIO_IN);
    gpio_pull_up(SD_CD_PIN);

    // CD pin is active low (0 = card present)
    if (gpio_get(SD_CD_PIN) != 0) {
        printf("[SD] No card detected (CD pin high)\n");
        return false;
    }
    printf("[SD] Card detected\n");

    // Initialize SPI for SD card
    spi_init(SD_SPI_ID, 10 * 1000 * 1000);  // Start at 10MHz

    // Set up SPI pins
    gpio_set_function(SD_CLK_PIN, GPIO_FUNC_SPI);
    gpio_set_function(SD_MOSI_PIN, GPIO_FUNC_SPI);
    gpio_set_function(SD_MISO_PIN, GPIO_FUNC_SPI);

    // CS pin as output, start high
    gpio_init(SD_CS_PIN);
    gpio_set_dir(SD_CS_PIN, GPIO_OUT);
    gpio_put(SD_CS_PIN, 1);

    printf("[SD] SPI initialized (CLK=GP%d, MOSI=GP%d, MISO=GP%d, CS=GP%d)\n",
           SD_CLK_PIN, SD_MOSI_PIN, SD_MISO_PIN, SD_CS_PIN);

    // Mount the filesystem
    FRESULT fr = f_mount(&fs, "", 1);
    if (fr != FR_OK) {
        printf("[SD] Failed to mount filesystem (error %d)\n", fr);
        return false;
    }
    printf("[SD] Filesystem mounted\n");

    // Generate unique filename
    generateFilename(current_filename, sizeof(current_filename));

    // Create and open the log file
    fr = f_open(&fil, current_filename, FA_CREATE_ALWAYS | FA_WRITE);
    if (fr != FR_OK) {
        printf("[SD] Failed to create file '%s' (error %d)\n", current_filename, fr);
        f_unmount("");
        return false;
    }

    file_handle = &fil;
    sd_mounted = true;

    printf("[SD] Created log file: %s\n", current_filename);

    // Write CSV header
    const char* header = "timestamp_ms,latitude,longitude,altitude,satellites,temperature,"
                        "accel_x,accel_y,accel_z,gyro_x,gyro_y,gyro_z,"
                        "orient_x,orient_y,orient_z,battery_v,flight_mode\n";

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

    // Format packet as CSV line
    char line[256];
    int len = snprintf(line, sizeof(line),
        "%u,%d,%d,%.2f,%u,%.2f,"
        "%.2f,%.2f,%.2f,%.2f,%.2f,%.2f,"
        "%.2f,%.2f,%.2f,%.2f,%u\n",
        packet.ms_since_boot,
        packet.latitude_udeg,
        packet.longitude_udeg,
        packet.altitude,
        packet.satellites,
        packet.temperature,
        packet.accel_x,
        packet.accel_y,
        packet.accel_z,
        packet.gyro_x,
        packet.gyro_y,
        packet.gyro_z,
        packet.orient_x,
        packet.orient_y,
        packet.orient_z,
        packet.battery_voltage,
        (packet.raw_metadata >> 13) & 0x07  // Extract flight mode bits
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

    if (sd_mounted) {
        f_unmount("");
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
