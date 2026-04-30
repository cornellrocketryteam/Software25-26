use heapless::Deque;
use crate::rocket_sim::rocket_sim;

/// Fixed window length for the rolling sensor buffer.
const SENSOR_BUFFER_SIZE: usize = 10;

// -----------------------------------------------------------------------------
// Constants
// -----------------------------------------------------------------------------
pub const DT: f32 = 0.01;
pub const TARGET_APOGEE: f32 = 3048.0;
pub const R: f32 = 287.05;
pub const G: f32 = 9.80665;
pub const L: f32 = 0.0065;
pub const GROUND_TEMP_K: f32 = 288.15;
pub const AIRBRAKE_MIN: f32 = 0.0;
pub const AIRBRAKE_MAX: f32 = 1.0;
pub const AIRBRAKE_CD: f32 = 0.3;
pub const AIRBRAKE_AREA_MIN: f32 = 0.001848;   // 2.86479 in²
pub const AIRBRAKE_AREA_MAX: f32 = 0.021935;   // 34 in²
pub const MAX_TILT_DEG: f32 = 50.0;
pub const SEA_LEVEL_PRESSURE_PA: f32 = 101325.0;

// -----------------------------------------------------------------------------
// Flight phase — passed in by the flight computer / simulator
// -----------------------------------------------------------------------------
#[derive(Debug, Clone, PartialEq)]
pub enum Phase {
    Pad,
    Boost,
    Coast,
}

// -----------------------------------------------------------------------------
// Sensor data packet (barometer + gyroscope)
// -----------------------------------------------------------------------------
pub struct SensorData {
    pub time: f32,
    pub altitude: f32,
    pub gyro_x: f32,
    pub gyro_y: f32,
    pub phase: Phase,
}

// -----------------------------------------------------------------------------
// Sensor buffer — rolling window of the last SENSOR_BUFFER_SIZE readings
// Uses heapless::Deque so no heap allocator is required.
// -----------------------------------------------------------------------------
pub struct SensorBuffer {
    altitudes:  Deque<f32, SENSOR_BUFFER_SIZE>,
    timestamps: Deque<f32, SENSOR_BUFFER_SIZE>,
}

impl SensorBuffer {
    pub fn new() -> Self {
        SensorBuffer {
            altitudes:  Deque::new(),
            timestamps: Deque::new(),
        }
    }

    pub fn add(&mut self, altitude: f32, timestamp: f32) {
        if self.altitudes.is_full() {
            self.altitudes.pop_front();
            self.timestamps.pop_front();
        }
        let _ = self.altitudes.push_back(altitude);
        let _ = self.timestamps.push_back(timestamp);
    }

    pub fn is_ready(&self) -> bool {
        self.altitudes.is_full()
    }

    /// Velocity via least-squares linear fit over all buffered points.
    /// slope = Σ((t_i - t̄)(h_i - h̄)) / Σ((t_i - t̄)²)
    pub fn get_velocity(&self) -> f32 {
        let n = self.altitudes.len();
        if n < 2 {
            return 0.0;
        }
        let n_f = n as f32;
        let t_mean: f32 = self.timestamps.iter().sum::<f32>() / n_f;
        let h_mean: f32 = self.altitudes.iter().sum::<f32>() / n_f;

        let mut num = 0.0f32;
        let mut den = 0.0f32;
        for (t, a) in self.timestamps.iter().zip(self.altitudes.iter()) {
            let dt = t - t_mean;
            num += dt * (a - h_mean);
            den += dt * dt;
        }

        if den > 0.0 { num / den } else { 0.0 }
    }

    pub fn last_altitude(&self) -> f32 {
        self.altitudes.back().copied().unwrap_or(0.0)
    }
}

impl Default for SensorBuffer {
    fn default() -> Self {
        Self::new()
    }
}

// -----------------------------------------------------------------------------
// Aerodynamic helpers
// -----------------------------------------------------------------------------
pub fn air_density(altitude: f32, p0: f32, t0: f32) -> f32 {
    let t = (t0 - L * altitude).max(1.0);
    let p = p0 * libm::powf(t / t0, G / (R * L));
    (p / (R * t)).max(0.001)
}

pub fn deployment_to_area(deployment: f32) -> f32 {
    AIRBRAKE_AREA_MIN + (AIRBRAKE_AREA_MAX - AIRBRAKE_AREA_MIN) * deployment
}

// -----------------------------------------------------------------------------
// Controller output — returned every step
// -----------------------------------------------------------------------------
pub struct ControllerOutput {
    pub deployment: f32,
    pub predicted_apogee: f32,
    pub error: f32,
}

// -----------------------------------------------------------------------------
// Main controller
// -----------------------------------------------------------------------------
pub struct AirbrakeController {
    pub target_apogee: f32,
    pub ground_pressure: f32,
    pub ground_temp: f32,
    ground_pressure_calibrated: bool,
    pub sensor_buffer: SensorBuffer,
    pub current_airbrake: f32,
    pub beginning_tilt: f32,
    pub integrated_tilt_x: f32,
    pub integrated_tilt_y: f32,
    pub integrated_tilt: f32,
    pub coast_initialized: bool,
    previous_time: Option<f32>,
    pub burnout_velocity: f32,
}

impl AirbrakeController {
    pub fn new(target_apogee: f32, ground_temp: f32) -> Self {
        AirbrakeController {
            target_apogee,
            ground_pressure: 0.0,
            ground_temp,
            ground_pressure_calibrated: false,
            sensor_buffer: SensorBuffer::new(),
            current_airbrake: 0.0,
            beginning_tilt: 0.0,
            integrated_tilt_x: 0.0,
            integrated_tilt_y: 0.0,
            integrated_tilt: 0.0,
            coast_initialized: false,
            previous_time: None,
            burnout_velocity: 0.0,
        }
    }

    pub fn set_beginning_tilt(&mut self, tilt_x_deg: f32, tilt_y_deg: f32) {
        self.integrated_tilt_x = tilt_x_deg;
        self.integrated_tilt_y = tilt_y_deg;
        self.integrated_tilt = libm::sqrtf(
            tilt_x_deg * tilt_x_deg + tilt_y_deg * tilt_y_deg
        );
        self.beginning_tilt = self.integrated_tilt;
    }

    fn integrate_gyroscope(&mut self, gyro_x: f32, gyro_y: f32, dt: f32) {
        self.integrated_tilt_x += gyro_x * dt;
        self.integrated_tilt_y += gyro_y * dt;
        self.integrated_tilt = libm::sqrtf(
            self.integrated_tilt_x * self.integrated_tilt_x
            + self.integrated_tilt_y * self.integrated_tilt_y
        );
    }

    /// Binary search (20 iters) for deployment that hits target apogee.
    fn find_optimal_deployment(&self, height: f32, velocity: f32, tilt_deg: f32) -> f32 {
        let predicted_no_brakes = rocket_sim(height, velocity, tilt_deg, 0.0, self.ground_pressure);
        if predicted_no_brakes <= self.target_apogee {
            return AIRBRAKE_MIN;
        }

        let mut lo = AIRBRAKE_MIN;
        let mut hi = AIRBRAKE_MAX;
        for _ in 0..20 {
            let mid = (lo + hi) / 2.0;
            let predicted = rocket_sim(height, velocity, tilt_deg, mid, self.ground_pressure);
            if predicted >= self.target_apogee {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo.min(AIRBRAKE_MAX)
    }

    pub fn step(&mut self, sensor_data: &SensorData) -> ControllerOutput {
        let current_time = sensor_data.time;

        if !self.ground_pressure_calibrated {
            self.ground_pressure = SEA_LEVEL_PRESSURE_PA
                * libm::powf(1.0 - sensor_data.altitude / 44330.0, 1.0 / 0.1903);
            self.ground_pressure_calibrated = true;
        }

        let dt = self.previous_time.map_or(DT, |pt| current_time - pt);
        self.previous_time = Some(current_time);

        self.sensor_buffer.add(sensor_data.altitude, current_time);
        self.integrate_gyroscope(sensor_data.gyro_x, sensor_data.gyro_y, dt);

        let mut predicted_apogee = 0.0f32;

        match sensor_data.phase {
            Phase::Pad => {}

            Phase::Boost => {
                if self.sensor_buffer.is_ready() {
                    self.burnout_velocity = self.sensor_buffer.get_velocity();
                }
            }

            Phase::Coast => {
                let height = sensor_data.altitude;
                let velocity = if !self.coast_initialized {
                    self.burnout_velocity
                } else {
                    self.sensor_buffer.get_velocity()
                };
                let tilt = self.integrated_tilt;

                if !self.coast_initialized && self.sensor_buffer.is_ready() {
                    self.coast_initialized = true;
                }

                if self.coast_initialized && velocity <= 0.0 {
                    self.current_airbrake = AIRBRAKE_MIN;
                    predicted_apogee = height;
                    let error = predicted_apogee - self.target_apogee;
                    return ControllerOutput {
                        deployment: self.current_airbrake,
                        predicted_apogee,
                        error,
                    };
                }

                if tilt > MAX_TILT_DEG {
                    self.current_airbrake = AIRBRAKE_MIN;
                    predicted_apogee = rocket_sim(height, velocity, tilt, self.current_airbrake, self.ground_pressure);
                    let error = predicted_apogee - self.target_apogee;
                    return ControllerOutput {
                        deployment: self.current_airbrake,
                        predicted_apogee,
                        error,
                    };
                }

                self.current_airbrake = self.find_optimal_deployment(height, velocity, tilt);
                predicted_apogee = rocket_sim(height, velocity, tilt, self.current_airbrake, self.ground_pressure);
            }
        }

        let error = predicted_apogee - self.target_apogee;
        ControllerOutput {
            deployment: self.current_airbrake,
            predicted_apogee,
            error,
        }
    }
}
