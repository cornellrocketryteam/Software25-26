use core::f32;
use embassy_time::{Duration, Instant, Timer};

use blims::blims_state::BlimsDataIn;

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
    PreVent,      // SV Open for 2s
    SvToMavWait,  // 1s gap between SV close and MAV open
    MavOpen,      // MAV Open for MAV_OPEN_DURATION_MS
    Done,         // Sequence finished; SV reopens later on entry to Drogue/Main/Fault
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

    // BLiMS parafoil guidance system (None until hardware is wired)
    blims: Option<blims::Blims<'static>>,
    blims_target_set: bool,
    blims_target_lat: f32,
    blims_target_lon: f32,
    blims_wind_from_deg: f32,

    // Internal state tracking
    alt_buffer: [f32; 10],
    alt_index: usize,
    // L3 launch-detect: 10-sample moving avg of Y-axis accel (m/s²).
    accel_buffer: [f32; 10],
    accel_index: usize,
    accel_sum: f32,
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
    last_full_log: Option<Instant>,
    last_heartbeat: Option<Instant>,

    // Launch sequence
    pub launch_sequence_stage: LaunchStage,
    launch_stage_start_time: Option<Instant>,
    recovery_vent_sent: bool,

    // Payload Commands tracking
    low_alt_time: Option<Instant>,
    n2_low_speed_count: u8,
    n2_sent: bool,
    n3_sent: bool,
    n4_sent: bool,

    // Fault signaling
    fault_signal_sent: bool,
    last_alt: f32,

    // Overpressure latch — once PT3 has been above the threshold for 3
    // consecutive cycles we open SV and fault. Single-sample noise spikes
    // won't trigger.
    overpressure_triggered: bool,
    overpressure_count: u8,

    /// Sim only: if Some, overrides altitude + forces altimeter VALID after read_sensors().
    /// Set to None in normal flight — zero cost.
    pub sim_altitude_override: Option<f32>,

    /// Sim only: if Some, overrides packet.vel_d (NED down, m/s, positive = descending)
    /// after read_sensors() so GPS velocity reflects the simulated altitude profile.
    pub sim_vel_d_override: Option<f64>,

    /// Sim only: if Some, forces key_armed after read_sensors() overwrites it from
    /// the physical GPIO pin. Required for no-hardware Startup → Standby transition.
    pub sim_key_armed_override: Option<bool>,
}

impl FlightLoop {
    pub fn new(flight_state: FlightState) -> Self {
        // Derive runtime flags implied by the recovered flight mode so that
        // check_transitions doesn't immediately kick a recovered mode back to
        // Startup (e.g. Standby requires key_armed, which isn't otherwise persisted).
        let recovered = flight_state.flight_mode;
        let key_armed = matches!(
            recovered,
            FlightMode::Standby
                | FlightMode::Ascent
                | FlightMode::Coast
                | FlightMode::DrogueDeployed
                | FlightMode::MainDeployed
        );
        let alt_armed = matches!(
            recovered,
            FlightMode::Ascent
                | FlightMode::Coast
                | FlightMode::DrogueDeployed
                | FlightMode::MainDeployed
        );
        let drogue_deployed = matches!(
            recovered,
            FlightMode::DrogueDeployed | FlightMode::MainDeployed
        );
        let main_chutes_deployed = matches!(recovered, FlightMode::MainDeployed);

        // Reconstruct the launch sequence timer from the recovered snapshot so that
        // a crash mid-burn resumes with the remaining MAV time rather than stalling
        // in Ascent forever (the timer is wall-clock and can't survive a reboot).
        let (launch_sequence_stage, launch_stage_start_time) =
            if matches!(recovered, FlightMode::Ascent) {
                let stage = flight_state.snap_launch_stage;
                let elapsed_ms = flight_state.snap_launch_elapsed_ms as u64;
                let stage_limit_ms: u64 = match stage {
                    1 => constants::LAUNCH_SV_PREVENT_MS,
                    2 => constants::LAUNCH_SV_TO_MAV_WAIT_MS,
                    3 => constants::MAV_OPEN_DURATION_MS,
                    _ => 0,
                };
                if stage == 0 || stage >= 4 || elapsed_ms >= stage_limit_ms {
                    // Sequence complete — Done stage will be handled by check_transitions
                    // on the first loop iteration to push mode to Coast.
                    (LaunchStage::Done, None)
                } else {
                    // Backdate the start instant so the remaining duration fires naturally.
                    let backdated = Instant::now()
                        .checked_sub(Duration::from_millis(elapsed_ms))
                        .unwrap_or(Instant::from_ticks(0));
                    let recovered_stage = match stage {
                        1 => LaunchStage::PreVent,
                        2 => LaunchStage::SvToMavWait,
                        _ => LaunchStage::MavOpen,
                    };
                    log::info!(
                        "Launch sequence resumed: stage={} elapsed_ms={}",
                        stage, elapsed_ms
                    );
                    (recovered_stage, Some(backdated))
                }
            } else {
                (LaunchStage::None, None)
            };

        Self {
            flight_state,
            key_armed,
            alt_armed,
            umbilical_state: false,
            umbilical_launch: false,
            mav_open: false,
            sv_open: false,
            camera_deployed: false,
            alt_sum: 0.0,
            airbrakes_init: false,
            drogue_deployed,
            main_chutes_deployed,
            blims_armed: false,
            log_armed: false,
            blims: None,
            // L3: BLiMS target is hard-coded — no ground command.
            blims_target_set: true,
            blims_target_lat: constants::BLIMS_TARGET_LAT,
            blims_target_lon: constants::BLIMS_TARGET_LON,
            blims_wind_from_deg: 0.0,

            // Initialize internal state
            alt_buffer: [0.0; 10],
            alt_index: 0,
            accel_buffer: [0.0; 10],
            accel_index: 0,
            accel_sum: 0.0,
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
            last_full_log: None,
            last_heartbeat: None,
            launch_sequence_stage,
            launch_stage_start_time,
            recovery_vent_sent: false,
            low_alt_time: None,
            n2_low_speed_count: 0,
            n2_sent: false,
            n3_sent: false,
            n4_sent: false,
            fault_signal_sent: false,
            last_alt: 0.0,
            overpressure_triggered: false,
            overpressure_count: 0,
            sim_altitude_override: None,
            sim_vel_d_override: None,
            sim_key_armed_override: None,
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

        // 2a. Sim override: replace altitude + force altimeter VALID.
        //     None in normal flight — zero cost path.
        if let Some(alt_m) = self.sim_altitude_override {
            self.flight_state.packet.altitude = alt_m;
            self.flight_state.altimeter_state = crate::state::SensorState::VALID;
        }

        // Sim override: replace GPS vel_d so payload comms and BLiMS see
        // realistic vertical velocity instead of 0 from a bench GPS reading.
        if let Some(vel_d) = self.sim_vel_d_override {
            self.flight_state.packet.vel_d = vel_d;
        }

        // Sim override: force key_armed after read_sensors() overwrites it from
        // the physical arming switch pin so the sim can run without hardware.
        if let Some(armed) = self.sim_key_armed_override {
            self.flight_state.key_armed = armed;
            self.key_armed = armed;
        }

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
                time:     self.flight_state.packet.ms_since_boot_cfc as f32 / 1000.0,
                altitude: self.flight_state.packet.altitude,
                vel_d:    self.flight_state.packet.vel_d as f32,
                reference_pressure: self.flight_state.reference_pressure,
                gyro_x:   self.flight_state.packet.gyro_x,
                gyro_y:   self.flight_state.packet.gyro_y,
                gyro_z:   self.flight_state.packet.gyro_z,
                accel_x:  self.flight_state.packet.accel_x,
                accel_y:  self.flight_state.packet.accel_y,
                accel_z:  self.flight_state.packet.accel_z,
                phase,
            });
        }

        // 2c. Overpressure latch: if PT3 exceeds the threshold, open SV and
        // force Fault. Checked every cycle regardless of flight mode so tank
        // overpressure before launch is handled the same as during flight.
        self.check_overpressure().await;

        // 3. Process logic and transitions
        self.check_transitions().await;
        self.handle_launch_sequence().await;

        // Sync packet flight mode after transitions so telemetry always reflects
        // the mode that was active when the data was produced, not the mode from
        // the start of the cycle before check_transitions() ran.
        self.flight_state.packet.flight_mode = self.flight_state.flight_mode as u32;

        // Sync launch sequence info into FlightState so all snapshot writes (periodic
        // and transition-triggered) capture the current stage and elapsed time.
        self.flight_state.snap_launch_stage = self.launch_sequence_stage as u32;
        self.flight_state.snap_launch_elapsed_ms = self
            .launch_stage_start_time
            .map(|t| t.elapsed().as_millis() as u32)
            .unwrap_or(0);

        // 4. Update actuators
        self.flight_state.update_actuators().await;

        // 5. Transmit telemetry (radio + USB umbilical binary frames)
        self.flight_state.transmit().await;

        // Save packet to QSPI Flash
        let now = Instant::now();

        // 6. Payload Heartbeat (1 Hz)
        let should_heartbeat = match self.last_heartbeat {
            None => true,
            Some(last) => now.duration_since(last).as_millis() >= 1000,
        };
        if should_heartbeat {
            let _ = self.flight_state.payload_uart.write(b"A\n").await;
            self.last_heartbeat = Some(now);
        }
        let should_log = match self.last_flash_log {
            None => true,
            Some(last) => {
                now.duration_since(last).as_millis() >= constants::FLASH_LOGGING_PERIOD_MS
            }
        };

        if should_log {
            let write_full = match self.last_full_log {
                None => true,
                Some(last) => {
                    now.duration_since(last).as_millis() >= constants::FULL_LOGGING_PERIOD_MS
                }
            };
            self.flight_state.save_packet_to_flash(write_full).await;
            self.last_flash_log = Some(now);
            if write_full {
                self.last_full_log = Some(now);
            }
        }

        // Snapshot ring: throttled to 1 Hz internally, runs in every mode.
        self.flight_state.log_to_fram().await;

        // Track previous altitude for fault velocity estimate (A2 payload signal)
        self.last_alt = self.flight_state.packet.altitude;
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
                        self.flight_state.packet.cmd_n1 = 1;
                    }
                }
                Command::N2 => {
                    let _ = self.flight_state.payload_uart.write(b"N2\n").await;
                    log::info!("PAYLOAD: Sent N2");
                    self.flight_state.packet.cmd_n2 = 1;
                }
                Command::N3 => {
                    let _ = self.flight_state.payload_uart.write(b"N3\n").await;
                    log::info!("PAYLOAD: Sent N3");
                    self.n3_sent = true;
                    self.flight_state.packet.cmd_n3 = 1;
                }
                Command::N4 => {
                    let _ = self.flight_state.payload_uart.write(b"N4\n").await;
                    log::info!("PAYLOAD: Sent N4");
                    self.n4_sent = true;
                    self.flight_state.packet.cmd_n4 = 1;
                }
                Command::A1 => {
                    log::warn!("A1");
                    let _ = self.flight_state.payload_uart.write(b"A1\n").await;
                    self.flight_state.packet.cmd_a1 = 1;
                }
                Command::A2 => {
                    log::warn!("A2");
                    let _ = self.flight_state.payload_uart.write(b"A2\n").await;
                    self.flight_state.packet.cmd_a2 = 1;
                }
                Command::A3 => {
                    log::warn!("A3");
                    let _ = self.flight_state.payload_uart.write(b"A3\n").await;
                    self.flight_state.packet.cmd_a3 = 1;
                }
                Command::ForceMode(mode_val) => {
                    log::warn!("CMD: Force Flight Mode {}", mode_val);
                    self.set_flight_mode(FlightMode::from_u32(mode_val));
                }
            }
        }
    }

    pub async fn check_umbilical_commands(&mut self) {
        // L3: umbilical commanding removed — drain queue and discard.
        while let Some(_cmd) = umbilical::try_recv_command() {}
        return;

        #[allow(unreachable_code)]
        while let Some(cmd) = umbilical::try_recv_command() {
            match cmd {
                UmbilicalCommand::Launch => {
                    log::warn!("UMBILICAL CMD: Launch received");
                    self.set_launch_command(true);
                }
                UmbilicalCommand::OpenMav => {
                    // TODO: CHANGE THIS TO MAV DELAY LATER FOR WET DRESS
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
                    log::warn!("UMBILICAL CMD: Safe — closing MAV, opening SV to vent");
                    self.flight_state.close_mav().await;
                    self.flight_state.open_sv(0).await;
                    self.mav_open = false;
                    self.sv_open = true;
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
                        self.flight_state.packet.cmd_n1 = 1;
                    }
                }
                UmbilicalCommand::PayloadN2 => {
                    let _ = self.flight_state.payload_uart.write(b"N2\n").await;
                    log::info!("UMBILICAL: Sent N2");
                    self.flight_state.packet.cmd_n2 = 1;
                }
                UmbilicalCommand::PayloadN3 => {
                    let _ = self.flight_state.payload_uart.write(b"N3\n").await;
                    log::info!("UMBILICAL: Sent N3");
                    self.n3_sent = true;
                    self.flight_state.packet.cmd_n3 = 1;
                }
                UmbilicalCommand::PayloadN4 => {
                    let _ = self.flight_state.payload_uart.write(b"N4\n").await;
                    log::info!("UMBILICAL: Sent N4");
                    self.n4_sent = true;
                    self.flight_state.packet.cmd_n4 = 1;
                }
                UmbilicalCommand::PayloadA1 => {
                    log::warn!("UMBILICAL: Sent A1");
                    let _ = self.flight_state.payload_uart.write(b"A1\n").await;
                    self.flight_state.packet.cmd_a1 = 1;
                }
                UmbilicalCommand::PayloadA2 => {
                    log::warn!("UMBILICAL: Sent A2");
                    let _ = self.flight_state.payload_uart.write(b"A2\n").await;
                    self.flight_state.packet.cmd_a2 = 1;
                }
                UmbilicalCommand::PayloadA3 => {
                    log::warn!("UMBILICAL: Sent A3");
                    let _ = self.flight_state.payload_uart.write(b"A3\n").await;
                    self.flight_state.packet.cmd_a3 = 1;
                }
                UmbilicalCommand::WipeFramReboot => {
                    log::warn!("UMBILICAL CMD: Wipe Flash + FRAM and Reboot");
                    self.flight_state.wipe_flash_storage().await;
                    self.flight_state.reset_fram().await;
                    cortex_m::peripheral::SCB::sys_reset();
                }
                UmbilicalCommand::KeyArm => {
                    log::warn!("UMBILICAL CMD: Key Arm");
                    self.key_armed = true;
                }
                UmbilicalCommand::KeyDisarm => {
                    log::warn!("UMBILICAL CMD: Key Disarm");
                    self.key_armed = false;
                }
                UmbilicalCommand::SetBlimsTarget { lat, lon } => {
                    log::warn!("UMBILICAL CMD: Set BLiMS target lat={} lon={}", lat, lon);
                    self.set_blims_target(lat, lon);
                    self.blims_target_set = true;
                }
                UmbilicalCommand::TriggerDrogue => {
                    log::warn!("UMBILICAL CMD: Trigger Drogue");
                    self.flight_state.trigger_drogue().await;
                }
                UmbilicalCommand::TriggerMain => {
                    log::warn!("UMBILICAL CMD: Trigger Main");
                    self.flight_state.trigger_main().await;
                }
                /*
                UmbilicalCommand::DrogueMode => {  // Remove only for testing
                    log::warn!("UMBILICAL CMD: Force Drogue Mode");
                    self.set_flight_mode(FlightMode::DrogueDeployed);
                }
                UmbilicalCommand::MainMode => {
                    log::warn!("UMBILICAL CMD: Force Main Mode");
                    self.set_flight_mode(FlightMode::MainDeployed);
                }
                */
                UmbilicalCommand::DeployAirbrakes => {
                    log::warn!("UMBILICAL CMD: Deploy Airbrakes");
                    self.flight_state.airbrake_system.enable();
                    self.flight_state.airbrake_system.set_deployment(1.0);
                    self.flight_state.packet.airbrake_deployment = 1.0;
                }
                UmbilicalCommand::RetractAirbrakes => {
                    log::warn!("UMBILICAL CMD: Retract Airbrakes");
                    self.flight_state.airbrake_system.set_deployment(0.0);
                    self.flight_state.packet.airbrake_deployment = 0.0;
                }
                UmbilicalCommand::TriggerBLiMS => {
                    log::warn!("UMBILICAL CMD: Trigger BLiMS nudge");
                    self.trigger_blims().await;
                }
            }
        }
    }

    /// If PT3 (scaled PSI) exceeds `PT3_OVERPRESSURE_THRESHOLD` for 3
    /// consecutive cycles, latch SV open and force Fault. One-shot: once
    /// fired, further calls are a no-op so sensor noise can't re-issue
    /// commands or overwrite mode decisions.
    pub async fn check_overpressure(&mut self) {
        // L3: no ADC, no PT3 — overpressure detection disabled.
        return;
        #[allow(unreachable_code)]
        if self.overpressure_triggered {
            return;
        }
        let pt3 = self.flight_state.packet.pt3;
        if pt3 > constants::PT3_OVERPRESSURE_THRESHOLD {
            self.overpressure_count = self.overpressure_count.saturating_add(1);
            log::warn!(
                "OVERPRESSURE: PT3 = {:.1} > {:.1} (count {}/3)",
                pt3,
                constants::PT3_OVERPRESSURE_THRESHOLD,
                self.overpressure_count,
            );
            if self.overpressure_count >= 3 {
                log::error!(
                    "OVERPRESSURE LATCHED: opening SV and transitioning to Fault"
                );
                // Open SV with no auto-close — it stays open for the rest of the flight.
                self.flight_state.open_sv(0).await;
                self.sv_open = true;
                self.flight_state.flight_mode = FlightMode::Fault;
                self.flight_state.write_packet_to_fram().await;
                self.overpressure_triggered = true;
            }
        } else {
            // Reset on any in-range sample so the three spikes must be consecutive.
            self.overpressure_count = 0;
        }
    }

    pub async fn check_transitions(&mut self) {
        // Retrieve current values for easier access
        let _packet = &self.flight_state.packet;
        let _mode = self.flight_state.flight_mode;

        // L3: SV removed — recovery vent disabled.

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
                if self.flight_state.altimeter_state == crate::state::SensorState::INVALID {
                    self.alt_armed = false;
                    self.flight_state.flight_mode = FlightMode::Fault;
                    self.flight_state.write_packet_to_fram().await;
                    log::error!("Altimeter invalid at Startup; transitioning to Fault");
                    return;
                }
                // L3: arming is driven solely by GPIO 41 (CFC_ARM). High = armed.
                if self.flight_state.cfc_arm_active
                    && self.flight_state.altimeter_state == crate::state::SensorState::VALID
                {
                    self.alt_armed = true;
                    self.flight_state.arming_altitude = self.flight_state.read_altimeter();
                    log::info!(
                        "Arming altitude set to {}",
                        self.flight_state.arming_altitude
                    );

                    // L3: arm payload on Standby entry — send N1 (camera deploy).
                    let _ = self.flight_state.payload_uart.write(b"N1\n").await;
                    self.flight_state.packet.cmd_n1 = 1;
                    log::info!("PAYLOAD: Sent N1 (arm) on Standby entry");

                    self.flight_state.flight_mode = FlightMode::Standby;
                    self.flight_state.write_packet_to_fram().await;
                    log::info!("Transitioning to Standby (CFC_ARM high)");
                }
            }
            FlightMode::Standby => {
                if self.flight_state.altimeter_state != crate::state::SensorState::VALID {
                    self.alt_armed = false;
                    self.flight_state.flight_mode = FlightMode::Fault;
                    self.flight_state.write_packet_to_fram().await;
                    log::error!("Altimeter invalid at Standby; transitioning to Fault");
                    return;
                }

                // L3: if GPIO 41 (CFC_ARM) goes low while in Standby, drop back to Startup.
                if !self.flight_state.cfc_arm_active {
                    self.flight_state.flight_mode = FlightMode::Startup;
                    self.flight_state.write_packet_to_fram().await;
                    log::info!("CFC_ARM low in Standby; transitioning back to Startup");
                    return;
                }

                // L3 launch detect: 10-sample moving avg of Y-axis accel > 4 g.
                let ay = self.flight_state.packet.accel_y;
                self.accel_sum -= self.accel_buffer[self.accel_index];
                self.accel_buffer[self.accel_index] = ay;
                self.accel_sum += ay;
                self.accel_index = (self.accel_index + 1) % 10;
                let avg_ay_ms2 = self.accel_sum / 10.0;
                let threshold_ms2 =
                    constants::LAUNCH_ACCEL_Y_THRESHOLD_G * constants::G_TO_MS2;

                if avg_ay_ms2 > threshold_ms2 {
                    log::warn!(
                        "LAUNCH DETECTED: avg accel_y = {:.2} m/s² (> {:.2})",
                        avg_ay_ms2, threshold_ms2
                    );
                    self.flight_state.reference_pressure = self.flight_state.read_barometer();
                    self.alt_armed = true;
                    // Use existing MavOpen stage as the 3500 ms Ascent timer.
                    // No SV/MAV actuation — just reuses the timer + Ascent→Coast hand-off.
                    self.launch_sequence_stage = LaunchStage::MavOpen;
                    self.launch_stage_start_time = Some(Instant::now());
                    self.flight_state.flight_mode = FlightMode::Ascent;
                    self.flight_state.write_packet_to_fram().await;
                    log::info!("Transitioning to Ascent (3.2s timer to Coast)");
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
                    self.flight_state.write_packet_to_fram().await;
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
            }

            FlightMode::Coast => {

                if self.flight_state.altimeter_state != crate::state::SensorState::VALID {
                    // altimeter is not working
                    self.alt_armed = false;
                    self.flight_state.flight_mode = FlightMode::Fault;
                    self.flight_state.write_packet_to_fram().await;
                    log::error!("Altimeter invalid at Coast; transitioning to Fault");
                    return;
                }


                if self.flight_state.altimeter_state == SensorState::VALID && self.alt_armed {
                    // Capture oldest value (≈0.5 s ago at 20 Hz) before overwrite
                    let alt_half_sec_ago = self.alt_buffer[self.alt_index];
                    // Remove old value from sum
                    self.alt_sum -= alt_half_sec_ago;
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

                    // Read latest airbrake deployment from Core 1
                    // and drive the ODrive RC PWM servo
                    let deployment = crate::airbrake_task::get_deployment();
                    log::info!("Airbrake deployment from Core 1: {:.1}%", deployment * 100.0);
                    self.flight_state.airbrake_system.set_deployment(deployment);
                    self.flight_state.packet.predicted_apogee = crate::airbrake_task::get_predicted_apogee();
                    self.flight_state.packet.airbrake_deployment = deployment;
                    log::info!("Airbrake deployment: {:.1}%", deployment * 100.0);

                    // N2: vertical speed < 50 ft/s for 5 consecutive loops, only above arming altitude
                    // Slope over 0.5 s (10 loops) smooths altimeter noise vs. frame-to-frame diff
                    let vert_speed_ft_s = (current_alt - alt_half_sec_ago) * 2.0 * 3.28084;
                    if current_alt > constants::N2_ARM_ALTITUDE_M && !self.n2_sent {
                        if vert_speed_ft_s < 50.0 {
                            self.n2_low_speed_count += 1;
                            if self.n2_low_speed_count >= 5 {
                                let _ = self.flight_state.payload_uart.write(b"N2\n").await;
                                log::info!("PAYLOAD: Sent N2 (vert={:.1} ft/s)", vert_speed_ft_s);
                                self.n2_sent = true;
                                self.flight_state.packet.cmd_n2 = 1;
                            }
                        } else {
                            self.n2_low_speed_count = 0;
                        }
                    }

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
                        self.camera_deployed = true;
                        // Airbrakes retract at apogee
                        // Coast signals so it will hold at 0.0 deployment
                        self.flight_state.airbrake_system.retract();
                        //self.flight_state.airbrake_system.disable();
                        self.airbrakes_init = false;
                        self.flight_state.packet.airbrake_deployment = 0.0;
                        log::info!("Airbrakes retracted");
                        log::info!("Cameras deployed");
                        log::info!("Apogee reached at {:.2} m", self.filtered_alt[1]);

                        // Deploy Drogue
                        self.flight_state.trigger_drogue().await;
                        self.flight_state.packet.ssa_drogue_deployed = 1;

                        log::info!("Drogue deployed");
                        self.drogue_deployed = true;
                        self.flight_state.flight_mode = FlightMode::DrogueDeployed;
                        self.flight_state.write_packet_to_fram().await;
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
                    self.flight_state.write_packet_to_fram().await;
                    log::error!("Altimeter invalid at DrogueDeployed; transitioning to Fault");
                    return;
                }

                // N3: altitude < 76.2m (250ft) for 1s
                if self.flight_state.packet.altitude < 76.2 && !self.n3_sent {
                    if self.low_alt_time.is_none() {
                        self.low_alt_time = Some(Instant::now());
                    } else if self.low_alt_time.unwrap().elapsed().as_millis() >= 1000 {
                        let _ = self.flight_state.payload_uart.write(b"N3\n").await;
                        log::info!("PAYLOAD: Sent N3");
                        self.n3_sent = true;
                        self.flight_state.packet.cmd_n3 = 1;
                    }
                } else if self.flight_state.packet.altitude >= 76.2 {
                    self.low_alt_time = None;
                }

                // Get time since entry
                if let Some(entry_time) = self.drogue_entry_time {
                    if entry_time.elapsed().as_millis() >= constants::MAIN_DEPLOY_DELAY_MS {
                        // L3: deploy main below 2000 ft AGL. Altimeter is in meters, convert.
                        let alt_ft = self.flight_state.read_altimeter() * 3.28084;
                        if alt_ft < constants::MAIN_DEPLOY_ALTITUDE_FT {
                            // Deploy Main
                            self.flight_state.trigger_main().await;
                            self.flight_state.packet.ssa_main_deployed = 1;

                            self.main_chutes_deployed = true;
                            log::info!("Main deployed");
                            self.flight_state.flight_mode = FlightMode::MainDeployed;
                            self.flight_state.write_packet_to_fram().await;
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
                    self.flight_state.write_packet_to_fram().await;
                    log::error!("Altimeter invalid at MainDeployed; transitioning to Fault");
                    return;
                }

                // N3: altitude < 76.2m (250ft) for 1s
                if self.flight_state.packet.altitude < 76.2 && !self.n3_sent {
                    if self.low_alt_time.is_none() {
                        self.low_alt_time = Some(Instant::now());
                    } else if self.low_alt_time.unwrap().elapsed().as_millis() >= 1000 {
                        let _ = self.flight_state.payload_uart.write(b"N3\n").await;
                        log::info!("PAYLOAD: Sent N3");
                        self.n3_sent = true;
                        self.flight_state.packet.cmd_n3 = 1;
                    }
                } else if self.flight_state.packet.altitude >= 76.2 {
                    self.low_alt_time = None;
                }

                // N4: any accel axis > 50 m/s²
                if !self.n4_sent {
                    let ax = self.flight_state.packet.accel_x;
                    let ay = self.flight_state.packet.accel_y;
                    let az = self.flight_state.packet.accel_z;
                    if ax.abs() > 50.0 || ay.abs() > 50.0 || az.abs() > 50.0 {
                        let _ = self.flight_state.payload_uart.write(b"N4\n").await;
                        log::info!("PAYLOAD: Sent N4");
                        self.n4_sent = true;
                        self.flight_state.packet.cmd_n4 = 1;
                    }
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

                // BLiMS
                if let Some(blims) = &mut self.blims {
                    // Arm guidance on first entry to MainDeployed only if a target
                    // has been set via the umbilical SetBlimsTarget command. If the
                    // ground never set a target, BLiMS stays disarmed — we will not
                    // fly to a default coordinate.
                    if !self.blims_armed {
                        if self.blims_target_set {
                            blims.set_target(self.blims_target_lat, self.blims_target_lon);
                            self.blims_armed = true;
                            log::info!(
                                "BLiMS: target set lat={} lon={}, guidance active",
                                self.blims_target_lat,
                                self.blims_target_lon
                            );
                        } else {
                            log::error!(
                                "BLiMS: no target set from ground; guidance disabled"
                            );
                        }
                    }

                    if self.blims_armed {
                        let p = &self.flight_state.packet;
                        let data_in = BlimsDataIn {
                            lat:         (p.latitude  * 1e7_f32) as i32,
                            lon:         (p.longitude * 1e7_f32) as i32,
                            altitude_ft:  p.altitude * 3.28084_f32,
                            fix_type:     p.fix_type,
                            gps_state:    p.num_satellites > 0,
                            head_mot:     p.head_mot,
                            vel_n:        p.vel_n as i32,
                            vel_e:        p.vel_e as i32,
                            vel_d:        p.vel_d as i32,
                            g_speed:      p.g_speed as i32,
                            h_acc:        p.h_acc,
                            v_acc:        p.v_acc,
                            s_acc:        p.s_acc,
                            head_acc:     p.head_acc,
                        };

                        let out = blims.execute(&data_in);
                        let p = &mut self.flight_state.packet;
                        p.blims_motor_position   = out.motor_position;
                        p.blims_phase_id         = out.phase_id;
                        p.blims_pid_p            = out.pid_p;
                        p.blims_pid_i            = out.pid_i;
                        p.blims_bearing          = out.bearing;
                        p.blims_loiter_step      = out.loiter_step;
                        p.blims_heading_des      = out.heading_des;
                        p.blims_heading_error    = out.heading_error;
                        p.blims_error_integral   = out.error_integral;
                        p.blims_dist_to_target_m = out.dist_to_target_m;
                        p.blims_target_lat       = self.blims_target_lat;
                        p.blims_target_lon       = self.blims_target_lon;
                        p.blims_wind_from_deg    = self.blims_wind_from_deg;
                    }
                }
            }
            FlightMode::Fault => {
                if !self.fault_signal_sent {
                    if self.drogue_deployed {
                        match self.flight_state.payload_uart.write(b"A3\n").await {
                            Ok(_) => {
                                log::warn!("A3 — fault after apogee");
                                self.flight_state.packet.cmd_a3 = 1;
                            }
                            Err(e) => log::error!("PAYLOAD UART write A3 failed: {:?}", e),
                        }
                    } else {
                        let alt = self.flight_state.packet.altitude;
                        let vel = (alt - self.last_alt) * 20.0;
                        let mut buf = heapless::String::<32>::new();
                        let _ = core::fmt::write(&mut buf, format_args!("A2,{:.1},{:.1}\n", alt, vel));
                        match self.flight_state.payload_uart.write(buf.as_bytes()).await {
                            Ok(_) => {
                                log::warn!("A2 — fault before apogee");
                                self.flight_state.packet.cmd_a2 = 1;
                            }
                            Err(e) => log::error!("PAYLOAD UART write A2 failed: {:?}", e),
                        }
                    }
                    self.fault_signal_sent = true;
                }
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
        //self.key_armed = armed;
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

    pub fn set_blims(&mut self, blims: blims::Blims<'static>) {
        self.blims = Some(blims);
    }

    /// Set the BLiMS landing-zone target. Updates the stored coordinates and,
    /// if the BLiMS hardware is wired, pushes the target into the controller
    /// so a retarget mid-flight takes effect immediately. Arming (enabling
    /// guidance execution) is decided separately on entry to MainDeployed.
    pub fn set_blims_target(&mut self, lat: f32, lon: f32) {
        self.blims_target_lat = lat;
        self.blims_target_lon = lon;
        self.blims_target_set = true;
        self.flight_state.packet.blims_target_lat = lat;
        self.flight_state.packet.blims_target_lon = lon;
        if let Some(b) = &mut self.blims {
            b.set_target(lat, lon);
        }
    }

    /// Nudge the BLiMS motor slightly right then return to neutral.
    /// Safe to call at any time — keeps movement small to protect the airframe.
    pub async fn trigger_blims(&mut self) {
        use blims::blims_constants::{LOITER_RIGHT_POS, NEUTRAL_POS};
        if let Some(b) = &mut self.blims {
            b.set_motor_raw(LOITER_RIGHT_POS); // 0.65 — slight right
            Timer::after_millis(1_000).await;
            b.set_motor_raw(NEUTRAL_POS);       // 0.5 — back to neutral
            log::info!("BLiMS nudge complete → neutral");
        } else {
            log::warn!("BLiMS nudge: no BLiMS hardware attached");
        }
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
                // L3: SV/MAV actuation removed — stage unreachable, no-op.
                self.launch_sequence_stage = LaunchStage::None;
            }
            LaunchStage::SvToMavWait => {
                // L3: SV/MAV actuation removed — stage unreachable, no-op.
                self.launch_sequence_stage = LaunchStage::None;
            }
            LaunchStage::MavOpen => {
                // L3: repurposed as fixed Ascent→Coast timer (MAV_OPEN_DURATION_MS).
                // No MAV actuation occurs.
                if let Some(start) = self.launch_stage_start_time {
                    if sequence_now.duration_since(start).as_millis()
                        >= constants::MAV_OPEN_DURATION_MS
                    {
                        // TRANSITION TO COAST if currently in Ascent
                        if self.flight_state.flight_mode == FlightMode::Ascent {
                            log::warn!("Ascent timer complete; transitioning Ascent → Coast.");
                            self.flight_state.airbrake_system.enable();
                            // Seed alt_buffer with the current altitude so the 10-entry
                            // moving average starts at the real value, not 0m.
                            // Without this the first 10 Coast cycles produce a spurious
                            // low→high ramp that delays or distorts apogee detection.
                            let cur = self.flight_state.packet.altitude;
                            self.alt_buffer = [cur; 10];
                            self.alt_sum = cur * 10.0;
                            self.flight_state.flight_mode = FlightMode::Coast;
                            self.flight_state.write_packet_to_fram().await;
                        }

                        self.launch_sequence_stage = LaunchStage::Done;
                        self.launch_stage_start_time = None;
                    }
                }
            }
            LaunchStage::Done => {
                // Handles recovery: if we rebooted mid-sequence and restored Done,
                // push to Coast on the first iteration rather than waiting forever.
                if self.flight_state.flight_mode == FlightMode::Ascent {
                    log::warn!("Launch sequence Done on recovery; transitioning Ascent → Coast.");
                    self.flight_state.airbrake_system.enable();
                    let cur = self.flight_state.packet.altitude;
                    self.alt_buffer = [cur; 10];
                    self.alt_sum = cur * 10.0;
                    self.flight_state.flight_mode = FlightMode::Coast;
                    self.flight_state.write_packet_to_fram().await;
                }
            }
            LaunchStage::None => {}
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
