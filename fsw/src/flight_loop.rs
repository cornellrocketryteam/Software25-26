use core::f32;
use embassy_time::Instant;

use crate::state::{FlightMode, FlightState};
use crate::constants;
use crate::state::SensorState;
use crate::umbilical::{self, UmbilicalCommand};

// TODO: Add //CHALLENGE_# to each fault with its solution
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
    drogue_entry_time: Option<Instant>,
    main_entry_time: Option<Instant>,
    airbrakes_logged: bool,
    
    // Umbilical logic
    umbilical_disconnect_time: Option<Instant>,
    vent_signal_sent: bool,
    mav_open_time: Option<Instant>,
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
            drogue_entry_time: None,
            main_entry_time: None,
            airbrakes_logged: false,
            umbilical_disconnect_time: None,
            vent_signal_sent: false,
            mav_open_time: None,
        }
    }

    // Resets the internal moving average filters and state.
    pub fn reset_filter_buffers(&mut self) {
        self.alt_buffer = [self.flight_state.packet.altitude; 10];
        self.alt_sum = self.flight_state.packet.altitude * 10.0;
        self.alt_index = 0;
        self.filtered_alt = [-1.0; 3];
        self.drogue_deployed = false;
        // self.flight_state.flight_mode = FlightMode::Startup;
    }

    pub fn set_flight_mode(&mut self, mode: FlightMode) {
        log::warn!("Manual Override: Force transition from {:?} to {:?}", self.flight_state.flight_mode, mode);
        self.flight_state.flight_mode = mode;
    }
    
    pub async fn execute(&mut self) {
        self.flight_state.read_sensors().await;
        
        // Check subsystem health
        self.flight_state.check_subsystem_health().await;
        
        if !self.flight_state.payload_comms_ok {
            log::warn!("FLAG_PAYLOAD_COMMS_FAIL: Payload communication failure detected!");
        }
        if !self.flight_state.recovery_comms_ok {
            log::warn!("FLAG_RECOVERY_COMMS_FAIL: Recovery communication failure detected!");
        }
        
        // Update local loop state from FlightState
        self.key_armed = self.flight_state.key_armed;
        
        // Placeholder: Check umbilical state here (e.g. read another GPIO)
        self.umbilical_state = self.flight_state.umbilical_connected;

        self.check_ground_commands().await;
        self.check_umbilical_commands().await;
        self.check_transitions().await;
        self.flight_state.transmit().await;
        
        // Log current state
        log::info!(
            "Current Flight Mode: {} on cycle {} \n",
            self.flight_state.flight_mode_name(),
            self.flight_state.cycle_count
        );
    }
    
    pub async fn check_ground_commands(&mut self) {
        // TODO: Implement actual radio command receiving logic here
        // For now  placeholder for the fill station vent.
        log::info!("Sent vent command to Fill Station.");
        // if let Some(cmd) = self.flight_state.radio.receive().await {
        //     match cmd {
        //         Command::Vent => {
        //             log::warn!("CMD: Vent Command Received");
        //             self.vent_signal_sent = true;
        //         },
        //         Command::ForceMode(mode) => {
        //             self.set_flight_mode(mode);
        //         },
        //         _ => {}
        //     }
        // }
    }

    pub async fn check_umbilical_commands(&mut self) {
        while let Some(cmd) = umbilical::try_recv_command() {
            match cmd {
                UmbilicalCommand::Launch => {
                    log::warn!("UMBILICAL CMD: Launch received");
                    self.set_launch_command(true);
                },
                UmbilicalCommand::OpenMav => {
                    log::warn!("UMBILICAL CMD: Open MAV");
                    self.flight_state.open_mav(constants::MAV_OPEN_DURATION_MS).await;
                    self.mav_open = true;
                    self.mav_open_time = Some(Instant::now());
                },
                UmbilicalCommand::CloseMav => {
                    log::warn!("UMBILICAL CMD: Close MAV");
                    self.flight_state.close_mav().await;
                    self.mav_open = false;
                    self.mav_open_time = None;
                },
                UmbilicalCommand::OpenSv => {
                    log::warn!("UMBILICAL CMD: Open SV");
                    self.flight_state.open_sv(0).await;
                    self.sv_open = true;
                },
                UmbilicalCommand::CloseSv => {
                    log::warn!("UMBILICAL CMD: Close SV");
                    self.flight_state.close_sv().await;
                    self.sv_open = false;
                },
                UmbilicalCommand::Safe => {
                    log::warn!("UMBILICAL CMD: Safe — closing all actuators");
                    self.flight_state.close_mav().await;
                    self.flight_state.close_sv().await;
                    self.mav_open = false;
                    self.sv_open = false;
                    self.mav_open_time = None;
                },
                UmbilicalCommand::ResetFram => {
                    log::warn!("UMBILICAL CMD: Reset FRAM");
                    self.flight_state.reset_fram().await;
                },
                UmbilicalCommand::ResetCard => {
                    log::warn!("UMBILICAL CMD: Reset SD Card");
                    // TODO: Implement SD card reset when SD logging is enabled
                },
                UmbilicalCommand::Reboot => {
                    log::warn!("UMBILICAL CMD: Reboot");
                    cortex_m::peripheral::SCB::sys_reset();
                },
            }
        }
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
                if self.flight_state.umbilical_connected {
                    // For now just log the state for umbilical
                    log::info!("Umbilical connected");
                    self.umbilical_disconnect_time = None;
                    self.vent_signal_sent = false;
                    self.flight_state.buzz(2);
                } else {
                    log::info!("Umbilical disconnected");
                    // Start timer if not started
                    if self.umbilical_disconnect_time.is_none() {
                        self.umbilical_disconnect_time = Some(Instant::now());
                    } else if let Some(disconnect_time) = self.umbilical_disconnect_time {
                        if disconnect_time.elapsed().as_millis() as u64 > constants::UMBILICAL_TIMEOUT_MS && !self.vent_signal_sent {
                            log::warn!("Umbilical Disconnected > 15s. Signal sent to fill station to vent.");
                            // TODO: Replace with actual command: umbilical.send_vent_command();
                            log::info!("CMD: Vent Command Sent");
                            self.vent_signal_sent = true;
                        }
                    }
                    self.flight_state.buzz(3);
                }

                if self.flight_state.altimeter_state == crate::state::SensorState::INVALID {
                    self.alt_armed = false;
                    self.flight_state.flight_mode = FlightMode::Fault;
                    self.flight_state.write_state_to_fram().await;
                    log::error!("Altimeter invalid at Startup; transitioning to Fault");
                    return;
                }

                if self.key_armed {
                    if self.flight_state.altimeter_state == crate::state::SensorState::VALID {
                        // Record arming altitude (TODO: implement into storage)
                        self.alt_armed = true;
                        self.flight_state.arming_altitude = self.flight_state.read_altimeter();
                        log::info!("Arming altitude set to {}", self.flight_state.arming_altitude);
                        self.flight_state.flight_mode = FlightMode::Standby;
                        log::info!("Transitioning to Standby");
                    } 
                }
                
            }
            FlightMode::Standby => {
                /* Maybe go back to startup? Or some buzzer or way to signal the altimeter is bad?
                Add an indicator if the altimeter is bad for either standby and/or startup so we can try to fix it, if no fix 
                then go to fault mode
                */
                if self.flight_state.altimeter_state != crate::state::SensorState::VALID {
                    // altimeter is not working
                    self.alt_armed = false;
                    self.flight_state.flight_mode = FlightMode::Fault;
                    self.flight_state.write_state_to_fram().await;
                    log::error!("Altimeter invalid at Standby; transitioning to Fault");
                    return;
                }
                // TODO: add umbilical check command logic with written class (no need to fault, use this to know to connect umbilical)
                if self.flight_state.umbilical_connected {
                    // For now just log the state for umbilical
                    log::info!("Umbilical connected");
                    self.umbilical_disconnect_time = None;
                    self.vent_signal_sent = false;
                    self.flight_state.buzz(2);
                } else {
                    log::info!("Umbilical disconnected");
                    // Start timer if not started
                    if self.umbilical_disconnect_time.is_none() {
                        self.umbilical_disconnect_time = Some(Instant::now());
                    } else if let Some(disconnect_time) = self.umbilical_disconnect_time {
                        if disconnect_time.elapsed().as_millis() as u64 > constants::UMBILICAL_TIMEOUT_MS && !self.vent_signal_sent {
                            log::warn!("Umbilical Disconnected > 15s. Signal sent to fill station to vent.");
                            // TODO: Replace with actual command: umbilical.send_vent_command();
                            log::info!("CMD: Vent Command Sent");
                            self.vent_signal_sent = true;
                        }
                    }
                    self.flight_state.buzz(3);
                }

                // Check altimeter for launch with umbilical
                if self.umbilical_launch {
                    // TODO: Send command for launch to payload
                    // send_launch_command();
                    log::info!("Payload launch command sent");
                    
                    // Open MAV and SV
                    self.flight_state.open_mav(constants::MAV_OPEN_DURATION_MS).await;
                    self.flight_state.open_sv(0).await; 

                    self.mav_open = true; // logic flag
                    self.sv_open = true;
                    self.mav_open_time = Some(Instant::now());
                    
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
                    self.flight_state.write_state_to_fram().await;
                    log::error!("Altimeter invalid at Ascent; transitioning to Fault");
                    return;
                }
                // Look at scenario where not above armed altitude and MAV is closed
                if self.flight_state.altimeter_state == SensorState::VALID 
                    && !self.alt_armed
                    && self.flight_state.read_altimeter() > constants::ARMING_ALTITUDE 
                {
                    self.alt_armed = true;

                    log::info!("Altimeter Armed at {} m", self.flight_state.read_altimeter());
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
                    self.flight_state.buzz(3);
                } else {
                    log::info!("Umbilical connected");
                    //self.flight_state.flight_mode = FlightMode::Fault; dont know this yet
                    self.flight_state.buzz(2);
                }
                if !self.mav_open {
                    self.flight_state.flight_mode = FlightMode::Coast;
                    log::warn!("MAV closed in Ascent; transitioning to Coast");
                } else {
                    // Check timer
                    if let Some(open_time) = self.mav_open_time {
                        if open_time.elapsed().as_millis() as u64 >= constants::MAV_OPEN_DURATION_MS {
                             self.mav_open = false;
                             self.mav_open_time = None;
                             self.flight_state.close_mav().await;
                             log::info!("MAV closed after {}ms", constants::MAV_OPEN_DURATION_MS);
                        }
                    }
                }
            }

            
            FlightMode::Coast => {
                if self.flight_state.altimeter_state != crate::state::SensorState::VALID {
                    // altimeter is not working
                    self.alt_armed = false;
                    self.flight_state.flight_mode = FlightMode::Fault;
                    self.flight_state.write_state_to_fram().await;
                    log::error!("Altimeter invalid at Coast; transitioning to Fault");
                    return;
                }
                // TODO: Initiate airbrakes
                if self.airbrakes_init && !self.airbrakes_logged {
                    //airbrakes.initiate();
                    self.airbrakes_logged = true;
                    log::info!("Airbrakes initiated");
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
                    // Apogee detection
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
                        
                        // Deploy Drogue
                        self.flight_state.trigger_drogue().await;
                        
                        log::info!("Drogue deployed");
                        self.drogue_deployed = true;
                        self.flight_state.flight_mode = FlightMode::DrogueDeployed;
                        self.drogue_entry_time = Some(Instant::now());
                        log::info!("Transitioning to DrogueDeployed");
                    }
                }
            }
            FlightMode::DrogueDeployed => {
                if self.flight_state.altimeter_state != crate::state::SensorState::VALID {
                    // altimeter is not working
                    self.alt_armed = false;
                    self.flight_state.flight_mode = FlightMode::Fault;
                    self.flight_state.write_state_to_fram().await;
                    log::error!("Altimeter invalid at DrogueDeployed; transitioning to Fault");
                    return;
                }
                
                // Get time since entry
                if let Some(entry_time) = self.drogue_entry_time {
                   if entry_time.elapsed().as_millis() >= constants::MAIN_DEPLOY_DELAY_MS {
                        if self.flight_state.read_altimeter() < constants::MAIN_DEPLOY_ALTITUDE {
                            // Deploy Main
                            self.flight_state.trigger_main().await;
                            
                            self.main_chutes_deployed = true;
                            log::info!("Main deployed");
                            self.flight_state.flight_mode = FlightMode::MainDeployed;
                            self.main_entry_time = Some(Instant::now());
                            log::info!("Transitioning to MainDeployed");
                        }
                   }
                }
            }
            FlightMode::MainDeployed => {
                if self.flight_state.altimeter_state != crate::state::SensorState::VALID {
                    // altimeter is not working
                    self.alt_armed = false;
                    self.flight_state.flight_mode = FlightMode::Fault;
                    self.flight_state.write_state_to_fram().await;
                    log::error!("Altimeter invalid at MainDeployed; transitioning to Fault");
                    return;
                }
                
                if let Some(entry_time) = self.main_entry_time {
                    if entry_time.elapsed().as_millis() >= constants::MAIN_LOG_TIMEOUT_MS {
                         if self.log_armed {
                            // TODO: Main log shutdown
                            // log.shutdown();
                            self.log_armed = false;
                            log::info!("Main log shutdown after timeout");
                         }
                    }
                }

                // TODO: Initiate BLiMS
                if !self.blims_armed {
                    // blims.initiate();
                    self.blims_armed = true;
                    log::info!("BLiMS initiated");
                }
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

    pub fn get_altitude(&mut self) -> f32{
        return self.flight_state.packet.altitude;
    }

    // Run a simulation cycle: skips hardware reads/writes, only logic.
    pub async fn simulate_cycle(&mut self) {
        // Sync local fields from state (in case modified directly)
        self.key_armed = self.flight_state.key_armed;
        self.umbilical_state = self.flight_state.umbilical_connected;
        
        // Log Simulated Sensor Data
        //if !(self.flight_state.flight_mode == FlightMode::Ascent) || !(self.flight_state.flight_mode == FlightMode::Coast) {
        //log::info!(
        //    "[SIM] Alt: {:.2}, Pres: {:.2}, State: {:?}",
        //    self.flight_state.packet.altitude,
        //    self.flight_state.packet.pressure,
        //    self.flight_state.altimeter_state,
        //);
        //}

        // Run logic
        self.check_transitions().await;
    }
}
