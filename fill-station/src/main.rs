mod command;
mod hardware;
mod components;
mod csv_logger;
mod mqtt_publisher;
use anyhow::Result;
use async_tungstenite::{WebSocketStream, tungstenite};
use smol::Async;
use smol::stream::StreamExt;
use smol::Timer;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::time::{SystemTime, UNIX_EPOCH};
use smol::lock::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::{Instrument, Level, debug, error, info, span, warn};
use tracing_subscriber::fmt;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tungstenite::Message;

use crate::command::{ActuatorState, AdcReadings, Command, CommandResponse, UmbilicalReadings};
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::command::ChannelReading;
use crate::hardware::Hardware;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::components::umbilical::FswTelemetry;

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::components::ads1015::{Channel, Gain, DataRate};

// ============================================================================
// ADC CONFIGURATION - Easy to modify
// ============================================================================

/// ADC sampling rate in Hz (samples per second)
const ADC_SAMPLE_RATE_HZ: u64 = 100;

// ADC configuration constants - only needed on Linux/Android
#[cfg(any(target_os = "linux", target_os = "android"))]
const ADC_GAIN: Gain = Gain::One; // ±4.096V range

#[cfg(any(target_os = "linux", target_os = "android"))]
const ADC_DATA_RATE: DataRate = DataRate::Sps3300; // Maximum speed

/// Maximum retry attempts for failed ADC reads before logging error
const ADC_MAX_RETRIES: u32 = 5;



/// Delay between retry attempts (milliseconds)
const ADC_RETRY_DELAY_MS: u64 = 10;

/// PT1 scaling (ADC1 Ch0) — 0-1500 PSI range
/// Formula: scaled = raw * SCALE + OFFSET
const PT1500_SCALE: f32 = 0.909754;
const PT1500_OFFSET: f32 = 5.08926;

/// PT2 scaling (ADC1 Ch1) — 0-1000 PSI range
/// Formula: scaled = raw * SCALE + OFFSET
const PT1000_SCALE: f32 = 0.6125;
const PT1000_OFFSET: f32 = 5.0;

/// Load Cell scaling (ADC2 Ch1)
/// Formula: scaled = raw * SCALE + OFFSET
const LOADCELL_SCALE: f32 = 0.264;
const LOADCELL_OFFSET: f32 = -14.9;

// ============================================================================
// UMBILICAL CONFIGURATION
// ============================================================================

/// Serial device path for the umbilical CDC-ACM port
#[cfg(any(target_os = "linux", target_os = "android"))]
const UMBILICAL_DEVICE: &str = "/dev/ttyACM0";

/// Baud rate for umbilical serial communication.
/// Note: baud rate is ignored for USB CDC-ACM; kept for serialport crate API.
#[cfg(any(target_os = "linux", target_os = "android"))]
const UMBILICAL_BAUD: u32 = 115200;

/// Read timeout for serial port (milliseconds)
#[cfg(any(target_os = "linux", target_os = "android"))]
const UMBILICAL_READ_TIMEOUT_MS: u64 = 200;

/// Max age of the most recent `$TELEM` line before the umbilical is considered
/// disconnected, even if the underlying serial port is still open. Catches
/// FSW hangs where USB stays up but the flight loop has stalled.
const TELEM_FRESHNESS_MS: u64 = 3_000;

// ============================================================================
// SHARED ADC STATE
// ============================================================================



fn main() -> Result<()> {
    // Create a log layer for file output
    #[cfg(target_os = "linux")]
    let log_dir = "/tmp/fill-station/logs";
    #[cfg(not(target_os = "linux"))]
    let log_dir = "logs";

    let file_appender = tracing_appender::rolling::hourly(log_dir, "tracing.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    let file_layer = fmt::layer().with_writer(non_blocking).with_ansi(false); // Disable colors in file

    // Create a log layer for stdout
    let stdout_layer = fmt::layer().with_writer(std::io::stdout);

    // Combine both layers and enable logging
    tracing_subscriber::registry()
        .with(file_layer)
        .with(stdout_layer)
        .init();

    smol::block_on(async {
        info!("Initializing fill station...");
        let hardware = Arc::new(Mutex::new(Hardware::new().await?));

        // Create shared ADC readings state
        let adc_readings = Arc::new(Mutex::new(AdcReadings::default()));

        // Create shared actuator state (last-commanded ball valve + QD)
        let actuator_state = Arc::new(ActuatorState::default());

        // Create shared umbilical readings state and command channel
        let umbilical_readings = Arc::new(Mutex::new(UmbilicalReadings::default()));
        let (umb_cmd_tx, umb_cmd_rx) = smol::channel::bounded::<String>(8);

        // Spawn umbilical background task
        info!("Starting Umbilical monitoring task...");
        let umb_task_readings = umbilical_readings.clone();
        smol::spawn(umbilical_task(umb_task_readings, umb_cmd_rx)).detach();

        // Active client tracker
        let active_client_count = Arc::new(AtomicUsize::new(0));

        // Spawn Safety Monitor Task
        info!("Starting Safety Monitor task...");
        let safety_hw = hardware.clone();
        let safety_counts = active_client_count.clone();
        let safety_umb_readings = umbilical_readings.clone();
        let safety_umb_tx = umb_cmd_tx.clone();
        smol::spawn(safety_monitor_task(safety_hw, safety_counts, safety_umb_readings, safety_umb_tx)).detach();

        // Spawn background ADC monitoring task
        info!("Starting ADC monitoring task at {} Hz...", ADC_SAMPLE_RATE_HZ);
        let adc_task_hw = hardware.clone();
        let adc_task_readings = adc_readings.clone();

        smol::spawn(adc_monitoring_task(adc_task_hw, adc_task_readings)).detach();

        // Spawn CSV Logger Task
        let log_hw = hardware.clone();
        let log_adc = adc_readings.clone();
        let log_umb = umbilical_readings.clone();
        smol::spawn(csv_logger::start_logging(log_hw, log_adc, log_umb)).detach();

        // Spawn MQTT Publisher Task
        let mqtt_hw = hardware.clone();
        let mqtt_adc = adc_readings.clone();
        let mqtt_umb = umbilical_readings.clone();
        let mqtt_actuators = actuator_state.clone();
        smol::spawn(mqtt_publisher::start_mqtt_publisher(mqtt_hw, mqtt_adc, mqtt_umb, mqtt_actuators)).detach();

        info!("Initializing web socket server...");
        let listener = Async::<TcpListener>::bind(([0, 0, 0, 0], 9000))?;
        let host = listener.get_ref().local_addr()?;

        info!("Server listening on ws://{}", host);

        loop {
            // Accept incoming connection - don't crash server on error
            let Ok((stream, _)) = listener.accept().await else {
                error!("Failed to accept connection");
                continue;
            };

            let client_ip = stream
                .get_ref()
                .peer_addr()
                .map(|s| s.ip().to_string())
                .unwrap_or("unknown".to_string());

            // Perform WebSocket handshake - don't crash server on error
            let Ok(stream) = async_tungstenite::accept_async(stream).await else {
                error!("WebSocket handshake failed");
                continue;
            };

            // Spawn handler for this connection
            let span = span!(Level::INFO, "websocket", client_ip);
            let hw = hardware.clone();
            let adc = adc_readings.clone();
            let umb = umbilical_readings.clone();
            let umb_tx = umb_cmd_tx.clone();
            let active_clients = active_client_count.clone();
            let actuators = actuator_state.clone();
            smol::spawn(handle_socket(stream, hw, adc, umb, umb_tx, active_clients, actuators).instrument(span)).detach();
        }
    })
}

/// Handle WebSocket connection lifecycle
async fn handle_socket(
    mut stream: WebSocketStream<Async<TcpStream>>,
    hardware: Arc<Mutex<Hardware>>,
    adc_readings: Arc<Mutex<AdcReadings>>,
    umbilical_readings: Arc<Mutex<UmbilicalReadings>>,
    umb_cmd_tx: smol::channel::Sender<String>,
    active_client_count: Arc<AtomicUsize>,
    actuator_state: Arc<ActuatorState>,
) {
    info!("Client connected");
    active_client_count.fetch_add(1, Ordering::SeqCst);

    // Track streaming state and last sent timestamp
    let mut streaming_enabled = false;
    let mut last_sent_timestamp = 0u64;
    let mut fsw_streaming_enabled = false;
    let mut last_sent_fsw_timestamp = 0u64;
    let mut last_heartbeat = Instant::now();
    
    // Small timeout for non-blocking message receive
    let poll_interval = Duration::from_millis(50);
    
    loop {
        // specific check for 15s timeout
        if last_heartbeat.elapsed() > Duration::from_secs(15) {
             error!("Client timed out (no heartbeat for 15s) - disconnecting");
             break;
        }

        // Try to receive a message with timeout
        let msg_future = stream.next();
        let timeout_future = Timer::after(poll_interval);
        
        match smol::future::or(
            async {
                match msg_future.await {
                    Some(msg) => Some(msg),
                    None => None,
                }
            },
            async {
                timeout_future.await;
                None
            }
        ).await {
            Some(Ok(Message::Text(message))) => {
                // Reset heartbeat timer on any valid message
                last_heartbeat = Instant::now();
                let response = process_message(&message, &hardware, &adc_readings, &mut streaming_enabled, &mut fsw_streaming_enabled, &umb_cmd_tx, &actuator_state).await;
                if let Err(e) = send_response(&mut stream, response).await {
                    error!("Error sending message: {}", e);
                    break;
                }
            }
            Some(Ok(Message::Close(_))) => break,
            Some(Ok(_)) => {}
            Some(Err(e)) => {
                error!("Error receiving message: {}", e);
                break;
            }
            None => {
                // Timeout - check if we should send ADC data
                if streaming_enabled {
                    let readings = adc_readings.lock().await;

                    // Send if we have new data
                    if readings.timestamp_ms != last_sent_timestamp {
                        last_sent_timestamp = readings.timestamp_ms;

                        let response = CommandResponse::AdcData {
                            timestamp_ms: readings.timestamp_ms,
                            valid: readings.valid,
                            adc1: readings.adc1,
                            adc2: readings.adc2,
                        };

                        if let Err(e) = send_response(&mut stream, response).await {
                            error!("Error sending ADC stream data: {}", e);
                            break;
                        }
                    }
                }
                // Check if we should send FSW telemetry
                if fsw_streaming_enabled {
                    let umb = umbilical_readings.lock().await;
                    if umb.timestamp_ms != last_sent_fsw_timestamp {
                        last_sent_fsw_timestamp = umb.timestamp_ms;
                        let response = CommandResponse::FswTelemetry {
                            timestamp_ms: umb.timestamp_ms,
                            connected: umb.connected,
                            flight_mode: umb.telemetry.flight_mode_name().to_string(),
                            telemetry: umb.telemetry,
                        };
                        if let Err(e) = send_response(&mut stream, response).await {
                            error!("Error sending FSW telemetry stream data: {}", e);
                            break;
                        }
                    }
                }
            }
        }
    }
    
    info!("Client disconnected");
    active_client_count.fetch_sub(1, Ordering::SeqCst);
}

async fn process_message(
    message: &str,
    hardware: &Arc<Mutex<Hardware>>,
    _adc_readings: &Arc<Mutex<AdcReadings>>,
    streaming_enabled: &mut bool,
    fsw_streaming_enabled: &mut bool,
    umb_cmd_tx: &smol::channel::Sender<String>,
    actuator_state: &Arc<ActuatorState>,
) -> CommandResponse {
    debug!("Received message: {}", message);

    match serde_json::from_str(message) {
        Ok(command) => {
            info!("Received command: {:?}", command);
            execute_command(command, hardware, streaming_enabled, fsw_streaming_enabled, umb_cmd_tx, actuator_state).await
        }
        Err(e) => {
            warn!("Failed to parse command: {}", e);
            CommandResponse::Error
        }
    }
}

async fn execute_command(
    command: Command,
    hardware: &Arc<Mutex<Hardware>>,
    streaming_enabled: &mut bool,
    fsw_streaming_enabled: &mut bool,
    umb_cmd_tx: &smol::channel::Sender<String>,
    actuator_state: &Arc<ActuatorState>,
) -> CommandResponse {
    match command {
        Command::Ignite => {
            let hw_bg = hardware.clone();
            smol::spawn(async move {
                info!("Ignition sequence started (background)...");

                #[cfg(any(target_os = "linux", target_os = "android"))]
                {
                    // 1. Lock and Turn ON
                    {
                        let hw = hw_bg.lock().await;
                        // Use join to set both simultaneously if possible, or just sequential is fine generally 
                        // as await won't block long for GPIO
                        if let Err(e) = hw.ig1.set_actuated(true).await {
                             error!("Failed to actuate igniter 1: {}", e);
                        }
                        if let Err(e) = hw.ig2.set_actuated(true).await {
                             error!("Failed to actuate igniter 2: {}", e);
                        }
                    }

                    // 2. Wait 3 seconds (without lock)
                    Timer::after(Duration::from_secs(3)).await;

                    // 3. Lock and Turn OFF
                    {
                        let hw = hw_bg.lock().await;
                        if let Err(e) = hw.ig1.set_actuated(false).await {
                             error!("Failed to turn off igniter 1: {}", e);
                        }
                        if let Err(e) = hw.ig2.set_actuated(false).await {
                             error!("Failed to turn off igniter 2: {}", e);
                        }
                    }
                }
                #[cfg(not(any(target_os = "linux", target_os = "android")))]
                {
                    let _ = hw_bg;
                    warn!("Ignite command not supported on this platform");
                     // Simulate delay for mock
                    Timer::after(Duration::from_secs(3)).await;
                }
                
                info!("Ignition sequence completed");
            }).detach();

            CommandResponse::Success
        }
        Command::GetIgniterContinuity { id } => {
            #[cfg(any(target_os = "linux", target_os = "android"))]
            {
                let hw = hardware.lock().await;
                let continuity = match id {
                    1 => Some(hw.ig1.has_continuity().await),
                    2 => Some(hw.ig2.has_continuity().await),
                    _ => None,
                };
                
                if let Some(c) = continuity {
                    CommandResponse::IgniterContinuity { id, continuity: c }
                } else {
                    warn!("Invalid igniter ID requested: {}", id);
                    CommandResponse::Error
                }
            }
            #[cfg(not(any(target_os = "linux", target_os = "android")))]
            {
                let _ = hardware;
                warn!("GetIgniterContinuity command not supported on this platform: {}", id);
                CommandResponse::IgniterContinuity { id, continuity: false }
            }
        }
        Command::ActuateValve { valve, open } => {
            #[cfg(any(target_os = "linux", target_os = "android"))]
            {
                let hw = hardware.lock().await;
                let result = match valve.to_lowercase().as_str() {
                    "sv1" => hw.sv1.set_open(open).await,
                    _ => {
                        warn!("Unknown valve: {}", valve);
                        return CommandResponse::Error;
                    }
                };

                match result {
                    Ok(_) => {
                        info!("Valve {} set to {}", valve, if open { "OPEN" } else { "CLOSED" });
                        CommandResponse::Success
                    }
                    Err(e) => {
                        error!("Failed to set valve {} {}: {}", valve, if open { "open" } else { "closed" }, e);
                        CommandResponse::Error
                    }
                }
            }
            #[cfg(not(any(target_os = "linux", target_os = "android")))]
            {
                let _ = hardware;
                warn!("ActuateValve command not supported on this platform: {} -> {}", valve, open);
                CommandResponse::Success
            }
        }
        Command::GetValveState { valve } => {
            #[cfg(any(target_os = "linux", target_os = "android"))]
            {
                let hw = hardware.lock().await;
                let result = match valve.to_lowercase().as_str() {
                    "sv1" => Some((hw.sv1.is_open().await, hw.sv1.check_continuity().await)),
                    _ => None,
                };

                match result {
                    Some((Ok(open), Ok(continuity))) => {
                        CommandResponse::ValveState { valve, open, continuity }
                    }
                    Some((Err(e), _)) => {
                        error!("Failed to get valve state: {}", e);
                        CommandResponse::Error
                    }
                    Some((_, Err(e))) => {
                        error!("Failed to get valve continuity: {}", e);
                        CommandResponse::Error
                    }
                    None => {
                        warn!("Unknown valve: {}", valve);
                        CommandResponse::Error
                    }
                }
            }
            #[cfg(not(any(target_os = "linux", target_os = "android")))]
            {
                let _ = hardware;
                warn!("GetValveState command not supported on this platform: {}", valve);
                CommandResponse::ValveState { valve: valve.to_string(), open: false, continuity: false }
            }
        }
        Command::StartAdcStream => {
            info!("Starting ADC stream for client");
            *streaming_enabled = true;
            CommandResponse::Success
        }
        Command::StopAdcStream => {
            info!("Stopping ADC stream for client");
            *streaming_enabled = false;
            CommandResponse::Success
        }
        Command::BVOpen => {
            let hw = hardware.lock().await;
            info!("Executing BallValve Open Sequence");
            if let Err(e) = hw.ball_valve.open_sequence().await {
                error!("Failed to open ball valve: {}", e);
                CommandResponse::Error
            } else {
                actuator_state.ball_valve_open.store(true, std::sync::atomic::Ordering::Relaxed);
                CommandResponse::Success
            }
        }
        Command::BVClose => {
            let hw = hardware.lock().await;
            info!("Executing BallValve Close Sequence");
            if let Err(e) = hw.ball_valve.close_sequence().await {
                error!("Failed to close ball valve: {}", e);
                CommandResponse::Error
            } else {
                actuator_state.ball_valve_open.store(false, std::sync::atomic::Ordering::Relaxed);
                CommandResponse::Success
            }
        }
        Command::BVSignal { state } => {
             let hw = hardware.lock().await;
             let high = match state.to_lowercase().as_str() {
                 "high" | "open" | "true" => true,
                 "low" | "close" | "false" => false,
                 _ => {
                     warn!("Invalid signal state: {}", state);
                     return CommandResponse::Error;
                 }
             };
             info!("Setting BallValve Signal to {}", if high { "HIGH" } else { "LOW" });
             
             if let Err(e) = hw.ball_valve.set_signal_safe(high).await {
                 error!("Failed to set ball valve signal: {}", e);
                 // If error is due to ON_OFF being high, it will be caught here
                 CommandResponse::Error
             } else {
                 CommandResponse::Success
             }
        }
        Command::BVOnOff { state } => {
             let hw = hardware.lock().await;
             let high = match state.to_lowercase().as_str() {
                 "high" | "on" | "true" => true,
                 "low" | "off" | "false" => false,
                 _ => {
                     warn!("Invalid ON/OFF state: {}", state);
                     return CommandResponse::Error;
                 }
             };
             info!("Setting BallValve ON_OFF to {}", if high { "HIGH" } else { "LOW" });
             
             if let Err(e) = hw.ball_valve.set_on_off(high).await {
                 error!("Failed to set ball valve ON_OFF: {}", e);
                 CommandResponse::Error
             } else {
                 CommandResponse::Success
             }
        }
        Command::QdMove { steps, direction } => {
            let hw_bg = hardware.clone();
            smol::spawn(async move {
                let hw = hw_bg.lock().await;
                if let Err(e) = hw.qd_stepper.move_steps(steps, direction).await {
                    error!("QD move failed: {}", e);
                }
            }).detach();
            CommandResponse::Success
        }
        Command::QdRetract => {
            use crate::components::qd_stepper::{QD_RETRACT_STEPS, QD_RETRACT_DIRECTION};
            let hw_bg = hardware.clone();
            smol::spawn(async move {
                let hw = hw_bg.lock().await;
                if let Err(e) = hw.qd_stepper.move_steps(QD_RETRACT_STEPS, QD_RETRACT_DIRECTION).await {
                    error!("QD retract failed: {}", e);
                }
            }).detach();
            actuator_state.qd_state.store(-1, std::sync::atomic::Ordering::Relaxed);
            CommandResponse::Success
        }
        Command::QdExtend => {
            use crate::components::qd_stepper::{QD_EXTEND_STEPS, QD_EXTEND_DIRECTION};
            let hw_bg = hardware.clone();
            smol::spawn(async move {
                let hw = hw_bg.lock().await;
                if let Err(e) = hw.qd_stepper.move_steps(QD_EXTEND_STEPS, QD_EXTEND_DIRECTION).await {
                    error!("QD extend failed: {}", e);
                }
            }).detach();
            actuator_state.qd_state.store(1, std::sync::atomic::Ordering::Relaxed);
            CommandResponse::Success
        }
        Command::GetBallValveState => {
            CommandResponse::BallValveState {
                open: actuator_state.ball_valve_open.load(std::sync::atomic::Ordering::Relaxed),
            }
        }
        Command::GetQdState => {
            CommandResponse::QdState {
                state: actuator_state.qd_state.load(std::sync::atomic::Ordering::Relaxed),
            }
        }
        Command::Heartbeat => {
            // Heartbeat command just keeps the connection alive
            CommandResponse::Success
        }
        // FSW Umbilical Commands
        Command::FswLaunch => {
            info!("Sending FSW Launch command via umbilical");
            match umb_cmd_tx.try_send("<L>".into()) {
                Ok(_) => CommandResponse::Success,
                Err(e) => { error!("Failed to send FSW command: {}", e); CommandResponse::Error }
            }
        }
        Command::FswOpenMav => {
            info!("Sending FSW Open MAV command via umbilical");
            match umb_cmd_tx.try_send("<M>".into()) {
                Ok(_) => CommandResponse::Success,
                Err(e) => { error!("Failed to send FSW command: {}", e); CommandResponse::Error }
            }
        }
        Command::FswCloseMav => {
            info!("Sending FSW Close MAV command via umbilical");
            match umb_cmd_tx.try_send("<m>".into()) {
                Ok(_) => CommandResponse::Success,
                Err(e) => { error!("Failed to send FSW command: {}", e); CommandResponse::Error }
            }
        }
        Command::FswOpenSv => {
            info!("Sending FSW Open SV command via umbilical");
            match umb_cmd_tx.try_send("<S>".into()) {
                Ok(_) => CommandResponse::Success,
                Err(e) => { error!("Failed to send FSW command: {}", e); CommandResponse::Error }
            }
        }
        Command::FswCloseSv => {
            info!("Sending FSW Close SV command via umbilical");
            match umb_cmd_tx.try_send("<s>".into()) {
                Ok(_) => CommandResponse::Success,
                Err(e) => { error!("Failed to send FSW command: {}", e); CommandResponse::Error }
            }
        }
        Command::FswSafe => {
            info!("Sending FSW Safe command via umbilical");
            match umb_cmd_tx.try_send("<V>".into()) {
                Ok(_) => CommandResponse::Success,
                Err(e) => { error!("Failed to send FSW command: {}", e); CommandResponse::Error }
            }
        }
        Command::FswResetFram => {
            info!("Sending FSW Reset FRAM command via umbilical");
            match umb_cmd_tx.try_send("<F>".into()) {
                Ok(_) => CommandResponse::Success,
                Err(e) => { error!("Failed to send FSW command: {}", e); CommandResponse::Error }
            }
        }
        Command::FswDumpFram => {
            info!("Sending FSW Dump FRAM command via umbilical");
            match umb_cmd_tx.try_send("<f>".into()) {
                Ok(_) => CommandResponse::Success,
                Err(e) => { error!("Failed to send FSW command: {}", e); CommandResponse::Error }
            }
        }
        Command::FswFaultMode => {
            info!("Sending FSW Fault Mode command via umbilical");
            match umb_cmd_tx.try_send("<X>".into()) {
                Ok(_) => CommandResponse::Success,
                Err(e) => { error!("Failed to send FSW command: {}", e); CommandResponse::Error }
            }
        }
        Command::FswResetCard => {
            info!("Sending FSW Reset Card command via umbilical");
            match umb_cmd_tx.try_send("<D>".into()) {
                Ok(_) => CommandResponse::Success,
                Err(e) => { error!("Failed to send FSW command: {}", e); CommandResponse::Error }
            }
        }
        Command::FswReboot => {
            info!("Sending FSW Reboot command via umbilical");
            match umb_cmd_tx.try_send("<R>".into()) {
                Ok(_) => CommandResponse::Success,
                Err(e) => { error!("Failed to send FSW command: {}", e); CommandResponse::Error }
            }
        }
        Command::FswDumpFlash => {
            info!("Sending FSW Dump Flash command via umbilical");
            match umb_cmd_tx.try_send("<G>".into()) {
                Ok(_) => CommandResponse::Success,
                Err(e) => { error!("Failed to send FSW command: {}", e); CommandResponse::Error }
            }
        }
        Command::FswWipeFlash => {
            info!("Sending FSW Wipe Flash command via umbilical");
            match umb_cmd_tx.try_send("<W>".into()) {
                Ok(_) => CommandResponse::Success,
                Err(e) => { error!("Failed to send FSW command: {}", e); CommandResponse::Error }
            }
        }
        Command::FswFlashInfo => {
            info!("Sending FSW Flash Info command via umbilical");
            match umb_cmd_tx.try_send("<I>".into()) {
                Ok(_) => CommandResponse::Success,
                Err(e) => { error!("Failed to send FSW command: {}", e); CommandResponse::Error }
            }
        }
        Command::FswPayloadN1 => {
            info!("Sending FSW Payload N1 command via umbilical");
            match umb_cmd_tx.try_send("<1>".into()) {
                Ok(_) => CommandResponse::Success,
                Err(e) => { error!("Failed to send FSW command: {}", e); CommandResponse::Error }
            }
        }
        Command::FswPayloadN2 => {
            info!("Sending FSW Payload N2 command via umbilical");
            match umb_cmd_tx.try_send("<2>".into()) {
                Ok(_) => CommandResponse::Success,
                Err(e) => { error!("Failed to send FSW command: {}", e); CommandResponse::Error }
            }
        }
        Command::FswPayloadN3 => {
            info!("Sending FSW Payload N3 command via umbilical");
            match umb_cmd_tx.try_send("<3>".into()) {
                Ok(_) => CommandResponse::Success,
                Err(e) => { error!("Failed to send FSW command: {}", e); CommandResponse::Error }
            }
        }
        Command::FswPayloadN4 => {
            info!("Sending FSW Payload N4 command via umbilical");
            match umb_cmd_tx.try_send("<4>".into()) {
                Ok(_) => CommandResponse::Success,
                Err(e) => { error!("Failed to send FSW command: {}", e); CommandResponse::Error }
            }
        }
        Command::StartFswStream => {
            info!("Starting FSW telemetry stream for client");
            *fsw_streaming_enabled = true;
            CommandResponse::Success
        }
        Command::StopFswStream => {
            info!("Stopping FSW telemetry stream for client");
            *fsw_streaming_enabled = false;
            CommandResponse::Success
        }
    }
}

/// Send JSON response back through WebSocket
async fn send_response(
    socket: &mut WebSocketStream<Async<TcpStream>>,
    response: CommandResponse,
) -> Result<()> {
    let json = serde_json::to_string(&response)?;
    socket.send(Message::Text(json.into())).await?;
    Ok(())
}

// ============================================================================
// SAFETY MONITOR
// ============================================================================

async fn safety_monitor_task(
    hardware: Arc<Mutex<Hardware>>,
    active_client_count: Arc<AtomicUsize>,
    umbilical_readings: Arc<Mutex<UmbilicalReadings>>,
    umb_cmd_tx: smol::channel::Sender<String>,
) {
    let mut disconnect_start: Option<Instant> = None;
    let mut safety_triggered = false;
    let mut qd_retract_triggered = false;

    let mut umb_disconnect_start: Option<Instant> = None;
    let mut umb_safety_triggered = false;
    let mut umb_qd_retract_triggered = false;

    loop {
        let count = active_client_count.load(Ordering::SeqCst);

        // Control station disconnect logic
        if count == 0 {
            // If no clients, verify how long we've been disconnected
            if disconnect_start.is_none() {
                info!("No active clients. Starting safety timer.");
                disconnect_start = Some(Instant::now());
                safety_triggered = false;
                qd_retract_triggered = false;
            }

            if let Some(start) = disconnect_start {
                let elapsed = start.elapsed();

                if !safety_triggered && elapsed > Duration::from_secs(15) {
                    warn!("SAFETY TIMEOUT (15s) - Executing Emergency Shutdown");
                    perform_emergency_shutdown(&hardware, &umb_cmd_tx).await;
                    safety_triggered = true;
                }

                if !qd_retract_triggered && elapsed > Duration::from_secs(20) {
                    warn!("SAFETY TIMEOUT (20s) - Retracting QD");
                    {
                        use crate::components::qd_stepper::{QD_RETRACT_STEPS, QD_RETRACT_DIRECTION};
                        let hw = hardware.lock().await;
                        if let Err(e) = hw.qd_stepper.move_steps(QD_RETRACT_STEPS, QD_RETRACT_DIRECTION).await {
                            error!("QD retract during safety timeout failed: {}", e);
                        }
                    }
                    qd_retract_triggered = true;
                }
            }
        } else {
            // Client(s) connected
            if disconnect_start.is_some() {
                info!("Client connected. Safety timer cancelled.");
                disconnect_start = None;
                safety_triggered = false;
                qd_retract_triggered = false;
            }
        }

        // Umbilical disconnect logic.
        // Recompute `connected` from telemetry freshness so a hung FSW (USB up
        // but no $TELEM flowing) correctly reads as disconnected.
        let umb_connected = {
            let mut umb = umbilical_readings.lock().await;
            let fresh = umb.last_telem_instant
                .map(|t| t.elapsed() < Duration::from_millis(TELEM_FRESHNESS_MS))
                .unwrap_or(false);
            umb.connected = fresh;
            fresh
        };
        if !umb_connected {
            if umb_disconnect_start.is_none() {
                info!("Umbilical disconnected. Starting umbilical safety timer.");
                umb_disconnect_start = Some(Instant::now());
                umb_safety_triggered = false;
                umb_qd_retract_triggered = false;
            }

            if let Some(start) = umb_disconnect_start {
                let elapsed = start.elapsed();

                if !umb_safety_triggered && elapsed > Duration::from_secs(15) {
                    warn!("UMBILICAL SAFETY TIMEOUT (15s) - Closing BV, Opening SV1");
                    #[cfg(any(target_os = "linux", target_os = "android"))]
                    {
                        let hw = hardware.lock().await;
                        // Close Ball Valve
                        if let Err(e) = hw.ball_valve.close_sequence().await {
                            error!("Failed to close Ball Valve during umbilical safety: {}", e);
                        }
                        // Open SV1
                        if let Err(e) = hw.sv1.set_open(true).await {
                            error!("Failed to open SV1 during umbilical safety: {}", e);
                        }
                    }
                    #[cfg(not(any(target_os = "linux", target_os = "android")))]
                    {
                        warn!("MOCK UMBILICAL SAFETY (15s) triggered - closing BV, opening SV1");
                    }
                    umb_safety_triggered = true;
                }

                if !umb_qd_retract_triggered && elapsed > Duration::from_secs(20) {
                    warn!("UMBILICAL SAFETY TIMEOUT (20s) - Retracting QD");
                    {
                        use crate::components::qd_stepper::{QD_RETRACT_STEPS, QD_RETRACT_DIRECTION};
                        let hw = hardware.lock().await;
                        if let Err(e) = hw.qd_stepper.move_steps(QD_RETRACT_STEPS, QD_RETRACT_DIRECTION).await {
                            error!("QD retract during umbilical safety timeout failed: {}", e);
                        }
                    }
                    umb_qd_retract_triggered = true;
                }
            }
        } else {
            if umb_disconnect_start.is_some() {
                info!("Umbilical reconnected. Umbilical safety timer cancelled.");
                umb_disconnect_start = None;
                umb_safety_triggered = false;
                umb_qd_retract_triggered = false;
            }
        }

        Timer::after(Duration::from_millis(500)).await;
    }
}

async fn perform_emergency_shutdown(
    hardware: &Arc<Mutex<Hardware>>,
    umb_cmd_tx: &smol::channel::Sender<String>,
) {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        let hw = hardware.lock().await;
        info!("EMERGENCY SHUTDOWN: Closing all Valves");

        // Close SV1
        let _ = hw.sv1.set_open(false).await;

        // Close Ball Valve
        let _ = hw.ball_valve.close_sequence().await;
    }

    // Send FSW Safe command via umbilical to close FSW SV
    info!("EMERGENCY SHUTDOWN: Sending FSW Open SV command via umbilical");
    if let Err(e) = umb_cmd_tx.try_send("<S>".into()) {
        error!("Failed to send FSW Open SV command during emergency shutdown: {}", e);
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        let _ = hardware;
        warn!("MOCK EMERGENCY SHUTDOWN triggered");
    }
}

// ============================================================================
// ADC BACKGROUND TASKS
// ============================================================================

/// Background task that continuously reads ADCs and updates shared state
#[cfg(any(target_os = "linux", target_os = "android"))]
async fn adc_monitoring_task(
    hardware: Arc<Mutex<Hardware>>,
    adc_readings: Arc<Mutex<AdcReadings>>,
) {
    let sample_interval = Duration::from_millis(1000 / ADC_SAMPLE_RATE_HZ);
    let channels = [Channel::Ain0, Channel::Ain1, Channel::Ain2, Channel::Ain3];
    
    info!("ADC monitoring task started");
    
    loop {
        let start = std::time::Instant::now();
        
        // Attempt to read ADCs with retry logic
        match read_all_adcs(&hardware, &channels).await {
            Ok((adc1_readings, adc2_readings)) => {
                // Get current timestamp
                let timestamp_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
                
                // Update shared state
                let mut readings = adc_readings.lock().await;
                readings.timestamp_ms = timestamp_ms;
                readings.valid = true;
                readings.adc1 = adc1_readings;
                readings.adc2 = adc2_readings;
                
                debug!("ADC readings updated successfully");
            }
            Err(e) => {
                error!("Failed to read ADCs after {} retries: {}", ADC_MAX_RETRIES, e);
                
                // Mark readings as invalid
                let mut readings = adc_readings.lock().await;
                readings.valid = false;
                readings.timestamp_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
            }
        }
        
        // Sleep for remainder of sample interval
        let elapsed = start.elapsed();
        if elapsed < sample_interval {
            Timer::after(sample_interval - elapsed).await;
        } else {
            warn!("ADC read took {}ms, longer than {}ms interval", 
                  elapsed.as_millis(), sample_interval.as_millis());
        }
    }
}

/// Read all ADC channels with retry logic
#[cfg(any(target_os = "linux", target_os = "android"))]
async fn read_all_adcs(
    hardware: &Arc<Mutex<Hardware>>,
    channels: &[Channel; 4],
) -> Result<([ChannelReading; 4], [ChannelReading; 4])> {
    let mut last_error = None;
    
    for attempt in 1..=ADC_MAX_RETRIES {
        match try_read_all_adcs(hardware, channels).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if attempt < ADC_MAX_RETRIES {
                    warn!("ADC read attempt {}/{} failed: {}, retrying...", 
                          attempt, ADC_MAX_RETRIES, e);
                    Timer::after(Duration::from_millis(ADC_RETRY_DELAY_MS)).await;
                }
                last_error = Some(e);
            }
        }
    }
    
    Err(last_error.unwrap())
}

/// Attempt to read all ADC channels once
#[cfg(any(target_os = "linux", target_os = "android"))]
async fn try_read_all_adcs(
    hardware: &Arc<Mutex<Hardware>>,
    channels: &[Channel; 4],
) -> Result<([ChannelReading; 4], [ChannelReading; 4])> {
    let mut hw = hardware.lock().await;
    
    let mut adc1_readings = [ChannelReading { raw: 0, voltage: 0.0, scaled: None }; 4];
    let mut adc2_readings = [ChannelReading { raw: 0, voltage: 0.0, scaled: None }; 4];
    
    // Read ADC1 channels
    for (i, &channel) in channels.iter().enumerate() {
        let raw = hw.adc1.read_raw(channel, ADC_GAIN, ADC_DATA_RATE)?;
        let voltage = (raw as f32) * ADC_GAIN.lsb_size();
        
        // PT1 (Ch0): PT1500 scaling, PT2 (Ch1): PT2000 scaling, others: no scaling
        let scaled = match i {
            0 => Some(raw as f32 * PT1500_SCALE + PT1500_OFFSET),
            1 => Some(raw as f32 * PT1000_SCALE + PT1000_OFFSET),
            _ => None,
        };
        
        adc1_readings[i] = ChannelReading { raw, voltage, scaled };
    }
    
    // Read ADC2 channels
    for (i, &channel) in channels.iter().enumerate() {
        let raw = hw.adc2.read_raw(channel, ADC_GAIN, ADC_DATA_RATE)?;
        let voltage = (raw as f32) * ADC_GAIN.lsb_size();
        
        // Load Cell (Ch1): LOADCELL scaling, others: no scaling
        let scaled = if i == 1 {
            Some(raw as f32 * LOADCELL_SCALE + LOADCELL_OFFSET)
        } else {
            None
        };
        
        adc2_readings[i] = ChannelReading { raw, voltage, scaled };
    }
    
    Ok((adc1_readings, adc2_readings))
}

/// Stub for non-Linux platforms
#[cfg(not(any(target_os = "linux", target_os = "android")))]
async fn adc_monitoring_task(
    _hardware: Arc<Mutex<Hardware>>,
    _adc_readings: Arc<Mutex<AdcReadings>>,
) {
    warn!("ADC monitoring not supported on this platform");
    // Just sleep forever
    loop {
        Timer::after(Duration::from_secs(3600)).await;
    }
}

// ============================================================================
// UMBILICAL BACKGROUND TASK
// ============================================================================

/// Background task that manages the serial connection to the FSW Pico 2 via umbilical.
/// Reads text lines from USB serial. Lines prefixed with `$TELEM,` are parsed as
/// CSV telemetry; all other lines are FSW log output (ignored for data purposes).
/// Commands are written as `<X>` tokens to the same serial port.
#[cfg(any(target_os = "linux", target_os = "android"))]
async fn umbilical_task(
    umbilical_readings: Arc<Mutex<UmbilicalReadings>>,
    cmd_rx: smol::channel::Receiver<String>,
) {
    use std::io::Read as _;
    use std::io::Write as _;

    info!("Umbilical task started, looking for device at {}", UMBILICAL_DEVICE);

    loop {
        // Try to open the serial port
        let port = serialport::new(UMBILICAL_DEVICE, UMBILICAL_BAUD)
            .timeout(Duration::from_millis(UMBILICAL_READ_TIMEOUT_MS))
            .open();

        let mut port = match port {
            Ok(p) => {
                info!("Umbilical serial port opened: {}", UMBILICAL_DEVICE);
                p
            }
            Err(e) => {
                debug!("Umbilical not available ({}), retrying in 2s...", e);
                {
                    let mut umb = umbilical_readings.lock().await;
                    umb.connected = false;
                    umb.last_telem_instant = None;
                }
                Timer::after(Duration::from_secs(2)).await;
                continue;
            }
        };

        // Port is open, but DO NOT mark connected yet — we wait for a fresh
        // $TELEM line so a hung FSW with a live USB stack reads as disconnected.
        {
            let mut umb = umbilical_readings.lock().await;
            umb.last_telem_instant = None;
        }

        // CSV line-buffered reader. The FSW emits one telemetry record per
        // line as `$TELEM,<56 fields>\n`; all other lines are FSW log output
        // and are forwarded to debug logs.
        let mut line_buf = String::with_capacity(1024);
        // S4a: discard everything until the second `\n` after a fresh
        // (re)connect so the first line we parse can't be a truncated frame.
        let mut newlines_seen: u8 = 0;
        // Cap line_buf growth in case the FSW hangs mid-line (S5).
        const LINE_BUF_MAX: usize = 8 * 1024;

        // 1 Hz heartbeat to FSW. FSW gates `umbilical_connected` on freshness
        // of these `<H>` tokens (see fsw/src/umbilical.rs).
        let mut last_heartbeat_sent = std::time::Instant::now()
            .checked_sub(Duration::from_secs(2))
            .unwrap_or_else(std::time::Instant::now);
        const HEARTBEAT_INTERVAL: Duration = Duration::from_millis(1000);

        loop {
            // Send heartbeat if due
            if last_heartbeat_sent.elapsed() >= HEARTBEAT_INTERVAL {
                let write_result = smol::unblock({
                    let mut port_clone = port.try_clone().expect("Failed to clone serial port");
                    move || port_clone.write_all(b"<H>")
                }).await;
                if let Err(e) = write_result {
                    error!("Umbilical heartbeat write failed: {}", e);
                    break;
                }
                last_heartbeat_sent = std::time::Instant::now();
            }

            // Check for pending commands to send
            while let Ok(cmd) = cmd_rx.try_recv() {
                let cmd_bytes = cmd.into_bytes();
                let write_result = smol::unblock({
                    let mut port_clone = port.try_clone().expect("Failed to clone serial port");
                    move || port_clone.write_all(&cmd_bytes)
                }).await;
                if let Err(e) = write_result {
                    error!("Umbilical write failed: {}", e);
                    break;
                }
            }

            // Read available bytes from serial port (blocking read wrapped in unblock)
            let read_result = smol::unblock({
                let mut port_clone = port.try_clone().expect("Failed to clone serial port");
                move || {
                    let mut temp = [0u8; 256];
                    match port_clone.read(&mut temp) {
                        Ok(n) => Ok((temp, n)),
                        Err(e) => Err(e),
                    }
                }
            }).await;

            match read_result {
                Ok((data, bytes_read)) => {
                    // Append received bytes as text (lossy — non-UTF8 bytes
                    // become replacement chars; affected frames will fail to
                    // parse and be warn-logged below).
                    let text = String::from_utf8_lossy(&data[..bytes_read]);
                    line_buf.push_str(&text);

                    if line_buf.len() > LINE_BUF_MAX {
                        warn!("umbilical line buffer overflow ({} bytes), resetting", line_buf.len());
                        line_buf.clear();
                        newlines_seen = 0;
                        continue;
                    }

                    while let Some(newline_pos) = line_buf.find('\n') {
                        let line: String = line_buf.drain(..=newline_pos).collect();
                        let line = line.trim();

                        // S4a: skip the first two newline-terminated chunks
                        // after connect — the first is almost certainly a
                        // partial line, the second may also be truncated if
                        // we opened the port mid-record.
                        if newlines_seen < 2 {
                            newlines_seen += 1;
                            continue;
                        }

                        if let Some(csv) = line.strip_prefix("$TELEM,") {
                            let fields: Vec<&str> = csv.split(',').collect();
                            if let Some(telemetry) = FswTelemetry::from_csv(&fields) {
                                let timestamp_ms = SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap()
                                    .as_millis() as u64;

                                {
                                    let mut umb = umbilical_readings.lock().await;
                                    umb.timestamp_ms = timestamp_ms;
                                    umb.telemetry = telemetry;
                                    umb.last_telem_instant = Some(Instant::now());
                                    // Flip connected true eagerly on fresh telemetry;
                                    // safety monitor is the source of truth for staleness.
                                    umb.connected = true;
                                }

                                debug!("FSW telemetry received: mode={}", telemetry.flight_mode_name());
                            } else {
                                warn!("Failed to parse FSW telemetry CSV: {}", line);
                            }
                        } else if !line.is_empty() {
                            debug!("FSW: {}", line);
                        }
                    }
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::TimedOut {
                        // Timeout is normal — just loop again
                        continue;
                    }
                    error!("Umbilical read error: {}, reconnecting...", e);
                    break;
                }
            }
        }

        // Connection lost — mark disconnected and retry
        {
            let mut umb = umbilical_readings.lock().await;
            umb.connected = false;
            umb.last_telem_instant = None;
        }
        warn!("Umbilical disconnected, retrying in 2s...");
        Timer::after(Duration::from_secs(2)).await;
    }
}

/// Stub for non-Linux platforms
#[cfg(not(any(target_os = "linux", target_os = "android")))]
async fn umbilical_task(
    _umbilical_readings: Arc<Mutex<UmbilicalReadings>>,
    _cmd_rx: smol::channel::Receiver<String>,
) {
    warn!("Umbilical not supported on this platform (no serial port)");
    loop {
        Timer::after(Duration::from_secs(3600)).await;
    }
}
