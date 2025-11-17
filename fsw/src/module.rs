//! USB Logger and Sensor Module

use bmp390::{Bmp390, Configuration};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice as SharedI2cDevice;
use embassy_rp::gpio::Output;
use embassy_rp::i2c::{Config as I2cConfig, I2c, InterruptHandler as I2cInterruptHandler};
use embassy_rp::peripherals::{I2C0, PIN_0, PIN_1, PIN_16, PIN_17, PIN_18, PIN_19, SPI0, USB};
use embassy_rp::spi::{Config as SpiConfig, Spi};
use embassy_rp::usb::{Driver, InterruptHandler as UsbInterruptHandler};
use embassy_rp::{bind_interrupts, i2c, spi, Peri};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::Delay;

pub type SharedI2c = Mutex<NoopRawMutex, I2c<'static, I2C0, i2c::Async>>;
pub type I2cDevice<'a> = SharedI2cDevice<'a, NoopRawMutex, I2c<'static, I2C0, i2c::Async>>;

bind_interrupts!(pub struct Irqs {
    USBCTRL_IRQ => UsbInterruptHandler<USB>;
    I2C0_IRQ => I2cInterruptHandler<I2C0>;
});

/// Initialize USB driver for logger
pub fn init_usb_driver(usb: Peri<'static, USB>) -> Driver<'static, USB> {
    Driver::new(usb, Irqs)
}

/// Initialize shared I2C bus
///
/// Returns a shared I2C instance wrapped in a Mutex that can be used by multiple sensors
pub fn init_shared_i2c(
    i2c0: Peri<'static, I2C0>,
    sda: Peri<'static, PIN_0>,
    scl: Peri<'static, PIN_1>,
) -> &'static SharedI2c {
    // Configure I2C with 400kHz (fast mode)
    let mut i2c_config = I2cConfig::default();
    i2c_config.frequency = 400_000;

    let i2c = I2c::new_async(i2c0, scl, sda, Irqs, i2c_config);

    // Store in static memory
    static I2C_BUS: static_cell::StaticCell<SharedI2c> = static_cell::StaticCell::new();
    I2C_BUS.init(Mutex::new(i2c))
}

/// Initialize BMP390 sensor
///
/// Takes a shared I2C bus and returns a BMP390 sensor configured for pressure, temperature, and altitude readings
pub async fn init_bmp390(i2c_bus: &'static SharedI2c) -> Bmp390<I2cDevice<'static>> {
    let i2c_device = SharedI2cDevice::new(i2c_bus);

    // BMP390 default I2C address (0x77, or 0x76 if SDO is low)
    let address = bmp390::Address::Up; // 0x77

    // Create BMP390 configuration
    let config = Configuration::default();

    // Initialize BMP390 sensor
    let sensor = Bmp390::try_new(i2c_device, address, Delay, &config)
        .await
        .expect("Failed to initialize BMP390 sensor");

    log::info!("BMP390 sensor initialized successfully");

    sensor
}

/// Initialize SPI for FRAM
///
/// Returns SPI instance and CS pin
pub fn init_spi(
    spi0: Peri<'static, SPI0>,
    miso: Peri<'static, PIN_16>,
    mosi: Peri<'static, PIN_19>,
    clk: Peri<'static, PIN_18>,
    cs: Peri<'static, PIN_17>,
) -> (Spi<'static, SPI0, spi::Blocking>, Output<'static>) {
    // Configure SPI - FRAM can typically run at MHz speeds
    let mut spi_config = SpiConfig::default();
    spi_config.frequency = 1_000_000; // 1 MHz for safety, can go higher

    let spi = Spi::new_blocking(spi0, clk, mosi, miso, spi_config);

    // CS pin starts high (inactive)
    let cs = Output::new(cs, embassy_rp::gpio::Level::High);

    (spi, cs)
}
