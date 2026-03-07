#include "Arduino.h"
#include <stdio.h>

#include "AzEl.h"
#include "GeoMath.h"
#include "LLA.h"
#include "StepperMotor.h"

#include "KalmanCV.h"
#include "NMEAParser.h"
#include "packet_simulator.h"
#include "packet_types.h"

// Set USE_GPS = false to disable Native GPS NMEA Parser for testing without
// module
static constexpr bool USE_GPS = false;
static constexpr bool USE_PACKET_SIMULATOR =
    true; // set false when using real radio data

// Motors (DIR, STEP, EN)
// Note: Motor UART TX = GP4, RX = GP5 (Unused statically by AccelStepper, but
// available for TMC UART)
StepperMotor azMotor(7, 6, 8);
StepperMotor elMotor(10, 9, 11);

// LLA for RATS and rocket
LLA ratsLLA = {0.0, 0.0, 0.0};
LLA rocketLLA;

// Radio packets
bool newPacketAvailable = false;

PacketSimulator packetSim;
RadioPacket rxPacket;

// Kalman filtered rocket
KalmanCV kalmanRocket;

// Native NMEA GPS Parser
NMEAParser gps;

unsigned long lastPacketTimeMs = 0;
static constexpr unsigned long PACKET_PERIOD_MS = 100; // 10 Hz

void setup() {
  stdio_init_all();
  // Sets up both motors with same speed and acceleration

  delay(1000);

  azMotor.begin();
  elMotor.begin();

  azMotor.setMaxSpeed(8000);
  azMotor.setAcceleration(4000);

  elMotor.setMaxSpeed(8000);
  elMotor.setAcceleration(4000);

  // Sets current position as 0 degrees
  azMotor.reset();
  elMotor.reset();

  Vec3 initialPos = {0.0, 0.0, 0.0};
  kalmanRocket.init(0.0, initialPos, 25.0, 25.0);

  // Initialize Native GPS over UART0 (TX: GP0, RX: GP1)
  if (USE_GPS) {
    // NOTE: We connect the u-blox GPS module to the Pico UART matching these
    // pins: Pico GP0 (UART0 TX) -> Hook to GPS RX Pico GP1 (UART0 RX) -> Hook
    // to GPS TX
    gps.init(uart0, 0, 1, 9600);
  }
}

void handleRadioPacket(const RadioPacket &packet) {
  rocketLLA.lat = packet.latitude / 1e6; // µdeg to deg
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

  // Use GPS coords if using module, else default to fake mockup near Ithaca NY
  // for test sim Ping the Native UART NMEA Parser
  if (USE_GPS) {
    if (gps.process()) {
      ratsLLA.lat = gps.getLatitude();
      ratsLLA.lon = gps.getLongitude();
      ratsLLA.alt =
          gps.getAltitude() /
          1000.0; // Assume altitude already comes in meters, but check
    }
  } else {
    // FAKE DATA FOR TESTING WITHOUT GPS MODULE
    // Positioned roughly ~2km South of the packet_simulator rocket start point
    ratsLLA.lat = 42.336789;
    ratsLLA.lon = -76.497123;
    ratsLLA.alt = 100.0;
  }

  // either use packet sim data or real data from radio
  if (now - lastPacketTimeMs >= PACKET_PERIOD_MS) {
    lastPacketTimeMs = now;

    if (USE_PACKET_SIMULATOR) {
      // Wait 5 seconds before starting the simulator to allow for
      // initialization and serial monitor connection
      if (now >= 5000) {
        pollPacketSimulator();
      }
    } else {
      pollRealRadio();
    }
  }

  // Check if we have valid coordinates
  bool hasRatsFix = (ratsLLA.lat != 0.0 && ratsLLA.lon != 0.0);
  bool hasRocketFix = (rocketLLA.lat != 0.0 && rocketLLA.lon != 0.0);

  if (hasRatsFix && hasRocketFix) {
    // Compute ENU from LLA with current packet, also update Kalman filtered
    // rocket
    if (newPacketAvailable) {
      newPacketAvailable = false;

      Vec3 enuPos = GeoMath::llatoENU(ratsLLA, rocketLLA);

      // 1. Predict the Kalman state to the current time using seconds
      double timeInSeconds = now / 1000.0;
      kalmanRocket.predict(timeInSeconds);

      // 2. Update the Kalman filter with the new GPS measurement
      kalmanRocket.updatePosition(enuPos, 25.0);
    }

    // 3. Smooth continuous tracking: Predict where the rocket is *right now*
    // using the elapsed time since the last packet update.
    // As the loop runs between 10Hz packets, this prediction smoothly moves
    // forward.
    double currentTotalTimeInSeconds = millis() / 1000.0;
    double timeSinceLastUpdate =
        currentTotalTimeInSeconds - (lastPacketTimeMs / 1000.0);

    // Add a slight latency offset if we want to trace slightly ahead of the
    // model (e.g. 0.05 seconds)
    State6 futureState = kalmanRocket.predictFuture(timeSinceLastUpdate + 0.05);
    Vec3 filteredPos = {futureState.d[0], futureState.d[1], futureState.d[2]};

    // Compute Az/El from filtered ENU
    AzEl azel = GeoMath::enuToAzEl(filteredPos);

    // Move motors
    azMotor.moveAngleTo(azel.azimuth);
    elMotor.moveAngleTo(constrain(azel.elevation, 0.0, 90.0));
  }

  // Update motors constantly. If no fix yet, they remain at 0 position.
  azMotor.update();
  elMotor.update();

  static unsigned long lastPrintTimeMs = 0;
  if (now - lastPrintTimeMs >= 500) {
    lastPrintTimeMs = now;

    // Map mode enum to string for printing
    const char *modeStr = "UNKNOWN";
    if (USE_PACKET_SIMULATOR) {
      switch (packetSim.getCurrentMode()) {
      case STANDBY:
        modeStr = "STANDBY";
        break;
      case ASCENT:
        modeStr = "ASCENT";
        break;
      case DROGUE_DEPLOYED:
        modeStr = "DROGUE";
        break;
      case MAIN_DEPLOYED:
        modeStr = "MAIN";
        break;
      }
    } else {
      modeStr = "RADIO";
    }

    printf("[%s] Fix=%d | Az: tgt=%.2f pos=%ld | El: tgt=%.2f pos=%ld\n",
           modeStr, (hasRatsFix && hasRocketFix), azMotor.currentAngle(),
           azMotor.currentPosition(), elMotor.currentAngle(),
           elMotor.currentPosition());
  }
}

int main() {
  setup();
  printf("Starting packet simulator to monitor motors...\n");
  while (true) {
    loop();
  }
  return 0;
}
