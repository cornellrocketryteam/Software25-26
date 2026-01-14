/// ADS1015 I2C ADC Driver for Fill Station
///
/// This module provides a driver for the TI ADS1015 12-bit ADC over I2C.
/// The ADS1015 supports 4 single-ended or 2 differential inputs with
/// programmable gain and data rates up to 3300 SPS.
///
/// Example usage:
/// ```no_run
/// use fill_station::components::ads1015::{Ads1015, Channel, Gain, DataRate};
///
/// let mut adc = Ads1015::new("/dev/i2c-2", 0x48)?;
/// let voltage = adc.read_voltage(Channel::Ain0, Gain::TwoThirds)?;
/// ```

use anyhow::Result;

#[cfg(any(target_os = "linux", target_os = "android"))]
use anyhow::Context;
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::thread;
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::time::Duration;

#[cfg(any(target_os = "linux", target_os = "android"))]
use i2cdev::core::I2CDevice;
#[cfg(any(target_os = "linux", target_os = "android"))]
use i2cdev::linux::LinuxI2CDevice;

// ADS1015 Register Addresses
#[cfg(any(target_os = "linux", target_os = "android"))]
const REG_CONVERSION: u8 = 0x00;
#[cfg(any(target_os = "linux", target_os = "android"))]
const REG_CONFIG: u8 = 0x01;

// Configuration Register Bits
#[cfg(any(target_os = "linux", target_os = "android"))]
const OS_SINGLE: u16 = 0x8000;  // Start single conversion
#[cfg(any(target_os = "linux", target_os = "android"))]
const MUX_SHIFT: u16 = 12;       // Multiplexer shift amount
#[cfg(any(target_os = "linux", target_os = "android"))]
const PGA_SHIFT: u16 = 9;        // PGA shift amount
#[cfg(any(target_os = "linux", target_os = "android"))]
const MODE_SINGLE: u16 = 0x0100; // Single-shot mode
#[cfg(any(target_os = "linux", target_os = "android"))]
const DR_SHIFT: u16 = 5;         // Data rate shift amount
#[cfg(any(target_os = "linux", target_os = "android"))]
const COMP_QUE_DISABLE: u16 = 0x0003; // Disable comparator

/// ADC input channel selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    /// Single-ended input on AIN0
    Ain0 = 0b100,
    /// Single-ended input on AIN1
    Ain1 = 0b101,
    /// Single-ended input on AIN2
    Ain2 = 0b110,
    /// Single-ended input on AIN3
    Ain3 = 0b111,
    /// Differential input: AIN0 - AIN1
    Diff0_1 = 0b000,
    /// Differential input: AIN0 - AIN3
    Diff0_3 = 0b001,
    /// Differential input: AIN1 - AIN3
    Diff1_3 = 0b010,
    /// Differential input: AIN2 - AIN3
    Diff2_3 = 0b011,
}

/// Programmable gain amplifier settings
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gain {
    /// ±6.144V range (2/3x gain)
    TwoThirds = 0b000,
    /// ±4.096V range (1x gain)
    One = 0b001,
    /// ±2.048V range (2x gain)
    Two = 0b010,
    /// ±1.024V range (4x gain)
    Four = 0b011,
    /// ±0.512V range (8x gain)
    Eight = 0b100,
    /// ±0.256V range (16x gain)
    Sixteen = 0b101,
}

impl Gain {
    /// Get the voltage range for this gain setting
    pub fn voltage_range(&self) -> f32 {
        match self {
            Gain::TwoThirds => 6.144,
            Gain::One => 4.096,
            Gain::Two => 2.048,
            Gain::Four => 1.024,
            Gain::Eight => 0.512,
            Gain::Sixteen => 0.256,
        }
    }

    /// Calculate the LSB size in volts for 12-bit resolution
    pub fn lsb_size(&self) -> f32 {
        self.voltage_range() / 2048.0 // ADS1015 is 12-bit (2^11 = 2048 for signed)
    }
}

/// Data rate (samples per second)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataRate {
    /// 128 samples per second
    Sps128 = 0b000,
    /// 250 samples per second
    Sps250 = 0b001,
    /// 490 samples per second
    Sps490 = 0b010,
    /// 920 samples per second
    Sps920 = 0b011,
    /// 1600 samples per second (default)
    Sps1600 = 0b100,
    /// 2400 samples per second
    Sps2400 = 0b101,
    /// 3300 samples per second (max)
    Sps3300 = 0b110,
}

impl DataRate {
    /// Get conversion time in milliseconds
    pub fn conversion_time_ms(&self) -> u64 {
        match self {
            DataRate::Sps128 => 8,
            DataRate::Sps250 => 4,
            DataRate::Sps490 => 3,
            DataRate::Sps920 => 2,
            DataRate::Sps1600 => 1,
            DataRate::Sps2400 => 1,
            DataRate::Sps3300 => 1,
        }
    }

    /// Get conversion time in microseconds
    /// This provides higher precision for high sample rates
    pub fn conversion_time_us(&self) -> u64 {
        match self {
            DataRate::Sps128 => 8000,
            DataRate::Sps250 => 4000,
            DataRate::Sps490 => 2041, // 1/490s ≈ 2.04ms
            DataRate::Sps920 => 1087, // 1/920s ≈ 1.08ms
            DataRate::Sps1600 => 625, // 1/1600s = 0.625ms
            DataRate::Sps2400 => 417, // 1/2400s ≈ 0.417ms
            DataRate::Sps3300 => 303, // 1/3300s ≈ 0.303ms
        }
    }
}

/// ADS1015 12-bit ADC driver
#[cfg(any(target_os = "linux", target_os = "android"))]
pub struct Ads1015 {
    device: LinuxI2CDevice,
    address: u16,
}

#[cfg(any(target_os = "linux", target_os = "android"))]
impl Ads1015 {
    /// Create a new ADS1015 instance
    ///
    /// # Arguments
    /// * `bus` - I2C bus path (e.g., "/dev/i2c-2")
    /// * `address` - I2C address (typically 0x48, 0x49, 0x4A, or 0x4B)
    ///
    /// # Example
    /// ```no_run
    /// let adc = Ads1015::new("/dev/i2c-2", 0x48)?;
    /// ```
    pub fn new(bus: &str, address: u16) -> Result<Self> {
        let device = LinuxI2CDevice::new(bus, address)
            .with_context(|| format!("Failed to open I2C device {} at address 0x{:02X}", bus, address))?;
        
        Ok(Self { device, address })
    }

    /// Get the I2C address of this device
    pub fn address(&self) -> u16 {
        self.address
    }

    /// Read raw ADC value (12-bit signed)
    ///
    /// Returns the raw conversion value from -2048 to 2047
    pub fn read_raw(&mut self, channel: Channel, gain: Gain, data_rate: DataRate) -> Result<i16> {
        // Build configuration register value
        let config = OS_SINGLE                              // Start single conversion
            | ((channel as u16) << MUX_SHIFT)              // Set input multiplexer
            | ((gain as u16) << PGA_SHIFT)                 // Set gain
            | MODE_SINGLE                                   // Single-shot mode
            | ((data_rate as u16) << DR_SHIFT)             // Set data rate
            | COMP_QUE_DISABLE;                            // Disable comparator

        // Write configuration as 16-bit word to start conversion
        // ADS1015 expects MSB first (big-endian)
        self.device.smbus_write_word_data(REG_CONFIG, config.swap_bytes())
            .context("Failed to write config register")?;

        // Wait for conversion to complete
        // Use microsecond precision for higher sampling rates
        thread::sleep(Duration::from_micros(data_rate.conversion_time_us()));

        // Read conversion result as 16-bit word
        // ADS1015 returns MSB first, but smbus_read_word_data expects little-endian
        let raw_word = self.device.smbus_read_word_data(REG_CONVERSION)
            .context("Failed to read conversion register")?;
        
        // Swap bytes back to big-endian and shift right by 4 
        // (ADS1015 is 12-bit, left-aligned in 16 bits)
        let raw = (raw_word.swap_bytes() as i16) >> 4;
        
        Ok(raw)
    }

    /// Read raw ADC value with averaging
    ///
    /// # Arguments
    /// * `channel` - ADC channel to read
    /// * `gain` - Gain setting
    /// * `data_rate` - Data rate setting
    /// * `samples` - Number of samples to average
    ///
    /// Returns the averaged raw value
    pub fn read_raw_averaged(&mut self, channel: Channel, gain: Gain, data_rate: DataRate, samples: usize) -> Result<i16> {
        if samples == 0 {
            return Ok(0);
        }
        if samples == 1 {
            return self.read_raw(channel, gain, data_rate);
        }

        let mut sum: i32 = 0;
        for _ in 0..samples {
            sum += self.read_raw(channel, gain, data_rate)? as i32;
        }

        Ok((sum / samples as i32) as i16)
    }

    /// Read voltage from the specified channel
    ///
    /// # Arguments
    /// * `channel` - ADC channel to read
    /// * `gain` - Gain setting (determines voltage range)
    ///
    /// Returns the measured voltage
    pub fn read_voltage(&mut self, channel: Channel, gain: Gain) -> Result<f32> {
        self.read_voltage_with_rate(channel, gain, DataRate::Sps1600)
    }

    /// Read voltage with specified data rate
    pub fn read_voltage_with_rate(&mut self, channel: Channel, gain: Gain, data_rate: DataRate) -> Result<f32> {
        let raw = self.read_raw(channel, gain, data_rate)?;
        let voltage = (raw as f32) * gain.lsb_size();
        Ok(voltage)
    }

    /// Check if the ADC is ready (conversion complete)
    ///
    /// Returns true if a conversion is complete and data is ready to read
    pub fn is_ready(&mut self) -> Result<bool> {
        let config = self.device.smbus_read_word_data(REG_CONFIG)
            .context("Failed to read config register")?;
        
        // Swap bytes to get big-endian, then check OS bit (bit 15)
        // OS bit is 1 when conversion is complete
        let config_be = config.swap_bytes();
        Ok((config_be & OS_SINGLE) != 0)
    }
}

/// Stub implementation for non-Linux platforms
#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub struct Ads1015 {
    _address: u16,
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
impl Ads1015 {
    pub fn new(_bus: &str, address: u16) -> Result<Self> {
        Ok(Self { _address: address })
    }

    pub fn address(&self) -> u16 {
        self._address
    }

    pub fn read_raw(&mut self, _channel: Channel, _gain: Gain, _data_rate: DataRate) -> Result<i16> {
        anyhow::bail!("ADS1015 is only supported on Linux/Android")
    }

    pub fn read_voltage(&mut self, _channel: Channel, _gain: Gain) -> Result<f32> {
        anyhow::bail!("ADS1015 is only supported on Linux/Android")
    }

    pub fn read_voltage_with_rate(&mut self, _channel: Channel, _gain: Gain, _data_rate: DataRate) -> Result<f32> {
        anyhow::bail!("ADS1015 is only supported on Linux/Android")
    }

    pub fn read_raw_averaged(&mut self, _channel: Channel, _gain: Gain, _data_rate: DataRate, _samples: usize) -> Result<i16> {
        anyhow::bail!("ADS1015 is only supported on Linux/Android")
    }

    pub fn is_ready(&mut self) -> Result<bool> {
        anyhow::bail!("ADS1015 is only supported on Linux/Android")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gain_voltage_ranges() {
        assert_eq!(Gain::TwoThirds.voltage_range(), 6.144);
        assert_eq!(Gain::One.voltage_range(), 4.096);
        assert_eq!(Gain::Two.voltage_range(), 2.048);
    }

    #[test]
    fn test_lsb_calculation() {
        // ADS1015 is 12-bit, so 2048 steps per polarity
        assert!((Gain::One.lsb_size() - 0.002).abs() < 0.0001);
    }

    #[test]
    fn test_data_rate_timing() {
        assert_eq!(DataRate::Sps128.conversion_time_ms(), 8);
        assert_eq!(DataRate::Sps3300.conversion_time_ms(), 1);
        
        assert_eq!(DataRate::Sps128.conversion_time_us(), 8000);
        assert_eq!(DataRate::Sps3300.conversion_time_us(), 303);
    }
}
