use bmp390_rs::{Bmp390, ResetPolicy};
use bmp390_rs::config::Configuration;
use crate::module::{SpiDevice, SharedSpi};
use embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice as SharedSpiDevice;
use embassy_rp::gpio::Output;
use embassy_time::Delay;

/// Expected value of the BMP390 CHIP_ID register (0x00). A healthy BMP390 always
/// reports this; a floating/disconnected SPI bus reports 0xFF or 0x00 instead.
const BMP390_CHIP_ID: u8 = 0x60;

/// BMP390 sensor driver wrapper
pub struct Bmp390Sensor<'a> {
    sensor: Option<Bmp390<bmp390_rs::bus::Spi<SpiDevice<'a>>>>,
    altimeter_init: bool,
}

impl<'a> Bmp390Sensor<'a> {
    /// Create a new BMP390 sensor instance
    ///
    /// Takes a shared SPI bus and returns a BMP390 sensor configured for pressure, temperature, and altitude readings
    pub async fn new(spi_bus: &'static SharedSpi, cs: Output<'a>) -> Self {
        let spi_device = SharedSpiDevice::new(spi_bus, cs);

        // Create BMP390 configuration
        let config = Configuration::default();

        // Initialize BMP390 sensor via SPI
        let mut delay = Delay;
        let (sensor_opt, init_success) = match Bmp390::new_spi(spi_device, config, ResetPolicy::Soft, &mut delay).await {
            Ok(s) => {
                log::info!("BMP390 sensor initialized successfully via SPI");
                (Some(s), true)
            }
            Err(e) => {
                // Log the error but no crash
                log::error!("Failed to initialize BMP390: {:?}", e);
                (None, false)
            }
        };

        Self { 
            sensor: sensor_opt,
            altimeter_init: init_success,
        }
    }

    /// Read BMP390 sensor data and update the packet
    ///
    /// This method measures pressure, temperature, and altitude from the BMP390
    /// and directly updates the provided packet.
    pub async fn read_into_packet(
    &mut self,
    packet: &mut crate::packet::Packet,
    ) -> Result<(), bmp390_rs::error::Bmp390Error<<SpiDevice<'a> as embedded_hal_async::spi::ErrorType>::Error>> {
        let sensor = self.sensor.as_mut().ok_or(bmp390_rs::error::Bmp390Error::NotConnected)?;

        // Disconnect detector: read CHIP_ID (0x00) first. A healthy BMP390 always
        // returns 0x60. When the sensor is disconnected mid-flight, SPI reads
        // "succeed" but MISO floats — reading 0xFF (floating high) or 0x00 (pulled
        // low), neither of which is 0x60. This deterministically catches the dead
        // sensor regardless of what bogus pressure the float would compensate to,
        // which is what lets the pressure range below stay very loose.
        let chip_id = sensor.read::<bmp390_rs::register::chip_id::ChipId>().await?;
        if chip_id != BMP390_CHIP_ID {
            return Err(bmp390_rs::error::Bmp390Error::NotConnected);
        }

        let meas = sensor.read_sensor_data().await?;

        let pressure = meas.pressure();

        // Loose sanity backstop only — disconnect is handled by the CHIP_ID check
        // above. Bounds are PRESSURE_MIN_PA..=PRESSURE_MAX_PA; the floor sits far
        // above any altitude we can reach so real flight pressure is never rejected.
        if !(crate::constants::PRESSURE_MIN_PA..=crate::constants::PRESSURE_MAX_PA).contains(&pressure) {
            return Err(bmp390_rs::error::Bmp390Error::NotConnected);
        }

        packet.pressure = pressure;
        packet.temp = meas.temperature();

        // Calculate altitude using the NOAA formula
        let sea_level_pa = 101325.0;
        packet.altitude = 44330.0 * (1.0 - libm::powf(pressure / sea_level_pa, 0.190295));

        Ok(())
    }

    /// Check if the sensor was successfully initialized
    pub fn is_init(&self) -> bool {
        self.altimeter_init
    }

    /// Construct an "unavailable" BMP390 for cases where init timed out or
    /// the bus is suspect. All reads will return `NotConnected`.
    pub fn unavailable() -> Self {
        Self {
            sensor: None,
            altimeter_init: false,
        }
    }
}