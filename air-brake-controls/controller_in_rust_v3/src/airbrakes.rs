//! Top-level controller: glues velocity, tilt, apogee predictor, and control law.

use crate::apogee::predict_apogee;
use crate::constants::{MAX_TILT_DEG, TARGET_APOGEE_M};
use crate::control::{rate_limit, target_deployment};
use crate::tilt::TiltEstimator;
use crate::types::{AirbrakeOutput, Phase, SensorInput};
use crate::velocity::VelocityEstimator;

pub struct AirbrakeSystem {
    velocity: VelocityEstimator,
    tilt: TiltEstimator,
    last_deployment: f32,
    last_time: Option<f32>,
}

impl AirbrakeSystem {
    pub fn new() -> Self {
        Self {
            velocity: VelocityEstimator::new(),
            tilt: TiltEstimator::new(),
            last_deployment: 0.0,
            last_time: None,
        }
    }

    pub fn execute(&mut self, input: &SensorInput) -> AirbrakeOutput {
        let dt = match self.last_time {
            Some(t) => (input.time - t).clamp(0.0, 0.5),
            None => 0.05,
        };
        self.last_time = Some(input.time);

        let velocity_up = self
            .velocity
            .update(input.altitude, input.time, input.vel_d);
        let tilt_deg = self
            .tilt
            .update(input.accel_x, input.accel_y, input.accel_z);

        let inputs_invalid = !input.altitude.is_finite()
            || !input.reference_pressure.is_finite()
            || input.reference_pressure <= 0.0;

        let abort = inputs_invalid
            || input.phase != Phase::Coast
            || velocity_up <= 0.0
            || tilt_deg > MAX_TILT_DEG;

        let (deployment, predicted_apogee) = if abort {
            // Rate-limit toward 0 so we don't slam the servo on a transient failsafe.
            let limited = rate_limit(0.0, self.last_deployment, dt);
            (limited, input.altitude)
        } else {
            let predicted = predict_apogee(
                input.altitude,
                velocity_up,
                tilt_deg,
                self.last_deployment,
                input.reference_pressure,
            );
            let target = target_deployment(predicted);
            let limited = rate_limit(target, self.last_deployment, dt);
            (limited, predicted)
        };

        self.last_deployment = deployment;

        AirbrakeOutput {
            deployment,
            predicted_apogee,
            error: predicted_apogee - TARGET_APOGEE_M,
            velocity_used: velocity_up,
            tilt_used: tilt_deg,
        }
    }
}

impl Default for AirbrakeSystem {
    fn default() -> Self {
        Self::new()
    }
}
