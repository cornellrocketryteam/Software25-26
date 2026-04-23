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

    // Zero out everything first
    memset(&packet, 0, sizeof(RadioPacket));

    // Sync word
    packet.sync_word = SYNC_WORD;  // "CRT!"

    packet.flight_mode = current_mode;
    packet.timestamp = sim_time_ms / 1000.0f;

    // Altimeter data
    packet.altitude = sim_altitude;
    packet.temp = 20.0f - (sim_altitude / 150.0f); // Temperature lapse rate
    packet.pressure = 1013.25f * powf(1.0f - 2.25577e-5f * sim_altitude, 5.25588f);

    // GPS data (Ithaca, NY area)
    packet.latitude = 42.356789f;
    packet.longitude = -76.497123f;
    packet.num_satellites = 12;

    // IMU data - simulate some motion
    float t = sim_time_ms / 1000.0f;
    packet.accel_x = 0.1f * sinf(t);
    packet.accel_y = 0.1f * cosf(t);
    packet.accel_z = 9.81f;  // Gravity
    packet.gyro_x = 5.0f * sinf(t * 0.5f);
    packet.gyro_y = 5.0f * cosf(t * 0.5f);
    packet.gyro_z = 1.0f;
    packet.mag_x = 10.0f * sinf(t * 0.2f);
    packet.mag_y = 10.0f * cosf(t * 0.2f);
    packet.mag_z = 45.0f;

    // ADC and BLiMS data
    packet.pt3 = 800.0f + 50.0f * sinf(t * 0.1f);
    packet.pt4 = 750.0f + 30.0f * cosf(t * 0.1f);
    packet.rtd = 25.0f + 2.0f * sinf(t * 0.05f);
    packet.blims_motor_position = (current_mode == MAIN_DEPLOYED) ? 2.5f : 0.0f;
}

void PacketSimulator::serializeRadioPacket(const RadioPacket& packet, uint8_t* buffer) {
    // Serialize full 199-byte Radio Packet
    memcpy(buffer, &packet, sizeof(RadioPacket));
}
