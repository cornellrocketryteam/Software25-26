SELECT
    -- Top-Level Radio
    payload.sync_word as sync_word,

    -- Shared Telemetry (Rust 'Packet' Struct)
    payload.flight_mode as flight_mode,
    payload.pressure as pressure,
    payload.temp as temp,
    payload.altitude as altitude,
    payload.latitude as latitude,
    payload.longitude as longitude,
    payload.num_satellites as num_satellites,
    payload.timestamp as timestamp,
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
    payload.sv_open as sv_open,
    payload.mav_open as mav_open,

    -- Fill Station Specific
    payload.pt_1_pressure as pt_1_pressure,
    payload.pt_2_pressure as pt_2_pressure,
    payload.ball_valve_open as ball_valve_open,
    payload.sv_1_open as sv_1_open,
    payload.sv_2_open as sv_2_open,
    payload.load_cell as load_cell,
    payload.ignition as ignition,
    payload.qd_state as qd_state,

    -- Unit ID routing (Dynamically grabbed from topic "rats/raw/0")
    int(nth(3, split(topic, '/'))) as unit_id
  
FROM
    "rats/raw/+"
