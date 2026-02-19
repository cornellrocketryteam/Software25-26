pub const SYNC_WORD: u32 = 0x3E5D5967;

pub struct Packet {
    // Header
    pub sync_word: u32,         // 0: Sync word ("CRT!")
    pub metadata: u16,          // 4: Metadata
    pub ms_since_boot: u32,     // 6: Milliseconds since boot
    pub events: u32,            // 10: Events bitfield

    // Altimeter
    pub altitude: f32,          // 14: Altitude (m)
    pub temperature: f32,       // 18: Temperature (C)

    // GPS
    pub latitude: i32,          // 22: Latitude (µ deg)
    pub longitude: i32,         // 26: Longitude (µ deg)
    pub satellites_in_view: u8, // 30: Satellites in view
    pub unix_time: u32,         // 31: Unix time (seconds)
    pub horizontal_accuracy: u32, // 35: Horizontal accuracy (mm)

    // IMU
    pub imu_accel_x: f32,      // 39: Acceleration X (m/s^2)
    pub imu_accel_y: f32,      // 43: Acceleration Y (m/s^2)
    pub imu_accel_z: f32,      // 47: Acceleration Z (m/s^2)
    pub gyro_x: f32,           // 51: Gyro X (deg/s)
    pub gyro_y: f32,           // 55: Gyro Y (deg/s)
    pub gyro_z: f32,           // 59: Gyro Z (deg/s)
    pub orientation_x: f32,    // 63: Orientation X (deg)
    pub orientation_y: f32,    // 67: Orientation Y (deg)
    pub orientation_z: f32,    // 71: Orientation Z (deg)

    // High-G Accelerometer
    pub hi_g_accel_x: f32,     // 75: Acceleration X (g)
    pub hi_g_accel_y: f32,     // 79: Acceleration Y (g)
    pub hi_g_accel_z: f32,     // 83: Acceleration Z (g)

    // Internal ADC
    pub battery_voltage: f32,  // 87: Battery voltage (V)

    // External ADC
    pub pt3_pressure: f32,     // 91: PT 3 pressure (PSI)
    pub pt4_pressure: f32,     // 95: PT 4 pressure (PSI)
    pub rtd_temperature: f32,  // 99: RTD temperature (C)

    // BLiMS
    pub motor_state: f32,      // 103: Motor state (inches)
}

impl Default for Packet {
    fn default() -> Self {
        Self {
            sync_word: SYNC_WORD,
            metadata: 0,
            ms_since_boot: 0,
            events: 0,
            altitude: 0.0,
            temperature: 0.0,
            latitude: 0,
            longitude: 0,
            satellites_in_view: 0,
            unix_time: 0,
            horizontal_accuracy: 0,
            imu_accel_x: 0.0,
            imu_accel_y: 0.0,
            imu_accel_z: 0.0,
            gyro_x: 0.0,
            gyro_y: 0.0,
            gyro_z: 0.0,
            orientation_x: 0.0,
            orientation_y: 0.0,
            orientation_z: 0.0,
            hi_g_accel_x: 0.0,
            hi_g_accel_y: 0.0,
            hi_g_accel_z: 0.0,
            battery_voltage: 0.0,
            pt3_pressure: 0.0,
            pt4_pressure: 0.0,
            rtd_temperature: 0.0,
            motor_state: 0.0,
        }
    }
}

impl Packet {
    /// Size of the packet in bytes when serialized
    pub const SIZE: usize = 107;

    /// Serialize the packet to bytes (little-endian)
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut b = [0u8; Self::SIZE];

        b[0..4].copy_from_slice(&self.sync_word.to_le_bytes());
        b[4..6].copy_from_slice(&self.metadata.to_le_bytes());
        b[6..10].copy_from_slice(&self.ms_since_boot.to_le_bytes());
        b[10..14].copy_from_slice(&self.events.to_le_bytes());

        b[14..18].copy_from_slice(&self.altitude.to_le_bytes());
        b[18..22].copy_from_slice(&self.temperature.to_le_bytes());

        b[22..26].copy_from_slice(&self.latitude.to_le_bytes());
        b[26..30].copy_from_slice(&self.longitude.to_le_bytes());
        b[30] = self.satellites_in_view;
        b[31..35].copy_from_slice(&self.unix_time.to_le_bytes());
        b[35..39].copy_from_slice(&self.horizontal_accuracy.to_le_bytes());

        b[39..43].copy_from_slice(&self.imu_accel_x.to_le_bytes());
        b[43..47].copy_from_slice(&self.imu_accel_y.to_le_bytes());
        b[47..51].copy_from_slice(&self.imu_accel_z.to_le_bytes());
        b[51..55].copy_from_slice(&self.gyro_x.to_le_bytes());
        b[55..59].copy_from_slice(&self.gyro_y.to_le_bytes());
        b[59..63].copy_from_slice(&self.gyro_z.to_le_bytes());
        b[63..67].copy_from_slice(&self.orientation_x.to_le_bytes());
        b[67..71].copy_from_slice(&self.orientation_y.to_le_bytes());
        b[71..75].copy_from_slice(&self.orientation_z.to_le_bytes());

        b[75..79].copy_from_slice(&self.hi_g_accel_x.to_le_bytes());
        b[79..83].copy_from_slice(&self.hi_g_accel_y.to_le_bytes());
        b[83..87].copy_from_slice(&self.hi_g_accel_z.to_le_bytes());

        b[87..91].copy_from_slice(&self.battery_voltage.to_le_bytes());

        b[91..95].copy_from_slice(&self.pt3_pressure.to_le_bytes());
        b[95..99].copy_from_slice(&self.pt4_pressure.to_le_bytes());
        b[99..103].copy_from_slice(&self.rtd_temperature.to_le_bytes());

        b[103..107].copy_from_slice(&self.motor_state.to_le_bytes());

        b
    }

    /// Deserialize the packet from bytes (little-endian)
    pub fn from_bytes(bytes: &[u8; Self::SIZE]) -> Self {
        Self {
            sync_word: u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            metadata: u16::from_le_bytes([bytes[4], bytes[5]]),
            ms_since_boot: u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]),
            events: u32::from_le_bytes([bytes[10], bytes[11], bytes[12], bytes[13]]),

            altitude: f32::from_le_bytes([bytes[14], bytes[15], bytes[16], bytes[17]]),
            temperature: f32::from_le_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]),

            latitude: i32::from_le_bytes([bytes[22], bytes[23], bytes[24], bytes[25]]),
            longitude: i32::from_le_bytes([bytes[26], bytes[27], bytes[28], bytes[29]]),
            satellites_in_view: bytes[30],
            unix_time: u32::from_le_bytes([bytes[31], bytes[32], bytes[33], bytes[34]]),
            horizontal_accuracy: u32::from_le_bytes([bytes[35], bytes[36], bytes[37], bytes[38]]),

            imu_accel_x: f32::from_le_bytes([bytes[39], bytes[40], bytes[41], bytes[42]]),
            imu_accel_y: f32::from_le_bytes([bytes[43], bytes[44], bytes[45], bytes[46]]),
            imu_accel_z: f32::from_le_bytes([bytes[47], bytes[48], bytes[49], bytes[50]]),
            gyro_x: f32::from_le_bytes([bytes[51], bytes[52], bytes[53], bytes[54]]),
            gyro_y: f32::from_le_bytes([bytes[55], bytes[56], bytes[57], bytes[58]]),
            gyro_z: f32::from_le_bytes([bytes[59], bytes[60], bytes[61], bytes[62]]),
            orientation_x: f32::from_le_bytes([bytes[63], bytes[64], bytes[65], bytes[66]]),
            orientation_y: f32::from_le_bytes([bytes[67], bytes[68], bytes[69], bytes[70]]),
            orientation_z: f32::from_le_bytes([bytes[71], bytes[72], bytes[73], bytes[74]]),

            hi_g_accel_x: f32::from_le_bytes([bytes[75], bytes[76], bytes[77], bytes[78]]),
            hi_g_accel_y: f32::from_le_bytes([bytes[79], bytes[80], bytes[81], bytes[82]]),
            hi_g_accel_z: f32::from_le_bytes([bytes[83], bytes[84], bytes[85], bytes[86]]),

            battery_voltage: f32::from_le_bytes([bytes[87], bytes[88], bytes[89], bytes[90]]),

            pt3_pressure: f32::from_le_bytes([bytes[91], bytes[92], bytes[93], bytes[94]]),
            pt4_pressure: f32::from_le_bytes([bytes[95], bytes[96], bytes[97], bytes[98]]),
            rtd_temperature: f32::from_le_bytes([bytes[99], bytes[100], bytes[101], bytes[102]]),

            motor_state: f32::from_le_bytes([bytes[103], bytes[104], bytes[105], bytes[106]]),
        }
    }
}
