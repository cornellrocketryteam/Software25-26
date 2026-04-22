use serde::{Deserialize, Serialize};

/// Number of comma-separated fields in a `$TELEM,` line, matching the FSW
/// emitter in `fsw/src/umbilical.rs`. Must be kept in sync on both sides.
pub const TELEM_FIELD_COUNT: usize = 22;

/// FSW telemetry packet parsed from CSV text lines.
/// The FSW emits lines like: `$TELEM,0,101325.0,25.0,0.0,...,0,0\n`
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
    // valve states
    pub sv_open: bool,
    pub mav_open: bool,
}

impl FswTelemetry {
    /// Total serialized size in bytes (kept for binary compat if needed).
    pub const SIZE: usize = 82;

    /// Parse from a CSV field slice (the 22 fields after the `$TELEM,` prefix).
    /// Returns `None` if the field count or any field fails to parse.
    pub fn from_csv(fields: &[&str]) -> Option<Self> {
        if fields.len() != TELEM_FIELD_COUNT {
            return None;
        }
        Some(Self {
            flight_mode:    fields[0].trim().parse().ok()?,
            pressure:       fields[1].trim().parse().ok()?,
            temp:           fields[2].trim().parse().ok()?,
            altitude:       fields[3].trim().parse().ok()?,
            latitude:       fields[4].trim().parse().ok()?,
            longitude:      fields[5].trim().parse().ok()?,
            num_satellites: fields[6].trim().parse().ok()?,
            timestamp:      fields[7].trim().parse().ok()?,
            mag_x:          fields[8].trim().parse().ok()?,
            mag_y:          fields[9].trim().parse().ok()?,
            mag_z:          fields[10].trim().parse().ok()?,
            accel_x:        fields[11].trim().parse().ok()?,
            accel_y:        fields[12].trim().parse().ok()?,
            accel_z:        fields[13].trim().parse().ok()?,
            gyro_x:         fields[14].trim().parse().ok()?,
            gyro_y:         fields[15].trim().parse().ok()?,
            gyro_z:         fields[16].trim().parse().ok()?,
            pt3:            fields[17].trim().parse().ok()?,
            pt4:            fields[18].trim().parse().ok()?,
            rtd:            fields[19].trim().parse().ok()?,
            sv_open:        fields[20].trim().parse::<u8>().ok()? != 0,
            mav_open:       fields[21].trim().parse::<u8>().ok()? != 0,
        })
    }

    /// Deserialize from an 82-byte little-endian buffer (legacy binary format).
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
            sv_open:        buf[80] != 0,
            mav_open:       buf[81] != 0,
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
