#include "packet_parser.h"
#include <string.h>
#include <stdio.h>

// Template specialization for reading values from buffer
template<typename T>
T PacketParser::readValue(const uint8_t* buffer, size_t& offset) {
    T value;
    memcpy(&value, buffer + offset, sizeof(T));
    offset += sizeof(T);
    return value;
}

Metadata PacketParser::parseMetadata(uint16_t raw_metadata) {
    Metadata meta;
    meta.altitude_armed = (raw_metadata >> 0) & 1;
    meta.altimeter_valid = (raw_metadata >> 1) & 1;
    meta.gps_valid = (raw_metadata >> 2) & 1;
    meta.imu_valid = (raw_metadata >> 3) & 1;
    meta.accelerometer_valid = (raw_metadata >> 4) & 1;
    meta.umbilical_locked = (raw_metadata >> 5) & 1;
    meta.adc_valid = (raw_metadata >> 6) & 1;
    meta.fram_valid = (raw_metadata >> 7) & 1;
    meta.sd_valid = (raw_metadata >> 8) & 1;
    meta.gps_fresh = (raw_metadata >> 9) & 1;
    meta.safed = (raw_metadata >> 10) & 1;
    meta.mav_state = (raw_metadata >> 11) & 1;
    meta.sv_state = (raw_metadata >> 12) & 1;
    meta.flight_mode = static_cast<FlightMode>((raw_metadata >> 13) & 0x7);
    return meta;
}

Events PacketParser::parseEvents(uint32_t raw_events) {
    Events evt;
    evt.altitude_armed = (raw_events >> 0) & 1;
    evt.altimeter_init_failed = (raw_events >> 1) & 1;
    evt.altimeter_read_failed = (raw_events >> 2) & 1;
    evt.gps_init_failed = (raw_events >> 3) & 1;
    evt.gps_read_failed = (raw_events >> 4) & 1;
    evt.imu_init_failed = (raw_events >> 5) & 1;
    evt.imu_read_failed = (raw_events >> 6) & 1;
    evt.accel_init_failed = (raw_events >> 7) & 1;
    evt.accel_read_failed = (raw_events >> 8) & 1;
    evt.adc_init_failed = (raw_events >> 9) & 1;
    evt.adc_read_failed = (raw_events >> 10) & 1;
    evt.fram_init_failed = (raw_events >> 11) & 1;
    evt.fram_read_failed = (raw_events >> 12) & 1;
    evt.fram_write_failed = (raw_events >> 13) & 1;
    evt.sd_init_failed = (raw_events >> 14) & 1;
    evt.sd_write_failed = (raw_events >> 15) & 1;
    evt.mav_actuated = (raw_events >> 16) & 1;
    evt.sv_actuated = (raw_events >> 17) & 1;
    evt.main_deploy_wait_end = (raw_events >> 18) & 1;
    evt.main_log_shutoff = (raw_events >> 19) & 1;
    evt.cycle_overflow = (raw_events >> 20) & 1;
    evt.unknown_cmd = (raw_events >> 21) & 1;
    evt.launch_cmd = (raw_events >> 22) & 1;
    evt.mav_cmd = (raw_events >> 23) & 1;
    evt.sv_cmd = (raw_events >> 24) & 1;
    evt.safe_cmd = (raw_events >> 25) & 1;
    evt.reset_card_cmd = (raw_events >> 26) & 1;
    evt.reset_fram_cmd = (raw_events >> 27) & 1;
    evt.state_change_cmd = (raw_events >> 28) & 1;
    evt.umbilical_disconnected = (raw_events >> 29) & 1;
    return evt;
}

bool PacketParser::parseRadioPacket(const uint8_t* buffer, size_t length, RadioPacket& packet) {
    if (length < 107) return false;  // Radio packet is 107 bytes
    
    size_t offset = 0;
    
    packet.sync_word = readValue<uint32_t>(buffer, offset);
    packet.metadata = parseMetadata(readValue<uint16_t>(buffer, offset));
    packet.ms_since_boot = readValue<uint32_t>(buffer, offset);
    packet.events = parseEvents(readValue<uint32_t>(buffer, offset));
    
    packet.altitude = readValue<float>(buffer, offset);
    packet.temperature = readValue<float>(buffer, offset);
    
    packet.latitude_udeg = readValue<int32_t>(buffer, offset);
    packet.longitude_udeg = readValue<int32_t>(buffer, offset);
    packet.satellites = readValue<uint8_t>(buffer, offset);
    packet.unix_time = readValue<uint32_t>(buffer, offset);
    packet.horizontal_accuracy_mm = readValue<uint32_t>(buffer, offset);
    
    packet.accel_x = readValue<float>(buffer, offset);
    packet.accel_y = readValue<float>(buffer, offset);
    packet.accel_z = readValue<float>(buffer, offset);
    packet.gyro_x = readValue<float>(buffer, offset);
    packet.gyro_y = readValue<float>(buffer, offset);
    packet.gyro_z = readValue<float>(buffer, offset);
    packet.orient_x = readValue<float>(buffer, offset);
    packet.orient_y = readValue<float>(buffer, offset);
    packet.orient_z = readValue<float>(buffer, offset);
    
    packet.accel2_x = readValue<float>(buffer, offset);
    packet.accel2_y = readValue<float>(buffer, offset);
    packet.accel2_z = readValue<float>(buffer, offset);
    
    packet.battery_voltage = readValue<float>(buffer, offset);
    packet.pt3_pressure = readValue<float>(buffer, offset);
    packet.pt4_pressure = readValue<float>(buffer, offset);
    packet.rtd_temperature = readValue<float>(buffer, offset);
    packet.motor_state = readValue<float>(buffer, offset);
    
    return true;
}

bool PacketParser::parseUmbilicalPacket(const uint8_t* buffer, size_t length, UmbilicalPacket& packet) {
    if (length < 30) return false;  // Umbilical packet is 30 bytes
    
    size_t offset = 0;
    
    packet.metadata = parseMetadata(readValue<uint16_t>(buffer, offset));
    packet.ms_since_boot = readValue<uint32_t>(buffer, offset);
    packet.events = parseEvents(readValue<uint32_t>(buffer, offset));
    packet.battery_voltage = readValue<float>(buffer, offset);
    packet.pt3_pressure = readValue<float>(buffer, offset);
    packet.pt4_pressure = readValue<float>(buffer, offset);
    packet.rtd_temperature = readValue<float>(buffer, offset);
    packet.altitude = readValue<float>(buffer, offset);
    
    return true;
}

void PacketParser::radioPacketToJSON(const RadioPacket& packet, char* json_buffer, size_t buffer_size) {
    snprintf(json_buffer, buffer_size,
        "{"
        "\"sync_word\":%u,"
        "\"ms_since_boot\":%u,"
        "\"flight_mode\":%d,"
        "\"altitude\":%.2f,"
        "\"temperature\":%.2f,"
        "\"gps\":{"
            "\"lat\":%.6f,"
            "\"lon\":%.6f,"
            "\"satellites\":%u,"
            "\"unix_time\":%u,"
            "\"h_accuracy_mm\":%u"
        "},"
        "\"imu\":{"
            "\"accel\":[%.3f,%.3f,%.3f],"
            "\"gyro\":[%.3f,%.3f,%.3f],"
            "\"orient\":[%.3f,%.3f,%.3f]"
        "},"
        "\"accel2\":[%.3f,%.3f,%.3f],"
        "\"battery\":%.2f,"
        "\"pt3\":%.2f,"
        "\"pt4\":%.2f,"
        "\"rtd_temp\":%.2f,"
        "\"motor_state\":%.2f"
        "}",
        packet.sync_word,
        packet.ms_since_boot,
        packet.metadata.flight_mode,
        packet.altitude,
        packet.temperature,
        packet.latitude_udeg / 1000000.0,
        packet.longitude_udeg / 1000000.0,
        packet.satellites,
        packet.unix_time,
        packet.horizontal_accuracy_mm,
        packet.accel_x, packet.accel_y, packet.accel_z,
        packet.gyro_x, packet.gyro_y, packet.gyro_z,
        packet.orient_x, packet.orient_y, packet.orient_z,
        packet.accel2_x, packet.accel2_y, packet.accel2_z,
        packet.battery_voltage,
        packet.pt3_pressure,
        packet.pt4_pressure,
        packet.rtd_temperature,
        packet.motor_state
    );
}

void PacketParser::umbilicalPacketToJSON(const UmbilicalPacket& packet, char* json_buffer, size_t buffer_size) {
    snprintf(json_buffer, buffer_size,
        "{"
        "\"ms_since_boot\":%u,"
        "\"flight_mode\":%d,"
        "\"battery\":%.2f,"
        "\"pt3\":%.2f,"
        "\"pt4\":%.2f,"
        "\"rtd_temp\":%.2f,"
        "\"altitude\":%.2f"
        "}",
        packet.ms_since_boot,
        packet.metadata.flight_mode,
        packet.battery_voltage,
        packet.pt3_pressure,
        packet.pt4_pressure,
        packet.rtd_temperature,
        packet.altitude
    );
}