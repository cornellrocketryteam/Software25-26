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

// Full Radio Packet Structure (203 bytes)
// 4 byte sync word + 199 byte payload from Rust
#pragma pack(push, 1)
struct RadioPacket {
    // Byte 0-3: Sync word
    uint32_t sync_word;              // "CRT!" identifier

    // Shared Telemetry (Rust 'Packet' Struct)
    uint32_t flight_mode;
    
    // altimeter
    float pressure;
    float temp;
    float altitude;
    
    // gps
    float latitude;
    float longitude;
    uint32_t num_satellites;
    float timestamp;
    
    // magnetometer
    float mag_x;
    float mag_y;
    float mag_z;
    
    // imu - accelerometer
    float accel_x;
    float accel_y;
    float accel_z;
    
    // imu - gyroscope
    float gyro_x;
    float gyro_y;
    float gyro_z;
    
    // adc
    float pt3;
    float pt4;
    float rtd;
    
    // valve states
    bool sv_open;
    bool mav_open;
    
    // event flags
    uint8_t ssa_drogue_deployed;
    uint8_t ssa_main_deployed;
    uint8_t cmd_n1;
    uint8_t cmd_n2;
    uint8_t cmd_n3;
    uint8_t cmd_n4;
    uint8_t cmd_a1;
    uint8_t cmd_a2;
    uint8_t cmd_a3;
    
    // airbrake state
    uint8_t airbrake_state;
    
    // airbrake controller output
    float predicted_apogee;
    
    // Advanced GPS
    uint32_t h_acc;
    uint32_t v_acc;
    double vel_n;
    double vel_e;
    double vel_d;
    double g_speed;
    uint32_t s_acc;
    uint32_t head_acc;
    uint8_t fix_type;
    int32_t head_mot;
    
    // BLiMS outputs
    float blims_motor_position;
    int8_t blims_phase_id;
    float blims_pid_p;
    float blims_pid_i;
    float blims_bearing;
    int8_t blims_loiter_step;
    float blims_heading_des;
    float blims_heading_error;
    float blims_error_integral;
    float blims_dist_to_target_m;
    
    // BLiMS config
    float blims_target_lat;
    float blims_target_lon;
    float blims_wind_from_deg;
    
    // monotonic clock: milliseconds since CFC boot
    uint32_t ms_since_boot_cfc;
};
#pragma pack(pop)

#endif // PACKET_TYPES_H