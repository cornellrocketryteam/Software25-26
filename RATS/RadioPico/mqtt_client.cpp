#include "mqtt_client.h"
#include "pico/cyw43_arch.h"
#include "lwip/dns.h"
#include <stdio.h>
#include <string.h>

MQTT_CLIENT_T MqttClient::state;

// --- Public Methods ---
bool MqttClient::init() {
    printf("[MQTT] Initializing MQTT Client...\n");

    // Initialize state struct
    memset(&state, 0, sizeof(MQTT_CLIENT_T));
    state.last_retry = nil_time;
    snprintf(state.client_id_str, sizeof(state.client_id_str), "rats_pico_w_%d", RATS_UNIT_ID);
    printf("[MQTT] Client ID set to: %s\n", state.client_id_str);

    // Initialize Wi-Fi
    if (cyw43_arch_init()) {
        printf("[MQTT] Wi-Fi init failed\n");
        return false;
    }
    cyw43_arch_enable_sta_mode();
    printf("[MQTT] Connecting to Wi-Fi: %s\n", WIFI_SSID);

    // Connect to Wi-Fi (blocking with 30s timeout)
    if (cyw43_arch_wifi_connect_timeout_ms(WIFI_SSID, WIFI_PASS, CYW43_AUTH_WPA2_AES_PSK, 30000)) {
        printf("[MQTT] Failed to connect to Wi-Fi\n");
        return false;
    }
    printf("[MQTT] Wi-Fi Connected!\n");

    // Initialize MQTT client
    state.mqtt_client = mqtt_client_new();
    if (!state.mqtt_client) {
        printf("[MQTT] mqtt_client_new() failed\n");
        return false;
    }

    // Resolve broker address
    printf("[MQTT] Resolving broker address: %s\n", MQTT_BROKER_ADDRESS);
    
    // Pass 'this' MqttClient instance as the callback argument
    err_t err = dns_gethostbyname(MQTT_BROKER_ADDRESS, &state.remote_ip, mqtt_dns_found_cb, &state);

    if (err == ERR_OK) {
        // IP was already cached, connect immediately
        connect_to_broker();
    } else if (err != ERR_INPROGRESS) {
        printf("[MQTT] DNS request failed immediately: %d\n", err);
        return false;
    }
    
    return true;
}

void MqttClient::publish(const char *json_payload) {
    if (!state.mqtt_client || !state.connected) {
        return; // Not ready to publish
    }

    err_t err = mqtt_publish(state.mqtt_client, MQTT_TOPIC, json_payload,
                             strlen(json_payload), 0, 0, NULL, NULL);

    if (err != ERR_OK) {
        printf("[MQTT] mqtt_publish error: %d\n", err);
    }
}

void MqttClient::poll() {
    // Required for non-blocking Wi-Fi
    cyw43_arch_poll();

    // Reconnect if needed
    if (!state.connected && state.remote_ip.addr != 0) {
         absolute_time_t now = get_absolute_time();

         // Get the time difference since the last retry in microseconds
         int64_t diff_us = absolute_time_diff_us(state.last_retry, now);

         // Check if 5 seconds (5000000us) have passed
         if (diff_us > 5000000) {
             printf("[MQTT] Retrying connection...\n");
             connect_to_broker();
             state.last_retry = now; // Update the last retry time
         }
    }
}

// --- Internal Helper Methods ---
void MqttClient::connect_to_broker() {
    if (state.connected) {
        return;
    }

    printf("[MQTT] Connecting to broker at %s...\n", ipaddr_ntoa(&state.remote_ip));
    
    struct mqtt_connect_client_info_t ci;
    memset(&ci, 0, sizeof(ci));
    ci.client_id = state.client_id_str; // You can make this unique if needed
    ci.keep_alive = 60; // 60-second keep-alive

    // Pass 'this' MqttClient instance as the callback argument
    err_t err = mqtt_client_connect(state.mqtt_client, &state.remote_ip, MQTT_BROKER_PORT,
                                    mqtt_connection_cb, &state, &ci);
    
    if (err != ERR_OK) {
        printf("[MQTT] mqtt_client_connect error: %d\n", err);
    }
}

// --- Static Callbacks ---
void MqttClient::mqtt_connection_cb(mqtt_client_t *client, void *arg, mqtt_connection_status_t status) {
    MQTT_CLIENT_T *state_ptr = static_cast<MQTT_CLIENT_T*>(arg);

    if (status == MQTT_CONNECT_ACCEPTED) {
        printf("[MQTT] Connected to broker\n");
        state_ptr->connected = true;
    } else {
        printf("[MQTT] Connection failed! Status: %d\n", status);
        state_ptr->connected = false;
    }
}

void MqttClient::mqtt_dns_found_cb(const char *name, const ip_addr_t *ipaddr, void *arg) {
    MQTT_CLIENT_T *state_ptr = static_cast<MQTT_CLIENT_T*>(arg);

    if (ipaddr) {
        state_ptr->remote_ip = *ipaddr;
        printf("[MQTT] DNS resolved. Broker IP: %s\n", ipaddr_ntoa(ipaddr));
        connect_to_broker();
    } else {
        printf("[MQTT] DNS resolution failed for: %s\n", name);
    }
}