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
    const char* header =
        "sync_word,flight_mode,"
        "pressure,temp,altitude,"
        "latitude,longitude,num_satellites,gps_timestamp,"
        "mag_x,mag_y,mag_z,"
        "accel_x,accel_y,accel_z,"
        "gyro_x,gyro_y,gyro_z,"
        "pt3,pt4,rtd,"
        "sv_open,mav_open,"
        "ssa_drogue_deployed,ssa_main_deployed,"
        "cmd_n1,cmd_n2,cmd_n3,cmd_n4,cmd_a1,cmd_a2,cmd_a3,"
        "airbrake_deployment,predicted_apogee,"
        "h_acc,v_acc,vel_n,vel_e,vel_d,g_speed,s_acc,head_acc,fix_type,head_mot,"
        "blims_brakeline_diff,blims_phase_id,blims_pid_p,blims_pid_i,blims_bearing,"
        "blims_upwind_lat,blims_upwind_lon,blims_downwind_lat,blims_downwind_lon,blims_wind_from_deg,"
        "ms_since_boot_cfc\n";

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
    char line[1024];

    int len = snprintf(line, sizeof(line),
        "0x%08lX,%lu,"                                  // sync, flight_mode
        "%.3f,%.3f,%.3f,"                               // pressure, temp, altitude
        "%.6f,%.6f,%lu,%.3f,"                           // lat, lon, sats, gps timestamp
        "%.3f,%.3f,%.3f,"                               // mag
        "%.3f,%.3f,%.3f,"                               // accel
        "%.3f,%.3f,%.3f,"                               // gyro
        "%.3f,%.3f,%.3f,"                               // pt3, pt4, rtd
        "%u,%u,"                                        // sv_open, mav_open
        "%u,%u,"                                        // ssa drogue/main
        "%u,%u,%u,%u,%u,%u,%u,"                         // cmd_n1..n4, cmd_a1..a3
        "%.3f,%.3f,"                                      // airbrake_deployment, predicted_apogee
        "%lu,%lu,%.6f,%.6f,%.6f,%.6f,%lu,%lu,%u,%ld,"   // advanced GPS
        "%.3f,%d,%.6f,%.6f,%.3f,"                       // blims motor/phase/pid/bearing
        "%.6f,%.6f,%.6f,%.6f,%.3f,"                     // blims lat/lon config
        "%lu\n",                                        // ms_since_boot_cfc
        (unsigned long)packet.sync_word,
        (unsigned long)packet.flight_mode,
        packet.pressure,
        packet.temp,
        packet.altitude,
        packet.latitude,
        packet.longitude,
        (unsigned long)packet.num_satellites,
        packet.timestamp,
        packet.mag_x, packet.mag_y, packet.mag_z,
        packet.accel_x, packet.accel_y, packet.accel_z,
        packet.gyro_x, packet.gyro_y, packet.gyro_z,
        packet.pt3, packet.pt4, packet.rtd,
        packet.sv_open ? 1u : 0u,
        packet.mav_open ? 1u : 0u,
        packet.ssa_drogue_deployed,
        packet.ssa_main_deployed,
        packet.cmd_n1, packet.cmd_n2, packet.cmd_n3, packet.cmd_n4,
        packet.cmd_a1, packet.cmd_a2, packet.cmd_a3,
        packet.airbrake_deployment,
        packet.predicted_apogee,
        (unsigned long)packet.h_acc,
        (unsigned long)packet.v_acc,
        packet.vel_n, packet.vel_e, packet.vel_d,
        packet.g_speed,
        (unsigned long)packet.s_acc,
        (unsigned long)packet.head_acc,
        packet.fix_type,
        (long)packet.head_mot,
        packet.blims_brakeline_diff,
        (int)packet.blims_phase_id,
        packet.blims_pid_p, packet.blims_pid_i,
        packet.blims_bearing,
        packet.blims_upwind_lat,
        packet.blims_upwind_lon,
        packet.blims_downwind_lat,
        packet.blims_downwind_lon,
        packet.blims_wind_from_deg,
        (unsigned long)packet.ms_since_boot_cfc
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
