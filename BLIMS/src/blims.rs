/// blims.rs
/// BLiMS (Brake Line Manipulation System) – Parafoil Guidance Controller
///
/// CONTROL CONCEPT:
///   motorPosition 0.5  = neutral (no brake)
///   motorPosition < 0.5 = pull left  brake → turn left
///   motorPosition > 0.5 = pull right brake → turn right
///
/// GPS "track" (headMot) is used rather than compass heading because it
/// accounts for wind drift and canopy oscillation naturally.
///
/// FLIGHT PHASES (altitude AGL):
///   Phase::HELD     (0) – GPS invalid or not ready,  motor at neutral
///   Phase::TRACK    (1) – Above 1000 ft & outside 400 ft of target, PI control
///   Phase::DOWNWIND (2) – 1000–600 ft, fly with the wind
///   Phase::BASE     (3) –  600–300 ft, fly perpendicular to wind
///   Phase::FINAL    (4) –  300–100 ft, fly into the wind
///   Phase::NEUTRAL  (5) – Below 100 ft, hands-off for touchdown
///   Phase::LOITER   (6) – Above 1000 ft & within 400 ft of target, spiral down
///
/// Hardware abstraction
/// --------------------
/// The Pico SDK calls (pwm_set_chan_level, gpio_put, to_ms_since_boot, …) are
/// expressed here through the `Hardware` trait so that the guidance logic can
/// be tested on a host without embedded peripherals.
use core::f32::consts::PI;
use crate::blims_constants::*;
use crate::blims_state::*;
use crate::blims_state::BLIMSMode::*;


// ── Hardware abstraction trait ────────────────────────────────────────────

/// All side-effecting hardware operations the guidance controller requires.
/// Implement this trait for the RP2040 (or a mock for unit tests).
pub trait Hardware {
    /// Current monotonic timestamp in milliseconds.
    fn now_ms(&self) -> u32;

    /// Write a PWM duty-cycle level to the motor channel.
    fn set_pwm_level(&mut self, pin: u8, level: u16);

    /// Drive the enable GPIO high or low.
    fn set_enable_pin(&mut self, pin: u8, high: bool);

    /// Schedule a one-shot alarm `delayMs` milliseconds from now.
    /// The implementation must set `loiterAdvancePending = true` when it fires.
    /// Returns an opaque alarm handle (or -1 on failure).
    fn schedule_alarm_ms(&mut self, delay_ms: u32) -> i64;

    /// Cancel a previously scheduled alarm by handle.
    fn cancel_alarm(&mut self, alarm_id: i64);
}

// ── Phase and LoiterStep enums ────────────────────────────────────────────

/// Flight phases for the landing-pattern state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i8)]
enum Phase {
    HELD     = 0,  // GPS invalid or not ready - hold neutral
    TRACK    = 1,  // PI control towards target bearing 
    DOWNWIND = 2,  // Fly with the wind (heading = wind direction)
    BASE     = 3,  // Fly perpendicular to wind (crosswind leg)
    FINAL    = 4,  // Fly into the wind (heading = opposite of wind)
    NEUTRAL  = 5,  // Below minimum altitude - hands off 
    LOITER   = 6,  //Spiral turns to bleed altitude while staying near target
}

/// Sub-states within the LOITER phase.
///
/// When loitering, we alternate between turning right and left with neutral 
/// periods in betweem. This creates a figure-8 or spiral pattern that bleeds 
/// altitude without drifting far from the target
///   TURN_RIGHT (6 s) → PAUSE_RIGHT (2.5 s) → TURN_LEFT (6 s) → PAUSE_LEFT (2.5 s) → …
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i8)]
enum LoiterStep {
    TURN_RIGHT  = 0, //Pull right brake 
    PAUSE_RIGHT = 1, //Neutral after right turn 
    TURN_LEFT   = 2, //Pull left brake 
    PAUSE_LEFT  = 3, //Neutral after left turn 
}

// ── Main driver struct ────────────────────────────────────────────────────

/// BLiMS guidance controller.
pub struct BLIMS {

    state: BLIMSState,
    loiter_step: LoiterStep,
    loiter_alarm_id: i64,
    loiter_advance_pending: bool,
    error_integral: f32,
    last_phase: Phase,
}

impl BLIMS {
    /// Create a new, uninitialised BLIMS instance.
    pub fn new() -> Self {
        Self {
            state:                 BLIMSState::default(),
            loiter_step:           LoiterStep::TURN_RIGHT,
            loiter_alarm_id:        -1,
            loiter_advance_pending: false,
            error_integral:        0.0,
            last_phase:            Phase::HELD,
        }
    }

    // ── Public API ────────────────────────────────────────────────────────

    /// Initialise the BLiMS system.  Call once before `execute`.
    ///
    /// Configures PWM at 50 Hz (standard servo frequency), enables the
    /// motor driver, and drives the motor to the neutral position.
    pub fn begin<H: Hardware>(
        &mut self,
        hw: &mut H,
        pwm_pin: u8,
        enable_pin: u8,
    ) {
        self.state.flight.flight_mode      = BLIMSMode::STANDBY;
        self.state.flight.blims_pwm_pin    = pwm_pin;
        self.state.flight.blims_enable_pin = enable_pin;

        // Enable motor driver
        hw.set_enable_pin(enable_pin, true);

        // Drive to neutral
        self.set_motor_position(hw, NEUTRAL_POS);

        // Initialise state
        self.error_integral        = 0.0;
        self.last_phase            = Phase::HELD;
        self.loiter_step           = LoiterStep::TURN_RIGHT;
        self.loiter_alarm_id        = -1;
        self.loiter_advance_pending = false;

        self.state.flight.blims_init = true;
    }

    /// Set the target landing coordinates.
    pub fn set_target(&mut self, lat: f32, lon: f32) {
        self.state.lv.target_lat = lat;
        self.state.lv.target_lon = lon;
    }

    /// Set the wind direction (direction wind is coming **FROM**), degrees [0, 360).
    pub fn set_wind_from_deg(&mut self, deg: f32) {
        self.state.lv.wind_from_deg = wrap360(deg);
    }

    /// Upload a layered wind profile (loaded via umbilical before launch).
    /// Arrays must have the same length; excess elements beyond `MAX_WIND_LAYERS` are dropped.
    pub fn set_wind_profile(
        &mut self,
        altitudes_m: &[f32],
        directions_deg: &[f32],
    ) {
        let size = altitudes_m.len().min(directions_deg.len()).min(MAX_WIND_LAYERS);
        self.state.lv.wind_profile_size = size;
        for i in 0..size {
            self.state.lv.wind_altitudes_m[i] = altitudes_m[i];
            self.state.lv.wind_dirs_deg[i]    = directions_deg[i];
        }
    }

    /// Notify BLiMS that the loiter alarm has fired (call from your alarm ISR or task).
    ///
    /// On embedded targets the alarm callback cannot safely modify complex state,
    /// so it should call this method (or set the flag directly) and let the main
    /// loop pick it up on the next `execute` call.
    pub fn notify_loiter_alarm(&mut self) {
        self.loiter_advance_pending = true;
    }

    /// Main execution function – call every control-loop iteration (~20 Hz / 50 ms).
    ///
    /// Processes GPS data, selects the current flight phase, applies the
    /// appropriate motor command, and returns telemetry.
    pub fn execute<H: Hardware>(&mut self, hw: &mut H, dataIn: &BLIMSDataIn) -> BLIMSDataOut {
        // ── Timing ───────────────────────────────────────────────────────
        self.state.flight.prev_time = self.state.flight.curr_time;
        self.state.flight.curr_time = hw.now_ms();
        let dt = (self.state.flight.curr_time.wrapping_sub(self.state.flight.prev_time)) as f32
            / 1000.0;

        // ── Ingest GPS data ───────────────────────────────────────────────
        self.state.flight.gps_lat   = dataIn.lat as f32 * 1e-7;
        self.state.flight.gps_lon   = dataIn.lon as f32 * 1e-7;
        self.state.flight.g_speed   = dataIn.g_speed;
        self.state.flight.head_mot  = dataIn.head_mot;
        self.state.flight.fix_type  = dataIn.fix_type;
        self.state.lv.gps_state     = dataIn.gps_state;

        let altitude_ft = dataIn.altitude_ft;

        // ── Phase determination ───────────────────────────────────────────
        let gps_valid = self.state.lv.gps_state && (self.state.flight.fix_type >= 2);
        let current_phase = self.determine_phase(altitude_ft, gps_valid);

        // ── Phase-change handling ─────────────────────────────────────────
        if current_phase != self.last_phase {
            // Reset integral to prevent windup carryover
            self.error_integral = 0.0;

            // Cancel loiter alarm if leaving loiter
            if self.last_phase == Phase::LOITER {
                self.cancel_loiter_alarm(hw);
            }

            // Initialise loiter when entering it
            if current_phase == Phase::LOITER {
                self.reset_loiter_state(hw);
            }

            self.last_phase = current_phase;
        }

        // ── Bearing to target (used by TRACK and for telemetry) ───────────
        let bearing_to_target = self.calculate_bearing_to_target();
        self.state.lv.bearing = bearing_to_target;

        // ── Phase-specific control ────────────────────────────────────────
        match current_phase {
            Phase::HELD | Phase::NEUTRAL => {
                self.set_motor_position(hw, NEUTRAL_POS);
                self.state.lv.pid_p = 0.0;
                self.state.lv.pid_i = 0.0;
            }

            Phase::LOITER => {
                self.execute_loiter(hw);
            }

            Phase::TRACK | Phase::DOWNWIND | Phase::BASE | Phase::FINAL => {
                let desired_heading = self.get_desired_heading(current_phase, bearing_to_target, altitude_ft);
                let current_heading = self.state.flight.head_mot as f32 * 1e-5;
                self.execute_pi_control(hw, desired_heading, current_heading, dt);
            }
        }

        // ── Build telemetry output ────────────────────────────────────────
        BLIMSDataOut {
            motor_position: self.state.flight.motor_position,
            pid_p:          self.state.lv.pid_p,
            pid_i:          self.state.lv.pid_i,
            bearing:        self.state.lv.bearing,
            phase_id:       self.state.flight.data_out.phase_id as i8,
            loiter_step:    self.loiter_step as i8,
        }
    }

    // ── Private: motor control ────────────────────────────────────────────

    /// Clamp `position` to [MOTOR_MIN, MOTOR_MAX] and drive the PWM output.
    fn set_motor_position<H: Hardware>(&mut self, hw: &mut H, mut position: f32) {
        position = position.clamp(MOTOR_MIN, MOTOR_MAX);
        self.state.flight.motor_position = position;

        // PWM at 50 Hz: 5 % duty = minimum (servo low pulse), 10 % = maximum.
        // The ODrive receives a standard hobby-servo PWM signal.
        let five_percent_duty = WRAP_CYCLE_COUNT as f32 * 0.05;
        let duty = (five_percent_duty + position * five_percent_duty) as u16;

        hw.set_pwm_level(self.state.flight.blims_pwm_pin, duty);
    }

    // ── Private: geometry helpers ─────────────────────────────────────────

    /// Bearing from current GPS position to target, degrees [0, 360).
    ///
    /// Uses a flat-earth approximation accurate to < 0.1° for distances < 2 km
    /// at mid-latitudes (well within GPS accuracy limits).
    fn calculate_bearing_to_target(&self) -> f32 {
        let d_lat = self.state.lv.target_lat - self.state.flight.gps_lat;
        let d_lon = self.state.lv.target_lon - self.state.flight.gps_lon;

        let lat_rad     = self.state.flight.gps_lat * (PI / 180.0);
        let d_lon_corrected = d_lon * libm::cosf(lat_rad);

        let bearing_rad = libm::atan2f(d_lon_corrected, d_lat);
        wrap360(bearing_rad * (180.0 / PI))
    }

    /// Distance from current GPS position to target, metres.
    fn calculate_distance_to_target(&self) -> f32 {
        let d_lat = self.state.lv.target_lat - self.state.flight.gps_lat;
        let d_lon = self.state.lv.target_lon - self.state.flight.gps_lon;

        let lat_rad    = self.state.flight.gps_lat * (PI / 180.0);
        let d_north_m   = d_lat * 111_320.0;
        let d_east_m    = d_lon * 111_320.0 * libm::cosf(lat_rad);

        libm::sqrtf(d_north_m * d_north_m + d_east_m * d_east_m)
    }

    /// Wind direction at a given altitude, interpolated from the wind profile.
    fn get_wind_at_altitude(&self, altitude_m: f32) -> f32 {
        let profile = &self.state.lv;

        if profile.wind_profile_size == 0 {
            return profile.wind_from_deg; // fallback to single value
        }

        let alts = &profile.wind_altitudes_m[..profile.wind_profile_size];
        let dirs = &profile.wind_dirs_deg[..profile.wind_profile_size];

        if altitude_m <= alts[0] {
            return dirs[0];
        }
        if altitude_m >= alts[profile.wind_profile_size - 1] {
            return dirs[profile.wind_profile_size - 1];
        }

        // Linear interpolation between bracketing layers
        for i in 0..profile.wind_profile_size - 1 {
            if altitude_m >= alts[i] && altitude_m < alts[i + 1] {
                let t = (altitude_m - alts[i]) / (alts[i + 1] - alts[i]);
                return dirs[i] + t * (dirs[i + 1] - dirs[i]);
            }
        }

        dirs[0]
    }

    // ── Private: phase logic ──────────────────────────────────────────────

    /// Select the current flight phase from altitude and GPS validity.
    fn determine_phase(&self, altitude_ft: f32, gps_valid: bool) -> Phase {
        if !gps_valid {
            return Phase::HELD;
        }
        if altitude_ft < ALT_NEUTRAL_FT {
            return Phase::NEUTRAL;
        }
        if altitude_ft > ALT_DOWNWIND_FT {
            let distance_ft = self.calculate_distance_to_target() * FT_PER_M;
            return if distance_ft < SET_RADIUS_FT {
                Phase::LOITER
            } else {
                Phase::TRACK
            };
        }
        if altitude_ft > ALT_BASE_FT {
            Phase::DOWNWIND
        } else if altitude_ft > ALT_FINAL_FT {
            Phase::BASE
        } else {
            Phase::FINAL
        }
    }

    /// Desired heading for PI control phases.
    fn get_desired_heading(&self, phase: Phase, bearing_to_target: f32, altitude_ft: f32) -> f32 {
        let altitude_m = altitude_ft / FT_PER_M;
        let wind_from  = self.get_wind_at_altitude(altitude_m);
        let wind_to    = wrap360(wind_from + 180.0); // direction wind is blowing TO

        match phase {
            Phase::TRACK => bearing_to_target,

            Phase::DOWNWIND => wind_to,

            Phase::BASE => {
                let crosswind_left  = wrap360(wind_from - 90.0);
                let crosswind_right = wrap360(wind_from + 90.0);

                let current_heading = self.state.flight.head_mot as f32 * 1e-5;
                let error_left  = wrap180(crosswind_left  - current_heading).abs();
                let error_right = wrap180(crosswind_right - current_heading).abs();

                if error_left < error_right { crosswind_left } else { crosswind_right }
            }

            Phase::FINAL => wind_from,

            // HELD, NEUTRAL, LOITER do not use heading control
            _ => 0.0,
        }
    }

    // ── Private: PI controller ────────────────────────────────────────────

    /// Execute one PI step and drive the motor.
    ///
    /// ```text
    /// error    = wrap180(desiredHeading – currentHeading)
    /// motor    = neutral – Kp·error – Ki·∫error dt
    /// ```
    ///
    /// Positive error means "need to turn right" → increase motor position.
    /// The sign in the formula is negative because the motor mapping is
    /// inverted relative to the error sign convention.
    fn execute_pi_control<H: Hardware>(
        &mut self,
        hw: &mut H,
        desired_heading: f32,
        current_heading: f32,
        dt: f32,
    ) {
        let error = wrap180(desired_heading - current_heading);

        // Integrate with anti-windup clamp
        self.error_integral = (self.error_integral + error * dt)
            .clamp(-INTEGRAL_MAX, INTEGRAL_MAX);

        let p_term = -KP * error;
        let i_term = -KI * self.error_integral;

        self.state.lv.pid_p = p_term;
        self.state.lv.pid_i = i_term;

        let motor_position = NEUTRAL_POS + p_term + i_term;
        self.set_motor_position(hw, motor_position);
    }

    // ── Private: loiter state machine ─────────────────────────────────────

    /// Duration (ms) for a given loiter sub-step.
    fn loiter_step_duration(step: LoiterStep) -> u32 {
        match step {
            LoiterStep::TURN_RIGHT | LoiterStep::TURN_LEFT   => LOITER_TURN_DURATION_MS,
            LoiterStep::PAUSE_RIGHT | LoiterStep::PAUSE_LEFT => LOITER_PAUSE_DURATION_MS,
        }
    }

    /// Advance through the loiter step sequence.
    fn next_loiter_step(current: LoiterStep) -> LoiterStep {
        match current {
            LoiterStep::TURN_RIGHT  => LoiterStep::PAUSE_RIGHT,
            LoiterStep::PAUSE_RIGHT => LoiterStep::TURN_LEFT,
            LoiterStep::TURN_LEFT   => LoiterStep::PAUSE_LEFT,
            LoiterStep::PAUSE_LEFT  => LoiterStep::TURN_RIGHT,
        }
    }

    /// Cancel any pending loiter alarm.
    fn cancel_loiter_alarm<H: Hardware>(&mut self, hw: &mut H) {
        if self.loiter_alarm_id >= 0 {
            hw.cancel_alarm(self.loiter_alarm_id);
            self.loiter_alarm_id = -1;
        }
        self.loiter_advance_pending = false;
    }

    /// Schedule the next loiter transition alarm.
    fn schedule_loiter_alarm<H: Hardware>(&mut self, hw: &mut H, delay_ms: u32) {
        self.loiter_alarm_id = hw.schedule_alarm_ms(delay_ms);
    }

    /// Set the motor position for the active loiter step.
    fn apply_loiter_motor_position<H: Hardware>(&mut self, hw: &mut H) {
        let pos = match self.loiter_step {
            LoiterStep::TURN_RIGHT              => LOITER_RIGHT_POS,
            LoiterStep::TURN_LEFT               => LOITER_LEFT_POS,
            LoiterStep::PAUSE_RIGHT |
            LoiterStep::PAUSE_LEFT              => NEUTRAL_POS,
        };
        self.set_motor_position(hw, pos);
    }

    /// Reset loiter to the first step and arm the first alarm.
    fn reset_loiter_state<H: Hardware>(&mut self, hw: &mut H) {
        self.cancel_loiter_alarm(hw);
        self.loiter_step           = LoiterStep::TURN_RIGHT;
        self.loiter_advance_pending = false;

        let duration = Self::loiter_step_duration(self.loiter_step);
        self.schedule_loiter_alarm(hw, duration);
        self.apply_loiter_motor_position(hw);
    }

    /// Execute one loiter iteration.
    ///
    /// Non-blocking: transitions are driven by the alarm flag set in
    /// `notifyLoiterAlarm` (or the hardware ISR).
    fn execute_loiter<H: Hardware>(&mut self, hw: &mut H) {
        if self.loiter_advance_pending {
            self.loiter_advance_pending = false;

            self.loiter_step = Self::next_loiter_step(self.loiter_step);

            let duration = Self::loiter_step_duration(self.loiter_step);
            self.schedule_loiter_alarm(hw, duration);
        }

        // Reapply motor position every iteration (idempotent, no cost)
        self.apply_loiter_motor_position(hw);
    }
}


// ── Angle utilities ───────────────────────────────────────────────────────

/// Wrap angle to [0, 360).
#[inline]
fn wrap360(mut angle: f32) -> f32 {
    angle = libm::fmodf(angle, 360.0);
    if angle < 0.0 {
        angle += 360.0;
    }
    angle
}

/// Wrap angle to [-180, 180).
///
/// Positive result → turn right; negative → turn left.
#[inline]
fn wrap180(mut angle: f32) -> f32 {
    angle = libm::fmodf(angle, 360.0);
    if angle > 180.0 {
        angle -= 360.0;
    } else if angle < -180.0 {
        angle += 360.0;
    }
    angle
}



    