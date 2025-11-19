-- ============================================================================
-- Create Telemetry Table
-- This single table is designed to hold data from ALL sources
-- (RATS units and the Umbilical/Fill Station).
--
-- - unit_id = 0 (Umbilical)
-- - unit_id = 1 (Primary RATS)
-- - unit_id = 2 (Secondary RATS)
--
-- RATS-specific fields (e.g., latitude, gyro_x) will be NULL for umbilical.
-- Umbilical-specific fields (e.g., load_cell, pt_1_pressure) will be NULL for RATS.
-- ============================================================================

CREATE TABLE telemetry_data (
    -- === Core Indexes ===
    "time" TIMESTAMPTZ NOT NULL,
    unit_id SMALLINT NOT NULL,

    -- === Shared Data (from both RATS & Umbilical) ===
    ms_since_boot BIGINT,  -- uint32_t
    battery_voltage DOUBLE PRECISION,
    pt_3_pressure DOUBLE PRECISION,   -- Shared PT (PT 3)
    rtd_temperature DOUBLE PRECISION,
    altitude DOUBLE PRECISION,

    -- === Shared Metadata (from both RATS & Umbilical) ===
    -- Note: Some fields may always be false/null for one source
    metadata_altitude_armed BOOLEAN,
    metadata_altimeter_is_valid BOOLEAN,
    metadata_gps_is_valid BOOLEAN,
    metadata_imu_is_valid BOOLEAN,
    metadata_accelerometer_is_valid BOOLEAN,
    metadata_umbilical_lock BOOLEAN,
    metadata_adc_is_valid BOOLEAN,
    metadata_fram_is_valid BOOLEAN,
    metadata_sd_card_is_valid BOOLEAN,
    metadata_gps_message_fresh BOOLEAN,
    metadata_rocket_was_safed BOOLEAN,
    metadata_mav_state BOOLEAN,
    metadata_sv_state BOOLEAN,
    metadata_flight_mode SMALLINT,

    -- === Shared Events (from both RATS & Umbilical) ===
    events SMALLINT[], -- Array of event IDs (bit numbers)

    -- === RATS-Specific Data (NULL for Umbilical) ===
    sync_word BIGINT, -- uint32_t
    temperature DOUBLE PRECISION,
    latitude INTEGER, -- int32_t (microdegrees)
    longitude INTEGER, -- int32_t (microdegrees)
    satellites_in_view SMALLINT, -- uint8_t
    unix_time BIGINT, -- uint32_t (from GPS, also used for "time")
    horizontal_accuracy BIGINT, -- uint32_t (mm)
    pt_4_pressure DOUBLE PRECISION,   -- RATS-specific PT (PT 4)
    
    -- IMU (RATS-specific)
    acceleration_x DOUBLE PRECISION,
    acceleration_y DOUBLE PRECISION,
    acceleration_z DOUBLE PRECISION,
    gyro_x DOUBLE PRECISION,
    gyro_y DOUBLE PRECISION,
    gyro_z DOUBLE PRECISION,
    orientation_x DOUBLE PRECISION,
    orientation_y DOUBLE PRECISION,
    orientation_z DOUBLE PRECISION,
    
    -- Accelerometer (RATS-specific)
    accelerometer_x DOUBLE PRECISION,
    accelerometer_y DOUBLE PRECISION,
    accelerometer_z DOUBLE PRECISION,
    
    -- BLiMS (RATS-specific)
    motor_state DOUBLE PRECISION,

    -- === Umbilical-Specific Data (NULL for RATS) ===
    pt_1_pressure DOUBLE PRECISION,   -- Umbilical-specific PT (PT 1)
    pt_2_pressure DOUBLE PRECISION,   -- Umbilical-specific PT (PT 2)
    ball_valve_open BOOLEAN,
    load_cell DOUBLE PRECISION,
    ignition BOOLEAN,

    -- This ensures that the combination of (time, unit_id) is unique.
    PRIMARY KEY ("time", unit_id)
);

-- --- TimescaleDB ---
-- Convert the table into a hypertable.
SELECT create_hypertable('telemetry_data', 'time', 'unit_id', 4, chunk_time_interval => INTERVAL '1 hour');
