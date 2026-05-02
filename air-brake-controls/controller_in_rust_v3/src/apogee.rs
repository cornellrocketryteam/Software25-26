//! Apogee predictor.
//!
//! Implements a forward Euler integration to simulate the flight path to apogee,
//! accounting for varying air density.

use crate::constants::{
    AIRBRAKE_AREA_MAX_M2, AIRBRAKE_AREA_MIN_M2, AIRBRAKE_CD, BODY_AREA_M2, BODY_CD, G, L, MASS_KG, R, T0_K,
};

pub fn deployment_to_area(deployment: f32) -> f32 {
    AIRBRAKE_AREA_MIN_M2 + (AIRBRAKE_AREA_MAX_M2 - AIRBRAKE_AREA_MIN_M2) * deployment
}

pub fn air_density(altitude: f32, ground_pressure: f32) -> f32 {
    let t = (T0_K - L * altitude).max(1.0);
    let p = ground_pressure * libm::powf(t / T0_K, G / (R * L));
    (p / (R * t)).max(0.001)
}

/// Total drag coefficient × area at a given deployment.
pub fn cd_a_total(deployment: f32) -> f32 {
    BODY_CD * BODY_AREA_M2 + AIRBRAKE_CD * deployment_to_area(deployment)
}

/// Predict apogee given current altitude (m), upward velocity (m/s), tilt (degrees),
/// deployment (0..1), and ground pressure (Pa).
///
/// Uses a forward Euler integration (single-pass) to simulate the remaining flight
/// path, accurately accounting for varying air density with altitude.
pub fn predict_apogee(
    altitude: f32,
    velocity_up: f32,
    tilt_deg: f32,
    deployment: f32,
    ground_pressure: f32,
) -> f32 {
    if velocity_up <= 0.0 {
        return altitude;
    }

    const DT: f32 = 0.01;
    let cos_tilt = libm::cosf(libm::fabsf(tilt_deg).to_radians()).max(1e-6);
    let cd_a = cd_a_total(deployment);

    let mut h = altitude;
    let mut v = velocity_up;

    // Safety limit to prevent infinite loops if math goes wrong
    let mut iters = 0;
    while v > 0.0 && iters < 5000 {
        let rho = air_density(h, ground_pressure);

        // True axial velocity through the air mass
        let v_air = libm::fabsf(v) / cos_tilt;
        let dynamic_pressure = 0.5 * rho * v_air * v_air;

        let drag = dynamic_pressure * cd_a;

        // Acceleration (downward is negative)
        let acc = -G - (drag / MASS_KG);

        // Forward Euler step
        let v_next = v + acc * DT;
        let h_next = h + v * DT + 0.5 * acc * DT * DT;

        v = v_next;
        h = h_next;
        iters += 1;
    }

    h
}
