use core::f32;
use embassy_time::Instant;

use crate::airbrake_task::{AirbrakeInput, AirbrakePhase, AIRBRAKE_INPUT};
use crate::constants;
use crate::state::SensorState;
use crate::state::{FlightMode, FlightState};
use crate::umbilical::{self, UmbilicalCommand};

// TODO: Add //CHALLENGE_# to each fault with its solution
// TODO: Remove some bools and edit FlightLoop to be able to trigger events with methods
// ex: a function to say that the umbilical is connected, or umbilical launch, etc.
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum LaunchStage {
    None,
    PreVent,   // SV Open for 2s
    MavOpen,   // MAV Open for 7.88s
    PostWait,  // Wait 10s (override if apogee)
    FinalVent, // SV Open for rest of flight
}

// GPIO 32 for TX UART, GPIO 33 for RX UART
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
    umbilical_prev: bool,
    cfc_arm_prev: bool,

    // Flash logging timing
    last_flash_log: Option<Instant>,

    // Launch sequence
    pub launch_sequence_stage: LaunchStage,
    launch_stage_start_time: Option<Instant>,

    // Payload Commands tracking
    low_alt_time: Option<Instant>,
    n3_sent: bool,
    n4_sent: bool,
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
            umbilical_prev: false,
            cfc_arm_prev: false,
            last_flash_log: None,
            launch_sequence_stage: LaunchStage::None,
            launch_stage_start_time: None,
            low_alt_time: None,
            n3_sent: false,
            n4_sent: false,
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
        self.flight_state.flight_mode = mode;
    }

    pub async fn execute(&mut self) {
        // 1. Check for commands (GSE, Umbilical, etc.)
        self.check_umbilical_commands().await;
        self.check_ground_commands().await;

        // 2. Read sensor data
        self.flight_state.read_sensors().await;

        // 2b. Forward latest sensor data to the airbrake controller on Core 1.
        // Signal::signal() is non-blocking and always delivers the most recent
        // value, so the flight loop is never delayed by airbrake computation.
        let airbrake_phase = match self.flight_state.flight_mode {
            FlightMode::Startup | FlightMode::Standby => Some(AirbrakePhase::Pad),
            FlightMode::Ascent  => Some(AirbrakePhase::Boost),
            FlightMode::Coast   => Some(AirbrakePhase::Coast),
            _ => None, // DrogueDeployed / MainDeployed / Fault — airbrakes inactive
        };
        if let Some(phase) = airbrake_phase {
            AIRBRAKE_INPUT.signal(AirbrakeInput {
                time:     self.flight_state.packet.timestamp,
                altitude: self.flight_state.packet.altitude,
                gyro_x:   self.flight_state.packet.gyro_x,
                gyro_y:   self.flight_state.packet.gyro_y,
                accel_x:  self.flight_state.packet.accel_x,
                accel_y:  self.flight_state.packet.accel_y,
                accel_z:  self.flight_state.packet.accel_z,
                phase,
            });
        }

        // 3. Process logic and transitions
        self.check_transitions().await;
        self.handle_launch_sequence().await;

        // 4. Update actuators
        self.flight_state.update_actuators().await;

        // 5. Transmit telemetry (radio + USB umbilical binary frames)
        self.flight_state.transmit().await;

        // Save packet to QSPI Flash
        let now = Instant::now();
        let should_log = match self.last_flash_log {
            None => true,
            Some(last) => {
                now.duration_since(last).as_millis() >= constants::FLASH_LOGGING_PERIOD_MS
            }
        };

        if should_log {
            self.flight_state.save_packet_to_flash().await;
            self.last_flash_log = Some(now);
        }
    }

    pub async fn check_ground_commands(&mut self) {
        use crate::packet::Command;

        // Poll for radio commands from ground station
        if let Some(cmd) = self.flight_state.poll_radio_command().await {
            match cmd {
                Command::Vent => {
                    log::warn!("CMD: Vent Command Received");
                    self.vent_signal_sent = true;
                }
                Command::N1 => {
                    // Start-up sequence for camera deployment
                    if matches!(
                        self.flight_state.flight_mode,
                        FlightMode::Startup | FlightMode::Standby
                    ) {
                        let _ = self.flight_state.payload_uart.write(b"N1\n").await;
                        log::info!("PAYLOAD: Sent N1 (Camera Deploy)");
                    }
                }
                Command::N2 => {
                    let _ = self.flight_state.payload_uart.write(b"N2\n").await;
                    log::info!("PAYLOAD: Sent N2");
                }
                Command::N3 => {
                    if self.flight_state.packet.altitude < 250.0 {
                        if self.low_alt_time.is_none() {
                            self.low_alt_time = Some(Instant::now());
                        } else if let Some(low_time) = self.low_alt_time {
                            if low_time.elapsed().as_millis() as u64 > 1000 && !self.n3_sent {
                                log::warn!("Low Altitude Detected (>1s).");
                                let _ = self.flight_state.payload_uart.write(b"N3\n").await;
                                self.n3_sent = true;
                            }
                        }
                    } else {
                        self.low_alt_time = None;
                    }
                }
                Command::N4 => {
                    let ax = self.flight_state.packet.accel_x;
                    let ay = self.flight_state.packet.accel_y;
                    let az = self.flight_state.packet.accel_z;

                    if (ax.abs() > 30.0 || ay.abs() > 30.0 || az.abs() > 30.0) && !self.n4_sent {
                        log::warn!("High Acceleration Detected. Sending N4.");
                        let _ = self.flight_state.payload_uart.write(b"N4\n").await;
                        self.n4_sent = true;
                    }
                }
                Command::ForceMode(mode_val) => {
                    log::warn!("CMD: Force Flight Mode {}", mode_val);
                    self.set_flight_mode(FlightMode::from_u32(mode_val));
                }
            }
        }
    }

    pub async fn check_umbilical_commands(&mut self) {
        while let Some(cmd) = umbilical::try_recv_command() {
            match cmd {
                UmbilicalCommand::Launch => {
                    log::warn!("UMBILICAL CMD: Launch received");
                    self.set_launch_command(true);
                }
                UmbilicalCommand::OpenMav => {
                    // CHANGE THIS TO MAV DELAY LATER FOR WET DRESS
                    log::warn!("UMBILICAL CMD: Open MAV");
                    self.flight_state.open_mav(0).await; // 0 = no auto-close timer (manual close only)
                    self.mav_open = true;
                    self.mav_open_time = None; // no timer tracking needed
                }
                UmbilicalCommand::CloseMav => {
                    log::warn!("UMBILICAL CMD: Close MAV");
                    self.flight_state.close_mav().await;
                    self.mav_open = false;
                    self.mav_open_time = None;
                }
                UmbilicalCommand::OpenSv => {
                    log::warn!("UMBILICAL CMD: Open SV");
                    self.flight_state.open_sv(0).await;
                    self.sv_open = true;
                }
                UmbilicalCommand::CloseSv => {
                    log::warn!("UMBILICAL CMD: Close SV");
                    self.flight_state.close_sv().await;
                    self.sv_open = false;
                }
                UmbilicalCommand::Safe => {
                    log::warn!("UMBILICAL CMD: Safe — closing all actuators");
                    self.flight_state.close_mav().await;
                    self.flight_state.close_sv().await;
                    self.mav_open = false;
                    self.sv_open = false;
                    self.mav_open_time = None;
                }
                UmbilicalCommand::ResetFram => {
                    log::warn!("UMBILICAL CMD: Reset FRAM");
                    self.flight_state.reset_fram().await;
                }
                UmbilicalCommand::DumpFram => {
                    log::warn!("UMBILICAL CMD: Dump FRAM");
                    self.flight_state.dump_fram().await;
                }
                UmbilicalCommand::ResetCard => {
                    log::warn!("UMBILICAL CMD: Reset SD Card");
                    // TODO: Implement SD card reset when SD logging is enabled
                }
                UmbilicalCommand::Reboot => {
                    log::warn!("UMBILICAL CMD: Reboot");
                    cortex_m::peripheral::SCB::sys_reset();
                }
                UmbilicalCommand::DumpFlash => {
                    log::warn!("UMBILICAL CMD: Dump Flash Data");
                    self.flight_state.print_flash_dump().await;
                }
                UmbilicalCommand::WipeFlash => {
                    log::warn!("UMBILICAL CMD: Wipe Flash Data");
                    self.flight_state.wipe_flash_storage().await;
                }
                UmbilicalCommand::FlashInfo => {
                    log::warn!("UMBILICAL CMD: Flash Storage Info");
                    self.flight_state.print_flash_status().await;
                }
                UmbilicalCommand::PayloadN1 => {
                    if matches!(
                        self.flight_state.flight_mode,
                        FlightMode::Startup | FlightMode::Standby
                    ) {
                        let _ = self.flight_state.payload_uart.write(b"N1\n").await;
                        log::info!("UMBILICAL: Sent N1 (Camera Deploy)");
                    }
                }
                UmbilicalCommand::PayloadN2 => {
                    let _ = self.flight_state.payload_uart.write(b"N2\n").await;
                    log::info!("UMBILICAL: Sent N2");
                }
                UmbilicalCommand::PayloadN3 => {
                    let _ = self.flight_state.payload_uart.write(b"N3\n").await;
                    log::info!("UMBILICAL: Sent N3");
                    self.n3_sent = true;
                }
                UmbilicalCommand::PayloadN4 => {
                    let _ = self.flight_state.payload_uart.write(b"N4\n").await;
                    log::info!("UMBILICAL: Sent N4");
                    self.n4_sent = true;
                }
            }
        }
    }

    pub async fn check_transitions(&mut self) {
        // Retrieve current values for easier access
        let _packet = &self.flight_state.packet;
        let _mode = self.flight_state.flight_mode;

        // CFC_ARM rising edge: buzz once when arming signal goes high
        let cfc_arm_now = self.flight_state.cfc_arm_active;
        if cfc_arm_now && !self.cfc_arm_prev {
            log::info!("CFC_ARM detected: arming signal received");
            self.flight_state.buzz(2);
        }
        self.cfc_arm_prev = cfc_arm_now;

        // Transition logic
        match self.flight_state.flight_mode {
            FlightMode::Startup => {
                if self.flight_state.altimeter_state == crate::state::SensorState::VALID {
                    // Update reference pressure before launch
                    self.flight_state.reference_pressure = self.flight_state.read_barometer();
                    log::info!(
                        "Reference pressure set to {}",
                        self.flight_state.reference_pressure
                    );
                }
                if self.flight_state.umbilical_connected {
                    log::info!("Umbilical connected");
                    self.umbilical_disconnect_time = None;
                    self.vent_signal_sent = false;
                    if !self.umbilical_prev {
                        self.flight_state.buzz(2); // just reconnected
                    }
                } else {
                    log::info!("Umbilical disconnected");
                    // Start timer if not started
                    if self.umbilical_disconnect_time.is_none() {
                        self.umbilical_disconnect_time = Some(Instant::now());
                    } else if let Some(disconnect_time) = self.umbilical_disconnect_time {
                        if disconnect_time.elapsed().as_millis() as u64
                            > constants::UMBILICAL_TIMEOUT_MS
                            && !self.vent_signal_sent
                        {
                            log::warn!("Umbilical Disconnected > 15s. Opening SV to vent.");
                            self.flight_state.open_sv(0).await;
                            self.sv_open = true;
                            self.vent_signal_sent = true;
                        }
                    }
                    if self.umbilical_prev {
                        self.flight_state.buzz(3); // just disconnected
                    }
                }
                self.umbilical_prev = self.flight_state.umbilical_connected;

                if self.flight_state.altimeter_state == crate::state::SensorState::INVALID {
                    self.alt_armed = false;
                    self.flight_state.flight_mode = FlightMode::Fault;
                    // self.flight_state.write_state_to_fram().await; // Note: cannot await inside check_transitions easily if not mut
                    log::error!("Altimeter invalid at Startup; transitioning to Fault");
                    return;
                }

                if self.key_armed && self.flight_state.umbilical_connected {
                    if self.flight_state.altimeter_state == crate::state::SensorState::VALID {
                        // Record arming altitude (TODO: implement into storage)
                        self.alt_armed = true;
                        self.flight_state.arming_altitude = self.flight_state.read_altimeter();
                        log::info!(
                            "Arming altitude set to {}",
                            self.flight_state.arming_altitude
                        );
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
                if self.flight_state.umbilical_connected {
                    log::info!("Umbilical connected");
                    self.umbilical_disconnect_time = None;
                    self.vent_signal_sent = false;
                    if !self.umbilical_prev {
                        self.flight_state.buzz(2); // just reconnected
                    }
                } else {
                    log::info!("Umbilical disconnected");
                    self.umbilical_launch = false; // Abort any pending launch command if umbilical drops
                    // Start timer if not started
                    if self.umbilical_disconnect_time.is_none() {
                        self.umbilical_disconnect_time = Some(Instant::now());
                    } else if let Some(disconnect_time) = self.umbilical_disconnect_time {
                        if disconnect_time.elapsed().as_millis() as u64
                            > constants::UMBILICAL_TIMEOUT_MS
                            && !self.vent_signal_sent
                        {
                            log::warn!("Umbilical Disconnected > 15s. Opening SV to vent.");
                            self.flight_state.open_sv(0).await;
                            self.sv_open = true;
                            self.vent_signal_sent = true;
                        }
                    }
                    if self.umbilical_prev {
                        self.flight_state.buzz(3); // just disconnected
                    }
                }
                self.umbilical_prev = self.flight_state.umbilical_connected;

                // Check altimeter for launch with umbilical
                if self.umbilical_launch && self.flight_state.umbilical_connected {
                    // TODO: Send command for launch to payload
                    // send_launch_command();                    log::info!("Payload launch command sent");

                    // START LAUNCH SEQUENCE
                    log::warn!("LAUNCH INITIATED: Starting actuator sequence.");
                    self.launch_sequence_stage = LaunchStage::PreVent;
                    self.launch_stage_start_time = Some(Instant::now());

                    self.flight_state.reference_pressure = self.flight_state.read_barometer();
                    log::info!(
                        "Reference pressure set to {}",
                        self.flight_state.reference_pressure
                    );

                    // Stage 1: SV Open (2s vent)
                    self.flight_state.open_sv(0).await;
                    self.sv_open = true;

                    self.alt_armed = true;
                    self.flight_state.flight_mode = FlightMode::Ascent;
                    log::info!("Transitioning to Ascent");
                } else if !self.key_armed {
                    self.flight_state.flight_mode = FlightMode::Startup;
                    log::info!("Key not armed; Transitioning to Startup");
                }
            }
            FlightMode::Ascent => {
                // if self.flight_state.umbilical_connected {
                //     log::error!(
                //         "CRITICAL: Umbilical still connected during Ascent! Transitioning to Fault"
                //     );
                //     self.flight_state.flight_mode = FlightMode::Fault;
                //     return;
                // }

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

                    log::info!(
                        "Altimeter Armed at {} m",
                        self.flight_state.read_altimeter()
                    );
                }
                // Fallback logging if SD card is not ready
                if !self.flight_state.sd_logging_enabled {
                    self.flight_state.log_to_fram().await;
                }

                // --- Launch Sequence State Machine ---
                if !self.flight_state.sd_logging_enabled {}
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

                    // Read latest airbrake deployment from Core 1 (non-blocking).
                    // TODO: drive airbrake servo here once servo is added to actuator.rs
                    let deployment = crate::airbrake_task::get_deployment();
                    log::info!("Airbrake deployment: {:.1}%", deployment * 100.0);

                    // Apogee detection
                    self.filtered_alt[2] = self.filtered_alt[1];
                    self.filtered_alt[1] = self.filtered_alt[0];
                    self.filtered_alt[0] = avg_alt;

                    // Apogee detection logic
                    if self.filtered_alt[2] != -1.0
                        && self.filtered_alt[1] != -1.0
                        && self.filtered_alt[0] != -1.0
                        && self.filtered_alt[2] > self.filtered_alt[1]
                        && self.filtered_alt[1] > self.filtered_alt[0]
                    {
                        // TODO: Trigger payload actuators right before apogee
                        // cameras_deployed();
                        self.camera_deployed = true;
                        // Airbrakes retract at apogee — Core 1 stops receiving
                        // Coast signals so it will hold at 0.0 deployment.
                        self.airbrakes_init = false;
                        log::info!("Airbrakes retracted");
                        log::info!("Cameras deployed");
                        log::info!("Apogee reached at {:.2} m", self.filtered_alt[1]);

                        // Deploy Drogue
                        self.flight_state.trigger_drogue().await;

                        // Override: Open SV on apogee if still in PostWait
                        if self.launch_sequence_stage == LaunchStage::PostWait {
                            log::info!("Apogee override: Opening SV regardless of 10s timer.");
                            self.flight_state.open_sv(0).await;
                            self.sv_open = true;
                            self.launch_sequence_stage = LaunchStage::FinalVent;
                        }

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

    pub async fn handle_launch_sequence(&mut self) {
        let sequence_now = Instant::now();
        match self.launch_sequence_stage {
            LaunchStage::PreVent => {
                if let Some(start) = self.launch_stage_start_time {
                    if sequence_now.duration_since(start).as_millis()
                        >= constants::LAUNCH_SV_PREVENT_MS
                    {
                        log::info!("Pre-launch vent complete. Closing SV, opening MAV.");
                        self.flight_state.close_sv().await;
                        self.sv_open = false;

                        // Stage 2: MAV Open (7.88s)
                        self.flight_state
                            .open_mav(constants::MAV_OPEN_DURATION_MS)
                            .await;
                        self.mav_open = true;

                        self.launch_sequence_stage = LaunchStage::MavOpen;
                        self.launch_stage_start_time = Some(sequence_now);
                    }
                }
            }
            LaunchStage::MavOpen => {
                if let Some(start) = self.launch_stage_start_time {
                    if sequence_now.duration_since(start).as_millis()
                        >= constants::MAV_OPEN_DURATION_MS
                    {
                        log::info!("MAV cycle complete. Closing MAV, waiting 10s.");
                        self.flight_state.close_mav().await;
                        self.mav_open = false;

                        // TRANSITION TO COAST if currently in Ascent
                        if self.flight_state.flight_mode == FlightMode::Ascent {
                            log::warn!("MAV closed; Transitioning from Ascent to Coast.");
                            self.flight_state.flight_mode = FlightMode::Coast;
                        }

                        // Stage 3: Post-MAV Wait (10s)
                        self.launch_sequence_stage = LaunchStage::PostWait;
                        self.launch_stage_start_time = Some(sequence_now);
                    }
                }
            }
            LaunchStage::PostWait => {
                if let Some(start) = self.launch_stage_start_time {
                    if sequence_now.duration_since(start).as_millis()
                        >= constants::LAUNCH_POST_MAV_WAIT_MS
                    {
                        log::info!("Post-MAV wait complete. Opening SV for final vent.");
                        self.flight_state.open_sv(0).await;
                        self.sv_open = true;

                        // Stage 4: Final Vent (Rest of flight)
                        self.launch_sequence_stage = LaunchStage::FinalVent;
                    }
                }
            }
            _ => {}
        }
    }

    pub fn get_altitude(&mut self) -> f32 {
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

        // Process any commands sent over USB during simulation
        self.check_umbilical_commands().await;
        self.check_ground_commands().await;

        // Run logic
        self.check_transitions().await;
        self.handle_launch_sequence().await;

        // Continuously update actuators so timers and physical pins actually output during simulation tests
        self.flight_state.update_actuators().await;
    }
}
