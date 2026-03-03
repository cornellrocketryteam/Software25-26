//! USB Logger and Sensor Module
use crate::constants;
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice as SharedI2cDevice;
use embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice as SharedSpiDevice;
use embassy_rp::gpio::Output;
use embassy_rp::i2c::{Config as I2cConfig, I2c, InterruptHandler as I2cInterruptHandler};
use embassy_rp::peripherals::{
    DMA_CH0, DMA_CH1, DMA_CH2, DMA_CH3, DMA_CH4, FLASH, I2C0, PIN_0, PIN_1, PIN_4, PIN_5, PIN_8, PIN_16, PIN_18, PIN_19, PIN_21, PIN_36, PIN_39, PIN_40, PIN_47, PWM_SLICE4, SPI0, UART1, USB};
use embassy_rp::spi::{Config as SpiConfig, Spi};
use embassy_rp::uart::{Config as UartConfig, InterruptHandler as UartInterruptHandler, Uart};
use embassy_rp::usb::{Driver, InterruptHandler as UsbInterruptHandler};
use embassy_rp::dma::InterruptHandler as DmaInterruptHandler;
use embassy_rp::{bind_interrupts, i2c, spi, uart, Peri};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_usb::UsbDevice;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use static_cell::StaticCell;

//use crate::driver::onboard_flash::OnboardFlash;
pub type UsbDriver = Driver<'static, USB>;
pub type SharedI2c = Mutex<NoopRawMutex, I2c<'static, I2C0, i2c::Async>>;
pub type I2cDevice<'a> = SharedI2cDevice<'a, NoopRawMutex, I2c<'static, I2C0, i2c::Async>>;

pub type SharedSpi = Mutex<NoopRawMutex, Spi<'static, SPI0, spi::Async>>;
pub type SpiDevice<'a> = SharedSpiDevice<'a, NoopRawMutex, Spi<'static, SPI0, spi::Async>, Output<'a>>;

bind_interrupts!(pub struct Irqs {
    USBCTRL_IRQ => UsbInterruptHandler<USB>;
    I2C0_IRQ => I2cInterruptHandler<I2C0>;
    UART1_IRQ => UartInterruptHandler<UART1>;
    DMA_IRQ_0 => DmaInterruptHandler<DMA_CH0>, DmaInterruptHandler<DMA_CH1>, DmaInterruptHandler<DMA_CH2>, DmaInterruptHandler<DMA_CH3>, DmaInterruptHandler<DMA_CH4>;
});

// Initialize USB driver for logger
pub fn init_usb_driver(usb: Peri<'static, USB>) -> Driver<'static, USB> {
    Driver::new(usb, Irqs)
}

// Initialize USB device to use for umbilical
pub fn init_usb_device(driver: UsbDriver) -> (UsbDevice<'static, UsbDriver>, CdcAcmClass<'static, UsbDriver>) {
    // Create embassy-usb Config
    let config = {
        let mut config = embassy_usb::Config::new(0xc0de, 0xcafe);
        config.manufacturer = Some("Cornell Rocketry Team");
        config.product = Some("Umbilical");
        config.serial_number = Some("12345678");
        config.max_power = 100;
        config.max_packet_size_0 = 64;
        config
    };

    // Create embassy-usb DeviceBuilder using the driver and config.
    let mut builder = {
        static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
        static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
        static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

        let builder = embassy_usb::Builder::new(
            driver,
            config,
            CONFIG_DESCRIPTOR.init([0; 256]),
            BOS_DESCRIPTOR.init([0; 256]),
            &mut [], // no msos descriptors
            CONTROL_BUF.init([0; 64]),
        );
        builder
    };

    // Create classes on the builder.
    let class = {
        static STATE: StaticCell<State> = StaticCell::new();
        let state = STATE.init(State::new());
        CdcAcmClass::new(&mut builder, state, 64)
    };
    let usb_device = builder.build();

    (usb_device, class)
}

// Initialize shared I2C bus
//
// Returns a shared I2C instance wrapped in a Mutex that can be used by multiple sensors
pub fn init_shared_i2c(
    i2c0: Peri<'static, I2C0>,
    sda: Peri<'static, PIN_0>,
    scl: Peri<'static, PIN_1>,
) -> &'static SharedI2c {
    // Configure I2C (fast mode)
    let mut i2c_config = I2cConfig::default();
    i2c_config.frequency = constants::I2C_FREQUENCY;

    let i2c = I2c::new_async(i2c0, scl, sda, Irqs, i2c_config);

    // Store in static memory
    static I2C_BUS: static_cell::StaticCell<SharedI2c> = static_cell::StaticCell::new();
    I2C_BUS.init(Mutex::new(i2c))
}

// Initialize shared SPI bus
//
// Returns a shared SPI instance wrapped in a Mutex that can be used by multiple sensors
pub fn init_shared_spi(
    spi0: Peri<'static, SPI0>,
    miso: Peri<'static, PIN_16>,
    mosi: Peri<'static, PIN_19>,
    clk: Peri<'static, PIN_18>,
    tx_dma: Peri<'static, DMA_CH2>,
    rx_dma: Peri<'static, DMA_CH3>,
) -> &'static SharedSpi {
    // Configure SPI (1MHz usually safe for all SPI on the bus)
    let mut spi_config = SpiConfig::default();
    spi_config.frequency = constants::SPI_FREQUENCY; // 1MHz from constants

    let spi = Spi::new(spi0, clk, mosi, miso, tx_dma, rx_dma, Irqs, spi_config);

    // Store in static memory
    static SPI_BUS: static_cell::StaticCell<SharedSpi> = static_cell::StaticCell::new();
    SPI_BUS.init(Mutex::new(spi))
}

// Initialize UART1 for RFD900x radio
//
// Returns async UART instance configured at 9600 baud
pub fn init_uart1(
    uart1: Peri<'static, UART1>,
    tx: Peri<'static, PIN_4>,
    rx: Peri<'static, PIN_5>,
    tx_dma: Peri<'static, DMA_CH0>,
    rx_dma: Peri<'static, DMA_CH1>,
) -> Uart<'static, uart::Async> {
    // Configure UART for RFD900x (8N1)
    let mut uart_config = UartConfig::default();
    uart_config.baudrate = constants::UART_BAUDRATE;

    Uart::new(uart1, tx, rx, Irqs, tx_dma, rx_dma, uart_config)
}

use crate::actuator::Ssa;

// Initialize SSA
pub fn init_ssa(
    drogue_pin: Peri<'static, PIN_36>,
    main_pin: Peri<'static, PIN_39>,
) -> Ssa<'static> {
    Ssa::new(
        Output::new(drogue_pin, embassy_rp::gpio::Level::Low),
        Output::new(main_pin, embassy_rp::gpio::Level::Low),
    )
}

use crate::actuator::Buzzer;

// Initialize Buzzer
pub fn init_buzzer(pin: Peri<'static, PIN_21>) -> Buzzer<'static> {
    Buzzer::new(Output::new(pin, embassy_rp::gpio::Level::Low))
}

use crate::actuator::Mav;
use embassy_rp::pwm::{Config as PwmConfig, Pwm};

// Initialize MAV
pub fn init_mav(
    slice: Peri<'static, PWM_SLICE4>,
    pin: Peri<'static, PIN_8>,
) -> Mav<'static> {
    let mut config = PwmConfig::default();
    
    // For 125 MHz system clock -> 330 Hz Servo frequency:
    // divider = 125.0
    // top = 3030 
    config.top = 3030;
    config.divider = 125.into(); // Needs to be integer for into() here
    
    // Using output A for Pin 8 (Slice 4A) from pinout
    let pwm = Pwm::new_output_a(slice, pin, config);
    Mav::new(pwm)
}

use crate::actuator::SV;

// Initialize SV
pub fn init_sv(pin: Peri<'static, PIN_47>) -> SV<'static> {
    SV::new(Output::new(pin, embassy_rp::gpio::Level::High)) // Active Low, so now High (Closed)
}

// Initialize all actuators
pub fn init_actuators(
    drogue_pin: Peri<'static, PIN_36>,
    main_pin: Peri<'static, PIN_39>,
    buzzer_pin: Peri<'static, PIN_21>,
    mav_slice: Peri<'static, PWM_SLICE4>,
    mav_pin: Peri<'static, PIN_8>,
    sv_pin: Peri<'static, PIN_47>,
) -> (Ssa<'static>, Buzzer<'static>, Mav<'static>, SV<'static>) {
    let ssa = init_ssa(drogue_pin, main_pin);
    let buzzer = init_buzzer(buzzer_pin);
    let mav = init_mav(mav_slice, mav_pin);
    let sv = init_sv(sv_pin);
    
    (ssa, buzzer, mav, sv)
}
/// Initialize onboard QSPI flash for packet storage
///
/// Returns an OnboardFlash driver for reading/writing packets
pub fn init_onboard_flash(
    flash: Peri<'static, FLASH>,
    dma: Peri<'static, DMA_CH4>,
) -> crate::driver::onboard_flash::OnboardFlash<'static> {
    crate::driver::onboard_flash::OnboardFlash::new(flash, dma)
}