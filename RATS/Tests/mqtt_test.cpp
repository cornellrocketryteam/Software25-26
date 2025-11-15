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

    // Initialize Wi-Fi and MQTT
    if (!MqttClient::init()) {
        printf("FATAL: Failed to init MQTT client. Halting.\n");
        while(true) {
            cyw43_arch_gpio_put(CYW43_WL_GPIO_LED_PIN, 1);
            sleep_ms(LED_BLINK_ERROR);
            cyw43_arch_gpio_put(CYW43_WL_GPIO_LED_PIN, 0);
            sleep_ms(LED_BLINK_ERROR);
        }
    }

    printf("Initialization complete. Starting publish loop...\n");

    absolute_time_t last_publish_time = get_absolute_time();

    // Main loop: poll network and publish data
    while (true) {
        // Poll Network Stack
        MqttClient::poll();

        // Check if 1 second (1,000,000 us) has passed
        absolute_time_t now = get_absolute_time();
        int64_t diff_us = absolute_time_diff_us(last_publish_time, now);

        if (diff_us > 1000 * 1000) { // 1 second
            last_publish_time = now;

            // Generate a new simulated packet
            RadioPacket packet;
            simulator.generateRadioPacket(packet);

            // Parse to JSON format
            PacketParser::radioPacketToJSON(packet, json_buffer, sizeof(json_buffer));

            // Publish to MQTT
            printf("Publishing packet:\n%s\n\n", json_buffer);
            MqttClient::publish(json_buffer);
        }

        // Give the network stack time to breathe
        sleep_ms(10);
    }

    return 0;
}