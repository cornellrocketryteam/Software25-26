use crate::module::{I2cDevice, SharedI2c};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice as SharedI2cDevice;
use embassy_time::{Duration, Timer};
use embedded_hal_async::i2c::I2c;

/// AK09915 I2C address
const AK09915_ADDR: u8 = 0x0C;

/// Register addresses
const REG_ST1: u8 = 0x10; // Status 1
const REG_HXL: u8 = 0x11; // X-axis data low
const REG_ST2: u8 = 0x18; // Status 2
const REG_CNTL2: u8 = 0x31; // Control 2 (mode)
const REG_CNTL3: u8 = 0x32; // Control 3 (reset)

/// Mode values for CNTL2 register
const MODE_CONTINUOUS_200HZ: u8 = 0x10;

/// Soft reset value for CNTL3 register
const SOFT_RESET: u8 = 0x01;

/// Status bits
const ST1_DRDY: u8 = 0x01; // Data ready bit in ST1
const ST2_HOFL: u8 = 0x08; // Magnetic sensor overflow bit in ST2
const ST2_INV: u8 = 0x04; // Invalid data bit in ST2

/// Sensitivity: 0.15 µT per LSB
const SENSITIVITY: f32 = 0.15;

/// AK09915 magnetometer errors
#[derive(Debug)]
pub enum Ak09915Error<E> {
    I2c(E),
    MagneticSensorOverflow,
    InvalidData,
}

impl<E> From<E> for Ak09915Error<E> {
    fn from(error: E) -> Self {
        Ak09915Error::I2c(error)
    }
}

/// AK09915 magnetometer driver wrapper
pub struct Ak09915Sensor {
    i2c: I2cDevice<'static>,
}

impl Ak09915Sensor {
    /// Create a new AK09915 sensor instance
    ///
    /// Takes a shared I2C bus and returns an AK09915 magnetometer configured for continuous readings
    pub async fn new(i2c_bus: &'static SharedI2c) -> Self {
        let mut sensor = Self {
            i2c: SharedI2cDevice::new(i2c_bus),
        };

        // Initialize sensor: soft reset and configure to 200Hz continuous mode
        match sensor.init().await {
            Ok(_) => log::info!("AK09915 magnetometer initialized successfully at 200Hz"),
            Err(_) => log::error!("Failed to initialize AK09915"),
        }

        sensor
    }

    /// Initialize the sensor with soft reset and 200Hz continuous mode
    async fn init(&mut self) -> Result<(), Ak09915Error<<I2cDevice<'static> as embedded_hal_async::i2c::ErrorType>::Error>> {
        // Soft reset
        self.write_register(REG_CNTL3, SOFT_RESET).await?;

        // Wait for reset to complete (100us as per datasheet, add margin)
        Timer::after(Duration::from_millis(1)).await;

        // Set continuous 200Hz mode
        self.write_register(REG_CNTL2, MODE_CONTINUOUS_200HZ).await?;

        // Wait for first measurement to complete at 200Hz (5ms per cycle + margin)
        Timer::after(Duration::from_millis(10)).await;

        Ok(())
    }

    /// Write a value to a register
    async fn write_register(&mut self, register: u8, value: u8) -> Result<(), Ak09915Error<<I2cDevice<'static> as embedded_hal_async::i2c::ErrorType>::Error>> {
        let data = [register, value];
        self.i2c.write(AK09915_ADDR, &data).await?;
        Ok(())
    }

    /// Read a single register
    async fn read_register(&mut self, register: u8) -> Result<u8, Ak09915Error<<I2cDevice<'static> as embedded_hal_async::i2c::ErrorType>::Error>> {
        let mut buffer = [0u8; 1];
        self.i2c.write_read(AK09915_ADDR, &[register], &mut buffer).await?;
        Ok(buffer[0])
    }

    /// Read multiple bytes starting from a register
    async fn read_registers(&mut self, register: u8, buffer: &mut [u8]) -> Result<(), Ak09915Error<<I2cDevice<'static> as embedded_hal_async::i2c::ErrorType>::Error>> {
        self.i2c.write_read(AK09915_ADDR, &[register], buffer).await?;
        Ok(())
    }

    /// Read AK09915 magnetometer data and update the packet
    ///
    /// This method reads magnetic field measurements (X, Y, Z axes) from the AK09915
    /// and directly updates the provided packet with values in microtesla (µT).
    pub async fn read_into_packet(
        &mut self,
        packet: &mut crate::packet::Packet,
    ) -> Result<(), Ak09915Error<<I2cDevice<'static> as embedded_hal_async::i2c::ErrorType>::Error>> {
        // Check if data is ready by reading ST1
        let st1 = self.read_register(REG_ST1).await?;
        if (st1 & ST1_DRDY) == 0 {
            // Data not ready yet, return without error (keep previous values)
            return Ok(());
        }

        // Read 8 bytes: HXL, HXH, HYL, HYH, HZL, HZH, TMPS, ST2
        // This reads from 0x11 to 0x18 in a single transaction
        // ST2 must be read immediately after data to trigger next measurement
        let mut data = [0u8; 8];
        self.read_registers(REG_HXL, &mut data).await?;

        // ST2 is the last byte (index 7)
        let st2 = data[7];

        // Check for magnetic sensor overflow
        if (st2 & ST2_HOFL) != 0 {
            return Err(Ak09915Error::MagneticSensorOverflow);
        }

        // Check for invalid data
        if (st2 & ST2_INV) != 0 {
            return Err(Ak09915Error::InvalidData);
        }

        // Convert bytes to signed 16-bit integers (little-endian)
        let mag_x_raw = i16::from_le_bytes([data[0], data[1]]);
        let mag_y_raw = i16::from_le_bytes([data[2], data[3]]);
        let mag_z_raw = i16::from_le_bytes([data[4], data[5]]);

        // Convert to microtesla (µT) using sensitivity of 0.15 µT/LSB
        packet.mag_x = mag_x_raw as f32 * SENSITIVITY;
        packet.mag_y = mag_y_raw as f32 * SENSITIVITY;
        packet.mag_z = mag_z_raw as f32 * SENSITIVITY;

        Ok(())
    }
}
