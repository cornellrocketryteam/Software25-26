use serde::{Deserialize, Serialize};

/// Number of comma-separated fields in a `$TELEM,` line, matching the FSW
/// emitter in `fsw/src/umbilical.rs`. Must be kept in sync on both sides.
pub const TELEM_FIELD_COUNT: usize = 57;

/// FSW telemetry packet parsed from CSV text lines.
/// The FSW emits lines like: `$TELEM,0,101325.0,25.0,0.0,...,0,0\n`
/// Field order must match `fsw/src/umbilical.rs::emit_telemetry`.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct FswTelemetry {
    pub flight_mode: u32,
    pub pressure: f32,       // Pa
    pub temp: f32,           // C
    pub altitude: f32,       // m
    pub latitude: f32,       // degrees
    pub longitude: f32,      // degrees
    pub num_satellites: u32,
    pub gps_time: f32,        // s since midnight UTC
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
    // valve states (sv_open is FSW-side SV2 on the wire)
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
    // airbrake
    pub airbrake_deployment: f32,
    pub predicted_apogee: f32,
    // u-blox advanced GPS
    pub h_acc: u32,
    pub v_acc: u32,
    pub vel_n: f64,
    pub vel_e: f64,
    pub vel_d: f64,
    pub g_speed: f64,
    pub s_acc: u32,
    pub head_acc: u32,
    pub fix_type: u8,
    pub head_mot: i32,
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
    // CFC boot time
    pub ms_since_boot_cfc: u32,
}

impl FswTelemetry {
    /// Parse from a CSV field slice (the fields after the `$TELEM,` prefix).
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
            gps_time:       fields[7].trim().parse().ok()?,
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
            ssa_drogue_deployed: fields[22].trim().parse().ok()?,
            ssa_main_deployed:   fields[23].trim().parse().ok()?,
            cmd_n1:         fields[24].trim().parse().ok()?,
            cmd_n2:         fields[25].trim().parse().ok()?,
            cmd_n3:         fields[26].trim().parse().ok()?,
            cmd_n4:         fields[27].trim().parse().ok()?,
            cmd_a1:         fields[28].trim().parse().ok()?,
            cmd_a2:         fields[29].trim().parse().ok()?,
            cmd_a3:         fields[30].trim().parse().ok()?,
            airbrake_deployment: fields[31].trim().parse().ok()?,
            predicted_apogee: fields[32].trim().parse().ok()?,
            h_acc:          fields[33].trim().parse().ok()?,
            v_acc:          fields[34].trim().parse().ok()?,
            vel_n:          fields[35].trim().parse().ok()?,
            vel_e:          fields[36].trim().parse().ok()?,
            vel_d:          fields[37].trim().parse().ok()?,
            g_speed:        fields[38].trim().parse().ok()?,
            s_acc:          fields[39].trim().parse().ok()?,
            head_acc:       fields[40].trim().parse().ok()?,
            fix_type:       fields[41].trim().parse().ok()?,
            head_mot:       fields[42].trim().parse().ok()?,
            blims_motor_position:   fields[43].trim().parse().ok()?,
            blims_phase_id:         fields[44].trim().parse().ok()?,
            blims_pid_p:            fields[45].trim().parse().ok()?,
            blims_pid_i:            fields[46].trim().parse().ok()?,
            blims_bearing:          fields[47].trim().parse().ok()?,
            blims_loiter_step:      fields[48].trim().parse().ok()?,
            blims_heading_des:      fields[49].trim().parse().ok()?,
            blims_heading_error:    fields[50].trim().parse().ok()?,
            blims_error_integral:   fields[51].trim().parse().ok()?,
            blims_dist_to_target_m: fields[52].trim().parse().ok()?,
            blims_target_lat:       fields[53].trim().parse().ok()?,
            blims_target_lon:       fields[54].trim().parse().ok()?,
            blims_wind_from_deg:    fields[55].trim().parse().ok()?,
            ms_since_boot_cfc:      fields[56].trim().parse().ok()?,
        })
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
