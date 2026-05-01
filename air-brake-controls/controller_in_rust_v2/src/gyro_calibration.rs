/// Gyro drift calibration from pad-phase readings.
///
/// During the pad phase the rocket is stationary, so any change in the gyro
/// reading is pure sensor drift.  We compute the average rate of change
/// (deg/s per second) for each axis across 40 consecutive pad readings
/// and return those as the per-axis drift rates.

pub const PAD_CALIBRATION_COUNT: usize = 40;

pub struct GyroDriftRates {
    pub x: f32,   // deg/s per second
    pub y: f32,
}

/// `readings` must contain exactly `PAD_CALIBRATION_COUNT` (40) entries of
/// (time_s, gyro_x, gyro_y) from the pad phase.
pub fn compute_drift(readings: &[(f32, f32, f32)]) -> GyroDriftRates {
    if readings.len() != PAD_CALIBRATION_COUNT {
        return GyroDriftRates { x: 0.0, y: 0.0 };
    }

    let n = readings.len();
    let (mut sum_x, mut sum_y) = (0.0f32, 0.0f32);
    let mut valid_pairs = 0u32;

    for i in 0..(n - 1) {
        let dt = readings[i + 1].0 - readings[i].0;
        if dt > 0.0 {
            sum_x += (readings[i + 1].1 - readings[i].1) / dt;
            sum_y += (readings[i + 1].2 - readings[i].2) / dt;
            valid_pairs += 1;
        }
    }

    if valid_pairs == 0 {
        return GyroDriftRates { x: 0.0, y: 0.0 };
    }

    let n = valid_pairs as f32;
    GyroDriftRates {
        x: sum_x / n,
        y: sum_y / n,
    }
}
