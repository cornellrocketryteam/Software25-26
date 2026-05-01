/// Initial tilt measurement from accelerometer pad-phase readings.
///
/// While the rocket is stationary on the pad, the accelerometer reads only
/// gravity.  Any deviation from (0, 0, 1g) means the rocket is tilted from
/// vertical.  We average all 40 pad samples to reduce noise, then decompose
/// the tilt into its x and y components:
///
///   tilt_x = atan2(ax, az)
///   tilt_y = atan2(ay, az)

use num_traits::Float;

pub struct BeginningTilt {
    pub x_deg: f32,
    pub y_deg: f32,
}

/// `readings` is a slice of (accel_x, accel_y, accel_z) in g's.
/// Returns the x and y tilt components in degrees.
pub fn measure_tilt(readings: &[(f32, f32, f32)]) -> BeginningTilt {
    if readings.is_empty() {
        return BeginningTilt { x_deg: 0.0, y_deg: 0.0 };
    }

    let n = readings.len() as f32;
    let ax = readings.iter().map(|r| r.0).sum::<f32>() / n;
    let ay = readings.iter().map(|r| r.1).sum::<f32>() / n;
    let az = readings.iter().map(|r| r.2).sum::<f32>() / n;

    BeginningTilt {
        x_deg: libm::atan2f(ax, az).to_degrees(),
        y_deg: libm::atan2f(ay, az).to_degrees(),
    }
}
