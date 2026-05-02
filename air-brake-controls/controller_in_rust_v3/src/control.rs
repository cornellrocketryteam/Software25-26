//! Proportional control on apogee error with rate limit.

use crate::constants::{
    APOGEE_ERROR_FOR_FULL_DEPLOY_M, MAX_DEPLOYMENT_RATE_PER_SEC, TARGET_APOGEE_M,
};

pub fn target_deployment(predicted_apogee: f32) -> f32 {
    let raw = (predicted_apogee - TARGET_APOGEE_M) / APOGEE_ERROR_FOR_FULL_DEPLOY_M;
    raw.clamp(0.0, 1.0)
}

pub fn rate_limit(target: f32, prev: f32, dt: f32) -> f32 {
    let max_step = MAX_DEPLOYMENT_RATE_PER_SEC * dt;
    let delta = target - prev;
    if delta > max_step {
        prev + max_step
    } else if delta < -max_step {
        prev - max_step
    } else {
        target
    }
}
