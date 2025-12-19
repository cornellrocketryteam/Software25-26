use serde::{Deserialize, Serialize};

/// All supported commands for the fill station
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Command {
    Ignite,
    /// Start streaming ADC readings to this client
    StartAdcStream,
    /// Stop streaming ADC readings to this client
    StopAdcStream,
}

/// Response sent back to WebSocket clients after command execution
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandResponse {
    Success,
    Error,
    /// ADC reading data
    AdcData {
        timestamp_ms: u64,
        valid: bool,
        adc1: [ChannelReading; 4],
        adc2: [ChannelReading; 4],
    },
}

/// Single ADC channel reading with all relevant data
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ChannelReading {
    pub raw: i16,
    pub voltage: f32,
    pub scaled: Option<f32>, // Some channels have pressure sensor scaling
}
