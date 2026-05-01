#include "Arduino.h"
#include <stdio.h>

#include "AzEl.h"
#include "GeoMath.h"
#include "LLA.h"
#include "StepperMotor.h"

#include "KalmanCV.h"
#include "NMEAParser.h"
#include "packet_types.h"
#include "../Common/serial_protocol.h"
#include "hardware/uart.h"

#include "pico/multicore.h"
#include "pico/util/queue.h"

#if USE_PACKET_SIMULATOR
#include "packet_simulator.h"
#endif

// Set USE_GPS = false to disable Native GPS NMEA Parser for testing without module
static constexpr bool USE_GPS = false;

// Set to 1 to use internal packet simulator, 0 for real radio data
#define USE_PACKET_SIMULATOR 1

// Set to 1 to run a deterministic math and motor interrupt test sequence
#define MATH_TEST_MODE 1

// Maximum apogee to predict (10,000 feet in meters)
static constexpr double MAX_APOGEE_METERS = 3048.0;

// Motors (DIR, STEP, EN, StepsPerRev, Microsteps)
// 1.8 degree motor = 200 steps/rev. 8 microsteps = 1600 pulses/rev.
StepperMotor azMotor(7, 6, 8, 200, 8);
StepperMotor elMotor(10, 9, 11, 200, 8);

// LLA for RATS and rocket
LLA ratsLLA = {0.0, 0.0, 0.0};
LLA rocketLLA;

bool newPacketAvailable = false;
uint32_t currentFlightMode = STARTUP;
volatile uint32_t totalPacketsReceived = 0;

#if USE_PACKET_SIMULATOR
PacketSimulator packetSim;
RadioPacket rxPacket;
#endif

KalmanCV kalmanRocket;
NMEAParser gps;

unsigned long lastPacketTimeMs = 0;
unsigned long lastSimPollTimeMs = 0;
static constexpr unsigned long PACKET_PERIOD_MS = 50; // 20 Hz

// Thread-safe targets for Core 0
struct TargetAngles {
    double azimuth;
    double elevation;
    bool motors_enabled;
};

queue_t angle_queue;

enum class TrackerState {
    GPS_SEARCH,
    GPS_AVERAGING,
    STANDBY,
    PAD_IDLE,
    ACTIVE_TRACKING,
    SIGNAL_LOST
};

TrackerState currentState = TrackerState::GPS_SEARCH;

const char* getStateName(TrackerState state) {
    switch(state) {
        case TrackerState::GPS_SEARCH: return "SEARCH";
        case TrackerState::GPS_AVERAGING: return "AVG";
        case TrackerState::STANDBY: return "STANDBY";
        case TrackerState::PAD_IDLE: return "PAD_IDLE";
        case TrackerState::ACTIVE_TRACKING: return "TRACKING";
        case TrackerState::SIGNAL_LOST: return "SIG_LOST";
        default: return "UNKNOWN";
    }
}

void setup() {
  stdio_init_all();
  delay(6000); // Wait for serial monitor to connect

  azMotor.begin();
  elMotor.begin();

#if MATH_TEST_MODE
  azMotor.setMaxSpeed(400);
  azMotor.setAcceleration(200);
  elMotor.setMaxSpeed(400);
  elMotor.setAcceleration(200);
#else
  azMotor.setMaxSpeed(8000);
  azMotor.setAcceleration(4000);
  elMotor.setMaxSpeed(8000);
  elMotor.setAcceleration(4000);
#endif

  azMotor.reset();
  elMotor.reset();

  Vec3 initialPos = {0.0, 0.0, 0.0};
  kalmanRocket.init(0.0, initialPos, 25.0, 25.0);

  if (USE_GPS) {
    gps.init(uart0, 0, 1, 9600);
  }

  uart_init(uart1, 115200);
  gpio_set_function(5, GPIO_FUNC_UART);
  uart_set_format(uart1, 8, 1, UART_PARITY_NONE);
  uart_set_fifo_enabled(uart1, true);
  
  queue_init(&angle_queue, sizeof(TargetAngles), 10);
}

void handleRadioPacket(const RadioPacket &packet) {
  double newLat = packet.latitude / 1e6;
  double newLon = packet.longitude / 1e6;
  
  // If GPS cuts out, retain the last valid latitude and longitude
  // and only update altitude from the barometric sensor.
#if MATH_TEST_MODE
  rocketLLA.lat = newLat;
  rocketLLA.lon = newLon;
#else
  if (newLat != 0.0 && newLon != 0.0) {
      rocketLLA.lat = newLat;
      rocketLLA.lon = newLon;
  }
#endif
  rocketLLA.alt = packet.altitude;
  
  currentFlightMode = packet.flight_mode;
  newPacketAvailable = true;
  totalPacketsReceived++;
  lastPacketTimeMs = millis();
}

#if USE_PACKET_SIMULATOR
void pollPacketSimulator() {
  packetSim.generateRadioPacket(rxPacket);
  handleRadioPacket(rxPacket);
}
#endif

void pollRealRadio() {
  static uint8_t rx_buffer[20];
  static int rx_index = 0;
  
  while (uart_is_readable(uart1)) {
      uint8_t c = uart_getc(uart1);
      if (rx_index < 20) {
          rx_buffer[rx_index++] = c;
      }
      if (rx_index == 20) {
          uint32_t sync;
          memcpy(&sync, rx_buffer, 4);
          if (sync == TRACKING_SYNC_WORD) {
              TrackingData data;
              memcpy(&data, rx_buffer + 4, sizeof(TrackingData));
              
              double newLat = lat_udeg_to_degrees(data.latitude_udeg);
              double newLon = lon_udeg_to_degrees(data.longitude_udeg);
              
              // If GPS cuts out, retain the last valid latitude and longitude
              // and only update altitude from the barometric sensor.
#if MATH_TEST_MODE
              rocketLLA.lat = newLat;
              rocketLLA.lon = newLon;
#else
              if (newLat != 0.0 && newLon != 0.0) {
                  rocketLLA.lat = newLat;
                  rocketLLA.lon = newLon;
              }
#endif
              rocketLLA.alt = data.altitude;
              
              currentFlightMode = data.flight_mode;
              newPacketAvailable = true;
              totalPacketsReceived++;
              lastPacketTimeMs = millis();
              rx_index = 0;
          } else {
              memmove(rx_buffer, rx_buffer + 1, 19);
              rx_index = 19;
          }
      }
  }
}

void core1_entry() {
    printf("[Core 1] Started FSM and Math loop\n");

    gpio_init(28);
    gpio_set_dir(28, GPIO_OUT);
    
    int validGpsPoints = 0;
    double sumLat = 0, sumLon = 0, sumAlt = 0;
    
    unsigned long lastLedToggle = 0;
    bool ledState = false;

    // Calibration offsets
    double calibration_az_offset = 0.0;
    double calibration_el_offset = 0.0;
    bool has_calibrated = false;

#if MATH_TEST_MODE
    ratsLLA.lat = 0.0;
    ratsLLA.lon = 0.0;
    ratsLLA.alt = 0.0;
    currentState = TrackerState::ACTIVE_TRACKING;
#endif

    // Start with default targets
    TargetAngles targets = {0.0, 0.0, false};
    TrackerState lastPrintedState = currentState;

    while (true) {
        unsigned long now = millis();
        
        // Check for state transitions to print immediately
        if (currentState != lastPrintedState) {
            printf("\n>>> FSM TRANSITION: %s -> %s <<<\n\n", getStateName(lastPrintedState), getStateName(currentState));
            lastPrintedState = currentState;
        }
        
        // 1. POLLING
        if (USE_GPS) {
            if (gps.process()) {
                if (gps.getFixQuality() > 0 && currentState == TrackerState::GPS_SEARCH) {
                    currentState = TrackerState::GPS_AVERAGING;
                }
                
                if (currentState == TrackerState::GPS_AVERAGING) {
                    sumLat += gps.getLatitude();
                    sumLon += gps.getLongitude();
                    sumAlt += gps.getAltitude() / 1000.0;
                    validGpsPoints++;
                    
                    if (validGpsPoints >= 120) {
                        ratsLLA.lat = sumLat / 120.0;
                        ratsLLA.lon = sumLon / 120.0;
                        ratsLLA.alt = sumAlt / 120.0;
                        currentState = TrackerState::STANDBY;
                    }
                }
            }
        } else {
#if !MATH_TEST_MODE
            if (currentState == TrackerState::GPS_SEARCH || currentState == TrackerState::GPS_AVERAGING) {
                ratsLLA.lat = 42.336789;
                ratsLLA.lon = -76.497123;
                ratsLLA.alt = 100.0;
                currentState = TrackerState::STANDBY;
            }
#endif
        }

#if !USE_PACKET_SIMULATOR
        pollRealRadio();
#endif

        if (now - lastSimPollTimeMs >= PACKET_PERIOD_MS) {
            lastSimPollTimeMs = now;
#if USE_PACKET_SIMULATOR
            if (now >= 5000) {
                pollPacketSimulator();
            }
#endif
        }

        // Check timeout
        bool signalLost = false;
        if (lastPacketTimeMs > 0) {
            signalLost = ((now - lastPacketTimeMs) > 5000);
        }

        // FSM TRANSITIONS & LED LOGIC
        switch (currentState) {
            case TrackerState::GPS_SEARCH:
                if (now - lastLedToggle > 100) {
                    ledState = !ledState;
                    gpio_put(28, ledState);
                    lastLedToggle = now;
                }
                targets.motors_enabled = false;
                break;
                
            case TrackerState::GPS_AVERAGING:
                if (now - lastLedToggle > 500) {
                    ledState = !ledState;
                    gpio_put(28, ledState);
                    lastLedToggle = now;
                }
                targets.motors_enabled = false;
                break;
                
            case TrackerState::STANDBY:
                gpio_put(28, 1);
                targets.motors_enabled = false;
                if (!signalLost && newPacketAvailable) {
                    if (currentFlightMode == STARTUP || currentFlightMode == STANDBY) {
                        currentState = TrackerState::PAD_IDLE;
                    } else {
                        currentState = TrackerState::ACTIVE_TRACKING;
                    }
                }
                break;

            case TrackerState::PAD_IDLE:
                if (now - lastLedToggle > 1000) {
                    ledState = !ledState;
                    gpio_put(28, ledState);
                    lastLedToggle = now;
                }
                targets.motors_enabled = false;
                if (signalLost) {
                    currentState = TrackerState::STANDBY;
                } else if (currentFlightMode == ASCENT) {
                    currentState = TrackerState::ACTIVE_TRACKING;
                }
                break;
                
            case TrackerState::ACTIVE_TRACKING:
                if (newPacketAvailable) {
                    ledState = !ledState;
                    gpio_put(28, ledState);
                }
                targets.motors_enabled = true;
                
                if (signalLost) {
                    currentState = TrackerState::SIGNAL_LOST;
                }
                break;
                
            case TrackerState::SIGNAL_LOST:
                gpio_put(28, 0); // Solid OFF
                targets.motors_enabled = false;
                if (!signalLost && newPacketAvailable) {
                    currentState = TrackerState::ACTIVE_TRACKING;
                }
                break;
        }

        // MATH & CALIBRATION
#if MATH_TEST_MODE
        bool hasRatsFix = true;
        bool hasRocketFix = true;
#else
        bool hasRatsFix = (ratsLLA.lat != 0.0 && ratsLLA.lon != 0.0);
        bool hasRocketFix = (rocketLLA.lat != 0.0 && rocketLLA.lon != 0.0);
#endif

        if (hasRatsFix && hasRocketFix && (currentState == TrackerState::PAD_IDLE || currentState == TrackerState::ACTIVE_TRACKING || currentState == TrackerState::SIGNAL_LOST)) {
            if (newPacketAvailable) {
                newPacketAvailable = false;
                Vec3 enuPos = GeoMath::llatoENU(ratsLLA, rocketLLA);
                double timeInSeconds = now / 1000.0;
                kalmanRocket.predict(timeInSeconds);
                kalmanRocket.updatePosition(enuPos, 25.0);
            }

            double currentTotalTimeInSeconds = now / 1000.0;
            double timeSinceLastUpdate = currentTotalTimeInSeconds - (lastPacketTimeMs / 1000.0);
            State6 futureState = kalmanRocket.predictFuture(timeSinceLastUpdate + 0.05);

            // Hard clamp altitude to 10k feet apogee
            if ((ratsLLA.alt + futureState.d[2]) > MAX_APOGEE_METERS) {
                futureState.d[2] = MAX_APOGEE_METERS - ratsLLA.alt;
            }

            Vec3 filteredPos = {futureState.d[0], futureState.d[1], futureState.d[2]};
            AzEl azel = GeoMath::enuToAzEl(filteredPos);
            
#if !MATH_TEST_MODE
            // If we are idling on the pad, continuously update the calibration offset to cancel GPS drift.
            if (currentState == TrackerState::PAD_IDLE) {
                calibration_az_offset = azel.azimuth;
                calibration_el_offset = azel.elevation;
                has_calibrated = true;
            } 
            // Fallback: If we jumped straight to active tracking (e.g. reboot mid-flight), calibrate to the first point.
            else if (currentState == TrackerState::ACTIVE_TRACKING && !has_calibrated) {
                calibration_az_offset = azel.azimuth;
                calibration_el_offset = azel.elevation;
                has_calibrated = true;
            }
#endif

            // Apply relative offsets so the physical 0 degree matches the launch pad
            targets.azimuth = azel.azimuth - calibration_az_offset;
            targets.elevation = azel.elevation - calibration_el_offset;

        } else {
            // Drop newPacketAvailable if we aren't tracking
            newPacketAvailable = false;
        }

        // Send to queue
        if (queue_is_full(&angle_queue)) {
            TargetAngles dummy;
            queue_try_remove(&angle_queue, &dummy);
        }
        queue_try_add(&angle_queue, &targets);

        // DEBUG PRINT
        static unsigned long lastPrintTimeMs = 0;
        static uint32_t lastTotalPackets = 0;
        
        if (now - lastPrintTimeMs >= 500) {
            lastPrintTimeMs = now;
            
            uint32_t packetsThisInterval = totalPacketsReceived - lastTotalPackets;
            lastTotalPackets = totalPacketsReceived;
            
            printf("[FSM: %s] Az: tgt=%.2f | El: tgt=%.2f | Enabled=%d | Rx: %u pkts\n", 
                   getStateName(currentState), targets.azimuth, targets.elevation, targets.motors_enabled, packetsThisInterval);
        }
        
        sleep_ms(1);
    }
}

int main() {
    setup();
    printf("\n=== StepperPico ===\n");
    printf("Core 0: High Priority Motor Control\n");
    printf("Core 1: FSM, Math, & Sensors\n\n");

#if USE_PACKET_SIMULATOR
    printf("*** SIMULATOR MODE ACTIVE ***\n");
    printf("Waiting 5 seconds before starting simulated packets...\n\n");
#else
    printf("*** REAL HARDWARE MODE ***\n");
    printf("Waiting for RadioPico Telemetry...\n\n");
#endif
    
    multicore_launch_core1(core1_entry);
    
    bool currently_enabled = true; // default AccelStepper state
    
    // Core 0 loop: HIGH PRIORITY MOTOR CONTROL
    while (true) {
        TargetAngles targets;
        while (queue_try_remove(&angle_queue, &targets)) {
            azMotor.moveAngleTo(targets.azimuth);
            elMotor.moveAngleTo(constrain(targets.elevation, -90.0, 90.0));
            
            if (targets.motors_enabled != currently_enabled) {
                if (targets.motors_enabled) {
                    azMotor.enable();
                    elMotor.enable();
                } else {
                    azMotor.disable();
                    elMotor.disable();
                }
                currently_enabled = targets.motors_enabled;
            }
        }
        
        if (currently_enabled) {
            azMotor.update();
            elMotor.update();
        }
    }
    
    return 0;
}
