// Pin definitions (for reference - HAL uses compile-time pin types)
pub const LED_PIN: u8 = 25;
pub const I2C_SDA_PIN: u8 = 0;
pub const I2C_SCL_PIN: u8 = 1;

// I2C configuration
pub const I2C_FREQ_KHZ: u32 = 400;

// Clock configuration
pub const XOSC_FREQ_HZ: u32 = 12_000_000;

// Watchdog timeout (milliseconds)
pub const WATCHDOG_TIMEOUT_MS: u32 = 500;
