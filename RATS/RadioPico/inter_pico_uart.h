#ifndef INTER_PICO_UART_H
#define INTER_PICO_UART_H

#include "pico/stdlib.h"
#include "hardware/uart.h"
#include "serial_protocol.h"

// Inter-Pico UART Communication
// RadioPico (TX) -> StepperPico (RX)
// GP4 (UART1 TX) -> GP5 on StepperPico

class InterPicoUART {
public:
    // Initialize UART1 for inter-Pico communication
    // RadioPico uses GP4 (TX) to send to StepperPico
    static void init();

    // Send tracking data packet with sync word
    // Returns true if sent successfully
    static bool sendTrackingData(int32_t latitude_udeg, int32_t longitude_udeg, float altitude);

    // Get statistics
    static void getStats(uint32_t& packets_sent, uint32_t& bytes_sent);

private:
    static uint32_t packets_sent;
    static uint32_t bytes_sent;
};

#endif // INTER_PICO_UART_H
