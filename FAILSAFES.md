# Failsafe & Safety Mechanism Reference

> Living document — update whenever thresholds, timeouts, or safety logic changes.
> Last updated: 2026-04-27

---

## FSW (Flight Software)

### Hardware

| Failsafe | Threshold | Trigger | Action | File |
|---|---|---|---|---|
| Hardware watchdog | 120 ms | `execute()` hangs without feeding watchdog | Hardware chip reset | `watchdog.rs`, `constants.rs:74` |

### Sensors

| Failsafe | Threshold | Trigger | Action | File |
|---|---|---|---|---|
| Sensor init timeout | 500 ms per sensor | BMP390 / GPS / IMU / ADC fails to init | Mark sensor unavailable, boot continues | `state.rs:196–245`, `constants.rs:92` |
| Sensor read timeout | 30 ms per cycle | Per-cycle sensor read hangs | Mark reading INVALID, flight loop continues | `state.rs:357–435`, `constants.rs:85` |
| Pressure bounds check | 80,000 – 120,000 Pa | Altimeter reading outside range | Reject reading entirely, skip altitude calc | `bmp390.rs:60`, `constants.rs:104–105` |
| Altimeter validity guard (Startup) | Any | Altimeter = INVALID at Startup | Force transition to Fault immediately | `flight_loop.rs:539–545` |
| Altimeter validity guard (Standby) | Any | Altimeter becomes INVALID while armed | Force transition to Fault immediately | `flight_loop.rs:568–575` |

### Pressure / Overpressure

| Failsafe | Threshold | Trigger | Action | File |
|---|---|---|---|---|
| Overpressure latch | 1000 PSI, **3 consecutive samples** | PT3 tank pressure exceeds threshold | Open SV, force Fault mode, log to FRAM — one-shot | `flight_loop.rs:441–473`, `constants.rs:140` |

### Umbilical / Ground Connection

| Failsafe | Threshold | Trigger | Action | File |
|---|---|---|---|---|
| Heartbeat timeout | 3000 ms | No `<H>` received from ground | Mark umbilical as disconnected | `umbilical.rs:33–40`, `constants.rs:124` |
| Umbilical disconnect safety | 15 s after disconnect | Umbilical reads disconnected | Open SV — one-shot, resets on reconnect | `flight_loop.rs:521–534`, `constants.rs:119` |

### State Machine Guards

| Failsafe | Threshold | Trigger | Action | File |
|---|---|---|---|---|
| Key arm gate | — | Startup → Standby attempted | Blocked unless: key_armed=true AND umbilical connected AND altimeter VALID | `flight_loop.rs:547–559` |
| Recovery vent | — | Entry to DrogueDeployed / MainDeployed / Fault | Opens SV once — one-shot flag prevents repeats | `flight_loop.rs:481–491` |
| Invalid flight mode recovery | mode > Fault | Boot with corrupted FRAM | Defaults to Fault | `state.rs:179–184` |

### Actuators

| Failsafe | Threshold | Trigger | Action | File |
|---|---|---|---|---|
| MAV auto-close | 12 s | After MAV opens | Force-closes MAV regardless of command state | `actuator.rs:220–239`, `constants.rs:125` |
| SSA auto-off | Fixed pulse duration | After ematch fires | Forces pin LOW — prevents continuous current through ematch | `actuator.rs:14–65` |
| Launch sequence gate | State machine | Launch command | Enforces: SV open 2 s → close → 1 s gap → MAV open 12 s → close | `flight_loop.rs`, `constants.rs:125–127` |

### Flash Operations

| Failsafe | Threshold | Trigger | Action | File |
|---|---|---|---|---|
| Flash operation timeout | 200 ms | QSPI read/write/erase hangs | Log timeout, watchdog fed during long ops | `state.rs:516–620`, `constants.rs:87` |
| Flash wipe timeout | 300,000 ms (5 min) | Full-flash erase hangs | Log timeout | `state.rs:664`, `constants.rs:90` |

### Payload

| Failsafe | Threshold | Trigger | Action | File |
|---|---|---|---|---|
| **N1 flight-mode gate** | Startup or Standby only | N1 command received in any other mode | Command silently dropped — N1 (camera deploy) blocked mid-flight | `flight_loop.rs:254–263`, `flight_loop.rs:367–375` |
| **N3 auto-send (low altitude)** | altitude < 76.2 m (250 ft) **for 1 continuous second** | DrogueDeployed or MainDeployed phase, altitude holds below threshold | Sends `N3\n` to payload UART — one-shot (`n3_sent` flag) | `flight_loop.rs:769–779`, `flight_loop.rs:811–822` |
| **N4 auto-send (hard landing)** | Any accel axis > 50 m/s² | MainDeployed phase | Sends `N4\n` to payload UART — one-shot (`n4_sent` flag) | `flight_loop.rs:825–834` |
| **Main chute auto-deploy** | altitude < 610 m AND elapsed > 1000 ms since drogue | DrogueDeployed phase | Fires main chute, transitions to MainDeployed, writes to FRAM | `flight_loop.rs:783–798`, `constants.rs:109–111` |
| **Payload heartbeat** | 1 Hz | Every second during flight loop | Sends `A\n` to payload UART to keep payload board alive | `flight_loop.rs:216–222` |
| **Payload UART loopback timeout** | 5 s | Ground test: no data received back from payload | Logs warning — confirms payload link integrity | `main.rs:767–777` |
| **N2 arm altitude gate** | 500 m AGL | N2 velocity-derived signal | N2 only armed above this altitude | `constants.rs:117` |

### Airbrake (Core 1)

| Failsafe | Threshold | Trigger | Action | File |
|---|---|---|---|---|
| Airbrake phase guard | — | Entry to DrogueDeployed / MainDeployed / Fault | Signals Core 1 to stop and retract airbrakes | `flight_loop.rs:179–196` |

---

## Fill-Station (Embedded Server)

### Client / WebSocket

| Failsafe | Threshold | Trigger | Action | File |
|---|---|---|---|---|
| Client disconnect emergency shutdown | 15 s | Zero WebSocket clients connected | Closes SV1, closes ball valve, sends `<S>` (Open SV) to FSW | `main.rs:826–850` |
| Client heartbeat watchdog | 15 s | No valid message from a connected client | Disconnect that client | `main.rs:230–234` |
| `umb_ever_connected` guard | Boot | FSW never connected yet | Umbilical safety timer will not arm until first FSW connection | `main.rs:828–832` |

### Umbilical / FSW Link

| Failsafe | Threshold | Trigger | Action | File |
|---|---|---|---|---|
| Telemetry freshness | 3000 ms | No `$TELEM` lines received | Mark umbilical as disconnected | `main.rs:90`, `main.rs:858` |
| Umbilical disconnect safety | 15 s after freshness expires | FSW telemetry stale (catches hung FSW with USB still up) | Closes ball valve, opens SV1 | `main.rs:852–904` |
| Serial read timeout | 200 ms | UART read blocks | Timeout, triggers reconnect loop | `main.rs:85`, `main.rs:1101` |
| Line buffer overflow | 8 KB | Hung FSW sends partial line | Clears buffer, restarts line parsing | `main.rs:1127`, `main.rs:1183–1187` |

### ADC / Sensors

| Failsafe | Threshold | Trigger | Action | File |
|---|---|---|---|---|
| ADC read retry | 5 attempts, 10 ms apart | I2C read failure | After all retries fail: mark readings invalid | `main.rs:48–53` |
| ADC validity marking | — | All retries exhausted | CSV and MQTT write `N/A` instead of garbage values | `main.rs:957–975` |

### Hardware / Actuators

| Failsafe | Threshold | Trigger | Action | File |
|---|---|---|---|---|
| Solenoid valve default-safe init | Boot | Power-on | All SVs start CLOSED (NC=LOW de-energized, NO=HIGH energized shut) | `solenoid_valve.rs:31–37` |
| Ball valve signal interlock | — | Signal line change attempted while ON_OFF is HIGH | Returns error, blocks state change | `ball_valve.rs:87–92` |
| Ball valve settling delay | 100 ms | Before any open/close signal change | Ensures actuator is in stable state before switching | `ball_valve.rs:61`, `ball_valve.rs:77` |
| Ignition forced OFF | 3 s | After ignition fires | Hard timeout forces both igniters OFF | `main.rs:367` |

### Input Validation

| Failsafe | Threshold | Trigger | Action | File |
|---|---|---|---|---|
| GPS coordinate validation | lat ∈ [-90,90], lon ∈ [-180,180] | `fsw_set_blims_target` command | Rejects out-of-range coordinates | `main.rs:758–763` |
| Igniter ID validation | Only 1 or 2 accepted | `get_igniter_continuity` command | Returns error on invalid ID | `main.rs:393–408` |
| JSON parse guard | — | Malformed WebSocket message | Returns `CommandResponse::Error`, does not crash | `main.rs:323–335` |

---

## Dashboard (dashboard_v2.py)

> The dashboard has no independent safety enforcement. All safety is server/FSW-side.
> The dashboard's role is to maintain connection and display state.

| Mechanism | Threshold | Trigger | Action | File |
|---|---|---|---|---|
| WebSocket auto-reconnect | 2 s backoff | Disconnect / exception | Automatically retries connection | `dashboard_v2.py:143–152` |
| Heartbeat sender | Every 5 s | Connection alive | Sends `heartbeat` command to prevent fill-station 15s client timeout | `dashboard_v2.py:69–76` |

---

## Cross-System Timeout Summary

| System | Mechanism | Timeout |
|---|---|---|
| FSW | Hardware watchdog | 120 ms |
| FSW | Sensor init | 500 ms |
| FSW | Sensor read | 30 ms |
| FSW | Heartbeat freshness | 3000 ms |
| FSW | Umbilical safety vent | 15 s |
| FSW | MAV auto-close | 12 s |
| FSW | Flash timeout | 200 ms |
| FSW | Flash wipe timeout | 300 s |
| Fill-Station | Telemetry freshness | 3000 ms |
| Fill-Station | Umbilical safety | 15 s |
| Fill-Station | Client disconnect safety | 15 s |
| Fill-Station | Client heartbeat | 15 s |
| Fill-Station | Serial read | 200 ms |
| Fill-Station | Ignition forced OFF | 3 s |
| Dashboard | Heartbeat send interval | 5 s |
| Dashboard | Reconnect backoff | 2 s |
