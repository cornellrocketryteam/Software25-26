//! Cornell Rocketry Team Flight Software

#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_time::Timer;
use {defmt_rtt as _, panic_probe as _};

mod driver;
mod module;

mod state;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // Initialize USB driver for logger
    let driver = module::init_usb_driver(p.USB);

    // Spawn USB logger task
    spawner.spawn(logger_task(driver).unwrap());

    let i2c_bus = module::init_shared_i2c(p.I2C0, p.PIN_0, p.PIN_1);
    let (spi, cs) = module::init_spi(p.SPI0, p.PIN_16, p.PIN_19, p.PIN_18, p.PIN_17);

    // GPIO 25 is the onboard LED
    let mut led = Output::new(p.PIN_25, Level::Low);

    let mut flight_state = state::FlightState::new(i2c_bus, spi, cs).await;
    loop {
        flight_state.transition().await;
        flight_state.execute().await;

        log::info!("Current Flight Mode: {}", flight_state.flight_mode_name());

        // Toggle LED
        led.toggle();
        Timer::after_millis(500).await;
    }
}

#[embassy_executor::task]
async fn logger_task(driver: embassy_rp::usb::Driver<'static, embassy_rp::peripherals::USB>) -> ! {
    embassy_usb_logger::run!(1024, log::LevelFilter::Info, driver);
}
