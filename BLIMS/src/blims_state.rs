#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum BlimsMode {
    #[default]
    Standby, // system initialised but not active
    Lv,      // Launch Vehicle – full L3 logic
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(i8)]
pub enum Phase {
    Held     = 0, // GPS invalid or not ready  → motor neutral
    Track    = 1, // >1000 ft + far from target → PI toward target
    Downwind = 2, // 600–1000 ft               → fly with wind
    Base     = 3, // 300–600 ft                → fly crosswind
    Final    = 4, // 100–300 ft                → fly into wind
    Neutral  = 5, // <100 ft                   → hands off
    Loiter   = 6, // >1000 ft + near target    → spiral to bleed altitude
}

 
#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(i8)]
pub enum LoiterStep {
    TurnRight  = 0,
    PauseRight = 1,
    TurnLeft   = 2,
    PauseLeft  = 3,
}

 
#[derive(Debug, Default, Clone)]
pub struct BlimsDataIn {
    // Position
    pub lon: i32,         // degrees × 1e7
    pub lat: i32,         // degrees × 1e7
 
    // Altitude (BMP390 barometer, processed by FSW)
    pub altitude_ft: f32, // AGL in feet
 
    // Accuracy estimates
    pub h_acc: u32,        // horizontal accuracy  mm
    pub v_acc: u32,        // vertical accuracy    mm
 
    // Velocity
    pub vel_n: i32,        // north velocity  mm/s
    pub vel_e: i32,        // east velocity   mm/s
    pub vel_d: i32,        // down velocity   mm/s  (positive = descending)
 
    // Speed / heading
    pub g_speed:  i32,     // ground speed         mm/s
    pub head_mot: i32,     // heading of motion    degrees × 1e5
 
    // Accuracy estimates
    pub s_acc:    u32,     // speed accuracy       mm/s
    pub head_acc: u32,     // heading accuracy     degrees × 1e5
 
    // GPS status
    pub fix_type:   u8,    // 0=none  2=2D  3=3D  4=3D+DGPS
    pub gps_state: bool,  // validity flag from FSW
}

 
#[derive(Debug, Default, Clone)]
pub struct BlimsDataOut {
    pub motor_position: f32, // 0.3–0.7, neutral = 0.5
    pub pid_p:   f32,
    pub pid_i:   f32,
    pub bearing: f32,        // bearing to target, degrees (0 = North CW)
    pub phase_id:    i8,     // current Phase cast to i8
    pub loiter_step: i8,     // current LoiterStep cast to i8 (valid in Loiter only)
}
 