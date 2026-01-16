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

// FatFS file object and filesystem
static FIL fil;
static FATFS fs;
static sd_card_t *pSD = nullptr;

bool SDLogger::init() {
    printf("[SD] Initializing SD card on SPI1...\n");

    // Get SD card object (configured in hw_config.c)
    pSD = sd_get_by_num(0);
    if (!pSD) {
        printf("[SD] Failed to get SD card object\n");
        return false;
    }

    // Mount the filesystem (empty string "" for default drive)
    FRESULT fr = f_mount(&fs, "", 1);
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
        f_unmount("");
        return false;
    }

    file_handle = &fil;
    sd_mounted = true;

    printf("[SD] Created log file: %s\n", current_filename);

    // Write CSV header for ALL telemetry data fields
    const char* header = "sync_word,metadata,ms_since_boot,events,altitude,temperature,"
                        "latitude_deg,longitude_deg,num_satellites,gps_unix_time,gps_horizontal_accuracy,"
                        "imu_accel_x,imu_accel_y,imu_accel_z,imu_gyro_x,imu_gyro_y,imu_gyro_z,"
                        "imu_orient_x,imu_orient_y,imu_orient_z,"
                        "accel_x,accel_y,accel_z,"
                        "battery_voltage,pt3_pressure,pt4_pressure,rtd_temperature,blims_motor_state\n";

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

    // Format packet data as CSV line with ALL fields
    char line[512];  // Increased buffer for all fields
    float lat_deg = packet.latitude / 1000000.0f;
    float lon_deg = packet.longitude / 1000000.0f;

    int len = snprintf(line, sizeof(line),
        "0x%08X,%u,%u,0x%08X,%.2f,%.2f,"  // sync, metadata, ms, events, alt, temp
        "%.6f,%.6f,%u,%u,%u,"              // lat, lon, sats, gps_time, gps_acc
        "%.3f,%.3f,%.3f,%.3f,%.3f,%.3f,"   // imu accel, gyro
        "%.2f,%.2f,%.2f,"                   // imu orient
        "%.3f,%.3f,%.3f,"                   // accel
        "%.3f,%.2f,%.2f,%.2f,%.3f\n",      // battery, pressures, rtd, blims
        packet.sync_word,
        packet.metadata,
        packet.ms_since_boot,
        packet.events,
        packet.altitude,
        packet.temperature,
        lat_deg,
        lon_deg,
        packet.num_satellites,
        packet.gps_unix_time,
        packet.gps_horizontal_accuracy,
        packet.imu_accel_x,
        packet.imu_accel_y,
        packet.imu_accel_z,
        packet.imu_gyro_x,
        packet.imu_gyro_y,
        packet.imu_gyro_z,
        packet.imu_orient_x,
        packet.imu_orient_y,
        packet.imu_orient_z,
        packet.accel_x,
        packet.accel_y,
        packet.accel_z,
        packet.battery_voltage,
        packet.pt3_pressure,
        packet.pt4_pressure,
        packet.rtd_temperature,
        packet.blims_motor_state
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
