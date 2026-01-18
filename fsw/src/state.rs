use crate::module::*;

use crate::packet::Packet;

use crate::driver::ak09915::Ak09915Sensor;
use crate::driver::bmp390::Bmp390Sensor;
use crate::driver::main_fram::Fram;
use crate::driver::icm42688::Icm42688Sensor;
use crate::driver::rfd900x::Rfd900x;
use crate::driver::ublox_max_m10s::UbloxMaxM10s;

use embassy_rp::gpio::{Input, Output};
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
    pub key_armed: bool,
    pub umbilical_connected: bool,

    // altimeter
    altimeter: Bmp390Sensor,
    pub altimeter_state: SensorState,
    pub reference_pressure: f32,
    
    // storage status
    pub sd_logging_enabled: bool,

    // fram
    fram: Fram<'static>,

    // gps
    gps: UbloxMaxM10s<'static, I2cDevice<'static>>,

    // magnetometer
    magnetometer: Ak09915Sensor,

    // imu
    imu: Icm42688Sensor,

    // actuators
    arming_switch: Input<'static>,
    umbilical_sense: Input<'static>,
    pub arming_altitude: f32,

    // telemetry
    radio: Rfd900x<'static>,
}

impl FlightState {
    pub async fn new(
        i2c_bus: &'static SharedI2c,
        spi: Spi<'static, SPI0, embassy_rp::spi::Async>,
        cs: Output<'static>,
        arming_switch: Input<'static>,
        umbilical_sense: Input<'static>,
        uart: Uart<'static, Async>,
    ) -> Self {
        let packet = Packet::default();
        let altimeter = Bmp390Sensor::new(i2c_bus).await;
        let fram = Fram::new(spi, cs);
        let mut gps = UbloxMaxM10s::new(i2c_bus);

        // Configure GPS module to output NAV-PVT messages
        if let Err(e) = gps.configure().await {
            log::error!("Failed to configure GPS: {:?}", e);
        }

        let magnetometer = Ak09915Sensor::new(i2c_bus).await;
        let imu = Icm42688Sensor::new(i2c_bus).await;
        let radio = Rfd900x::new(uart);

        Self {
            packet: packet,
            flight_mode: FlightMode::Startup,
            cycle_count: 0,
            key_armed: false,
            umbilical_connected: false,
            altimeter: altimeter,
            altimeter_state: SensorState::OFF,
            sd_logging_enabled: false, // Default to false (SD failure assumed for now)
            fram: fram,
            gps: gps,
            magnetometer: magnetometer,
            imu: imu,
            arming_switch: arming_switch,
            umbilical_sense: umbilical_sense,
            arming_altitude: 0.0,
            radio: radio,
            reference_pressure: 0.0,
        }
    }

    pub fn read_altimeter(&mut self) -> f32{
        return self.packet.altitude;
    }

    pub fn read_barometer(&mut self) -> f32{
        return self.packet.pressure;
    }

    pub async fn read_sensors(&mut self) {
        // Update packet flight mode
        self.packet.flight_mode = self.flight_mode as u32;

        // Update key armed status
        self.key_armed = self.arming_switch.is_high();
        self.umbilical_connected = self.umbilical_sense.is_high();

        // Note: Umbilical state reading will be handled directly in flight_loop for strict logic,
        // or we can add a field to FlightState if needed globally.

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
                self.altimeter_state = SensorState::VALID;
                log::info!(
                    "BMP | Pressure = {:.2} Pa, Temp = {:.2} °C, Alt = {:.2} m",
                    self.packet.pressure,
                    self.packet.temp,
                    self.packet.altitude
                );
            }
            Err(e) => {
                self.altimeter_state = SensorState::INVALID;
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

        // Read magnetometer and update packet
        match self.magnetometer.read_into_packet(&mut self.packet).await {
            Ok(_) => {
                log::info!(
                    "MAG | X = {:.2} µT, Y = {:.2} µT, Z = {:.2} µT",
                    self.packet.mag_x,
                    self.packet.mag_y,
                    self.packet.mag_z
                );
            }
            Err(e) => {
                log::error!("Failed to read AK09915 magnetometer: {:?}", e);
            }
        }

        // Read IMU and update packet
        match self.imu.read_into_packet(&mut self.packet).await {
            Ok(_) => {
                log::info!(
                    "IMU | Accel: X={:.2} Y={:.2} Z={:.2} m/s² | Gyro: X={:.2} Y={:.2} Z={:.2} °/s",
                    self.packet.accel_x,
                    self.packet.accel_y,
                    self.packet.accel_z,
                    self.packet.gyro_x,
                    self.packet.gyro_y,
                    self.packet.gyro_z
                );
            }
            Err(e) => {
                log::error!("Failed to read ICM-42688-P IMU: {:?}", e);
            }
        }
    }

    pub async fn transmit(&mut self) {
        let mut data = [0u8; 68];
        data[0..4].copy_from_slice(&self.packet.flight_mode.to_le_bytes());
        data[4..8].copy_from_slice(&self.packet.pressure.to_le_bytes());
        data[8..12].copy_from_slice(&self.packet.temp.to_le_bytes());
        data[12..16].copy_from_slice(&self.packet.altitude.to_le_bytes());
        data[16..20].copy_from_slice(&self.packet.latitude.to_le_bytes());
        data[20..24].copy_from_slice(&self.packet.longitude.to_le_bytes());
        data[24..28].copy_from_slice(&self.packet.num_satellites.to_le_bytes());
        data[28..32].copy_from_slice(&self.packet.timestamp.to_le_bytes());
        data[32..36].copy_from_slice(&self.packet.mag_x.to_le_bytes());
        data[36..40].copy_from_slice(&self.packet.mag_y.to_le_bytes());
        data[40..44].copy_from_slice(&self.packet.mag_z.to_le_bytes());
        data[44..48].copy_from_slice(&self.packet.accel_x.to_le_bytes());
        data[48..52].copy_from_slice(&self.packet.accel_y.to_le_bytes());
        data[52..56].copy_from_slice(&self.packet.accel_z.to_le_bytes());
        data[56..60].copy_from_slice(&self.packet.gyro_x.to_le_bytes());
        data[60..64].copy_from_slice(&self.packet.gyro_y.to_le_bytes());
        data[64..68].copy_from_slice(&self.packet.gyro_z.to_le_bytes());

        // Transmit via radio
        match self.radio.send(&data).await {
            Ok(_) => {
                log::info!("Transmitted packet via radio");
            }
            Err(e) => {
                log::warn!("Failed to transmit packet via radio: {:?}", e);
            }
        }
    }
    /*
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
    */

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

    /// Logs critical PT (Pressure/Temperature/Altitude) data to FRAM
    /// This is a fallback if SD logging fails.
    pub async fn log_to_fram(&mut self) {
        // Simple ring buffer or sequential write could be implemented here.
        // For now, we just overwrite a scratchpad area (e.g., address 100) 
        // with the latest Altitude (as u32 representation).
        // Real implementation would manage addresses.
        
        let alt_bits = self.packet.altitude.to_bits();
        if let Err(_) = self.fram.write_u32(100, alt_bits).await {
             log::warn!("Failed to write PT data to FRAM");
        }
    }
}
