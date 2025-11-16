//! Cornell Rocketry Team Flight Software

#![no_std]
#![no_main]

use cortex_m::delay::Delay;
use cortex_m_rt::entry;
use embedded_hal::delay::DelayNs;
use panic_halt as _;

mod constants;
mod drivers;
mod module;
mod state;

pub struct DelayWrapper(Delay);

impl DelayNs for DelayWrapper {
    fn delay_ns(&mut self, ns: u32) {
        let us = ns / 1000;
        if us > 0 {
            self.0.delay_us(us);
        }
    }
}

#[entry]
fn main() -> ! {
    let pac = hal::pac::Peripherals::take().unwrap();
    let core = cortex_m::Peripherals::take().unwrap();

    module::initialize_modules(pac, core);

    loop {
        module::feed_watchdog();

        let current_mode = module::get_current_mode();
        current_mode.execute();
        current_mode.transition();

        unsafe {
            let delay_ptr = core::ptr::addr_of_mut!(module::delay::DELAY);
            if let Some(delay) = (*delay_ptr).as_mut() {
                delay.delay_ms(100);
            }
        }
    }
}

use rp_hal as hal;
