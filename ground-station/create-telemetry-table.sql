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
    timestamp DOUBLE PRECISION,        -- f32 (GPS)
    mag_x DOUBLE PRECISION,            -- f32 (Magnetometer)
    mag_y DOUBLE PRECISION,            -- f32 (Magnetometer)
    mag_z DOUBLE PRECISION,            -- f32 (Magnetometer)
    accel_x DOUBLE PRECISION,          -- f32 (IMU)
    accel_y DOUBLE PRECISION,          -- f32 (IMU)
    accel_z DOUBLE PRECISION,          -- f32 (IMU)
    gyro_x DOUBLE PRECISION,           -- f32 (IMU)
    gyro_y DOUBLE PRECISION,           -- f32 (IMU)
    gyro_z DOUBLE PRECISION,           -- f32 (IMU)
    pt3 DOUBLE PRECISION,              -- f32 (ADC Ch 0)
    pt4 DOUBLE PRECISION,              -- f32 (ADC Ch 1)
    rtd DOUBLE PRECISION,              -- f32 (ADC Ch 2)
    sv_open BOOLEAN,                   -- bool (Valve State)
    mav_open BOOLEAN,                  -- bool (Valve State)

    -- Fill Station Specific (Umbilical)
    pt_1_pressure DOUBLE PRECISION,    -- Fill Station PT1
    pt_2_pressure DOUBLE PRECISION,    -- Fill Station PT2
    ball_valve_open BOOLEAN,           -- Motorized Ball Valve
    sv_1_open BOOLEAN,                 -- Solenoid Valve 1
    sv_2_open BOOLEAN,                 -- Solenoid Valve 2
    load_cell DOUBLE PRECISION,        -- Propellant Mass
    ignition BOOLEAN,                  -- Ignition Command
    qd_state SMALLINT                  -- Quick Disconnect Integer
);
