CREATE TABLE telemetry_data (
    -- Core Routing
    time TIMESTAMPTZ NOT NULL,         -- Added by MQTT Broker
    unit_id SMALLINT NOT NULL,         -- ID of source (0=Fill Station, 1=RATS, etc.)
    
    -- Top-Level Radio Data
    sync_word BIGINT,                  -- 32-bit unsigned integer

    -- Shared Telemetry (Rust 'Packet' Struct)
    flight_mode BIGINT,                -- u32
    pressure DOUBLE PRECISION,         -- f32 (Altimeter)
    temp DOUBLE PRECISION,             -- f32 (Altimeter)
    altitude DOUBLE PRECISION,         -- f32 (Altimeter)
    latitude DOUBLE PRECISION,         -- f32 (GPS)
    longitude DOUBLE PRECISION,        -- f32 (GPS)
    num_satellites BIGINT,             -- u32 (GPS)
    gps_time DOUBLE PRECISION,        -- f32 (GPS)
    mag_x DOUBLE PRECISION,            -- f32 (Magnetometer)
    mag_y DOUBLE PRECISION,            -- f32 (Magnetometer)
    mag_z DOUBLE PRECISION,            -- f32 (Magnetometer)
    accel_x DOUBLE PRECISION,          -- f32 (IMU)
    accel_y DOUBLE PRECISION,          -- f32 (IMU)
    accel_z DOUBLE PRECISION,          -- f32 (IMU)
    gyro_x DOUBLE PRECISION,           -- f32 (IMU)
    gyro_y DOUBLE PRECISION,           -- f32 (IMU)
    gyro_z DOUBLE PRECISION,           -- f32 (IMU)
    pt3 DOUBLE PRECISION,              -- f32 (ADC Ch 3)
    pt4 DOUBLE PRECISION,              -- f32 (ADC Ch 2)
    rtd DOUBLE PRECISION,              -- f32 (ADC Ch 1)
    sv_2_open BOOLEAN,                   -- bool (Valve State)
    mav_open BOOLEAN,                  -- bool (Valve State)
    ms_since_boot_cfc BIGINT,          -- u32

    -- Event Flags
    ssa_drogue_deployed SMALLINT,      -- u8
    ssa_main_deployed SMALLINT,        -- u8
    cmd_n1 SMALLINT,                   -- u8
    cmd_n2 SMALLINT,                   -- u8
    cmd_n3 SMALLINT,                   -- u8
    cmd_n4 SMALLINT,                   -- u8
    cmd_a1 SMALLINT,                   -- u8
    cmd_a2 SMALLINT,                   -- u8
    cmd_a3 SMALLINT,                   -- u8

    -- Airbrake & Control States
    airbrake_deployment DOUBLE PRECISION, -- f32 (0.0=retracted, 1.0=fully deployed)
    predicted_apogee DOUBLE PRECISION, -- f32

    -- Advanced GPS / U-Blox Metrics
    h_acc BIGINT,                      -- u32 (mm)
    v_acc BIGINT,                      -- u32 (mm)
    vel_n DOUBLE PRECISION,            -- f64 (m/s)
    vel_e DOUBLE PRECISION,            -- f64 (m/s)
    vel_d DOUBLE PRECISION,            -- f64 (m/s)
    g_speed DOUBLE PRECISION,          -- f64 (m/s)
    s_acc BIGINT,                      -- u32 (mm/s)
    head_acc BIGINT,                   -- u32 (deg*1e5)
    fix_type SMALLINT,                 -- u8
    head_mot INTEGER,                  -- i32 (deg*1e5)

    -- BLiMS Outputs
    blims_motor_position DOUBLE PRECISION, -- f32
    blims_phase_id SMALLINT,               -- i8
    blims_pid_p DOUBLE PRECISION,          -- f32
    blims_pid_i DOUBLE PRECISION,          -- f32
    blims_bearing DOUBLE PRECISION,        -- f32
    blims_loiter_step SMALLINT,            -- i8
    blims_heading_des DOUBLE PRECISION,    -- f32
    blims_heading_error DOUBLE PRECISION,  -- f32
    blims_error_integral DOUBLE PRECISION, -- f32
    blims_dist_to_target_m DOUBLE PRECISION, -- f32

    -- BLiMS Config
    blims_target_lat DOUBLE PRECISION,     -- f32
    blims_target_lon DOUBLE PRECISION,     -- f32
    blims_wind_from_deg DOUBLE PRECISION,  -- f32

    -- Fill Station Specific (Umbilical)
    pt_1_pressure DOUBLE PRECISION,    -- Fill Station PT1
    pt_2_pressure DOUBLE PRECISION,    -- Fill Station PT2
    ball_valve_open BOOLEAN,           -- Motorized Ball Valve
    sv_1_open BOOLEAN,                 -- Solenoid Valve 1
    load_cell DOUBLE PRECISION,        -- Propellant Mass
    ignition BOOLEAN,                  -- Ignition Command
    qd_state SMALLINT,                  -- Quick Disconnect Integer
    ms_since_boot_fill BIGINT         -- u32
);

SELECT create_hypertable('telemetry_data', 'time');
CREATE INDEX idx_unit_id_time ON telemetry_data (unit_id, time DESC);
