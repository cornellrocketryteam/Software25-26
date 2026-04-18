//! Airbrake controller — runs on Core 1.
//!
//! ## How it works
//!
//! The RP2350B has two Cortex-M33 cores.  Core 0 runs the main Embassy
//! executor (flight loop, sensors, actuators, USB/radio).  Core 1 runs its
//! own Embassy executor with exactly one task: this airbrake task.
//!
//! Every time Core 0 reads sensors it signals Core 1 via `AIRBRAKE_INPUT`
//! (an Embassy Signal — always holds the *latest* value, no queuing).
//! Core 1 wakes up, runs the controller (including 20× rocket_sim binary
//! search), and writes the resulting deployment level to `AIRBRAKE_DEPLOYMENT`
//! (an AtomicU32 storing f32 bits — lock-free single-word exchange).
//!
//! Core 0 reads the deployment level non-blocking with `get_deployment()`.
//! If Core 1 hasn't finished its computation yet the previous value is used,
//! which is fine — the controller output changes slowly relative to 20 Hz.
//!
//! ## Phases
//!
//! | FSW FlightMode          | AirbrakePhase | Controller behaviour              |
//! |-------------------------|---------------|-----------------------------------|
//! | Startup / Standby       | Pad           | Collect gyro/accel calibration    |
//! | Ascent                  | Boost         | Track burnout velocity            |
//! | Coast                   | Coast         | Run binary-search deployment ctrl |
//! | DrogueDeployed and later| (not sent)    | Core 1 blocks, airbrakes retract  |

use core::sync::atomic::{AtomicU32, Ordering};

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;

use controller_in_rust::airbrakes::AirbrakeSystem;
use controller_in_rust::controller::Phase;

// ---------------------------------------------------------------------------
// Shared phase enum — mirrors controller::Phase without importing it into the
// flight loop (keeps the coupling one-directional).
// ---------------------------------------------------------------------------
#[derive(Clone, Copy)]
pub enum AirbrakePhase {
    Pad,
    Boost,
    Coast,
}

// ---------------------------------------------------------------------------
// Input signal: Core 0 writes latest sensor data, Core 1 waits for it.
// Signal always delivers the *most recent* value — if Core 1 is slow it
// silently skips intermediate frames rather than building up a backlog.
// ---------------------------------------------------------------------------
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

// ---------------------------------------------------------------------------
// Output: deployment level stored as f32 bits in an AtomicU32.
// Lock-free, wait-free — safe to read from Core 0 at any time.
// 0.0 = fully retracted (safe default before first computation).
// ---------------------------------------------------------------------------
static AIRBRAKE_DEPLOYMENT: AtomicU32 = AtomicU32::new(0); // 0.0f32 as bits

/// Read the latest deployment level (0.0 – 1.0) computed by Core 1.
/// Non-blocking — returns the last known value immediately.
pub fn get_deployment() -> f32 {
    f32::from_bits(AIRBRAKE_DEPLOYMENT.load(Ordering::Acquire))
}

// ---------------------------------------------------------------------------
// Core 1 task
// ---------------------------------------------------------------------------
#[embassy_executor::task]
pub async fn airbrake_core1_task() {
    // No logging on Core 1: the USB logger channel uses CriticalSectionRawMutex,
    // which on RP2350 is a cross-core hardware spinlock. Logging here blocks
    // Core 0's I²C/SPI transactions and can trip the flight-loop watchdog.
    let mut system = AirbrakeSystem::new();

    loop {
        let input = AIRBRAKE_INPUT.wait().await;

        let ctrl_phase = match input.phase {
            AirbrakePhase::Pad   => Phase::Pad,
            AirbrakePhase::Boost => Phase::Boost,
            AirbrakePhase::Coast => Phase::Coast,
        };

        let output = system.execute(
            input.time,
            input.altitude,
            input.gyro_x,
            input.gyro_y,
            input.accel_x,
            input.accel_y,
            input.accel_z,
            ctrl_phase,
        );

        AIRBRAKE_DEPLOYMENT.store(output.deployment.to_bits(), Ordering::Release);
    }
}
