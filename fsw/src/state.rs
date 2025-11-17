use crate::module::*;

use crate::driver::fram::Fram;
use bmp390::Bmp390;

use embassy_rp::gpio::Output;
use embassy_rp::peripherals::SPI0;
use embassy_rp::spi::Spi;
use ublox::AlignmentToReferenceTime;

#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum SensorState {
    OFF = 0,
    VALID = 1,
    INVALID = 2,
}

#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum FlightMode {
    Startup = 0,
    Standby = 1,
    Ascent = 2,
    Coast = 3,
    DrogueDeployed = 4,
    MainDeployed = 5,
    Fault = 6,
}

impl FlightMode {
    pub fn from_u32(raw: u32) -> Self {
        match raw {
            0 => Self::Startup,
            1 => Self::Standby,
            2 => Self::Ascent,
            3 => Self::Coast,
            4 => Self::DrogueDeployed,
            5 => Self::MainDeployed,
            _ => Self::Fault,
        }
    }
}

pub struct FlightState {
    // state variables
    flight_mode: FlightMode,
    cycle_count: u32,
    timestamp: u32,
    boot_count: u16,
    watchdog_boot_count: u8,
    old_mode: FlightMode,

    key_armed: bool,
    alt_armed: bool,
    safed: bool,

    // altimeter
    altimeter_status: SensorState,
    altimeter_failed_reads: u8,
    altimeter: Bmp390<I2cDevice<'static>>,

    // fram
    fram_initialized: bool,
    pointer_index: u32,
    fram: Fram<'static>,
    // actuators

    // telemetry
}

impl FlightState {
    pub async fn new(
        i2c_bus: &'static SharedI2c,
        spi: Spi<'static, SPI0, embassy_rp::spi::Blocking>,
        cs: Output<'static>,
    ) -> Self {
        let altimeter = init_bmp390(i2c_bus).await;
        let fram = Fram::new(spi, cs);

        Self {
            flight_mode: FlightMode::Startup,
            cycle_count: 0,
            timestamp: 0,
            boot_count: 0,
            watchdog_boot_count: 0,
            old_mode: FlightMode::Startup,
            key_armed: false,
            alt_armed: false,
            safed: false,
            altimeter_status: SensorState::VALID,
            altimeter_failed_reads: 0,
            altimeter: altimeter,
            fram_initialized: true,
            pointer_index: 0,
            fram: fram,
        }
    }

    pub async fn read_sensors(&mut self) {
        match self.fram.read_u32(self.pointer_index) {
            Ok(raw) => {
                log::info!("FlightMode read from FRAM: {:?}", FlightMode::from_u32(raw));
            }
            Err(_) => {
                log::warn!("Failed to read the FlightMode from FRAM!");
            }
        }

        if let Err(_) = self
            .fram
            .write_u32(self.pointer_index, self.flight_mode as u32)
        {
            // 6 is "Fault"
            log::warn!("Failed to read the FlightMode from FRAM!");
        }

        match self.altimeter.measure().await {
            Ok(meas) => {
                use uom::si::length::meter;
                use uom::si::pressure::pascal;
                use uom::si::thermodynamic_temperature::degree_celsius;

                let pressure = meas.pressure.get::<pascal>();
                let temp = meas.temperature.get::<degree_celsius>();
                let alt = meas.altitude.get::<meter>();

                log::info!(
                    "BMP | Pressure = {:.2} Pa, Temp = {:.2} °C, Alt = {:.2} m",
                    pressure,
                    temp,
                    alt
                );
            }
            Err(e) => {
                log::error!("Failed to read BMP390: {:?}", e);
            }
        }
    }

    pub async fn transition(&mut self) {
        self.flight_mode = match self.flight_mode {
            FlightMode::Startup => FlightMode::Standby,
            FlightMode::Standby => FlightMode::Ascent,
            FlightMode::Ascent => FlightMode::Coast,
            FlightMode::Coast => FlightMode::DrogueDeployed,
            FlightMode::DrogueDeployed => FlightMode::MainDeployed,
            FlightMode::MainDeployed => FlightMode::Fault,
            FlightMode::Fault => FlightMode::Startup,
        }
    }

    pub async fn execute(&mut self) {
        match self.flight_mode {
            FlightMode::Startup => self.read_sensors().await,
            FlightMode::Standby => self.read_sensors().await,
            FlightMode::Ascent => self.read_sensors().await,
            FlightMode::Coast => self.read_sensors().await,
            FlightMode::DrogueDeployed => self.read_sensors().await,
            FlightMode::MainDeployed => self.read_sensors().await,
            FlightMode::Fault => self.read_sensors().await,
        }
    }

    pub fn flight_mode_name(&mut self) -> &'static str {
        match self.flight_mode {
            FlightMode::Startup => "Startup",
            FlightMode::Standby => "Standby",
            FlightMode::Ascent => "Ascent",
            FlightMode::Coast => "Coast",
            FlightMode::DrogueDeployed => "DrogueDeployed",
            FlightMode::MainDeployed => "MainDeployed",
            FlightMode::Fault => "Fault",
        }
    }
}
