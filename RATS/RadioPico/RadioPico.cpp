#include <stdio.h>
#include "pico/stdlib.h"
#include "packet_types.h"
#include "packet_parser.h"

int main()
{
    stdio_init_all();
    
    // Wait for USB serial connection
    sleep_ms(2000);
    
    printf("\n=== RadioPico Ground Station ===\n");
    printf("Waiting for RFD900x packets...\n\n");
    
    // Buffers for packet handling
    uint8_t radio_buffer[107];  // Radio packet buffer
    RadioPacket parsed_packet;
    char json_buffer[1024];     // Buffer for JSON output
    
    int packet_count = 0;
    
    while (true) {
        // === TODO: Replace this section with RFD900x receive code ===
        // When radio is added, this is where data is received:
        // if (radio.receive(radio_buffer, sizeof(radio_buffer))) {
        //     ... process packet ...
        // }
        
        // For now, just blink to show it's running
        printf("Ready to receive packets (connect RFD900x)...\n");
        sleep_ms(1000);
        
        // === EXAMPLE ===
        /*
        if (PacketParser::parseRadioPacket(radio_buffer, sizeof(radio_buffer), parsed_packet)) {
            packet_count++;
            
            // Convert to JSON
            PacketParser::radioPacketToJSON(parsed_packet, json_buffer, sizeof(json_buffer));
            
            // Send over USB serial
            printf("Packet #%d:\n%s\n", packet_count, json_buffer);
            
            // Display key telemetry
            printf("  Flight Mode: %d | Altitude: %.1fm | GPS: %.6f,%.6f | Sats: %d\n\n",
                   parsed_packet.metadata.flight_mode,
                   parsed_packet.altitude,
                   parsed_packet.latitude_udeg / 1000000.0,
                   parsed_packet.longitude_udeg / 1000000.0,
                   parsed_packet.satellites);
        } else {
            printf("ERROR: Failed to parse packet!\n");
        }
        */
    }
    
    return 0;
}