use crate::module::{I2cDevice, SharedI2c};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice as SharedI2cDevice;
use embassy_time::Timer;
use embedded_hal_async::i2c::I2c;

/// MMC5603NJ I2C address (7-bit: 0b0110000 = 0x30)
const I2C_ADDR: u8 = 0x30;

/// Expected product ID
const PRODUCT_ID: u8 = 0x10;

// Register addresses
mod reg {
    pub const XOUT0: u8 = 0x00;
    pub const XOUT1: u8 = 0x01;
    pub const YOUT0: u8 = 0x02;
    pub const YOUT1: u8 = 0x03;
    pub const ZOUT0: u8 = 0x04;
    pub const ZOUT1: u8 = 0x05;
    pub const XOUT2: u8 = 0x06;
    pub const YOUT2: u8 = 0x07;
    pub const ZOUT2: u8 = 0x08;
    pub const TOUT: u8 = 0x09;
    pub const STATUS1: u8 = 0x18;
    pub const ODR: u8 = 0x1A;
    pub const CTRL0: u8 = 0x1B;
    pub const CTRL1: u8 = 0x1C;
    pub const CTRL2: u8 = 0x1D;
    pub const PRODUCT_ID: u8 = 0x39;
}

// Control Register 0 bits
mod ctrl0 {
    pub const TAKE_MEAS_M: u8 = 0x01;
    pub const TAKE_MEAS_T: u8 = 0x02;
    pub const DO_SET: u8 = 0x08;
    pub const DO_RESET: u8 = 0x10;
    pub const AUTO_SR_EN: u8 = 0x20;
    pub const AUTO_ST_EN: u8 = 0x40;
    pub const CMM_FREQ_EN: u8 = 0x80;
}

// Control Register 1 bits
mod ctrl1 {
    pub const BW_MASK: u8 = 0x03;
    pub const SW_RESET: u8 = 0x80;
}

// Control Register 2 bits
mod ctrl2 {
    pub const CMM_EN: u8 = 0x10;
    pub const EN_PRD_SET: u8 = 0x08;
    pub const HPOWER: u8 = 0x80;
}

// Status1 bits
mod status1 {
    pub const MEAS_M_DONE: u8 = 0x40;
    pub const MEAS_T_DONE: u8 = 0x80;
    pub const OTP_READ_DONE: u8 = 0x10;
}

/// Bandwidth / measurement duration setting
#[derive(Clone, Copy, Default)]
pub enum Bandwidth {
    /// 6.6ms measurement, 75Hz max ODR, lowest noise
    #[default]
    Bw00 = 0,
    /// 3.5ms measurement, 150Hz max ODR
    Bw01 = 1,
    /// 2.0ms measurement, 255Hz max ODR
    Bw10 = 2,
    /// 1.2ms measurement, 255Hz (1000Hz with hpower) max ODR
    Bw11 = 3,
}

/// 3-axis magnetic field reading in Gauss
#[derive(Clone, Copy, Default)]
pub struct MagReading {
    /// X-axis field in Gauss
    pub x: f32,
    /// Y-axis field in Gauss
    pub y: f32,
    /// Z-axis field in Gauss
    pub z: f32,
}

/// MMC5603NJ magnetometer driver
pub struct Mmc5603nj {
    i2c: I2cDevice<'static>,
    bandwidth: Bandwidth,
}

impl Mmc5603nj {
    /// Create and initialize a new MMC5603NJ sensor on the shared I2C bus.
    ///
    /// Verifies the product ID and performs an initial SET operation.
    pub async fn new(i2c_bus: &'static SharedI2c) -> Self {
        let i2c = SharedI2cDevice::new(i2c_bus);
        let mut sensor = Self {
            i2c,
            bandwidth: Bandwidth::default(),
        };

        // Wait for device power-up (datasheet: 5ms min after VDD valid)
        Timer::after_millis(10).await;

        // Verify product ID
        let id = sensor.read_reg(reg::PRODUCT_ID).await;
        assert!(
            id == PRODUCT_ID,
            "MMC5603NJ: unexpected product ID 0x{:02X}, expected 0x{:02X}",
            id,
            PRODUCT_ID,
        );

        // Software reset to known state
        sensor.write_reg(reg::CTRL1, ctrl1::SW_RESET).await;
        Timer::after_millis(25).await;

        // Set bandwidth
        sensor
            .write_reg(reg::CTRL1, sensor.bandwidth as u8 & ctrl1::BW_MASK)
            .await;

        // Perform initial SET for optimal sensor conditioning
        sensor.set().await;

        log::info!("MMC5603NJ magnetometer initialized successfully");

        sensor
    }

    /// Perform a SET operation to condition the sensor.
    pub async fn set(&mut self) {
        self.write_reg(reg::CTRL0, ctrl0::DO_SET).await;
        Timer::after_millis(1).await; // tSR minimum 1ms
    }

    /// Perform a RESET operation.
    pub async fn reset(&mut self) {
        self.write_reg(reg::CTRL0, ctrl0::DO_RESET).await;
        Timer::after_millis(1).await;
    }

    /// Take a single magnetic field measurement (20-bit, with auto SET/RESET).
    ///
    /// Returns the raw 20-bit unsigned values for X, Y, Z converted to Gauss.
    /// The sensor output is unsigned with a null-field value of 524288 counts (20-bit).
    /// Sensitivity at 20-bit is 16384 counts/G.
    pub async fn read_magnetic(&mut self) -> MagReading {
        // Trigger measurement with auto SET/RESET
        self.write_reg(reg::CTRL0, ctrl0::TAKE_MEAS_M | ctrl0::AUTO_SR_EN)
            .await;

        // Poll for measurement complete
        self.wait_meas_m_done().await;

        // Read all 9 bytes (Xout0..Zout2) in one burst
        let mut buf = [0u8; 9];
        self.read_regs(reg::XOUT0, &mut buf).await;

        let x_raw = ((buf[0] as u32) << 12) | ((buf[1] as u32) << 4) | ((buf[6] as u32) >> 4);
        let y_raw = ((buf[2] as u32) << 12) | ((buf[3] as u32) << 4) | ((buf[7] as u32) >> 4);
        let z_raw = ((buf[4] as u32) << 12) | ((buf[5] as u32) << 4) | ((buf[8] as u32) >> 4);

        // Convert to Gauss: (raw - 524288) / 16384.0
        const NULL_FIELD: f32 = 524288.0;
        const SENSITIVITY: f32 = 16384.0;

        MagReading {
            x: (x_raw as f32 - NULL_FIELD) / SENSITIVITY,
            y: (y_raw as f32 - NULL_FIELD) / SENSITIVITY,
            z: (z_raw as f32 - NULL_FIELD) / SENSITIVITY,
        }
    }

    /// Read the on-chip temperature sensor.
    ///
    /// Returns temperature in degrees Celsius.
    /// Range: -75 to +125 C, ~0.8 C/LSB, 0x00 = -75C.
    pub async fn read_temperature(&mut self) -> f32 {
        self.write_reg(reg::CTRL0, ctrl0::TAKE_MEAS_T).await;
        self.wait_meas_t_done().await;

        let raw = self.read_reg(reg::TOUT).await;
        -75.0 + (raw as f32) * 0.8
    }

    /// Read magnetic field data directly into a Packet.
    pub async fn read_into_packet(&mut self, packet: &mut crate::packet::Packet) {
        let mag = self.read_magnetic().await;
        packet.mag_x = mag.x;
        packet.mag_y = mag.y;
        packet.mag_z = mag.z;
    }

    /// Start continuous-mode measurements at the given ODR (1..=255 Hz).
    ///
    /// With auto SET/RESET enabled. Use `read_magnetic_continuous` to read latest data.
    pub async fn start_continuous(&mut self, odr: u8) {
        assert!(odr > 0, "ODR must be non-zero for continuous mode");

        // Set ODR
        self.write_reg(reg::ODR, odr).await;

        // Enable auto SET/RESET and start frequency calculation
        self.write_reg(reg::CTRL0, ctrl0::AUTO_SR_EN | ctrl0::CMM_FREQ_EN)
            .await;

        // Enable continuous mode with periodic set
        self.write_reg(reg::CTRL2, ctrl2::CMM_EN | ctrl2::EN_PRD_SET)
            .await;
    }

    /// Stop continuous-mode measurements.
    pub async fn stop_continuous(&mut self) {
        self.write_reg(reg::CTRL2, 0x00).await;
        self.write_reg(reg::ODR, 0x00).await;
    }

    /// Set the measurement bandwidth.
    pub async fn set_bandwidth(&mut self, bw: Bandwidth) {
        self.bandwidth = bw;
        self.write_reg(reg::CTRL1, bw as u8 & ctrl1::BW_MASK).await;
    }

    // --- private helpers ---

    async fn wait_meas_m_done(&mut self) {
        for _ in 0..100 {
            let status = self.read_reg(reg::STATUS1).await;
            if status & status1::MEAS_M_DONE != 0 {
                return;
            }
            Timer::after_millis(1).await;
        }
        log::warn!("MMC5603NJ: timed out waiting for magnetic measurement");
    }

    async fn wait_meas_t_done(&mut self) {
        for _ in 0..100 {
            let status = self.read_reg(reg::STATUS1).await;
            if status & status1::MEAS_T_DONE != 0 {
                return;
            }
            Timer::after_millis(1).await;
        }
        log::warn!("MMC5603NJ: timed out waiting for temperature measurement");
    }

    async fn read_reg(&mut self, reg: u8) -> u8 {
        let mut buf = [0u8; 1];
        self.i2c
            .write_read(I2C_ADDR, &[reg], &mut buf)
            .await
            .expect("MMC5603NJ: I2C read failed");
        buf[0]
    }

    async fn read_regs(&mut self, start_reg: u8, buf: &mut [u8]) {
        self.i2c
            .write_read(I2C_ADDR, &[start_reg], buf)
            .await
            .expect("MMC5603NJ: I2C burst read failed");
    }

    async fn write_reg(&mut self, reg: u8, value: u8) {
        self.i2c
            .write(I2C_ADDR, &[reg, value])
            .await
            .expect("MMC5603NJ: I2C write failed");
    }
}
