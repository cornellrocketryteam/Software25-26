//! Cornell Rocketry Team Flight Software

#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_time::Timer;
use {defmt_rtt as _, panic_probe as _};

mod constants;
mod driver;
mod module;
mod packet;
mod state;
mod flight_loop;

// Include simulation module
#[path = "../Test/flight_sim.rs"]
mod flight_sim;

const SIMULATION_MODE: bool = true;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // Initialize USB driver for logger
    let driver = module::init_usb_driver(p.USB);

    // Spawn USB logger task
    spawner.spawn(logger_task(driver).unwrap());

    // Give logger a moment to attach if using a tool that waits, though on Pico it's usually fine
    // But logs might be lost if sent immediately before host connects.
    // However, the user is waiting.
    
    // We can't log immediately if the host isn't connected, but we can try.
    Timer::after_millis(500).await;
    log::info!("Booting Cornell Rocketry FSW...");

    let i2c_bus = module::init_shared_i2c(p.I2C0, p.PIN_0, p.PIN_1);
    let (spi, cs) = module::init_spi(
        p.SPI0, p.PIN_16, p.PIN_19, p.PIN_18, p.PIN_17, p.DMA_CH2, p.DMA_CH3,
    );
    let uart = module::init_uart1(p.UART1, p.PIN_4, p.PIN_5, p.DMA_CH0, p.DMA_CH1);

    // Onboard LED
    let mut led = Output::new(p.PIN_25, Level::Low);

    // Arming Switch and Umbilical Sense (Pins are placeholders don't know which ones to use yet)
    let arming_switch = embassy_rp::gpio::Input::new(p.PIN_2, embassy_rp::gpio::Pull::Down);
    let umbilical = embassy_rp::gpio::Input::new(p.PIN_3, embassy_rp::gpio::Pull::Down);
    
    log::info!("Initializing Flight State (Sensors)...");
    let flight_state = state::FlightState::new(i2c_bus, spi, cs, arming_switch, umbilical, uart).await;
    log::info!("Flight State Initialized.");
    let mut flight_loop = flight_loop::FlightLoop::new(flight_state);

    if SIMULATION_MODE {
        Timer::after_secs(5).await;
        log::info!("\nStarting Simulation Mode...");
        flight_sim::simulate_flight_s1(&mut flight_loop).await;
        loop {
            // Blink rapidly to indicate sim complete
            led.toggle();
            Timer::after_millis(100).await;
        }
    } else {
        loop {
            flight_loop.flight_state.cycle_count += 1;
            flight_loop.execute().await;

            // Toggle LED
            led.toggle();
            Timer::after_millis(constants::MAIN_LOOP_DELAY_MS).await;
        }
    }
}

#[embassy_executor::task]
async fn logger_task(driver: embassy_rp::usb::Driver<'static, embassy_rp::peripherals::USB>) -> ! {
    embassy_usb_logger::run!({ constants::USB_LOGGER_BUFFER_SIZE }, log::LevelFilter::Info, driver);
}
