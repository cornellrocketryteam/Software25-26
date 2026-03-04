use bmp390_rs::{Bmp390, ResetPolicy};
use bmp390_rs::config::Configuration;
use crate::module::{SpiDevice, SharedSpi};
use embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice as SharedSpiDevice;
use embassy_rp::gpio::Output;
use embassy_time::Delay;

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
        let meas = sensor.read_sensor_data().await?;

        packet.pressure = meas.pressure();
        packet.temp = meas.temperature();
        
        // Calculate altitude using the NOAA formula
        let sea_level_pa = 101325.0; // Standard sea level pressure
        packet.altitude = 44330.0 * (1.0 - libm::powf(packet.pressure / sea_level_pa, 0.190295));

        Ok(())
    }

    /// Check if the sensor was successfully initialized
    pub fn is_init(&self) -> bool {
        self.altimeter_init
    }
}