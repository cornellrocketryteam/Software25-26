use heapless::Vec as HVec;
use crate::controller::{AirbrakeController, Phase, SensorData, GROUND_TEMP_K, TARGET_APOGEE};
use crate::gyro_calibration::{compute_drift, PAD_CALIBRATION_COUNT};
use crate::measure_tilt::measure_tilt;

pub struct AirbrakeOutput {
    pub deployment: f32,
    pub predicted_apogee: f32,
    pub error: f32,
}

pub struct AirbrakeSystem {
    controller: AirbrakeController,
    pad_gyro_readings: HVec<(f32, f32, f32), PAD_CALIBRATION_COUNT>,
    pad_accel_readings: HVec<(f32, f32, f32), PAD_CALIBRATION_COUNT>,
    drift_x: f32,
    drift_y: f32,
    calibrated: bool,
}

impl AirbrakeSystem {
    pub fn new() -> Self {
        AirbrakeSystem {
            controller: AirbrakeController::new(TARGET_APOGEE, GROUND_TEMP_K),
            pad_gyro_readings: HVec::new(),
            pad_accel_readings: HVec::new(),
            drift_x: 0.0,
            drift_y: 0.0,
            calibrated: false,
        }
    }

    /// Called once per sensor reading by flight software.
    /// Returns deployment level (0.0–1.0), predicted apogee, and error.
    pub fn execute(
        &mut self,
        time: f32,
        altitude: f32,
        gyro_x: f32,
        gyro_y: f32,
        accel_x: f32,
        accel_y: f32,
        accel_z: f32,
        phase: Phase,
    ) -> AirbrakeOutput {
        // Collect calibration data during pad (only until we have 40 readings)
        if phase == Phase::Pad && !self.calibrated {
            let _ = self.pad_gyro_readings.push((time, gyro_x, gyro_y));
            let _ = self.pad_accel_readings.push((accel_x, accel_y, accel_z));

            if self.pad_gyro_readings.len() == PAD_CALIBRATION_COUNT {
                let drift = compute_drift(&self.pad_gyro_readings);
                self.drift_x = drift.x;
                self.drift_y = drift.y;

                let tilt = measure_tilt(&self.pad_accel_readings);
                self.controller.set_beginning_tilt(tilt.x_deg, tilt.y_deg);

                self.calibrated = true;
            }
        }

        // Apply drift correction for all non-pad phases
        let corrected_gyro_x = if phase == Phase::Pad { gyro_x } else { gyro_x - self.drift_x };
        let corrected_gyro_y = if phase == Phase::Pad { gyro_y } else { gyro_y - self.drift_y };

        let sensor_data = SensorData {
            time,
            altitude,
            gyro_x: corrected_gyro_x,
            gyro_y: corrected_gyro_y,
            phase,
        };

        let out = self.controller.step(&sensor_data);

        AirbrakeOutput {
            deployment: out.deployment,
            predicted_apogee: out.predicted_apogee,
            error: out.error,
        }
    }
}
