#include <stdio.h>
#include <cstring>
#include "pico/stdlib.h"
#include "pico/multicore.h"
#include "pico/util/queue.h"
#include "pico/cyw43_arch.h"
#include "config.h"
#include "packet_types.h"
#include "packet_parser.h"
#include "rfd900x_uart.h"
#include "packet_simulator.h"
#include "sd_logger.h"
#include "mqtt_client.h"
#include "inter_pico_uart.h"

// Test Mode: Set to 1 for dual-radio test (TX+RX), 0 for normal operation (RX only)
// Normal operation: Rocket -> RFD900x -> GP1 (RX)
#define DUAL_RADIO_TEST_MODE 0

// Loopback Test Mode: Set to 1 to test with jumper wire GP0->GP1
// Tests: RX, SD logging, and inter-Pico UART (no MQTT)
// Hardware: Connect GP0 to GP1 with jumper wire
#define LOOPBACK_TEST_MODE 1

// Inter-core communication queue
queue_t packet_queue;

// Core 1 Entry Point - Processing and Logging
void core1_entry() {
    printf("[Core 1] Started - Processing & Logging\n");

    // Wait for Core 0 to finish initialization
    sleep_ms(2000);

    // Initialize SD card
    bool sd_ready = SDLogger::init();
    if (!sd_ready) {
        printf("[Core 1] WARNING: SD card failed to initialize - logging disabled\n");
    }

#if !LOOPBACK_TEST_MODE
    // Connect to Wi-Fi and MQTT broker (skip in loopback test mode)
    // Blocks until Wi-Fi is connected
    bool mqtt_ready = MqttClient::init();
    if (!mqtt_ready) {
        printf("[Core 1]: Failed to init MQTT client - datalink failure\n");
        while(true) {
            cyw43_arch_gpio_put(CYW43_WL_GPIO_LED_PIN, 1);
            sleep_ms(LED_BLINK_ERROR);
            cyw43_arch_gpio_put(CYW43_WL_GPIO_LED_PIN, 0);
            sleep_ms(LED_BLINK_ERROR);
        }
    }
#else
    bool mqtt_ready = false;  // MQTT disabled in loopback test
    printf("[Core 1] MQTT/WiFi disabled (loopback test mode)\n");
#endif

    char json_buffer[2048];
    RadioPacket packet;
    RadioPacket batch_buffer[SD_LOG_BATCH_SIZE];
    uint32_t batch_count = 0;
    uint32_t last_stats_time = 0;

    while (true) {
        // Poll Network Stack (only if MQTT is enabled)
#if !LOOPBACK_TEST_MODE
        MqttClient::poll();
#endif

        // Wait for packets from Core 0
        if (queue_try_remove(&packet_queue, &packet)) {
            // Convert to JSON (can be slow, that's OK on Core 1)
            PacketParser::radioPacketToJSON(packet, json_buffer, sizeof(json_buffer));

            // Add to batch buffer for SD logging
            if (sd_ready && batch_count < SD_LOG_BATCH_SIZE) {
                batch_buffer[batch_count++] = packet;

                // Write batch when full
                if (batch_count >= SD_LOG_BATCH_SIZE) {
                    if (SDLogger::logPacketBatch(batch_buffer, batch_count)) {
                        // Batch written successfully
                    } else {
                        printf("[Core 1] SD write error\n");
                    }
                    batch_count = 0;
                }
            }

            // Send individual packets over MQTT immediately
            if (mqtt_ready) {
                MqttClient::publish(json_buffer);
            }

        }

        // Print SD stats every 30 seconds
        uint32_t now = to_ms_since_boot(get_absolute_time());
        if (sd_ready && (now - last_stats_time > 30000)) {
            uint32_t packets, bytes, errors;
            SDLogger::getStats(packets, bytes, errors);
            printf("[SD Stats] Packets: %u | Bytes: %u | Errors: %u | File: %s\n",
                   packets, bytes, errors, SDLogger::getCurrentFilename());
            last_stats_time = now;
        }

        // Core 1 can afford to sleep
        sleep_ms(1);
    }
}

int main() {
    stdio_init_all();
    sleep_ms(6000);

    printf("\n=== RadioPico ===\n");
    printf("Core 0: Real-time I/O\n");
    printf("Core 1: Processing & Logging\n\n");

#if LOOPBACK_TEST_MODE
    printf("*** LOOPBACK TEST MODE ***\n");
    printf("Hardware Setup:\n");
    printf("  1. Connect GP0 to GP1 with jumper wire\n");
    printf("  2. SD card inserted (optional but recommended)\n");
    printf("  3. No RFD900x radio needed\n\n");
    printf("This test simulates full operation:\n");
    printf("  - Generates packets on GP0 (TX)\n");
    printf("  - Receives on GP1 (RX)\n");
    printf("  - Logs to SD card via Core 1\n");
    printf("  - Sends to StepperPico via GP4 (UART1)\n");
    printf("  - No MQTT/WiFi (faster testing)\n\n");

    // Test mode: create simulator for generating packets
    PacketSimulator simulator;
    uint32_t last_transmit_time = 0;
    printf("Starting loopback packet transmission at 10Hz...\n\n");

#elif DUAL_RADIO_TEST_MODE
    printf("*** DUAL RFD900x TEST MODE ***\n");
    printf("Transmit Radio (RFD #1):\n");
    printf("  Pin 1,2 (GND) -> Pico GND\n");
    printf("  Pin 4 (Vcc) -> 5V supply\n");
    printf("  Pin 7 (RX) -> Pico GP0 (UART0 TX)\n\n");
    printf("Receive Radio (RFD #2):\n");
    printf("  Pin 1,2 (GND) -> Pico GND\n");
    printf("  Pin 4 (Vcc) -> 5V supply\n");
    printf("  Pin 9 (TX) -> Pico GP1 (UART0 RX)\n\n");
    printf("Simulating rocket telemetry at 10Hz\n");
    printf("Both radios must have Network ID = 217\n\n");

    // Test mode: create simulator for generating packets
    PacketSimulator simulator;
    uint32_t last_transmit_time = 0;
    printf("Starting packet transmission over air...\n\n");
#else
    printf("*** NORMAL OPERATION MODE ***\n");
    printf("Connect RFD900x:\n");
    printf("  Pin 1,2 (GND) -> Pico GND\n");
    printf("  Pin 4 (Vcc) -> 5V supply\n");
    printf("  Pin 9 (TX) -> Pico GP1 (UART0 RX)\n\n");
#endif
    
    // Initialize inter-core queue (holds up to 64 packets)
    queue_init(&packet_queue, sizeof(RadioPacket), 64);

    // Initialize inter-Pico UART (RadioPico -> StepperPico)
    InterPicoUART::init();

    // Launch Core 1
    multicore_launch_core1(core1_entry);

    // Initialize UART for RFD900x
    RFD900xUART::init();
    printf("[Core 0] Ready for packets\n\n");
    
    // Core 0 main loop - FAST I/O ONLY
    // Full 107-byte Radio Packet buffer
    uint8_t radio_buffer[107];
    RadioPacket parsed_packet;
    uint32_t packet_count = 0;
    uint32_t last_stats_time = 0;
    
    while (true) {
#if DUAL_RADIO_TEST_MODE || LOOPBACK_TEST_MODE
        // Transmit a test packet every 100ms (10Hz)
        uint32_t now = to_ms_since_boot(get_absolute_time());
        if (now - last_transmit_time >= 100) {
            last_transmit_time = now;

            // Generate full 107-byte Radio Packet
            RadioPacket sim_packet;
            simulator.generateRadioPacket(sim_packet);

            // Serialize to bytes (107 bytes full structure)
            uint8_t tx_buffer[107];
            PacketSimulator::serializeRadioPacket(sim_packet, tx_buffer);

            // Transmit over UART0 to RFD900x #1 (or loopback to GP1 via GP0)
            uart_write_blocking(RFD_UART_ID, tx_buffer, sizeof(tx_buffer));

            // Debug: confirm transmission
            static uint32_t tx_count = 0;
            if (++tx_count % 10 == 0) {
                uint8_t flight_mode = (sim_packet.metadata >> 13) & 0x07;
                printf("[TX] Sent %u packets (Sync: 0x%08X, Mode: %u, Alt: %.1fm)\n",
                       tx_count, sim_packet.sync_word, flight_mode, sim_packet.altitude);
            }
        }
#endif
        
        // Check for packets (non-blocking, interrupt-driven)
        if (RFD900xUART::packetAvailable()) {
            // Read packet
            if (RFD900xUART::readPacket(radio_buffer, sizeof(radio_buffer))) {
                // Validate sync word
                uint32_t potential_sync;
                memcpy(&potential_sync, radio_buffer, sizeof(uint32_t));

                if (potential_sync == SYNC_WORD) {
                    // Parse (fast operation)
                    if (PacketParser::parseRadioPacket(radio_buffer, sizeof(radio_buffer), parsed_packet)) {
                        packet_count++;

                        // Send tracking data to Stepper Pico via UART1
                        InterPicoUART::sendTrackingData(
                            parsed_packet.latitude,
                            parsed_packet.longitude,
                            parsed_packet.altitude
                        );

                        // Send to Core 1 for processing (non-blocking)
                        if (!queue_try_add(&packet_queue, &parsed_packet)) {
                            printf("[Core 0] Warning: Queue full, packet dropped\n");
                        }
                    }
                } else {
                    printf("[Core 0] Invalid sync word: 0x%08X (expected 0x%08X)\n", potential_sync, SYNC_WORD);
                }
            }
        }
        
        // Stats every 5 seconds
        uint32_t now_stats = to_ms_since_boot(get_absolute_time());
        if (now_stats - last_stats_time > 5000) {
            uint32_t total_packets, errors, bytes;
            RFD900xUART::getStats(total_packets, errors, bytes);

            static uint32_t last_packet_count = 0;
            static uint32_t last_byte_count = 0;
            float packets_per_sec = (total_packets - last_packet_count) / 5.0f;
            uint32_t bytes_received = bytes - last_byte_count;
            last_packet_count = total_packets;
            last_byte_count = bytes;

            printf("[RX Stats] Packets: %u (%.1f Hz) | Bytes: %u (%u new) | Errors: %u | Buffer: %u bytes\n",
                   total_packets, packets_per_sec, bytes, bytes_received, errors, RFD900xUART::available());
            last_stats_time = now_stats;
        }
        
        // Minimal delay - Core 0 stays responsive
        tight_loop_contents();
    }
    
    return 0;
}