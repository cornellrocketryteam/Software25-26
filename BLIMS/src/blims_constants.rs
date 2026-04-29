/// BLIMS Constants

/// Motor Representation: we want to represent normalized PWM values (between 0 and 1) in actual inches to represent physical brakeline differential values
/// We want to have a max of 4.5 inches of pull to either side (-9 to 9 inches of differential)
/// `set_motor_position()` is the single conversion point to PWM:
///
///   pwm_normalized = (position_in − MOTOR_MIN) / (MOTOR_MAX − MOTOR_MIN)
///                  = (position_in + 9) / 18       ∈ [0, 1]
///   duty_cycle     = 5 % + pwm_normalized × 5 %   ∈ [5 %, 10 %]

///________________________|____left_turn____|___neutral___|__right_turn__|
/// pwm.                   |        0        |     0.5     |       1      |
/// controller output.     |       -9        |      0      |       9      |
/// brakeline differential |      -9 in.     |     0 in.   |      9 in.   |
/// motor turns            |      -10.5      |      0.     |      10.5    |

/// ** Right turn = positive inches.  Left turn = negative inches. **

/// ## ODrive configuration dependency
///
/// This mapping requires the ODrive to be configured so that the full
/// 5–10 % duty-cycle range (pwm_normalized 0 → 1) corresponds to
/// exactly ±10.5 motor turns.  If `gpio8_pwm_mapping` min/max turns
/// are ever changed, MOTOR_MIN/MAX here must be updated to match,
/// and the PI gains must be rescaled accordingly (see below).
///
/// ## Gain rescaling from old [0, 1] PWM representation
///
///   Old: NEUTRAL=0.5,  authority from neutral = MOTOR_MAX−0.5 = 0.2
///   New: NEUTRAL=0.0,  authority from neutral = MOTOR_MAX−0.0 = 9.0
///   Scale factor: 9.0 / 0.2 = 45
///
///   new_Kp           = 0.009 × 45 = 0.405
///   new_Ki           = 0.001 × 45 = 0.045
///   new_INTEGRAL_MAX = 10.0  × 45 = 450
///
/// This preserves identical closed-loop behaviour — a given heading error
/// applies the same *fraction* of total brakeline authority as before.
///
/// ## MVP altitude thresholds (from flight plan, image 2)
///
///   Main deploy ≈ 2000 ft AGL  (FSW activates BLiMS)
///   Upwind leg:   2000 → 1000 ft   head into wind
///   Downwind leg: 1000 →  200 ft   head with wind toward target
///   Neutral:              < 200 ft  hands-off landing flare

pub const M_PI: f32 = 3.141_592_653_589_793_238_462_643_383_279_502_88;

// unit conversions
pub const DEG_TO_RAD: f32 = M_PI / 180.0;
pub const RAD_TO_DEG: f32 = 180.0 / M_PI;
pub const FT_PER_M: f32 = 3.2808; // feet per meter conversion

//pub const BRAKE_ALT: f32 = 10.0; - old touchdown threshold

pub const INITIAL_HOLD_THRESHOLD: u32 = 10000; // parafoil stabilization
// want wrap to be as large as possible, increases the amount of steps 
pub const WRAP_CYCLE_COUNT: u16 = 65_535;

// Motor Position Limits (inches of differential) 
// conversion to raw PWM is done in `set_motor_position()` - every call to this this function uses these limits 
pub const NEUTRAL_POS: f32 = 0.0; // straight flight: neutral, both lines equal
pub const MOTOR_MIN: f32 = -9.0;   // max left: -9 inches, motor reeled 9 in. left of neutral
pub const MOTOR_MAX: f32 = 9.0;   // max right 9 inches, motor reeled 9 in. right of neutral

// Maximum number of wind profile layers
pub const MAX_WIND_LAYERS: usize = 20;

// PI controller gains — revalidate via car testing
// Units: KP  [in/°],  KI  [in/(°·s)],  INTEGRAL_MAX  [°]
//
// At 10° error:  p_term = 0.405 × 10 = 4.05 in  (45 % authority) — healthy.
// At 90° error:  p_term = 0.405 × 90 = 36.5 in  → saturated at 9 in — same
//                saturation behaviour as old gains in old units.
pub const ALPHA: f32 = 0.1;
/// Anti-windup integral clamp.  450 °·s at 20 Hz means the integrator
/// saturates after ~22 s of 1 °/s steady-state error, keeping the I-term
/// meaningful but bounded.  Tune this from flight data.
pub const INTEGRAL_MAX: f32 = 450.0;
pub const KP: f32 = 0.405;
pub const KI: f32 = 0.045;

// MVP L3 altitude thresholds (feet AGL) 
// upwind/downwind corssover threshold 
// upwind: head into wind_from
// downwind: when above alt_neutral_ft, downwind (head with wind)
pub const ALT_UPWIND_FT: f32 = 1000.0; 
pub const ALT_NEUTRAL_FT: f32  =  200.0; // hands off for landing flare (motor at 0 in.)

// Minimum groundspeed for reliable GPS heading (mm/s)
pub const GSPEED_MIN_FOR_HEADING: i32 = 3_000; // 3 m/s








