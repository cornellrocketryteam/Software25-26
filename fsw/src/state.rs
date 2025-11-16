use crate::module;
use embedded_hal::digital::StatefulOutputPin;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlightMode {
    Startup = 0,
    Standby = 1,
    Ascent = 2,
    DrogueDeployed = 3,
    MainDeployed = 4,
    Fault = 5,
}

impl FlightMode {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn name(self) -> &'static str {
        match self {
            FlightMode::Startup => "Startup",
            FlightMode::Standby => "Standby",
            FlightMode::Ascent => "Ascent",
            FlightMode::DrogueDeployed => "DrogueDeployed",
            FlightMode::MainDeployed => "MainDeployed",
            FlightMode::Fault => "Fault",
        }
    }

    pub fn execute(&self) {
        module::read_sensors();

        match self {
            FlightMode::Startup => Self::execute_startup(),
            FlightMode::Standby => Self::execute_standby(),
            FlightMode::Ascent => Self::execute_ascent(),
            FlightMode::DrogueDeployed => Self::execute_drogue_deployed(),
            FlightMode::MainDeployed => Self::execute_main_deployed(),
            FlightMode::Fault => Self::execute_fault(),
        }
    }

    pub fn transition(&self) {
        match self {
            FlightMode::Startup => Self::transition_startup(),
            FlightMode::Standby => Self::transition_standby(),
            FlightMode::Ascent => Self::transition_ascent(),
            FlightMode::DrogueDeployed => Self::transition_drogue_deployed(),
            FlightMode::MainDeployed => Self::transition_main_deployed(),
            FlightMode::Fault => Self::transition_fault(),
        }
    }
    fn execute_startup() {
        toggle_led();
    }

    fn transition_startup() {
        unsafe {
            if module::alt::STATUS == module::SensorState::Valid {
                module::to_mode(FlightMode::Standby);
            }
        }
    }

    fn execute_standby() {
        toggle_led();
    }

    fn transition_standby() {
        // TODO: Check for liftoff
    }

    fn execute_ascent() {
        toggle_led();
    }

    fn transition_ascent() {
        // TODO: Check for apogee
    }

    fn execute_drogue_deployed() {
        toggle_led();
    }

    fn transition_drogue_deployed() {
        // TODO: Check altitude for main deployment
    }

    fn execute_main_deployed() {
        toggle_led();
    }

    fn transition_main_deployed() {
        // TODO: Check for landing
    }

    fn execute_fault() {
        toggle_led();
    }

    fn transition_fault() {}
}

fn toggle_led() {
    unsafe {
        let led_ptr = core::ptr::addr_of_mut!(module::gpio_pins::LED);
        if let Some(led) = (*led_ptr).as_mut() {
            let _ = led.toggle();
        }
    }
}
