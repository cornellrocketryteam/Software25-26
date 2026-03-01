use crate::blims_constants::*;

pub const MAX_WIND_LAYERS: usize = 0; 

#[derive(Debug)]
pub enum BLIMSMode
{
    STANDBY,    //system initialized but not active  
    LV //Launch Vehicle - Full L3 logic 
} 

// * BLIMSDataIn: struct for data coming from FSK to BLiMS controller 
// * passed to BLiMS::execute() ever iteration (50ms of 20Hz)
// * All GPS fields come directly from the u-blox MAX-M10M UBX-NAV-PVT message.

#[derive(Debug)]
pub struct BLIMSDataIn 
{
    //Position (from GPS)
    pub lon: i32,  ///Longitude in degrees * 1e7
    pub lat: i32,  ///Latitude in degrees 1e7
    
    //Altitude (from BMP390 barmometer, processed from FSW)
    pub altitude_ft: f32, //Altitude AGL in feet

    //Accurancy estimates (from GPS)
    pub h_acc: u32,  //Horizontal accurancy estimate in mm
    pub v_acc: u32,  //Vertical accurancy estimate in mm 

    //Velocity (from GPS)
    pub vel_n: i32, //velocity north in mm/s
    pub vel_e: i32, //velocity east in mm/s
    pub vel_d: i32, //down velocity in mm/positive = descending

    //Speed and heading (from GPS)
    pub g_speed: i32,   //Ground speed in mm/s 
    pub head_mot: i32,  //heading accuracy estimate in degrees * 1e5

    //Accuracy estimates (from GPS)
    pub s_acc: u32,     //speed accuracy estimate in mm/s
    pub head_acc: u32,  //heading of motion accuracy estimate in degrees * 1e5

    //GPS status 
    pub fix_type: u8,   //GPS fix type: 0=none, 2=2D, 3=3D, 4=3D+DGPS
    pub gps_state: bool //GPS validity flag from FSW
}

#[derive(Clone, Copy, Default,Debug)]
pub struct BLIMSDataOut
{
    pub motor_position: f32, //neutrial is 0.5, bound btwn 0.3 and 0.7, range is 0 to 1
    pub pid_p: f32,
    pub pid_i: f32, 
    pub bearing: f32, //0 = North CW 
    // -1 = held, 0 = track, 1 = downwind, 2 = base, 3 = final, 4 = neutral, 5 = held
    pub phase_id: i8, //current flight phase (0-6): 0=HELD, 1=TRACK, 2=DOWNWIND, 3=BASE, 4=FINAL, 5=NEUTRAL, 6=LOITER
    pub loiter_step: i8 //current loiter sub-state (0-3), only valid in LOITER phase  
}

#[derive(Debug)]
pub struct Flight {
    pub blims_pwm_pin: u8,
    pub blims_enable_pin: u8,
    pub blims_init: bool,
    pub flight_mode: BLIMSMode,
    pub motor_position: f32,
    pub data_out: BLIMSDataOut,

    //Processed via GPS
    pub gps_lon: f32,
    pub gps_lat: f32,
    pub altitude_ft: f32, 
    pub h_acc: u32,
    pub v_acc: u32,
    pub vel_n: i32,
    pub vel_e: i32,
    pub vel_d: i32,
    pub g_speed: i32,
    pub head_mot: i32,
    pub s_acc: u32,
    pub head_acc: u32,
    pub fix_type: u8,

    pub curr_time: u32,
    pub prev_time: u32,
    pub time_passed: u32,//double check, but can use this for loiter time block?

    }

impl Default for Flight{
    fn default() -> Self {
        Self {
            blims_pwm_pin: 0,
            blims_enable_pin: 0,
            blims_init: false,
            flight_mode: BLIMSMode::STANDBY,
            motor_position: 0.0,
            data_out: BLIMSDataOut::default(),
            gps_lon: 0.0,
            gps_lat: 0.0,
            altitude_ft: 0.0,
            h_acc: 0,
            v_acc: 0,
            vel_n: 0,
            vel_e: 0,
            vel_d: 0,
            g_speed: 0,
            head_mot: 0,
            s_acc: 0,
            head_acc: 0,
            curr_time: 0,
            prev_time: 0,
            time_passed: 0,
            fix_type: 0,
        }
    }
 }
#[derive(Debug)]
pub struct LV {
    pub target_lat: f32, // set in begin
    pub target_lon: f32, // set in begin
    pub bearing: f32,
    pub prev_error: f32,
    pub pid_p: f32,
    pub pid_i: f32,
    pub gps_state: bool,
    pub error_integral: f32,

    // NEW (L3-1): wind direction "FROM" (deg 0-360), uploaded preflight
    pub wind_from_deg: f32,
    pub wind_profile_size: usize, 
    pub wind_altitudes_m: [f32;MAX_WIND_LAYERS], 
    pub wind_dirs_deg: [f32;MAX_WIND_LAYERS],

}

impl Default for LV {
    fn default() -> Self {
        Self {
            target_lat: 0.0, // set in begin
            target_lon: 0.0, // set in begin
            bearing: 0.0,
            prev_error: 0.0,
            pid_p: 0.0,
            pid_i: 0.0,
            gps_state: false,
            error_integral: 0.0,

            // NEW (L3-1): wind direction "FROM" (deg 0-360), uploaded preflight
            wind_from_deg: 0.0,
            wind_profile_size: 0, 
            wind_altitudes_m: [0.0; MAX_WIND_LAYERS], 
            wind_dirs_deg: [0.0; MAX_WIND_LAYERS],
        }
    }
}

pub struct MVP {
    pub CURR_ACTION_DURATION: i32,
    pub CURR_ACTION_INDEX: i32
}

impl Default for MVP {
    fn default() -> Self {
        Self {
            CURR_ACTION_DURATION: 0,
            CURR_ACTION_INDEX: 0

        }
    }
} 


#[derive(Debug, Default)]
pub struct BLIMSState {
    pub flight: Flight,
    pub lv:     LV,
}








