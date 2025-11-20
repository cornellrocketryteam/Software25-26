#ifndef PACKET_TYPES_H
#define PACKET_TYPES_H

#include <stdint.h>

// Flight modes
enum FlightMode {
    STARTUP = 0,
    STANDBY = 1,
    ASCENT = 2,
    DROGUE_DEPLOYED = 3,
    MAIN_DEPLOYED = 4,
    FAULT = 5
};

// Metadata bitfield structure
struct Metadata {
    bool altitude_armed;
    bool altimeter_valid;
    bool gps_valid;
    bool imu_valid;
    bool accelerometer_valid;
    bool umbilical_locked;
    bool adc_valid;
    bool fram_valid;
    bool sd_valid;
    bool gps_fresh;
    bool safed;
    bool mav_state;
    bool sv_state;
    FlightMode flight_mode;
};

// Events bitfield structure
struct Events {
    bool altitude_armed;
    bool altimeter_init_failed;
    bool altimeter_read_failed;
    bool gps_init_failed;
    bool gps_read_failed;
    bool imu_init_failed;
    bool imu_read_failed;
    bool accel_init_failed;
    bool accel_read_failed;
    bool adc_init_failed;
    bool adc_read_failed;
    bool fram_init_failed;
    bool fram_read_failed;
    bool fram_write_failed;
    bool sd_init_failed;
    bool sd_write_failed;
    bool mav_actuated;
    bool sv_actuated;
    bool main_deploy_wait_end;
    bool main_log_shutoff;
    bool cycle_overflow;
    bool unknown_cmd;
    bool launch_cmd;
    bool mav_cmd;
    bool sv_cmd;
    bool safe_cmd;
    bool reset_card_cmd;
    bool reset_fram_cmd;
    bool state_change_cmd;
    bool umbilical_disconnected;
};

// Full Radio Packet Structure (107 bytes)
// Per RATS specification document
struct RadioPacket {
    // Byte 0-3: Sync word
    uint32_t sync_word;              // "CRT!" identifier

    // Byte 4-5: Metadata (16-bit bitfield)
    uint16_t metadata;               // See Metadata structure for bit definitions

    // Byte 6-9: Milliseconds since boot
    uint32_t ms_since_boot;          // Timestamp in milliseconds

    // Byte 10-13: Events (32-bit bitfield)
    uint32_t events;                 // See Events structure for bit definitions

    // Byte 14-21: Altimeter data
    float altitude;                  // meters
    float temperature;               // Celsius

    // Byte 22-38: GPS data
    int32_t latitude;                // micro-degrees (µdeg)
    int32_t longitude;               // micro-degrees (µdeg)
    uint8_t num_satellites;          // Satellites in view
    uint32_t gps_unix_time;          // Unix timestamp in seconds
    uint32_t gps_horizontal_accuracy; // millimeters

    // Byte 39-74: IMU data
    float imu_accel_x;               // m/s^2
    float imu_accel_y;               // m/s^2
    float imu_accel_z;               // m/s^2
    float imu_gyro_x;                // deg/s
    float imu_gyro_y;                // deg/s
    float imu_gyro_z;                // deg/s
    float imu_orient_x;              // degrees
    float imu_orient_y;              // degrees
    float imu_orient_z;              // degrees

    // Byte 75-86: Accelerometer data
    float accel_x;                   // g (gravity)
    float accel_y;                   // g
    float accel_z;                   // g

    // Byte 87-106: ADC and BLiMS data
    float battery_voltage;           // Volts
    float pt3_pressure;              // PSI
    float pt4_pressure;              // PSI
    float rtd_temperature;           // Celsius
    float blims_motor_state;         // inches
};

#endif // PACKET_TYPES_H