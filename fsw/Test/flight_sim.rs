use crate::flight_loop::FlightLoop;
use crate::state::{FlightMode, SensorState};
use crate::constants;
use embassy_time::Timer;

/// Runs a full flight simulation (Scenario 1 of just simple transitions between each state) on the given FlightLoop object.
/// This verifies logic transitions without needing real hardware inputs from the sensor modules.
pub async fn simulate_flight_s1(flight_loop: &mut FlightLoop) {
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
    flight_loop.set_key_switch(true);
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
    let mut main_deployed_verified = false;

    // We only need to check umbilical disconnect once
    flight_loop.set_umbilical(false);

    let altitudes: [f32; 20] = [0.0, 100.0, 189.0, 311.0, 420.0, 732.0, 864.1, 1029.4, 1413.9, 1692.1, 1999.9, 2209.9, 2509.9, 2900.9, 2618.8, 2163.1, 1300.0, 949.0, 400.0, 0.0];

    for (_i,alt) in altitudes.iter().enumerate() {
        flight_loop.set_altitude(*alt);
        
        // Run logic for this altitude
        for _ in 0..=constants::ALT_SAMPLE_INTERVAL {
             flight_loop.simulate_cycle().await;
        }

        let mode = flight_loop.flight_state.flight_mode;

        // ASCENT Checks
        if mode == FlightMode::Ascent {
             if flight_loop.alt_armed && *alt > constants::ARMING_ALTITUDE {
                 // Altimeter arming verification happens continuously
             }
             
             // Simulate MAV Close when passing 1000m (Ascent -> Coast)
             if !mav_close_simulated {
                 log::info!("\n[SIM] Simulating MAV/SV Close at {:.2}m", alt);
                 flight_loop.set_mav_open(false);
                 flight_loop.set_sv_open(false);
                 mav_close_simulated = true;
                 
                 // The next cycle check_transitions move to Coast
                 flight_loop.simulate_cycle().await;
                 if flight_loop.flight_state.flight_mode == FlightMode::Coast {
                     log::info!("[SIM] SUCCESS: Transitioned to Coast");
                     // Trigger airbrakes
                     flight_loop.set_airbrakes(true);
                 }
             }
        }
        
        // COAST Checks (Apogee detection is automatic)
        if mode == FlightMode::Coast {
            flight_loop.set_cameras_deployed(true);
            flight_loop.set_airbrakes(false);
            flight_loop.simulate_cycle().await;

        }
        
        // DROGUE Checks

        if mode == FlightMode::DrogueDeployed {
             // Verify side effects once
            if flight_loop.camera_deployed && !flight_loop.airbrakes_init {
                log::info!("\n[SIM] SUCCESS: Transitioned to DrogueDeployed at {:.2}m", alt);
            }
            flight_loop.simulate_cycle().await;
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
    log::info!("Final Mode: {:?}", flight_loop.flight_state.flight_mode);
}
