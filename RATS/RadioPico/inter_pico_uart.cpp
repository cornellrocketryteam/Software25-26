#include "inter_pico_uart.h"
#include <cstring>
#include <cstdio>

// UART configuration for inter-Pico communication
#define INTER_PICO_UART uart1
#define INTER_PICO_TX_PIN 4    // RadioPico GP4 -> StepperPico GP5
#define INTER_PICO_RX_PIN 5    // RadioPico GP5 <- StepperPico GP4 (not used for TX-only)
#define INTER_PICO_BAUD 115200

// Sync word for packet start detection
// Using unique pattern unlikely to appear in GPS/altitude data
#define TRACKING_SYNC_WORD 0x54524B21  // "TRK!" in ASCII

// Statistics
uint32_t InterPicoUART::packets_sent = 0;
uint32_t InterPicoUART::bytes_sent = 0;

void InterPicoUART::init() {
    // Initialize UART1
    uart_init(INTER_PICO_UART, INTER_PICO_BAUD);

    // Set TX pin (GP4)
    gpio_set_function(INTER_PICO_TX_PIN, GPIO_FUNC_UART);

    // Optional: Set RX pin if we want bidirectional later
    // gpio_set_function(INTER_PICO_RX_PIN, GPIO_FUNC_UART);

    // Set UART format: 8 data bits, 1 stop bit, no parity (8N1)
    uart_set_format(INTER_PICO_UART, 8, 1, UART_PARITY_NONE);

    // Disable FIFO (we're sending small packets)
    uart_set_fifo_enabled(INTER_PICO_UART, false);

    printf("[Inter-Pico UART] Initialized on UART1, TX=GP%d, Baud=%d\n",
           INTER_PICO_TX_PIN, INTER_PICO_BAUD);
}

bool InterPicoUART::sendTrackingData(int32_t latitude_udeg, int32_t longitude_udeg, float altitude) {
    // Build packet with sync word prefix
    uint8_t packet_buffer[16];  // 4 bytes sync + 12 bytes data
    uint32_t offset = 0;

    // Add sync word (4 bytes)
    uint32_t sync_word = TRACKING_SYNC_WORD;
    memcpy(packet_buffer + offset, &sync_word, sizeof(uint32_t));
    offset += sizeof(uint32_t);

    // Add tracking data (12 bytes)
    TrackingData data;
    data.latitude_udeg = latitude_udeg;
    data.longitude_udeg = longitude_udeg;
    data.altitude = altitude;

    memcpy(packet_buffer + offset, &data, sizeof(TrackingData));
    offset += sizeof(TrackingData);

    // Send entire packet (16 bytes total)
    uart_write_blocking(INTER_PICO_UART, packet_buffer, offset);

    // Update statistics
    packets_sent++;
    bytes_sent += offset;

    // Debug: Print every 10th packet (only for testing - disable in production)
    #ifdef DEBUG_PRINT_PACKETS
    static uint32_t debug_count = 0;
    if (++debug_count % 10 == 0) {
        printf("[Inter-Pico] Sent %u packets (Lat: %.6f, Lon: %.6f, Alt: %.2fm)\n",
               packets_sent, latitude_udeg / 1000000.0f, longitude_udeg / 1000000.0f, altitude);
    }
    #endif

    return true;
}

void InterPicoUART::getStats(uint32_t& packets, uint32_t& bytes) {
    packets = packets_sent;
    bytes = bytes_sent;
}
