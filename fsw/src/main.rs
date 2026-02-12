//! Cornell Rocketry Team Flight Software

#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_time::Timer;
use embedded_hal_async::i2c::I2c;
use {defmt_rtt as _, panic_probe as _};

mod constants;
mod driver;
mod module;
mod packet;
mod state;

// debugging to find the addresses, but not needed to be run all the time 
async fn scan_i2c_bus(i2c_bus: &'static module::SharedI2c) {
    log::info!("Scanning I2C bus...");
    
    for addr in 0x08..=0x77 {
        let mut i2c = embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice::new(i2c_bus);
        let mut buf = [0u8; 1];
        
        match i2c.write_read(addr, &[], &mut buf).await {
            Ok(_) => log::info!("Found device at address 0x{:02X}", addr),
            Err(_) => {} // No device at this address
        }
    }
    
    log::info!("I2C scan complete");
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    
    let p = embassy_rp::init(Default::default());
       

    

    // Initialize USB driver for logger
    let driver = module::init_usb_driver(p.USB);


    // Spawn USB logger task
    spawner.spawn(logger_task(driver).unwrap());
    

    let i2c_bus = module::init_shared_i2c(p.I2C0, p.PIN_20, p.PIN_21);

    Timer::after_secs(10).await;

     scan_i2c_bus(i2c_bus).await;
    

    let (spi, cs) = module::init_spi(
        p.SPI0, p.PIN_16, p.PIN_19, p.PIN_18, p.PIN_17, p.DMA_CH2, p.DMA_CH3,
    );
    let uart = module::init_uart1(p.UART1, p.PIN_4, p.PIN_5, p.DMA_CH0, p.DMA_CH1);


    // Onboard LED
    let mut led = Output::new(p.PIN_25, Level::Low); 

    let mut flight_state = state::FlightState::new(i2c_bus, spi, cs, uart).await;
    loop {
        flight_state.cycle_count += 1;

        flight_state.transition().await;
        flight_state.execute().await;

        log::info!(
            "Current Flight Mode: {} on cycle {}",
            flight_state.flight_mode_name(),
            flight_state.cycle_count
        );

        // Toggle LED
        led.toggle();
        Timer::after_millis(constants::MAIN_LOOP_DELAY_MS).await;
    }
}

#[embassy_executor::task]
async fn logger_task(driver: embassy_rp::usb::Driver<'static, embassy_rp::peripherals::USB>) -> ! {
    embassy_usb_logger::run!({ constants::USB_LOGGER_BUFFER_SIZE }, log::LevelFilter::Info, driver);
}
