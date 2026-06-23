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

// Accel: ±16g range. LSM6DSOX ±16g sensitivity is 0.488 mg/LSB = 2048 LSB/g.
// ±2g saturated during boost (rocket pulls ~11g), so we must use the full ±16g range.
const ACCEL_SCALE_16G: f32 = 9.80665 / 2048.0;    // m/s² per LSB at ±16g
// Gyro: ±2000dps range. LSM6DSOX ±2000dps sensitivity is 70 mdps/LSB.
// ±250dps saturated during high-roll boost, so we must use the full ±2000dps range.
const GYRO_SCALE_2000DPS: f32 = 70.0 / 1000.0;    // °/s per LSB at ±2000dps


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
    initialized: bool,
}

impl Lsm6dsoxSensor {
    pub fn unavailable(i2c_bus: &'static SharedI2c) -> Self {
        Self { i2c: SharedI2cDevice::new(i2c_bus), initialized: false }
    }

    pub async fn new(i2c_bus: &'static SharedI2c) -> Self {
        let mut sensor = Self {
            i2c: SharedI2cDevice::new(i2c_bus),
            initialized: false,
        };

        match sensor.init().await {
            Ok(_) => { log::info!("LSM6DSOX initialized"); sensor.initialized = true; }
            Err(e) => log::error!("Failed to initialize LSM6DSOX: {:?}", e),
        }

        sensor
    }

    async fn init(
        &mut self
    ) -> Result<(), Lsm6dsoxError<<I2cDevice<'static> as embedded_hal_async::i2c::ErrorType>::Error>> {
        Timer::after(Duration::from_millis(20)).await;
        let who_am_i = self.read_register(REG_WHO_AM_I).await?;
        log::info!("Address 0x{:02X} WHO_AM_I = 0x{:02X}", LSM6DSOX_ADDR, who_am_i);
        if who_am_i != WHO_AM_I_VALUE {
            return Err(Lsm6dsoxError::InvalidDeviceId(who_am_i));
        }

        // Configure accel: 416 Hz, ±16g (CTRL1_XL: ODR_XL=0110, FS_XL=01)
        self.write_register(REG_CTRL1_XL, 0x64).await?;
        // Configure gyro: 416 Hz, ±2000 dps (CTRL2_G: ODR_G=0110, FS_G=11)
        self.write_register(REG_CTRL2_G, 0x6C).await?;
        // BDU enable (block data update)
        self.write_register(REG_CTRL3_C, 0x04).await?;
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

    /// Lightweight presence check: read WHO_AM_I and return true if the sensor
    /// responds with the expected value. Used to detect reconnection without
    /// re-running the full init sequence.
    pub async fn probe(&mut self) -> bool {
        match self.read_register(REG_WHO_AM_I).await {
            Ok(id) => id == WHO_AM_I_VALUE,
            Err(_) => false,
        }
    }

    pub async fn read_into_packet(&mut self,packet: &mut crate::packet::Packet,) -> Result<(), Lsm6dsoxError<<I2cDevice<'static> as embedded_hal_async::i2c::ErrorType>::Error>> {
        if !self.initialized {
            return Ok(());
        }

        let mut data = [0u8; 12];

        self.read_registers(REG_OUTX_L_G, &mut data).await?;

        let gyro_x_raw = i16::from_le_bytes([data[0], data[1]]);
        let gyro_y_raw = i16::from_le_bytes([data[2], data[3]]);
        let gyro_z_raw = i16::from_le_bytes([data[4], data[5]]);

        let accel_x_raw = i16::from_le_bytes([data[6], data[7]]);
        let accel_y_raw = i16::from_le_bytes([data[8], data[9]]);
        let accel_z_raw = i16::from_le_bytes([data[10], data[11]]);

        packet.accel_x = accel_x_raw as f32 * ACCEL_SCALE_16G;
        packet.accel_y = accel_y_raw as f32 * ACCEL_SCALE_16G;
        packet.accel_z = accel_z_raw as f32 * ACCEL_SCALE_16G;

        packet.gyro_x = gyro_x_raw as f32 * GYRO_SCALE_2000DPS;
        packet.gyro_y = gyro_y_raw as f32 * GYRO_SCALE_2000DPS;
        packet.gyro_z = gyro_z_raw as f32 * GYRO_SCALE_2000DPS;

        Ok(())
    }
}