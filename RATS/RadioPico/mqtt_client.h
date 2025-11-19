#ifndef MQTT_CLIENT_H
#define MQTT_CLIENT_H

#include "pico/stdlib.h"
#include "lwip/apps/mqtt.h"
#include "config.h"

// Struct to hold all MQTT client state
struct MQTT_CLIENT_T {
    ip_addr_t remote_ip;
    mqtt_client_t *mqtt_client;
    bool connected;
    absolute_time_t last_retry;
    char client_id_str[32];
};

class MqttClient {
public:
    // Initializes the Wi-Fi and MQTT client
    // Returns true if successful, false if unsuccessful
    static bool init();

    // Publishes a JSON payload
    // Call this from Core 1 after a packet is received
    static void publish(const char *json_payload);

    // This must be called regularly in the Core 1 loop
    // to handle Wi-Fi polling and keep-alives.
    static void poll();

private:
    static MQTT_CLIENT_T state;

    // Called when connection status with broker changes
    static void mqtt_connection_cb(mqtt_client_t *client, void *arg, mqtt_connection_status_t status);

    // Called when broker IP is resolved by DNS
    static void mqtt_dns_found_cb(const char *name, const ip_addr_t *ipaddr, void *arg);

    // Attempt to connect to broker
    static void connect_to_broker();
};

#endif // MQTT_CLIENT_H