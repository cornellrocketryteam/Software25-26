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
    if (length < 107) return false;  // Radio packet is 107 bytes
    
    size_t offset = 0;
    
    packet.sync_word = readValue<uint32_t>(buffer, offset);
    
    // Read raw bits from metadata and events
    packet.raw_metadata = readValue<uint16_t>(buffer, offset);    
    
    packet.ms_since_boot = readValue<uint32_t>(buffer, offset);

    packet.raw_events = readValue<uint32_t>(buffer, offset);   

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

void PacketParser::radioPacketToJSON(const RadioPacket& packet, char* json_buffer, size_t buffer_size) {
    // --- Format Events Array ---
    char events_buf[128] = "[";
    bool first_event = true;
    for (int i = 0; i < 32; i++) {
        // Check which bits in raw_events are 1
        if ((packet.raw_events >> i) & 1) {
            if (!first_event) {
                strncat(events_buf, ",", sizeof(events_buf) - strlen(events_buf) - 1);
            }
            char event_num[4];
            snprintf(event_num, 4, "%d", i);
            strncat(events_buf, event_num, sizeof(events_buf) - strlen(events_buf) - 1);
            first_event = false;
        }
    }
    strncat(events_buf, "]", sizeof(events_buf) - strlen(events_buf) - 1);


    // --- Build Final JSON ---
    // Using snprintf for safe string formatting
    int offset = 0;
    offset += snprintf(json_buffer + offset, buffer_size - offset, "{");
    
    // Main time field
    offset += snprintf(json_buffer + offset, buffer_size - offset, "\"time\":%u,", packet.unix_time);
    
    // Other non-struct fields
    offset += snprintf(json_buffer + offset, buffer_size - offset,
        "\"sync_word\":%u,"
        "\"ms_since_boot\":%u,",
        packet.sync_word,
        packet.ms_since_boot
    );

    // Metadata as individual booleans
    offset += snprintf(json_buffer + offset, buffer_size - offset,
        "\"metadata_altitude_armed\":%s,"
        "\"metadata_altimeter_is_valid\":%s,"
        "\"metadata_gps_is_valid\":%s,"
        "\"metadata_imu_is_valid\":%s,"
        "\"metadata_accelerometer_is_valid\":%s,"
        "\"metadata_umbilical_lock\":%s,"
        "\"metadata_adc_is_valid\":%s,"
        "\"metadata_fram_is_valid\":%s,"
        "\"metadata_sd_card_is_valid\":%s,"
        "\"metadata_gps_message_fresh\":%s,"
        "\"metadata_rocket_was_safed\":%s,"
        "\"metadata_mav_state\":%s,"
        "\"metadata_sv_state\":%s,"
        "\"metadata_flight_mode\":%d,",
        (packet.raw_metadata >> 0 & 1) ? "true" : "false",
        (packet.raw_metadata >> 1 & 1) ? "true" : "false",
        (packet.raw_metadata >> 2 & 1) ? "true" : "false",
        (packet.raw_metadata >> 3 & 1) ? "true" : "false",
        (packet.raw_metadata >> 4 & 1) ? "true" : "false",
        (packet.raw_metadata >> 5 & 1) ? "true" : "false",
        (packet.raw_metadata >> 6 & 1) ? "true" : "false",
        (packet.raw_metadata >> 7 & 1) ? "true" : "false",
        (packet.raw_metadata >> 8 & 1) ? "true" : "false",
        (packet.raw_metadata >> 9 & 1) ? "true" : "false",
        (packet.raw_metadata >> 10 & 1) ? "true" : "false",
        (packet.raw_metadata >> 11 & 1) ? "true" : "false",
        (packet.raw_metadata >> 12 & 1) ? "true" : "false",
        (packet.raw_metadata >> 13) & 0x7
    );

    // Events as array
    offset += snprintf(json_buffer + offset, buffer_size - offset, "\"events\":%s,", events_buf);

    // Rest of the data
    offset += snprintf(json_buffer + offset, buffer_size - offset,
        "\"altitude\":%.2f,"
        "\"temperature\":%.2f,"
        "\"latitude\":%d,"
        "\"longitude\":%d,"
        "\"satellites_in_view\":%u,"
        "\"unix_time\":%u,"
        "\"horizontal_accuracy\":%u,"
        "\"acceleration_x\":%.4f,"
        "\"acceleration_y\":%.4f,"
        "\"acceleration_z\":%.4f,"
        "\"gyro_x\":%.4f,"
        "\"gyro_y\":%.4f,"
        "\"gyro_z\":%.4f,"
        "\"orientation_x\":%.4f,"
        "\"orientation_y\":%.4f,"
        "\"orientation_z\":%.4f,"
        "\"accelerometer_x\":%.4f,"
        "\"accelerometer_y\":%.4f,"
        "\"accelerometer_z\":%.4f,"
        "\"battery_voltage\":%.2f,"
        "\"pt_3_pressure\":%.2f,"
        "\"pt_4_pressure\":%.2f,"
        "\"rtd_temperature\":%.2f,"
        "\"motor_state\":%.2f",
        packet.altitude,
        packet.temperature,
        packet.latitude_udeg,
        packet.longitude_udeg,
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

    snprintf(json_buffer + offset, buffer_size - offset, "}");
}
