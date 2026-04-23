use embassy_rp::gpio::Output;
use embassy_rp::pwm::Pwm;
use embassy_time::{Duration, Instant};
use embedded_hal::pwm::SetDutyCycle;

// 330 Hz servo frequency, 3030 µs period
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Chute {
    Drogue,
    Main,
}

// SSA
pub struct Ssa<'a> {
    drogue_pin: Output<'a>,
    main_pin: Output<'a>,
    drogue_off_time: Option<Instant>,
    main_off_time: Option<Instant>,
}

impl<'a> Ssa<'a> {
    // Create a new SSA controller with pins for drogue and main chutes
    pub fn new(drogue_pin: Output<'a>, main_pin: Output<'a>) -> Self {
        Self {
            drogue_pin,
            main_pin,
            drogue_off_time: None,
            main_off_time: None,
        }
    }

    // Trigger a specific chute for a duration (ms)
    pub fn trigger(&mut self, chute: Chute, duration_ms: u64) {
        let end = Instant::now() + embassy_time::Duration::from_millis(duration_ms);
        match chute {
            Chute::Drogue => {
                self.drogue_pin.set_high();
                self.drogue_off_time = Some(end);
            }
            Chute::Main => {
                self.main_pin.set_high();
                self.main_off_time = Some(end);
            }
        }
    }

    // Update loop to turn off pins when duration expires
    pub fn update(&mut self) {
        let now = Instant::now();

        if let Some(time) = self.drogue_off_time {
            if now >= time {
                self.drogue_pin.set_low();
                self.drogue_off_time = None;
            }
        }

        if let Some(time) = self.main_off_time {
            if now >= time {
                self.main_pin.set_low();
                self.main_off_time = None;
            }
        }
    }
}

// ============================================================================
// Buzzer
// ============================================================================
//
// CFC_ARM_Indicator (GPIO 21): PWM output at 400 Hz driving the buzzer + LED.
// CFC_ARM (GPIO 41):           Input (Pull::Down) — off-board arming signal.

pub struct Buzzer<'a> {
    /// GPIO 21 – CFC_ARM_Indicator, PWM at 400 Hz (50% duty = tone on, 0% = off)
    pwm: Pwm<'a>,
    remaining_beeps: u32,
    next_toggle_time: Option<Instant>,
    is_on: bool,
}

impl<'a> Buzzer<'a> {
    // 4 kHz: clk=150 MHz, divider=6, top=6249
    // freq = 150_000_000 / (6 * (6249 + 1)) = 4000 Hz
    const TOP: u16 = 6249;

    /// `pwm` = PWM on GPIO 21 (CFC_ARM_Indicator), configured for 400 Hz in module.rs.
    pub fn new(pwm: Pwm<'a>) -> Self {
        Self {
            pwm,
            remaining_beeps: 0,
            next_toggle_time: None,
            is_on: false,
        }
    }

    fn set_on(&mut self) {
        // 50% duty cycle → square wave at 400 Hz
        let _ = self.pwm.set_duty_cycle_fraction(1, 2);
    }

    fn set_off(&mut self) {
        let _ = self.pwm.set_duty_cycle_fraction(0, Self::TOP);
    }

    /// Request `num` beeps (100 ms on / 100 ms off each).
    /// Ignored if a beep sequence is already in progress.
    pub fn buzz(&mut self, num: u32) {
        if self.remaining_beeps == 0 {
            self.remaining_beeps = num;
            self.set_on();
            self.is_on = true;
            self.next_toggle_time =
                Some(Instant::now() + embassy_time::Duration::from_millis(100));
        }
    }

    /// Call every loop cycle. Manages on/off beep pattern via PWM duty cycle.
    pub fn update(&mut self) {
        if let Some(time) = self.next_toggle_time {
            if Instant::now() >= time {
                if self.is_on {
                    // End of ON phase
                    self.set_off();
                    self.is_on = false;
                    self.remaining_beeps -= 1;

                    if self.remaining_beeps > 0 {
                        // Gap before next beep
                        self.next_toggle_time =
                            Some(Instant::now() + embassy_time::Duration::from_millis(100));
                    } else {
                        self.next_toggle_time = None;
                    }
                } else {
                    // End of gap phase — start next beep
                    if self.remaining_beeps > 0 {
                        self.set_on();
                        self.is_on = true;
                        self.next_toggle_time =
                            Some(Instant::now() + embassy_time::Duration::from_millis(100));
                    } else {
                        self.next_toggle_time = None;
                    }
                }
            }
        }
    }
}


// MAV
/// Servo driver for ProModeler DS2685BLHV
///
/// PWM requirements:
/// - 330 Hz frame rate
/// - 3030 µs period
/// - 800–2200 µs pulse width
pub struct Mav<'a> {
    pwm: Pwm<'a>,
    open_deadline: Option<Instant>,
    state_open: bool,
}

impl<'a> Mav<'a> {
    // ======== Servo Constants ========

    const SERVO_FREQ_HZ: u32 = 330;
    const SERVO_PERIOD_US: u16 = (1_000_000 / Self::SERVO_FREQ_HZ) as u16; // 3030 µs
    const SERVO_MIN_US: u16 = 800;
    const SERVO_MAX_US: u16 = 2200;
    const SERVO_OPEN_US: u16 = 1950;
    const SERVO_CLOSE_US: u16 = 883;

    /// Create new MAV servo driver.
    /// Assumes PWM slice already configured for:
    /// - top = 3030
    /// - divider = 150 (for 150 MHz system clock)
    pub fn new(pwm: Pwm<'a>) -> Self {
        let mut mav = Self {
            pwm,
            open_deadline: None,
            state_open: false,
        };

        // Start at closed position
        mav.set_pulse_width(Self::SERVO_CLOSE_US);
        mav
    }

    /// Core low-level function:
    /// Sets pulse width in microseconds directly.
    /// Clamped to SERVO_MIN_US..SERVO_MAX_US (800–2200 µs).
    fn set_pulse_width(&mut self, pulse_us: u16) {
        let pulse = pulse_us.clamp(Self::SERVO_MIN_US, Self::SERVO_MAX_US);
        let _ = self
            .pwm
            .set_duty_cycle_fraction(pulse, Self::SERVO_PERIOD_US);
    }

    /// Set servo position as normalized value:
    /// 0.0 = min pulse (800 µs)
    /// 1.0 = max pulse (2200 µs)
    pub fn set_position(&mut self, position: f32) {
        let pos = position.clamp(0.0, 1.0);

        let span = (Self::SERVO_MAX_US - Self::SERVO_MIN_US) as f32;
        let pulse = Self::SERVO_MIN_US as f32 + (span * pos);

        // Add 0.5 for rounding before truncating to integer in no_std
        self.set_pulse_width((pulse + 0.5) as u16);
    }

    /// Open valve to SERVO_OPEN_US (2015 µs)
    pub fn open(&mut self, duration_ms: u64) {
        self.set_pulse_width(Self::SERVO_OPEN_US);
        self.state_open = true;

        if duration_ms > 0 {
            self.open_deadline = Some(Instant::now() + Duration::from_millis(duration_ms));
        } else {
            self.open_deadline = None;
        }
    }

    /// Close valve to SERVO_CLOSE_US (995 µs)
    pub fn close(&mut self) {
        self.set_pulse_width(Self::SERVO_CLOSE_US);
        self.state_open = false;
        self.open_deadline = None;
    }

    /// Must be called periodically to handle timed open()
    pub fn update(&mut self) {
        if let Some(deadline) = self.open_deadline {
            if Instant::now() >= deadline {
                self.close();
            }
        }
    }

    pub fn is_open(&self) -> bool {
        self.state_open
    }
}

// ============================================================================
// AirbrakeActuator
// ============================================================================
//
// Drives the ODrive S1 motor controller via standard RC PWM on GPIO 38.
// ODrive S1 RC PWM input (G08, isolated IO):
//   - 50 Hz frame rate (20 ms period)
//   - 1000 µs pulse → fully retracted (0% deployment)
//   - 2000 µs pulse → fully deployed  (100% deployment)
//
// GPIO 37 = ENABLE output (High = enable ODrive, Low = disable)
// GPIO 38 = PWM signal to ODrive RC PWM IN

pub struct AirbrakeActuator<'a> {
    enable: Output<'a>,
    pwm: Pwm<'a>,
    current_deployment: f32,
}

impl<'a> AirbrakeActuator<'a> {
    // RC PWM standard: 50 Hz, 1000–2000 µs
    const SERVO_PERIOD_US: u16 = 20_000; // 20 ms period
    const SERVO_CLOSE_US: u16  = 1_000;  // fully retracted
    const SERVO_OPEN_US: u16   = 2_000;  // fully deployed

    /// Create a new AirbrakeActuator.
    /// PWM slice must be configured for 50 Hz in module.rs:
    ///   top = 59999, divider = 50 (for 150 MHz system clock)
    pub fn new(enable: Output<'a>, pwm: Pwm<'a>) -> Self {
        let mut ab = Self {
            enable,
            pwm,
            current_deployment: 0.0,
        };
        ab.retract(); // safe starting position
        ab
    }

    /// Enable the ODrive (assert ENABLE pin high).
    pub fn enable(&mut self) {
        self.enable.set_high();
    }

    /// Disable the ODrive (assert ENABLE pin low).
    pub fn disable(&mut self) {
        self.enable.set_low();
    }

    /// Set airbrake deployment level.
    /// `deployment`: 0.0 = fully retracted, 1.0 = fully deployed.
    /// Outputs a proportional RC PWM pulse between 1000 µs and 2000 µs.
    pub fn set_deployment(&mut self, deployment: f32) {
        let dep = deployment.clamp(0.0, 1.0);
        self.current_deployment = dep;

        let span = (Self::SERVO_OPEN_US - Self::SERVO_CLOSE_US) as f32;
        let pulse_us = Self::SERVO_CLOSE_US as f32 + span * dep;

        self.set_pulse_width((pulse_us + 0.5) as u16);
    }

    /// Fully retract the airbrakes (1000 µs pulse).
    pub fn retract(&mut self) {
        self.set_deployment(0.0);
    }

    /// Returns the last commanded deployment level (0.0–1.0).
    pub fn current_deployment(&self) -> f32 {
        self.current_deployment
    }

    fn set_pulse_width(&mut self, pulse_us: u16) {
        let pulse = pulse_us.clamp(Self::SERVO_CLOSE_US, Self::SERVO_OPEN_US);
        let _ = self
            .pwm
            .set_duty_cycle_fraction(pulse, Self::SERVO_PERIOD_US);
    }
}

// SV
pub struct SV<'a> {
    pin: Output<'a>,
    open_delay: Option<Instant>,
    state_open: bool,
}

impl<'a> SV<'a> {
    pub fn new(pin: Output<'a>) -> Self {
        let mut sv = Self {
            pin,
            open_delay: None,
            state_open: false,
        };
        sv.open(0); // Ensure open initially
        sv
    }

    // Open with optional delay (ms)
    // Active Low
    pub fn open(&mut self, delay_ms: u64) {
        if delay_ms > 0 {
            self.open_delay = Some(Instant::now() + embassy_time::Duration::from_millis(delay_ms));
        } else {
            self.pin.set_low();
            self.state_open = true;
            self.open_delay = None;
        }
    }

    // Close immediately
    // Active High
    pub fn close(&mut self) {
        self.pin.set_high();
        self.state_open = false;
        self.open_delay = None;
    }

    pub fn update(&mut self) {
        if let Some(time) = self.open_delay {
            if Instant::now() >= time {
                self.pin.set_low();
                self.state_open = true;
                self.open_delay = None;
            }
        }
    }

    pub fn is_open(&self) -> bool {
        self.state_open
    }
}
