//! Airbrake controller — runs on Core 1.

use core::sync::atomic::{AtomicU32, Ordering};

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;

use controller_in_rust_v3::airbrakes::AirbrakeSystem;
use controller_in_rust_v3::types::{Phase, SensorInput as ControllerInput};

#[derive(Clone, Copy)]
pub enum AirbrakePhase {
    Pad,
    Boost,
    Coast,
}

pub struct AirbrakeInput {
    pub time: f32,
    pub altitude: f32,
    pub vel_d: f32,
    pub reference_pressure: f32,
    pub gyro_x: f32,
    pub gyro_y: f32,
    pub gyro_z: f32,
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
    let mut system = AirbrakeSystem::new();

    loop {
        let input = AIRBRAKE_INPUT.wait().await;

        let ctrl_phase = match input.phase {
            AirbrakePhase::Pad   => Phase::Pad,
            AirbrakePhase::Boost => Phase::Boost,
            AirbrakePhase::Coast => Phase::Coast,
        };

        let output = system.execute(&ControllerInput {
            time: input.time,
            altitude: input.altitude,
            vel_d: input.vel_d,
            reference_pressure: input.reference_pressure,
            gyro_x: input.gyro_x,
            gyro_y: input.gyro_y,
            gyro_z: input.gyro_z,
            accel_x: input.accel_x,
            accel_y: input.accel_y,
            accel_z: input.accel_z,
            phase: ctrl_phase,
        });

        AIRBRAKE_DEPLOYMENT.store(output.deployment.to_bits(), Ordering::Release);
        AIRBRAKE_PREDICTED_APOGEE.store(output.predicted_apogee.to_bits(), Ordering::Release);
    }
}
