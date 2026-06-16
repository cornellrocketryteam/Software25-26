#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Pad,
    Boost,
    Coast,
}

#[derive(Debug, Clone, Copy)]
pub struct SensorInput {
    pub time: f32,                // s, monotonic
    pub altitude: f32,            // m AGL
    pub vel_d: f32,               // m/s, NED-down (positive = descending)
    pub reference_pressure: f32,  // Pa, FSW-calibrated ground pressure
    pub gyro_x: f32,              // deg/s
    pub gyro_y: f32,
    pub gyro_z: f32,
    pub accel_x: f32,             // m/s²
    pub accel_y: f32,
    pub accel_z: f32,
    pub phase: Phase,
}

#[derive(Debug, Clone, Copy)]
pub struct AirbrakeOutput {
    pub deployment: f32,        // 0.0 ..= 1.0
    pub predicted_apogee: f32,  // m
    pub error: f32,             // predicted - target, m
    pub velocity_used: f32,     // m/s, upward — for FSW debug log
    pub tilt_used: f32,         // deg — for FSW debug log
}
