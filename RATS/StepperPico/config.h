#ifndef RATS_CONFIG_H
#define RATS_CONFIG_H

// ============================================================================
// RATS System Configuration
// Shared constants for both RadioPico and StepperPico
// ============================================================================

// --- Wi-Fi & MQTT Configuration ---
#define WIFI_SSID "CornellRocketry-2.4G" // <-- SET THIS
#define WIFI_PASS "Rocketry2526"     // <-- SET THIS

// RATS Unit ID (for MQTT topic)
// 0 = Umbilical, 1 = Primary RATS, 2 = Secondary RATS, etc.
#define RATS_UNIT_ID 1

// MQTT Broker Address (e.g., Mini PC's IP)
#define MQTT_BROKER_ADDRESS "192.168.1.2" // <-- SET THIS
#define MQTT_BROKER_PORT 1883

// Base topic. The unit ID will be appended.
// e.g., "rats/raw/1"
#define MQTT_TOPIC_BASE "rats/raw/"

// The final topic this Pico will publish to
// Note: This requires RATS_UNIT_ID to be a simple integer
#define STRINGIFY(x) #x
#define TOSTRING(x) STRINGIFY(x)
#define MQTT_TOPIC MQTT_TOPIC_BASE TOSTRING(RATS_UNIT_ID)

// --- System Configuration ---

// Packet sync word - "CRT!"
#define SYNC_WORD 0x3E5D5967

// Telemetry rates
#define EXPECTED_PACKET_RATE_HZ 10
#define PACKET_INTERVAL_MS (1000 / EXPECTED_PACKET_RATE_HZ)

// Link loss detection
#define LINK_LOST_TIMEOUT_MS 500  // Consider link lost after 500ms no packets

// Ground station location (update with actual coordinates)
// Because we are using gps module, these are just defaults for testing
#define GROUND_STATION_LAT_DEG 42.356000  // Cornell area default
#define GROUND_STATION_LON_DEG -76.497000
#define GROUND_STATION_ALT_M 100.0        // meters above sea level

// UART configuration for RFD900x (RadioPico)
#define RFD900X_BAUD_RATE 115200  // Match RFD900x serial speed setting
#define RFD900X_DATA_BITS 8
#define RFD900X_STOP_BITS 1
#define RFD900X_PARITY 0  // No parity

// UART configuration for inter-Pico communication
#define INTER_PICO_BAUD_RATE 115200

// Buffer sizes
#define RFD_RX_BUFFER_SIZE 512
#define RADIO_PACKET_SIZE 107  // Full Radio Packet structure per RATS specification
#define TRACKING_DATA_SIZE 12

// SD card logging
#define SD_LOG_BATCH_SIZE 10  // Write every N packets

// Status LED blink patterns (milliseconds)
#define LED_BLINK_NORMAL 1000     // Normal operation
#define LED_BLINK_NO_LINK 250     // Lost radio link
#define LED_BLINK_ERROR 100       // Error condition

#endif // RATS_CONFIG_H