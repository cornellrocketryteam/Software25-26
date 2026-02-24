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
pub mod actuator;
pub mod umbilical;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // Initialize USB subsystem (umbilical in release mode)
    let usb_driver = module::init_usb_driver(p.USB);
    umbilical::setup(&spawner, usb_driver);

    // Give logger a moment to attach in debug mode
    if cfg!(debug_assertions) {
        Timer::after_millis(5000).await;
    }

    let i2c_bus = module::init_shared_i2c(p.I2C0, p.PIN_0, p.PIN_1);
    let (spi, cs) = module::init_spi(
        p.SPI0, p.PIN_16, p.PIN_19, p.PIN_18, p.PIN_17, p.DMA_CH2, p.DMA_CH3,
    );
    let uart = module::init_uart1(p.UART1, p.PIN_4, p.PIN_5, p.DMA_CH0, p.DMA_CH1);

    // Onboard LED for heartbeat
    let mut led = Output::new(p.PIN_25, Level::Low);

    // Arming Switch and Umbilical Sense
    let arming_switch = embassy_rp::gpio::Input::new(p.PIN_10, embassy_rp::gpio::Pull::Down);
    let umbilical_sense = embassy_rp::gpio::Input::new(p.PIN_24, embassy_rp::gpio::Pull::Down);

    // Actuators
    let (ssa, buzzer, mav, sv) = module::init_actuators(
        p.PIN_2,
        p.PIN_3,
        p.PIN_6,
        p.PWM_SLICE3,
        p.PIN_7,
        p.PIN_8,
    );

    log::info!("Initializing Flight State (Sensors & Actuators)...");
    let mut flight_state = state::FlightState::new(
        i2c_bus, spi, cs, arming_switch, umbilical_sense, uart,
        ssa,
        buzzer,
        mav,
        sv,
    ).await;
    log::info!("Flight State Initialized.");

    // Reset FRAM for testing (COMMENT OUT FOR REAL FLIGHT)
    flight_state.reset_fram().await;

    let mut flight_loop = flight_loop::FlightLoop::new(flight_state);

    // STEP 4: Run actual flight loop with real telemetry
    loop {
        flight_loop.flight_state.cycle_count += 1;
        flight_loop.execute().await;

        // Toggle LED for heartbeat (every 10 cycles = 1 Hz blink at 10 Hz loop)
        if flight_loop.flight_state.cycle_count % 10 == 0 {
            led.toggle();
        }
        Timer::after_millis(constants::MAIN_LOOP_DELAY_MS).await;
    }
}
