INSERT INTO telemetry_data (
    -- The Primary Live X-Axis Clock
    time,
    
    -- Unit ID
    unit_id,

    -- Top-Level Packet Info
    sync_word,

    -- Shared Telemetry (Rust 'Packet' Struct)
    flight_mode,
    pressure,
    temp,
    altitude,
    latitude,
    longitude,
    num_satellites,
    gps_time, 
    mag_x,
    mag_y,
    mag_z,
    accel_x,
    accel_y,
    accel_z,
    gyro_x,
    gyro_y,
    gyro_z,
    pt3,
    pt4,
    rtd,
    sv_2_open,
    mav_open,
    ms_since_boot_cfc,

    -- Event Flags
    ssa_drogue_deployed,
    ssa_main_deployed,
    cmd_n1,
    cmd_n2,
    cmd_n3,
    cmd_n4,
    cmd_a1,
    cmd_a2,
    cmd_a3,

    -- Airbrake & Control States
    airbrake_deployment,
    predicted_apogee,

    -- Advanced GPS / U-Blox Metrics
    h_acc,
    v_acc,
    vel_n,
    vel_e,
    vel_d,
    g_speed,
    s_acc,
    head_acc,
    fix_type,
    head_mot,

    -- BLiMS Outputs
    blims_brakeline_diff,
    blims_phase_id,
    blims_pid_p,
    blims_pid_i,
    blims_bearing,

    -- BLiMS Config
    blims_upwind_lat,
    blims_upwind_lon,
    blims_downwind_lat,
    blims_downwind_lon,
    blims_wind_from_deg,

    -- Fill Station Specific (Umbilical)
    pt_1_pressure,
    pt_2_pressure,
    ball_valve_open,
    sv_1_open,
    load_cell,
    ignition,
    qd_state,
    ms_since_boot_fill
) VALUES (
    -- Convert EMQX's internal arrival clock into a standard PostgreSQL TIMESTAMPTZ
    to_timestamp(${broker_arrival_ms} / 1000.0),
    
    -- Unit ID
    ${unit_id},

    -- Top-Level Packet Info
    ${sync_word},

    -- Shared Telemetry
    ${flight_mode},
    ${pressure},
    ${temp},
    ${altitude},
    ${latitude},
    ${longitude},
    ${num_satellites},
    ${gps_time},
    ${mag_x},
    ${mag_y},
    ${mag_z},
    ${accel_x},
    ${accel_y},
    ${accel_z},
    ${gyro_x},
    ${gyro_y},
    ${gyro_z},
    ${pt3},
    ${pt4},
    ${rtd},
    ${sv_2_open},
    ${mav_open},
    ${ms_since_boot_cfc},

    -- Event Flags
    ${ssa_drogue_deployed},
    ${ssa_main_deployed},
    ${cmd_n1},
    ${cmd_n2},
    ${cmd_n3},
    ${cmd_n4},
    ${cmd_a1},
    ${cmd_a2},
    ${cmd_a3},

    -- Airbrake & Control States
    ${airbrake_deployment},
    ${predicted_apogee},

    -- Advanced GPS / U-Blox Metrics
    ${h_acc},
    ${v_acc},
    ${vel_n},
    ${vel_e},
    ${vel_d},
    ${g_speed},
    ${s_acc},
    ${head_acc},
    ${fix_type},
    ${head_mot},

    -- BLiMS Outputs
    ${blims_brakeline_diff},
    ${blims_phase_id},
    ${blims_pid_p},
    ${blims_pid_i},
    ${blims_bearing},

    -- BLiMS Config
    ${blims_upwind_lat},
    ${blims_upwind_lon},
    ${blims_downwind_lat},
    ${blims_downwind_lon},
    ${blims_wind_from_deg},

    -- Fill Station Specific
    ${pt_1_pressure},
    ${pt_2_pressure},
    ${ball_valve_open},
    ${sv_1_open},
    ${load_cell},
    ${ignition},
    ${qd_state},
    ${ms_since_boot_fill}
);
