//! Airbrake task stub — controller_in_rust is not linked in this build.
//! Core 1 runs this task but does nothing; all deployment outputs stay at 0.

use core::sync::atomic::{AtomicU32, Ordering};

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;

#[derive(Clone, Copy)]
pub enum AirbrakePhase {
    Pad,
    Boost,
    Coast,
}

pub struct AirbrakeInput {
    pub time: f32,
    pub altitude: f32,
    pub gyro_x: f32,
    pub gyro_y: f32,
    pub accel_x: f32,
    pub accel_y: f32,
    pub accel_z: f32,
    pub phase: AirbrakePhase,
}

pub static AIRBRAKE_INPUT: Signal<CriticalSectionRawMutex, AirbrakeInput> = Signal::new();

static AIRBRAKE_DEPLOYMENT: AtomicU32 = AtomicU32::new(0);
static AIRBRAKE_PREDICTED_APOGEE: AtomicU32 = AtomicU32::new(0);

pub fn get_deployment() -> f32 {
    f32::from_bits(AIRBRAKE_DEPLOYMENT.load(Ordering::Acquire))
}

pub fn get_predicted_apogee() -> f32 {
    f32::from_bits(AIRBRAKE_PREDICTED_APOGEE.load(Ordering::Acquire))
}

#[embassy_executor::task]
pub async fn airbrake_core1_task() {
    loop {
        let _ = AIRBRAKE_INPUT.wait().await;
        // controller_in_rust not linked — deployment stays 0.0
    }
}
