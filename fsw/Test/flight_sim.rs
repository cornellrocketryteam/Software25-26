use crate::constants::{self, TEST_ALTS_LST};
use crate::flight_loop::{FlightLoop, LaunchStage};
use crate::state::{FlightMode, SensorState};
use embassy_time::{Duration, Instant, Timer};
// Runs a full flight simulation (Scenario of just simple transitions between each state) on the given FlightLoop object.
// This verifies logic transitions without needing real hardware inputs from the sensor modules.
pub async fn simulate_flight_simple(flight_loop: &mut FlightLoop) {
    log::info!("\n--- STARTING FLIGHT SIMULATION ---");

    // 1. Initial State: Startup
    if flight_loop.flight_state.flight_mode != FlightMode::Startup {
        log::warn!("Sim started but not in Startup mode");
        return;
    }
    flight_loop.set_altimeter_state(SensorState::VALID);
    flight_loop.set_altitude(0.0);
    flight_loop.set_pressure(101325.0);
    flight_loop.simulate_cycle().await;

    // 2. Transition to Standby
    Timer::after_secs(2).await;
    log::info!("[SIM] Testing Startup -> Standby");
    flight_loop.set_umbilical(true); // Umbilical must be connected now
    flight_loop.set_key_switch(true); // And key switch armed
    flight_loop.simulate_cycle().await;

    if flight_loop.flight_state.flight_mode == FlightMode::Standby {
        log::info!("\n[SIM] SUCCESS: Transitioned to Standby");
    } else {
        log::error!(
            "\n[SIM] FAILED: Did not transition to Standby. Mode: {:?}",
            flight_loop.flight_state.flight_mode
        );
    }

    // 3. Transition to Ascent
    Timer::after_secs(2).await;
    log::info!("[SIM] Testing Standby -> Ascent");
    flight_loop.set_umbilical(true);
    flight_loop.simulate_cycle().await;

    // Send Launch Command
    flight_loop.set_launch_command(true);
    flight_loop.simulate_cycle().await;

    if flight_loop.flight_state.flight_mode == FlightMode::Ascent {
        log::info!("\n[SIM] SUCCESS: Transitioned to Ascent");
        log::info!(
            "[SIM] MAV Open: {}, SV Open: {}",
            flight_loop.mav_open,
            flight_loop.sv_open
        );
    } else {
        log::error!("[SIM] FAILED: Did not transition to Ascent\n");
    }

    log::info!(
        "[SIM] Starting Flight Profile with {} altitude points...",
        constants::TEST_ALTS_LST.len()
    );

    // - Arming altitude (Ascent)
    // - MAV Close (Ascent -> Coast)
    // - Apogee (Coast -> Drogue)
    // - Main Deployment (Drogue -> Main)

    let mut mav_close_simulated = false;
    let mut drogue_deployed_verified = false;
    let mut main_deployed_verified = false;

    // We only need to check umbilical disconnect once
    flight_loop.set_umbilical(false);

    //let altitudes: [f32; 20] = [0.0, 100.0, 189.0, 311.0, 420.0, 732.0, 864.1, 1029.4, 1413.9, 1692.1, 1999.9, 2209.9, 2509.9, 2900.9, 2618.8, 2163.1, 1300.0, 949.0, 400.0, 0.0];

    for (_i, alt) in TEST_ALTS_LST.iter().enumerate() {
        if _i % 20 == 0 {
            log::info!("[SIM] Current Simulated Altitude: {:.2}m", alt);
        }
        flight_loop.set_altitude(*alt);
        Timer::after_millis(10).await; // Reduced delay to speed up simulation loop

        // Run logic for this altitude
        // Must run multiple cycles to saturate the moving average buffer at this EXTRA altitude
        for _ in 0..10 {
            flight_loop.simulate_cycle().await;
        }
        let mode = flight_loop.flight_state.flight_mode;

        // ASCENT Checks
        if mode == FlightMode::Ascent {
            if flight_loop.alt_armed && *alt > constants::ARMING_ALTITUDE {
                // Altimeter arming verification happens continuously
            }

            // Simulate MAV Close when passing 1000m (Ascent -> Coast)
            if !mav_close_simulated && flight_loop.get_altitude() > constants::ARMING_ALTITUDE {
                log::info!("[SIM] Simulating MAV/SV Close at {:.2}m", alt);
                flight_loop.set_mav_open(false);
                flight_loop.set_sv_open(false);
                mav_close_simulated = true;

                // The next cycle check_transitions move to Coast
                flight_loop.simulate_cycle().await;
                if flight_loop.flight_state.flight_mode == FlightMode::Coast {
                    log::info!("\n[SIM] SUCCESS: Transitioned to Coast");
                    Timer::after_secs(2).await;
                    // Trigger airbrakes
                    //flight_loop.set_airbrakes(true);
                }
            }
        }

        // COAST Checks (Apogee detection is automatic)
        if mode == FlightMode::Coast {
            //flight_loop.set_cameras_deployed(true);
            //flight_loop.set_airbrakes(false);
        }

        // DROGUE Checks
        if mode == FlightMode::DrogueDeployed {
            // Verify side effects once
            if flight_loop.camera_deployed
                && !flight_loop.airbrakes_init
                && !drogue_deployed_verified
            {
                log::info!(
                    "\n[SIM] SUCCESS: Transitioned to DrogueDeployed at {:.2}m",
                    alt
                );
                drogue_deployed_verified = true;
            }
        }

        // MAIN Checks
        if mode == FlightMode::MainDeployed {
            if !main_deployed_verified {
                log::info!(
                    "\n[SIM] SUCCESS: Transitioned to MainDeployed at {:.2}m",
                    alt
                );
                main_deployed_verified = true;
            }
        }
    }

    log::info!("\n--- SIMULATION COMPLETE ---");
}

// Runs fault scenario tests
pub async fn simulate_fault_scenarios(flight_loop: &mut FlightLoop) {
    log::info!("\n--- STARTING FAULT SIMULATION ---");

    // 1. Startup -> Fault (Invalid Altimeter)
    log::info!("\n[FAULT SIM] Testing Startup -> Fault");
    // Reset state
    flight_loop.flight_state.flight_mode = FlightMode::Startup;
    flight_loop.set_key_switch(true);
    flight_loop.set_altimeter_state(SensorState::INVALID);
    flight_loop.simulate_cycle().await;

    if flight_loop.flight_state.flight_mode == FlightMode::Fault {
        log::info!("[FAULT SIM] SUCCESS: Transitioned to Fault from Startup");
    } else {
        log::error!(
            "[FAULT SIM] FAILED: Startup -> Fault. Mode: {:?}",
            flight_loop.flight_state.flight_mode
        );
    }

    Timer::after_millis(100).await;

    // 2. Standby -> Fault (Invalid Altimeter)
    log::info!("\n[FAULT SIM] Testing Standby -> Fault");
    flight_loop.flight_state.flight_mode = FlightMode::Standby;
    flight_loop.set_altimeter_state(SensorState::INVALID);
    flight_loop.simulate_cycle().await;

    if flight_loop.flight_state.flight_mode == FlightMode::Fault {
        log::info!("[FAULT SIM] SUCCESS: Transitioned to Fault from Standby");
    } else {
        log::error!(
            "[FAULT SIM] FAILED: Standby -> Fault. Mode: {:?}",
            flight_loop.flight_state.flight_mode
        );
    }
    Timer::after_millis(100).await;

    // 3. Ascent -> Fault (Invalid Altimeter)
    log::info!("\n[FAULT SIM] Testing Ascent -> Fault");
    flight_loop.flight_state.flight_mode = FlightMode::Ascent;
    flight_loop.set_altimeter_state(SensorState::INVALID);
    flight_loop.simulate_cycle().await;

    if flight_loop.flight_state.flight_mode == FlightMode::Fault {
        log::info!("[FAULT SIM] SUCCESS: Transitioned to Fault from Ascent");
    } else {
        log::error!(
            "[FAULT SIM] FAILED: Ascent -> Fault. Mode: {:?}",
            flight_loop.flight_state.flight_mode
        );
    }

    Timer::after_millis(100).await;

    // 4. Coast -> Fault (Invalid Altimeter)
    log::info!("\n[FAULT SIM] Testing Coast -> Fault");
    flight_loop.flight_state.flight_mode = FlightMode::Coast;
    flight_loop.set_altimeter_state(SensorState::INVALID);
    flight_loop.simulate_cycle().await;

    if flight_loop.flight_state.flight_mode == FlightMode::Fault {
        log::info!("[FAULT SIM] SUCCESS: Transitioned to Fault from Coast");
    } else {
        log::error!(
            "[FAULT SIM] FAILED: Coast -> Fault. Mode: {:?}",
            flight_loop.flight_state.flight_mode
        );
    }

    Timer::after_millis(100).await;

    // 5. DrogueDeployed -> Fault (Invalid Altimeter)
    log::info!("\n[FAULT SIM] Testing DrogueDeployed -> Fault");
    flight_loop.flight_state.flight_mode = FlightMode::DrogueDeployed;
    flight_loop.set_altimeter_state(SensorState::INVALID);
    flight_loop.simulate_cycle().await;

    if flight_loop.flight_state.flight_mode == FlightMode::Fault {
        log::info!("[FAULT SIM] SUCCESS: Transitioned to Fault from DrogueDeployed");
    } else {
        log::error!(
            "[FAULT SIM] FAILED: DrogueDeployed -> Fault. Mode: {:?}",
            flight_loop.flight_state.flight_mode
        );
    }

    Timer::after_millis(100).await;

    // 6. MainDeployed -> Fault (Invalid Altimeter)
    log::info!("\n[FAULT SIM] Testing MainDeployed -> Fault");
    flight_loop.flight_state.flight_mode = FlightMode::MainDeployed;
    flight_loop.set_altimeter_state(SensorState::INVALID);
    flight_loop.simulate_cycle().await;

    if flight_loop.flight_state.flight_mode == FlightMode::Fault {
        log::info!("[FAULT SIM] SUCCESS: Transitioned to Fault from MainDeployed");
    } else {
        log::error!(
            "[FAULT SIM] FAILED: MainDeployed -> Fault. Mode: {:?}",
            flight_loop.flight_state.flight_mode
        );
    }

    Timer::after_millis(100).await;

    log::info!("\nFAULT SIMULATION FULLY COMPLETE ");
    Timer::after_millis(1000).await; // Flush logs before halting
}

// Runs stability scenario tests (dwelling in modes and backtracking)
pub async fn simulate_stability_scenarios(flight_loop: &mut FlightLoop) {
    log::info!("\n--- STARTING STABILITY SIMULATION ---");

    let stability_duration = Duration::from_secs(2);

    // 1. Startup Stability
    log::info!("[STABILITY SIM] Testing Startup Stability");
    flight_loop.flight_state.flight_mode = FlightMode::Startup;
    flight_loop.set_altimeter_state(SensorState::VALID);
    flight_loop.set_key_switch(false); // Not armed

    let start = Instant::now();
    while start.elapsed() < stability_duration {
        flight_loop.simulate_cycle().await;
        Timer::after_millis(500).await;
        log::info!("[STABILITY SIM] Remaining in Startup...");
    }

    if flight_loop.flight_state.flight_mode == FlightMode::Startup {
        log::info!("[STABILITY SIM] SUCCESS: Remained in Startup");
    } else {
        log::error!(
            "[STABILITY SIM] FAILED: Drifted from Startup to {:?}",
            flight_loop.flight_state.flight_mode
        );
    }

    // 2. Standby Stability
    log::info!("[STABILITY SIM] Testing Standby Stability");
    flight_loop.set_key_switch(true);
    flight_loop.set_umbilical(true); // Umbilical must be connected to transition to Standby
    flight_loop.set_launch_command(false);
    flight_loop.simulate_cycle().await; // Transition to Standby
    if flight_loop.flight_state.flight_mode != FlightMode::Standby {
        log::error!("[STABILITY SIM] Setup Failed: Could not get to Standby");
        return;
    }

    let start = Instant::now();
    while start.elapsed() < stability_duration {
        flight_loop.simulate_cycle().await;
        Timer::after_millis(500).await;
        log::info!("[STABILITY SIM] Remaining in Standby...");
    }

    if flight_loop.flight_state.flight_mode == FlightMode::Standby {
        log::info!("[STABILITY SIM] SUCCESS: Remained in Standby");
    } else {
        log::error!(
            "[STABILITY SIM] FAILED: Drifted from Standby to {:?}",
            flight_loop.flight_state.flight_mode
        );
    }

    // 3. Standby -> Startup (Backtracking)
    log::info!("[STABILITY SIM] Testing Standby -> Startup (Disarm Key)");
    flight_loop.flight_state.flight_mode = FlightMode::Standby;
    flight_loop.set_key_switch(false);
    flight_loop.simulate_cycle().await;

    if flight_loop.flight_state.flight_mode == FlightMode::Startup {
        log::info!("[STABILITY SIM] SUCCESS: Transitioned Standby -> Startup");
    } else {
        log::error!(
            "[STABILITY SIM] FAILED: Did not go back to Startup. Mode: {:?}",
            flight_loop.flight_state.flight_mode
        );
    }

    // 4. Ascent Stability
    log::info!("[STABILITY SIM] Testing Ascent Stability");
    // Get back to Ascent
    flight_loop.set_key_switch(true);
    flight_loop.simulate_cycle().await; // Standby
    flight_loop.set_umbilical(true);
    flight_loop.set_launch_command(true);
    flight_loop.simulate_cycle().await; // Ascent
    flight_loop.set_umbilical(false); // Disconnect umbilical immediately after launch

    if flight_loop.flight_state.flight_mode != FlightMode::Ascent {
        log::error!("[STABILITY SIM] Setup Failed: Could not get to Ascent");
        return;
    }

    flight_loop.set_altitude(50.0); // Below arming altitude
    flight_loop.set_mav_open(true); // MAV open

    let start = Instant::now();
    while start.elapsed() < stability_duration {
        flight_loop.simulate_cycle().await;
        Timer::after_millis(500).await;
        log::info!("[STABILITY SIM] Remaining in Ascent...");
    }

    if flight_loop.flight_state.flight_mode == FlightMode::Ascent {
        log::info!("[STABILITY SIM] SUCCESS: Remained in Ascent");
    } else {
        log::error!(
            "[STABILITY SIM] FAILED: Drifted from Ascent to {:?}",
            flight_loop.flight_state.flight_mode
        );
    }

    // 5. Coast Stability
    log::info!("[STABILITY SIM] Testing Coast Stability");
    flight_loop.set_altitude(1000.0);
    flight_loop.reset_filter_buffers(); // Reset filters to 1000.0 to prevent jump

    // Force transition to Coast
    flight_loop.flight_state.flight_mode = FlightMode::Coast;
    flight_loop.alt_armed = true;

    let start = Instant::now();
    let mut current_alt = 1000.0;
    while start.elapsed() < stability_duration {
        current_alt += 10.0; // Simulate climbing
        flight_loop.set_altitude(current_alt);
        flight_loop.simulate_cycle().await;
        Timer::after_millis(500).await;
        log::info!(
            "[STABILITY SIM] Remaining in Coast... Alt: {:.2}",
            current_alt
        );
    }

    if flight_loop.flight_state.flight_mode == FlightMode::Coast {
        log::info!("[STABILITY SIM] SUCCESS: Remained in Coast");
    } else {
        log::error!(
            "[STABILITY SIM] FAILED: Drifted from Coast to {:?}",
            flight_loop.flight_state.flight_mode
        );
    }

    log::info!("\n--- STABILITY SIMULATION COMPLETE ---");
}

// Runs tests for extra features: MAV Timer, Mode Change, Payload Comms
pub async fn simulate_extra_features(flight_loop: &mut FlightLoop) {
    log::info!("\n--- STARTING EXTRA FEATURES SIMULATION ---");

    // 1. Test MAV Timeout (Ascent -> Coast)
    log::info!("[EXTRA FEATURE SIM] Testing MAV Timeout");
    // Setup Ascent State
    flight_loop.flight_state.flight_mode = FlightMode::Ascent;
    flight_loop.set_mav_open(true);
    // Simulate time passing
    log::info!(
        "[EXTRA FEATURE SIM] Waiting for MAV Timeout ({}ms)...",
        constants::MAV_OPEN_DURATION_MS
    );
    // Force the timer to start
    // Reset to ensure timer starts
    flight_loop.set_flight_mode(FlightMode::Ascent);
    // The timer is set when `umbilical_launch` is true in `Standby`.

    // Standby -> Ascent transition
    flight_loop.flight_state.flight_mode = FlightMode::Standby;
    flight_loop.set_umbilical(true);
    flight_loop.set_launch_command(true);
    flight_loop.simulate_cycle().await; // Should go to Ascent and start timer
    flight_loop.set_umbilical(false); // Disconnect umbilical immediately after launch

    if flight_loop.flight_state.flight_mode == FlightMode::Ascent && flight_loop.mav_open {
        log::info!("[EXTRA FEATURE SIM] Setup: In Ascent, MAV Open");
        Timer::after_millis(constants::MAV_OPEN_DURATION_MS + 100).await;
        flight_loop.simulate_cycle().await; // Should trigger timeout

        if !flight_loop.mav_open {
            log::info!("[EXTRA FEATURE SIM] SUCCESS: MAV Closed after timeout");
        } else {
            log::error!("[EXTRA FEATURE SIM] FAILED: MAV did not close");
        }

        // Next cycle should transition to Coast
        flight_loop.simulate_cycle().await;
        if flight_loop.flight_state.flight_mode == FlightMode::Coast {
            log::info!("[EXTRA FEATURE SIM] SUCCESS: Transitioned to Coast");
        } else {
            log::error!("[EXTRA FEATURE SIM] FAILED: Did not transition to Coast");
        }
    } else {
        log::error!("[EXTRA FEATURE SIM] Setup Failed: Could not enter Ascent properly");
    }

    Timer::after_millis(100).await;

    // 2. Test Mode Change
    log::info!("[EXTRA FEATURE SIM] Testing Manual Flight Mode Change");
    flight_loop.set_flight_mode(FlightMode::Startup);
    if flight_loop.flight_state.flight_mode == FlightMode::Startup {
        log::info!("[EXTRA FEATURE SIM] SUCCESS: Manually set to Startup");
    } else {
        log::error!("[EXTRA FEATURE SIM] FAILED: Manual set to Startup");
    }

    flight_loop.set_flight_mode(FlightMode::MainDeployed);
    if flight_loop.flight_state.flight_mode == FlightMode::MainDeployed {
        log::info!("[EXTRA FEATURE SIM] SUCCESS: Manually set to MainDeployed");
    } else {
        log::error!("[EXTRA FEATURE SIM] FAILED: Manual set to MainDeployed");
    }

    Timer::after_millis(100).await;

    // 3. Test Payload comms logging
    log::info!("[EXTRA FEATURE SIM] Testing Payload Comms Logging");
    flight_loop.flight_state.payload_comms_ok = false;
    flight_loop.flight_state.recovery_comms_ok = false;
    log::info!("[EXTRA FEATURE SIM] Expecting Comms Failure Logs on next cycle:");
    flight_loop.simulate_cycle().await;

    // Restore
    flight_loop.flight_state.payload_comms_ok = true;
    flight_loop.flight_state.recovery_comms_ok = true;

    Timer::after_millis(100).await;

    log::info!("  EXTRA FEATURES SIMULATION FULLY COMPLETE ");
    Timer::after_millis(1000).await; // Flush logs before stopping
}

// Runs a test for Onboard QSPI Flash storage
pub async fn simulate_flash_storage(flight_loop: &mut FlightLoop) {
    log::info!("\n--- STARTING QSPI FLASH SIMULATION ---");

    // 1. Write simulated data as CSV appends
    log::info!("[FLASH SIM] Appending multiple packets to Flash...");

    for i in 0..5 {
        flight_loop.set_altitude(100.0 * i as f32);
        flight_loop.flight_state.packet.flight_mode = FlightMode::Ascent as u32;
        flight_loop.flight_state.save_packet_to_flash().await;
    }

    Timer::after_millis(100).await;

    log::info!(
        "[FLASH SIM] Verification: Use picotool or a custom script to read the last 2MB of flash to see CSV data."
    );
    log::info!(
        "[FLASH SIM] Header: {}",
        crate::packet::Packet::CSV_HEADER.trim()
    );

    log::info!("       FLASH SIMULATION FULLY COMPLETE     ");
    Timer::after_millis(1000).await;
}

// Reads live hardware sensors and injects TEST_ALTS_LST altitude, then calls
// execute() — the identical code path used in normal flight. Every subsystem
// (launch sequence, actuators, telemetry, flash logging, payload heartbeat)
// runs exactly as it would in a real flight. The only delta from flight is that
// the altimeter reading is replaced by pre-recorded data via sim_altitude_override
// after read_sensors() inside execute().
//
// Flash with:  cargo build --release --features sim_real_flight
// Release: insert key switch, type <L> over USB to launch.
// Debug:   key + umbilical auto-asserted; launch fires after 5 standby cycles.
pub async fn simulate_real_flight(flight_loop: &mut FlightLoop) {
    log::info!("\n--- STARTING REAL FLIGHT SIMULATION (REBOOT RECOVERY MODE) ---");
    log::info!("Altitude injected from TEST_ALTS_LST ({} points).", constants::TEST_ALTS_LST.len());
    log::info!("Each mode dwells 60s — reboot during the dwell to test snapshot recovery.");

    // How long to pause after entering each new flight mode before advancing
    // the altitude profile. 60 s gives plenty of time to power-cycle the board
    // and confirm it recovers to the correct mode.
    const DWELL_MS: u64 = 60_000;

    let mut alt_index: usize = 0;
    let mut sim_standby_cycles: u32 = 0;

    // Sentinel value that won't match any real FlightMode, so the very first
    // loop iteration always triggers the mode-entry banner and starts the dwell.
    let mut last_mode = FlightMode::Fault;
    let mut mode_entry_time: Option<Instant> = None;
    // Altitude held fixed while dwelling so the mode doesn't spuriously advance.
    let mut dwell_alt: f32 = 0.0;

    loop {
        let current_mode = flight_loop.flight_state.flight_mode;

        // ── Mode-entry detection ──────────────────────────────────────────────
        if current_mode != last_mode {
            log::info!("[SIM] ================================================");
            log::info!("[SIM]  ENTERED MODE: {}", flight_loop.flight_state.flight_mode_name());
            log::info!("[SIM]  REBOOT NOW to verify snapshot recovery.");
            log::info!("[SIM]  Dwelling {}s before advancing altitude.", DWELL_MS / 1000);
            log::info!("[SIM] ================================================");
            mode_entry_time = Some(Instant::now());
            // Freeze at the last altitude the profile produced, or 0 at launch.
            dwell_alt = if alt_index == 0 {
                0.0
            } else {
                constants::TEST_ALTS_LST[(alt_index - 1).min(constants::TEST_ALTS_LST.len() - 1)]
            };
            last_mode = current_mode;
        }

        let elapsed_ms = mode_entry_time
            .map(|t| t.elapsed().as_millis())
            .unwrap_or(0);
        let in_dwell = elapsed_ms < DWELL_MS;

        // ── Altitude override ─────────────────────────────────────────────────
        if in_dwell {
            // Hold altitude fixed so the flight-mode logic doesn't auto-advance.
            flight_loop.sim_altitude_override = Some(dwell_alt);

            // Print a countdown every 10 s so the operator knows how long is left.
            if elapsed_ms % 10_000 < 1_100 {
                let secs_left = (DWELL_MS - elapsed_ms) / 1000;
                log::info!(
                    "[SIM] {} | {}s left in dwell | alt held {:.0}m",
                    flight_loop.flight_state.flight_mode_name(),
                    secs_left,
                    dwell_alt
                );
            }
        } else {
            // Advance the altitude profile only while in a flight mode.
            if matches!(
                current_mode,
                FlightMode::Ascent
                    | FlightMode::Coast
                    | FlightMode::DrogueDeployed
                    | FlightMode::MainDeployed
            ) {
                if alt_index < constants::TEST_ALTS_LST.len() {
                    let alt = constants::TEST_ALTS_LST[alt_index];
                    flight_loop.sim_altitude_override = Some(alt);
                    if alt_index % 50 == 0 {
                        log::info!(
                            "[SIM] alt: {:.1}m  idx: {}  mode: {}",
                            alt,
                            alt_index,
                            flight_loop.flight_state.flight_mode_name()
                        );
                    }
                    alt_index += 1;
                } else {
                    log::info!("[SIM] End of TEST_ALTS_LST — simulation complete.");
                    break;
                }
            } else {
                flight_loop.sim_altitude_override = Some(0.0);
            }
        }

        // ── Hardware overrides (run every cycle, debug and release) ───────────
        // Force key and umbilical so no physical hardware is required.
        flight_loop.flight_state.key_armed = true;
        if matches!(current_mode, FlightMode::Startup | FlightMode::Standby) {
            flight_loop.flight_state.umbilical_connected = true;
        } else {
            flight_loop.flight_state.umbilical_connected = false;
        }
        // Launch only fires after the Standby dwell completes, giving the
        // operator time to reboot and verify Standby recovery first.
        if current_mode == FlightMode::Standby && !in_dwell {
            sim_standby_cycles += 1;
            if sim_standby_cycles == 3 {
                log::info!("[SIM] Auto-injecting Launch Command...");
                flight_loop.set_launch_command(true);
            }
        }

        // ── Execute and sleep ─────────────────────────────────────────────────
        flight_loop.execute().await;

        // Slow cycle during dwell (snapshot ring still writes at 1 Hz regardless),
        // normal 20 Hz once the dwell ends and the altitude profile is running.
        Timer::after_millis(if in_dwell { 1_000 } else { constants::MAIN_LOOP_DELAY_MS }).await;
    }

    flight_loop.sim_altitude_override = None;
}

// Runs BLiMS parafoil guidance hardware against real GPS with the L3 Launch 4
// real descent altitude profile (~2000 ft → ~58 ft AGL, 3072 samples at 20 Hz).
//
// Each cycle explicitly:
//   1. Reads live sensors (real GPS lat/lon/heading, IMU)
//   2. Injects descent altitude directly into the packet (no execute() indirection)
//   3. Calls check_transitions() — this is where blims.execute() is called in
//      the MainDeployed branch, which sets the ODrive PWM and moves the motor
//   4. Transmits telemetry and logs to flash
//
// CSV output matches BLIMS/examples/blims_car_test/car_test.rs exactly:
//   lat,lon,target_lat,target_lon,heading,bearing,motor_pos,
//   timestamp_ms,P,I,phase,altitude,loiter_step
//
// Prerequisites before calling:
//   flight_loop.set_blims(blims);
//   flight_loop.set_blims_target(lat, lon);
//
// Flash with:  cargo build --release --features sim_blims
pub async fn simulate_blims_descent(flight_loop: &mut FlightLoop) {
    use blims::sim_data::{DESCENT_ALT_FT, DESCENT_DATA_SIZE};

    log::info!("# ================================================");
    log::info!("# BLiMS Descent Simulation (FSW sim_blims)");
    log::info!("# ================================================");
    log::info!("# Real GPS + L3 Launch 4 descent profile ({} samples @ 20 Hz)", DESCENT_DATA_SIZE);
    log::info!("# BLiMS motor active ~2000 ft -> ~58 ft AGL");
    {
        let p = &flight_loop.flight_state.packet;
        log::info!("# Target:  {:.6}, {:.6}", p.blims_target_lat, p.blims_target_lon);
    }
    log::info!("# CSV: lat,lon,target_lat,target_lon,heading,bearing,motor_pos,timestamp_ms,P,I,phase,altitude,loiter_step");
    log::info!("# ================================================");
    log::info!("# Waiting for GPS fix before starting descent...");

    // Poll until we have a real GPS fix, then set the target from that position.
    let mut last_no_fix_log = Instant::now();
    loop {
        flight_loop.flight_state.read_sensors().await;
        // Copy fields out before dropping the borrow so we can call set_blims_target.
        let fix_type = flight_loop.flight_state.packet.fix_type;
        let fix_lat  = flight_loop.flight_state.packet.latitude;
        let fix_lon  = flight_loop.flight_state.packet.longitude;
        if fix_type >= 2 {
            let target_lat = fix_lat + constants::BLIMS_SIM_TARGET_LAT_OFFSET_DEG;
            let target_lon = fix_lon + constants::BLIMS_SIM_TARGET_LON_OFFSET_DEG;
            flight_loop.set_blims_target(target_lat, target_lon);
            log::info!("# GPS FIX: lat={:.6} lon={:.6} (type={})", fix_lat, fix_lon, fix_type);
            log::info!("# TARGET:  lat={:.6} lon={:.6}  (offset {:.3},{:.3} deg)",
                target_lat, target_lon,
                constants::BLIMS_SIM_TARGET_LAT_OFFSET_DEG,
                constants::BLIMS_SIM_TARGET_LON_OFFSET_DEG,
            );
            log::info!("# DESCENT STARTING -- {} samples @ 20 Hz", DESCENT_DATA_SIZE);
            break;
        }
        if last_no_fix_log.elapsed().as_millis() >= 5_000 {
            log::info!("# No fix yet (type={}) -- waiting...", fix_type);
            last_no_fix_log = Instant::now();
        }
        Timer::after_millis(200).await;
    }

    // Force into MainDeployed for the whole run so check_transitions always
    // reaches the BLiMS branch.
    flight_loop.set_flight_mode(FlightMode::MainDeployed);
    flight_loop.main_chutes_deployed = true;

    let mut last_phase_id: i8 = -1;

    for (i, &alt_ft) in DESCENT_ALT_FT.iter().enumerate() {
        let cycle_start = Instant::now();

        // 1. Read real hardware: GPS provides live lat/lon/heading/velocity for BLiMS.
        flight_loop.flight_state.read_sensors().await;

        let fix_type = flight_loop.flight_state.packet.fix_type;
        let gps_valid = fix_type >= 2;

        // 2. Inject simulated altitude directly — feet → meters, force VALID so
        //    check_transitions doesn't abort to Fault on a missing barometer.
        flight_loop.flight_state.packet.altitude = alt_ft / 3.28084_f32;
        flight_loop.flight_state.altimeter_state = SensorState::VALID;

        // 3. Run check_transitions. In MainDeployed mode this calls blims.execute()
        //    which computes the motor command and writes it to the ODrive PWM pin.
        flight_loop.check_transitions().await;

        // 4. Transmit telemetry over radio + USB and write to flash.
        flight_loop.flight_state.transmit().await;
        flight_loop.flight_state.save_packet_to_flash().await;

        // 5. Log — matches car_test.rs output format.
        {
            let p = &flight_loop.flight_state.packet;

            // Phase-change banner
            if p.blims_phase_id != last_phase_id {
                log::info!(
                    "# PHASE: {} (alt={:.1} ft, sample {}/{})",
                    p.blims_phase_id,
                    alt_ft,
                    i,
                    DESCENT_DATA_SIZE,
                );
                last_phase_id = p.blims_phase_id;
            }

            if gps_valid {
                let heading_deg = p.head_mot as f32 * 1e-5_f32;
                let now_ms = Instant::now().as_millis();
                // CSV: lat,lon,target_lat,target_lon,heading,bearing,motor_pos,
                //      timestamp_ms,P,I,phase,altitude,loiter_step
                log::info!(
                    "{:.7},{:.7},{:.6},{:.6},{:.5},{:.5},{:.4},{},{:.6},{:.6},{},{:.2},{}",
                    p.latitude,
                    p.longitude,
                    p.blims_target_lat,
                    p.blims_target_lon,
                    heading_deg,
                    p.blims_bearing,
                    p.blims_motor_position,
                    now_ms,
                    p.blims_pid_p,
                    p.blims_pid_i,
                    p.blims_phase_id,
                    alt_ft,
                    p.blims_loiter_step,
                );
            } else {
                log::info!("# No fix (type={})", fix_type);
            }
        }

        // 6. Cycle timing — sleep only remaining time, warn on overrun.
        let elapsed_ms = cycle_start.elapsed().as_millis();
        if elapsed_ms < constants::MAIN_LOOP_DELAY_MS {
            Timer::after_millis(constants::MAIN_LOOP_DELAY_MS - elapsed_ms).await;
        } else {
            log::warn!("# WARN: cycle overrun {} ms", elapsed_ms);
        }
    }

    log::info!("# ================================================");
    log::info!("# BLiMS DESCENT SIMULATION COMPLETE ({} cycles)", DESCENT_DATA_SIZE);
    log::info!("# ================================================");
}

// Verifies the exact 4-stage launch sequence and apogee override
pub async fn simulate_launch_sequence(flight_loop: &mut FlightLoop) {
    log::info!("\n--- STARTING LAUNCH SEQUENCE SIMULATION ---");

    // 1. Setup Standby State
    flight_loop.flight_state.flight_mode = FlightMode::Standby;
    flight_loop.set_key_switch(true);
    flight_loop.set_umbilical(true);
    flight_loop.set_altimeter_state(SensorState::VALID);
    flight_loop.set_altitude(0.0);
    flight_loop.simulate_cycle().await;

    // 2. Trigger Launch
    log::info!("[LAUNCH SIM] Sending Launch Command...");
    flight_loop.set_launch_command(true);
    flight_loop.simulate_cycle().await;
    flight_loop.set_umbilical(false); // Disconnect immediately to avoid Ascent fault

    // --- STAGE 1: PreVent (2s) ---
    if flight_loop.launch_sequence_stage == LaunchStage::PreVent && flight_loop.sv_open {
        log::info!("[LAUNCH SIM] SUCCESS: Stage 1 (PreVent) active, SV Open.");
    } else {
        log::error!(
            "[LAUNCH SIM] FAILED: Stage 1 not active. Stage: {:?}, SV: {}",
            flight_loop.launch_sequence_stage,
            flight_loop.sv_open
        );
    }
    Timer::after_millis(constants::LAUNCH_SV_PREVENT_MS + 100).await;
    flight_loop.simulate_cycle().await;

    // --- STAGE 2: MavOpen (7.88s) ---
    if flight_loop.launch_sequence_stage == LaunchStage::MavOpen
        && flight_loop.mav_open
        && !flight_loop.sv_open
    {
        log::info!("[LAUNCH SIM] SUCCESS: Stage 2 (MavOpen) active, MAV Open, SV Closed.");
    } else {
        log::error!(
            "[LAUNCH SIM] FAILED: Stage 2 not active. Stage: {:?}, MAV: {}, SV: {}",
            flight_loop.launch_sequence_stage,
            flight_loop.mav_open,
            flight_loop.sv_open
        );
    }
    Timer::after_millis(constants::MAV_OPEN_DURATION_MS + 100).await;
    flight_loop.simulate_cycle().await;

    // --- STAGE 3: Done (sequence complete, MAV and SV both closed) ---
    if flight_loop.launch_sequence_stage == LaunchStage::Done
        && !flight_loop.mav_open
        && !flight_loop.sv_open
    {
        log::info!("[LAUNCH SIM] SUCCESS: Stage 3 (Done) active, MAV Closed, SV Closed.");
        if flight_loop.flight_state.flight_mode == FlightMode::Coast {
            log::info!("[LAUNCH SIM] SUCCESS: Transitioned to Coast during wait.");
        }
    } else {
        log::error!(
            "[LAUNCH SIM] FAILED: Stage 3 not active. Stage: {:?}, MAV: {}, SV: {}",
            flight_loop.launch_sequence_stage,
            flight_loop.mav_open,
            flight_loop.sv_open
        );
    }

    // --- TEST APOGEE OVERRIDE ---
    log::info!("[LAUNCH SIM] Testing Apogee Override during PostWait...");

    // Saturation: Run 10 cycles at 100m to fill the buffer
    flight_loop.set_altitude(100.0);
    for _ in 0..11 {
        flight_loop.simulate_cycle().await;
    }

    // Drop 1: Shift the average down
    flight_loop.set_altitude(95.0);
    for _ in 0..11 {
        flight_loop.simulate_cycle().await;
    }

    // Drop 2: Apogee detection requires filtered_alt[2] > filtered_alt[1] > filtered_alt[0]
    flight_loop.set_altitude(90.0);
    for _ in 0..11 {
        flight_loop.simulate_cycle().await;
    }

    // FinalVent no longer exists as a LaunchStage — SV now reopens via the one-shot
    // recovery vent in check_transitions when entering DrogueDeployed/MainDeployed/Fault.
    if flight_loop.flight_state.flight_mode == FlightMode::DrogueDeployed && flight_loop.sv_open {
        log::info!("[LAUNCH SIM] SUCCESS: Apogee triggered DrogueDeployed and recovery vent opened SV.");
    } else {
        log::error!(
            "[LAUNCH SIM] FAILED: Apogee did not open SV. Mode: {:?}, SV: {}",
            flight_loop.flight_state.flight_mode,
            flight_loop.sv_open
        );
    }

    log::info!("\n--- LAUNCH SEQUENCE SIMULATION COMPLETE ---");
}

// Simulation for Payload Specialized Ground Commands
#[cfg(feature = "sim_payload")]
pub async fn simulate_payload_commands(flight_loop: &mut FlightLoop) {
    log::info!("\n--- STARTING PAYLOAD COMMAND SIMULATION ---");

    // 1. Test Command::N1 in Startup
    log::info!("[SIM] Testing N1 command in Startup mode...");
    flight_loop.set_flight_mode(FlightMode::Startup);
    flight_loop.flight_state.sim_radio_command = Some(crate::packet::Command::N1);
    flight_loop.simulate_cycle().await;
    // (Manual check: logs should show "PAYLOAD: Sent N1")

    // 2. Test Command::N2
    log::info!("[SIM] Testing N2 command...");
    flight_loop.flight_state.sim_radio_command = Some(crate::packet::Command::N2);
    flight_loop.simulate_cycle().await;
    // (Manual check: logs should show "PAYLOAD: Sent N2")

    // 3. Test Command::N3 (Altitude Timeout)
    log::info!("[SIM] Testing N3 command (Altitude Trigger < 250m)...");
    flight_loop.set_altitude(200.0);
    flight_loop.flight_state.sim_radio_command = Some(crate::packet::Command::N3);
    
    // Cycle 1: Start timer
    flight_loop.simulate_cycle().await;
    log::info!("[SIM] N3 Cycle 1 (Timer started)");
    
    // Cycle 2: Wait > 1s
    Timer::after_millis(1100).await;
    flight_loop.flight_state.sim_radio_command = Some(crate::packet::Command::N3);
    flight_loop.simulate_cycle().await;
    log::info!("[SIM] N3 Cycle 2 (Triggered)");
    // (Manual check: logs should show "Low Altitude Detected (>1s). Sending N3.")

    // 4. Test Command::N4 (Acceleration Trigger)
    log::info!("[SIM] Testing N4 command (Acceleration Trigger > 30m/s^2)...");
    flight_loop.flight_state.packet.accel_x = 35.0;
    flight_loop.flight_state.sim_radio_command = Some(crate::packet::Command::N4);
    flight_loop.simulate_cycle().await;
    // (Manual check: logs should show "High Dynamics Detected. Sending N4.")

    // 5. Test ForceMode
    log::info!("[SIM] Testing ForceMode (Startup -> DrogueDeployed)...");
    flight_loop.flight_state.sim_radio_command = Some(crate::packet::Command::ForceMode(FlightMode::DrogueDeployed as u32));
    flight_loop.simulate_cycle().await;
    
    if flight_loop.flight_state.flight_mode == FlightMode::DrogueDeployed {
        log::info!("[SIM] SUCCESS: ForceMode effective.");
    } else {
        log::error!("[SIM] FAILED: ForceMode did not change flight mode to DrogueDeployed. Current: {:?}", flight_loop.flight_state.flight_mode);
    }

    // 6. Test Umbilical-based Payload Commands
    log::info!("[SIM] Testing Umbilical-based Payload Commands...");
    use crate::umbilical::{self, UmbilicalCommand};
    
    log::info!("[SIM] Sending Umbilical N1...");
    flight_loop.set_flight_mode(FlightMode::Startup);
    umbilical::push_command(UmbilicalCommand::PayloadN1);
    flight_loop.simulate_cycle().await;
    // (Logs confirm "UMBILICAL: Sent N1")

    log::info!("[SIM] Sending Umbilical N2...");
    umbilical::push_command(UmbilicalCommand::PayloadN2);
    flight_loop.simulate_cycle().await;
    // (Logs confirm "UMBILICAL: Sent N2")

    log::info!("[SIM] Sending Umbilical N3...");
    umbilical::push_command(UmbilicalCommand::PayloadN3);
    flight_loop.simulate_cycle().await;
    // (Logs confirm "UMBILICAL: Sent N3")

    log::info!("[SIM] Sending Umbilical N4...");
    umbilical::push_command(UmbilicalCommand::PayloadN4);
    flight_loop.simulate_cycle().await;
    // (Logs confirm "UMBILICAL: Sent N4")

    log::info!("--- PAYLOAD COMMAND SIMULATION COMPLETE ---\n");
}
