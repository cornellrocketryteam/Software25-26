use crate::module::*;

use crate::packet::Packet;

use crate::driver::bmp390::Bmp390Sensor;
use crate::driver::fram::Fram;
use crate::driver::rfd900x::Rfd900x;
use crate::driver::ublox_max_m10s::UbloxMaxM10s;

use embassy_rp::gpio::Output;
use embassy_rp::peripherals::SPI0;
use embassy_rp::spi::Spi;
use embassy_rp::uart::{Async, Uart};

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
    // packet
    pub packet: Packet,
    // state variables
    pub flight_mode: FlightMode,
    pub cycle_count: u32,

    // altimeter
    altimeter: Bmp390Sensor,

    // fram
    fram: Fram<'static>,

    // gps
    gps: UbloxMaxM10s<'static, I2cDevice<'static>>,

    // actuators

    // telemetry
    radio: Rfd900x<'static>,
}

impl FlightState {
    pub async fn new(
        i2c_bus: &'static SharedI2c,
        spi: Spi<'static, SPI0, embassy_rp::spi::Async>,
        cs: Output<'static>,
        uart: Uart<'static, Async>,
    ) -> Self {
        let packet = Packet::default();
        let altimeter = Bmp390Sensor::new(i2c_bus).await;
        let fram = Fram::new(spi, cs);
        let gps = UbloxMaxM10s::new(i2c_bus);
        let radio = Rfd900x::new(uart);

        Self {
            packet: packet,
            flight_mode: FlightMode::Startup,
            cycle_count: 0,
            altimeter: altimeter,
            fram: fram,
            gps: gps,
            radio: radio,
        }
    }

    pub async fn read_sensors(&mut self) {
        // Update packet flight mode
        self.packet.flight_mode = self.flight_mode as u32;

        // Read from FRAM
        match self.fram.read_u32(0).await {
            Ok(raw) => {
                log::info!("FlightMode read from FRAM: {:?}", FlightMode::from_u32(raw));
            }
            Err(_) => {
                log::warn!("Failed to read the FlightMode from FRAM!");
            }
        }

        // Write to FRAM
        if let Err(_) = self.fram.write_u32(0, self.flight_mode as u32).await {
            log::warn!("Failed to write the FlightMode to FRAM!");
        }

        // Read altimeter and update packet
        match self.altimeter.read_into_packet(&mut self.packet).await {
            Ok(_) => {
                log::info!(
                    "BMP | Pressure = {:.2} Pa, Temp = {:.2} °C, Alt = {:.2} m",
                    self.packet.pressure,
                    self.packet.temp,
                    self.packet.altitude
                );
            }
            Err(e) => {
                log::error!("Failed to read BMP390: {:?}", e);
            }
        }

        // Read GPS and update packet
        match self.gps.read_into_packet(&mut self.packet).await {
            Ok(_) => {
                log::info!(
                    "GPS | Lat = {:.6}°, Lon = {:.6}°, Sats = {}, Time = {:.0} s",
                    self.packet.latitude,
                    self.packet.longitude,
                    self.packet.num_satellites,
                    self.packet.timestamp
                );
            }
            Err(e) => {
                log::error!("Failed to read GPS: {:?}", e);
            }
        }
    }

    pub async fn transmit(&mut self) {
        use core::fmt::Write;
        let mut buffer = heapless::String::<256>::new();

        // Format packet data into a single transmission
        let _ = write!(
            &mut buffer,
            "FM={},P={:.2},T={:.2},A={:.2},LAT={:.6},LON={:.6},SAT={},TS={:.0}\n",
            self.packet.flight_mode,
            self.packet.pressure,
            self.packet.temp,
            self.packet.altitude,
            self.packet.latitude,
            self.packet.longitude,
            self.packet.num_satellites,
            self.packet.timestamp
        );

        // Transmit via radio
        match self.radio.send(buffer.as_bytes()).await {
            Ok(_) => {
                log::info!("Transmitted packet via radio");
            }
            Err(e) => {
                log::warn!("Failed to transmit packet via radio: {:?}", e);
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
        // Read sensors and update packet
        self.read_sensors().await;

        // Transmit packet via radio
        self.transmit().await;
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
