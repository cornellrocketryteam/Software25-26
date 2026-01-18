use crate::state::{FlightMode, FlightState};
use crate::constants;
use crate::state::SensorState;

// TODO: Remove some bools and edit FlightLoop to be able to trigger events with methods 
// TODO: Remove some bools and edit FlightLoop to be able to trigger events with methods 
// ex: a function to say that the umbilical is connected, or umbilical launch, etc.
pub struct FlightLoop {
    pub flight_state: FlightState,
    pub key_armed: bool,
    pub alt_armed: bool,
    pub umbilical_state: bool,
    pub umbilical_launch: bool,
    pub mav_open: bool,
    pub sv_open: bool,
    pub camera_deployed: bool,
    pub alt_sum: f32,
    pub airbrakes_init: bool,
    pub drogue_deployed: bool,
    pub main_chutes_deployed: bool,
    pub blims_armed: bool,
    pub log_armed: bool,
    
    // Internal state tracking
    alt_buffer: [f32; 10],
    alt_index: usize,
    filtered_alt: [f32; 3],
    main_cycle_count: u32,
    log_cycle_count: u32,
}

impl FlightLoop {
    pub fn new(flight_state: FlightState) -> Self {
        Self {
            flight_state,
            key_armed: false,
            alt_armed: false,
            umbilical_state: false,
            umbilical_launch: false,
            mav_open: false,
            sv_open: false,
            camera_deployed: false,
            alt_sum: 0.0,
            airbrakes_init: false,
            drogue_deployed: false,
            main_chutes_deployed: false,
            blims_armed: false,
            log_armed: false,
            
            // Initialize internal state
            alt_buffer: [0.0; 10],
            alt_index: 0,
            filtered_alt: [-1.0; 3],
            main_cycle_count: 0,
            log_cycle_count: 0,
        }
    }

    pub async fn execute(&mut self) {
        self.flight_state.read_sensors().await;
        
        // Update local loop state from FlightState
        self.key_armed = self.flight_state.key_armed;
        
        // Placeholder: Check umbilical state here (e.g. read another GPIO)
        self.umbilical_state = self.flight_state.umbilical_connected;

        self.check_transitions().await;
        self.flight_state.transmit().await;
        
        // Log current state
        log::info!(
            "Current Flight Mode: {} on cycle {}",
            self.flight_state.flight_mode_name(),
            self.flight_state.cycle_count
        );
    }

    pub async fn check_transitions(&mut self) {
        // Retrieve current values for easier access
        let _packet = &self.flight_state.packet;
        let _mode = self.flight_state.flight_mode;

        // Transition logic
        match self.flight_state.flight_mode {
            FlightMode::Startup => {
                if self.flight_state.altimeter_state == crate::state::SensorState::VALID {
                    // Update reference pressure before launch
                    self.flight_state.reference_pressure = self.flight_state.read_barometer();
                    log::info!("Reference pressure set to {}", self.flight_state.reference_pressure);
                }
                // TODO: add umbilical check command logic (no need to fault, use this to know to connect umbilical)
                // TODO: add buzzer logic
                if self.flight_state.umbilical_connected {
                    // For now just log the state for umbilical
                    log::info!("Umbilical connected");
                    //buzzer.buzz_num_times(2);
                } else {
                    log::info!("Umbilical disconnected");
                    //buzzer.buzz_num_times(3);
                }
                
                if self.key_armed {
                    if self.flight_state.altimeter_state == crate::state::SensorState::VALID {
                        // Record arming altitude (TODO: implement into storage)
                        self.alt_armed = true;
                        self.flight_state.arming_altitude = self.flight_state.read_altimeter();
                        log::info!("Arming altitude set to {}", self.flight_state.arming_altitude);
                        self.flight_state.flight_mode = FlightMode::Standby;
                        log::info!("Transitioning to Standby");
                    } else if self.flight_state.altimeter_state == crate::state::SensorState::INVALID {
                        self.alt_armed = false;
                        self.flight_state.flight_mode = FlightMode::Fault;
                        log::error!("Altimeter invalid at Startup; transitioning to Fault");
                    }
                }
                
            }
            FlightMode::Standby => {
                if self.flight_state.altimeter_state != crate::state::SensorState::VALID {
                    // altimeter is not working
                    self.alt_armed = false;
                    self.flight_state.flight_mode = FlightMode::Fault;
                        log::error!("Altimeter invalid at Standby; transitioning to Fault");
                }
                // TODO: add umbilical check command logic with written class (no need to fault, use this to know to connect umbilical)
                // TODO: add buzzer logic
                if self.flight_state.umbilical_connected {
                    // For now just log the state for umbilical
                    log::info!("Umbilical connected");
                    //buzzer.buzz_num_times(2);
                } else {
                    log::info!("Umbilical disconnected");
                    //buzzer.buzz_num_times(3);
                }

                // Check accelerometer/altimeter for launch detection 
                if self.umbilical_launch {
                    // TODO: Send command for launch
                    // send_launch_command();
                    log::info!("Payload launch command sent");
                    //mav.open(mav_open_time);
                    //sv.open(sv_open_time);
                    self.mav_open = true;
                    self.sv_open = true;
                    log::info!("MAV and SV opened; Cameras deployed");
                    self.flight_state.reference_pressure = self.flight_state.read_barometer();
                    log::info!("Reference pressure set to {}", self.flight_state.reference_pressure);
                    self.alt_armed = true;
                    self.flight_state.flight_mode = FlightMode::Ascent;
                    log::info!("Transitioning to Ascent");
                } else if !self.key_armed {
                    self.flight_state.flight_mode = FlightMode::Startup;
                    log::info!("Key not armed; Transitioning to Startup");
                }
            }
            FlightMode::Ascent => {
                if self.flight_state.altimeter_state != crate::state::SensorState::VALID {
                    // altimeter is not working
                    self.alt_armed = false;
                    self.flight_state.flight_mode = FlightMode::Fault;
                        log::error!("Altimeter invalid at Ascent; transitioning to Fault");
                }
                // Look at scenario where not above armed altitude and MAV is closed
                if self.flight_state.altimeter_state == SensorState::VALID 
                    && !self.alt_armed
                    && self.flight_state.read_altimeter() > constants::ARMING_ALTITUDE 
                {
                    self.alt_armed = true;

                    log::info!("Altimeter Armed at {} m", self.flight_state.read_altimeter());
                } else if self.flight_state.altimeter_state != crate::state::SensorState::VALID {
                    self.alt_armed = false;
                    self.flight_state.flight_mode = FlightMode::Fault;
                        log::error!("Altimeter invalid at Ascent; transitioning to Fault");
                }
                // Fallback logging if SD card is not ready
                if !self.flight_state.sd_logging_enabled {
                    self.flight_state.log_to_fram().await;
                }
                
                // TODO: Add umbilical check command logic 
                /* ex:
                    umbilical.check_command();
                    umbilical.transmit();
                 */
                if !self.flight_state.umbilical_connected {
                    // For now just log the state for umbilical
                    //log::info!("Umbilical disconnected");
                    //buzzer.buzz_num_times(3);
                } else {
                    log::info!("Umbilical connected");
                    //self.flight_state.flight_mode = FlightMode::Fault; dont know this yet
                    //buzzer.buzz_num_times(2);
                }
                if !self.mav_open {
                    self.flight_state.flight_mode = FlightMode::Coast;
                    log::warn!("MAV closed in Ascent; transitioning to Coast");
                }
            }

            
            FlightMode::Coast => {
                if self.flight_state.altimeter_state != crate::state::SensorState::VALID {
                    // altimeter is not working
                    self.alt_armed = false;
                    self.flight_state.flight_mode = FlightMode::Fault;
                        log::error!("Altimeter invalid at Coast; transitioning to Fault");
                }
                // TODO: Initiate airbrakes
                if self.airbrakes_init {
                    //airbrakes.initiate();
                    // log::info!("Airbrakes initiated"); // Commented out to prevent simulation spam
                }
                if self.flight_state.altimeter_state == SensorState::VALID && self.alt_armed {
                    // Remove old value from sum
                    self.alt_sum -= self.alt_buffer[self.alt_index];
                    // Read new value
                    let current_alt = self.flight_state.read_altimeter();
                    self.alt_buffer[self.alt_index] = current_alt;
                    // Add new value to sum
                    self.alt_sum += current_alt;
                    
                    self.alt_index += 1;
                    if self.alt_index >= 10 {
                        self.alt_index = 0;
                    }
                    
                    let avg_alt = self.alt_sum / 10.0;
                    //Apogee detection
                    self.filtered_alt[2] = self.filtered_alt[1];
                    self.filtered_alt[1] = self.filtered_alt[0];
                    self.filtered_alt[0] = avg_alt;
                    
                    // Apogee detection logic 
                    if self.filtered_alt[2] != -1.0 && self.filtered_alt[1] != -1.0 && self.filtered_alt[0] != -1.0 &&
                       self.filtered_alt[2] > self.filtered_alt[1] && self.filtered_alt[1] > self.filtered_alt[0] 
                    {
                        // TODO: Trigger payload actuators right before apogee
                        // cameras_deployed();
                        self.camera_deployed = true;
                        // TODO: Retract airbrakes
                        // airbrakes.retract();
                        self.airbrakes_init = false;
                        log::info!("Airbrakes retracted");
                        log::info!("Cameras deployed");
                        log::info!("Apogee reached at {:.2} m", self.filtered_alt[1]);
                        // TODO: Open drogue
                        // drogue.open();
                        log::info!("Drogue deployed");
                        self.drogue_deployed = true;
                        self.flight_state.flight_mode = FlightMode::DrogueDeployed;
                        log::info!("Transitioning to DrogueDeployed");
                    }
                }
            }
            FlightMode::DrogueDeployed => {
                if self.flight_state.altimeter_state != crate::state::SensorState::VALID {
                    // altimeter is not working
                    self.alt_armed = false;
                    self.flight_state.flight_mode = FlightMode::Fault;
                        log::error!("Altimeter invalid at DrogueDeployed; transitioning to Fault");
                }
                if self.main_cycle_count < constants::MAIN_DEPLOY_WAIT_CYCLES {
                    self.main_cycle_count += 1;
                } else if self.main_cycle_count == constants::MAIN_DEPLOY_WAIT_CYCLES {
                    self.main_cycle_count += 1;
                    // Push event where main_deploy_wait_cycles is complete
                    log::info!("Main deploy wait cycles complete");
                } else if self.flight_state.read_altimeter() < constants::MAIN_DEPLOY_ALTITUDE {
                    // TODO: Deploy main parachutes
                    // main.open();
                    self.main_chutes_deployed = true;
                    log::info!("Main deployed");
                    self.flight_state.flight_mode = FlightMode::MainDeployed;
                    log::info!("Transitioning to MainDeployed");
                }
            }
            FlightMode::MainDeployed => {
                if self.flight_state.altimeter_state != crate::state::SensorState::VALID {
                    // altimeter is not working
                    self.alt_armed = false;
                    self.flight_state.flight_mode = FlightMode::Fault;
                    log::error!("Altimeter invalid at MainDeployed; transitioning to Fault");
                }
                if self.log_cycle_count < constants::MAIN_LOG_END_CYCLES {
                    self.log_cycle_count += 1;
                } 
                // TODO: Turn off data logging after certain amount of cycles to not overwrite data
                if self.log_cycle_count == constants::MAIN_LOG_END_CYCLES {
                    // TODO: Main log shutdown
                    // log.shutdown();
                    self.log_armed = false;
                    log::info!("Main log shutdown");
                    self.log_cycle_count += 1;
                }
                // TODO: Initiate BLiMS
                // blims.initiate();
                self.blims_armed = true;
                // log::info!("BLiMS initiated"); // Commented out prevent log spam
            }
            FlightMode::Fault => {
                // TODO: Flight software does nothing in Fault mode
                // TODO: Umbilical check ex:
                // umbilical.check_command();
                // umbilical.transmit();
                log::error!("Flight mode is Fault");
            }
        }
    }

    // Simulation Helpers

    pub fn set_altitude(&mut self, altitude: f32) {
        self.flight_state.packet.altitude = altitude;
    }

    pub fn set_pressure(&mut self, pressure: f32) {
        self.flight_state.packet.pressure = pressure;
    }

    pub fn set_key_switch(&mut self, armed: bool) {
        self.flight_state.key_armed = armed;
        self.key_armed = armed;
    }

    pub fn set_umbilical(&mut self, connected: bool) {
        self.flight_state.umbilical_connected = connected;
        self.umbilical_state = connected;
    }

    pub fn set_launch_command(&mut self, launch: bool) {
        self.umbilical_launch = launch;
    }

    pub fn set_altimeter_state(&mut self, state: SensorState) {
        self.flight_state.altimeter_state = state;
    }

    pub fn set_airbrakes(&mut self, armed: bool) {
        self.airbrakes_init = armed;
    }

    pub fn set_cameras_deployed(&mut self, deployed: bool) {
        self.camera_deployed = deployed;
    }

    pub fn set_mav_open(&mut self, open: bool) {
        self.mav_open = open;
    }

    pub fn set_sv_open(&mut self, open: bool) {
        self.sv_open = open;
    }

    /// Run a simulation cycle: skips hardware reads/writes, only logic.
    pub async fn simulate_cycle(&mut self) {
        // Sync local fields from state (in case modified directly)
        self.key_armed = self.flight_state.key_armed;
        self.umbilical_state = self.flight_state.umbilical_connected;
        
        // Log Simulated Sensor Data
        //if !(self.flight_state.flight_mode == FlightMode::Ascent) || !(self.flight_state.flight_mode == FlightMode::Coast) {
        log::info!(
            "[SIM] Alt: {:.2}, Pres: {:.2}, State: {:?}",
            self.flight_state.packet.altitude,
            self.flight_state.packet.pressure,
            self.flight_state.altimeter_state,
        );
        //}

        // Run logic
        self.check_transitions().await;
    }
}
