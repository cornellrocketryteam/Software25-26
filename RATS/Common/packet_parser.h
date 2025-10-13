#ifndef PACKET_PARSER_H
#define PACKET_PARSER_H

#include "packet_types.h"
#include <stddef.h>

class PacketParser {
public:
    // Parse raw bytes into RadioPacket structure
    static bool parseRadioPacket(const uint8_t* buffer, size_t length, RadioPacket& packet);
    
    // Parse raw bytes into UmbilicalPacket structure
    static bool parseUmbilicalPacket(const uint8_t* buffer, size_t length, UmbilicalPacket& packet);
    
    // Convert packet to JSON string
    static void radioPacketToJSON(const RadioPacket& packet, char* json_buffer, size_t buffer_size);
    static void umbilicalPacketToJSON(const UmbilicalPacket& packet, char* json_buffer, size_t buffer_size);

private:
    // Helper functions to parse bitfields
    static Metadata parseMetadata(uint16_t raw_metadata);
    static Events parseEvents(uint32_t raw_events);
    
    // Helper to read values from buffer
    template<typename T>
    static T readValue(const uint8_t* buffer, size_t& offset);
};

#endif // PACKET_PARSER_H