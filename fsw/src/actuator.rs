/// Actuator module for parachute deployment and other actuators

use defmt::*;
use embassy_rp::gpio::Output;

/// Actuator manager for parachute deployment
pub struct ActuatorManager<'a> {
    drogue_pin: Option<Output<'a>>,
    main_pin: Option<Output<'a>>,
    drogue_deployed: bool,
    main_deployed: bool,
}

impl<'a> ActuatorManager<'a> {
    pub fn new() -> Self {
        Self {
            drogue_pin: None,
            main_pin: None,
            drogue_deployed: false,
            main_deployed: false,
        }
    }

    pub fn init(
        &mut self,
        drogue_pin: Output<'a>,
        main_pin: Output<'a>,
    ) {
        info!("Initializing actuators");
        self.drogue_pin = Some(drogue_pin);
        self.main_pin = Some(main_pin);
        info!("Actuators initialized");
    }

    /// Deploy drogue parachute
    pub fn deploy_drogue(&mut self) {
        if !self.drogue_deployed {
            info!("DEPLOYING DROGUE PARACHUTE");
            if let Some(ref mut pin) = self.drogue_pin {
                pin.set_high();
                self.drogue_deployed = true;
            } else {
                warn!("Drogue pin not initialized");
            }
        }
    }

    /// Deploy main parachute
    pub fn deploy_main(&mut self) {
        if !self.main_deployed {
            info!("DEPLOYING MAIN PARACHUTE");
            if let Some(ref mut pin) = self.main_pin {
                pin.set_high();
                self.main_deployed = true;
            } else {
                warn!("Main pin not initialized");
            }
        }
    }

    /// Reset actuators (for testing only)
    pub fn reset(&mut self) {
        info!("Resetting actuators");
        if let Some(ref mut pin) = self.drogue_pin {
            pin.set_low();
        }
        if let Some(ref mut pin) = self.main_pin {
            pin.set_low();
        }
        self.drogue_deployed = false;
        self.main_deployed = false;
    }

    pub fn is_drogue_deployed(&self) -> bool {
        self.drogue_deployed
    }

    pub fn is_main_deployed(&self) -> bool {
        self.main_deployed
    }
}
