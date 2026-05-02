SELECT
    -- Top-Level Packet Info
    timestamp as broker_arrival_ms,
    payload.sync_word as sync_word,

    -- Shared Telemetry (Rust 'Packet' Struct)
    payload.flight_mode as flight_mode,
    payload.pressure as pressure,
    payload.temp as temp,
    payload.altitude as altitude,
    payload.latitude as latitude,
    payload.longitude as longitude,
    payload.num_satellites as num_satellites,
    payload.timestamp as gps_time, -- ms since midnight UTC
    payload.mag_x as mag_x,
    payload.mag_y as mag_y,
    payload.mag_z as mag_z,
    payload.accel_x as accel_x,
    payload.accel_y as accel_y,
    payload.accel_z as accel_z,
    payload.gyro_x as gyro_x,
    payload.gyro_y as gyro_y,
    payload.gyro_z as gyro_z,
    payload.pt3 as pt3,
    payload.pt4 as pt4,
    payload.rtd as rtd,
    payload.sv_2_open as sv_2_open,
    payload.mav_open as mav_open,
    payload.ms_since_boot_cfc as ms_since_boot_cfc,

    -- Event Flags
    payload.ssa_drogue_deployed as ssa_drogue_deployed,
    payload.ssa_main_deployed as ssa_main_deployed,
    payload.cmd_n1 as cmd_n1,
    payload.cmd_n2 as cmd_n2,
    payload.cmd_n3 as cmd_n3,
    payload.cmd_n4 as cmd_n4,
    payload.cmd_a1 as cmd_a1,
    payload.cmd_a2 as cmd_a2,
    payload.cmd_a3 as cmd_a3,

    -- Airbrake & Control States
    payload.airbrake_deployment as airbrake_deployment,
    payload.predicted_apogee as predicted_apogee,

    -- Advanced GPS / U-Blox Metrics
    payload.h_acc as h_acc,
    payload.v_acc as v_acc,
    payload.vel_n as vel_n,
    payload.vel_e as vel_e,
    payload.vel_d as vel_d,
    payload.g_speed as g_speed,
    payload.s_acc as s_acc,
    payload.head_acc as head_acc,
    payload.fix_type as fix_type,
    payload.head_mot as head_mot,

    -- BLiMS Outputs
    payload.blims_brakeline_diff as blims_brakeline_diff,
    payload.blims_phase_id as blims_phase_id,
    payload.blims_pid_p as blims_pid_p,
    payload.blims_pid_i as blims_pid_i,
    payload.blims_bearing as blims_bearing,

    -- BLiMS Config
    payload.blims_upwind_lat as blims_upwind_lat,
    payload.blims_upwind_lon as blims_upwind_lon,
    payload.blims_downwind_lat as blims_downwind_lat,
    payload.blims_downwind_lon as blims_downwind_lon,
    payload.blims_wind_from_deg as blims_wind_from_deg,

    -- Fill Station Specific (Umbilical)
    payload.pt_1_pressure as pt_1_pressure,
    payload.pt_2_pressure as pt_2_pressure,
    payload.ball_valve_open as ball_valve_open,
    payload.sv_1_open as sv_1_open,
    payload.load_cell as load_cell,
    payload.ignition as ignition,
    payload.qd_state as qd_state,
    payload.ms_since_boot_fill as ms_since_boot_fill,

    -- Unit ID routing (Dynamically grabbed from topic "rats/raw/0")
    int(nth(3, split(topic, '/'))) as unit_id
  
FROM
    "rats/raw/+"
