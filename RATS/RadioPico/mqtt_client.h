#ifndef MQTT_CLIENT_H
#define MQTT_CLIENT_H

#include "pico/stdlib.h"
#include "lwip/apps/mqtt.h"

// Define the MQTT topic in config.h
#include "config.h"

// Struct to hold all MQTT client state
struct MQTT_CLIENT_T {
    ip_addr_t remote_ip;
    mqtt_client_t *mqtt_client;
    bool connected;
};

// Initializes the Wi-Fi and MQTT client
// Call this once from main() on Core 1
void mqtt_client_init();

// Publishes a JSON payload
// Call this from Core 1 after a packet is received
void mqtt_client_publish(const char *json_payload);

// This must be called regularly in the Core 1 loop
// to handle Wi-Fi polling and keep-alives.
void mqtt_client_poll();

#endif // MQTT_CLIENT_H