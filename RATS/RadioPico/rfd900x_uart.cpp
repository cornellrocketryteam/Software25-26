#include "rfd900x_uart.h"
#include "hardware/irq.h"
#include <stdio.h>
#include <string.h>

// Static member initialization
uint8_t RFD900xUART::rx_buffer[RFD_RX_BUFFER_SIZE];
uint32_t RFD900xUART::rx_head = 0;
uint32_t RFD900xUART::rx_tail = 0;
uint32_t RFD900xUART::total_packets_received = 0;
uint32_t RFD900xUART::packet_errors = 0;
uint32_t RFD900xUART::total_bytes_received = 0;

void RFD900xUART::init() {
    // Initialize UART with settings from config.h
    uart_init(RFD_UART_ID, RFD900X_BAUD_RATE);
    
    // Set UART format
    uart_set_format(RFD_UART_ID, RFD900X_DATA_BITS, RFD900X_STOP_BITS, 
                    (uart_parity_t)RFD900X_PARITY);
    
    // Set up pins
    gpio_set_function(RFD_TX_PIN, GPIO_FUNC_UART);
    gpio_set_function(RFD_RX_PIN, GPIO_FUNC_UART);
    
    // Enable UART FIFO
    uart_set_fifo_enabled(RFD_UART_ID, true);
    
    // Set up interrupt handler
    int UART_IRQ = (RFD_UART_ID == uart0) ? UART0_IRQ : UART1_IRQ;
    irq_set_exclusive_handler(UART_IRQ, uartRxHandler);
    irq_set_enabled(UART_IRQ, true);
    
    // Enable RX interrupt
    uart_set_irq_enables(RFD_UART_ID, true, false);
    
    printf("RFD900x UART initialized on UART1 (TX=GP%d, RX=GP%d) at %d baud\n", 
           RFD_TX_PIN, RFD_RX_PIN, RFD900X_BAUD_RATE);
}

void RFD900xUART::uartRxHandler() {
    // Read all available bytes from UART into circular buffer
    while (uart_is_readable(RFD_UART_ID)) {
        uint8_t byte = uart_getc(RFD_UART_ID);
        
        uint32_t next_head = (rx_head + 1) % RFD_RX_BUFFER_SIZE;
        
        // Check for buffer overflow
        if (next_head != rx_tail) {
            rx_buffer[rx_head] = byte;
            rx_head = next_head;
            total_bytes_received++;
        } else {
            // Buffer overflow - this is an error condition
            packet_errors++;
        }
    }
}

uint32_t RFD900xUART::bufferAvailable() {
    if (rx_head >= rx_tail) {
        return rx_head - rx_tail;
    } else {
        return RFD_RX_BUFFER_SIZE - rx_tail + rx_head;
    }
}

uint8_t RFD900xUART::readBufferByte() {
    uint8_t byte = rx_buffer[rx_tail];
    rx_tail = (rx_tail + 1) % RFD_RX_BUFFER_SIZE;
    return byte;
}

uint8_t RFD900xUART::peekBufferByte(uint32_t offset) {
    uint32_t pos = (rx_tail + offset) % RFD_RX_BUFFER_SIZE;
    return rx_buffer[pos];
}

bool RFD900xUART::findSyncWord(uint32_t& position) {
    uint32_t available = bufferAvailable();
    
    // Need at least 4 bytes to check for sync word
    if (available < 4) {
        return false;
    }
    
    // Search for sync word (from config.h: 0x3E5D5967 = "CRT!")
    for (uint32_t i = 0; i <= available - 4; i++) {
        uint32_t word = 0;
        word |= peekBufferByte(i);
        word |= (uint32_t)peekBufferByte(i + 1) << 8;
        word |= (uint32_t)peekBufferByte(i + 2) << 16;
        word |= (uint32_t)peekBufferByte(i + 3) << 24;
        
        if (word == SYNC_WORD) {
            position = i;
            return true;
        }
    }
    
    return false;
}

bool RFD900xUART::packetAvailable() {
    uint32_t sync_pos;
    
    // Look for sync word
    if (!findSyncWord(sync_pos)) {
        // No sync word found - if buffer is getting full, discard old data
        if (bufferAvailable() > RFD_RX_BUFFER_SIZE - RADIO_PACKET_SIZE) {
            // Discard half the buffer
            for (int i = 0; i < RFD_RX_BUFFER_SIZE / 2; i++) {
                readBufferByte();
            }
            packet_errors++;
        }
        return false;
    }
    
    // Discard bytes before sync word
    for (uint32_t i = 0; i < sync_pos; i++) {
        readBufferByte();
    }
    
    // Check if we have a full packet (107 bytes starting from sync word)
    return bufferAvailable() >= RADIO_PACKET_SIZE;
}

bool RFD900xUART::readPacket(uint8_t* buffer, size_t buffer_size) {
    if (buffer_size < RADIO_PACKET_SIZE) {
        return false;
    }
    
    if (!packetAvailable()) {
        return false;
    }
    
    // Read the packet
    for (int i = 0; i < RADIO_PACKET_SIZE; i++) {
        buffer[i] = readBufferByte();
    }
    
    total_packets_received++;
    return true;
}

uint32_t RFD900xUART::available() {
    return bufferAvailable();
}

void RFD900xUART::getStats(uint32_t& total_packets, uint32_t& error_count, uint32_t& bytes_received) {
    total_packets = total_packets_received;
    error_count = packet_errors;
    bytes_received = total_bytes_received;
}

void RFD900xUART::flush() {
    rx_head = 0;
    rx_tail = 0;
}