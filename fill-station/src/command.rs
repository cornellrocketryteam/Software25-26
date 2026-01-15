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
    /// Actuate a solenoid valve
    ActuateValve {
        /// Name of the valve (e.g. "SV1", "SV2")
        valve: String,
        /// True to actuate (open/active), False to deactivate.
        /// Actual electrical state depends on NO/NC configuration.
        state: bool,
    },
    /// Set MAV angle in degrees (0-90)
    SetMavAngle {
        /// Name of valve (usually "MAV", but just for logging context if needed)
        valve: String,
        angle: f32,
    },
    /// Open MAV (90 degrees)
    MavOpen { valve: String },
    /// Close MAV (0 degrees)
    MavClose { valve: String },
    /// Set MAV to neutral (1520us)
    MavNeutral { valve: String },

    // Ball Valve Commands
    #[serde(rename = "bv_open")]
    BVOpen,
    #[serde(rename = "bv_close")]
    BVClose,
    #[serde(rename = "bv_signal")]
    BVSignal { state: String }, // "high" or "low"
    #[serde(rename = "bv_on_off")]
    BVOnOff { state: String },  // "high" or "low"
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
