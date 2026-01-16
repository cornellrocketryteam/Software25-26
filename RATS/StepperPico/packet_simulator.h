#ifndef PACKET_SIMULATOR_H
#define PACKET_SIMULATOR_H

#include "packet_types.h"
#include <stdint.h>
#include <stddef.h>



// TODO:FIX ENTIRE TEST SYSTEM

// Simulates generating telemetry packets for testing
class PacketSimulator {
public:
    PacketSimulator();
    
    // Generate a simulated radio packet with realistic values
    void generateRadioPacket(RadioPacket& packet);
    
    // Serialize packet to raw bytes (as would come from radio)
    static void serializeRadioPacket(const RadioPacket& packet, uint8_t* buffer);
    
private:
    uint32_t sim_time_ms;
    FlightMode current_mode;  // Current flight mode state
    float sim_altitude;
    float sim_velocity;

    void updateSimulation();
};

#endif // PACKET_SIMULATOR_H