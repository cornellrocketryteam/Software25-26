//! Physical and tuning constants. Vehicle params confirmed against current LV airframe.

pub const TARGET_APOGEE_M: f32 = 3048.0; // 10,000 ft

// ISA atmosphere
pub const G: f32 = 9.80665;
pub const R: f32 = 287.05;
pub const L: f32 = 0.0065;
pub const T0_K: f32 = 288.15;
pub const SEA_LEVEL_PRESSURE_PA: f32 = 101_325.0;

// Vehicle (LV)
pub const MASS_KG: f32 = 51.26; // 113 lb
pub const BODY_DIAMETER_M: f32 = 0.1524; // 6 in
pub const BODY_AREA_M2: f32 =
    core::f32::consts::PI * (BODY_DIAMETER_M * 0.5) * (BODY_DIAMETER_M * 0.5);
pub const BODY_CD: f32 = 0.5;

// Airbrake aero
pub const AIRBRAKE_CD: f32 = 0.3;
pub const AIRBRAKE_AREA_MIN_M2: f32 = 0.001848; // ~2.86 in², fully retracted
pub const AIRBRAKE_AREA_MAX_M2: f32 = 0.021935; // 34 in², fully deployed

// Failsafe thresholds
pub const MAX_TILT_DEG: f32 = 50.0;

// Control tuning
pub const APOGEE_ERROR_FOR_FULL_DEPLOY_M: f32 = 200.0;
pub const MAX_DEPLOYMENT_RATE_PER_SEC: f32 = 2.0; // 0→1 in 0.5 s
