use crate::blims_constants::*;

// ============================================================================
// Types used by the new Blims controller (blims.rs)
// ============================================================================

/// Flight phase — value matches the integer logged in the CSV output.
#[repr(i8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Phase {
    Held     = 0,
    Track    = 1,
    Downwind = 2,
    Base     = 3,
    Final    = 4,
    Neutral  = 5,
    Loiter   = 6,
}

/// Sub-state within the Loiter phase.
#[repr(i8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LoiterStep {
    TurnRight  = 0,
    PauseRight = 1,
    TurnLeft   = 2,
    PauseLeft  = 3,
}

/// Sensor data passed into Blims::execute() every cycle.
/// All GPS fields come directly from the u-blox UBX-NAV-PVT message.
#[derive(Debug)]
pub struct BlimsDataIn {
    // Position
    pub lon: i32, // Longitude  × 1e7
    pub lat: i32, // Latitude   × 1e7

    // Altitude (from barometer, processed by FSW)
    pub altitude_ft: f32, // Altitude AGL in feet

    // Accuracy estimates
    pub h_acc: u32, // Horizontal accuracy  (mm)
    pub v_acc: u32, // Vertical accuracy    (mm)

    // Velocity
    pub vel_n: i32, // North velocity (mm/s)
    pub vel_e: i32, // East  velocity (mm/s)
    pub vel_d: i32, // Down  velocity (mm/s, positive = descending)

    // Speed and heading
    pub g_speed:  i32, // Ground speed            (mm/s)
    pub head_mot: i32, // Heading of motion × 1e5 (degrees)

    // Accuracy estimates
    pub s_acc:    u32, // Speed accuracy   (mm/s)
    pub head_acc: u32, // Heading accuracy × 1e5 (degrees)

    // GPS status
    pub fix_type:  u8,   // 0=none, 2=2D, 3=3D, 4=3D+DGPS
    pub gps_state: bool, // validity flag from FSW
}

/// Control outputs returned from Blims::execute() every cycle.
#[derive(Clone, Copy, Default, Debug)]
pub struct BlimsDataOut {
    pub motor_position: f32, // 0.3–0.7; 0.5 = neutral
    pub pid_p: f32,
    pub pid_i: f32,
    pub bearing: f32,    // bearing to target, degrees [0, 360)
    pub phase_id: i8,    // Phase as integer (0–6)
    pub loiter_step: i8, // LoiterStep as integer (0–3); 0 outside Loiter
}

// ============================================================================
// Legacy types — kept for backwards compatibility
// ============================================================================

#[derive(Debug)]
pub enum BLIMSMode {
    STANDBY,
    LV,
}

#[derive(Debug)]
pub struct BLIMSDataIn {
    pub lon:         i32,
    pub lat:         i32,
    pub altitude_ft: f32,
    pub h_acc:       u32,
    pub v_acc:       u32,
    pub vel_n:       i32,
    pub vel_e:       i32,
    pub vel_d:       i32,
    pub g_speed:     i32,
    pub head_mot:    i32,
    pub s_acc:       u32,
    pub head_acc:    u32,
    pub fix_type:    u8,
    pub gps_state:   bool,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct BLIMSDataOut {
    pub motor_position: f32,
    pub pid_p:          f32,
    pub pid_i:          f32,
    pub bearing:        f32,
    pub phase_id:       i8,
    pub loiter_step:    i8,
}

#[derive(Debug)]
pub struct Flight {
    pub blims_pwm_pin:    u8,
    pub blims_enable_pin: u8,
    pub blims_init:       bool,
    pub flight_mode:      BLIMSMode,
    pub motor_position:   f32,
    pub data_out:         BLIMSDataOut,
    pub gps_lon:          f32,
    pub gps_lat:          f32,
    pub altitude_ft:      f32,
    pub h_acc:            u32,
    pub v_acc:            u32,
    pub vel_n:            i32,
    pub vel_e:            i32,
    pub vel_d:            i32,
    pub g_speed:          i32,
    pub head_mot:         i32,
    pub s_acc:            u32,
    pub head_acc:         u32,
    pub fix_type:         u8,
    pub curr_time:        u32,
    pub prev_time:        u32,
    pub time_passed:      u32,
}

impl Default for Flight {
    fn default() -> Self {
        Self {
            blims_pwm_pin:    0,
            blims_enable_pin: 0,
            blims_init:       false,
            flight_mode:      BLIMSMode::STANDBY,
            motor_position:   0.0,
            data_out:         BLIMSDataOut::default(),
            gps_lon:          0.0,
            gps_lat:          0.0,
            altitude_ft:      0.0,
            h_acc:            0,
            v_acc:            0,
            vel_n:            0,
            vel_e:            0,
            vel_d:            0,
            g_speed:          0,
            head_mot:         0,
            s_acc:            0,
            head_acc:         0,
            fix_type:         0,
            curr_time:        0,
            prev_time:        0,
            time_passed:      0,
        }
    }
}

#[derive(Debug)]
pub struct LV {
    pub target_lat:        f32,
    pub target_lon:        f32,
    pub bearing:           f32,
    pub prev_error:        f32,
    pub pid_p:             f32,
    pub pid_i:             f32,
    pub gps_state:         bool,
    pub error_integral:    f32,
    pub wind_from_deg:     f32,
    pub wind_profile_size: usize,
    pub wind_altitudes_m:  [f32; MAX_WIND_LAYERS],
    pub wind_dirs_deg:     [f32; MAX_WIND_LAYERS],
}

impl Default for LV {
    fn default() -> Self {
        Self {
            target_lat:        0.0,
            target_lon:        0.0,
            bearing:           0.0,
            prev_error:        0.0,
            pid_p:             0.0,
            pid_i:             0.0,
            gps_state:         false,
            error_integral:    0.0,
            wind_from_deg:     0.0,
            wind_profile_size: 0,
            wind_altitudes_m:  [0.0; MAX_WIND_LAYERS],
            wind_dirs_deg:     [0.0; MAX_WIND_LAYERS],
        }
    }
}

#[derive(Debug, Default)]
pub struct BLIMSState {
    pub flight: Flight,
    pub lv:     LV,
}
