//! Tilt-from-accelerometer with first-order low-pass.
//!
//! Only updates when |a| ≈ 1g (i.e. quasi-static). Under thrust or freefall the filter
//! holds its previous value. Used solely for the 50° abort failsafe.

use crate::constants::G;

const FILTER_ALPHA: f32 = 0.05; // ~1 s τ at 20 Hz
const VALID_LOW: f32 = 0.5 * G;
const VALID_HIGH: f32 = 1.5 * G;

pub struct TiltEstimator {
    filtered_deg: f32,
    initialized: bool,
}

impl TiltEstimator {
    pub fn new() -> Self {
        Self {
            filtered_deg: 0.0,
            initialized: false,
        }
    }

    pub fn update(&mut self, ax: f32, ay: f32, az: f32) -> f32 {
        let mag = libm::sqrtf(ax * ax + ay * ay + az * az);
        if !mag.is_finite() || mag < VALID_LOW || mag > VALID_HIGH {
            return self.filtered_deg;
        }
        let horizontal = libm::sqrtf(ax * ax + ay * ay);
        let raw_deg = libm::atan2f(horizontal, libm::fabsf(az)).to_degrees();
        if !self.initialized {
            self.filtered_deg = raw_deg;
            self.initialized = true;
        } else {
            self.filtered_deg += FILTER_ALPHA * (raw_deg - self.filtered_deg);
        }
        self.filtered_deg
    }

    pub fn current(&self) -> f32 {
        self.filtered_deg
    }
}

impl Default for TiltEstimator {
    fn default() -> Self {
        Self::new()
    }
}
