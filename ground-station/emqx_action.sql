INSERT INTO telemetry_data(
  time, unit_id, sync_word, flight_mode, pressure, temp, altitude, 
  latitude, longitude, num_satellites, timestamp, mag_x, mag_y, mag_z, 
  accel_x, accel_y, accel_z, gyro_x, gyro_y, gyro_z, pt3, pt4, rtd, 
  sv_open, mav_open, pt_1_pressure, pt_2_pressure, ball_valve_open, 
  sv_1_open, sv_2_open, load_cell, ignition, qd_state
)
VALUES (
  NOW(), ${unit_id}, ${sync_word}, ${flight_mode}, ${pressure}, ${temp}, ${altitude}, 
  ${latitude}, ${longitude}, ${num_satellites}, ${timestamp}, ${mag_x}, ${mag_y}, ${mag_z}, 
  ${accel_x}, ${accel_y}, ${accel_z}, ${gyro_x}, ${gyro_y}, ${gyro_z}, ${pt3}, ${pt4}, ${rtd}, 
  ${sv_open}, ${mav_open}, ${pt_1_pressure}, ${pt_2_pressure}, ${ball_valve_open}, 
  ${sv_1_open}, ${sv_2_open}, ${load_cell}, ${ignition}, ${qd_state}
)
