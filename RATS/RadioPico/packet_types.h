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

// Main radio packet structure (107 bytes)
struct RadioPacket {
    uint32_t sync_word;
    Metadata metadata;
    uint32_t ms_since_boot;
    Events events;
    
    // Altimeter
    float altitude;
    float temperature;
    
    // GPS
    int32_t latitude_udeg;      // microdegrees
    int32_t longitude_udeg;     // microdegrees
    uint8_t satellites;
    uint32_t unix_time;
    uint32_t horizontal_accuracy_mm;
    
    // IMU
    float accel_x;
    float accel_y;
    float accel_z;
    float gyro_x;
    float gyro_y;
    float gyro_z;
    float orient_x;
    float orient_y;
    float orient_z;
    
    // Accelerometer
    float accel2_x;
    float accel2_y;
    float accel2_z;
    
    // ADC
    float battery_voltage;
    float pt3_pressure;
    float pt4_pressure;
    float rtd_temperature;
    
    // BLiMS
    float motor_state;
};

// Umbilical packet structure (30 bytes)
struct UmbilicalPacket {
    Metadata metadata;
    uint32_t ms_since_boot;
    Events events;
    float battery_voltage;
    float pt3_pressure;
    float pt4_pressure;
    float rtd_temperature;
    float altitude;
};

#endif // PACKET_TYPES_H