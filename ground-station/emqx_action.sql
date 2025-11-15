INSERT INTO telemetry_data(
    -- === Core Indexes ===
    "time",
    unit_id,

    -- === Shared Data ===
    ms_since_boot,
    battery_voltage,
    pt_3_pressure,
    rtd_temperature,
    altitude,

    -- === Shared Metadata ===
    metadata_altitude_armed,
    metadata_altimeter_is_valid,
    metadata_gps_is_valid,
    metadata_imu_is_valid,
    metadata_accelerometer_is_valid,
    metadata_umbilical_lock,
    metadata_adc_is_valid,
    metadata_fram_is_valid,
    metadata_sd_card_is_valid,
    metadata_gps_message_fresh,
    metadata_rocket_was_safed,
    metadata_mav_state,
    metadata_sv_state,
    metadata_flight_mode,

    -- === Shared Events ===
    events,

    -- === RATS-Specific Data ===
    sync_word,
    temperature,
    latitude,
    longitude,
    satellites_in_view,
    unix_time,
    horizontal_accuracy,
    pt_4_pressure,
    acceleration_x,
    acceleration_y,
    acceleration_z,
    gyro_x,
    gyro_y,
    gyro_z,
    orientation_x,
    orientation_y,
    orientation_z,
    accelerometer_x,
    accelerometer_y,
    accelerometer_z,
    motor_state,

    -- === Umbilical-Specific Data ===
    pt_1_pressure,
    pt_2_pressure,
    ball_valve_open,
    load_cell,
    ignition
)
VALUES (
    -- === Core Indexes ===
    NOW(),
    ${unit_id},

    -- === Shared Data ===
    ${ms_since_boot},
    ${battery_voltage},
    ${pt_3_pressure},
    ${rtd_temperature},
    ${altitude},

    -- === Shared Metadata ===
    ${metadata_altitude_armed},
    ${metadata_altimeter_is_valid},
    ${metadata_gps_is_valid},
    ${metadata_imu_is_valid},
    ${metadata_accelerometer_is_valid},
    ${metadata_umbilical_lock},
    ${metadata_adc_is_valid},
    ${metadata_fram_is_valid},
    ${metadata_sd_card_is_valid},
    ${metadata_gps_message_fresh},
    ${metadata_rocket_was_safed},
    ${metadata_mav_state},
    ${metadata_sv_state},
    ${metadata_flight_mode},

    -- === Shared Events ===
    ${events},

    -- === RATS-Specific Data ===
    ${sync_word},
    ${temperature},
    ${latitude},
    ${longitude},
    ${satellites_in_view},
    ${unix_time},
    ${horizontal_accuracy},
    ${pt_4_pressure},
    ${acceleration_x},
    ${acceleration_y},
    ${acceleration_z},
    ${gyro_x},
    ${gyro_y},
    ${gyro_z},
    ${orientation_x},
    ${orientation_y},
    ${orientation_z},
    ${accelerometer_x},
    ${accelerometer_y},
    ${accelerometer_z},
    ${motor_state},

    -- === Umbilical-Specific Data ===
    ${pt_1_pressure},
    ${pt_2_pressure},
    ${ball_valve_open},
    ${load_cell},
    ${ignition}
)
