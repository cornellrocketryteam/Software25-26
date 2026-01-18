mod command;
mod hardware;
mod components;
mod csv_logger;
use anyhow::Result;
use async_tungstenite::{WebSocketStream, tungstenite};
use smol::Async;
use smol::stream::StreamExt;
use smol::Timer;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use smol::lock::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::{Instrument, Level, debug, error, info, span, warn};
use tracing_subscriber::fmt;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tungstenite::Message;

use crate::command::{AdcReadings, ChannelReading, Command, CommandResponse};
use crate::hardware::Hardware;

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::components::ads1015::{Channel, Gain, DataRate};

// ============================================================================
// ADC CONFIGURATION - Easy to modify
// ============================================================================

/// ADC sampling rate in Hz (samples per second)
const ADC_SAMPLE_RATE_HZ: u64 = 10;

// ADC configuration constants - only needed on Linux/Android
#[cfg(any(target_os = "linux", target_os = "android"))]
const ADC_GAIN: Gain = Gain::One; // ±4.096V range

#[cfg(any(target_os = "linux", target_os = "android"))]
const ADC_DATA_RATE: DataRate = DataRate::Sps3300; // Maximum speed

/// Maximum retry attempts for failed ADC reads before logging error
const ADC_MAX_RETRIES: u32 = 5;



/// Delay between retry attempts (milliseconds)
const ADC_RETRY_DELAY_MS: u64 = 10;

/// Pressure sensor scaling for a PT with range to 1500
/// Formula: scaled = raw * SCALE_A + OFFSET_A
const PT1500_SCALE: f32 = 0.909754;
const PT1500_OFFSET: f32 = 5.08926;

/// Pressure sensor scaling for a PT with range to 2000
/// Formula: scaled = raw * SCALE_B + OFFSET_B
const PT2000_SCALE: f32 = 1.22124;
const PT2000_OFFSET: f32 = 5.37052;

/// Pressure sensor scaling for a LoadCell
/// Formula: scaled = raw * SCALE_C + OFFSET_C
const LOADCELL_SCALE: f32 = 1.69661;
const LOADCELL_OFFSET: f32 = 75.37882;

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

        // Active client tracker
        let active_client_count = Arc::new(AtomicUsize::new(0));

        // Spawn Safety Monitor Task
        info!("Starting Safety Monitor task...");
        let safety_hw = hardware.clone();
        let safety_counts = active_client_count.clone();
        smol::spawn(safety_monitor_task(safety_hw, safety_counts)).detach();

        // Spawn background ADC monitoring task
        info!("Starting ADC monitoring task at {} Hz...", ADC_SAMPLE_RATE_HZ);
        let adc_task_hw = hardware.clone();
        let adc_task_readings = adc_readings.clone();

        smol::spawn(adc_monitoring_task(adc_task_hw, adc_task_readings)).detach();

        // Spawn CSV Logger Task
        let log_hw = hardware.clone();
        let log_adc = adc_readings.clone();
        smol::spawn(csv_logger::start_logging(log_hw, log_adc)).detach();

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
            let active_clients = active_client_count.clone();
            smol::spawn(handle_socket(stream, hw, adc, active_clients).instrument(span)).detach();
        }
    })
}

/// Handle WebSocket connection lifecycle
async fn handle_socket(
    mut stream: WebSocketStream<Async<TcpStream>>, 
    hardware: Arc<Mutex<Hardware>>,
    adc_readings: Arc<Mutex<AdcReadings>>,
    active_client_count: Arc<AtomicUsize>,
) {
    info!("Client connected");
    active_client_count.fetch_add(1, Ordering::SeqCst);
    
    // Track streaming state and last sent timestamp
    let mut streaming_enabled = false;
    let mut last_sent_timestamp = 0u64;
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
                let response = process_message(&message, &hardware, &adc_readings, &mut streaming_enabled).await;
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
) -> CommandResponse {
    debug!("Received message: {}", message);

    match serde_json::from_str(message) {
        Ok(command) => {
            info!("Received command: {:?}", command);
            execute_command(command, hardware, streaming_enabled).await
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
        Command::ActuateValve { valve, state } => {
            #[cfg(any(target_os = "linux", target_os = "android"))]
            {
                let hw = hardware.lock().await;
                let result = match valve.to_lowercase().as_str() {
                    "sv1" => hw.sv1.actuate(state).await,
                    "sv2" => hw.sv2.actuate(state).await,
                    "sv3" => hw.sv3.actuate(state).await,
                    "sv4" => hw.sv4.actuate(state).await,
                    "sv5" => hw.sv5.actuate(state).await,
                    _ => {
                        warn!("Unknown valve: {}", valve);
                        return CommandResponse::Error;
                    }
                };

                match result {
                    Ok(_) => {
                        info!("Valve {} actuated: {}", valve, state);
                        CommandResponse::Success
                    }
                    Err(e) => {
                        error!("Failed to actuate valve {}: {}", valve, e);
                        CommandResponse::Error
                    }
                }
            }
            #[cfg(not(any(target_os = "linux", target_os = "android")))]
            {
                let _ = hardware;
                warn!("ActuateValve command not supported on this platform: {} -> {}", valve, state);
                CommandResponse::Success // Maintain consistent response type even if mocked
            }
        }
        Command::GetValveState { valve } => {
            #[cfg(any(target_os = "linux", target_os = "android"))]
            {
                let hw = hardware.lock().await;
                let result = match valve.to_lowercase().as_str() {
                    "sv1" => Some((hw.sv1.is_actuated().await, hw.sv1.check_continuity().await)),
                    "sv2" => Some((hw.sv2.is_actuated().await, hw.sv2.check_continuity().await)),
                    "sv3" => Some((hw.sv3.is_actuated().await, hw.sv3.check_continuity().await)),
                    "sv4" => Some((hw.sv4.is_actuated().await, hw.sv4.check_continuity().await)),
                    "sv5" => Some((hw.sv5.is_actuated().await.map(|b| !b), hw.sv5.check_continuity().await)), // using NOT operator for a external reason
                    _ => None,
                };

                match result {
                    Some((Ok(actuated), Ok(continuity))) => {
                        CommandResponse::ValveState { actuated, continuity }
                    }
                    Some((Err(e), _)) => {
                        error!("Failed to get valve actuation state: {}", e);
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
                CommandResponse::ValveState { valve, actuated: false, continuity: false }
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
        Command::SetMavAngle { valve: _valve, angle } => {
            let hw = hardware.lock().await;
            info!("Setting MAV angle to {}", angle);
            if let Err(e) = hw.mav.set_angle(angle).await {
                error!("Failed to set MAV angle: {}", e);
                CommandResponse::Error
            } else {
                CommandResponse::Success
            }
        }
        Command::MavOpen { valve: _valve } => {
            let hw = hardware.lock().await;
            info!("Opening MAV");
            if let Err(e) = hw.mav.open().await {
                error!("Failed to open MAV: {}", e);
                CommandResponse::Error
            } else {
                CommandResponse::Success
            }
        }
        Command::MavClose { valve: _valve } => {
            let hw = hardware.lock().await;
            info!("Closing MAV");
            if let Err(e) = hw.mav.close().await {
                error!("Failed to close MAV: {}", e);
                CommandResponse::Error
            } else {
                CommandResponse::Success
            }
        }
        Command::MavNeutral { valve: _valve } => {
            let hw = hardware.lock().await;
            info!("Setting MAV to neutral");
            if let Err(e) = hw.mav.neutral().await {
                error!("Failed to set MAV neutral: {}", e);
                CommandResponse::Error
            } else {
                CommandResponse::Success
            }
        }
        Command::GetMavState { valve: _valve } => {
            let hw = hardware.lock().await;
            match hw.mav.get_pulse_width_us().await {
                Ok(us) => {
                    let angle = hw.mav.get_angle().await.unwrap_or(0.0);
                    CommandResponse::MavState { angle, pulse_width_us: us }
                }
                Err(e) => {
                    error!("Failed to get MAV state: {}", e);
                    CommandResponse::Error
                }
            }
        }
        Command::BVOpen => {
            let hw = hardware.lock().await;
            info!("Executing BallValve Open Sequence");
            if let Err(e) = hw.ball_valve.open_sequence().await {
                error!("Failed to open ball valve: {}", e);
                CommandResponse::Error
            } else {
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
        Command::Heartbeat => {
            // Heartbeat command just keeps the connection alive
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
    active_client_count: Arc<AtomicUsize>
) {
    let mut disconnect_start: Option<Instant> = None;
    let mut safety_triggered = false;

    loop {
        let count = active_client_count.load(Ordering::SeqCst);
        
        if count == 0 {
            // If no clients, verify how long we've been disconnected
            if disconnect_start.is_none() {
                info!("No active clients. Starting safety timer.");
                disconnect_start = Some(Instant::now());
                safety_triggered = false;
            }

            if let Some(start) = disconnect_start {
                if !safety_triggered && start.elapsed() > Duration::from_secs(15) {
                    warn!("SAFETY TIMEOUT (15s) - Executing Emergency Shutdown");
                    perform_emergency_shutdown(&hardware).await;
                    safety_triggered = true;
                }
            }
        } else {
            // Client(s) connected
            if disconnect_start.is_some() {
                info!("Client connected. Safety timer cancelled.");
                disconnect_start = None;
                safety_triggered = false;
            }
        }

        Timer::after(Duration::from_millis(500)).await;
    }
}

async fn perform_emergency_shutdown(hardware: &Arc<Mutex<Hardware>>) {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        let hw = hardware.lock().await;
        info!("EMERGENCY SHUTDOWN: Closing all Valves");
        
        // Close all SVs (Signal Low)
        // Note: We use actuate(false) which sets standard "de-actuated" state.
        // If NormallyClosed (default), this sets pin false (Low).
        let _ = hw.sv1.actuate(false).await;
        let _ = hw.sv2.actuate(false).await;
        let _ = hw.sv3.actuate(false).await;
        let _ = hw.sv4.actuate(false).await;
        let _ = hw.sv5.actuate(false).await;
        
        // Close MAV
        let _ = hw.mav.close().await;
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
        
        // Apply PT1500 scaling to channel 0 on ADC 1 and PT2000 scaling on other channels
        let scaled = if i == 0 {
            Some(raw as f32 * PT1500_SCALE + PT1500_OFFSET)
        } else {
            Some(raw as f32 * PT2000_SCALE + PT2000_OFFSET)
        };
        
        adc1_readings[i] = ChannelReading { raw, voltage, scaled };
    }
    
    // Read ADC2 channels
    for (i, &channel) in channels.iter().enumerate() {
        let raw = hw.adc2.read_raw(channel, ADC_GAIN, ADC_DATA_RATE)?;
        let voltage = (raw as f32) * ADC_GAIN.lsb_size();
        
        // Apply Loadcell Scaling to channel 1 on ADC 2 and PT2000 scaling on other channels
        let scaled = if i == 1 {
            Some(raw as f32 * LOADCELL_SCALE + LOADCELL_OFFSET)
        } else {
            Some(raw as f32 * PT2000_SCALE + PT2000_OFFSET)
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
