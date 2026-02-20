use embassy_rp::gpio::Output;
use embassy_time::Instant;

use embedded_hal::pwm::SetDutyCycle;
// 360 ms use duty cycle 330 Hz for period
// 1520/3300 for duty cycle
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
            },
            Chute::Main => {
                self.main_pin.set_high();
                self.main_off_time = Some(end);
            },
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
                        self.next_toggle_time = Some(Instant::now() + embassy_time::Duration::from_millis(100));
                    } else {
                        // Done
                        self.next_toggle_time = None;
                    }
                } else {
                    // Turn on for next beep (if remaining > 0)
                    if self.remaining_beeps > 0 {
                        self.pin.set_high();
                        self.is_on = true;
                        self.next_toggle_time = Some(Instant::now() + embassy_time::Duration::from_millis(100));
                    } else {
                        self.next_toggle_time = None;
                    }
                }
            }
        }
    }
}

// MAV
pub struct Mav<'a> {
    pwm: embassy_rp::pwm::Pwm<'a>,
    open_deadline: Option<Instant>,
}

impl<'a> Mav<'a> {
    pub fn new(pwm: embassy_rp::pwm::Pwm<'a>) -> Self {
        let mut mav = Self { 
            pwm,
            open_deadline: None,
        };
        mav.close();
        mav
    }

    pub fn set_position(&mut self, position: f32) {
        // Clamp position 0.0 to 1.0
        let pos = position.clamp(0.0, 1.0);
        
        // Map 0.0 -> 1520, 1.0 -> 3000 (Based on 1520 Open / 3300 Top) from MAV spec
        let min_duty = 1520.0;
        let max_duty = 3000.0; 
        
        let duty = min_duty + (max_duty - min_duty) * pos;
        let _ = self.pwm.set_duty_cycle_fraction(duty as u16, 3300);
    }

    pub fn open(&mut self, duration_ms: u64) {
        // Open = Position 0.0 (based on 21845/65535 33% 1100/3300)
        let _ = self.pwm.set_duty_cycle_fraction(1520, 3300);
        
        if duration_ms > 0 {
            self.open_deadline = Some(Instant::now() + embassy_time::Duration::from_millis(duration_ms));
        } else {
            self.open_deadline = None;
        }
    }

    pub fn close(&mut self) {
        // Close = Position 1.0
        let _ = self.pwm.set_duty_cycle_fraction(3000, 3300);
        self.open_deadline = None;
    }

    pub fn update(&mut self) {
        if let Some(deadline) = self.open_deadline {
            if Instant::now() >= deadline {
                self.close();
            }
        }
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
