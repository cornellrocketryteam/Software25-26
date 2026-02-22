use serde::{Deserialize, Serialize};

/// FSW telemetry packet — 80 bytes, little-endian.
/// Mirrors the Packet struct serialized in fsw/src/state.rs:transmit().
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct FswTelemetry {
    pub flight_mode: u32,
    pub pressure: f32,       // Pa
    pub temp: f32,           // C
    pub altitude: f32,       // m
    pub latitude: f32,       // degrees
    pub longitude: f32,      // degrees
    pub num_satellites: u32,
    pub timestamp: f32,      // s
    pub mag_x: f32,          // uT
    pub mag_y: f32,
    pub mag_z: f32,
    pub accel_x: f32,        // m/s^2
    pub accel_y: f32,
    pub accel_z: f32,
    pub gyro_x: f32,         // deg/s
    pub gyro_y: f32,
    pub gyro_z: f32,
    pub pt3: f32,            // raw ADC counts
    pub pt4: f32,
    pub rtd: f32,
}

impl FswTelemetry {
    /// Total serialized size in bytes.
    pub const SIZE: usize = 80;

    /// Deserialize from an 80-byte little-endian buffer.
    pub fn from_bytes(buf: &[u8; Self::SIZE]) -> Self {
        Self {
            flight_mode:    u32::from_le_bytes(buf[0..4].try_into().unwrap()),
            pressure:       f32::from_le_bytes(buf[4..8].try_into().unwrap()),
            temp:           f32::from_le_bytes(buf[8..12].try_into().unwrap()),
            altitude:       f32::from_le_bytes(buf[12..16].try_into().unwrap()),
            latitude:       f32::from_le_bytes(buf[16..20].try_into().unwrap()),
            longitude:      f32::from_le_bytes(buf[20..24].try_into().unwrap()),
            num_satellites: u32::from_le_bytes(buf[24..28].try_into().unwrap()),
            timestamp:      f32::from_le_bytes(buf[28..32].try_into().unwrap()),
            mag_x:          f32::from_le_bytes(buf[32..36].try_into().unwrap()),
            mag_y:          f32::from_le_bytes(buf[36..40].try_into().unwrap()),
            mag_z:          f32::from_le_bytes(buf[40..44].try_into().unwrap()),
            accel_x:        f32::from_le_bytes(buf[44..48].try_into().unwrap()),
            accel_y:        f32::from_le_bytes(buf[48..52].try_into().unwrap()),
            accel_z:        f32::from_le_bytes(buf[52..56].try_into().unwrap()),
            gyro_x:         f32::from_le_bytes(buf[56..60].try_into().unwrap()),
            gyro_y:         f32::from_le_bytes(buf[60..64].try_into().unwrap()),
            gyro_z:         f32::from_le_bytes(buf[64..68].try_into().unwrap()),
            pt3:            f32::from_le_bytes(buf[68..72].try_into().unwrap()),
            pt4:            f32::from_le_bytes(buf[72..76].try_into().unwrap()),
            rtd:            f32::from_le_bytes(buf[76..80].try_into().unwrap()),
        }
    }

    /// Human-readable flight mode name.
    pub fn flight_mode_name(&self) -> &'static str {
        match self.flight_mode {
            0 => "Startup",
            1 => "Standby",
            2 => "Ascent",
            3 => "Coast",
            4 => "DrogueDeployed",
            5 => "MainDeployed",
            6 => "Fault",
            _ => "Unknown",
        }
    }
}
