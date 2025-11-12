/*
 * RATS MQTT Test Application
 * * This is a simple, single-core application to test the entire data pipeline:
 * 1. Simulates a packet
 * 2. Parses it to JSON
 * 3. Connects to Wi-Fi and MQTT
 * 4. Publishes the JSON to the broker
 * * This allows testing the network stack and data format in isolation
 * from the dual-core and real-radio hardware.
 */

#include <stdio.h>
#include <string.h>
#include "pico/stdlib.h"
#include "pico/cyw43_arch.h"

// Includes from our project
#include "config.h"
#include "packet_types.h"
#include "packet_parser.h"
#include "packet_simulator.h"
#include "mqtt_client.h"

// JSON buffer
char json_buffer[2048];

// Packet simulator
PacketSimulator simulator;

int main() {
    stdio_init_all();
    
    // Wait a few seconds for USB serial to connect
    sleep_ms(5000);
    printf("\n=== RATS MQTT Test ===\n");
    printf("Connecting to Wi-Fi and MQTT broker...\n");

    // Initialize Wi-Fi and MQTT client
    // This will block until Wi-Fi is connected.
    mqtt_client_init();

    printf("Initialization complete. Starting publish loop...\n");

    absolute_time_t last_publish_time = get_absolute_time();

    // Main loop: poll network and publish data
    while (true) {
        // --- CRITICAL ---
        // This must be called continuously in the loop
        // to handle the non-blocking network stack.
        mqtt_client_poll();

        // Check if 1 second (1,000,000 us) has passed
        absolute_time_t now = get_absolute_time();
        int64_t diff_us = absolute_time_diff_us(last_publish_time, now);

        if (diff_us > 1000 * 1000) { // 1 second
            last_publish_time = now;

            // 1. Generate a new simulated packet
            RadioPacket packet;
            simulator.generateRadioPacket(packet);

            // 2. Parse it to our JSON format
            PacketParser::radioPacketToJSON(packet, json_buffer, sizeof(json_buffer));

            // 3. Publish to MQTT
            printf("Publishing packet:\n%s\n\n", json_buffer);
            mqtt_client_publish(json_buffer);
        }

        // Give the network stack time to breathe
        // This is not strictly necessary but is good practice
        sleep_ms(10);
    }

    return 0;
}