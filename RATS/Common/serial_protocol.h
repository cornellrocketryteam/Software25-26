#ifndef SERIAL_PROTOCOL_H
#define SERIAL_PROTOCOL_H

#include <stdint.h>

// Inter-Pico communication protocol
// RadioPico (UART1 RX) -> StepperPico (UART0 RX)
// Baud: 115200, 8N1

// Minimal tracking data packet (12 bytes)
// Sent at 10Hz from RadioPico to StepperPico
struct TrackingData {
    int32_t latitude_udeg;   // 4 bytes - microdegrees (1e-6 degrees)
    int32_t longitude_udeg;  // 4 bytes - microdegrees
    float altitude;          // 4 bytes - meters above sea level
} __attribute__((packed));

// Sync byte to detect packet start (optional, for future use)
#define TRACKING_SYNC_BYTE 0xAA

// Helper functions for conversion
inline float lat_udeg_to_degrees(int32_t udeg) {
    return udeg / 1000000.0f;
}

inline float lon_udeg_to_degrees(int32_t udeg) {
    return udeg / 1000000.0f;
}

inline int32_t degrees_to_udeg(float degrees) {
    return (int32_t)(degrees * 1000000.0f);
}

#endif // SERIAL_PROTOCOL_H