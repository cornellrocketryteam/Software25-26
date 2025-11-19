#include "packet_simulator.h"
#include <string.h>
#include <math.h>
#include "../Common/config.h"

PacketSimulator::PacketSimulator() 
    : sim_time_ms(0), current_mode(STANDBY), sim_altitude(100.0f), sim_velocity(0.0f) {
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
    
    packet.sync_word = SYNC_WORD;
    packet.ms_since_boot = sim_time_ms;
    
    // Raw metadata
    packet.raw_metadata = 0;
    packet.raw_metadata |= (true << 0); // altitude_armed
    packet.raw_metadata |= (true << 1); // altimeter_valid
    packet.raw_metadata |= (true << 2); // gps_valid
    packet.raw_metadata |= (true << 3); // imu_valid
    packet.raw_metadata |= (true << 4); // accelerometer_valid
    packet.raw_metadata |= ((current_mode == STANDBY) << 5); // umbilical_locked
    packet.raw_metadata |= (true << 6); // adc_valid
    packet.raw_metadata |= (true << 7); // fram_valid
    packet.raw_metadata |= (true << 8); // sd_valid
    packet.raw_metadata |= (true << 9); // gps_fresh
    packet.raw_metadata |= (false << 10); // safed
    packet.raw_metadata |= ((current_mode == ASCENT) << 11); // mav_state
    packet.raw_metadata |= (false << 12); // sv_state
    packet.raw_metadata |= (current_mode << 13); // flight_mode
    
    // Events (mostly false for normal operation)
    bool launch_cmd = (current_mode == ASCENT && sim_time_ms == 5100);
    packet.raw_events = 0;
    packet.raw_events |= (launch_cmd << 22);

    
    // Altimeter
    packet.altitude = sim_altitude;
    packet.temperature = 20.0f + (sim_altitude / 100.0f);
    
    // GPS (simulated location near launch site)
    packet.latitude_udeg = 42356000;   // ~42.356° N (Ithaca area)
    packet.longitude_udeg = -76497000; // ~-76.497° W
    packet.satellites = 12;
    packet.unix_time = 1727740800 + (sim_time_ms / 1000);
    packet.horizontal_accuracy_mm = 2500;
    
    // IMU
    float accel_z = (current_mode == ASCENT) ? 20.0f : 9.81f;
    packet.accel_x = 0.1f * sinf(sim_time_ms * 0.001f);
    packet.accel_y = 0.05f * cosf(sim_time_ms * 0.001f);
    packet.accel_z = accel_z;
    packet.gyro_x = 0.5f;
    packet.gyro_y = -0.3f;
    packet.gyro_z = 0.1f;
    packet.orient_x = 0.0f;
    packet.orient_y = 90.0f;
    packet.orient_z = 0.0f;
    
    // Accelerometer
    packet.accel2_x = packet.accel_x / 9.81f;
    packet.accel2_y = packet.accel_y / 9.81f;
    packet.accel2_z = packet.accel_z / 9.81f;
    
    // ADC
    packet.battery_voltage = 12.4f - (sim_time_ms / 1000000.0f);
    packet.pt3_pressure = (current_mode == ASCENT) ? 800.0f : 0.0f;
    packet.pt4_pressure = (current_mode == ASCENT) ? 750.0f : 0.0f;
    packet.rtd_temperature = 25.0f;
    packet.motor_state = 0.0f;
}

void PacketSimulator::serializeRadioPacket(const RadioPacket& packet, uint8_t* buffer) {
    size_t offset = 0;
    
    memcpy(buffer + offset, &packet.sync_word, sizeof(uint32_t));
    offset += sizeof(uint32_t);
    
    // Pack metadata into uint16_t
    memcpy(buffer + offset, &packet.raw_metadata, sizeof(uint16_t));
    offset += sizeof(uint16_t);
    
    memcpy(buffer + offset, &packet.ms_since_boot, sizeof(uint32_t));
    offset += sizeof(uint32_t);
    
    // Pack events into uint32_t
    memcpy(buffer + offset, &packet.raw_events, sizeof(uint32_t));
    offset += sizeof(uint32_t);
    
    // Copy all float and int values
    memcpy(buffer + offset, &packet.altitude, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.temperature, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.latitude_udeg, sizeof(int32_t));
    offset += sizeof(int32_t);
    memcpy(buffer + offset, &packet.longitude_udeg, sizeof(int32_t));
    offset += sizeof(int32_t);
    memcpy(buffer + offset, &packet.satellites, sizeof(uint8_t));
    offset += sizeof(uint8_t);
    memcpy(buffer + offset, &packet.unix_time, sizeof(uint32_t));
    offset += sizeof(uint32_t);
    memcpy(buffer + offset, &packet.horizontal_accuracy_mm, sizeof(uint32_t));
    offset += sizeof(uint32_t);
    
    memcpy(buffer + offset, &packet.accel_x, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.accel_y, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.accel_z, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.gyro_x, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.gyro_y, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.gyro_z, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.orient_x, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.orient_y, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.orient_z, sizeof(float));
    offset += sizeof(float);
    
    memcpy(buffer + offset, &packet.accel2_x, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.accel2_y, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.accel2_z, sizeof(float));
    offset += sizeof(float);
    
    memcpy(buffer + offset, &packet.battery_voltage, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.pt3_pressure, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.pt4_pressure, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.rtd_temperature, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.motor_state, sizeof(float));
}
