CREATE TABLE telemetry_data (
    -- =========================================================================
    -- CORE / SHARED COLUMNS
    -- Data provided by all units (Fill Station, Primary RATS, Secondary RATS)
    -- =========================================================================
    time TIMESTAMPTZ NOT NULL,         -- Added by MQTT Broker/Rule Engine
    unit_id SMALLINT NOT NULL,         -- ID of source (0=Fill Station, 1=Primary RATS, etc.)
    ms_since_boot BIGINT,              -- uint32_t from packet
    battery_voltage DOUBLE PRECISION,  -- float from packet
    pt_3_pressure DOUBLE PRECISION,    -- float from packet
    pt_4_pressure DOUBLE PRECISION,    -- float from packet
    rtd_temperature DOUBLE PRECISION,  -- float from packet
    altitude DOUBLE PRECISION,         -- float from packet
    
    -- Metadata fields (Decoded from 16-bit integer)
    metadata_altitude_armed BOOLEAN,         -- Bit 0
    metadata_altimeter_is_valid BOOLEAN,     -- Bit 1
    metadata_gps_is_valid BOOLEAN,           -- Bit 2
    metadata_imu_is_valid BOOLEAN,           -- Bit 3
    metadata_accelerometer_is_valid BOOLEAN, -- Bit 4
    metadata_umbilical_lock BOOLEAN,         -- Bit 5
    metadata_adc_is_valid BOOLEAN,           -- Bit 6
    metadata_fram_is_valid BOOLEAN,          -- Bit 7
    metadata_sd_card_is_valid BOOLEAN,       -- Bit 8
    metadata_gps_message_fresh BOOLEAN,      -- Bit 9
    metadata_rocket_was_safed BOOLEAN,       -- Bit 10
    metadata_mav_state BOOLEAN,              -- Bit 11
    metadata_sv_state BOOLEAN,               -- Bit 12
    metadata_flight_mode SMALLINT,           -- Bits 13-15

    -- Array of active event bit numbers (e.g., [1, 14])
    events SMALLINT[],

    -- Additional Shared Data (provided by all units including Fill Station)
    sync_word BIGINT,                  -- uint32_t from packet
    temperature DOUBLE PRECISION,      -- float from packet
    latitude INTEGER,                  -- int32_t (microdegrees) from packet
    longitude INTEGER,                 -- int32_t (microdegrees) from packet
    satellites_in_view SMALLINT,       -- uint8_t from packet
    unix_time BIGINT,                  -- uint32_t (raw GPS epoch time)
    horizontal_accuracy BIGINT,        -- uint32_t (mm) from packet
    acceleration_x DOUBLE PRECISION,   -- IMU float from packet
    acceleration_y DOUBLE PRECISION,   -- IMU float from packet
    acceleration_z DOUBLE PRECISION,   -- IMU float from packet
    gyro_x DOUBLE PRECISION,           -- IMU float from packet
    gyro_y DOUBLE PRECISION,           -- IMU float from packet
    gyro_z DOUBLE PRECISION,           -- IMU float from packet
    orientation_x DOUBLE PRECISION,    -- IMU float from packet
    orientation_y DOUBLE PRECISION,    -- IMU float from packet
    orientation_z DOUBLE PRECISION,    -- IMU float from packet
    accelerometer_x DOUBLE PRECISION,  -- High-G float from packet
    accelerometer_y DOUBLE PRECISION,  -- High-G float from packet
    accelerometer_z DOUBLE PRECISION,  -- High-G float from packet
    motor_state DOUBLE PRECISION,      -- BLiMS float from packet

    -- =========================================================================
    -- FILL STATION-SPECIFIC COLUMNS
    -- Will be NULL when unit_id > 0 (RATS units)
    -- =========================================================================
    pt_1_pressure DOUBLE PRECISION,    -- Fill Station PT1 float
    pt_2_pressure DOUBLE PRECISION,    -- Fill Station PT2 float
    ball_valve_open BOOLEAN,           -- State of ball valve
    sv_1_open BOOLEAN,                 -- State of Solenoid Valve 1
    sv_2_open BOOLEAN,                 -- State of Solenoid Valve 2
    load_cell DOUBLE PRECISION,        -- Load cell reading float
    ignition BOOLEAN                   -- Ignition Command Status
    qd_state SMALLINT                  -- Quick Disconnect step/state integer
);

-- Turn the 'telemetry_data' table into a TimescaleDB hypertable partitioned by 'time'.
SELECT create_hypertable('telemetry_data', 'time');

-- Create an index on unit_id and time for fast filtering by tracker/station.
CREATE INDEX idx_unit_id_time ON telemetry_data (unit_id, time DESC);
