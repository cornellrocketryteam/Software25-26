//Phase::Upwind   (alt > 1000 ft) — PI-steer to `wind_from` heading.
///       Observes: does the parafoil respond to a sustained heading hold?
///       What are the steady-state motor commands needed to fight crosswind?
///
///   Phase::Downwind (200 ft < alt ≤ 1000 ft) — PI-steer to `wind_from + 180°`.
///       Observes: how quickly does the canopy reverse direction?
///       What turn rate and settling time does the PI controller produce?
///
///   Phase::Neutral  (alt ≤ 200 ft) — motor to neutral, hands off.
///
///   Phase::Held     — GPS invalid; motor to neutral.
///
/// The full landing-pattern state machine (Track / Loiter / Downwind-leg /
/// Base / Final) is intentionally absent in this MVP branch.  Re-integrate
/// from `full-blims` once parafoil dynamics are characterised from flight data.


use embassy_rp::gpio::Output;
use embassy_rp::pwm::{Config as PwmConfig, Pwm};
use embassy_time::{Duration, Instant};
 
use crate::blims_constants::*;
use crate::blims_state::{BlimsDataIn, BlimsDataOut, Phase};

/// Opaque hardware bundle consumed by [`Blims::new`]. - bundles hardware resources into one package
// single entry point, same scope, these are moved into blims struct, enforces initialization
pub struct Hardware<'d> {
    pub pwm:        Pwm<'d>,
    pub pwm_config: PwmConfig,
    pub enable_pin: Output<'d>,
}

pub struct Blims<'d> {
    // ── Embassy HAL handles ──────────────────────────────────────────────────
    pwm:        Pwm<'d>,
    pwm_config: PwmConfig,
    enable_pin: Output<'d>,

    // ── Pre-flight navigation config ─────────────────────────────────────────
    target_upwind_lat: f32,
    target_upwind_lon: f32,
    target_downwind_lat: f32,
    target_downwind_lon: f32,

    activation_time_ms: Option<u64>, // timestamp of first transition out of Held phase, used for InitialHold delay
    /// Surface-level wind direction (degrees FROM, 0 = N, 90 = E).
    /// Used as fallback when no altitude-aware profile is loaded.
    wind_from_deg:     f32,
    ///altitude-aware wind profile loaded before flight
    wind_profile_size: usize,
    wind_altitudes_m:  [f32; MAX_WIND_LAYERS],
    wind_dirs_deg:     [f32; MAX_WIND_LAYERS],

    // ── GPS snapshot (refreshed each execute()) ──────────────────────────────
    gps_lat:   f32,
    gps_lon:   f32,
    /// headMot: heading of motion, degrees × 1e5 (raw from GPS)
    head_mot:  i32,
    fix_type:  u8,
    gps_state: bool,

    // ── PI controller state ──────────────────────────────────────────────────
    error_integral: f32,
    pid_p: f32,
    pid_i: f32,

    // ── Phase / motor state ──────────────────────────────────────────────────
    last_phase:     Phase,
    bearing:        f32,
    brakeline_diff_in: f32,   // was: motor_position: f32

    // ── Timing (ms since boot; mirrors blims::flight::currTime/prevTime) ─────
    curr_time_ms: u64,
    prev_time_ms: u64,
}

impl<'d> Blims<'d> {
    // -------------------------------------------------------------------------
    // Construction  (mirrors BLIMS::begin())
    // -------------------------------------------------------------------------

    /// Initialise BLiMS.
    ///
    /// `pwm` and `pwm_config` must already be configured for 50 Hz with
    /// `top = WRAP_CYCLE_COUNT` and the correct clock divider (see main.rs).
    /// The enable pin is driven high immediately to activate the motor driver,
    /// then the motor is parked at neutral.
    pub fn new(hw: Hardware<'d>) -> Self {
        let mut b = Self {
            pwm:        hw.pwm,
            pwm_config: hw.pwm_config,
            enable_pin: hw.enable_pin,

            target_upwind_lat:   0.0,
            target_upwind_lon:   0.0,
            target_downwind_lat: 0.0,
            target_downwind_lon: 0.0,
            activation_time_ms:  None,  
            wind_from_deg:     0.0,
            wind_profile_size: 0,
            wind_altitudes_m:  [0.0; MAX_WIND_LAYERS],
            wind_dirs_deg:     [0.0; MAX_WIND_LAYERS],

            gps_lat:   0.0,
            gps_lon:   0.0,
            head_mot:  0,
            fix_type:  0,
            gps_state: false,

            error_integral: 0.0,
            pid_p: 0.0,
            pid_i: 0.0,

            last_phase:     Phase::Held,
            bearing:        0.0,
            brakeline_diff_in: NEUTRAL_POS, //now 0.0

            curr_time_ms: 0,
            prev_time_ms: 0,
        };
        //hold enable high continuously so motor driver stays armed
        b.enable_pin.set_high();
        //neutral until first execute() call
        b.set_brakeline_diff(NEUTRAL_POS);
        b
    }

    // -------------------------------------------------------------------------
    // Pre-flight setters
    // -------------------------------------------------------------------------

    pub fn set_upwind_target(&mut self, lat: f32, lon: f32) {
        self.target_upwind_lat = lat;
        self.target_upwind_lon = lon;
    }

    pub fn set_downwind_target(&mut self, lat: f32, lon: f32) {
        self.target_downwind_lat = lat;
        self.target_downwind_lon = lon;
    }

    //when no altitude-layered profile is available - single surface-level wind-from direction
    pub fn set_wind_from_deg(&mut self, deg: f32) {
        self.wind_from_deg = Self::wrap360(deg);
    }

    /// Load a multi-layer wind profile. Arrays must be the same length;
    /// excess layers beyond MAX_WIND_LAYERS are silently truncated.
    pub fn set_wind_profile(&mut self, altitudes_m: &[f32], directions_deg: &[f32]) {
        let size = altitudes_m
            .len()
            .min(directions_deg.len())
            .min(MAX_WIND_LAYERS);
        self.wind_profile_size = size;
        self.wind_altitudes_m[..size].copy_from_slice(&altitudes_m[..size]);
        self.wind_dirs_deg[..size].copy_from_slice(&directions_deg[..size]);
    }

    // -------------------------------------------------------------------------
    // Main control loop
    // -------------------------------------------------------------------------

    pub fn execute(&mut self, data_in: &BlimsDataIn) -> BlimsDataOut {

        self.prev_time_ms = self.curr_time_ms;
        self.curr_time_ms = Instant::now().as_millis();
        let dt_ms = self.curr_time_ms.saturating_sub(self.prev_time_ms);
       
        // clamp dt: ignore first call (prev = 0) and any gap > 200 ms like after a reset, to avoid integral-windup spikes on the first live cycle
        let dt = if self.prev_time_ms == 0 || dt_ms > 200 {
            0.05_f32
        } else {
            dt_ms as f32 / 1000.0
        };

        //intake sensor data
        self.gps_lat   = data_in.lat as f32 * 1e-7;
        self.gps_lon   = data_in.lon as f32 * 1e-7;
        self.head_mot  = data_in.head_mot;
        self.fix_type  = data_in.fix_type;
        self.gps_state = data_in.gps_state;
        let altitude_ft = data_in.altitude_ft;

        let gps_valid = self.gps_state && self.fix_type >= 2;
        let current_phase = self.determine_phase(altitude_ft, gps_valid);

        // ── Phase-change housekeeping ─────────────────────────────────────────
        if current_phase != self.last_phase {
            // Always zero integral on phase change to prevent windup carryover
            // Reset integral on every phase transition.
            //
            // Critical for the Upwind → Downwind reversal: the setpoint flips
            // 180°, so any accumulated integral from the previous phase would
            // immediately drive the motor in the wrong direction.  A clean
            // slate is always safer than carrying over a potentially stale term.
            self.error_integral = 0.0;
            self.last_phase = current_phase;
        }

        self.bearing = match current_phase {
            Phase::Upwind   => self.calculate_bearing_to(
                                self.target_upwind_lat, self.target_upwind_lon),
            Phase::Downwind => self.calculate_bearing_to(
                                self.target_downwind_lat, self.target_downwind_lon),
    _                       => self.bearing, // hold last value
};

        match current_phase {
            Phase::Held | Phase::InitialHold => {
                // No active control – park motor at neutral
                self.pid_p = 0.0;
                self.pid_i = 0.0;
                self.set_brakeline_diff(NEUTRAL_POS);
            }

            Phase::Upwind | Phase::Downwind => {
                // PI heading control.
                //
                // desired heading is resolved from wind direction (see
                // get_desired_heading); current heading comes from GPS
                // heading-of-motion (degrees × 1e5 → degrees).
                let desired = self.get_desired_heading(
                    current_phase, altitude_ft,
                );
                // headMot is degrees × 1e5 → convert to degrees
                let current_heading = self.head_mot as f32 * 1e-5;
                self.execute_pi_control(desired, current_heading, dt);
            }

            Phase::Neutral => {
                self.pid_p = 0.0;
                self.pid_i = 0.0;
                self.set_brakeline_diff(NEUTRAL_POS);
            }
        }

        BlimsDataOut {
            brakeline_diff_in: self.brakeline_diff_in,
            pid_p:          self.pid_p,
            pid_i:          self.pid_i,
            bearing:        self.bearing,
            phase_id:       current_phase as i8,
        }
    }

    // =========================================================================
    // Motor
    // =========================================================================
    fn set_brakeline_diff(&mut self, mut position: f32) {
        position = position.clamp(MOTOR_MIN, MOTOR_MAX);
        self.brakeline_diff_in = position;
        // Map inches [MOTOR_MIN, MOTOR_MAX] = [-9, +9] → normalised [0.0, 1.0].
        //
        //   pwm = (position − MOTOR_MIN) / (MOTOR_MAX − MOTOR_MIN)
        //       = (position + 9.0) / 18.0
        //
        // Then map [0, 1] → [5 %, 10 %] duty cycle (1–2 ms standard servo range):
        //
        //   duty = 5%·WRAP + pwm · 5%·WRAP
        //
        // Verify:  position = -9 → pwm = 0.0 → duty = 5%·WRAP  (1 ms, full left)
        //          position =  0 → pwm = 0.5 → duty = 7.5%·WRAP (1.5 ms, neutral)
        //          position = +9 → pwm = 1.0 → duty = 10%·WRAP  (2 ms, full right)

        let pwm_normalized = (position - MOTOR_MIN) / (MOTOR_MAX - MOTOR_MIN);
        let five_pct = WRAP_CYCLE_COUNT as f32 * 0.05;
        let duty = (five_pct + pwm_normalized * five_pct) as u16;

        self.pwm_config.compare_a = duty;
        self.pwm.set_config(&self.pwm_config);
    }

    // =========================================================================
    // PI controller
    // =========================================================================
    fn execute_pi_control(
        &mut self,
        desired_heading: f32,
        current_heading: f32,
        dt: f32,
    ) {
        let error = Self::compute_heading_error(desired_heading, current_heading);

        // Clamp the accumulated error integral so that the I term can contribute
        // at most INTEGRAL_MAX_INCHES of brakeline differential.
        // max_integral [deg·s] = INTEGRAL_MAX_INCHES [in] / KI [in/(deg·s)]
        let max_integral = INTEGRAL_MAX_INCHES / KI;
        self.error_integral =
            (self.error_integral + error * dt).clamp(-max_integral, max_integral);

        let p_term = KP * error;
        let i_term = KI * self.error_integral;

        self.pid_p = p_term;
        self.pid_i = i_term;

        self.set_brakeline_diff(NEUTRAL_POS + p_term + i_term);
    }

    // =========================================================================
    // Phase determination
    // =========================================================================
    /// Decision tree:
    /// 1. GPS invalid → Held
    /// 2. alt ≤ ALT_NEUTRAL_FT → Neutral
    /// 3. alt > ALT_UPWIND_FT  → Upwind
    /// 4. otherwise            → Downwind

    fn determine_phase(&mut self, altitude_ft: f32, gps_valid: bool) -> Phase {
        // GPS must be valid for any active control
        if !gps_valid {
            return Phase::Held;
        }
        let activation_ms = match self.activation_time_ms {
            Some(t) => t,
            None => {
                self.activation_time_ms = Some(self.curr_time_ms);
                self.curr_time_ms
            }
        };
        if self.curr_time_ms - activation_ms < INITIAL_HOLD_THRESHOLD as u64 {
            return Phase::InitialHold;
        }
        // Below min altitude – hands off touchdown
        if altitude_ft <= ALT_NEUTRAL_FT {
            return Phase::Neutral;
        }
        // Landing pattern bands
        if altitude_ft > ALT_UPWIND_FT {
            Phase::Upwind
        } else {
            Phase::Downwind
        }
    }

    // =========================================================================
    // Desired heading per phase
    // =========================================================================

    fn get_desired_heading(
        &self,
        phase: Phase,
        altitude_ft: f32,
    ) -> f32 {
        let altitude_m = altitude_ft / FT_PER_M;
        let wind_from = self.get_wind_at_altitude(altitude_m);
        // Direction the wind blows TO (opposite of "from")

        match phase {
            Phase::Upwind =>
                // fly into wind, "wind_from = 270°" means wind blows from
                // the west, so the canopy should point west (heading = 270°).

                //wind_from,
                self.calculate_bearing_to(self.target_upwind_lat, self.target_upwind_lon),

            Phase::Downwind =>
                // Head WITH the wind.  Opposite of wind_from by 180°.
                // A fixed setpoint (not bearing_to_target) is deliberately used
                // so the commanded heading doesn't shift every cycle as the
                // canopy drifts.  This makes the PI response cleaner to analyse.

                //Self::wrap360(wind_from + 180.0),

                self.calculate_bearing_to(self.target_downwind_lat, self.target_downwind_lon),

            // Held / Neutral do not use heading control
            _ => 0.0,
        }
    }


    // =========================================================================
    // UTILITY FUNCTIONS
    // =========================================================================

    /// Normalise angle to [0, 360)
    #[inline]
    pub fn wrap360(mut a: f32) -> f32 {
        a %= 360.0;
        if a < 0.0 { a += 360.0; }
        a
    }

    /// Normalise angle to (−180, 180]
    #[inline]
    pub fn wrap180(mut a: f32) -> f32 {
        a %= 360.0;
        if      a >  180.0 { a -= 360.0; }
        else if a < -180.0 { a += 360.0; }
        a
    }

    /// Heading error in (−180, 180]: positive → turn right, negative → turn left.
    #[inline]
    pub fn compute_heading_error(desired: f32, actual: f32) -> f32 {
        Self::wrap180(desired - actual)
    }

    /// Bearing from current GPS position to target, degrees [0, 360), 0 = North CW.
    /// Flat-earth approximation; accurate to <0.1° under 2 km at mid-latitudes.
    fn calculate_bearing_to(&self, target_lat: f32, target_lon: f32) -> f32 {
        let d_lat = target_lat - self.gps_lat;
        let d_lon = target_lon - self.gps_lon;
        let lat_rad = self.gps_lat * DEG_TO_RAD;
        let d_lon_corrected = d_lon * libm::cosf(lat_rad);
        let bearing_rad = libm::atan2f(d_lon_corrected, d_lat);
        Self::wrap360(bearing_rad * RAD_TO_DEG)
    }

    /// Distance from current GPS position to target in metres.
    fn calculate_distance_to_target(&self, target_lat: f32, target_lon: f32) -> f32 {
        let d_lat = target_lat - self.gps_lat;
        let d_lon = target_lon - self.gps_lon;
        // 111 320 m per degree of latitude at equator
        let lat_rad  = self.gps_lat * DEG_TO_RAD;
        let d_north  = d_lat * 111_320.0;
        let d_east   = d_lon * 111_320.0 * libm::cosf(lat_rad);
        libm::sqrtf(d_north * d_north + d_east * d_east)
    }

    /// Interpolate wind direction (degrees FROM) at the given altitude (metres).
    /// Falls back to the scalar `wind_from_deg` if no profile has been loaded.
    fn get_wind_at_altitude(&self, altitude_m: f32) -> f32 {
        if self.wind_profile_size == 0 {
            return self.wind_from_deg;
        }
        let alts = &self.wind_altitudes_m[..self.wind_profile_size];
        let dirs = &self.wind_dirs_deg[..self.wind_profile_size];

        // Clamp to profile bounds
        if altitude_m <= alts[0] {
            return dirs[0];
        }
        if altitude_m >= alts[self.wind_profile_size - 1] {
            return dirs[self.wind_profile_size - 1];
        }
        // Linear interpolation between bracketing layers
        for i in 0..self.wind_profile_size - 1 {
            if altitude_m >= alts[i] && altitude_m < alts[i + 1] {
                let t = (altitude_m - alts[i]) / (alts[i + 1] - alts[i]);
                return dirs[i] + t * (dirs[i + 1] - dirs[i]);
            }
        }
        dirs[0] // unreachable, but required for type completeness
    }
}

