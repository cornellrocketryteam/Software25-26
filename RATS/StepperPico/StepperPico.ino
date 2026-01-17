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
    unsigned long now = millis();

    // Use GPS coords if using module, else default to (0,0,0)
    if (USE_GPS) {
        if (gps.checkUblox() && gps.getPVT()) {
            ratsLLA.lat = gps.getLatitude() / 1e7;
            ratsLLA.lon = gps.getLongitude() / 1e7;
            ratsLLA.alt = gps.getAltitude() / 1000.0;
        }
    }

    // either use packet sim data or real data from radio
    if (now - lastPacketTimeMs >= PACKET_PERIOD_MS) {
        lastPacketTimeMs = now;

        if (USE_PACKET_SIMULATOR) {
            pollPacketSimulator();
            newPacketAvailable = true;
        } else {
            pollRealRadio();
        }
    }

    // Update Kalman when packet arrives
    static AzEl targetAzEl; // stores last target
    if (newPacketAvailable) {
        newPacketAvailable = false;

        Vec3 enuPos = GeoMath::llatoENU(ratsLLA, rocketLLA);
        kalmanRocket.updatePosition(now * 0.001, enuPos, 25.0);

        // Predict slightly ahead to account for motion
        State6 futureState = kalmanRocket.predictFuture(0.05);
        Vec3 filteredPos = {futureState.d[0], futureState.d[1], futureState.d[2]};

        targetAzEl = GeoMath::enuToAzEl(filteredPos);

        // restrict elevation
        if (targetAzEl.elevation < EL_MIN) targetAzEl.elevation = EL_MIN;
        if (targetAzEl.elevation > EL_MAX) targetAzEl.elevation = EL_MAX;
    }

    azMotor.moveAngleTo(targetAzEl.azimuth);
    elMotor.moveAngleTo(targetAzEl.elevation);

    // Run motors
    azMotor.run();
    elMotor.run();
}





