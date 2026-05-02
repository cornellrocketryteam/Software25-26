#include "packet_simulator.h"
#include "config.h"
#include "math.h"
#include <string.h>

PacketSimulator::PacketSimulator()
    : sim_time_ms(0), current_mode(STANDBY), sim_altitude(100.0f),
      sim_velocity(0.0f) {
  // Flight mode states: STANDBY, ASCENT, DROGUE_DEPLOYED, MAIN_DEPLOYED
}

void PacketSimulator::updateSimulation() {
#if MATH_TEST_MODE
  sim_time_ms += 50; // Real-time for motor testing
#else
  sim_time_ms += 1000; // 1 second per 50ms real loop tick
#endif

  // Simple flight simulation
  switch (current_mode) {
  case STANDBY:
    if (sim_time_ms > 1000) {
      current_mode = ASCENT;
    }
    break;

  case ASCENT:
    sim_velocity = 200.0f; // Slower ascent to take ~15 seconds to reach apogee
    sim_altitude += sim_velocity * 1.0f;
    if (sim_altitude > 3000.0f) {
      current_mode = DROGUE_DEPLOYED;
      sim_velocity = -25.0f; // Drogue falls at ~25m/s
    }
    break;

  case DROGUE_DEPLOYED:
    sim_altitude += sim_velocity * 1.0f;
    if (sim_altitude < 500.0f) {
      current_mode = MAIN_DEPLOYED; // Hits main at 500m (after ~100s in drogue)
      sim_velocity = -5.0f;         // Main falls slowly at ~5m/s
    }
    break;

  case MAIN_DEPLOYED:
    sim_altitude += sim_velocity * 1.0f;
    if (sim_altitude < 100.0f) { // Lands softly at 100m MSL
      sim_altitude = 100.0f;
      sim_velocity = 0.0f;
    }
    break;

  default:
    break;
  }
}

void PacketSimulator::generateRadioPacket(RadioPacket &packet) {
#if MATH_TEST_MODE
  updateSimulation();
  memset(&packet, 0, sizeof(RadioPacket));
  packet.sync_word = SYNC_WORD;
  packet.flight_mode = ASCENT;
  packet.timestamp = sim_time_ms / 1000.0f;
  
  float t = sim_time_ms / 1000.0f;
  
  // Distance east when lon = 0.001 deg at the equator: ~111.19 m.
  // Altitude for elevation X = distance * tan(X).
  // Each stage is ~3 s so the Kalman filter has time to settle between discontinuous position jumps.
  if (t < 3.0f) {
      // Pure North, horizon
      packet.latitude = 0.001f * 1e6;
      packet.longitude = 0.0f;
      packet.altitude = 0.0f;
  } else if (t < 6.0f) {
      // Az 30, horizon
      packet.latitude = 0.000866f * 1e6;
      packet.longitude = 0.0005f * 1e6;
      packet.altitude = 0.0f;
  } else if (t < 9.0f) {
      // Az 60, horizon
      packet.latitude = 0.0005f * 1e6;
      packet.longitude = 0.000866f * 1e6;
      packet.altitude = 0.0f;
  } else if (t < 13.0f) {
      // Az 90 (East), El 0
      packet.latitude = 0.0f;
      packet.longitude = 0.001f * 1e6;
      packet.altitude = 0.0f;
  } else if (t < 16.0f) {
      // Az 90, El 30
      packet.latitude = 0.0f;
      packet.longitude = 0.001f * 1e6;
      packet.altitude = 64.20f;
  } else if (t < 19.0f) {
      // Az 90, El 60
      packet.latitude = 0.0f;
      packet.longitude = 0.001f * 1e6;
      packet.altitude = 192.58f;
  } else {
      // Az 90, El ~88-89° (very high altitude — avoids the atan2 singularity at true zenith)
      packet.latitude = 0.0f;
      packet.longitude = 0.001f * 1e6;
      packet.altitude = 6370.0f; // 111.19 * tan(89°); apogee clamp drops it to 3048 m → ~87.9°
  }
  
  return;
#else

  updateSimulation();

  // Zero out everything first
  memset(&packet, 0, sizeof(RadioPacket));

  // Sync word
  packet.sync_word = SYNC_WORD; // "CRT!"

  packet.flight_mode = current_mode;
  packet.timestamp = sim_time_ms / 1000.0f;

  // Altimeter data
  packet.altitude = sim_altitude;
  packet.temp = 20.0f - (sim_altitude / 150.0f); // Temperature lapse rate
  packet.pressure = 1013.25f * powf(1.0f - 2.25577e-5f * sim_altitude, 5.25588f);

  float t = sim_time_ms / 1000.0f;

  if (current_mode != MAIN_DEPLOYED || sim_altitude > 100.0f) {
    // Rocket in flight: drift North but swing East and West smoothly
    // to force the Azimuth tracking motor to sweep back and forth visually
    packet.latitude = 42.336789f + 0.001f * t;
    packet.longitude = -76.497123f + 0.002f * sinf(t * 0.1f);
  } else {
    // Rocket landed: keep the final frozen position based on the exact time it landed!
    static bool landed = false;
    static float final_lat, final_lon;

    if (!landed) {
      final_lat = 42.336789f + 0.001f * t;
      final_lon = -76.497123f + 0.002f * sinf(t * 0.1f);
      landed = true;
    }

    packet.latitude = final_lat;
    packet.longitude = final_lon;
  }

  packet.num_satellites = 12;

  // IMU data - simulate some motion
  packet.accel_x = 0.1f * sinf(t);
  packet.accel_y = 0.1f * cosf(t);
  packet.accel_z = (current_mode == MAIN_DEPLOYED && sim_altitude <= 100.0f)
                           ? 9.81f
                           : 9.81f + 1.2f * sinf(t * 2.0f);
  packet.gyro_x = (current_mode == MAIN_DEPLOYED && sim_altitude <= 100.0f)
                          ? 0.0f
                          : 5.0f * sinf(t * 0.5f);
  packet.gyro_y = (current_mode == MAIN_DEPLOYED && sim_altitude <= 100.0f)
                          ? 0.0f
                          : 5.0f * cosf(t * 0.5f);
  packet.gyro_z =
      (current_mode == MAIN_DEPLOYED && sim_altitude <= 100.0f) ? 0.0f : 1.0f;
  packet.mag_x =
      (current_mode == MAIN_DEPLOYED && sim_altitude <= 100.0f)
          ? 0.0f
          : 10.0f * sinf(t * 0.2f);
  packet.mag_y =
      (current_mode == MAIN_DEPLOYED && sim_altitude <= 100.0f)
          ? 0.0f
          : 10.0f * cosf(t * 0.2f);
  packet.mag_z = 45.0f;

  // ADC and BLiMS data
  packet.pt3 = 800.0f + 50.0f * sinf(t * 0.1f);
  packet.pt4 = 750.0f + 30.0f * cosf(t * 0.1f);
  packet.rtd = 25.0f + 2.0f * sinf(t * 0.05f);
  packet.blims_motor_position = (current_mode == MAIN_DEPLOYED) ? 2.5f : 0.0f;
#endif
}

void PacketSimulator::serializeRadioPacket(const RadioPacket &packet,
                                           uint8_t *buffer) {
  // Serialize full 202-byte Radio Packet
  memcpy(buffer, &packet, sizeof(RadioPacket));
}
