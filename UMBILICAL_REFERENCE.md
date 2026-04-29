# Umbilical Reference

This document serves as the single source of truth for the Umbilical communication system. The umbilical provides bidirectional communication over USB CDC-ACM (Virtual Serial Port) between the flight computer and the ground station (fill-station).

## 1. Connection & Heartbeat
The umbilical system requires an active heartbeat to be considered "connected" by the flight software.

- **Connection Status:** Monitored via `<H>` (Heartbeat) commands. If no `<H>` is received within `HEARTBEAT_TIMEOUT_MS` (typically 15 seconds), the connection is considered lost.
- **Background Tasks:** The umbilical runs continuously in three asynchronous tasks (`usb_task`, `umbilical_sender_task`, `umbilical_receiver_task`) independent of the main flight loop.

## 2. Umbilical Commands

The flight computer accepts the following string-based commands from the ground station. Most commands are 3-4 bytes in length, wrapped in angle brackets.

### Core Vehicle Commands
* `<H>` : Heartbeat (bumps heartbeat timestamp to maintain connection)
* `<L>` : Launch (triggers the launch sequence if armed and in Standby)
* `<V>` : Safe vehicle (closes MAV and SV)
* `<KA>` / `<KD>` : Key Arm / Key Disarm (gates the Startup → Standby transition)
* `<M>` / `<m>` : Open MAV / Close MAV
* `<S>` / `<s>` : Open SV (Solenoid Valve) / Close SV
* `<D>` / `<d>` : Trigger Drogue / Trigger Main (manual override for recovery)

### Payload Commands
* `<1>` .. `<4>` : Payload Events N1 through N4
* `<A1>` .. `<A3>` : Payload Events A1 through A3

### Diagnostics & Storage Commands
* `<R>` : Reboot flight computer
* `<F>` / `<f>` : Reset FRAM / Dump FRAM
* `<G>` / `<W>` / `<I>` : Dump Flash / Wipe Flash / Flash Info
* `<X>` : Wipe FRAM and Reboot

### BLiMS (Steerable Parachute) Commands
* `<T,lat,lon>` : Set BLiMS landing-zone target (e.g. `<T,42.4419130,-76.4878000>`)

## 3. Telemetry Output

When not interrupted by a flash or FRAM dump, the flight software continuously emits a `$TELEM,...` string via the umbilical. 

The format is a 57-field comma-separated value (CSV) string ending with a newline `\n`. It consists of:

1. `flight_mode` (u32)
2. `pressure` (Pa, f32)
3. `temp` (°C, f32)
4. `altitude` (m, f32)
5. `latitude` (f32)
6. `longitude` (f32)
7. `num_satellites` (u32)
8. `timestamp` (f32)
9-11. `mag_x`, `mag_y`, `mag_z` (µT, f32)
12-14. `accel_x`, `accel_y`, `accel_z` (m/s², f32)
15-17. `gyro_x`, `gyro_y`, `gyro_z` (°/s, f32)
18-20. `pt3`, `pt4`, `rtd` (ADC values, f32)
21-22. `sv_open`, `mav_open` (1 = open, 0 = closed)
23-24. `ssa_drogue_deployed`, `ssa_main_deployed` (u8 flags)
25-28. `cmd_n1` through `cmd_n4` (u8 flags)
29-31. `cmd_a1` through `cmd_a3` (u8 flags)
32. `airbrake_state` (u8: 0=idle, 1=deployed, 2=retracted)
33. `predicted_apogee` (m, f32)
34-35. `h_acc`, `v_acc` (GPS accuracies, mm, u32)
36-38. `vel_n`, `vel_e`, `vel_d` (Velocities, m/s, f64)
39-41. `g_speed` (m/s, f64), `s_acc` (mm/s, u32), `head_acc` (deg*1e5, u32)
42-43. `fix_type` (u8), `head_mot` (deg*1e5, i32)
44-55. `blims_motor_position` (f32), `blims_phase_id` (i8), `blims_pid_p` (f32), `blims_pid_i` (f32), `blims_bearing` (f32), `blims_loiter_step` (i8), `blims_heading_des` (f32), `blims_heading_error` (f32), `blims_error_integral` (f32), `blims_dist_to_target_m` (f32), `blims_target_lat` (f32), `blims_target_lon` (f32)
56. `blims_wind_from_deg` (f32)
57. `ms_since_boot_cfc` (u32)

Note: Internal packet size is 199 bytes binary, but via umbilical it is sent exclusively as a `$TELEM` CSV string.
