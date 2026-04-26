use crate::module::{I2cDevice, SharedI2c};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice as SharedI2cDevice;
use embassy_time::{Duration, Timer};
use embedded_hal_async::i2c::I2c;

/// ICM-42688-P I2C address (AD0 pin grounded)
const ICM42688_ADDR: u8 = 0x68;

/// Register addresses (BANK 0)
const REG_WHO_AM_I: u8 = 0x75;          // Device ID register (should read 0x47)
const REG_PWR_MGMT0: u8 = 0x4E;         // Power management
const REG_GYRO_CONFIG0: u8 = 0x4F;      // Gyroscope configuration
const REG_ACCEL_CONFIG0: u8 = 0x50;     // Accelerometer configuration
const REG_ACCEL_DATA_X1: u8 = 0x1F;     // Accel X-axis (MSB)

/// Power Management Configuration
const PWR_MGMT0_ACCEL_MODE_LN: u8 = 0x03;  // Accelerometer Low Noise mode (bits 1:0)
const PWR_MGMT0_GYRO_MODE_LN: u8 = 0x0C;   // Gyroscope Low Noise mode (bits 3:2)
const PWR_MGMT0_TEMP_EN: u8 = 0x20;        // Enable temperature sensor

/// Accelerometer Configuration (±16g, ODR=1kHz)
const ACCEL_FS_SEL_16G: u8 = 0x00;     // ±16g range (bits 7-5 = 000)
const ACCEL_ODR_1KHZ: u8 = 0x06;       // 1kHz ODR (bits 3-0 = 0110)

/// Gyroscope Configuration (±2000°/s, ODR=1kHz)
const GYRO_FS_SEL_2000DPS: u8 = 0x00;  // ±2000°/s range (bits 7-5 = 000)
const GYRO_ODR_1KHZ: u8 = 0x06;        // 1kHz ODR (bits 3-0 = 0110)

/// Scale factors for ±16g accelerometer range
/// 16-bit signed: ±32768 = ±16g → 1g = 2048 LSB
/// To convert to m/s²: (raw / 2048.0) * 9.80665
const ACCEL_SCALE_16G: f32 = 9.80665 / 2048.0;  // m/s² per LSB

/// Scale factors for ±2000°/s gyroscope range
/// 16-bit signed: ±32768 = ±2000°/s → 1°/s = 16.384 LSB
const GYRO_SCALE_2000DPS: f32 = 1.0 / 16.384;   // °/s per LSB

/// Expected WHO_AM_I value
const WHO_AM_I_VALUE: u8 = 0x47;

/// ICM-42688-P IMU errors
#[derive(Debug)]
pub enum Icm42688Error<E> {
    I2c(E),
    InvalidDeviceId(u8),
}

impl<E> From<E> for Icm42688Error<E> {
    fn from(error: E) -> Self {
        Icm42688Error::I2c(error)
    }
}

/// ICM-42688-P 6-axis IMU driver
pub struct Icm42688Sensor {
    i2c: I2cDevice<'static>,
}

impl Icm42688Sensor {
    /// Create a new ICM-42688-P sensor instance
    ///
    /// Initializes the IMU with:
    /// - Accelerometer: ±16g range, 1kHz ODR, Low Noise mode
    /// - Gyroscope: ±2000°/s range, 1kHz ODR, Low Noise mode
    pub async fn new(i2c_bus: &'static SharedI2c) -> Self {
        let mut sensor = Self {
            i2c: SharedI2cDevice::new(i2c_bus),
        };

        // Initialize sensor
        match sensor.init().await {
            Ok(_) => log::info!("ICM-42688-P IMU initialized: ±16g accel, ±2000°/s gyro, 1kHz ODR"),
            Err(e) => log::error!("Failed to initialize ICM-42688-P: {:?}", e),
        }

        sensor
    }

    /// Initialize the sensor with power-on sequence
    async fn init(&mut self) -> Result<(), Icm42688Error<<I2cDevice<'static> as embedded_hal_async::i2c::ErrorType>::Error>> {
        // Step 1: Verify device ID
        let who_am_i = self.read_register(REG_WHO_AM_I).await?;
        if who_am_i != WHO_AM_I_VALUE {
            log::error!("ICM-42688-P WHO_AM_I mismatch: expected 0x{:02X}, got 0x{:02X}",
                       WHO_AM_I_VALUE, who_am_i);
            return Err(Icm42688Error::InvalidDeviceId(who_am_i));
        }
        log::info!("ICM-42688-P device ID verified: 0x{:02X}", who_am_i);

        // Step 2: Wait for device to be ready (datasheet: 1ms after power-on)
        Timer::after(Duration::from_millis(2)).await;

        // Step 3: Configure power management - enable accel, gyro, and temp in Low Noise mode
        let pwr_config = PWR_MGMT0_TEMP_EN | PWR_MGMT0_GYRO_MODE_LN | PWR_MGMT0_ACCEL_MODE_LN;
        self.write_register(REG_PWR_MGMT0, pwr_config).await?;

        // Wait for sensors to power up and stabilize (datasheet: accel=20ms, gyro=50ms)
        Timer::after(Duration::from_millis(60)).await;

        // Step 4: Configure accelerometer (±16g, 1kHz ODR)
        let accel_config = (ACCEL_FS_SEL_16G << 5) | ACCEL_ODR_1KHZ;
        self.write_register(REG_ACCEL_CONFIG0, accel_config).await?;

        // Step 5: Configure gyroscope (±2000°/s, 1kHz ODR)
        let gyro_config = (GYRO_FS_SEL_2000DPS << 5) | GYRO_ODR_1KHZ;
        self.write_register(REG_GYRO_CONFIG0, gyro_config).await?;

        // Wait for first measurement (1kHz = 1ms cycle time + margin)
        Timer::after(Duration::from_millis(5)).await;

        Ok(())
    }

    /// Write a value to a register
    async fn write_register(&mut self, register: u8, value: u8) -> Result<(), Icm42688Error<<I2cDevice<'static> as embedded_hal_async::i2c::ErrorType>::Error>> {
        let data = [register, value];
        self.i2c.write(ICM42688_ADDR, &data).await?;
        Ok(())
    }

    /// Read a single register
    async fn read_register(&mut self, register: u8) -> Result<u8, Icm42688Error<<I2cDevice<'static> as embedded_hal_async::i2c::ErrorType>::Error>> {
        let mut buffer = [0u8; 1];
        self.i2c.write_read(ICM42688_ADDR, &[register], &mut buffer).await?;
        Ok(buffer[0])
    }

    /// Read multiple bytes starting from a register
    async fn read_registers(&mut self, register: u8, buffer: &mut [u8]) -> Result<(), Icm42688Error<<I2cDevice<'static> as embedded_hal_async::i2c::ErrorType>::Error>> {
        self.i2c.write_read(ICM42688_ADDR, &[register], buffer).await?;
        Ok(())
    }

    /// Read ICM-42688-P IMU data and update the packet
    ///
    /// This method reads 6-axis data (accelerometer and gyroscope) from the ICM-42688-P
    /// and directly updates the provided packet with:
    /// - Acceleration in m/s² (X, Y, Z axes)
    /// - Angular velocity in °/s (X, Y, Z axes)
    pub async fn read_into_packet(
        &mut self,
        packet: &mut crate::packet::Packet,
    ) -> Result<(), Icm42688Error<<I2cDevice<'static> as embedded_hal_async::i2c::ErrorType>::Error>> {

        // Read 12 bytes of sensor data in a single burst read
        // Starting from ACCEL_DATA_X1 (0x1F) through GYRO_DATA_Z0 (0x2A)
        // Layout: ACCEL_X(H,L), ACCEL_Y(H,L), ACCEL_Z(H,L), GYRO_X(H,L), GYRO_Y(H,L), GYRO_Z(H,L)
        let mut data = [0u8; 12];
        self.read_registers(REG_ACCEL_DATA_X1, &mut data).await?;

        // Parse accelerometer data (big-endian: MSB first)
        let accel_x_raw = i16::from_be_bytes([data[0], data[1]]);
        let accel_y_raw = i16::from_be_bytes([data[2], data[3]]);
        let accel_z_raw = i16::from_be_bytes([data[4], data[5]]);

        // Parse gyroscope data (big-endian: MSB first)
        let gyro_x_raw = i16::from_be_bytes([data[6], data[7]]);
        let gyro_y_raw = i16::from_be_bytes([data[8], data[9]]);
        let gyro_z_raw = i16::from_be_bytes([data[10], data[11]]);

        // Convert to physical units
        // Accelerometer: ±16g range → m/s²
        packet.accel_x = accel_x_raw as f32 * ACCEL_SCALE_16G;
        packet.accel_y = accel_y_raw as f32 * ACCEL_SCALE_16G;
        packet.accel_z = accel_z_raw as f32 * ACCEL_SCALE_16G;

        // Gyroscope: ±2000°/s range → °/s
        packet.gyro_x = gyro_x_raw as f32 * GYRO_SCALE_2000DPS;
        packet.gyro_y = gyro_y_raw as f32 * GYRO_SCALE_2000DPS;
        packet.gyro_z = gyro_z_raw as f32 * GYRO_SCALE_2000DPS;

        Ok(())
    }
}
