#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Command {
    Vent,
    N1,
    N2,
    N3,
    N4,
    A1,
    A2,
    A3,
    ForceMode(u32),
}

#[derive(Default)]
pub struct Packet {
    pub flight_mode: u32,
    // altimeter
    pub pressure: f32,
    pub temp: f32,
    pub altitude: f32,
    //gps
    pub latitude: f32,
    pub longitude: f32,
    pub num_satellites: u32,
    pub timestamp: f32,
    // magnetometer
    pub mag_x: f32,
    pub mag_y: f32,
    pub mag_z: f32,
    // imu - accelerometer (m/s²)
    pub accel_x: f32,
    pub accel_y: f32,
    pub accel_z: f32,
    // imu - gyroscope (°/s)
    pub gyro_x: f32,
    pub gyro_y: f32,
    pub gyro_z: f32,
    // adc - ADS1015 (scaled)
    pub pt3: f32, // channel 3
    pub pt4: f32, // channel 2
    pub rtd: f32, // channel 1
    // valve states
    pub sv_open: bool,
    pub mav_open: bool,
    // event flags (0 = not triggered, 1 = triggered)
    pub ssa_drogue_deployed: u8,
    pub ssa_main_deployed: u8,
    pub cmd_n1: u8,
    pub cmd_n2: u8,
    pub cmd_n3: u8,
    pub cmd_n4: u8,
    pub cmd_a1: u8,
    pub cmd_a2: u8,
    pub cmd_a3: u8,
    // airbrake state (0 = idle, 1 = deployed, 2 = retracted)
    pub airbrake_state: u8,
    // airbrake controller output
    pub predicted_apogee: f32,
}

impl Packet {
    pub const SIZE: usize = 96;

    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut data = [0u8; Self::SIZE];
        data[0..4].copy_from_slice(&self.flight_mode.to_le_bytes());
        data[4..8].copy_from_slice(&self.pressure.to_le_bytes());
        data[8..12].copy_from_slice(&self.temp.to_le_bytes());
        data[12..16].copy_from_slice(&self.altitude.to_le_bytes());
        data[16..20].copy_from_slice(&self.latitude.to_le_bytes());
        data[20..24].copy_from_slice(&self.longitude.to_le_bytes());
        data[24..28].copy_from_slice(&self.num_satellites.to_le_bytes());
        data[28..32].copy_from_slice(&self.timestamp.to_le_bytes());
        data[32..36].copy_from_slice(&self.mag_x.to_le_bytes());
        data[36..40].copy_from_slice(&self.mag_y.to_le_bytes());
        data[40..44].copy_from_slice(&self.mag_z.to_le_bytes());
        data[44..48].copy_from_slice(&self.accel_x.to_le_bytes());
        data[48..52].copy_from_slice(&self.accel_y.to_le_bytes());
        data[52..56].copy_from_slice(&self.accel_z.to_le_bytes());
        data[56..60].copy_from_slice(&self.gyro_x.to_le_bytes());
        data[60..64].copy_from_slice(&self.gyro_y.to_le_bytes());
        data[64..68].copy_from_slice(&self.gyro_z.to_le_bytes());
        data[68..72].copy_from_slice(&self.pt3.to_le_bytes());
        data[72..76].copy_from_slice(&self.pt4.to_le_bytes());
        data[76..80].copy_from_slice(&self.rtd.to_le_bytes());
        data[80] = self.sv_open as u8;
        data[81] = self.mav_open as u8;
        data[82] = self.ssa_drogue_deployed;
        data[83] = self.ssa_main_deployed;
        data[84] = self.cmd_n1;
        data[85] = self.cmd_n2;
        data[86] = self.cmd_n3;
        data[87] = self.cmd_n4;
        data[88] = self.cmd_a1;
        data[89] = self.cmd_a2;
        data[90] = self.cmd_a3;
        data[91] = self.airbrake_state;
        data[92..96].copy_from_slice(&self.predicted_apogee.to_le_bytes());
        data
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        if bytes.len() < Self::SIZE {
            return Self::default();
        }

        Self {
            flight_mode: u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
            pressure: f32::from_le_bytes(bytes[4..8].try_into().unwrap()),
            temp: f32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            altitude: f32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            latitude: f32::from_le_bytes(bytes[16..20].try_into().unwrap()),
            longitude: f32::from_le_bytes(bytes[20..24].try_into().unwrap()),
            num_satellites: u32::from_le_bytes(bytes[24..28].try_into().unwrap()),
            timestamp: f32::from_le_bytes(bytes[28..32].try_into().unwrap()),
            mag_x: f32::from_le_bytes(bytes[32..36].try_into().unwrap()),
            mag_y: f32::from_le_bytes(bytes[36..40].try_into().unwrap()),
            mag_z: f32::from_le_bytes(bytes[40..44].try_into().unwrap()),
            accel_x: f32::from_le_bytes(bytes[44..48].try_into().unwrap()),
            accel_y: f32::from_le_bytes(bytes[48..52].try_into().unwrap()),
            accel_z: f32::from_le_bytes(bytes[52..56].try_into().unwrap()),
            gyro_x: f32::from_le_bytes(bytes[56..60].try_into().unwrap()),
            gyro_y: f32::from_le_bytes(bytes[60..64].try_into().unwrap()),
            gyro_z: f32::from_le_bytes(bytes[64..68].try_into().unwrap()),
            pt3: f32::from_le_bytes(bytes[68..72].try_into().unwrap()),
            pt4: f32::from_le_bytes(bytes[72..76].try_into().unwrap()),
            rtd: f32::from_le_bytes(bytes[76..80].try_into().unwrap()),
            sv_open: bytes[80] != 0,
            mav_open: bytes[81] != 0,
            ssa_drogue_deployed: bytes[82],
            ssa_main_deployed: bytes[83],
            cmd_n1: bytes[84],
            cmd_n2: bytes[85],
            cmd_n3: bytes[86],
            cmd_n4: bytes[87],
            cmd_a1: bytes[88],
            cmd_a2: bytes[89],
            cmd_a3: bytes[90],
            airbrake_state: bytes[91],
            predicted_apogee: f32::from_le_bytes(bytes[92..96].try_into().unwrap()),
        }
    }

    pub const CSV_HEADER: &'static str = "flight_mode,pressure,temp,altitude,latitude,longitude,num_satellites,timestamp,mag_x,mag_y,mag_z,accel_x,accel_y,accel_z,gyro_x,gyro_y,gyro_z,pt3,pt4,rtd,sv_open,mav_open,ssa_drogue_deployed,ssa_main_deployed,cmd_n1,cmd_n2,cmd_n3,cmd_n4,cmd_a1,cmd_a2,cmd_a3,airbrake_state,predicted_apogee\n";

    pub fn to_csv(&self, buf: &mut [u8]) -> usize {
        use core::fmt::Write;
        let mut wrapper = WriteWrapper::new(buf);
        let _ = write!(
            wrapper,
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            self.flight_mode,
            self.pressure,
            self.temp,
            self.altitude,
            self.latitude,
            self.longitude,
            self.num_satellites,
            self.timestamp,
            self.mag_x,
            self.mag_y,
            self.mag_z,
            self.accel_x,
            self.accel_y,
            self.accel_z,
            self.gyro_x,
            self.gyro_y,
            self.gyro_z,
            self.pt3,
            self.pt4,
            self.rtd,
            self.sv_open as u8,
            self.mav_open as u8,
            self.ssa_drogue_deployed,
            self.ssa_main_deployed,
            self.cmd_n1,
            self.cmd_n2,
            self.cmd_n3,
            self.cmd_n4,
            self.cmd_a1,
            self.cmd_a2,
            self.cmd_a3,
            self.airbrake_state,
            self.predicted_apogee
        );
        wrapper.offset
    }
}

struct WriteWrapper<'a> {
    buf: &'a mut [u8],
    offset: usize,
}

impl<'a> WriteWrapper<'a> {
    fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, offset: 0 }
    }
}

impl<'a> core::fmt::Write for WriteWrapper<'a> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let len = s.len();
        if self.offset + len > self.buf.len() {
            return Err(core::fmt::Error);
        }
        self.buf[self.offset..self.offset + len].copy_from_slice(s.as_bytes());
        self.offset += len;
        Ok(())
    }
}
