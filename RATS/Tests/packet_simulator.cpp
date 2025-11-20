#include "packet_simulator.h"
#include <string.h>
#include <math.h>
#include "../Common/config.h"

PacketSimulator::PacketSimulator()
    : sim_time_ms(0), current_mode(STANDBY), sim_altitude(100.0f), sim_velocity(0.0f) {
    // Flight mode states: STANDBY, ASCENT, DROGUE_DEPLOYED, MAIN_DEPLOYED
}

void PacketSimulator::updateSimulation() {
    sim_time_ms += 100;  // 10Hz update rate

    // Simple flight simulation
    switch (current_mode) {
        case STANDBY:
            if (sim_time_ms > 5000) {
                current_mode = ASCENT;
            }
            break;

        case ASCENT:
            sim_velocity += 9.81f * 0.1f;  // Simplified acceleration
            sim_altitude += sim_velocity * 0.1f;
            if (sim_altitude > 3000.0f) {
                current_mode = DROGUE_DEPLOYED;
                sim_velocity = -20.0f;
            }
            break;

        case DROGUE_DEPLOYED:
            sim_altitude += sim_velocity * 0.1f;
            if (sim_altitude < 500.0f) {
                current_mode = MAIN_DEPLOYED;
                sim_velocity = -5.0f;
            }
            break;

        case MAIN_DEPLOYED:
            sim_altitude += sim_velocity * 0.1f;
            if (sim_altitude < 100.0f) {
                sim_altitude = 100.0f;
                sim_velocity = 0.0f;
            }
            break;

        default:
            break;
    }
}

void PacketSimulator::generateRadioPacket(RadioPacket& packet) {
    updateSimulation();

    // Sync word
    packet.sync_word = SYNC_WORD;  // "CRT!"

    // Metadata: Build 16-bit bitfield with flight mode in bits 13-15
    uint16_t metadata = 0;
    metadata |= (1 << 1);  // Altimeter valid
    metadata |= (1 << 2);  // GPS valid
    metadata |= (1 << 3);  // IMU valid
    metadata |= (1 << 4);  // Accelerometer valid
    metadata |= (1 << 6);  // ADC valid
    metadata |= (1 << 8);  // SD card valid
    metadata |= (current_mode << 13);  // Flight mode in bits 13-15
    packet.metadata = metadata;

    // Timestamp
    packet.ms_since_boot = sim_time_ms;

    // Events (no events for simulation)
    packet.events = 0;

    // Altimeter data
    packet.altitude = sim_altitude;
    packet.temperature = 20.0f - (sim_altitude / 150.0f); // Temperature lapse rate

    // GPS data (Ithaca, NY area in micro-degrees)
    packet.latitude = (int32_t)(42.356789f * 1000000.0f);  // Convert to µdeg
    packet.longitude = (int32_t)(-76.497123f * 1000000.0f);
    packet.num_satellites = 12;
    packet.gps_unix_time = 1700000000 + (sim_time_ms / 1000);  // Mock Unix time
    packet.gps_horizontal_accuracy = 2500;  // 2.5 meters in mm

    // IMU data - simulate some motion
    float t = sim_time_ms / 1000.0f;
    packet.imu_accel_x = 0.1f * sinf(t);
    packet.imu_accel_y = 0.1f * cosf(t);
    packet.imu_accel_z = 9.81f;  // Gravity
    packet.imu_gyro_x = 5.0f * sinf(t * 0.5f);
    packet.imu_gyro_y = 5.0f * cosf(t * 0.5f);
    packet.imu_gyro_z = 1.0f;
    packet.imu_orient_x = 10.0f * sinf(t * 0.2f);
    packet.imu_orient_y = 10.0f * cosf(t * 0.2f);
    packet.imu_orient_z = 45.0f;

    // Accelerometer data
    packet.accel_x = 0.05f;
    packet.accel_y = 0.03f;
    packet.accel_z = 1.0f;  // 1g

    // ADC and BLiMS data
    packet.battery_voltage = 7.4f - (sim_time_ms / 1000000.0f);  // Slow drain
    packet.pt3_pressure = 800.0f + 50.0f * sinf(t * 0.1f);
    packet.pt4_pressure = 750.0f + 30.0f * cosf(t * 0.1f);
    packet.rtd_temperature = 25.0f + 2.0f * sinf(t * 0.05f);
    packet.blims_motor_state = (current_mode == MAIN_DEPLOYED) ? 2.5f : 0.0f;
}

void PacketSimulator::serializeRadioPacket(const RadioPacket& packet, uint8_t* buffer) {
    // Serialize full 107-byte Radio Packet
    size_t offset = 0;

    // Byte 0-3: Sync word
    memcpy(buffer + offset, &packet.sync_word, sizeof(uint32_t));
    offset += sizeof(uint32_t);

    // Byte 4-5: Metadata
    memcpy(buffer + offset, &packet.metadata, sizeof(uint16_t));
    offset += sizeof(uint16_t);

    // Byte 6-9: Milliseconds since boot
    memcpy(buffer + offset, &packet.ms_since_boot, sizeof(uint32_t));
    offset += sizeof(uint32_t);

    // Byte 10-13: Events
    memcpy(buffer + offset, &packet.events, sizeof(uint32_t));
    offset += sizeof(uint32_t);

    // Byte 14-21: Altimeter data
    memcpy(buffer + offset, &packet.altitude, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.temperature, sizeof(float));
    offset += sizeof(float);

    // Byte 22-38: GPS data
    memcpy(buffer + offset, &packet.latitude, sizeof(int32_t));
    offset += sizeof(int32_t);
    memcpy(buffer + offset, &packet.longitude, sizeof(int32_t));
    offset += sizeof(int32_t);
    memcpy(buffer + offset, &packet.num_satellites, sizeof(uint8_t));
    offset += sizeof(uint8_t);
    memcpy(buffer + offset, &packet.gps_unix_time, sizeof(uint32_t));
    offset += sizeof(uint32_t);
    memcpy(buffer + offset, &packet.gps_horizontal_accuracy, sizeof(uint32_t));
    offset += sizeof(uint32_t);

    // Byte 39-74: IMU data
    memcpy(buffer + offset, &packet.imu_accel_x, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.imu_accel_y, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.imu_accel_z, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.imu_gyro_x, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.imu_gyro_y, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.imu_gyro_z, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.imu_orient_x, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.imu_orient_y, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.imu_orient_z, sizeof(float));
    offset += sizeof(float);

    // Byte 75-86: Accelerometer data
    memcpy(buffer + offset, &packet.accel_x, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.accel_y, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.accel_z, sizeof(float));
    offset += sizeof(float);

    // Byte 87-106: ADC and BLiMS data
    memcpy(buffer + offset, &packet.battery_voltage, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.pt3_pressure, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.pt4_pressure, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.rtd_temperature, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.blims_motor_state, sizeof(float));
    offset += sizeof(float);

    // Total: 107 bytes
}
