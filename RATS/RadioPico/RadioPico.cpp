/**
 * RadioPico - RATS Radio/Data Pico Main Application
 *
 * Full system: RFD900x reception, SD logging, MQTT, Inter-Pico UART
 *
 * Hardware:
 *   - GP1: UART0 RX <- RFD900x TX (Pin 9) - Telemetry input
 *   - GP4: UART1 TX -> Stepper Pico GP5 - Tracking data output
 *   - GP10-13: SPI1 - SD Card
 *
 * Architecture:
 *   - Core 0: Real-time I/O (UART RX, Inter-Pico TX)
 *   - Core 1: Processing (SD logging, MQTT publish)
 */

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
#include "sd_logger.h"
#include "mqtt_client.h"
#include "inter_pico_uart.h"

// =============================================================================
// Configuration
// =============================================================================

// Debug output level (0=minimal, 1=normal, 2=verbose)
#define DEBUG_LEVEL 1

// =============================================================================
// Inter-core Communication
// =============================================================================
queue_t packet_queue;

// =============================================================================
// Statistics
// =============================================================================
volatile uint32_t g_packets_received = 0;
volatile uint32_t g_packets_logged = 0;
volatile uint32_t g_packets_published = 0;
volatile uint32_t g_interpico_sent = 0;
volatile uint32_t g_sync_errors = 0;
volatile uint32_t g_queue_drops = 0;

// =============================================================================
// Core 1: Processing & Logging
// =============================================================================
void core1_entry() {
    printf("[Core 1] Starting - SD Card & MQTT\n");

    // Wait for Core 0 to initialize
    sleep_ms(2000);

    // --- SD Card Initialization ---
    printf("[Core 1] Initializing SD card...\n");
    bool sd_ready = SDLogger::init();
    if (sd_ready) {
        printf("[Core 1] SD card ready: %s\n", SDLogger::getCurrentFilename());
    } else {
        printf("[Core 1] WARNING: SD card init failed - logging disabled\n");
    }

    // --- WiFi & MQTT Initialization ---
    printf("[Core 1] Initializing WiFi...\n");
    printf("[Core 1] SSID: %s\n", WIFI_SSID);

    bool mqtt_ready = MqttClient::init();
    if (mqtt_ready) {
        printf("[Core 1] MQTT connected to %s:%d\n", MQTT_BROKER_ADDRESS, MQTT_BROKER_PORT);
        printf("[Core 1] Publishing to topic: %s\n", MQTT_TOPIC);
    } else {
        printf("[Core 1] WARNING: MQTT init failed - will retry in background\n");
    }

    // --- Processing Loop ---
    char json_buffer[2048];
    RadioPacket packet;
    RadioPacket batch_buffer[SD_LOG_BATCH_SIZE];
    uint32_t batch_count = 0;
    uint32_t last_stats_time = 0;
    uint32_t mqtt_retry_time = 0;

    printf("[Core 1] Entering main loop\n\n");

    while (true) {
        uint32_t now = to_ms_since_boot(get_absolute_time());

        // --- Poll MQTT network stack ---
        if (mqtt_ready) {
            MqttClient::poll();
        }

        // --- Retry MQTT connection if failed ---
        if (!mqtt_ready && (now - mqtt_retry_time > 30000)) {
            printf("[Core 1] Retrying MQTT connection...\n");
            mqtt_ready = MqttClient::init();
            mqtt_retry_time = now;
            if (mqtt_ready) {
                printf("[Core 1] MQTT reconnected!\n");
            }
        }

        // --- Process packets from Core 0 ---
        if (queue_try_remove(&packet_queue, &packet)) {
            // Convert to JSON
            PacketParser::radioPacketToJSON(packet, json_buffer, sizeof(json_buffer));

            // SD Card logging (batch write)
            if (sd_ready) {
                batch_buffer[batch_count++] = packet;

                if (batch_count >= SD_LOG_BATCH_SIZE) {
                    if (SDLogger::logPacketBatch(batch_buffer, batch_count)) {
                        g_packets_logged += batch_count;
                        #if DEBUG_LEVEL >= 2
                        printf("[Core 1] SD batch written: %u packets\n", batch_count);
                        #endif
                    } else {
                        printf("[Core 1] ERROR: SD batch write failed\n");
                    }
                    batch_count = 0;
                }
            }

            // MQTT publish
            if (mqtt_ready) {
                MqttClient::publish(json_buffer);
                g_packets_published++;
                #if DEBUG_LEVEL >= 2
                printf("[Core 1] MQTT published packet #%u\n", g_packets_published);
                #endif
            }
        }

        // --- Stats every 10 seconds ---
        if (now - last_stats_time > 10000) {
            printf("\n========== SYSTEM STATUS ==========\n");
            printf("Uptime:         %u seconds\n", now / 1000);
            printf("Packets RX:     %u (%.1f Hz)\n", g_packets_received, g_packets_received * 1000.0f / now);
            printf("Packets logged: %u\n", g_packets_logged);
            printf("Packets MQTT:   %u\n", g_packets_published);
            printf("Inter-Pico TX:  %u\n", g_interpico_sent);
            printf("Sync errors:    %u\n", g_sync_errors);
            printf("Queue drops:    %u\n", g_queue_drops);

            if (sd_ready) {
                uint32_t sd_packets, sd_bytes, sd_errors;
                SDLogger::getStats(sd_packets, sd_bytes, sd_errors);
                printf("SD file:        %s\n", SDLogger::getCurrentFilename());
                printf("SD bytes:       %u\n", sd_bytes);
            }

            printf("MQTT status:    %s\n", mqtt_ready ? "CONNECTED" : "DISCONNECTED");
            printf("====================================\n\n");

            last_stats_time = now;
        }

        // Core 1 can sleep briefly
        sleep_ms(1);
    }
}

// =============================================================================
// Main (Core 0): Real-time I/O
// =============================================================================
int main() {
    // --- Standard initialization ---
    stdio_init_all();
    sleep_ms(3000);  // Wait for serial monitor

    // --- Banner ---
    printf("\n");
    printf("========================================================\n");
    printf("    RATS - Rotational Antenna Tracking System\n");
    printf("                  RadioPico v1.0\n");
    printf("========================================================\n");
    printf("  Core 0: RFD900x RX (GP1), Inter-Pico TX (GP4)\n");
    printf("  Core 1: SD Card (SPI1), MQTT (WiFi)\n");
    printf("========================================================\n\n");

    // --- Hardware Configuration Summary ---
    printf("Hardware Configuration:\n");
    printf("  UART0 (RFD900x): GP1 RX, %u baud\n", RFD900X_BAUD_RATE);
    printf("  UART1 (Inter-Pico): GP4 TX, 115200 baud\n");
    printf("  SPI1 (SD Card): GP10-13\n");
    printf("  Sync Word: 0x%08X\n", SYNC_WORD);
    printf("  Packet Size: 107 bytes\n\n");

    // --- Initialize inter-core queue ---
    printf("[Core 0] Initializing packet queue (64 slots)...\n");
    queue_init(&packet_queue, sizeof(RadioPacket), 64);

    // --- Initialize Inter-Pico UART ---
    printf("[Core 0] Initializing Inter-Pico UART (GP4 TX)...\n");
    InterPicoUART::init();

    // --- Launch Core 1 ---
    printf("[Core 0] Launching Core 1...\n");
    multicore_launch_core1(core1_entry);

    // --- Initialize RFD900x UART ---
    printf("[Core 0] Initializing RFD900x UART (GP1 RX)...\n");
    RFD900xUART::init();

    printf("[Core 0] Initialization complete - waiting for packets\n\n");

    // --- Core 0 Main Loop: Fast I/O Only ---
    uint8_t radio_buffer[107];
    RadioPacket parsed_packet;
    uint32_t last_debug_time = 0;

    while (true) {
        uint32_t now = to_ms_since_boot(get_absolute_time());

        // --- Check for incoming packets ---
        if (RFD900xUART::packetAvailable()) {
            if (RFD900xUART::readPacket(radio_buffer, sizeof(radio_buffer))) {
                // Validate sync word
                uint32_t sync;
                memcpy(&sync, radio_buffer, sizeof(uint32_t));

                if (sync == SYNC_WORD) {
                    // Parse packet
                    if (PacketParser::parseRadioPacket(radio_buffer, sizeof(radio_buffer), parsed_packet)) {
                        g_packets_received++;

                        // Extract key data for debug
                        uint8_t flight_mode = (parsed_packet.metadata >> 13) & 0x07;
                        float lat_deg = parsed_packet.latitude / 1000000.0f;
                        float lon_deg = parsed_packet.longitude / 1000000.0f;

                        #if DEBUG_LEVEL >= 1
                        printf("[RX] #%u | Mode:%u | Alt:%.1fm | GPS:%.4f,%.4f | Sats:%u\n",
                               g_packets_received, flight_mode, parsed_packet.altitude,
                               lat_deg, lon_deg, parsed_packet.num_satellites);
                        #endif

                        // --- Send to Stepper Pico via Inter-Pico UART ---
                        InterPicoUART::sendTrackingData(
                            parsed_packet.latitude,
                            parsed_packet.longitude,
                            parsed_packet.altitude
                        );
                        g_interpico_sent++;

                        // --- Queue for Core 1 (SD + MQTT) ---
                        if (!queue_try_add(&packet_queue, &parsed_packet)) {
                            g_queue_drops++;
                            printf("[Core 0] WARNING: Queue full - packet dropped!\n");
                        }
                    }
                } else {
                    g_sync_errors++;
                    #if DEBUG_LEVEL >= 1
                    printf("[Core 0] Sync error: got 0x%08X, expected 0x%08X\n", sync, SYNC_WORD);
                    #endif
                }
            }
        }

        // --- Debug heartbeat every 5 seconds ---
        #if DEBUG_LEVEL >= 1
        if (now - last_debug_time > 5000) {
            uint32_t total, errors, bytes;
            RFD900xUART::getStats(total, errors, bytes);
            printf("[Core 0] Heartbeat: %u packets, %u bytes, buffer: %u\n",
                   total, bytes, RFD900xUART::available());
            last_debug_time = now;
        }
        #endif

        // Keep loop tight
        tight_loop_contents();
    }

    return 0;
}
