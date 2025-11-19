use bmp390::{Bmp390, Configuration};
use crate::module::{I2cDevice, SharedI2c};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice as SharedI2cDevice;
use embassy_time::Delay;

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

/// Read BMP390 sensor data and update the packet
///
/// This function measures pressure, temperature, and altitude from the BMP390
/// and directly updates the provided packet.
pub async fn read_bmp390_into_packet<I2C>(
    sensor: &mut Bmp390<I2C>,
    packet: &mut crate::packet::Packet,
) -> Result<(), bmp390::Error<I2C::Error>>
where
    I2C: embedded_hal_async::i2c::I2c,
{
    use uom::si::length::meter;
    use uom::si::pressure::pascal;
    use uom::si::thermodynamic_temperature::degree_celsius;

    let meas = sensor.measure().await?;

    packet.pressure = meas.pressure.get::<pascal>() as f32;
    packet.temp = meas.temperature.get::<degree_celsius>() as f32;
    packet.altitude = meas.altitude.get::<meter>() as f32;

    Ok(())
}
