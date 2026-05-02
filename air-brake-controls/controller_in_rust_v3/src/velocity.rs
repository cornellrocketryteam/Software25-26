//! Upward velocity estimator.
//!
//! Primary: GPS vel_d, sign-flipped (vel_d positive = descending).
//! Fallback: least-squares slope of the last 5 (altitude, time) samples.
//!
//! GPS is preferred when |vel_d| ≥ MIN_GPS_VEL_TO_TRUST. NaN/inf falls through to baro.

use heapless::Deque;

const BARO_BUFFER_SIZE: usize = 5;
const MIN_GPS_VEL_TO_TRUST: f32 = 1.0;

pub struct VelocityEstimator {
    altitudes: Deque<f32, BARO_BUFFER_SIZE>,
    times: Deque<f32, BARO_BUFFER_SIZE>,
    last: f32,
}

impl VelocityEstimator {
    pub fn new() -> Self {
        Self {
            altitudes: Deque::new(),
            times: Deque::new(),
            last: 0.0,
        }
    }

    /// Push a new (altitude, time, vel_d) sample and return the current upward-velocity estimate.
    pub fn update(&mut self, altitude: f32, time: f32, vel_d: f32) -> f32 {
        if self.altitudes.is_full() {
            let _ = self.altitudes.pop_front();
            let _ = self.times.pop_front();
        }
        let _ = self.altitudes.push_back(altitude);
        let _ = self.times.push_back(time);

        let v_gps_up = -vel_d;
        if v_gps_up.is_finite() && libm::fabsf(v_gps_up) >= MIN_GPS_VEL_TO_TRUST {
            self.last = v_gps_up;
            return v_gps_up;
        }

        self.last = self.baro_slope().unwrap_or(self.last);
        self.last
    }

    fn baro_slope(&self) -> Option<f32> {
        let n = self.altitudes.len();
        if n < 2 {
            return None;
        }
        let n_f = n as f32;
        let t_mean: f32 = self.times.iter().sum::<f32>() / n_f;
        let h_mean: f32 = self.altitudes.iter().sum::<f32>() / n_f;
        let mut num = 0.0f32;
        let mut den = 0.0f32;
        for (t, a) in self.times.iter().zip(self.altitudes.iter()) {
            let dt = t - t_mean;
            num += dt * (a - h_mean);
            den += dt * dt;
        }
        if den > 0.0 {
            Some(num / den)
        } else {
            None
        }
    }
}

impl Default for VelocityEstimator {
    fn default() -> Self {
        Self::new()
    }
}
