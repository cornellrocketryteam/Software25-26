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
    log::info!("Booting Cornell Rocketry FSW...");
    Timer::after_millis(1000).await;
    let i2c_bus = module::init_shared_i2c(p.I2C0, p.PIN_0, p.PIN_1);
    
    // Perform an I2C scan
    /*
    log::info!("Scanning I2C Bus...");
    {
        use embedded_hal_async::i2c::I2c;
        let mut bus = i2c_bus.lock().await;
        let mut found = 0;
        for addr in 0x08u8..=0x77u8 {
            let mut buf = [0u8; 1];
            // Some I2C peripherals optimize away 0-byte reads. We request 1 byte.
            match bus.read(addr, &mut buf).await {
                Ok(_) => {
                    log::info!("Found I2C device at address: {:#04x}", addr);
                    found += 1;
                }
                Err(_) => {}
            }
        }
        log::info!("I2C scan complete. Found {} devices.", found);
    }
    */
    
    let spi_bus = module::init_shared_spi(
        p.SPI0, p.PIN_16, p.PIN_19, p.PIN_18, p.DMA_CH2, p.DMA_CH3,
    );
    let uart = module::init_uart1(p.UART1, p.PIN_4, p.PIN_5, p.DMA_CH0, p.DMA_CH1);

    let fram_cs = Output::new(p.PIN_17, Level::High);
    let _flash_cs = Output::new(p.PIN_6, Level::High); // For onboard flash
    let altimeter_cs = Output::new(p.PIN_7, Level::High);

    // Onboard LED
    let mut led = Output::new(p.PIN_25, Level::Low);

    // Arming Switch and Umbilical Sense
    let arming_switch = embassy_rp::gpio::Input::new(p.PIN_10, embassy_rp::gpio::Pull::Down);
    let umbilical_sense = embassy_rp::gpio::Input::new(p.PIN_24, embassy_rp::gpio::Pull::Down);

    // Actuators
    let (ssa, buzzer, mav, sv) = module::init_actuators(
        p.PIN_36,
        p.PIN_39,
        p.PIN_21,
        p.PWM_SLICE8,
        p.PIN_40,
        p.PIN_47,
    );
    let flash = module::init_onboard_flash(p.FLASH, p.DMA_CH4);
    
    log::info!("Initializing Flight State (Sensors & Actuators)...");
    let mut flight_state = state::FlightState::new(
        i2c_bus, spi_bus, fram_cs, altimeter_cs, arming_switch, umbilical_sense, uart,
        ssa,
        buzzer,
        mav,
        sv,
        flash,
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
