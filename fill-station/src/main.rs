mod command;
mod hardware;
mod components;

use anyhow::Result;
use async_tungstenite::{WebSocketStream, tungstenite};
use smol::Async;
use smol::stream::StreamExt;
use smol::Timer;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use smol::lock::Mutex;
use tracing::{Instrument, Level, debug, error, info, span, warn};
use tracing_subscriber::fmt;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tungstenite::Message;

use crate::command::{ChannelReading, Command, CommandResponse};
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

/// Number of samples to average for each reading
const ADC_AVG_SAMPLES: usize = 10;

/// Delay between retry attempts (milliseconds)
const ADC_RETRY_DELAY_MS: u64 = 10;

/// Pressure sensor scaling for ADC1 Channel 0
/// Formula: scaled = raw * SCALE_A + OFFSET_A
const ADC1_CH0_SCALE: f32 = 0.9365126677;
const ADC1_CH0_OFFSET: f32 = 3.719970194;

/// Pressure sensor scaling for ADC1 Channel 1
/// Formula: scaled = raw * SCALE_B + OFFSET_B
const ADC1_CH1_SCALE: f32 = 0.6285508522;
const ADC1_CH1_OFFSET: f32 = 1.783227975;

// ============================================================================
// SHARED ADC STATE
// ============================================================================

/// Shared ADC readings accessible across tasks
#[derive(Debug, Clone)]
pub struct AdcReadings {
    pub timestamp_ms: u64,
    pub valid: bool,
    pub adc1: [ChannelReading; 4],
    pub adc2: [ChannelReading; 4],
}

impl Default for AdcReadings {
    fn default() -> Self {
        Self {
            timestamp_ms: 0,
            valid: false,
            adc1: [ChannelReading { raw: 0, voltage: 0.0, scaled: None }; 4],
            adc2: [ChannelReading { raw: 0, voltage: 0.0, scaled: None }; 4],
        }
    }
}

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

        // Spawn background ADC monitoring task
        info!("Starting ADC monitoring task at {} Hz...", ADC_SAMPLE_RATE_HZ);
        let adc_task_hw = hardware.clone();
        let adc_task_readings = adc_readings.clone();
        smol::spawn(adc_monitoring_task(adc_task_hw, adc_task_readings)).detach();

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
            smol::spawn(handle_socket(stream, hw, adc).instrument(span)).detach();
        }
    })
}

/// Handle WebSocket connection lifecycle
async fn handle_socket(
    mut stream: WebSocketStream<Async<TcpStream>>, 
    hardware: Arc<Mutex<Hardware>>,
    adc_readings: Arc<Mutex<AdcReadings>>,
) {
    info!("Client connected");
    
    // Track streaming state and last sent timestamp
    let mut streaming_enabled = false;
    let mut last_sent_timestamp = 0u64;
    
    // Small timeout for non-blocking message receive
    let poll_interval = Duration::from_millis(50);
    
    loop {
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
    
    info!("Client disconnected")
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
            #[cfg(any(target_os = "linux", target_os = "android"))]
            {
                let hw = hardware.lock().await;
                info!("Igniting both ig1 and ig2...");
                
                // Ignite both concurrently so they fire at the same time
                let ignite_1 = hw.ig1.ignite();
                let ignite_2 = hw.ig2.ignite();
                smol::future::zip(ignite_1, ignite_2).await;
                
                info!("Ignition complete");
            }
            #[cfg(not(any(target_os = "linux", target_os = "android")))]
            {
                let _ = hardware; // Suppress unused warning on non-Linux platforms
                warn!("Ignite command not supported on this platform");
            }
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
                    "sv5" => Some((hw.sv5.is_actuated().await, hw.sv5.check_continuity().await)),
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
                CommandResponse::ValveState { actuated: false, continuity: false }
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
        Command::SetMavAngle { valve, angle } => {
            let hw = hardware.lock().await;
            info!("Setting MAV angle to {}", angle);
            if let Err(e) = hw.mav.set_angle(angle).await {
                error!("Failed to set MAV angle: {}", e);
                CommandResponse::Error
            } else {
                CommandResponse::Success
            }
        }
        Command::MavOpen { valve } => {
            let hw = hardware.lock().await;
            info!("Opening MAV");
            if let Err(e) = hw.mav.open().await {
                error!("Failed to open MAV: {}", e);
                CommandResponse::Error
            } else {
                CommandResponse::Success
            }
        }
        Command::MavClose { valve } => {
            let hw = hardware.lock().await;
            info!("Closing MAV");
            if let Err(e) = hw.mav.close().await {
                error!("Failed to close MAV: {}", e);
                CommandResponse::Error
            } else {
                CommandResponse::Success
            }
        }
        Command::MavNeutral { valve } => {
            let hw = hardware.lock().await;
            info!("Setting MAV to neutral");
            if let Err(e) = hw.mav.neutral().await {
                error!("Failed to set MAV neutral: {}", e);
                CommandResponse::Error
            } else {
                CommandResponse::Success
            }
        }
        Command::GetMavState { valve } => {
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
        let raw = hw.adc1.read_raw_averaged(channel, ADC_GAIN, ADC_DATA_RATE, ADC_AVG_SAMPLES)?;
        let voltage = (raw as f32) * ADC_GAIN.lsb_size();
        
        // Apply scaling for pressure sensors on channels 0 and 1
        let scaled = match i {
            0 => Some(raw as f32 * ADC1_CH0_SCALE + ADC1_CH0_OFFSET),
            1 => Some(raw as f32 * ADC1_CH1_SCALE + ADC1_CH1_OFFSET),
            _ => None, // Channels 2 and 3 have no scaling
        };
        
        adc1_readings[i] = ChannelReading { raw, voltage, scaled };
    }
    
    // Read ADC2 channels
    for (i, &channel) in channels.iter().enumerate() {
        let raw = hw.adc2.read_raw_averaged(channel, ADC_GAIN, ADC_DATA_RATE, ADC_AVG_SAMPLES)?;
        let voltage = (raw as f32) * ADC_GAIN.lsb_size();
        
        adc2_readings[i] = ChannelReading { raw, voltage, scaled: None };
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
