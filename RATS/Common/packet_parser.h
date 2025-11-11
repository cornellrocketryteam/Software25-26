#ifndef PACKET_PARSER_H
#define PACKET_PARSER_H

#include "packet_types.h"
#include <stddef.h>

class PacketParser {
public:
    // Parse raw bytes into RadioPacket structure
    static bool parseRadioPacket(const uint8_t* buffer, size_t length, RadioPacket& packet);

    // Convert packet to JSON string (NEW IMPLEMENTATION)
    // This now generates the flat JSON for TimescaleDB
    static void radioPacketToJSON(const RadioPacket& packet, char* json_buffer, size_t buffer_size);
    
private:
    // Helper to read values from buffer
    template<typename T>
    static T readValue(const uint8_t* buffer, size_t& offset);
};

#endif // PACKET_PARSER_H