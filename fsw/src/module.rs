//! Global module state and sensor data

use crate::drivers::bmp390::BMP390;
use cortex_m::delay::Delay;
use hal::gpio;
use hal::pac;
use hal::Clock;
use hal::I2C;
use rp_hal as hal;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensorState {
    Off = 0,
    Init = 1,
    Valid = 2,
    Invalid = 3,
}

// Altimeter state
pub mod alt {
    use super::*;

    pub static mut STATUS: SensorState = SensorState::Off;
    pub static mut FAILED_READS: u8 = 0;
    pub static mut PRESSURE: f32 = -1.0;
    pub static mut TEMPERATURE: f32 = -1.0;
    pub static mut ALTITUDE: f32 = -1.0;
    pub static mut REFERENCE_PRESSURE: f32 = -1.0;
}

// Flight mode state
pub mod flight {
    use super::*;
    use crate::state::FlightMode;

    pub static mut CURRENT_MODE: FlightMode = FlightMode::Startup;
    pub static mut CYCLE_COUNT: u32 = 0;
}

// GPIO pins
pub mod gpio_pins {
    use super::*;

    pub type LedPin = gpio::Pin<
        gpio::bank0::Gpio25,
        gpio::FunctionSio<gpio::SioOutput>,
        gpio::PullDown,
    >;

    pub static mut LED: Option<LedPin> = None;
}

// Sensors
pub mod sensors {
    use super::*;

    pub type Bmp390Type = BMP390<
        I2C<
            hal::pac::I2C0,
            (
                gpio::Pin<gpio::bank0::Gpio0, gpio::FunctionI2c, gpio::PullUp>,
                gpio::Pin<gpio::bank0::Gpio1, gpio::FunctionI2c, gpio::PullUp>,
            ),
        >,
    >;

    pub static mut BMP390: Option<Bmp390Type> = None;
}

// Delay provider
pub mod delay {
    use super::*;

    pub static mut DELAY: Option<crate::DelayWrapper> = None;
}

// Watchdog
pub mod watchdog {
    use super::*;

    pub static mut WATCHDOG: Option<hal::Watchdog> = None;
}

pub fn init_bmp390(sensor: sensors::Bmp390Type) {
    unsafe { sensors::BMP390 = Some(sensor) };
}

pub fn init_led(led: gpio_pins::LedPin) {
    unsafe { gpio_pins::LED = Some(led) };
}

pub fn init_delay(delay_wrapper: crate::DelayWrapper) {
    unsafe { delay::DELAY = Some(delay_wrapper) };
}

pub fn init_watchdog(wd: hal::Watchdog) {
    unsafe { watchdog::WATCHDOG = Some(wd) };
}

pub fn feed_watchdog() {
    unsafe {
        let wd_ptr = core::ptr::addr_of_mut!(watchdog::WATCHDOG);
        if let Some(wd) = (*wd_ptr).as_mut() {
            wd.feed();
        }
    }
}

pub fn read_sensors() {
    unsafe {
        let bmp_ptr = core::ptr::addr_of_mut!(sensors::BMP390);
        let delay_ptr = core::ptr::addr_of_mut!(delay::DELAY);

        if let Some(bmp) = (*bmp_ptr).as_mut() {
            if let Some(delay_ref) = (*delay_ptr).as_mut() {
                match bmp.read(delay_ref) {
                    Ok(reading) => {
                        alt::STATUS = SensorState::Valid;
                        alt::FAILED_READS = 0;
                        alt::PRESSURE = reading.pressure;
                        alt::TEMPERATURE = reading.temperature;
                        alt::ALTITUDE = reading.altitude;
                    }
                    Err(_) => {
                        alt::FAILED_READS += 1;
                        if alt::FAILED_READS >= 3 {
                            alt::STATUS = SensorState::Invalid;
                        }
                    }
                }
            }
        }

        flight::CYCLE_COUNT += 1;
    }
}

pub fn to_mode(new_mode: crate::state::FlightMode) {
    unsafe { flight::CURRENT_MODE = new_mode };
}

pub fn get_current_mode() -> crate::state::FlightMode {
    unsafe { flight::CURRENT_MODE }
}

pub fn initialize_modules(mut pac: pac::Peripherals, core: cortex_m::Peripherals) {
    use crate::constants::*;

    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);
    let clocks = hal::clocks::init_clocks_and_plls(
        XOSC_FREQ_HZ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    watchdog.start(fugit::MicrosDurationU32::millis(WATCHDOG_TIMEOUT_MS));

    let delay = crate::DelayWrapper(Delay::new(core.SYST, clocks.system_clock.freq().to_Hz()));

    let sio = hal::Sio::new(pac.SIO);
    let pins = hal::gpio::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    let led_pin = pins.gpio25.into_push_pull_output();

    let sda_pin = pins
        .gpio0
        .reconfigure::<hal::gpio::FunctionI2c, hal::gpio::PullUp>();
    let scl_pin = pins
        .gpio1
        .reconfigure::<hal::gpio::FunctionI2c, hal::gpio::PullUp>();

    let i2c = hal::I2C::i2c0(
        pac.I2C0,
        sda_pin,
        scl_pin,
        fugit::RateExtU32::kHz(I2C_FREQ_KHZ),
        &mut pac.RESETS,
        &clocks.system_clock,
    );

    let mut delay_for_init = delay;
    let bmp390 = BMP390::new(i2c, &mut delay_for_init).unwrap();

    init_led(led_pin);
    init_bmp390(bmp390);
    init_delay(delay_for_init);
    init_watchdog(watchdog);
}
