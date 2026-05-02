#include "packet_parser.h"
#include <string.h>
#include <stdio.h>
#include <time.h>

// Template specialization for reading values from buffer
template<typename T>
T PacketParser::readValue(const uint8_t* buffer, size_t& offset) {
    T value;
    memcpy(&value, buffer + offset, sizeof(T));
    offset += sizeof(T);
    return value;
}

bool PacketParser::parseRadioPacket(const uint8_t* buffer, size_t length, RadioPacket& packet) {
    if (length < sizeof(RadioPacket)) return false;

    // Since both Raspberry Pi Pico (ARM Cortex-M0+) and the Rust packet serialization
    // use little-endian, and the struct is packed, we can directly copy the memory.
    memcpy(&packet, buffer, sizeof(RadioPacket));

    return true;
}

void PacketParser::radioPacketToJSON(const RadioPacket& packet, char* json_buffer, size_t buffer_size) {
    // Generate JSON matching the telemetry_data DB schema
    snprintf(json_buffer, buffer_size,
        "{"
        "\"sync_word\":%u,"
        "\"flight_mode\":%u,"
        "\"pressure\":%.4f,"
        "\"temp\":%.4f,"
        "\"altitude\":%.4f,"
        "\"latitude\":%.6f,"
        "\"longitude\":%.6f,"
        "\"num_satellites\":%u,"
        "\"timestamp\":%.4f,"
        "\"mag_x\":%.4f,\"mag_y\":%.4f,\"mag_z\":%.4f,"
        "\"accel_x\":%.4f,\"accel_y\":%.4f,\"accel_z\":%.4f,"
        "\"gyro_x\":%.4f,\"gyro_y\":%.4f,\"gyro_z\":%.4f,"
        "\"pt3\":%.4f,\"pt4\":%.4f,\"rtd\":%.4f,"
        "\"sv_2_open\":%s,"
        "\"mav_open\":%s,"
        "\"ssa_drogue_deployed\":%u,"
        "\"ssa_main_deployed\":%u,"
        "\"cmd_n1\":%u,\"cmd_n2\":%u,\"cmd_n3\":%u,\"cmd_n4\":%u,"
        "\"cmd_a1\":%u,\"cmd_a2\":%u,\"cmd_a3\":%u,"
        "\"airbrake_deployment\":%.3f,"
        "\"predicted_apogee\":%.4f,"
        "\"h_acc\":%u,\"v_acc\":%u,"
        "\"vel_n\":%.4f,\"vel_e\":%.4f,\"vel_d\":%.4f,\"g_speed\":%.4f,"
        "\"s_acc\":%u,\"head_acc\":%u,"
        "\"fix_type\":%u,"
        "\"head_mot\":%d,"
        "\"blims_motor_position\":%.4f,"
        "\"blims_phase_id\":%d,"
        "\"blims_pid_p\":%.4f,\"blims_pid_i\":%.4f,"
        "\"blims_bearing\":%.4f,"
        "\"blims_loiter_step\":%d,"
        "\"blims_heading_des\":%.4f,"
        "\"blims_heading_error\":%.4f,"
        "\"blims_error_integral\":%.4f,"
        "\"blims_dist_to_target_m\":%.4f,"
        "\"blims_target_lat\":%.6f,"
        "\"blims_target_lon\":%.6f,"
        "\"blims_wind_from_deg\":%.4f,"
        "\"blims_downwind\":%.4f,"
        "\"blims_upwind\":%.4f,"
        "\"ms_since_boot_cfc\":%u"
        "}",
        packet.sync_word,
        packet.flight_mode,
        packet.pressure, packet.temp, packet.altitude,
        packet.latitude, packet.longitude,
        packet.num_satellites, packet.timestamp,
        packet.mag_x, packet.mag_y, packet.mag_z,
        packet.accel_x, packet.accel_y, packet.accel_z,
        packet.gyro_x, packet.gyro_y, packet.gyro_z,
        packet.pt3, packet.pt4, packet.rtd,
        packet.sv_open ? "true" : "false",
        packet.mav_open ? "true" : "false",
        packet.ssa_drogue_deployed, packet.ssa_main_deployed,
        packet.cmd_n1, packet.cmd_n2, packet.cmd_n3, packet.cmd_n4,
        packet.cmd_a1, packet.cmd_a2, packet.cmd_a3,
        packet.airbrake_deployment, packet.predicted_apogee,
        packet.h_acc, packet.v_acc,
        packet.vel_n, packet.vel_e, packet.vel_d, packet.g_speed,
        packet.s_acc, packet.head_acc, packet.fix_type,
        packet.head_mot,
        packet.blims_motor_position, packet.blims_phase_id,
        packet.blims_pid_p, packet.blims_pid_i, packet.blims_bearing,
        packet.blims_loiter_step,
        packet.blims_heading_des, packet.blims_heading_error,
        packet.blims_error_integral, packet.blims_dist_to_target_m,
        packet.blims_target_lat, packet.blims_target_lon,
        packet.blims_wind_from_deg,
        packet.blims_downwind,
        packet.blims_upwind,
        packet.ms_since_boot_cfc
    );
}
