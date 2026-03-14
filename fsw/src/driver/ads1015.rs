use crate::module::{I2cDevice, SharedI2c};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice as SharedI2cDevice;
use embassy_time::{Duration, Timer};
use embedded_hal_async::i2c::I2c;

use crate::constants;
use crate::packet::Packet;

// Register addresses
const REG_CONVERSION: u8 = 0x00;
const REG_CONFIG: u8 = 0x01;

// Config register bit fields
// OS[15]: 1 = start single-shot conversion
const CFG_OS_START: u16 = 0x8000;
// OS[15]: 1 = conversion complete (when reading)
const CFG_OS_READY: u16 = 0x8000;

// MUX[14:12]: single-ended channel selection
const CFG_MUX_AIN0: u16 = 0x4000; // AIN0 vs GND
const CFG_MUX_AIN1: u16 = 0x5000; // AIN1 vs GND
const CFG_MUX_AIN2: u16 = 0x6000; // AIN2 vs GND
const CFG_MUX_AIN3: u16 = 0x7000; // AIN3 vs GND

// PGA[11:9]: ±4.096V full-scale range
const CFG_PGA_4V: u16 = 0x0200;

// MODE[8]: single-shot mode
const CFG_MODE_SINGLE: u16 = 0x0100;

// DR[7:5]: 1600 SPS
const CFG_DR_1600: u16 = 0x0080;

// COMP_QUE[1:0]: disable comparator
const CFG_COMP_DISABLE: u16 = 0x0003;

/// Base config: single-shot, ±4.096V, 1600 SPS, comparator disabled
/// MUX bits are OR'd in per-channel before writing
const CFG_BASE: u16 = CFG_OS_START | CFG_PGA_4V | CFG_MODE_SINGLE | CFG_DR_1600 | CFG_COMP_DISABLE;

/// MUX values indexed by channel number (0, 1, 2, 3)
const CHANNEL_MUX: [u16; 4] = [CFG_MUX_AIN0, CFG_MUX_AIN1, CFG_MUX_AIN2, CFG_MUX_AIN3];

/// Maximum time to wait for a conversion to complete
const CONVERSION_TIMEOUT_MS: u64 = 10;

/// ADS1015 ADC errors
#[derive(Debug)]
pub enum Ads1015Error<E> {
    I2c(E),
    ConversionTimeout,
    InvalidChannel,
}

impl<E> From<E> for Ads1015Error<E> {
    fn from(error: E) -> Self {
        Ads1015Error::I2c(error)
    }
}

/// ADS1015 12-bit ADC driver
pub struct Ads1015Sensor {
    pub i2c: I2cDevice<'static>,
}

impl Ads1015Sensor {
    pub async fn new(i2c_bus: &'static SharedI2c) -> Self {
        let mut sensor = Self {
            i2c: SharedI2cDevice::new(i2c_bus),
        };

        match sensor.init().await {
            Ok(_) => log::info!("ADS1015 initialized at address {:#04X}", constants::ADS1015_I2C_ADDR),
            Err(_) => log::error!("Failed to initialize ADS1015"),
        }

        sensor
    }

    /// Verify communication by reading the config register default value
    async fn init(&mut self) -> Result<(), Ads1015Error<<I2cDevice<'static> as embedded_hal_async::i2c::ErrorType>::Error>> {
        let config = self.read_register(REG_CONFIG).await?;
        log::info!("ADS1015: Config register = {:#06X}", config);
        Ok(())
    }

    /// Write a 16-bit value to a register
    async fn write_register(&mut self, register: u8, value: u16) -> Result<(), Ads1015Error<<I2cDevice<'static> as embedded_hal_async::i2c::ErrorType>::Error>> {
        let msb = (value >> 8) as u8;
        let lsb = (value & 0xFF) as u8;
        self.i2c.write(constants::ADS1015_I2C_ADDR, &[register, msb, lsb]).await?;
        Ok(())
    }

    /// Read a 16-bit value from a register
    async fn read_register(&mut self, register: u8) -> Result<u16, Ads1015Error<<I2cDevice<'static> as embedded_hal_async::i2c::ErrorType>::Error>> {
        let mut buf = [0u8; 2];
        self.i2c.write_read(constants::ADS1015_I2C_ADDR, &[register], &mut buf).await?;
        Ok(((buf[0] as u16) << 8) | buf[1] as u16)
    }

    /// Read a single ADC channel (0, 1, 2, or 3) in single-shot mode.
    /// Returns the raw 12-bit signed value.
    pub async fn read_channel(&mut self, channel: u8) -> Result<i16, Ads1015Error<<I2cDevice<'static> as embedded_hal_async::i2c::ErrorType>::Error>> {
        if channel > 3 {
            return Err(Ads1015Error::InvalidChannel);
        }

        // Build config: base | channel MUX
        let config = CFG_BASE | CHANNEL_MUX[channel as usize];

        // Write config to start conversion
        self.write_register(REG_CONFIG, config).await?;

        // Poll for conversion complete (OS bit = 1)
        let mut elapsed_ms: u64 = 0;
        loop {
            Timer::after(Duration::from_millis(1)).await;
            elapsed_ms += 1;

            let status = self.read_register(REG_CONFIG).await?;
            if status & CFG_OS_READY != 0 {
                break;
            }

            if elapsed_ms >= CONVERSION_TIMEOUT_MS {
                return Err(Ads1015Error::ConversionTimeout);
            }
        }

        // Read conversion result (16-bit, 12-bit data in bits [15:4])
        let raw = self.read_register(REG_CONVERSION).await?;

        // Right-shift by 4 to get the 12-bit value (sign-extending)
        let value = (raw as i16) >> 4;

        Ok(value)
    }

    /// Read all 4 channels into the packet (ch0-ch2) and return all raw values.
    /// Returns `[ch0, ch1, ch2, ch3]` raw 12-bit values.
    pub async fn read_into_packet(&mut self, packet: &mut Packet) -> Result<[i16; 4], Ads1015Error<<I2cDevice<'static> as embedded_hal_async::i2c::ErrorType>::Error>> {
        let raw_ch0 = self.read_channel(0).await?;
        let raw_ch1 = self.read_channel(1).await?;
        let raw_ch2 = self.read_channel(2).await?;
        let raw_ch3 = self.read_channel(3).await?;

        // Store raw values in packet (ch0-ch2 only, packet unchanged)
        packet.pt3 = raw_ch0 as f32;
        packet.pt4 = raw_ch1 as f32;
        packet.rtd = raw_ch2 as f32;

        Ok([raw_ch0, raw_ch1, raw_ch2, raw_ch3])
    }
}
