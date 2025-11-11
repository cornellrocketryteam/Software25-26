#include "mqtt_client.h"
#include "pico/cyw43_arch.h"
#include "lwip/dns.h"
#include <stdio.h>
#include <string.h>

// Global state for our client
static MQTT_CLIENT_T* mqtt_state;

// Function prototypes for local callbacks
static void mqtt_connection_cb(mqtt_client_t *client, void *arg, mqtt_connection_status_t status);
static void mqtt_connect_to_broker(MQTT_CLIENT_T *state);

// --- MQTT Connection Callback ---
// Called when the connection status with the broker changes
static void mqtt_connection_cb(mqtt_client_t *client, void *arg, mqtt_connection_status_t status) {
    MQTT_CLIENT_T *state = (MQTT_CLIENT_T *)arg;
    if (status == MQTT_CONNECT_ACCEPTED) {
        printf("[MQTT] Connected to broker\n");
        state->connected = true;
    } else {
        printf("[MQTT] Connection failed! Status: %d\n", status);
        state->connected = false;
        // Try to reconnect after a delay
        // For simplicity retry on next poll
    }
}

// --- MQTT Broker Connection ---
static void mqtt_connect_to_broker(MQTT_CLIENT_T *state) {
    if (!state || state->connected) {
        return;
    }

    printf("[MQTT] Connecting to broker at %s...\n", ipaddr_ntoa(&state->remote_ip));
    
    struct mqtt_connect_client_info_t ci;
    memset(&ci, 0, sizeof(ci));
    ci.client_id = "rats_pico_w"; // You can make this unique if needed
    ci.keep_alive = 60; // 60-second keep-alive

    err_t err = mqtt_client_connect(state->mqtt_client, &state->remote_ip, MQTT_BROKER_PORT,
                                    mqtt_connection_cb, state, &ci);
    
    if (err != ERR_OK) {
        printf("[MQTT] mqtt_client_connect error: %d\n", err);
    }
}

// --- DNS Found Callback ---
// Called when the broker's IP address is resolved
static void mqtt_dns_found_cb(const char *name, const ip_addr_t *ipaddr, void *arg) {
    if (ipaddr) {
        MQTT_CLIENT_T *state = (MQTT_CLIENT_T *)arg;
        state->remote_ip = *ipaddr;
        printf("[MQTT] DNS resolved. Broker IP: %s\n", ipaddr_ntoa(ipaddr));
        mqtt_connect_to_broker(state);
    } else {
        printf("[MQTT] DNS resolution failed for: %s\n", name);
    }
}

// --- Public Functions ---

void mqtt_client_init() {
    printf("[MQTT] Initializing MQTT Client...\n");

    // Allocate state
    mqtt_state = (MQTT_CLIENT_T *)calloc(1, sizeof(MQTT_CLIENT_T));
    if (!mqtt_state) {
        printf("[MQTT] Failed to allocate state\n");
        return;
    }

    // Initialize Wi-Fi
    if (cyw43_arch_init()) {
        printf("[MQTT] Wi-Fi init failed\n");
        return;
    }
    cyw43_arch_enable_sta_mode();
    printf("[MQTT] Connecting to Wi-Fi: %s\n", WIFI_SSID);

    // Connect to Wi-Fi
    if (cyw43_arch_wifi_connect_timeout_ms(WIFI_SSID, WIFI_PASS, CYW43_AUTH_WPA2_AES_PSK, 30000)) {
        printf("[MQTT] Failed to connect to Wi-Fi\n");
        return;
    }
    printf("[MQTT] Wi-Fi Connected!\n");

    // Initialize MQTT client
    mqtt_state->mqtt_client = mqtt_client_new();
    if (!mqtt_state->mqtt_client) {
        printf("[MQTT] mqtt_client_new() failed\n");
        return;
    }

    // Resolve broker address
    printf("[MQTT] Resolving broker address: %s\n", MQTT_BROKER_ADDRESS);
    err_t err = dns_gethostbyname(MQTT_BROKER_ADDRESS, &mqtt_state->remote_ip, mqtt_dns_found_cb, mqtt_state);

    if (err == ERR_OK) {
        // IP was already cached
        mqtt_connect_to_broker(mqtt_state);
    } else if (err != ERR_INPROGRESS) {
        printf("[MQTT] DNS request failed immediately: %d\n", err);
    }
}

void mqtt_client_publish(const char *json_payload) {
    if (!mqtt_state || !mqtt_state->mqtt_client || !mqtt_state->connected) {
        return; // Not ready to publish
    }

    err_t err = mqtt_publish(mqtt_state->mqtt_client, MQTT_TOPIC, json_payload,
                             strlen(json_payload), 0, 0, NULL, NULL);

    if (err != ERR_OK) {
        printf("[MQTT] mqtt_publish error: %d\n", err);
    }
}

void mqtt_client_poll() {
    // Required for non-blocking Wi-Fi
    cyw43_arch_poll();

    // Reconnect if needed
    if (mqtt_state && !mqtt_state->connected && mqtt_state->remote_ip.addr != 0) {
         // Simple retry logic
         static absolute_time_t last_retry = 0;
         if (absolute_time_min(last_retry) > 5000) {
             printf("[MQTT] Retrying connection...\n");
             mqtt_connect_to_broker(mqtt_state);
             last_retry = get_absolute_time();
         }
    }
}