use embassy_rp::gpio::Output;
use embassy_rp::pwm::{Config as PwmConfig, Pwm};
use embassy_time::{Duration, Instant,Timer};
 
use crate::blims_constants::*;
use crate::blims_state::{BlimsDataIn, BlimsDataOut, LoiterStep, Phase};


// ============================================================================
// LoiterTimer
// ============================================================================

struct LoiterTimer {
    step: LoiterStep,
    /// Absolute time when the current step expires
    deadline: Instant,
}

impl LoiterTimer {
    /// Construct and immediately start the first step (TURN_RIGHT)
    fn new() -> Self {
        Self {
            step: LoiterStep::TurnRight,
            deadline: Instant::now()
                + Duration::from_millis(LOITER_TURN_DURATION_MS as u64),
        }
    }

    fn step_duration(step: LoiterStep) -> Duration {
        match step {
            LoiterStep::TurnRight | LoiterStep::TurnLeft => {
                Duration::from_millis(LOITER_TURN_DURATION_MS as u64)
            }
            LoiterStep::PauseRight | LoiterStep::PauseLeft => {
                Duration::from_millis(LOITER_PAUSE_DURATION_MS as u64)
            }
        }
    }

    /// TURN_RIGHT → PAUSE_RIGHT → TURN_LEFT → PAUSE_LEFT → TURN_RIGHT …
    fn next_step(step: LoiterStep) -> LoiterStep {
        match step {
            LoiterStep::TurnRight  => LoiterStep::PauseRight,
            LoiterStep::PauseRight => LoiterStep::TurnLeft,
            LoiterStep::TurnLeft   => LoiterStep::PauseLeft,
            LoiterStep::PauseLeft  => LoiterStep::TurnRight,
        }
    }

    /// Called every execute() tick; advances state when the deadline has passed.
    fn poll(&mut self) {
        if Instant::now() >= self.deadline {
            self.step = Self::next_step(self.step);
            self.deadline = Instant::now() + Self::step_duration(self.step);
        }
    }

    fn motor_position(&self) -> f32 {
        match self.step {
            LoiterStep::TurnRight               => LOITER_RIGHT_POS,
            LoiterStep::TurnLeft                => LOITER_LEFT_POS,
            LoiterStep::PauseRight |
            LoiterStep::PauseLeft               => NEUTRAL_POS,
        }
    }
}


pub struct Blims<'d> {
    // ── Embassy HAL handles ──────────────────────────────────────────────────
    pwm:        Pwm<'d>,
    pwm_config: PwmConfig,
    enable_pin: Output<'d>,

    // ── Pre-flight navigation config ─────────────────────────────────────────
    target_lat: f32,
    target_lon: f32,
    wind_from_deg:     f32,
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
    motor_position: f32,

    // ── Loiter state machine ─────────────────────────────────────────────────
    /// Some(_) only while in Phase::Loiter
    loiter: Option<LoiterTimer>,

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
    pub fn new(
        pwm:        Pwm<'d>,
        pwm_config: PwmConfig,
        enable_pin: Output<'d>,
    ) -> Self {
        let mut b = Self {
            pwm,
            pwm_config,
            enable_pin,
            target_lat: 0.0,
            target_lon: 0.0,
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
            motor_position: NEUTRAL_POS,
            loiter:          None,
            curr_time_ms: 0,
            prev_time_ms: 0,
        };
        b.enable_pin.set_low();
        Timer::after(Duration::from_secs(10)).await;
        b.enable_pin.set_high();
        b.set_motor_position(NEUTRAL_POS);
        b
    }

    // -------------------------------------------------------------------------
    // Pre-flight setters
    // -------------------------------------------------------------------------

    pub fn set_target(&mut self, lat: f32, lon: f32) {
        self.target_lat = lat;
        self.target_lon = lon;
    }

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
       
        let dt = if self.prev_time_ms == 0 || dt_ms > 200 {
            0.05_f32
        } else {
            dt_ms as f32 / 1000.0
        };
    
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
            self.error_integral = 0.0;

            // Leaving Loiter → drop the timer
            if self.last_phase == Phase::Loiter {
                self.loiter = None;
            }
            // Entering Loiter → create a fresh timer (starts TURN_RIGHT immediately)
            if current_phase == Phase::Loiter {
                self.loiter = Some(LoiterTimer::new());
            }

            self.last_phase = current_phase;
        }

        self.bearing = self.calculate_bearing_to_target();

        match current_phase {
            Phase::Held | Phase::Neutral => {
                // No active control – park motor at neutral
                self.pid_p = 0.0;
                self.pid_i = 0.0;
                self.set_motor_position(NEUTRAL_POS);
            }

            Phase::Loiter => {
                // Timed alternating turns – managed by LoiterTimer
                self.execute_loiter();
            }

            Phase::Track | Phase::Downwind | Phase::Base | Phase::Final => {
                let desired = self.get_desired_heading(
                    current_phase, self.bearing, altitude_ft,
                );
                // headMot is degrees × 1e5 → convert to degrees
                let current_heading = self.head_mot as f32 * 1e-5;
                self.execute_pi_control(desired, current_heading, dt);
            }
        }

        // ── Build output struct ───────────────────────────────────────────────
        let loiter_step_id = self
            .loiter
            .as_ref()
            .map(|lt| lt.step as i8)
            .unwrap_or(0);

        BlimsDataOut {
            motor_position: self.motor_position,
            pid_p:          self.pid_p,
            pid_i:          self.pid_i,
            bearing:        self.bearing,
            phase_id:       current_phase as i8,
            loiter_step:    loiter_step_id,
        }
    }

    // =========================================================================
    // Motor
    // =========================================================================
    fn set_motor_position(&mut self, mut position: f32) {
        position = position.clamp(MOTOR_MIN, MOTOR_MAX);
        self.motor_position = position;

        let five_pct = WRAP_CYCLE_COUNT as f32 * 0.05;
        let duty = (five_pct + position * five_pct) as u16;

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

        // Anti-windup: clamp integral to ±INTEGRAL_MAX
        self.error_integral =
            (self.error_integral + error * dt).clamp(-INTEGRAL_MAX, INTEGRAL_MAX);

        let p_term = KP * error;
        let i_term = KI * self.error_integral;

        self.pid_p = p_term;
        self.pid_i = i_term;

        self.set_motor_position(NEUTRAL_POS + p_term + i_term);
    }

    // =========================================================================
    // Loiter
    // =========================================================================

    fn execute_loiter(&mut self) {
        if let Some(ref mut lt) = self.loiter {
            lt.poll(); // advance step if deadline has passed
            let pos = lt.motor_position();
            self.set_motor_position(pos);
        }
    }

    // =========================================================================
    // Phase determination
    // =========================================================================

    fn determine_phase(&self, altitude_ft: f32, gps_valid: bool) -> Phase {
        // GPS must be valid for any active control
        if !gps_valid {
            return Phase::Held;
        }
        // Below min altitude – hands off touchdown
        if altitude_ft < ALT_NEUTRAL_FT {
            return Phase::Neutral;
        }
        // High altitude: either loiter near target or track toward 
        if altitude_ft > ALT_DOWNWIND_FT {
            let dist_ft = self.calculate_distance_to_target() * FT_PER_M;
            return if dist_ft < SET_RADIUS_FT {
                Phase::Loiter
            } else {
                Phase::Track
            };
        }
        // Landing pattern bands
        if altitude_ft > ALT_BASE_FT {
            Phase::Downwind
        } else if altitude_ft > ALT_FINAL_FT {
            Phase::Base
        } else {
            Phase::Final
        }
    }

    // =========================================================================
    // Desired heading per phase
    // =========================================================================

    fn get_desired_heading(
        &self,
        phase: Phase,
        bearing_to_target: f32,
        altitude_ft: f32,
    ) -> f32 {
        let altitude_m = altitude_ft / FT_PER_M;
        let wind_from = self.get_wind_at_altitude(altitude_m);
        // Direction the wind blows TO (opposite of "from")
        let wind_to = Self::wrap360(wind_from + 180.0);

        match phase {
            Phase::Track =>
                // Fly straight toward target
                bearing_to_target,

            Phase::Downwind =>
                // Fly with the wind (downwind leg, away from target)
                wind_to,

            Phase::Base => {
                // Fly perpendicular to wind; choose the leg requiring the shorter turn
                let crosswind_left  = Self::wrap360(wind_from - 90.0);
                let crosswind_right = Self::wrap360(wind_from + 90.0);
                let current_heading  = self.head_mot as f32 * 1e-5;
                let error_left  = Self::wrap180(crosswind_left  - current_heading).abs();
                let error_right = Self::wrap180(crosswind_right - current_heading).abs();
                if error_left < error_right { crosswind_left } else { crosswind_right }
            }

            Phase::Final =>
                // Fly into the wind (minimises ground speed at touchdown)
                wind_from,

            // Held / Neutral / Loiter do not use heading control
            _ => 0.0,
        }
    }


    // =========================================================================
    // UTILITY FUNCTIONS
    // =========================================================================

    /// Normalise angle to [0, 360)
    #[inline]
    fn wrap360(mut a: f32) -> f32 {
        a %= 360.0;
        if a < 0.0 { a += 360.0; }
        a
    }

    /// Normalise angle to (−180, 180]
    #[inline]
    fn wrap180(mut a: f32) -> f32 {
        a %= 360.0;
        if      a >  180.0 { a -= 360.0; }
        else if a < -180.0 { a += 360.0; }
        a
    }

    /// Bearing from current GPS position to target, degrees [0, 360), 0 = North CW.
    /// Flat-earth approximation; accurate to <0.1° under 2 km at mid-latitudes.
    fn calculate_bearing_to_target(&self) -> f32 {
        let d_lat = self.target_lat - self.gps_lat;
        let d_lon = self.target_lon - self.gps_lon;
        // Correct longitude delta for latitude convergence
        let lat_rad = self.gps_lat * DEG_TO_RAD;
        let d_lon_corrected = d_lon * libm::cosf(lat_rad);
        // atan2(east, north) → bearing from north
        let bearing_rad = libm::atan2f(d_lon_corrected, d_lat);
        Self::wrap360(bearing_rad * RAD_TO_DEG)
    }

    /// Distance from current GPS position to target in metres.
    fn calculate_distance_to_target(&self) -> f32 {
        let d_lat = self.target_lat - self.gps_lat;
        let d_lon = self.target_lon - self.gps_lon;
        // 111 320 m per degree of latitude at equator
        let lat_rad  = self.gps_lat * DEG_TO_RAD;
        let d_north  = d_lat * 111_320.0;
        let d_east   = d_lon * 111_320.0 * libm::cosf(lat_rad);
        libm::sqrtf(d_north * d_north + d_east * d_east)
    }

    /// Heading error in (−180, 180]: positive → turn right, negative → turn left
    #[inline]
    fn compute_heading_error(desired: f32, actual: f32) -> f32 {
        Self::wrap180(desired - actual)
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


//controller 6in to -6in 