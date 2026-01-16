#include "StepperMotor.h"
#include "GeoMath.h"
#include "LLA.h"
#include "AzEl.h"

#include "packet_simulator.h"
#include "packet_types.h"
#include "KalmanCV.h"

#include <Wire.h>
#include <SparkFun_u-blox_GNSS_Arduino_Library.h>

static constexpr bool USE_GPS = false; // set false when using packet sim
static constexpr bool USE_PACKET_SIMULATOR = true; // set false when using real data

// Motors
StepperMotor azMotor(4, 2);
StepperMotor elMotor(27, 28);

// LLA for RATS and rocket
LLA ratsLLA = {0.0, 0.0, 0.0};
LLA rocketLLA;

// Radio packets
bool newPacketAvailable = false;

PacketSimulator packetSim;
RadioPacket rxPacket;

// Kalman filtered rocket
KalmanCV kalmanRocket;

// GPS module
SFE_UBLOX_GNSS gps;

unsigned long lastPacketTimeMs = 0;
static constexpr unsigned long PACKET_PERIOD_MS = 100; // 10 Hz

// Restrict elevation
static constexpr double EL_MIN = 0.0;
static constexpr double EL_MAX = 90.0;

void setup() {
    // Sets up both motors with same speed and acceleration
    delay(1000);

    azMotor.setMaxSpeed(8000);
    azMotor.setAcceleration(4000);

    elMotor.setMaxSpeed(8000);
    elMotor.setAcceleration(4000);

    // Sets current position as 0 degrees
    azMotor.home();
    elMotor.home();
    azMotor.run();
    elMotor.run();

    azMotor.reset();
    elMotor.reset();
    
    Vec3 initialPos = {0.0, 0.0, 0.0};
    if (USE_GPS) {
        
    }
    kalmanRocket.init(0.0, initialPos, 25.0, 25.0);
    gps.setNavigationFrequency(1);  // 1 Hz update
}

void handleRadioPacket(const RadioPacket& packet) {
    rocketLLA.lat = packet.latitude  / 1e6;  // µdeg to deg
    rocketLLA.lon = packet.longitude / 1e6;
    rocketLLA.alt = packet.altitude;

    newPacketAvailable = true;
}

void pollPacketSimulator() {
    packetSim.generateRadioPacket(rxPacket);
    handleRadioPacket(rxPacket);
}

// Placeholder for real radio later
void pollRealRadio() {
    /*
        Example (future):
        if (radio.available()) {
            RadioPacket packet;
            radio.read(packet);
            handleRadioPacket(packet); idk
        }
    */
}

void loop() {
    // Move to 90 degrees
    azMotor.moveAngleTo(90.0);
    elMotor.moveAngleTo(30.0);

    // Wait until motors finish
    while (azMotor.isRunning() || elMotor.isRunning()) {
        azMotor.run();
        elMotor.run();
    }
    delay(1000);

    // Move back to 0 degrees
    azMotor.moveAngleTo(0.0);
    elMotor.moveAngleTo(0.0);

    while (azMotor.isRunning() || elMotor.isRunning()) {
        azMotor.run();
        elMotor.run();
    }
    delay(1000);
}





