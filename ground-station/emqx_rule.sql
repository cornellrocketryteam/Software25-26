SELECT
    -- === Shared Data (from both RATS & Umbilical) ===
    payload.ms_since_boot as ms_since_boot,
    payload.battery_voltage as battery_voltage,
    payload.pt_3_pressure as pt_3_pressure,
    payload.rtd_temperature as rtd_temperature,
    payload.altitude as altitude,

    -- === Shared Metadata (from both RATS & Umbilical) ===
    payload.metadata_altitude_armed as metadata_altitude_armed,
    payload.metadata_altimeter_is_valid as metadata_altimeter_is_valid,
    payload.metadata_gps_is_valid as metadata_gps_is_valid,
    payload.metadata_imu_is_valid as metadata_imu_is_valid,
    payload.metadata_accelerometer_is_valid as metadata_accelerometer_is_valid,
    payload.metadata_umbilical_lock as metadata_umbilical_lock,
    payload.metadata_adc_is_valid as metadata_adc_is_valid,
    payload.metadata_fram_is_valid as metadata_fram_is_valid,
    payload.metadata_sd_card_is_valid as metadata_sd_card_is_valid,
    payload.metadata_gps_message_fresh as metadata_gps_message_fresh,
    payload.metadata_rocket_was_safed as metadata_rocket_was_safed,
    payload.metadata_mav_state as metadata_mav_state,
    payload.metadata_sv_state as metadata_sv_state,
    payload.metadata_flight_mode as metadata_flight_mode,

    -- === Shared Events (from both RATS & Umbilical) ===
    payload.events as events,

    -- === RATS-Specific Data (NULL for Umbilical) ===
    payload.sync_word as sync_word,
    payload.temperature as temperature,
    payload.latitude as latitude,
    payload.longitude as longitude,
    payload.satellites_in_view as satellites_in_view,
    payload.unix_time as unix_time, -- This is now the original unix_time
    payload.horizontal_accuracy as horizontal_accuracy,
    payload.pt_4_pressure as pt_4_pressure,
    payload.acceleration_x as acceleration_x,
    payload.acceleration_y as acceleration_y,
    payload.acceleration_z as acceleration_z,
    payload.gyro_x as gyro_x,
    payload.gyro_y as gyro_y,
    payload.gyro_z as gyro_z,
    payload.orientation_x as orientation_x,
    payload.orientation_y as orientation_y,
    payload.orientation_z as orientation_z,
    payload.accelerometer_x as accelerometer_x,
    payload.accelerometer_y as accelerometer_y,
    payload.accelerometer_z as accelerometer_z,
    payload.motor_state as motor_state,

    -- === Umbilical-Specific Data (NULL for RATS) ===
    payload.pt_1_pressure as pt_1_pressure,
    payload.pt_2_pressure as pt_2_pressure,
    payload.ball_valve_open as ball_valve_open,
    payload.load_cell as load_cell,
    payload.ignition as ignition,

    -- === Unit ID (from topic) ===
    nth(3, split(topic, '/')) as unit_id
  
FROM
    "rats/raw/+"
