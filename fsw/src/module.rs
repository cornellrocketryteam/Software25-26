//! USB Logger and Sensor Module
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice as SharedI2cDevice;
use embassy_rp::gpio::Output;
use embassy_rp::i2c::{Config as I2cConfig, I2c, InterruptHandler as I2cInterruptHandler};
use embassy_rp::peripherals::{
    DMA_CH0, DMA_CH1, DMA_CH2, DMA_CH3, I2C0, PIN_0, PIN_1, PIN_16, PIN_17, PIN_18, PIN_19, PIN_4, PIN_5, SPI0,
    UART1, USB,
};
use embassy_rp::spi::{Config as SpiConfig, Spi};
use embassy_rp::uart::{Config as UartConfig, InterruptHandler as UartInterruptHandler, Uart};
use embassy_rp::usb::{Driver, InterruptHandler as UsbInterruptHandler};
use embassy_rp::{bind_interrupts, i2c, spi, uart, Peri};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;

pub type SharedI2c = Mutex<NoopRawMutex, I2c<'static, I2C0, i2c::Async>>;
pub type I2cDevice<'a> = SharedI2cDevice<'a, NoopRawMutex, I2c<'static, I2C0, i2c::Async>>;

bind_interrupts!(pub struct Irqs {
    USBCTRL_IRQ => UsbInterruptHandler<USB>;
    I2C0_IRQ => I2cInterruptHandler<I2C0>;
    UART1_IRQ => UartInterruptHandler<UART1>;
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

/// Initialize SPI for FRAM
///
/// Returns async SPI instance and CS pin
pub fn init_spi(
    spi0: Peri<'static, SPI0>,
    miso: Peri<'static, PIN_16>,
    mosi: Peri<'static, PIN_19>,
    clk: Peri<'static, PIN_18>,
    cs: Peri<'static, PIN_17>,
    tx_dma: Peri<'static, DMA_CH2>,
    rx_dma: Peri<'static, DMA_CH3>,
) -> (Spi<'static, SPI0, spi::Async>, Output<'static>) {
    // Configure SPI - FRAM can typically run at MHz speeds
    let mut spi_config = SpiConfig::default();
    spi_config.frequency = 1_000_000; // 1 MHz for safety, can go higher

    let spi = Spi::new(spi0, clk, mosi, miso, tx_dma, rx_dma, spi_config);

    // CS pin starts high (inactive)
    let cs = Output::new(cs, embassy_rp::gpio::Level::High);

    (spi, cs)
}

/// Initialize UART1 for RFD900x radio
///
/// Returns async UART instance configured at 9600 baud
pub fn init_uart1(
    uart1: Peri<'static, UART1>,
    tx: Peri<'static, PIN_4>,
    rx: Peri<'static, PIN_5>,
    tx_dma: Peri<'static, DMA_CH0>,
    rx_dma: Peri<'static, DMA_CH1>,
) -> Uart<'static, uart::Async> {
    // Configure UART for RFD900x (115200 baud, 8N1)
    let mut uart_config = UartConfig::default();
    uart_config.baudrate = 115200;

    Uart::new(uart1, tx, rx, Irqs, tx_dma, rx_dma, uart_config)
}
