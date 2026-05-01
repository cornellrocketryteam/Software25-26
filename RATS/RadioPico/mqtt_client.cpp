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
    state.last_dns_retry = nil_time;
    state.last_wifi_retry = nil_time;
    snprintf(state.client_id_str, sizeof(state.client_id_str), "rats_pico_w_%d", RATS_UNIT_ID);
    printf("[MQTT] Client ID set to: %s\n", state.client_id_str);

    // Initialize Wi-Fi Hardware
    if (cyw43_arch_init()) {
        printf("[MQTT] Wi-Fi hardware init failed\n");
        return false;
    }
    cyw43_arch_enable_sta_mode();
    printf("[MQTT] Starting async Wi-Fi connection to: %s\n", WIFI_SSID);

    // Connect to Wi-Fi asynchronously (does not block)
    cyw43_arch_wifi_connect_async(WIFI_SSID, WIFI_PASS, CYW43_AUTH_WPA2_AES_PSK);

    // Initialize MQTT client
    state.mqtt_client = mqtt_client_new();
    if (!state.mqtt_client) {
        printf("[MQTT] mqtt_client_new() failed\n");
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

    absolute_time_t now = get_absolute_time();
    uint32_t current_time_ms = to_ms_since_boot(now);
    
    // Onboard LED Debug Sequence
    static uint32_t last_led_toggle = 0;
    static bool led_state = false;
    
    if (state.connected) {
        // Solid ON when fully connected and authenticated to MQTT
        if (!led_state) {
            cyw43_arch_gpio_put(CYW43_WL_GPIO_LED_PIN, 1);
            led_state = true;
        }
    } else {
        // Slow blink (500ms) when disconnected or retrying connection
        if (current_time_ms - last_led_toggle > 500) {
            led_state = !led_state;
            cyw43_arch_gpio_put(CYW43_WL_GPIO_LED_PIN, led_state);
            last_led_toggle = current_time_ms;
        }
    }

    // Check Wi-Fi link status
    int link_status = cyw43_tcpip_link_status(&cyw43_state, CYW43_ITF_STA);
    
    if (link_status == CYW43_LINK_UP) {
        // Wi-Fi is connected
        if (state.remote_ip.addr == 0) {
            // Need to resolve DNS
            if (absolute_time_diff_us(state.last_dns_retry, now) > 5000000) { // Every 5s
                printf("[MQTT] Resolving broker address: %s\n", MQTT_BROKER_ADDRESS);
                err_t err = dns_gethostbyname(MQTT_BROKER_ADDRESS, &state.remote_ip, mqtt_dns_found_cb, &state);
                if (err == ERR_OK) {
                    connect_to_broker();
                }
                state.last_dns_retry = now;
            }
        } else if (!state.connected) {
             // Connect/Reconnect MQTT
             if (absolute_time_diff_us(state.last_retry, now) > 5000000) { // Every 5s
                 printf("[MQTT] Retrying MQTT connection...\n");
                 connect_to_broker();
                 state.last_retry = now;
             }
        }
    } else if (link_status < 0) {
        // Wi-Fi connection failed or dropped, retry
        if (absolute_time_diff_us(state.last_wifi_retry, now) > 10000000) { // Every 10s
             printf("[MQTT] Retrying Wi-Fi...\n");
             cyw43_arch_wifi_connect_async(WIFI_SSID, WIFI_PASS, CYW43_AUTH_WPA2_AES_PSK);
             state.last_wifi_retry = now;
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