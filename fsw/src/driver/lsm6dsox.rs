use crate::module::{I2cDevice, SharedI2c};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice as SharedI2cDevice;
use embassy_time::{Duration, Timer};
use embedded_hal_async::i2c::I2c;

/// LSM6DSOX I2C address (SA0 grounded)
const LSM6DSOX_ADDR: u8 = 0x6A; // try 0x6B

/// Register addresses
const REG_WHO_AM_I: u8 = 0x0F;

const REG_CTRL1_XL: u8 = 0x10;
const REG_CTRL2_G: u8  = 0x11;
const REG_CTRL3_C: u8  = 0x12;

const REG_OUTX_L_G: u8 = 0x22;


const WHO_AM_I_VALUE: u8 = 0x6C;

const ACCEL_SCALE_2G: f32 = 9.80665 / 16384.0;    // datasheet values for 2G -- double check 
const GYRO_SCALE_250DPS: f32 = 1.0 / 131.0;       // datasheet values for 2G -- double check 


#[derive(Debug)]
pub enum Lsm6dsoxError<E> {
    I2c(E),
    InvalidDeviceId(u8),
}

impl<E> From<E> for Lsm6dsoxError<E> {
    fn from(error: E) -> Self {
        Lsm6dsoxError::I2c(error)
    }
}

/// LSM6DSOX 6-axis IMU driver
pub struct Lsm6dsoxSensor {
    i2c: I2cDevice<'static>,
}

impl Lsm6dsoxSensor {
    pub async fn new(i2c_bus: &'static SharedI2c) -> Self {
        let mut sensor = Self {
            i2c: SharedI2cDevice::new(i2c_bus),
        };

        match sensor.init().await {
            Ok(_) => log::info!("LSM6DSOX initialized"),
            Err(e) => log::error!("Failed to initialize LSM6DSOX: {:?}", e),
        }

        sensor
    }

    async fn init(
        &mut self
    ) -> Result<(), Lsm6dsoxError<<I2cDevice<'static> as embedded_hal_async::i2c::ErrorType>::Error>> {
        Timer::after(Duration::from_millis(20)).await;
        log::info!("LSM6DSOX: reading WHO_AM_I...");
        let who_am_i = self.read_register(REG_WHO_AM_I).await?;
        log::info!("Address 0x{:02X} WHO_AM_I = 0x{:02X}", LSM6DSOX_ADDR, who_am_i);
        if who_am_i != WHO_AM_I_VALUE {
            return Err(Lsm6dsoxError::InvalidDeviceId(who_am_i));
        }

        // Configure accel: 416 Hz, ±2g
        log::info!("LSM6DSOX: setting CTRL1_XL = 0x60...");
        self.write_register(REG_CTRL1_XL, 0x60).await?;
        log::info!("LSM6DSOX: CTRL1_XL done");

        // Configure gyro: 416 Hz, 250 dps
        log::info!("LSM6DSOX: setting CTRL2_G = 0x60...");
        self.write_register(REG_CTRL2_G, 0x60).await?;
        log::info!("LSM6DSOX: CTRL2_G done");

        // BDU enable (block data update)
        log::info!("LSM6DSOX: setting CTRL3_C = 0x04...");
        self.write_register(REG_CTRL3_C, 0x04).await?;
        log::info!("LSM6DSOX: init complete");
        Ok(())
    }

    async fn write_register(&mut self,register: u8,value: u8) -> Result<(), Lsm6dsoxError<<I2cDevice<'static> as embedded_hal_async::i2c::ErrorType>::Error>> {
        let data = [register, value];
        self.i2c.write(LSM6DSOX_ADDR, &data).await?;
        Ok(())
    }

    async fn read_register(&mut self,register: u8) -> Result<u8, Lsm6dsoxError<<I2cDevice<'static> as embedded_hal_async::i2c::ErrorType>::Error>> {
        let mut buffer = [0u8; 1];
        self.i2c.write_read(LSM6DSOX_ADDR, &[register], &mut buffer).await?;
        Ok(buffer[0])
    }

    async fn read_registers(&mut self,register: u8, buffer: &mut [u8]) -> Result<(), Lsm6dsoxError<<I2cDevice<'static> as embedded_hal_async::i2c::ErrorType>::Error>> {
        self.i2c.write_read(LSM6DSOX_ADDR, &[register], buffer).await?;
        Ok(())
    }

    pub async fn read_into_packet(&mut self,packet: &mut crate::packet::Packet,) -> Result<(), Lsm6dsoxError<<I2cDevice<'static> as embedded_hal_async::i2c::ErrorType>::Error>> {

        let mut data = [0u8; 12];

        self.read_registers(REG_OUTX_L_G, &mut data).await?;

        let gyro_x_raw = i16::from_le_bytes([data[0], data[1]]);
        let gyro_y_raw = i16::from_le_bytes([data[2], data[3]]);
        let gyro_z_raw = i16::from_le_bytes([data[4], data[5]]);

        let accel_x_raw = i16::from_le_bytes([data[6], data[7]]);
        let accel_y_raw = i16::from_le_bytes([data[8], data[9]]);
        let accel_z_raw = i16::from_le_bytes([data[10], data[11]]);

        packet.accel_x = accel_x_raw as f32 * ACCEL_SCALE_2G;
        packet.accel_y = accel_y_raw as f32 * ACCEL_SCALE_2G;
        packet.accel_z = accel_z_raw as f32 * ACCEL_SCALE_2G;

        packet.gyro_x = gyro_x_raw as f32 * GYRO_SCALE_250DPS;
        packet.gyro_y = gyro_y_raw as f32 * GYRO_SCALE_250DPS;
        packet.gyro_z = gyro_z_raw as f32 * GYRO_SCALE_250DPS;

        Ok(())
    }
}