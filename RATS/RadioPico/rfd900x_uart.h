#ifndef RFD900X_UART_H
#define RFD900X_UART_H

#include "pico/stdlib.h"
#include "hardware/uart.h"
#include "config.h"         // Shared configuration
#include "packet_types.h"   // From Common folder

// UART Configuration
#define RFD_UART_ID uart1

// Pin definitions (using UART1)
#define RFD_TX_PIN 4   // Pico GP4 -> RFD900x RX (Pin 7)
#define RFD_RX_PIN 5   // Pico GP5 -> RFD900x TX (Pin 9)

class RFD900xUART {
public:
    // Initialize UART for RFD900x
    static void init();
    
    // Check if a complete packet is available
    static bool packetAvailable();
    
    // Read a complete packet into buffer
    // Returns true if packet read successfully
    static bool readPacket(uint8_t* buffer, size_t buffer_size);
    
    // Get number of bytes waiting in receive buffer
    static uint32_t available();
    
    // Get packet statistics
    static void getStats(uint32_t& total_packets, uint32_t& error_count, uint32_t& bytes_received);
    
    // Clear receive buffer
    static void flush();

private:
    static uint8_t rx_buffer[RFD_RX_BUFFER_SIZE];
    static uint32_t rx_head;
    static uint32_t rx_tail;
    static uint32_t total_packets_received;
    static uint32_t packet_errors;
    static uint32_t total_bytes_received;
    
    // IRQ handler for receiving data
    static void uartRxHandler();
    
    // Search for sync word in buffer
    static bool findSyncWord(uint32_t& position);
    
    // Calculate circular buffer available bytes
    static uint32_t bufferAvailable();
    
    // Read byte from circular buffer
    static uint8_t readBufferByte();
    
    // Peek byte from circular buffer without removing
    static uint8_t peekBufferByte(uint32_t offset);
};

#endif // RFD900X_UART_H