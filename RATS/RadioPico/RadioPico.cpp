#include <stdio.h>
#include <cstring>
#include "pico/stdlib.h"
#include "pico/multicore.h"
#include "pico/util/queue.h"
#include "config.h"
#include "packet_types.h"
#include "packet_parser.h"
#include "rfd900x_uart.h"
#include "packet_simulator.h"
#include "sd_logger.h"

// Test Mode: Set to 1 for dual-radio test (TX+RX), 0 for normal operation (RX only)
// Normal operation: Rocket -> RFD900x -> GP1 (RX)
#define DUAL_RADIO_TEST_MODE 0

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

    char json_buffer[2048];
    RadioPacket packet;
    RadioPacket batch_buffer[SD_LOG_BATCH_SIZE];
    uint32_t batch_count = 0;
    uint32_t last_stats_time = 0;

    while (true) {
        // Wait for packets from Core 0
        if (queue_try_remove(&packet_queue, &packet)) {
            // Convert to JSON (can be slow, that's OK on Core 1)
            PacketParser::radioPacketToJSON(packet, json_buffer, sizeof(json_buffer));

            // Send to USB serial (ground station)
            printf("%s\n", json_buffer);

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

            // TODO: Add MQTT publishing here
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
        sleep_ms(5);
    }
}

int main() {
    stdio_init_all();
    sleep_ms(6000);
    
    printf("\n=== RadioPico - Dual RFD900x Test ===\n");
    printf("Core 0: Real-time I/O\n");
    printf("Core 1: Processing & Logging\n\n");

#if DUAL_RADIO_TEST_MODE
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
#else
    printf("*** NORMAL OPERATION MODE ***\n");
    printf("Connect RFD900x:\n");
    printf("  Pin 1,2 (GND) -> Pico GND\n");
    printf("  Pin 4 (Vcc) -> 5V supply\n");
    printf("  Pin 9 (TX) -> Pico GP1 (UART0 RX)\n\n");
#endif
    
    // Initialize inter-core queue (holds up to 64 packets)
    queue_init(&packet_queue, sizeof(RadioPacket), 64);
    
    // Launch Core 1
    multicore_launch_core1(core1_entry);
    
    // Initialize UART for RFD900x
    RFD900xUART::init();
    printf("[Core 0] Ready for packets\n\n");

#if DUAL_RADIO_TEST_MODE
    // Test mode: create simulator for generating packets
    PacketSimulator simulator;
    uint32_t last_transmit_time = 0;
    printf("Starting packet transmission over air...\n\n");
#endif
    
    // Core 0 main loop - FAST I/O ONLY
    uint8_t radio_buffer[107];
    RadioPacket parsed_packet;
    uint32_t packet_count = 0;
    uint32_t last_stats_time = 0;
    
    while (true) {
#if DUAL_RADIO_TEST_MODE
        // Transmit a test packet every 100ms (10Hz)
        uint32_t now = to_ms_since_boot(get_absolute_time());
        if (now - last_transmit_time >= 100) {
            last_transmit_time = now;

            // Generate simulated packet
            RadioPacket sim_packet;
            simulator.generateRadioPacket(sim_packet);

            // Serialize to bytes
            uint8_t tx_buffer[107];
            PacketSimulator::serializeRadioPacket(sim_packet, tx_buffer);

            // Transmit over UART0 to RFD900x #1 (transmit radio)
            uart_write_blocking(RFD_UART_ID, tx_buffer, sizeof(tx_buffer));

            // Debug: confirm transmission
            static uint32_t tx_count = 0;
            if (++tx_count % 10 == 0) {
                printf("[TX] Sent %u packets (Sync: 0x%08X, Lat: %d, Alt: %.1fm)\n",
                       tx_count, sim_packet.sync_word, sim_packet.latitude_udeg, sim_packet.altitude);
            }
        }
#endif
        
        // Check for packets (non-blocking, interrupt-driven)
        if (RFD900xUART::packetAvailable()) {
            // Read packet
            if (RFD900xUART::readPacket(radio_buffer, sizeof(radio_buffer))) {
                // Quick validation
                uint32_t potential_sync;
                memcpy(&potential_sync, radio_buffer, sizeof(uint32_t));
                
                if (potential_sync == SYNC_WORD) {
                    // Parse (fast operation)
                    if (PacketParser::parseRadioPacket(radio_buffer, sizeof(radio_buffer), parsed_packet)) {
                        packet_count++;
                        
                        // TODO: Send minimal data to Stepper Pico via UART0
                        // (Just GPS + altitude, ~12 bytes)
                        
                        // Send to Core 1 for processing (non-blocking)
                        if (!queue_try_add(&packet_queue, &parsed_packet)) {
                            printf("[Core 0] Warning: Queue full, packet dropped\n");
                        }
                    }
                } else {
                    // Possible umbilical packet or noise
                    printf("[Core 0] Non-radio packet detected\n");
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