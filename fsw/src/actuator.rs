use embassy_rp::gpio::Output;
use embassy_rp::pwm::Pwm;
use embassy_time::{Duration, Instant};
use embedded_hal::pwm::SetDutyCycle;
// 330 Hz servo frequency, 3030 µs period
// open = 2015 µs, close = 995 µs, neutral = 1520 µs
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

// Buzzer
pub struct Buzzer<'a> {
    pin: Output<'a>,
    remaining_beeps: u32,
    next_toggle_time: Option<Instant>,
    is_on: bool,
}

impl<'a> Buzzer<'a> {
    pub fn new(pin: Output<'a>) -> Self {
        Self {
            pin,
            remaining_beeps: 0,
            next_toggle_time: None,
            is_on: false,
        }
    }

    // Each beep is 100ms on and 100ms off
    pub fn buzz(&mut self, num: u32) {
        if self.remaining_beeps == 0 {
            self.remaining_beeps = num;
            self.pin.set_high();
            self.is_on = true;
            // 100ms on
            self.next_toggle_time = Some(Instant::now() + embassy_time::Duration::from_millis(100));
        }
    }

    // Call this every loop cycle
    pub fn update(&mut self) {
        if let Some(time) = self.next_toggle_time {
            if Instant::now() >= time {
                if self.is_on {
                    // Turn off
                    self.pin.set_low();
                    self.is_on = false;
                    self.remaining_beeps -= 1;

                    if self.remaining_beeps > 0 {
                        // Wait 100ms before next beep
                        self.next_toggle_time =
                            Some(Instant::now() + embassy_time::Duration::from_millis(100));
                    } else {
                        // Done
                        self.next_toggle_time = None;
                    }
                } else {
                    // Turn on for next beep (if remaining > 0)
                    if self.remaining_beeps > 0 {
                        self.pin.set_high();
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
    const SERVO_NEUTRAL_US: u16 = 1300;

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

        // Start at neutral position
        mav.set_pulse_width(Self::SERVO_NEUTRAL_US);
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
        sv.close(); // Ensure closed initially
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
