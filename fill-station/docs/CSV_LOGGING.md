# CSV Logging Implementation

## Overview
The fill station automatically logs all system states to a CSV file on startup. This includes state information from actuators (SVs, MAV), the ADCs, and any connected Flight Software (FSW) received over the Umbilical. The logger runs continuously in the background at **100 Hz**.

## File Management
- **Location:** Logs are saved to `/tmp/data` on Linux (the TI board), or `logs/` on macOS/Windows.
- **Naming Convention:** `fill_station_log_<UNIX_TIMESTAMP>.csv`
- **File Continuation:** To prevent data loss from accidental overwrites, consecutive runs started within the same second will append a suffix (e.g., `_1`, `_2`) so that every new run gets its own brand-new CSV file.
- **Persistence:** The file flushes (syncs to disk) every 10 seconds (1,000 samples) to ensure data is preserved even in the event of an abrupt power cycle.

## Logged Data Columns

The CSV file contains the following columns, exactly matching the order below. If a component (like the ADCs or FSW over Umbilical) fails to return fresh data, their respective columns will be populated with `"N/A"`.

### Header Structure

**Timing and General Actuators:**
- `Loop`: Monotonically increasing loop counter starting at 1.
- `Timestamp_ms`: Unix timestamp of the data point.
- `MAV_Angle`: Mechanical Actuated Valve position in degrees.
- `MAV_Pulse_US`: MAV position in microseconds.
- `Igniter1_Active`, `Igniter2_Active`: Boolean indicating if the igniter is currently fired.

**Solenoid Valves:** (Columns repeat for SV1 through SV5)
- `<VALVE>_Actuated`: Boolean active state. Note: For SV5, this records the inverted logical state.
- `<VALVE>_Cont`: Boolean continuity state.

**Analog to Digital Converters (ADCs):** (Columns repeat for ADC1 and ADC2, Channels 0 through 3)
- `ADC<NUM>_<CH>_Raw`: 12-bit raw integer reading.
- `ADC<NUM>_<CH>_Scaled`: Extrapolated float data assuming standard sensor calibration constraints. If unscaled, will be `N/A`.

**Umbilical Telemetry (FSW):**
- `FSW_Connected`: True if the ground station has an active CDC-ACM serial umbilical connection to the flight software.
- `FSW_Mode`: Flight system state machine mode (Standby, Ascent, etc).
- `FSW_Pressure`, `FSW_Temp`, `FSW_Altitude`: Base barometric data.
- `FSW_Lat`, `FSW_Lon`, `FSW_Sats`: GPS positional data.
- `FSW_Timestamp`: Uptime on the FSW.
- `FSW_MagX`, `FSW_MagY`, `FSW_MagZ`: Internal magnetometer.
- `FSW_AccelX`, `FSW_AccelY`, `FSW_AccelZ`: Internal accelerometer.
- `FSW_GyroX`, `FSW_GyroY`, `FSW_GyroZ`: Internal gyroscope.
- `FSW_PT3`, `FSW_PT4`, `FSW_RTD`: Remote pressures and RTD temperatures on the vehicle.
