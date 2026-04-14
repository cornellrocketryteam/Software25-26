////rust float defaults f64

pub const M_PI: f32 = 3.14159265358979323846264338327950288;

//unit conversions
pub const DEG_TO_RAD: f32 = M_PI/180.0; 
pub const RAD_TO_DEG: f32 = 180.0/M_PI; 
pub const FT_PER_M: f32 = 3.2808; // feet per meter conversion 

//replaced by ALT_NEUTRAL_FT - no longer our brake alt 
pub const BRAKE_ALT: f32 = 10.0; // to be updated for when we want BLiMS to brake 

pub const INITIAL_HOLD_THRESHOLD: u32 = 10000; //parafoil stabilization 
// want wrap to be as large as possible, increases the amount of steps so that we have as much control as possible 
pub const WRAP_CYCLE_COUNT: u16 = 65535; 

////////////MVP Specific Constant??????
pub const TURN_HOLD_THRESHOLD: u32 = 10000; //10 sec per turn 
pub const NEUTRAL_HOLD_THRESHOLD: u32 = 7500; // 7.5 sec neutral 

// 0.0 to 1.0 maps to ODrive configurations of -17 to 17 turns 
pub const NEUTRAL_POS: f32 = 0.5; //straight flight 
pub const MOTOR_MIN: f32 = 0.3; //max left 
pub const MOTOR_MAX: f32 = 0.7; //max right 


/////////LV Specific Constants///////
pub const ALPHA: f32 = 0.1;      //low pass filter value. Higher values increase resistance to noise but slow down the responsivness of the data to fast changing values - unused in current implementation
pub const INTEGRAL_MAX: f32 = 10.0;   //clamp value for integral term to prevent too much integral windup
//revalidate via car testing**
pub const KP: f32 = 0.009;       //for controller 
pub const KI: f32 = 0.001;       //for controller -revalidate


////////L3-1 Landing Pattern: altitude-band U-turn, WIND-FORM. feet AGL////
pub const ALT_DOWNWIND_FT: f32 = 1000.0; //begin landing pattern 
pub const ALT_BASE_FT: f32  = 600.0; //turn perpendiular to downwind
pub const ALT_FINAL_FT: f32 = 300.0; //turn into wind
pub const ALT_NEUTRAL_FT: f32     = 100.0; //hands off for landing flare

// Set radius ("vicinity") + Loiter / altitude bleed
// Units: feet (horizontal distance) and milliseconds
pub const SET_RADIUS_FT: f32 = 400.0;      // TODO: pick based on field + expected drift

// Loiter pattern: alternate turn right/neutral/left/neutral...
pub const LOITER_TURN_DURATION_MS: u32 = 6000;    // 6s per turn 
pub const LOITER_PAUSE_DURATION_MS: u32  = 2500;  // 2.5s pause between turns
pub const LOITER_RIGHT_POS: f32 = 0.65;            // Right turn position
pub const LOITER_LEFT_POS: f32 = 0.35;             // Left turn position

pub const MAX_WIND_LAYERS: usize = 20;








