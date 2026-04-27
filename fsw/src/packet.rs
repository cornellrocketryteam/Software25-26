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
    pub h_acc: u32,    // horizontal accuracy (mm)
    pub v_acc: u32,    // vertical accuracy (mm)
    pub vel_n: f64,    // north velocity (m/s)
    pub vel_e: f64,    // east  velocity (m/s)
    pub vel_d: f64,    // down  velocity (m/s, positive = descending)
    pub g_speed: f64,  // ground speed (m/s)
    pub s_acc: u32,    // speed accuracy (mm/s)
    pub head_acc: u32, // heading accuracy (deg*1e5)
    pub fix_type: u8,  // 0=none, 2=2D, 3=3D, 4=3D+DGPS
    pub head_mot: i32, // heading of motion (deg*1e5)
    // BLiMS outputs
    pub blims_motor_position: f32,
    pub blims_phase_id: i8,
    pub blims_pid_p: f32,
    pub blims_pid_i: f32,
    pub blims_bearing: f32,
    pub blims_loiter_step: i8,
    pub blims_heading_des: f32,
    pub blims_heading_error: f32,
    pub blims_error_integral: f32,
    pub blims_dist_to_target_m: f32,
    // BLiMS config
    pub blims_target_lat: f32,
    pub blims_target_lon: f32,
    pub blims_wind_from_deg: f32,
    // monotonic clock: milliseconds since CFC boot (resets to 0 on reboot)
    pub ms_since_boot_cfc: u32,
}

impl Packet {
    // 149 GPS fields + 4(motor_pos) + 1(phase_id) + 4(pid_p) + 4(pid_i) + 4(bearing)
    //               + 1(loiter_step) + 4(heading_des) + 4(heading_error) + 4(error_integral)
    //               + 4(dist_to_target) + 4(target_lat) + 4(target_lon) + 4(wind_from_deg) = 195
    pub const SIZE: usize = 199;

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
        data[96..100].copy_from_slice(&self.h_acc.to_le_bytes());
        data[100..104].copy_from_slice(&self.v_acc.to_le_bytes());
        data[104..112].copy_from_slice(&self.vel_n.to_le_bytes());
        data[112..120].copy_from_slice(&self.vel_e.to_le_bytes());
        data[120..128].copy_from_slice(&self.vel_d.to_le_bytes());
        data[128..136].copy_from_slice(&self.g_speed.to_le_bytes());
        data[136..140].copy_from_slice(&self.s_acc.to_le_bytes());
        data[140..144].copy_from_slice(&self.head_acc.to_le_bytes());
        data[144] = self.fix_type;
        data[145..149].copy_from_slice(&self.head_mot.to_le_bytes());
        data[149..153].copy_from_slice(&self.blims_motor_position.to_le_bytes());
        data[153] = self.blims_phase_id as u8;
        data[154..158].copy_from_slice(&self.blims_pid_p.to_le_bytes());
        data[158..162].copy_from_slice(&self.blims_pid_i.to_le_bytes());
        data[162..166].copy_from_slice(&self.blims_bearing.to_le_bytes());
        data[166] = self.blims_loiter_step as u8;
        data[167..171].copy_from_slice(&self.blims_heading_des.to_le_bytes());
        data[171..175].copy_from_slice(&self.blims_heading_error.to_le_bytes());
        data[175..179].copy_from_slice(&self.blims_error_integral.to_le_bytes());
        data[179..183].copy_from_slice(&self.blims_dist_to_target_m.to_le_bytes());
        data[183..187].copy_from_slice(&self.blims_target_lat.to_le_bytes());
        data[187..191].copy_from_slice(&self.blims_target_lon.to_le_bytes());
        data[191..195].copy_from_slice(&self.blims_wind_from_deg.to_le_bytes());
        data[195..199].copy_from_slice(&self.ms_since_boot_cfc.to_le_bytes());
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
            h_acc:     u32::from_le_bytes(bytes[96..100].try_into().unwrap()),
            v_acc:     u32::from_le_bytes(bytes[100..104].try_into().unwrap()),
            vel_n:     f64::from_le_bytes(bytes[104..112].try_into().unwrap()),
            vel_e:     f64::from_le_bytes(bytes[112..120].try_into().unwrap()),
            vel_d:     f64::from_le_bytes(bytes[120..128].try_into().unwrap()),
            g_speed:   f64::from_le_bytes(bytes[128..136].try_into().unwrap()),
            s_acc:     u32::from_le_bytes(bytes[136..140].try_into().unwrap()),
            head_acc:  u32::from_le_bytes(bytes[140..144].try_into().unwrap()),
            fix_type:  bytes[144],
            head_mot:  i32::from_le_bytes(bytes[145..149].try_into().unwrap()),
            blims_motor_position:   f32::from_le_bytes(bytes[149..153].try_into().unwrap()),
            blims_phase_id:         bytes[153] as i8,
            blims_pid_p:            f32::from_le_bytes(bytes[154..158].try_into().unwrap()),
            blims_pid_i:            f32::from_le_bytes(bytes[158..162].try_into().unwrap()),
            blims_bearing:          f32::from_le_bytes(bytes[162..166].try_into().unwrap()),
            blims_loiter_step:      bytes[166] as i8,
            blims_heading_des:      f32::from_le_bytes(bytes[167..171].try_into().unwrap()),
            blims_heading_error:    f32::from_le_bytes(bytes[171..175].try_into().unwrap()),
            blims_error_integral:   f32::from_le_bytes(bytes[175..179].try_into().unwrap()),
            blims_dist_to_target_m: f32::from_le_bytes(bytes[179..183].try_into().unwrap()),
            blims_target_lat:       f32::from_le_bytes(bytes[183..187].try_into().unwrap()),
            blims_target_lon:       f32::from_le_bytes(bytes[187..191].try_into().unwrap()),
            blims_wind_from_deg:    f32::from_le_bytes(bytes[191..195].try_into().unwrap()),
            ms_since_boot_cfc:      u32::from_le_bytes(bytes[195..199].try_into().unwrap()),
        }
    }

    pub const CSV_HEADER: &'static str = "flight_mode,pressure,temp,altitude,latitude,longitude,num_satellites,timestamp,mag_x,mag_y,mag_z,accel_x,accel_y,accel_z,gyro_x,gyro_y,gyro_z,pt3,pt4,rtd,sv_open,mav_open,ssa_drogue_deployed,ssa_main_deployed,cmd_n1,cmd_n2,cmd_n3,cmd_n4,cmd_a1,cmd_a2,cmd_a3,airbrake_state,predicted_apogee,h_acc,v_acc,vel_n,vel_e,vel_d,g_speed,s_acc,head_acc,fix_type,head_mot,blims_motor_position,blims_phase_id,blims_pid_p,blims_pid_i,blims_bearing,blims_loiter_step,blims_heading_des,blims_heading_error,blims_error_integral,blims_dist_to_target_m,blims_target_lat,blims_target_lon,blims_wind_from_deg,ms_since_boot_cfc\n";

    pub fn to_csv(&self, buf: &mut [u8]) -> usize {
        use core::fmt::Write;
        let mut wrapper = WriteWrapper::new(buf);
        let _ = write!(
            wrapper,
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
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
            self.predicted_apogee,
            self.h_acc,
            self.v_acc,
            self.vel_n,
            self.vel_e,
            self.vel_d,
            self.g_speed,
            self.s_acc,
            self.head_acc,
            self.fix_type,
            self.head_mot,
            self.blims_motor_position,
            self.blims_phase_id,
            self.blims_pid_p,
            self.blims_pid_i,
            self.blims_bearing,
            self.blims_loiter_step,
            self.blims_heading_des,
            self.blims_heading_error,
            self.blims_error_integral,
            self.blims_dist_to_target_m,
            self.blims_target_lat,
            self.blims_target_lon,
            self.blims_wind_from_deg,
            self.ms_since_boot_cfc,
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
