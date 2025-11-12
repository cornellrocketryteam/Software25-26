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

// Set to 1 to enable loopback test mode (jumper GP4 to GP5)
// Set to 0 for normal operation with RFD900x
#define LOOPBACK_TEST_MODE 1

// Inter-core communication queue
queue_t packet_queue;

// Core 1 Entry Point - Processing and Logging
void core1_entry() {
    printf("[Core 1] Started - Processing & Logging\n");
    
    char json_buffer[2048];
    RadioPacket packet;
    
    while (true) {
        // Wait for packets from Core 0
        if (queue_try_remove(&packet_queue, &packet)) {
            // Convert to JSON (can be slow, that's OK on Core 1)
            PacketParser::radioPacketToJSON(packet, json_buffer, sizeof(json_buffer));
            
            // Send to USB serial (ground station)
            printf("%s\n", json_buffer);
            
            // TODO: Add SD card logging here
            // TODO: Add MQTT publishing here
        }
        
        // Core 1 can afford to sleep
        sleep_ms(5);
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
    printf("Connect GP4 to GP5 for self-test\n");
    printf("Simulating rocket telemetry at 10Hz\n\n");
#else
    printf("*** NORMAL OPERATION MODE ***\n");
    printf("Connect RFD900x:\n");
    printf("  RFD Pin 1,2 (GND) -> Pico GND\n");
    printf("  RFD Pin 4 (Vcc) -> External 5V supply\n");
    printf("  RFD Pin 7 (RX) -> Pico GP4 (TX)\n");  // Only if sending needed
    printf("  RFD Pin 9 (TX) -> Pico GP5 (RX)\n\n");
#endif
    
    // Initialize inter-core queue (holds up to 64 packets)
    queue_init(&packet_queue, sizeof(RadioPacket), 64);
    
    // Launch Core 1
    multicore_launch_core1(core1_entry);
    
    // Initialize UART
    RFD900xUART::init();
    printf("[Core 0] Ready for packets\n\n");
    
#if LOOPBACK_TEST_MODE
    // Test mode: create simulator for generating packets
    PacketSimulator simulator;
    uint32_t last_transmit_time = 0;
    printf("Starting packet transmission...\n\n");
#endif
    
    // Core 0 main loop - FAST I/O ONLY
    uint8_t radio_buffer[107];
    RadioPacket parsed_packet;
    uint32_t packet_count = 0;
    uint32_t last_stats_time = 0;
    
    while (true) {
#if LOOPBACK_TEST_MODE
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
            
            // Transmit over UART
            uart_write_blocking(RFD_UART_ID, tx_buffer, sizeof(tx_buffer));
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
        
        // Stats every 10 seconds
        uint32_t now_stats = to_ms_since_boot(get_absolute_time());
        if (now_stats - last_stats_time > 10000) {
            uint32_t total_packets, errors, bytes;
            RFD900xUART::getStats(total_packets, errors, bytes);
            
            static uint32_t last_packet_count = 0;
            float packets_per_sec = (total_packets - last_packet_count) / 10.0f;
            last_packet_count = total_packets;
            
            printf("[Core 0] Stats: %u packets (%.1f Hz), %u errors, %u bytes\n", 
                   total_packets, packets_per_sec, errors, bytes);
            last_stats_time = now_stats;
        }
        
        // Minimal delay - Core 0 stays responsive
        tight_loop_contents();
    }
    
    return 0;
}