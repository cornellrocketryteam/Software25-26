#ifndef RATS_CONFIG_H
#define RATS_CONFIG_H

// ============================================================================
// RATS System Configuration
// Shared constants for both RadioPico and StepperPico
// ============================================================================

// Packet sync word - "CRT!" (Cornell Rocket Team)
#define SYNC_WORD 0x3E5D5967

// Telemetry rates
#define EXPECTED_PACKET_RATE_HZ 10
#define PACKET_INTERVAL_MS (1000 / EXPECTED_PACKET_RATE_HZ)

// Link loss detection
#define LINK_LOST_TIMEOUT_MS 500  // Consider link lost after 500ms no packets

// Ground station location (update with actual coordinates)
// TODO: Set these to your actual launch site coordinates
#define GROUND_STATION_LAT_DEG 42.356000  // Cornell area default
#define GROUND_STATION_LON_DEG -76.497000
#define GROUND_STATION_ALT_M 100.0        // meters above sea level

// UART configuration for RFD900x (RadioPico)
#define RFD900X_BAUD_RATE 57600
#define RFD900X_DATA_BITS 8
#define RFD900X_STOP_BITS 1
#define RFD900X_PARITY 0  // No parity

// UART configuration for inter-Pico communication
#define INTER_PICO_BAUD_RATE 115200

// Buffer sizes
#define RFD_RX_BUFFER_SIZE 512
#define RADIO_PACKET_SIZE 107
#define TRACKING_DATA_SIZE 12

// SD card logging
#define SD_LOG_BATCH_SIZE 10  // Write every N packets

// MQTT configuration (for future use)
#define MQTT_BROKER_ADDRESS "mqtt.example.com"
#define MQTT_BROKER_PORT 1883
#define MQTT_TOPIC_PREFIX "rats/telemetry"

// Status LED blink patterns (milliseconds)
#define LED_BLINK_NORMAL 1000     // Normal operation
#define LED_BLINK_NO_LINK 250     // Lost radio link
#define LED_BLINK_ERROR 100       // Error condition

#endif // RATS_CONFIG_H