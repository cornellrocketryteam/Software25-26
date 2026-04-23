INSERT INTO telemetry_data(
  time, unit_id, sync_word, flight_mode, pressure, temp, altitude, 
  latitude, longitude, num_satellites, timestamp, mag_x, mag_y, mag_z, 
  accel_x, accel_y, accel_z, gyro_x, gyro_y, gyro_z, pt3, pt4, rtd, 
  sv_2_open, mav_open, 
  ssa_drogue_deployed, ssa_main_deployed, cmd_n1, cmd_n2, cmd_n3, cmd_n4, cmd_a1, cmd_a2, cmd_a3,
  airbrake_state, predicted_apogee,
  h_acc, v_acc, vel_n, vel_e, vel_d, g_speed, s_acc, head_acc, fix_type, head_mot,
  blims_motor_position, blims_phase_id, blims_pid_p, blims_pid_i, blims_bearing, blims_loiter_step,
  blims_heading_des, blims_heading_error, blims_error_integral, blims_dist_to_target_m,
  blims_target_lat, blims_target_lon, blims_wind_from_deg,
  pt_1_pressure, pt_2_pressure, ball_valve_open, sv_1_open, load_cell, ignition, qd_state
)
VALUES (
  NOW(), ${unit_id}, ${sync_word}, ${flight_mode}, ${pressure}, ${temp}, ${altitude}, 
  ${latitude}, ${longitude}, ${num_satellites}, ${timestamp}, ${mag_x}, ${mag_y}, ${mag_z}, 
  ${accel_x}, ${accel_y}, ${accel_z}, ${gyro_x}, ${gyro_y}, ${gyro_z}, ${pt3}, ${pt4}, ${rtd}, 
  ${sv_2_open}, ${mav_open}, 
  ${ssa_drogue_deployed}, ${ssa_main_deployed}, ${cmd_n1}, ${cmd_n2}, ${cmd_n3}, ${cmd_n4}, ${cmd_a1}, ${cmd_a2}, ${cmd_a3},
  ${airbrake_state}, ${predicted_apogee},
  ${h_acc}, ${v_acc}, ${vel_n}, ${vel_e}, ${vel_d}, ${g_speed}, ${s_acc}, ${head_acc}, ${fix_type}, ${head_mot},
  ${blims_motor_position}, ${blims_phase_id}, ${blims_pid_p}, ${blims_pid_i}, ${blims_bearing}, ${blims_loiter_step},
  ${blims_heading_des}, ${blims_heading_error}, ${blims_error_integral}, ${blims_dist_to_target_m},
  ${blims_target_lat}, ${blims_target_lon}, ${blims_wind_from_deg},
  ${pt_1_pressure}, ${pt_2_pressure}, ${ball_valve_open}, ${sv_1_open}, ${load_cell}, ${ignition}, ${qd_state}
)