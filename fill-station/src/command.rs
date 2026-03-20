use serde::{Deserialize, Serialize};
use crate::components::umbilical::FswTelemetry;

/// All supported commands for the fill station
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Command {
    Ignite,
    /// Query continuity for a specific igniter (1 or 2)
    GetIgniterContinuity { id: u8 },
    /// Start streaming ADC readings to this client
    StartAdcStream,
    /// Stop streaming ADC readings to this client
    StopAdcStream,
    /// Open or close a solenoid valve
    ActuateValve {
        /// Name of the valve (e.g. "SV1")
        valve: String,
        /// True to open the valve, False to close it.
        /// The server handles the correct GPIO level based on NO/NC configuration.
        open: bool,
    },
    // Ball Valve Commands
    #[serde(rename = "bv_open")]
    BVOpen,
    #[serde(rename = "bv_close")]
    BVClose,
    #[serde(rename = "bv_signal")]
    BVSignal { state: String }, // "high" or "low"
    #[serde(rename = "bv_on_off")]
    BVOnOff { state: String },  // "high" or "low"

    /// Get state of a solenoid valve (actuation and continuity)
    GetValveState {
        /// Name of the valve (e.g. "SV1")
        valve: String,
    },
    /// Move QD stepper a specific number of steps in a given direction
    QdMove { steps: u32, direction: bool },
    /// Retract QD using preset steps (CW)
    QdRetract,
    /// Extend QD using preset steps (CCW)
    QdExtend,

    /// Client heartbeat to indicate connection is alive
    Heartbeat,

    // FSW Umbilical Commands
    /// Send launch command to FSW
    FswLaunch,
    /// Open MAV on FSW
    FswOpenMav,
    /// Close MAV on FSW
    FswCloseMav,
    /// Open SV on FSW
    FswOpenSv,
    /// Close SV on FSW
    FswCloseSv,
    /// Safe all actuators on FSW
    FswSafe,
    /// Reset FRAM on FSW
    FswResetFram,
    /// Reset SD card on FSW
    FswResetCard,
    /// Reboot FSW
    FswReboot,
    /// Dump flash memory on FSW
    FswDumpFlash,
    /// Wipe flash memory on FSW
    FswWipeFlash,
    /// Query flash info on FSW
    FswFlashInfo,
    /// Trigger payload event N1 on FSW
    FswPayloadN1,
    /// Trigger payload event N2 on FSW
    FswPayloadN2,
    /// Trigger payload event N3 on FSW
    FswPayloadN3,
    /// Trigger payload event N4 on FSW
    FswPayloadN4,
    /// Start streaming FSW telemetry to this client
    StartFswStream,
    /// Stop streaming FSW telemetry to this client
    StopFswStream,
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
    /// Solenoid valve state
    ValveState {
        valve: String,
        open: bool,
        continuity: bool,
    },
    /// Igniter continuity state
    IgniterContinuity {
        id: u8,
        continuity: bool,
    },
    /// FSW telemetry data from umbilical
    FswTelemetry {
        timestamp_ms: u64,
        connected: bool,
        flight_mode: String,
        telemetry: FswTelemetry,
    },
}

/// Single ADC channel reading with all relevant data
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ChannelReading {
    pub raw: i16,
    pub voltage: f32,
    pub scaled: Option<f32>, // Some channels have pressure sensor scaling
}

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

/// Shared FSW telemetry readings from umbilical, accessible across tasks
#[derive(Debug, Clone)]
pub struct UmbilicalReadings {
    pub timestamp_ms: u64,
    pub connected: bool,
    pub telemetry: FswTelemetry,
}

impl Default for UmbilicalReadings {
    fn default() -> Self {
        Self {
            timestamp_ms: 0,
            connected: false,
            telemetry: FswTelemetry::default(),
        }
    }
}
