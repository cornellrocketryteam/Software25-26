use crate::flight_loop::FlightLoop;
use crate::state::{FlightMode, SensorState};
use crate::constants::{self, TEST_ALTS_LST};
use embassy_time::{Timer, Instant, Duration};
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
        log::error!("\n[SIM] FAILED: Did not transition to Standby. Mode: {:?}", flight_loop.flight_state.flight_mode);
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
        log::info!("[SIM] MAV Open: {}, SV Open: {}", flight_loop.mav_open, flight_loop.sv_open);
    } else {
        log::error!("[SIM] FAILED: Did not transition to Ascent\n");
    }

    log::info!("[SIM] Starting Flight Profile with {} altitude points...", constants::TEST_ALTS_LST.len());
    
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

    for (_i,alt) in TEST_ALTS_LST.iter().enumerate() {
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
            if flight_loop.camera_deployed && !flight_loop.airbrakes_init && !drogue_deployed_verified {
                log::info!("\n[SIM] SUCCESS: Transitioned to DrogueDeployed at {:.2}m", alt);
                drogue_deployed_verified = true;
            }
        }

        // MAIN Checks
        if mode == FlightMode::MainDeployed {
            if !main_deployed_verified {
                log::info!("\n[SIM] SUCCESS: Transitioned to MainDeployed at {:.2}m", alt);
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
    log::info!("[FAULT SIM] Testing Startup -> Fault");
    // Reset state
    flight_loop.flight_state.flight_mode = FlightMode::Startup;
    flight_loop.set_key_switch(true);
    flight_loop.set_altimeter_state(SensorState::INVALID);
    flight_loop.simulate_cycle().await;

    if flight_loop.flight_state.flight_mode == FlightMode::Fault {
        log::info!("[FAULT SIM] SUCCESS: Transitioned to Fault from Startup");
    } else {
        log::error!("[FAULT SIM] FAILED: Startup -> Fault. Mode: {:?}", flight_loop.flight_state.flight_mode);
    }

    // 2. Ascent -> Fault (Invalid Altimeter)
    log::info!("[FAULT SIM] Testing Ascent -> Fault");
    flight_loop.flight_state.flight_mode = FlightMode::Ascent;
    flight_loop.set_altimeter_state(SensorState::INVALID);
    flight_loop.simulate_cycle().await;

    if flight_loop.flight_state.flight_mode == FlightMode::Fault {
        log::info!("[FAULT SIM] SUCCESS: Transitioned to Fault from Ascent");
    } else {
        log::error!("[FAULT SIM] FAILED: Ascent -> Fault. Mode: {:?}", flight_loop.flight_state.flight_mode);
    }
    
    // 3. Coast -> Fault (Invalid Altimeter)
    log::info!("[FAULT SIM] Testing Coast -> Fault");
    flight_loop.flight_state.flight_mode = FlightMode::Coast;
    flight_loop.set_altimeter_state(SensorState::INVALID);
    flight_loop.simulate_cycle().await;

    if flight_loop.flight_state.flight_mode == FlightMode::Fault {
        log::info!("[FAULT SIM] SUCCESS: Transitioned to Fault from Coast");
    } else {
        log::error!("[FAULT SIM] FAILED: Coast -> Fault. Mode: {:?}", flight_loop.flight_state.flight_mode);
    }
    
    // 4. DrogueDeployed -> Fault (Invalid Altimeter)
    log::info!("[FAULT SIM] Testing DrogueDeployed -> Fault");
    flight_loop.flight_state.flight_mode = FlightMode::DrogueDeployed;
    flight_loop.set_altimeter_state(SensorState::INVALID);
    flight_loop.simulate_cycle().await;

    if flight_loop.flight_state.flight_mode == FlightMode::Fault {
        log::info!("[FAULT SIM] SUCCESS: Transitioned to Fault from DrogueDeployed");
    } else {
        log::error!("[FAULT SIM] FAILED: DrogueDeployed -> Fault. Mode: {:?}", flight_loop.flight_state.flight_mode);
    }

    // 5. MainDeployed -> Fault (Invalid Altimeter)
    log::info!("[FAULT SIM] Testing MainDeployed -> Fault");
    flight_loop.flight_state.flight_mode = FlightMode::MainDeployed;
    flight_loop.set_altimeter_state(SensorState::INVALID);
    flight_loop.simulate_cycle().await;

    if flight_loop.flight_state.flight_mode == FlightMode::Fault {
        log::info!("[FAULT SIM] SUCCESS: Transitioned to Fault from MainDeployed");
    } else {
        log::error!("[FAULT SIM] FAILED: MainDeployed -> Fault. Mode: {:?}", flight_loop.flight_state.flight_mode);
    }

    log::info!("\n--- FAULT SIMULATION COMPLETE ---");
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
         log::error!("[STABILITY SIM] FAILED: Drifted from Startup to {:?}", flight_loop.flight_state.flight_mode);
    }

    // 2. Standby Stability
    log::info!("[STABILITY SIM] Testing Standby Stability");
    flight_loop.set_key_switch(true);
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
         log::error!("[STABILITY SIM] FAILED: Drifted from Standby to {:?}", flight_loop.flight_state.flight_mode);
    }

    // 3. Standby -> Startup (Backtracking)
    log::info!("[STABILITY SIM] Testing Standby -> Startup (Disarm Key)");
    flight_loop.flight_state.flight_mode = FlightMode::Standby;
    flight_loop.set_key_switch(false);
    flight_loop.simulate_cycle().await;
    
    if flight_loop.flight_state.flight_mode == FlightMode::Startup {
        log::info!("[STABILITY SIM] SUCCESS: Transitioned Standby -> Startup");
    } else {
        log::error!("[STABILITY SIM] FAILED: Did not go back to Startup. Mode: {:?}", flight_loop.flight_state.flight_mode);
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
        log::error!("[STABILITY SIM] FAILED: Drifted from Ascent to {:?}", flight_loop.flight_state.flight_mode);
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
        log::info!("[STABILITY SIM] Remaining in Coast... Alt: {:.2}", current_alt);
    }
    
    if flight_loop.flight_state.flight_mode == FlightMode::Coast {
        log::info!("[STABILITY SIM] SUCCESS: Remained in Coast");
    } else {
        log::error!("[STABILITY SIM] FAILED: Drifted from Coast to {:?}", flight_loop.flight_state.flight_mode);
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
    log::info!("[EXTRA FEATURE SIM] Waiting for MAV Timeout ({}ms)...", constants::MAV_OPEN_DURATION_MS);
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

    // 3. Test Payload comms logging
    log::info!("[EXTRA FEATURE SIM] Testing Payload Comms Logging");
    flight_loop.flight_state.payload_comms_ok = false;
    flight_loop.flight_state.recovery_comms_ok = false;
    log::info!("[EXTRA FEATURE SIM] Expecting Comms Failure Logs on next cycle:");
    flight_loop.simulate_cycle().await;
    
    // Restore
    flight_loop.flight_state.payload_comms_ok = true;
    flight_loop.flight_state.recovery_comms_ok = true;

    log::info!("\n--- EXTRA FEATURES SIMULATION COMPLETE ---");
}

// Runs a Hardware physical simulation.
// This executes the actual hardware `execute()` loop, 
// processes real USB umbilical commands (`<L>`, `<M>`, etc.), and toggles actuators
// it overwrites the altimeter sensor data with the `TEST_ALTS_LST` array
// to test the entire physical breadboard setup
pub async fn simulate_flight_hsim(flight_loop: &mut FlightLoop) {
    log::info!("\n--- STARTING HARDWARE SIMULATION ---");
    log::info!("Insert Key Switch and Type <L> in Serial Monitor to Launch");

    flight_loop.set_altimeter_state(SensorState::VALID);
    let mut alt_index = 0;
    
    #[cfg(debug_assertions)]
    let mut debug_standby_cycles = 0;
    
    loop {
        // 1. Read sensors (this will flag altimeter as INVALID if missing)
        flight_loop.flight_state.read_sensors().await;
        
        // FOR SIMULATION ONLY: Force the altimeter state back to VALID so the flight controller
        // doesn't immediately abort into Fault mode when the physical sensor is unplugged.
        flight_loop.set_altimeter_state(SensorState::VALID);
        
        // 2. OVERWRITE the altitude sensor data before transition logic runs
        // Start feeding altimeter data once the rocket enters Ascent mode
        // type <L> in the serial console to launch
        if flight_loop.flight_state.flight_mode == FlightMode::Ascent 
            || flight_loop.flight_state.flight_mode == FlightMode::Coast 
            || flight_loop.flight_state.flight_mode == FlightMode::DrogueDeployed 
            || flight_loop.flight_state.flight_mode == FlightMode::MainDeployed 
        {
            if alt_index < constants::TEST_ALTS_LST.len() {
                // OVERWRITE the real altimeter readings
                flight_loop.set_altitude(constants::TEST_ALTS_LST[alt_index]);
                
                log::info!("[H SIM] Flying at Simulated Alt: {:.2}m", constants::TEST_ALTS_LST[alt_index]);
                alt_index += 1;
            } else {
                log::info!("\n[H SIM] Reached end of simulated altitude array");
                break;
            }
        } else {
            // While waiting on the launch pad in Startup/Standby, just keep it at 0m
            flight_loop.set_altitude(0.0);
        }

        // 3. Process the rest of the hardware loop logic
        flight_loop.flight_state.check_subsystem_health().await;
        
        // OVERRIDE for DEBUG builds.
        // In debug mode, the USB is used for logging, not the umbilical interface.
        // Also, we likely don't have the physical key switch plugged into the debugger.
        // We force these true so the state machine can transition from Startup -> Standby.
        #[cfg(debug_assertions)]
        {
            flight_loop.flight_state.key_armed = true;
            
            // Only force umbilical connected while on the pad. 
            // Disconnect it instantly when we launch so we don't fault in Ascent.
            if flight_loop.flight_state.flight_mode == FlightMode::Startup || flight_loop.flight_state.flight_mode == FlightMode::Standby {
                flight_loop.flight_state.umbilical_connected = true;
                Timer::after_millis(2000).await;
            } else {
                flight_loop.flight_state.umbilical_connected = false;
            }
            
            // Automatically launch after 5 cycles in Standby since USB umbilical is unavailable in debug
            if flight_loop.flight_state.flight_mode == FlightMode::Standby {
                debug_standby_cycles += 1;
                if debug_standby_cycles == 5 {
                    log::info!("[H SIM DEBUG] Auto-injecting Launch Command...");
                    flight_loop.set_launch_command(true);
                }
            }
        }

        flight_loop.key_armed = flight_loop.flight_state.key_armed;
        flight_loop.umbilical_state = flight_loop.flight_state.umbilical_connected;

        flight_loop.check_ground_commands().await;
        flight_loop.check_umbilical_commands().await;
        flight_loop.check_transitions().await;
        flight_loop.flight_state.transmit().await;

        log::info!(
            "Current Flight Mode: {} on cycle {} \n",
            flight_loop.flight_state.flight_mode_name(),
            flight_loop.flight_state.cycle_count
        );
        flight_loop.flight_state.cycle_count += 1;

        // Delay to match the real timing cycle
        Timer::after_millis(constants::MAIN_LOOP_DELAY_MS).await;
    }
}

// -------------------------------------------------------------
// INDIVIDUAL ACTUATOR HARDWARE SIMULATIONS
// These bypass the flight logic entirely, testing physical pins
// -------------------------------------------------------------

/// Tests the physical Main Actuation Valve (MAV) pin by opening and closing it.
pub async fn simulate_hsim_mav(flight_loop: &mut FlightLoop) {
    log::info!("\n--- STARTING HSIM: MAV VALVE TEST ---");
    log::info!("This simulation will repeatedly open MAV, wait, and close it.");
    
    loop {
        log::info!("[HSIM] Actuating MAV OPEN for {}ms...", constants::MAV_OPEN_DURATION_MS);
        // Should be ~1.08 V
        flight_loop.flight_state.open_mav(constants::MAV_OPEN_DURATION_MS).await;
        
        // Wait long enough to observe it open + a buffer
        Timer::after_millis(constants::MAV_OPEN_DURATION_MS + 2000).await;
        
        log::info!("[HSIM] Actuating MAV CLOSE...");
        // Should be ~2.17 V
        flight_loop.flight_state.close_mav().await;
        
        log::info!("[HSIM] Waiting 5 seconds before next cycle...");
        Timer::after_millis(5000).await;
    }
}

/// Tests the physical Solenoid Valve (SV) pin by opening and closing it.
pub async fn simulate_hsim_sv(flight_loop: &mut FlightLoop) {
    log::info!("\n--- STARTING HSIM: SV VALVE TEST ---");
    log::info!("This simulation will repeatedly open SV, wait, and close it.");
    
    loop {
        // SV doesn't have a specific duration constant yet, using 2 seconds
        let sv_test_duration = 2000;
        
        log::info!("[HSIM] Actuating SV OPEN for {}ms...", sv_test_duration);
        flight_loop.flight_state.open_sv(sv_test_duration).await;
        
        Timer::after_millis(sv_test_duration + 2000).await;
        
        log::info!("[HSIM] Actuating SV CLOSE...");
        flight_loop.flight_state.close_sv().await;
        
        log::info!("[HSIM] Waiting 5 seconds before next cycle...");
        Timer::after_millis(5000).await;
    }
}

/// Tests the physical Drogue parachute (SSA) pin.
pub async fn simulate_hsim_drogue(flight_loop: &mut FlightLoop) {
    log::info!("\n--- STARTING HSIM: DROGUE CHUTE TEST ---");
    log::info!("This simulation will fire the drogue ssa.");
    
    log::info!("[HSIM] Firing Drogue for {}ms...", constants::SSA_THRESHOLD_MS);
    flight_loop.flight_state.trigger_drogue().await;
    
    // Let the flight state update the actuators to actually hit the pin
    flight_loop.flight_state.update_actuators().await;
    
    Timer::after_millis(constants::SSA_THRESHOLD_MS).await;
    
    log::info!("[HSIM] Drogue deploy complete. Waiting 5 seconds...");
    
    // Make sure we spin the update actuator loop while waiting so pins settle
    for _ in 0..50 {
        flight_loop.flight_state.update_actuators().await;
        Timer::after_millis(100).await;
    }
}

/// Tests the physical Main parachute ematch (SSA) pin.
pub async fn simulate_hsim_main(flight_loop: &mut FlightLoop) {
    log::info!("\n--- STARTING HSIM: MAIN CHUTE TEST ---");
    log::info!("This simulation will repeatedly fire the main ssa.");
    
    loop {
        log::info!("[HSIM] Firing Main for {}ms...", constants::SSA_THRESHOLD_MS);
        flight_loop.flight_state.trigger_main().await;
        
        flight_loop.flight_state.update_actuators().await;
        
        Timer::after_millis(constants::SSA_THRESHOLD_MS).await;
        
        log::info!("[HSIM] Main deploy complete. Waiting 5 seconds...");
        
        for _ in 0..50 {
            flight_loop.flight_state.update_actuators().await;
            Timer::after_millis(100).await;
        }
    }
}
/*
pub async fn simulate_hsim_buzzer(flight_loop: &mut FlightLoop) {
    log::info
}
*/
