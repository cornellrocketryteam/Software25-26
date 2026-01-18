use bmp390::{Bmp390, Configuration};
use crate::module::{I2cDevice, SharedI2c};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice as SharedI2cDevice;
use embassy_time::Delay;
use defmt::println;

/// BMP390 sensor driver wrapper
pub struct Bmp390Sensor {
    sensor: Option<Bmp390<I2cDevice<'static>>>,
}

impl Bmp390Sensor {
    /// Create a new BMP390 sensor instance
    ///
    /// Takes a shared I2C bus and returns a BMP390 sensor configured for pressure, temperature, and altitude readings
    pub async fn new(i2c_bus: &'static SharedI2c) -> Self {
        let i2c_device = SharedI2cDevice::new(i2c_bus);

        // BMP390 default I2C address (0x77, or 0x76 if SDO is low)
        let address = bmp390::Address::Up; // 0x77

        // Create BMP390 configuration
        let config = Configuration::default();

        // Initialize BMP390 sensor
        let sensor: Option<Bmp390<SharedI2cDevice<'_, embassy_sync::blocking_mutex::raw::NoopRawMutex, embassy_rp::i2c::I2c<'static, embassy_rp::peripherals::I2C0, embassy_rp::i2c::Async>>>> = match Bmp390::try_new(i2c_device, address, Delay, &config).await {
            Ok(s) => {
                println!("altimeter read");
                log::info!("BMP390 sensor initialized successfully");
                Some(s)
            }
            Err(e) => {
                // Log the error but no crash
                println!("altimeter not read");
                log::error!("Failed to initialize BMP390: {:?}", e);
                None
            }
        };
        Self { sensor }
    }

    /// Read BMP390 sensor data and update the packet
    ///
    /// This method measures pressure, temperature, and altitude from the BMP390
    /// and directly updates the provided packet.
    pub async fn read_into_packet(
        &mut self,
        packet: &mut crate::packet::Packet,
    ) -> Result<(), bmp390::Error<<I2cDevice<'static> as embedded_hal_async::i2c::ErrorType>::Error>> {
        use uom::si::length::meter;
        use uom::si::pressure::pascal;
        use uom::si::thermodynamic_temperature::degree_celsius;

        if let Some(sensor) = &mut self.sensor {
            let meas = sensor.measure().await?;
            println!("altimeter data read");
            packet.pressure = meas.pressure.get::<pascal>() as f32;
            packet.temp = meas.temperature.get::<degree_celsius>() as f32;
            packet.altitude = meas.altitude.get::<meter>() as f32;
        } else {
            println!("altimeter data not read");
            packet.pressure = 0.0;
            packet.temp = 0.0;
            packet.altitude = 0.0;
        }
        Ok(())
    }
}
