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
    // Full 107-byte Radio Packet per RATS specification
    if (length < 107) return false;

    size_t offset = 0;

    // Byte 0-3: Sync word
    packet.sync_word = readValue<uint32_t>(buffer, offset);

    // Byte 4-5: Metadata
    packet.metadata = readValue<uint16_t>(buffer, offset);

    // Byte 6-9: Milliseconds since boot
    packet.ms_since_boot = readValue<uint32_t>(buffer, offset);

    // Byte 10-13: Events
    packet.events = readValue<uint32_t>(buffer, offset);

    // Byte 14-21: Altimeter data
    packet.altitude = readValue<float>(buffer, offset);
    packet.temperature = readValue<float>(buffer, offset);

    // Byte 22-38: GPS data
    packet.latitude = readValue<int32_t>(buffer, offset);
    packet.longitude = readValue<int32_t>(buffer, offset);
    packet.num_satellites = readValue<uint8_t>(buffer, offset);
    packet.gps_unix_time = readValue<uint32_t>(buffer, offset);
    packet.gps_horizontal_accuracy = readValue<uint32_t>(buffer, offset);

    // Byte 39-74: IMU data
    packet.imu_accel_x = readValue<float>(buffer, offset);
    packet.imu_accel_y = readValue<float>(buffer, offset);
    packet.imu_accel_z = readValue<float>(buffer, offset);
    packet.imu_gyro_x = readValue<float>(buffer, offset);
    packet.imu_gyro_y = readValue<float>(buffer, offset);
    packet.imu_gyro_z = readValue<float>(buffer, offset);
    packet.imu_orient_x = readValue<float>(buffer, offset);
    packet.imu_orient_y = readValue<float>(buffer, offset);
    packet.imu_orient_z = readValue<float>(buffer, offset);

    // Byte 75-86: Accelerometer data
    packet.accel_x = readValue<float>(buffer, offset);
    packet.accel_y = readValue<float>(buffer, offset);
    packet.accel_z = readValue<float>(buffer, offset);

    // Byte 87-106: ADC and BLiMS data
    packet.battery_voltage = readValue<float>(buffer, offset);
    packet.pt3_pressure = readValue<float>(buffer, offset);
    packet.pt4_pressure = readValue<float>(buffer, offset);
    packet.rtd_temperature = readValue<float>(buffer, offset);
    packet.blims_motor_state = readValue<float>(buffer, offset);

    return true;
}

void PacketParser::radioPacketToJSON(const RadioPacket& packet, char* json_buffer, size_t buffer_size) {
    // Full JSON output for complete Radio Packet structure
    // Extract flight mode from metadata (bits 13-15)
    uint8_t flight_mode = (packet.metadata >> 13) & 0x07;

    // Convert GPS coordinates from micro-degrees to decimal degrees
    float lat_deg = packet.latitude / 1000000.0f;
    float lon_deg = packet.longitude / 1000000.0f;

    snprintf(json_buffer, buffer_size,
        "{"
        "\"metadata\":%u,"
        "\"flight_mode\":%u,"
        "\"ms_since_boot\":%u,"
        "\"events\":%u,"
        "\"altitude\":%.2f,"
        "\"temperature\":%.2f,"
        "\"latitude\":%.6f,"
        "\"longitude\":%.6f,"
        "\"num_satellites\":%u,"
        "\"gps_unix_time\":%u,"
        "\"gps_h_accuracy\":%u,"
        "\"imu_accel\":[%.3f,%.3f,%.3f],"
        "\"imu_gyro\":[%.3f,%.3f,%.3f],"
        "\"imu_orient\":[%.3f,%.3f,%.3f],"
        "\"accel\":[%.3f,%.3f,%.3f],"
        "\"battery_voltage\":%.2f,"
        "\"pt3_pressure\":%.2f,"
        "\"pt4_pressure\":%.2f,"
        "\"rtd_temp\":%.2f,"
        "\"blims_motor\":%.2f"
        "}",
        packet.metadata,
        flight_mode,
        packet.ms_since_boot,
        packet.events,
        packet.altitude,
        packet.temperature,
        lat_deg,
        lon_deg,
        packet.num_satellites,
        packet.gps_unix_time,
        packet.gps_horizontal_accuracy,
        packet.imu_accel_x, packet.imu_accel_y, packet.imu_accel_z,
        packet.imu_gyro_x, packet.imu_gyro_y, packet.imu_gyro_z,
        packet.imu_orient_x, packet.imu_orient_y, packet.imu_orient_z,
        packet.accel_x, packet.accel_y, packet.accel_z,
        packet.battery_voltage,
        packet.pt3_pressure,
        packet.pt4_pressure,
        packet.rtd_temperature,
        packet.blims_motor_state
    );
}
