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
    
    // Metadata
    packet.metadata.altitude_armed = true;
    packet.metadata.altimeter_valid = true;
    packet.metadata.gps_valid = true;
    packet.metadata.imu_valid = true;
    packet.metadata.accelerometer_valid = true;
    packet.metadata.umbilical_locked = (current_mode == STANDBY);
    packet.metadata.adc_valid = true;
    packet.metadata.fram_valid = true;
    packet.metadata.sd_valid = true;
    packet.metadata.gps_fresh = true;
    packet.metadata.safed = false;
    packet.metadata.mav_state = (current_mode == ASCENT);
    packet.metadata.sv_state = false;
    packet.metadata.flight_mode = current_mode;
    
    // Events (mostly false for normal operation)
    memset(&packet.events, 0, sizeof(Events));
    if (current_mode == ASCENT && sim_time_ms == 5100) {
        packet.events.launch_cmd = true;
    }
    
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

void PacketSimulator::generateUmbilicalPacket(UmbilicalPacket& packet) {
    packet.ms_since_boot = sim_time_ms;
    
    packet.metadata.altitude_armed = false;
    packet.metadata.altimeter_valid = true;
    packet.metadata.gps_valid = false;
    packet.metadata.imu_valid = false;
    packet.metadata.accelerometer_valid = false;
    packet.metadata.umbilical_locked = true;
    packet.metadata.adc_valid = true;
    packet.metadata.fram_valid = true;
    packet.metadata.sd_valid = true;
    packet.metadata.gps_fresh = false;
    packet.metadata.safed = false;
    packet.metadata.mav_state = false;
    packet.metadata.sv_state = false;
    packet.metadata.flight_mode = STANDBY;
    
    memset(&packet.events, 0, sizeof(Events));
    
    packet.battery_voltage = 12.6f;
    packet.pt3_pressure = 0.0f;
    packet.pt4_pressure = 0.0f;
    packet.rtd_temperature = 20.0f;
    packet.altitude = 100.0f;
}

void PacketSimulator::serializeRadioPacket(const RadioPacket& packet, uint8_t* buffer) {
    size_t offset = 0;
    
    memcpy(buffer + offset, &packet.sync_word, sizeof(uint32_t));
    offset += sizeof(uint32_t);
    
    // Pack metadata into uint16_t
    uint16_t meta_bits = 0;
    meta_bits |= (packet.metadata.altitude_armed << 0);
    meta_bits |= (packet.metadata.altimeter_valid << 1);
    meta_bits |= (packet.metadata.gps_valid << 2);
    meta_bits |= (packet.metadata.imu_valid << 3);
    meta_bits |= (packet.metadata.accelerometer_valid << 4);
    meta_bits |= (packet.metadata.umbilical_locked << 5);
    meta_bits |= (packet.metadata.adc_valid << 6);
    meta_bits |= (packet.metadata.fram_valid << 7);
    meta_bits |= (packet.metadata.sd_valid << 8);
    meta_bits |= (packet.metadata.gps_fresh << 9);
    meta_bits |= (packet.metadata.safed << 10);
    meta_bits |= (packet.metadata.mav_state << 11);
    meta_bits |= (packet.metadata.sv_state << 12);
    meta_bits |= (packet.metadata.flight_mode << 13);
    memcpy(buffer + offset, &meta_bits, sizeof(uint16_t));
    offset += sizeof(uint16_t);
    
    memcpy(buffer + offset, &packet.ms_since_boot, sizeof(uint32_t));
    offset += sizeof(uint32_t);
    
    // Pack events into uint32_t
    uint32_t event_bits = 0;
    event_bits |= (packet.events.altitude_armed << 0);
    event_bits |= (packet.events.altimeter_init_failed << 1);
    event_bits |= (packet.events.altimeter_read_failed << 2);
    event_bits |= (packet.events.gps_init_failed << 3);
    event_bits |= (packet.events.gps_read_failed << 4);
    event_bits |= (packet.events.imu_init_failed << 5);
    event_bits |= (packet.events.imu_read_failed << 6);
    event_bits |= (packet.events.accel_init_failed << 7);
    event_bits |= (packet.events.accel_read_failed << 8);
    event_bits |= (packet.events.adc_init_failed << 9);
    event_bits |= (packet.events.adc_read_failed << 10);
    event_bits |= (packet.events.fram_init_failed << 11);
    event_bits |= (packet.events.fram_read_failed << 12);
    event_bits |= (packet.events.fram_write_failed << 13);
    event_bits |= (packet.events.sd_init_failed << 14);
    event_bits |= (packet.events.sd_write_failed << 15);
    event_bits |= (packet.events.mav_actuated << 16);
    event_bits |= (packet.events.sv_actuated << 17);
    event_bits |= (packet.events.main_deploy_wait_end << 18);
    event_bits |= (packet.events.main_log_shutoff << 19);
    event_bits |= (packet.events.cycle_overflow << 20);
    event_bits |= (packet.events.unknown_cmd << 21);
    event_bits |= (packet.events.launch_cmd << 22);
    event_bits |= (packet.events.mav_cmd << 23);
    event_bits |= (packet.events.sv_cmd << 24);
    event_bits |= (packet.events.safe_cmd << 25);
    event_bits |= (packet.events.reset_card_cmd << 26);
    event_bits |= (packet.events.reset_fram_cmd << 27);
    event_bits |= (packet.events.state_change_cmd << 28);
    event_bits |= (packet.events.umbilical_disconnected << 29);
    memcpy(buffer + offset, &event_bits, sizeof(uint32_t));
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

void PacketSimulator::serializeUmbilicalPacket(const UmbilicalPacket& packet, uint8_t* buffer) {
    size_t offset = 0;
    
    uint16_t meta_bits = 0;
    meta_bits |= (packet.metadata.altitude_armed << 0);
    meta_bits |= (packet.metadata.altimeter_valid << 1);
    meta_bits |= (packet.metadata.umbilical_locked << 5);
    meta_bits |= (packet.metadata.adc_valid << 6);
    meta_bits |= (packet.metadata.flight_mode << 13);
    memcpy(buffer + offset, &meta_bits, sizeof(uint16_t));
    offset += sizeof(uint16_t);
    
    memcpy(buffer + offset, &packet.ms_since_boot, sizeof(uint32_t));
    offset += sizeof(uint32_t);
    
    uint32_t event_bits = 0;
    memcpy(buffer + offset, &event_bits, sizeof(uint32_t));
    offset += sizeof(uint32_t);
    
    memcpy(buffer + offset, &packet.battery_voltage, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.pt3_pressure, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.pt4_pressure, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.rtd_temperature, sizeof(float));
    offset += sizeof(float);
    memcpy(buffer + offset, &packet.altitude, sizeof(float));
}